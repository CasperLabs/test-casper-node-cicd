#[cfg(test)]
use std::iter;
use std::{
    array::TryFromSliceError,
    error::Error as StdError,
    fmt::{self, Debug, Display, Formatter},
    hash::Hash,
};

use datasize::DataSize;
use hex::FromHexError;
use hex_fmt::{HexFmt, HexList};
#[cfg(test)]
use rand::Rng;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(test)]
use casper_types::auction::BLOCK_REWARD;

use super::{Item, Tag, Timestamp};
use crate::{
    components::{
        consensus::{self, EraId},
        storage::Value,
    },
    crypto::{
        asymmetric_key::{PublicKey, Signature},
        hash::{self, Digest},
    },
    types::DeployHash,
    utils::DisplayIter,
};
#[cfg(test)]
use crate::{
    crypto::asymmetric_key::{self, SecretKey},
    testing::TestRng,
};

/// Error returned from constructing or validating a `Block`.
#[derive(Debug, Error)]
pub enum Error {
    /// Error while encoding to JSON.
    #[error("encoding to JSON: {0}")]
    EncodeToJson(#[from] serde_json::Error),

    /// Error while decoding from JSON.
    #[error("decoding from JSON: {0}")]
    DecodeFromJson(Box<dyn StdError>),
}

impl From<FromHexError> for Error {
    fn from(error: FromHexError) -> Self {
        Error::DecodeFromJson(Box::new(error))
    }
}

impl From<TryFromSliceError> for Error {
    fn from(error: TryFromSliceError) -> Self {
        Error::DecodeFromJson(Box::new(error))
    }
}

pub trait BlockLike: Eq + Hash {
    fn deploys(&self) -> &Vec<DeployHash>;
}

/// A cryptographic hash identifying a `ProtoBlock`.
#[derive(
    Copy,
    Clone,
    DataSize,
    Ord,
    PartialOrd,
    Eq,
    PartialEq,
    Hash,
    Serialize,
    Deserialize,
    Debug,
    Default,
)]
pub struct ProtoBlockHash(Digest);

impl ProtoBlockHash {
    /// Constructs a new `ProtoBlockHash`.
    pub fn new(hash: Digest) -> Self {
        ProtoBlockHash(hash)
    }

    /// Returns the wrapped inner hash.
    pub fn inner(&self) -> &Digest {
        &self.0
    }

    /// Returns `true` is `self` is a hash of empty `ProtoBlock`.
    pub(crate) fn is_empty(self) -> bool {
        self == ProtoBlock::empty_random_bit_false() || self == ProtoBlock::empty_random_bit_true()
    }
}

impl Display for ProtoBlockHash {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "proto-block-hash({})", self.0)
    }
}

/// The piece of information that will become the content of a future block (isn't finalized or
/// executed yet)
///
/// From the view of the consensus protocol this is the "consensus value": The protocol deals with
/// finalizing an order of `ProtoBlock`s. Only after consensus has been reached, the block's
/// deploys actually get executed, and the executed block gets signed.
///
/// The word "proto" does _not_ refer to "protocol" or "protobuf"! It is just a prefix to highlight
/// that this comes before a block in the linear, executed, finalized blockchain is produced.
#[derive(Clone, DataSize, Debug, PartialOrd, Ord, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProtoBlock {
    hash: ProtoBlockHash,
    deploys: Vec<DeployHash>,
    random_bit: bool,
}

impl ProtoBlock {
    pub(crate) fn new(deploys: Vec<DeployHash>, random_bit: bool) -> Self {
        let hash = ProtoBlockHash::new(hash::hash(
            &rmp_serde::to_vec(&(&deploys, random_bit)).expect("serialize ProtoBlock"),
        ));

        ProtoBlock {
            hash,
            deploys,
            random_bit,
        }
    }

    pub(crate) fn hash(&self) -> &ProtoBlockHash {
        &self.hash
    }

    /// The list of deploy hashes included in the block.
    pub(crate) fn deploys(&self) -> &Vec<DeployHash> {
        &self.deploys
    }

