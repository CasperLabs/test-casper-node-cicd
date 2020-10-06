use std::{fmt, iter};

use datasize::DataSize;
use num_traits::Zero;
use rand::{
    distributions::{Distribution, Standard},
    Rng,
};
use serde::{Deserialize, Serialize};

use casper_types::{account::AccountHash, bytesrepr, Key, ProtocolVersion, PublicKey, U512};

use super::SYSTEM_ACCOUNT_ADDR;
use crate::{
    core::engine_state::execution_effect::ExecutionEffect,
    shared::{motes::Motes, newtypes::Blake2bHash, wasm_costs::WasmCosts, TypeMismatch},
    storage::global_state::CommitResult,
};

pub const PLACEHOLDER_KEY: Key = Key::Hash([0u8; 32]);
pub const POS_PAYMENT_PURSE: &str = "pos_payment_purse";
pub const POS_REWARDS_PURSE: &str = "pos_rewards_purse";

#[derive(Debug)]
pub enum GenesisResult {
    RootNotFound,
    KeyNotFound(Key),
    TypeMismatch(TypeMismatch),
    Serialization(bytesrepr::Error),
    Success {
        post_state_hash: Blake2bHash,
        effect: ExecutionEffect,
    },
}

impl fmt::Display for GenesisResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            GenesisResult::RootNotFound => write!(f, "Root not found"),
            GenesisResult::KeyNotFound(key) => write!(f, "Key not found: {}", key),
            GenesisResult::TypeMismatch(type_mismatch) => {
                write!(f, "Type mismatch: {:?}", type_mismatch)
            }
            GenesisResult::Serialization(error) => write!(f, "Serialization error: {:?}", error),
            GenesisResult::Success {
                post_state_hash,
                effect,
            } => write!(f, "Success: {} {:?}", post_state_hash, effect),
        }
    }
}

impl GenesisResult {
    pub fn from_commit_result(commit_result: CommitResult, effect: ExecutionEffect) -> Self {
        match commit_result {
            CommitResult::RootNotFound => GenesisResult::RootNotFound,
            CommitResult::KeyNotFound(key) => GenesisResult::KeyNotFound(key),
            CommitResult::TypeMismatch(type_mismatch) => GenesisResult::TypeMismatch(type_mismatch),
            CommitResult::Serialization(error) => GenesisResult::Serialization(error),
            CommitResult::Success { state_root, .. } => GenesisResult::Success {
                post_state_hash: state_root,
                effect,
            },
        }
    }
}

#[derive(DataSize, Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenesisAccount {
    /// Assumed to be a system account if `public_key` is not specified.
    public_key: Option<PublicKey>,
    account_hash: AccountHash,
    balance: Motes,
    bonded_amount: Motes,
}

impl GenesisAccount {
    pub fn system(balance: Motes, bonded_amount: Motes) -> Self {
        Self {
            public_key: None,
            account_hash: SYSTEM_ACCOUNT_ADDR,
            balance,
            bonded_amount,
        }
    }

    pub fn new(
        public_key: PublicKey,
        account_hash: AccountHash,
        balance: Motes,
        bonded_amount: Motes,
    ) -> Self {
        GenesisAccount {
            public_key: Some(public_key),
            account_hash,
            balance,
            bonded_amount,
        }
    }

    pub fn public_key(&self) -> Option<PublicKey> {
        self.public_key
    }

    pub fn account_hash(&self) -> AccountHash {
        self.account_hash
    }

    pub fn balance(&self) -> Motes {
        self.balance
    }

    pub fn bonded_amount(&self) -> Motes {
        self.bonded_amount
    }

    /// Checks if a given genesis account belongs to a virtual system account,
    pub fn is_system_account(&self) -> bool {
        self.public_key.is_none()
    }

    /// Checks if a given genesis account is a valid genesis validator.
    ///
    /// Genesis validators are the ones with a stake, and are not owned by a virtual system account.
    pub fn is_genesis_validator(&self) -> bool {
        !self.is_system_account() && !self.bonded_amount.is_zero()
    }
}

impl Distribution<GenesisAccount> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> GenesisAccount {
        let account_hash = AccountHash::new(rng.gen());

        let public_key = PublicKey::Ed25519(rng.gen());

        let mut u512_array = [0u8; 64];
        rng.fill_bytes(u512_array.as_mut());
        let balance = Motes::new(U512::from(u512_array));

        rng.fill_bytes(u512_array.as_mut());
        let bonded_amount = Motes::new(U512::from(u512_array));

