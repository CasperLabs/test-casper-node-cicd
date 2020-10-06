//! Consensus service is a component that will be communicating with the reactor.
//! It will receive events (like incoming message event or create new message event)
//! and propagate them to the underlying consensus protocol.
//! It tries to know as little as possible about the underlying consensus. The only thing
//! it assumes is the concept of era/epoch and that each era runs separate consensus instance.
//! Most importantly, it doesn't care about what messages it's forwarding.

use std::{
    collections::HashMap,
    convert::TryInto,
    fmt::{self, Debug, Formatter},
    rc::Rc,
};

use anyhow::Error;
use blake2::{
    digest::{Input, VariableOutput},
    VarBlake2b,
};
use datasize::DataSize;
use fmt::Display;
use num_traits::AsPrimitive;
use rand::Rng;
use serde::{Deserialize, Serialize};
use tracing::{error, info, trace, warn};

use casper_execution_engine::{
    core::engine_state::era_validators::GetEraValidatorsRequest, shared::motes::Motes,
};
use casper_types::{
    auction::{ValidatorWeights, BLOCK_REWARD},
    ProtocolVersion, U512,
};

use crate::{
    components::{
        chainspec_loader::{Chainspec, HighwayConfig},
        consensus::{
            consensus_protocol::{
                BlockContext, ConsensusProtocol, ConsensusProtocolResult,
                FinalizedBlock as CpFinalizedBlock,
            },
            highway_core::{highway::Params, validators::Validators},
            protocols::highway::{HighwayContext, HighwayProtocol, HighwaySecret},
            traits::NodeIdT,
            Config, ConsensusMessage, Event, ReactorEventT,
        },
    },
    crypto::{
        asymmetric_key::{self, PublicKey, SecretKey, Signature},
        hash,
    },
    effect::{EffectBuilder, EffectExt, Effects, Responder},
    types::{BlockHeader, CryptoRngCore, FinalizedBlock, ProtoBlock, Timestamp},
    utils::WithDir,
};

/// The number of recent eras to retain. Eras older than this are dropped from memory.
// TODO: This needs to be in sync with AUCTION_DELAY/booking_duration_millis. (Already duplicated!)
const RETAIN_ERAS: u64 = 4;

#[derive(
    DataSize, Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct EraId(pub(crate) u64);

impl EraId {
    fn message(self, payload: Vec<u8>) -> ConsensusMessage {
        ConsensusMessage {
            era_id: self,
            payload,
        }
    }

    pub(crate) fn successor(self) -> EraId {
        EraId(self.0 + 1)
    }
}

impl Display for EraId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

pub struct Era<I> {
    /// The consensus protocol instance.
    consensus: Box<dyn ConsensusProtocol<I, ProtoBlock, PublicKey>>,
    /// The height of this era's first block.
    start_height: u64,
}

impl<I> DataSize for Era<I>
where
    I: 'static,
{
    const IS_DYNAMIC: bool = true;

    const STATIC_HEAP_SIZE: usize = 0;

    #[inline]
    fn estimate_heap_size(&self) -> usize {
        // `DataSize` cannot be made object safe due its use of associated constants. We implement
        // it manually here, downcasting the consensus protocol as a workaround.

        let consensus_heap_size = {
            let any_ref = self.consensus.as_any();

            if let Some(highway) = any_ref.downcast_ref::<HighwayProtocol<I, HighwayContext>>() {
                highway.estimate_heap_size()
            } else {
                warn!(
                    "could not downcast consensus protocol to HighwayProtocol<I, HighwayContext> to determine heap allocation size"
                );
                0
            }
        };

        consensus_heap_size + self.start_height.estimate_heap_size()
    }
}

#[derive(DataSize)]
pub struct EraSupervisor<I> {
    /// A map of active consensus protocols.
    /// A value is a trait so that we can run different consensus protocol instances per era.
    active_eras: HashMap<EraId, Era<I>>,
    pub(super) secret_signing_key: Rc<SecretKey>,
    pub(super) public_signing_key: PublicKey,
    current_era: EraId,
    chainspec: Chainspec,
    node_start_time: Timestamp,
}