    /// A random bit needed for initializing a future era.
    pub(crate) fn random_bit(&self) -> bool {
        self.random_bit
    }

    pub(crate) fn destructure(self) -> (ProtoBlockHash, Vec<DeployHash>, bool) {
        (self.hash, self.deploys, self.random_bit)
    }

    /// Returns hash of empty ProtoBlock (no deploys) with a random bit set to false.
    /// Added here so that it's always aligned with how hash is calculated.
    pub(crate) fn empty_random_bit_false() -> ProtoBlockHash {
        *ProtoBlock::new(vec![], false).hash()
    }

    /// Returns hash of empty ProtoBlock (no deploys) with a random bit set to true.
    /// Added here so that it's always aligned with how hash is calculated.
    pub(crate) fn empty_random_bit_true() -> ProtoBlockHash {
        *ProtoBlock::new(vec![], true).hash()
    }
}

impl Display for ProtoBlock {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "proto block {}, deploys [{}], random bit {}",
            self.hash.inner(),
            DisplayIter::new(self.deploys.iter()),
            self.random_bit(),
        )
    }
}

impl BlockLike for ProtoBlock {
    fn deploys(&self) -> &Vec<DeployHash> {
        self.deploys()
    }
}

/// Equivocation and reward information to be included in the terminal finalized block.
pub type EraEnd = consensus::EraEnd<PublicKey>;

impl Display for EraEnd {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let slashings = DisplayIter::new(&self.equivocators);
        let rewards = DisplayIter::new(
            self.rewards
                .iter()
                .map(|(public_key, amount)| format!("{}: {}", public_key, amount)),
        );
        write!(f, "era end: slash {}, reward {}", slashings, rewards)
    }
}

/// The piece of information that will become the content of a future block after it was finalized
/// and before execution happened yet.
#[derive(Clone, DataSize, Debug, PartialOrd, Ord, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FinalizedBlock {
    proto_block: ProtoBlock,
    timestamp: Timestamp,
    era_end: Option<EraEnd>,
    era_id: EraId,
    height: u64,
    proposer: PublicKey,
}

impl FinalizedBlock {
    pub(crate) fn new(
        proto_block: ProtoBlock,
        timestamp: Timestamp,
        era_end: Option<EraEnd>,
        era_id: EraId,
        height: u64,
        proposer: PublicKey,
    ) -> Self {
        FinalizedBlock {
            proto_block,
            timestamp,
            era_end,
            era_id,
            height,
            proposer,
        }
    }

    /// The finalized proto block.
    pub(crate) fn proto_block(&self) -> &ProtoBlock {
        &self.proto_block
    }

    /// The timestamp from when the proto block was proposed.
    pub(crate) fn timestamp(&self) -> Timestamp {
        self.timestamp
    }

    /// Returns slashing and reward information if this is a switch block, i.e. the last block of
    /// its era.
    pub(crate) fn era_end(&self) -> &Option<EraEnd> {
        &self.era_end
    }

    /// Returns the ID of the era this block belongs to.
    pub(crate) fn era_id(&self) -> EraId {
        self.era_id
    }

    /// Returns the height of this block.
    pub(crate) fn height(&self) -> u64 {
        self.height
    }

    /// Returns true if block is Genesis' child.
    /// Genesis child block is from era 0 and height 0.
    pub(crate) fn is_genesis_child(&self) -> bool {
        self.era_id() == EraId(0) && self.height() == 0
    }