        GenesisAccount::new(public_key, account_hash, balance, bonded_amount)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenesisConfig {
    name: String,
    timestamp: u64,
    protocol_version: ProtocolVersion,
    ee_config: ExecConfig,
}

impl GenesisConfig {
    pub fn new(
        name: String,
        timestamp: u64,
        protocol_version: ProtocolVersion,
        ee_config: ExecConfig,
    ) -> Self {
        GenesisConfig {
            name,
            timestamp,
            protocol_version,
            ee_config,
        }
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    pub fn protocol_version(&self) -> ProtocolVersion {
        self.protocol_version
    }

    pub fn ee_config(&self) -> &ExecConfig {
        &self.ee_config
    }

    pub fn ee_config_mut(&mut self) -> &mut ExecConfig {
        &mut self.ee_config
    }

    pub fn take_ee_config(self) -> ExecConfig {
        self.ee_config
    }
}

impl Distribution<GenesisConfig> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> GenesisConfig {
        let count = rng.gen_range(1, 1000);
        let name = iter::repeat(())
            .map(|_| rng.gen::<char>())
            .take(count)
            .collect();

        let timestamp = rng.gen();

        let protocol_version = ProtocolVersion::from_parts(rng.gen(), rng.gen(), rng.gen());

        let ee_config = rng.gen();

        GenesisConfig {
            name,
            timestamp,
            protocol_version,
            ee_config,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecConfig {
    mint_installer_bytes: Vec<u8>,
    proof_of_stake_installer_bytes: Vec<u8>,
    standard_payment_installer_bytes: Vec<u8>,
    auction_installer_bytes: Vec<u8>,
    accounts: Vec<GenesisAccount>,
    wasm_costs: WasmCosts,
}

impl ExecConfig {
    pub fn new(
        mint_installer_bytes: Vec<u8>,
        proof_of_stake_installer_bytes: Vec<u8>,
        standard_payment_installer_bytes: Vec<u8>,
        auction_installer_bytes: Vec<u8>,
        accounts: Vec<GenesisAccount>,
        wasm_costs: WasmCosts,
    ) -> ExecConfig {
        ExecConfig {
            mint_installer_bytes,
            proof_of_stake_installer_bytes,
            standard_payment_installer_bytes,
            auction_installer_bytes,
            accounts,
            wasm_costs,
        }
    }

    pub fn mint_installer_bytes(&self) -> &[u8] {
        self.mint_installer_bytes.as_slice()
    }

    pub fn proof_of_stake_installer_bytes(&self) -> &[u8] {
        self.proof_of_stake_installer_bytes.as_slice()
    }

    pub fn standard_payment_installer_bytes(&self) -> &[u8] {
        self.standard_payment_installer_bytes.as_slice()
    }

    pub fn auction_installer_bytes(&self) -> &[u8] {
        self.auction_installer_bytes.as_slice()
    }

    pub fn wasm_costs(&self) -> WasmCosts {
        self.wasm_costs
    }

    pub fn get_bonded_validators(&self) -> impl Iterator<Item = &GenesisAccount> {
        self.accounts
            .iter()
            .filter(|&genesis_account| !genesis_account.bonded_amount().is_zero())
    }

    pub fn accounts(&self) -> &[GenesisAccount] {
        self.accounts.as_slice()
    }

    pub fn push_account(&mut self, account: GenesisAccount) {
        self.accounts.push(account)
    }
}

impl Distribution<ExecConfig> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> ExecConfig {
        let mut count = rng.gen_range(1000, 10_000);
        let mint_installer_bytes = iter::repeat(()).map(|_| rng.gen()).take(count).collect();

        count = rng.gen_range(1000, 10_000);
        let proof_of_stake_installer_bytes =
            iter::repeat(()).map(|_| rng.gen()).take(count).collect();

        count = rng.gen_range(1000, 10_000);
        let standard_payment_installer_bytes =
            iter::repeat(()).map(|_| rng.gen()).take(count).collect();
        count = rng.gen_range(1000, 10_000);
        let auction_installer_bytes = iter::repeat(()).map(|_| rng.gen()).take(count).collect();

        count = rng.gen_range(1, 10);
        let accounts = iter::repeat(()).map(|_| rng.gen()).take(count).collect();

        let wasm_costs = rng.gen();

        ExecConfig {
            mint_installer_bytes,
            proof_of_stake_installer_bytes,
            standard_payment_installer_bytes,
            auction_installer_bytes,
            accounts,
            wasm_costs,
        }
    }
}