impl<I> Debug for EraSupervisor<I> {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        let ae: Vec<_> = self.active_eras.keys().collect();
        write!(formatter, "EraSupervisor {{ active_eras: {:?}, .. }}", ae)
    }
}

impl<I> EraSupervisor<I>
where
    I: NodeIdT,
{
    /// Creates a new `EraSupervisor`, starting in era 0.
    pub(crate) fn new<REv: ReactorEventT<I>>(
        timestamp: Timestamp,
        config: WithDir<Config>,
        effect_builder: EffectBuilder<REv>,
        validator_stakes: Vec<(PublicKey, Motes)>,
        chainspec: &Chainspec,
        genesis_post_state_hash: hash::Digest,
        mut rng: &mut dyn CryptoRngCore,
    ) -> Result<(Self, Effects<Event<I>>), Error> {
        let (root, config) = config.into_parts();
        let secret_signing_key = Rc::new(config.secret_key_path.load(root)?);
        let public_signing_key = PublicKey::from(secret_signing_key.as_ref());

        let mut era_supervisor = Self {
            active_eras: Default::default(),
            secret_signing_key,
            public_signing_key,
            current_era: EraId(0),
            chainspec: chainspec.clone(),
            node_start_time: Timestamp::now(),
        };

        let results = era_supervisor.new_era(
            EraId(0),
            timestamp,
            validator_stakes,
            chainspec.genesis.highway_config.genesis_era_start_timestamp,
            0,
            genesis_post_state_hash,
        );
        let effects = era_supervisor
            .handling_wrapper(effect_builder, &mut rng)
            .handle_consensus_results(EraId(0), results);

        Ok((era_supervisor, effects))
    }

    /// Returns a temporary container with this `EraSupervisor`, `EffectBuilder` and random number
    /// generator, for handling events.
    pub(super) fn handling_wrapper<'a, REv: ReactorEventT<I>>(
        &'a mut self,
        effect_builder: EffectBuilder<REv>,
        rng: &'a mut dyn CryptoRngCore,
    ) -> EraSupervisorHandlingWrapper<'a, I, REv> {
        EraSupervisorHandlingWrapper {
            era_supervisor: self,
            effect_builder,
            rng,
        }
    }

    fn highway_config(&self) -> HighwayConfig {
        self.chainspec.genesis.highway_config
    }

    fn instance_id(&self, post_state_hash: hash::Digest, block_height: u64) -> hash::Digest {
        let mut result = [0; hash::Digest::LENGTH];
        let mut hasher = VarBlake2b::new(hash::Digest::LENGTH).expect("should create hasher");

        hasher.input(&self.chainspec.genesis.name);
        hasher.input(self.chainspec.genesis.timestamp.millis().to_le_bytes());
        hasher.input(post_state_hash);

        for upgrade_point in self
            .chainspec
            .upgrades
            .iter()
            .take_while(|up| up.activation_point.rank <= block_height)
        {
            hasher.input(upgrade_point.activation_point.rank.to_le_bytes());
            if let Some(bytes) = upgrade_point.upgrade_installer_bytes.as_ref() {
                hasher.input(bytes);
            }
            if let Some(bytes) = upgrade_point.upgrade_installer_args.as_ref() {
                hasher.input(bytes);
            }
        }

        hasher.variable_result(|slice| {
            result.copy_from_slice(slice);
        });
        result.into()
    }

    /// Starts a new era; panics if it already exists.
    fn new_era(
        &mut self,
        era_id: EraId,
        timestamp: Timestamp,
        validator_stakes: Vec<(PublicKey, Motes)>,
        start_time: Timestamp,
        start_height: u64,
        post_state_hash: hash::Digest,
    ) -> Vec<ConsensusProtocolResult<I, ProtoBlock, PublicKey>> {
        if self.active_eras.contains_key(&era_id) {
            panic!("{:?} already exists", era_id);
        }
        self.current_era = era_id;

        let sum_stakes: Motes = validator_stakes.iter().map(|(_, stake)| *stake).sum();
        assert!(
            !sum_stakes.value().is_zero(),
            "cannot start era with total weight 0"
        );
        info!(
            ?validator_stakes,
            ?start_time,
            ?timestamp,
            ?start_height,
            "starting era {}",
            era_id.0
        );
        // For Highway, we need u64 weights. Scale down by  sum / u64::MAX,  rounded up.
        // If we round up the divisor, the resulting sum is guaranteed to be  <= u64::MAX.
        let scaling_factor = (sum_stakes.value() + U512::from(u64::MAX) - 1) / U512::from(u64::MAX);
        let scale_stake = |(key, stake): (PublicKey, Motes)| {
            (key, AsPrimitive::<u64>::as_(stake.value() / scaling_factor))
        };
        let validators: Validators<PublicKey> =
            validator_stakes.into_iter().map(scale_stake).collect();

        let ftt = validators.total_weight()
            * u64::from(self.highway_config().finality_threshold_percent)
            / 100;
        // TODO: The initial round length should be the observed median of the switch block.
        let params = Params::new(
            0, // TODO: get a proper seed.
            BLOCK_REWARD,
            BLOCK_REWARD / 5, // TODO: Make reduced block reward configurable?
            self.highway_config().minimum_round_exponent,
            self.highway_config().minimum_era_height,
            start_time + self.highway_config().era_duration,
        );

        // Activate the era if this node was already running when the era began, it is still
        // ongoing based on its minimum duration, and we are one of the validators.
        let our_id = self.public_signing_key;
        let era_rounds_len = params.min_round_len() * params.end_height();
        let min_end_time = start_time + self.highway_config().era_duration.max(era_rounds_len);
        let should_activate = self.node_start_time < start_time
            && min_end_time >= timestamp
            && validators.iter().any(|v| *v.id() == our_id);

        let mut highway = HighwayProtocol::<I, HighwayContext>::new(
            self.instance_id(post_state_hash, start_height),
            validators,
            params,
            ftt,
        );

        let results = if should_activate {
            info!("start voting in era {}", era_id.0);
            let secret = HighwaySecret::new(Rc::clone(&self.secret_signing_key), our_id);
            highway.activate_validator(our_id, secret, timestamp.max(start_time))
        } else {
            info!("not voting in era {}", era_id.0);
            if start_time >= self.node_start_time {
                info!(
                    "node was started at time {}, which is not earlier than the era start {}",
                    self.node_start_time, start_time
                );
            } else if min_end_time < timestamp {
                info!(
                    "era started too long ago ({}; earliest end {}), current timestamp {}",
                    start_time, min_end_time, timestamp
                );
            } else {
                info!("not a validator; our ID: {}", our_id);
            }
            Vec::new()
        };

        let era = Era {
            consensus: Box::new(highway),
            start_height,
        };
        let _ = self.active_eras.insert(era_id, era);

        // Remove the era that has become obsolete now.
        if era_id.0 > RETAIN_ERAS {
            self.active_eras.remove(&EraId(era_id.0 - RETAIN_ERAS - 1));
        }

        results
    }

    /// Returns the current era.
    fn current_era_mut(&mut self) -> &mut Era<I> {
        self.active_eras
            .get_mut(&self.current_era)
            .expect("current era does not exist")
    }

    /// Inspect the active eras.
    #[cfg(test)]
    pub(crate) fn active_eras(&self) -> &HashMap<EraId, Era<I>> {
        &self.active_eras
    }
}