    /// Generates a random instance using a `TestRng`.
    #[cfg(test)]
    pub fn random(rng: &mut TestRng) -> Self {
        let deploy_count = rng.gen_range(0, 11);
        let deploy_hashes = iter::repeat_with(|| DeployHash::new(Digest::random(rng)))
            .take(deploy_count)
            .collect();
        let random_bit = rng.gen();
        let proto_block = ProtoBlock::new(deploy_hashes, random_bit);

        // TODO - make Timestamp deterministic.
        let timestamp = Timestamp::now();
        let era_end = if rng.gen_bool(0.1) {
            let equivocators_count = rng.gen_range(0, 5);
            let rewards_count = rng.gen_range(0, 5);
            Some(EraEnd {
                equivocators: iter::repeat_with(|| {
                    PublicKey::from(&SecretKey::new_ed25519(rng.gen()))
                })
                .take(equivocators_count)
                .collect(),
                rewards: iter::repeat_with(|| {
                    let pub_key = PublicKey::from(&SecretKey::new_ed25519(rng.gen()));
                    let reward = rng.gen_range(1, BLOCK_REWARD + 1);
                    (pub_key, reward)
                })
                .take(rewards_count)
                .collect(),
            })
        } else {
            None
        };
        let era = rng.gen_range(0, 5);
        let secret_key: SecretKey = SecretKey::new_ed25519(rng.gen());
        let public_key = PublicKey::from(&secret_key);

        FinalizedBlock::new(
            proto_block,
            timestamp,
            era_end,
            EraId(era),
            era * 10 + rng.gen_range(0, 10),
            public_key,
        )
    }
}

impl From<BlockHeader> for FinalizedBlock {
    fn from(header: BlockHeader) -> Self {
        let proto_block = ProtoBlock::new(header.deploy_hashes().clone(), header.random_bit);

        FinalizedBlock {
            proto_block,
            timestamp: header.timestamp,
            era_end: header.era_end,
            era_id: header.era_id,
            height: header.height,
            proposer: header.proposer,
        }
    }
}

impl Display for FinalizedBlock {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "finalized block {:10} in era {:?}, height {}, deploys {:10}, random bit {}, \
            timestamp {}",
            HexFmt(self.proto_block.hash().inner()),
            self.era_id,
            self.height,
            HexList(&self.proto_block.deploys),
            self.proto_block.random_bit,
            self.timestamp,
        )?;
        if let Some(ee) = &self.era_end {
            write!(formatter, ", era_end: {}", ee)?;
        }
        Ok(())
    }
}

/// A cryptographic hash identifying a [`Block`](struct.Block.html).
#[derive(
    Copy, Clone, DataSize, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize, Debug,
)]
pub struct BlockHash(Digest);

impl BlockHash {
    /// Constructs a new `BlockHash`.
    pub fn new(hash: Digest) -> Self {
        BlockHash(hash)
    }

    /// Returns the wrapped inner hash.
    pub fn inner(&self) -> &Digest {
        &self.0
    }
}

impl Display for BlockHash {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "block-hash({})", self.0,)
    }
}

impl From<Digest> for BlockHash {
    fn from(digest: Digest) -> Self {
        Self(digest)
    }
}

impl AsRef<[u8]> for BlockHash {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

/// The header portion of a [`Block`](struct.Block.html).
#[derive(Clone, DataSize, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize, Debug)]
pub struct BlockHeader {
    parent_hash: BlockHash,
    global_state_hash: Digest,
    body_hash: Digest,
    deploy_hashes: Vec<DeployHash>,
    random_bit: bool,
    era_end: Option<EraEnd>,
    timestamp: Timestamp,
    era_id: EraId,
    height: u64,
    proposer: PublicKey,
}

impl BlockHeader {
    /// The parent block's hash.
    pub fn parent_hash(&self) -> &BlockHash {
        &self.parent_hash
    }

    /// The root hash of the resulting global state.
    pub fn global_state_hash(&self) -> &Digest {
        &self.global_state_hash
    }

    /// The hash of the block's body.
    pub fn body_hash(&self) -> &Digest {
        &self.body_hash
    }

    /// The list of deploy hashes included in the block.
    pub fn deploy_hashes(&self) -> &Vec<DeployHash> {
        &self.deploy_hashes
    }

    /// A random bit needed for initializing a future era.
    pub fn random_bit(&self) -> bool {
        self.random_bit
    }

    /// The timestamp from when the proto block was proposed.
    pub fn timestamp(&self) -> Timestamp {
        self.timestamp
    }

    /// Returns reward and slashing information if this is the era's last block.
    pub fn era_end(&self) -> Option<&EraEnd> {
        self.era_end.as_ref()
    }

