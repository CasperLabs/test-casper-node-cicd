//! API server
//!
//! The API server provides clients with two types of service: a JSON-RPC API for querying state and
//! sending commands to the node, and an event-stream returning Server-Sent Events (SSEs) holding
//! JSON-encoded data.
//!
//! The actual server is run in backgrounded tasks.   RPCs requests are translated into reactor
//! requests to various components.
//!
//! This module currently provides both halves of what is required for an API server: An abstract
//! API Server that handles API requests and an external service endpoint based on HTTP.
//!
//! For the list of supported RPCs and SSEs, see
//! https://github.com/CasperLabs/ceps/blob/master/text/0009-client-api.md#rpcs

mod config;
mod event;
mod http_server;
mod rest_server;
pub mod rpcs;
mod sse_server;

use std::fmt::Debug;

use datasize::DataSize;
use futures::join;
use lazy_static::lazy_static;
use semver::Version;
use tokio::sync::mpsc::{self, UnboundedSender};
use tracing::info;

use casper_execution_engine::core::engine_state::{
    self, BalanceRequest, BalanceResult, QueryRequest, QueryResult,
};
use casper_types::{Key, URef};

use super::Component;
use crate::{
    components::storage::Storage,
    crypto::hash::Digest,
    effect::{
        announcements::ApiServerAnnouncement,
        requests::{
            ApiRequest, ChainspecLoaderRequest, ContractRuntimeRequest, LinearChainRequest,
            MetricsRequest, NetworkInfoRequest, StorageRequest,
        },
        EffectBuilder, EffectExt, Effects, Responder,
    },
    small_network::NodeId,
    types::{CryptoRngCore, StatusFeed},
};
pub use config::Config;
pub(crate) use event::Event;
pub use sse_server::SseData;

// TODO - confirm if we want to use the protocol version for this.
lazy_static! {
    static ref CLIENT_API_VERSION: Version = Version::new(1, 0, 0);
}

/// A helper trait whose bounds represent the requirements for a reactor event that `run_server` can
/// work with.
trait ReactorEventT:
    From<Event>
    + From<ApiRequest<NodeId>>
    + From<StorageRequest<Storage>>
    + From<LinearChainRequest<NodeId>>
    + From<ContractRuntimeRequest>
    + Send
{
}

impl<REv> ReactorEventT for REv where
    REv: From<Event>
        + From<ApiRequest<NodeId>>
        + From<StorageRequest<Storage>>
        + From<LinearChainRequest<NodeId>>
        + From<ContractRuntimeRequest>
        + Send
        + 'static
{
}

#[derive(DataSize, Debug)]
pub(crate) struct ApiServer {
    /// Channel sender to pass event-stream data to the event-stream server.
    // TODO - this should not be skipped.  Awaiting support for `UnboundedSender` in datasize crate.
    #[data_size(skip)]
    sse_data_sender: UnboundedSender<SseData>,
}

impl ApiServer {
    pub(crate) fn new<REv>(config: Config, effect_builder: EffectBuilder<REv>) -> Self
    where
        REv: From<Event>
            + From<ApiRequest<NodeId>>
            + From<StorageRequest<Storage>>
            + From<LinearChainRequest<NodeId>>
            + From<ContractRuntimeRequest>
            + Send,
    {
        let (sse_data_sender, sse_data_receiver) = mpsc::unbounded_channel();
        tokio::spawn(http_server::run(config, effect_builder, sse_data_receiver));

        ApiServer { sse_data_sender }
    }
}

impl ApiServer {
    fn handle_query<REv: ReactorEventT>(
        &mut self,
        effect_builder: EffectBuilder<REv>,
        global_state_hash: Digest,
        base_key: Key,
        path: Vec<String>,
        responder: Responder<Result<QueryResult, engine_state::Error>>,
    ) -> Effects<Event> {
        let query = QueryRequest::new(global_state_hash.into(), base_key, path);
        effect_builder
            .query_global_state(query)
            .event(move |result| Event::QueryGlobalStateResult {
                result,
                main_responder: responder,
            })
    }

    fn handle_get_balance<REv: ReactorEventT>(
        &mut self,
        effect_builder: EffectBuilder<REv>,
        global_state_hash: Digest,
        purse_uref: URef,
        responder: Responder<Result<BalanceResult, engine_state::Error>>,
    ) -> Effects<Event> {
        let query = BalanceRequest::new(global_state_hash.into(), purse_uref);
        effect_builder
            .get_balance(query)
            .event(move |result| Event::GetBalanceResult {
                result,
                main_responder: responder,
            })
    }