/// A mutable `EraSupervisor` reference, together with an `EffectBuilder`.
///
/// This is a short-lived convenience type to avoid passing the effect builder through lots of
/// message calls, and making every method individually generic in `REv`. It is only instantiated
/// for the duration of handling a single event.
pub(super) struct EraSupervisorHandlingWrapper<'a, I, REv: 'static> {
    pub(super) era_supervisor: &'a mut EraSupervisor<I>,
    pub(super) effect_builder: EffectBuilder<REv>,
    pub(super) rng: &'a mut dyn CryptoRngCore,
}

impl<'a, I, REv> EraSupervisorHandlingWrapper<'a, I, REv>
where
    I: NodeIdT,
    REv: ReactorEventT<I>,
{
    /// Applies `f` to the consensus protocol of the specified era.
    fn delegate_to_era<F>(&mut self, era_id: EraId, f: F) -> Effects<Event<I>>
    where
        F: FnOnce(
            &mut dyn ConsensusProtocol<I, ProtoBlock, PublicKey>,
            &mut dyn CryptoRngCore,
        ) -> Result<Vec<ConsensusProtocolResult<I, ProtoBlock, PublicKey>>, Error>,
    {
        match self.era_supervisor.active_eras.get_mut(&era_id) {
            None => {
                if era_id > self.era_supervisor.current_era {
                    info!("received message for future {:?}", era_id);
                } else {
                    info!("received message for obsolete {:?}", era_id);
                }
                Effects::new()
            }
            Some(era) => match f(&mut *era.consensus, self.rng) {
                Ok(results) => self.handle_consensus_results(era_id, results),
                Err(error) => {
                    error!(%error, ?era_id, "got error from era id {:?}: {:?}", era_id, error);
                    Effects::new()
                }
            },
        }
    }

    pub(super) fn handle_timer(
        &mut self,
        era_id: EraId,
        timestamp: Timestamp,
    ) -> Effects<Event<I>> {
        self.delegate_to_era(era_id, move |consensus, rng| {
            consensus.handle_timer(timestamp, rng)
        })
    }

    pub(super) fn handle_message(&mut self, sender: I, msg: ConsensusMessage) -> Effects<Event<I>> {
        let ConsensusMessage { era_id, payload } = msg;
        self.delegate_to_era(era_id, move |consensus, rng| {
            consensus.handle_message(sender, payload, rng)
        })
    }

    pub(super) fn handle_new_proto_block(
        &mut self,
        era_id: EraId,
        proto_block: ProtoBlock,
        block_context: BlockContext,
    ) -> Effects<Event<I>> {
        let mut effects = self
            .effect_builder
            .announce_proposed_proto_block(proto_block.clone())
            .ignore();
        effects.extend(self.delegate_to_era(era_id, move |consensus, rng| {
            consensus.propose(proto_block, block_context, rng)
        }));
        effects
    }

    pub(super) fn handle_linear_chain_block(
        &mut self,
        block_header: BlockHeader,
        responder: Responder<Signature>,
    ) -> Effects<Event<I>> {
        // TODO - we should only sign if we're a validator for the given era ID.
        let signature = asymmetric_key::sign(
            block_header.hash().inner(),
            &self.era_supervisor.secret_signing_key,
            &self.era_supervisor.public_signing_key,
            self.rng,
        );
        let mut effects = responder.respond(signature).ignore();
        if block_header.era_id() < self.era_supervisor.current_era {
            trace!("executed block in old era {}", block_header.era_id().0);
            return effects;
        }
        if block_header.switch_block() {
            // if the block is a switch block, we have to get the validators for the new era and
            // create it, before we can say we handled the block
            let request = GetEraValidatorsRequest::new(
                (*block_header.global_state_hash()).into(),
                block_header.era_id().successor().0,
                ProtocolVersion::V1_0_0,
            );
            effects.extend(self.effect_builder.get_validators(request).event(|result| {
                Event::GetValidatorsResponse {
                    block_header: Box::new(block_header),
                    get_validators_result: result,
                }
            }));
        } else {
            // if it's not a switch block, we can already declare it handled
            effects.extend(
                self.effect_builder
                    .announce_block_handled(block_header)
                    .ignore(),
            );
        }
        effects
    }

    pub(super) fn handle_get_validators_response(
        &mut self,
        block_header: BlockHeader,
        validator_weights: ValidatorWeights,
    ) -> Effects<Event<I>> {
        let validator_stakes = validator_weights
            .into_iter()
            .filter_map(|(key, stake)| match key.try_into() {
                Ok(key) => Some((key, Motes::new(stake))),
                Err(error) => {
                    warn!(%error, "error converting the bonded key: {:?}", error);
                    None
                }
            })
            .collect();
        self.era_supervisor
            .current_era_mut()
            .consensus
            .deactivate_validator();
        let new_era_id = block_header.era_id().successor();
        info!(?new_era_id, "Era created");
        let results = self.era_supervisor.new_era(
            new_era_id,
            Timestamp::now(), // TODO: This should be passed in.
            validator_stakes,
            block_header.timestamp(),
            block_header.height() + 1,
            *block_header.global_state_hash(),
        );
        let mut effects = self.handle_consensus_results(new_era_id, results);
        effects.extend(
            self.effect_builder
                .announce_block_handled(block_header)
                .ignore(),
        );
        effects
    }

    pub(super) fn handle_accept_proto_block(
        &mut self,
        era_id: EraId,
        proto_block: ProtoBlock,
    ) -> Effects<Event<I>> {
        let mut effects = self.delegate_to_era(era_id, |consensus, rng| {
            consensus.resolve_validity(&proto_block, true, rng)
        });
        effects.extend(
            self.effect_builder
                .announce_proposed_proto_block(proto_block)
                .ignore(),
        );
        effects
    }

    pub(super) fn handle_invalid_proto_block(
        &mut self,
        era_id: EraId,
        _sender: I,
        proto_block: ProtoBlock,
    ) -> Effects<Event<I>> {
        self.delegate_to_era(era_id, |consensus, rng| {
            consensus.resolve_validity(&proto_block, false, rng)
        })
    }

    fn handle_consensus_results<T>(&mut self, era_id: EraId, results: T) -> Effects<Event<I>>
    where
        T: IntoIterator<Item = ConsensusProtocolResult<I, ProtoBlock, PublicKey>>,
    {
        results
            .into_iter()
            .flat_map(|result| self.handle_consensus_result(era_id, result))
            .collect()
    }

    fn handle_consensus_result(
        &mut self,
        era_id: EraId,
        consensus_result: ConsensusProtocolResult<I, ProtoBlock, PublicKey>,
    ) -> Effects<Event<I>> {
        match consensus_result {
            ConsensusProtocolResult::InvalidIncomingMessage(_, sender, error) => {
                // TODO: we will probably want to disconnect from the sender here
                error!(
                    %sender,
                    ?error,
                    "invalid incoming message to consensus instance"
                );
                Default::default()
            }
            ConsensusProtocolResult::CreatedGossipMessage(out_msg) => {
                // TODO: we'll want to gossip instead of broadcast here
                self.effect_builder
                    .broadcast_message(era_id.message(out_msg).into())
                    .ignore()
            }
            ConsensusProtocolResult::CreatedTargetedMessage(out_msg, to) => self
                .effect_builder
                .send_message(to, era_id.message(out_msg).into())
                .ignore(),
            ConsensusProtocolResult::ScheduleTimer(timestamp) => {
                let timediff = timestamp.saturating_sub(Timestamp::now());
                self.effect_builder
                    .set_timeout(timediff.into())
                    .event(move |_| Event::Timer { era_id, timestamp })
            }
            ConsensusProtocolResult::CreateNewBlock { block_context } => self
                .effect_builder
                .request_proto_block(block_context, self.rng.gen())
                .event(move |(proto_block, block_context)| Event::NewProtoBlock {
                    era_id,
                    proto_block,
                    block_context,
                }),
            ConsensusProtocolResult::FinalizedBlock(CpFinalizedBlock {
                value: proto_block,
                timestamp,
                height,
                era_end,
                proposer,
            }) => {
                let finalized_block = FinalizedBlock::new(
                    proto_block,
                    timestamp,
                    era_end,
                    era_id,
                    self.era_supervisor.active_eras[&era_id].start_height + height,
                    proposer,
                );
                // Announce the finalized proto block.
                let mut effects = self
                    .effect_builder
                    .announce_finalized_block(finalized_block.clone())
                    .ignore();
                // Request execution of the finalized block.
                effects.extend(self.effect_builder.execute_block(finalized_block).ignore());
                effects
            }
            ConsensusProtocolResult::ValidateConsensusValue(sender, proto_block) => self
                .effect_builder
                .validate_block(sender.clone(), proto_block)
                .event(move |(is_valid, proto_block)| {
                    if is_valid {
                        Event::AcceptProtoBlock {
                            era_id,
                            proto_block,
                        }
                    } else {
                        Event::InvalidProtoBlock {
                            era_id,
                            sender,
                            proto_block,
                        }
                    }
                }),
        }
    }
}