    /// Returns `true` if this block is the last one in the current era.
    pub fn switch_block(&self) -> bool {
        self.era_end.is_some()
    }

    /// Era ID in which this block was created.
    pub fn era_id(&self) -> EraId {
        self.era_id
    }

    /// Returns the height of this block, i.e. the number of ancestors.
    pub fn height(&self) -> u64 {
        self.height
    }

    /// Block proposer.
    pub fn proposer(&self) -> &PublicKey {
        &self.proposer
    }

    /// Returns true if block is Genesis' child.
    /// Genesis child block is from era 0 and height 0.
    pub(crate) fn is_genesis_child(&self) -> bool {
        self.era_id() == EraId(0) && self.height() == 0
    }

    // Serialize the block header.
    fn serialize(&self) -> Result<Vec<u8>, rmp_serde::encode::Error> {
        rmp_serde::to_vec(self)
    }

    /// Hash of the block header.
    pub fn hash(&self) -> BlockHash {
        let serialized_header = Self::serialize(&self)
            .unwrap_or_else(|error| panic!("should serialize block header: {}", error));
        BlockHash::new(hash::hash(&serialized_header))
    }
}

impl Display for BlockHeader {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(
            formatter,
            "block header parent hash {}, post-state hash {}, body hash {}, deploys [{}], \
            random bit {}, timestamp {}",
            self.parent_hash.inner(),
            self.global_state_hash,
            self.body_hash,
            DisplayIter::new(self.deploy_hashes.iter()),
            self.random_bit,
            self.timestamp,
        )?;
        if let Some(ee) = &self.era_end {
            write!(formatter, ", era_end: {}", ee)?;
        }
        Ok(())
    }
}

/// A proto-block after execution, with the resulting post-state-hash.  This is the core component
/// of the Casper linear blockchain.
#[derive(DataSize, Clone, Debug, PartialOrd, Ord, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Block {
    hash: BlockHash,
    header: BlockHeader,
    body: (), // TODO: implement body of block
    proofs: Vec<Signature>,
}

impl Block {
    pub(crate) fn new(
        parent_hash: BlockHash,
        global_state_hash: Digest,
        finalized_block: FinalizedBlock,
    ) -> Self {
        let body = ();
        let serialized_body = Self::serialize_body(&body)
            .unwrap_or_else(|error| panic!("should serialize block body: {}", error));
        let body_hash = hash::hash(&serialized_body);

        let era_id = finalized_block.era_id();
        let height = finalized_block.height();

        let header = BlockHeader {
            parent_hash,
            global_state_hash,
            body_hash,
            deploy_hashes: finalized_block.proto_block.deploys,
            random_bit: finalized_block.proto_block.random_bit,
            era_end: finalized_block.era_end,
            timestamp: finalized_block.timestamp,
            era_id,
            height,
            proposer: finalized_block.proposer,
        };

        let hash = header.hash();

        Block {
            hash,
            header,
            body,
            proofs: vec![],
        }
    }

    pub(crate) fn header(&self) -> &BlockHeader {
        &self.header
    }

    pub(crate) fn take_header(self) -> BlockHeader {
        self.header
    }

    pub(crate) fn hash(&self) -> &BlockHash {
        &self.hash
    }

    pub(crate) fn global_state_hash(&self) -> &Digest {
        self.header.global_state_hash()
    }

    /// The deploy hashes included in this block.
    pub fn deploy_hashes(&self) -> &Vec<DeployHash> {
        self.header.deploy_hashes()
    }

    pub(crate) fn height(&self) -> u64 {
        self.header.height()
    }

    /// Appends the given signature to this block's proofs.  It should have been validated prior to
    /// this via `BlockHash::verify()`.
    pub(crate) fn append_proof(&mut self, proof: Signature) {
        self.proofs.push(proof)
    }

    fn serialize_body(body: &()) -> Result<Vec<u8>, rmp_serde::encode::Error> {
        rmp_serde::to_vec(body)
    }

