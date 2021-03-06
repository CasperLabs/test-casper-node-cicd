use datasize::DataSize;
use serde::{Deserialize, Serialize};

use crate::{
    logging::LoggingConfig, types::NodeConfig, ApiServerConfig, ConsensusConfig,
    ContractRuntimeConfig, GossipConfig, SmallNetworkConfig, StorageConfig,
};

/// Root configuration.
#[derive(DataSize, Debug, Default, Deserialize, Serialize)]
// Disallow unknown fields to ensure config files and command-line overrides contain valid keys.
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Node configuration.
    pub node: NodeConfig,
    /// Logging configuration.
    pub logging: LoggingConfig,
    /// Consensus configuration.
    pub consensus: ConsensusConfig,
    /// Network configuration.
    pub network: SmallNetworkConfig,
    /// HTTP API server configuration.
    pub http_server: ApiServerConfig,
    /// On-disk storage configuration.
    pub storage: StorageConfig,
    /// Gossip protocol configuration.
    pub gossip: GossipConfig,
    /// Contract runtime configuration.
    pub contract_runtime: ContractRuntimeConfig,
}
