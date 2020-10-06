//! Configuration options for the execution engine.

use serde::{Deserialize, Serialize};

use crate::shared::utils;

const DEFAULT_MAX_GLOBAL_STATE_SIZE: usize = 805_306_368_000; // 750 GiB
const DEFAULT_USE_SYSTEM_CONTRACTS: bool = false;

/// Contract runtime configuration.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
// Disallow unknown fields to ensure config files and command-line overrides contain valid keys.
#[serde(deny_unknown_fields)]
pub struct Config {
    use_system_contracts: Option<bool>,
    max_global_state_size: Option<usize>,
}

impl Config {
    /// Whether to use system contracts or not.  Defaults to false.
    pub fn use_system_contracts(&self) -> bool {
        self.use_system_contracts
            .unwrap_or(DEFAULT_USE_SYSTEM_CONTRACTS)
    }

    /// The maximum size of the database to use for the global state store.
    ///
    /// Defaults to 805,306,368,000 == 750 GiB.
    ///
    /// The size should be a multiple of the OS page size.
    pub fn max_global_state_size(&self) -> usize {
        let value = self
            .max_global_state_size
            .unwrap_or(DEFAULT_MAX_GLOBAL_STATE_SIZE);
        utils::check_multiple_of_page_size(value);
        value
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            use_system_contracts: Some(DEFAULT_USE_SYSTEM_CONTRACTS),
            max_global_state_size: Some(DEFAULT_MAX_GLOBAL_STATE_SIZE),
        }
    }
}