    /// Generates a random instance using a `TestRng`.
    #[cfg(test)]
    pub fn random(rng: &mut TestRng) -> Self {
        let parent_hash = BlockHash::new(Digest::random(rng));
        let global_state_hash = Digest::random(rng);
        let finalized_block = FinalizedBlock::random(rng);

        let mut block = Block::new(parent_hash, global_state_hash, finalized_block);

        let signatures_count = rng.gen_range(0, 11);
        for _ in 0..signatures_count {
            let secret_key = SecretKey::random(rng);
            let public_key = PublicKey::from(&secret_key);
            let signature = asymmetric_key::sign(block.hash.inner(), &secret_key, &public_key, rng);
            block.append_proof(signature);
        }

        block
    }
}

impl Display for Block {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "executed block {}, parent hash {}, post-state hash {}, body hash {}, deploys [{}], \
            random bit {}, timestamp {}, era_id {}, height {}, proofs count {}",
            self.hash.inner(),
            self.header.parent_hash.inner(),
            self.header.global_state_hash,
            self.header.body_hash,
            DisplayIter::new(self.header.deploy_hashes.iter()),
            self.header.random_bit,
            self.header.timestamp,
            self.header.era_id.0,
            self.header.height,
            self.proofs.len()
        )?;
        if let Some(ee) = &self.header.era_end {
            write!(formatter, ", era_end: {}", ee)?;
        }
        Ok(())
    }
}

impl BlockLike for Block {
    fn deploys(&self) -> &Vec<DeployHash> {
        self.deploy_hashes()
    }
}

impl BlockLike for BlockHeader {
    fn deploys(&self) -> &Vec<DeployHash> {
        self.deploy_hashes()
    }
}

impl Value for Block {
    type Id = BlockHash;
    type Header = BlockHeader;

    fn id(&self) -> &Self::Id {
        &self.hash
    }

    fn header(&self) -> &Self::Header {
        &self.header
    }

    fn take_header(self) -> Self::Header {
        self.header
    }
}

impl Item for Block {
    type Id = BlockHash;

    const TAG: Tag = Tag::Block;
    const ID_IS_COMPLETE_ITEM: bool = false;

    fn id(&self) -> Self::Id {
        *self.hash()
    }
}

/// A wrapper around `Block` for the purposes of fetching blocks by height in linear chain.
#[derive(Clone, Debug, PartialOrd, Ord, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BlockByHeight {
    Absent(u64),
    Block(Box<Block>),
}

impl From<Block> for BlockByHeight {
    fn from(block: Block) -> Self {
        BlockByHeight::new(block)
    }
}

impl BlockByHeight {
    /// Creates a new `BlockByHeight`
    pub fn new(block: Block) -> Self {
        BlockByHeight::Block(Box::new(block))
    }

    pub fn height(&self) -> u64 {
        match self {
            BlockByHeight::Absent(height) => *height,
            BlockByHeight::Block(block) => block.height(),
        }
    }
}

impl Display for BlockByHeight {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            BlockByHeight::Absent(height) => write!(f, "Block at height {} was absent.", height),
            BlockByHeight::Block(block) => {
                let hash: BlockHash = block.header().hash();
                write!(f, "Block at {} with hash {} found.", block.height(), hash)
            }
        }
    }
}

impl Item for BlockByHeight {
    type Id = u64;

    const TAG: Tag = Tag::BlockByHeight;
    const ID_IS_COMPLETE_ITEM: bool = false;

    fn id(&self) -> Self::Id {
        self.height()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TestRng;

    #[test]
    fn json_block_roundtrip() {
        let mut rng = TestRng::new();
        let block = Block::random(&mut rng);
        let json_string = serde_json::to_string_pretty(&block).unwrap();
        let decoded = serde_json::from_str(&json_string).unwrap();
        assert_eq!(block, decoded);
    }

    #[test]
    fn json_finalized_block_roundtrip() {
        let mut rng = TestRng::new();
        let finalized_block = FinalizedBlock::random(&mut rng);
        let json_string = serde_json::to_string_pretty(&finalized_block).unwrap();
        let decoded = serde_json::from_str(&json_string).unwrap();
        assert_eq!(finalized_block, decoded);
    }
}
