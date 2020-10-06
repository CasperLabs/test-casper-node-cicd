use crate::{
    account::AccountHash,
    bytesrepr::{FromBytes, ToBytes},
    system_contract_errors::auction::Error,
    CLTyped, Key, TransferResult, URef, BLAKE2B_DIGEST_LENGTH, U512,
};

/// Provider of runtime host functionality.
pub trait RuntimeProvider {
    /// This method should return the caller of the current context.
    fn get_caller(&self) -> AccountHash;

    /// Gets named key under a `name`.
    fn get_key(&self, name: &str) -> Option<Key>;

    /// Puts key under a `name`.
    fn put_key(&mut self, name: &str, key: Key);

    /// Returns a 32-byte BLAKE2b digest
    fn blake2b<T: AsRef<[u8]>>(&self, data: T) -> [u8; BLAKE2B_DIGEST_LENGTH];
}

/// Provides functionality of a contract storage.
pub trait StorageProvider {
    /// Reads data from [`URef`].
    fn read<T: FromBytes + CLTyped>(&mut self, uref: URef) -> Result<Option<T>, Error>;

    /// Writes data to [`URef].
    fn write<T: ToBytes + CLTyped>(&mut self, uref: URef, value: T) -> Result<(), Error>;
}

/// Provides functionality of a system module.
pub trait SystemProvider {
    /// Creates new purse.
    fn create_purse(&mut self) -> URef;

    /// Gets purse balance.
    fn get_balance(&mut self, purse: URef) -> Result<Option<U512>, Error>;

    /// Transfers specified `amount` of tokens from `source` purse into a `target` purse.
    fn transfer_from_purse_to_purse(
        &mut self,
        source: URef,
        target: URef,
        amount: U512,
    ) -> Result<(), Error>;
}

/// Provides an access to mint.
pub trait MintProvider {
    /// Transfers `amount` from `source` purse to a `target` account.
    fn transfer_purse_to_account(
        &mut self,
        source: URef,
        target: AccountHash,
        amount: U512,
    ) -> TransferResult;

    /// Transfers `amount` from `source` purse to a `target` purse.
    fn transfer_purse_to_purse(
        &mut self,
        source: URef,
        target: URef,
        amount: U512,
    ) -> Result<(), ()>;

    /// Checks balance of a `purse`. Returns `None` if given purse does not exist.
    fn balance(&mut self, purse: URef) -> Option<U512>;

    /// Reads the base round reward.
    fn read_base_round_reward(&mut self) -> Result<U512, Error>;

    /// Mints new token with given `initial_balance` balance. Returns new purse on success,
    /// otherwise an error.
    fn mint(&mut self, amount: U512) -> Result<URef, Error>;
}
