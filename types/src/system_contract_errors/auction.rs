//! Home of the Auction contract's [`Error`] type.
use alloc::vec::Vec;
use core::{
    convert::{TryFrom, TryInto},
    result,
};

use failure::Fail;

use crate::{
    bytesrepr::{self, FromBytes, ToBytes, U8_SERIALIZED_LENGTH},
    CLType, CLTyped,
};

/// Errors which can occur while executing the Auction contract.
#[derive(Fail, Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum Error {
    /// Unable to find named key in the contract's named keys.
    #[fail(display = "Missing key")]
    MissingKey = 0,
    /// Given named key contains invalid variant.
    #[fail(display = "Invalid key variant")]
    InvalidKeyVariant = 1,
    /// Value under an uref does not exist. This means the installer contract didn't work properly.
    #[fail(display = "Missing value")]
    MissingValue = 2,
    /// ABI serialization issue while reading or writing.
    #[fail(display = "Serialization error")]
    Serialization = 3,
    /// Triggered when contract was unable to transfer desired amount of tokens.
    #[fail(display = "Transfer error")]
    Transfer = 4,
    /// User passed invalid amount of tokens which might result in wrong values after calculation.
    #[fail(display = "Invalid amount")]
    InvalidAmount = 5,
    /// Unable to find a bid by account hash in `active_bids` map.
    #[fail(display = "Bid not found")]
    BidNotFound = 6,
    /// Validator's account hash was not found in the map.
    #[fail(display = "Validator not found")]
    ValidatorNotFound = 7,
    /// Delegator's account hash was not found in the map.
    #[fail(display = "Delegator not found")]
    DelegatorNotFound = 8,
    /// Storage problem.
    #[fail(display = "Storage error")]
    Storage = 9,
    /// Raised when system is unable to bond.
    #[fail(display = "Bonding error")]
    Bonding = 10,
    /// Raised when system is unable to unbond.
    #[fail(display = "Unbonding error")]
    Unbonding = 11,
    /// Raised when Mint contract is unable to release founder stake.
    #[fail(display = "Unable to release founder stake")]
    ReleaseFounderStake = 12,
    /// Raised when the system is unable to determine purse balance.
    #[fail(display = "Unable to get purse balance")]
    GetBalance = 13,
    /// Raised when an entry point is called from invalid account context.
    #[fail(display = "Invalid context")]
    InvalidContext = 14,
    /// Raised whenever a validator's funds are still locked in but an attempt to withdraw was
    /// made.
    #[fail(display = "Validator's funds are locked")]
    ValidatorFundsLocked = 15,
    /// Called when caller is not a system account.
    #[fail(display = "Not a system account")]
    InvalidCaller = 16,
    /// Validator is not not bonded.
    #[fail(display = "Validator's bond not found")]
    BondNotFound = 17,
    /// Unable to create purse.
    #[fail(display = "Unable to create purse")]
    CreatePurseFailed = 18,
    /// Attempted to unbond an amount which was too large.
    #[fail(display = "Unbond is too large")]
    UnbondTooLarge = 19,
    /// Attempted to bond with a stake which was too small.
    #[fail(display = "Bond is too small")]
    BondTooSmall = 20,
    /// Raised when rewards are to be distributed to delegators, but the validator has no
    /// delegations.
    #[fail(display = "Validators has not received any delegations")]
    MissingDelegations = 21,
    /// The validators returned by the consensus component should match
    /// current era validators when distributing rewards.
    #[fail(display = "Mismatched era validator sets to distribute rewards")]
    MismatchedEraValidators = 22,
    /// Failed to mint reward tokens
    #[fail(display = "Failed to mint rewards")]
    MintReward,
}

impl CLTyped for Error {
    fn cl_type() -> CLType {
        CLType::U8
    }
}

// This error type is not intended to be used by third party crates.
#[doc(hidden)]
pub struct TryFromU8ForError(());