    /// Broadcasts the SSE data to all clients connected to the event stream.
    fn broadcast(&mut self, sse_data: SseData) -> Effects<Event> {
        let _ = self.sse_data_sender.send(sse_data);
        Effects::new()
    }
}

impl<REv> Component<REv> for ApiServer
where
    REv: From<ApiServerAnnouncement>
        + From<NetworkInfoRequest<NodeId>>
        + From<LinearChainRequest<NodeId>>
        + From<ContractRuntimeRequest>
        + From<ChainspecLoaderRequest>
        + From<MetricsRequest>
        + From<StorageRequest<Storage>>
        + From<Event>
        + From<ApiRequest<NodeId>>
        + Send,
{
    type Event = Event;

    fn handle_event(
        &mut self,
        effect_builder: EffectBuilder<REv>,
        _rng: &mut dyn CryptoRngCore,
        event: Self::Event,
    ) -> Effects<Self::Event> {
        match event {
            Event::ApiRequest(ApiRequest::SubmitDeploy { deploy, responder }) => {
                let mut effects = effect_builder.announce_deploy_received(deploy).ignore();
                effects.extend(responder.respond(()).ignore());
                effects
            }
            Event::ApiRequest(ApiRequest::GetBlock {
                maybe_hash: Some(hash),
                responder,
            }) => effect_builder
                .get_block_from_storage(hash)
                .event(move |result| Event::GetBlockResult {
                    maybe_hash: Some(hash),
                    result: Box::new(result),
                    main_responder: responder,
                }),
            Event::ApiRequest(ApiRequest::GetBlock {
                maybe_hash: None,
                responder,
            }) => effect_builder
                .get_last_finalized_block()
                .event(move |result| Event::GetBlockResult {
                    maybe_hash: None,
                    result: Box::new(result),
                    main_responder: responder,
                }),
            Event::ApiRequest(ApiRequest::QueryGlobalState {
                global_state_hash,
                base_key,
                path,
                responder,
            }) => self.handle_query(effect_builder, global_state_hash, base_key, path, responder),
            Event::ApiRequest(ApiRequest::GetBalance {
                global_state_hash,
                purse_uref,
                responder,
            }) => self.handle_get_balance(effect_builder, global_state_hash, purse_uref, responder),
            Event::ApiRequest(ApiRequest::GetDeploy { hash, responder }) => effect_builder
                .get_deploy_and_metadata_from_storage(hash)
                .event(move |result| Event::GetDeployResult {
                    hash,
                    result: Box::new(result),
                    main_responder: responder,
                }),
            Event::ApiRequest(ApiRequest::GetPeers { responder }) => effect_builder
                .network_peers()
                .event(move |peers| Event::GetPeersResult {
                    peers,
                    main_responder: responder,
                }),
            Event::ApiRequest(ApiRequest::GetStatus { responder }) => async move {
                let (last_finalized_block, peers, chainspec_info) = join!(
                    effect_builder.get_last_finalized_block(),
                    effect_builder.network_peers(),
                    effect_builder.get_chainspec_info()
                );
                let status_feed = StatusFeed::new(last_finalized_block, peers, chainspec_info);
                info!("GetStatus --status_feed: {:?}", status_feed);
                responder.respond(status_feed).await;
            }
            .ignore(),
            Event::ApiRequest(ApiRequest::GetMetrics { responder }) => effect_builder
                .get_metrics()
                .event(move |text| Event::GetMetricsResult {
                    text,
                    main_responder: responder,
                }),
            Event::GetBlockResult {
                maybe_hash: _,
                result,
                main_responder,
            } => main_responder.respond(*result).ignore(),
            Event::QueryGlobalStateResult {
                result,
                main_responder,
            } => main_responder.respond(result).ignore(),
            Event::GetBalanceResult {
                result,
                main_responder,
            } => main_responder.respond(result).ignore(),
            Event::GetDeployResult {
                hash: _,
                result,
                main_responder,
            } => main_responder.respond(*result).ignore(),
            Event::GetPeersResult {
                peers,
                main_responder,
            } => main_responder.respond(peers).ignore(),
            Event::GetMetricsResult {
                text,
                main_responder,
            } => main_responder.respond(text).ignore(),
            Event::BlockFinalized(finalized_block) => {
                self.broadcast(SseData::BlockFinalized(*finalized_block))
            }
            Event::BlockAdded {
                block_hash,
                block_header,
            } => self.broadcast(SseData::BlockAdded {
                block_hash,
                block_header: *block_header,
            }),
            Event::DeployProcessed {
                deploy_hash,
                block_hash,
                execution_result,
            } => self.broadcast(SseData::DeployProcessed {
                deploy_hash,
                block_hash,
                execution_result,
            }),
        }
    }
}