// This conversion is not intended to be used by third party crates.
#[doc(hidden)]
impl TryFrom<u8> for Error {
    type Error = TryFromU8ForError;

    fn try_from(value: u8) -> result::Result<Self, Self::Error> {
        match value {
            d if d == Error::MissingKey as u8 => Ok(Error::MissingKey),
            d if d == Error::InvalidKeyVariant as u8 => Ok(Error::InvalidKeyVariant),
            d if d == Error::MissingValue as u8 => Ok(Error::MissingValue),
            d if d == Error::Serialization as u8 => Ok(Error::Serialization),
            d if d == Error::Transfer as u8 => Ok(Error::Transfer),
            d if d == Error::InvalidAmount as u8 => Ok(Error::InvalidAmount),
            d if d == Error::BidNotFound as u8 => Ok(Error::BidNotFound),
            d if d == Error::ValidatorNotFound as u8 => Ok(Error::ValidatorNotFound),
            d if d == Error::DelegatorNotFound as u8 => Ok(Error::DelegatorNotFound),
            d if d == Error::Storage as u8 => Ok(Error::Storage),
            d if d == Error::Bonding as u8 => Ok(Error::Bonding),
            d if d == Error::Unbonding as u8 => Ok(Error::Unbonding),
            d if d == Error::ReleaseFounderStake as u8 => Ok(Error::ReleaseFounderStake),
            d if d == Error::GetBalance as u8 => Ok(Error::GetBalance),
            d if d == Error::InvalidContext as u8 => Ok(Error::InvalidContext),
            d if d == Error::ValidatorFundsLocked as u8 => Ok(Error::ValidatorFundsLocked),
            d if d == Error::InvalidCaller as u8 => Ok(Error::InvalidCaller),
            d if d == Error::BondNotFound as u8 => Ok(Error::BondNotFound),
            d if d == Error::CreatePurseFailed as u8 => Ok(Error::CreatePurseFailed),
            d if d == Error::UnbondTooLarge as u8 => Ok(Error::UnbondTooLarge),
            d if d == Error::BondTooSmall as u8 => Ok(Error::BondTooSmall),
            d if d == Error::MissingDelegations as u8 => Ok(Error::MissingDelegations),
            d if d == Error::MismatchedEraValidators as u8 => Ok(Error::MismatchedEraValidators),
            d if d == Error::MintReward as u8 => Ok(Error::MintReward),
            _ => Err(TryFromU8ForError(())),
        }
    }
}

impl ToBytes for Error {
    fn to_bytes(&self) -> result::Result<Vec<u8>, bytesrepr::Error> {
        let value = *self as u8;
        value.to_bytes()
    }

    fn serialized_length(&self) -> usize {
        U8_SERIALIZED_LENGTH
    }
}

impl FromBytes for Error {
    fn from_bytes(bytes: &[u8]) -> result::Result<(Self, &[u8]), bytesrepr::Error> {
        let (value, rem): (u8, _) = FromBytes::from_bytes(bytes)?;
        let error: Error = value
            .try_into()
            // In case an Error variant is unable to be determined it would return an
            // Error::Formatting as if its unable to be correctly deserialized.
            .map_err(|_| bytesrepr::Error::Formatting)?;
        Ok((error, rem))
    }
}

impl From<bytesrepr::Error> for Error {
    fn from(_: bytesrepr::Error) -> Self {
        Error::Serialization
    }
}

/// An alias for `Result<T, auction::Error>`.
pub type Result<T> = result::Result<T, Error>;

// This error type is not intended to be used by third party crates.
#[doc(hidden)]
pub enum PurseLookupError {
    KeyNotFound,
    KeyUnexpectedType,
}

impl From<PurseLookupError> for Error {
    fn from(error: PurseLookupError) -> Self {
        match error {
            PurseLookupError::KeyNotFound => Error::MissingKey,
            PurseLookupError::KeyUnexpectedType => Error::InvalidKeyVariant,
        }
    }
}
