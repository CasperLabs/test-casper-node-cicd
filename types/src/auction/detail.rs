use alloc::vec::Vec;

use num_rational::Ratio;

use super::{
    Auction, BidPurses, UnbondingPurse, UnbondingPurses, BID_PURSES_KEY, DEFAULT_UNBONDING_DELAY,
    SYSTEM_ACCOUNT, UNBONDING_PURSES_KEY,
};
use crate::{
    auction::{internal, MintProvider, RuntimeProvider, StorageProvider, SystemProvider},
    system_contract_errors::auction::{Error, Result},
    Key, PublicKey, URef, U512,
};

/// Iterates over unbonding entries and checks if a locked amount can be paid already if
/// a specific era is reached.
///
/// This function can be called by the system only.
pub(crate) fn process_unbond_requests<P: Auction + ?Sized>(provider: &mut P) -> Result<()> {
    if provider.get_caller() != SYSTEM_ACCOUNT {
        return Err(Error::InvalidCaller);
    }
    let bid_purses_uref = provider
        .get_key(BID_PURSES_KEY)
        .and_then(Key::into_uref)
        .ok_or(Error::MissingKey)?;

    let bid_purses: BidPurses = provider.read(bid_purses_uref)?.ok_or(Error::Storage)?;

    // Update `unbonding_purses` data
    let unbonding_purses_uref = provider
        .get_key(UNBONDING_PURSES_KEY)
        .and_then(Key::into_uref)
        .ok_or(Error::MissingKey)?;
    let mut unbonding_purses: UnbondingPurses = provider
        .read(unbonding_purses_uref)?
        .ok_or(Error::Storage)?;

    let current_era_id = provider.read_era_id()?;

    for unbonding_list in unbonding_purses.values_mut() {
        let mut new_unbonding_list = Vec::new();
        for unbonding_purse in unbonding_list.iter() {
            let source = bid_purses
                .get(&unbonding_purse.origin)
                .ok_or(Error::BondNotFound)?;
            // Since `process_unbond_requests` is run before `run_auction`, we should check
            // if current era id is equal or greater than the `era_of_withdrawal` that was
            // calculated on `unbond` attempt.
            if current_era_id >= unbonding_purse.era_of_withdrawal as u64 {
                // Move funds from bid purse to unbonding purse
                provider.transfer_from_purse_to_purse(
                    *source,
                    unbonding_purse.purse,
                    unbonding_purse.amount,
                )?;
            } else {
                new_unbonding_list.push(*unbonding_purse);
            }
        }
        *unbonding_list = new_unbonding_list;
    }

    // Prune empty entries
    let new_unbonding_purses: UnbondingPurses = unbonding_purses
        .into_iter()
        .filter(|(_k, unbonding_purses)| !unbonding_purses.is_empty())
        .collect();

    provider.write(unbonding_purses_uref, new_unbonding_purses)?;
    Ok(())
}

/// Creates a new purse in bid_purses corresponding to a validator's key, or tops off an
/// existing one.
///
/// Returns the bid purse's key and current amount of motes.
pub(crate) fn bond<P: Auction + ?Sized>(
    provider: &mut P,
    public_key: PublicKey,
    source: URef,
    amount: U512,
) -> Result<(URef, U512)> {
    if amount.is_zero() {
        return Err(Error::BondTooSmall);
    }

    let bid_purses_uref = provider
        .get_key(BID_PURSES_KEY)
        .and_then(Key::into_uref)
        .ok_or(Error::MissingKey)?;

    let mut bid_purses: BidPurses = provider.read(bid_purses_uref)?.ok_or(Error::Storage)?;

    let target = match bid_purses.get(&public_key) {
        Some(purse) => *purse,
        None => {
            let new_purse = provider.create_purse();
            bid_purses.insert(public_key, new_purse);
            provider.write(bid_purses_uref, bid_purses)?;
            new_purse
        }
    };

    provider.transfer_from_purse_to_purse(source, target, amount)?;

    let total_amount = provider.get_balance(target)?.unwrap();

    Ok((target, total_amount))
}

/// Creates a new purse in unbonding_purses given a validator's key, amount, and a destination
/// unbonding purse. Returns the amount of motes remaining in the validator's bid purse.
pub(crate) fn unbond<P: Auction + ?Sized>(
    provider: &mut P,
    public_key: PublicKey,
    amount: U512,
    unbond_purse: URef,
) -> Result<U512> {
    let bid_purses_uref = provider
        .get_key(BID_PURSES_KEY)
        .and_then(Key::into_uref)
        .ok_or(Error::MissingKey)?;

    let bid_purses: BidPurses = provider.read(bid_purses_uref)?.ok_or(Error::Storage)?;

    let bid_purse = bid_purses
        .get(&public_key)
        .copied()
        .ok_or(Error::BondNotFound)?;

    if provider.get_balance(bid_purse)?.unwrap_or_default() < amount {
        return Err(Error::UnbondTooLarge);
    }

    // Update `unbonding_purses` data
    let unbonding_purses_uref = provider
        .get_key(UNBONDING_PURSES_KEY)
        .and_then(Key::into_uref)
        .ok_or(Error::MissingKey)?;
    let mut unbonding_purses: UnbondingPurses = provider
        .read(unbonding_purses_uref)?
        .ok_or(Error::Storage)?;

    let current_era_id = provider.read_era_id()?;
    let new_unbonding_purse = UnbondingPurse {
        purse: unbond_purse,
        origin: public_key,
        era_of_withdrawal: current_era_id + DEFAULT_UNBONDING_DELAY,
        amount,
    };
    unbonding_purses
        .entry(public_key)
        .or_default()
        .push(new_unbonding_purse);
    provider.write(unbonding_purses_uref, unbonding_purses)?;

    // Remaining motes in the validator's bid purse
    let remaining_bond = provider.get_balance(bid_purse)?.unwrap_or_default();
    Ok(remaining_bond)
}

/// Update delegators entry. Initialize if it doesn't exist.
pub fn update_delegators<P>(
    provider: &mut P,
    validator_public_key: PublicKey,
    delegator_public_key: PublicKey,
    delegation_amount: U512,
) -> Result<U512>
where
    P: RuntimeProvider + StorageProvider + ?Sized,
{
    let mut delegators = internal::get_delegators(provider)?;
    let new_quantity = *delegators
        .entry(validator_public_key)
        .or_default()
        .entry(delegator_public_key)
        .and_modify(|delegation| *delegation += delegation_amount)
        .or_insert_with(|| delegation_amount);
    internal::set_delegators(provider, delegators)?;
    Ok(new_quantity)
}

/// Update validator reward map.
pub fn update_delegator_rewards<P>(
    provider: &mut P,
    validator_public_key: PublicKey,
    rewards: impl Iterator<Item = (PublicKey, Ratio<U512>)>,
) -> Result<U512>
where
    P: MintProvider + RuntimeProvider + StorageProvider + SystemProvider + ?Sized,
{
    let mut total_delegator_payout = U512::zero();
    let mut outer = internal::get_delegator_reward_map(provider)?;
    let mut inner = outer.remove(&validator_public_key).unwrap_or_default();

    for (delegator_key, delegator_reward) in rewards {
        let delegator_reward_trunc = delegator_reward.to_integer();
        inner
            .entry(delegator_key)
            .and_modify(|sum| *sum += delegator_reward_trunc)
            .or_insert_with(|| delegator_reward_trunc);
        total_delegator_payout += delegator_reward_trunc;
    }

    outer.insert(validator_public_key, inner);
    internal::set_delegator_reward_map(provider, outer)?;
    Ok(total_delegator_payout)
}

/// Update validator reward map.
pub fn update_validator_reward<P>(
    provider: &mut P,
    validator_public_key: PublicKey,
    amount: U512,
) -> Result<()>
where
    P: MintProvider + RuntimeProvider + StorageProvider + SystemProvider + ?Sized,
{
    let mut validator_reward_map = internal::get_validator_reward_map(provider)?;
    validator_reward_map
        .entry(validator_public_key)
        .and_modify(|sum| *sum += amount)
        .or_insert_with(|| amount);
    internal::set_validator_reward_map(provider, validator_reward_map)?;
    Ok(())
}

/// Removes validator entries from either founders or validators, wherever they
/// might be found.
///
/// This function is intended to be called together with the slash function in the Mint
/// contract.
pub(crate) fn quash_bid<P: StorageProvider + RuntimeProvider + ?Sized>(
    provider: &mut P,
    validator_public_keys: &[PublicKey],
) -> Result<()> {
    // Clean up inside `bids`
    let mut validators = internal::get_bids(provider)?;

    let mut modified_validators = 0usize;

    for validator_public_key in validator_public_keys {
        if validators.remove(validator_public_key).is_some() {
            modified_validators += 1;
        }
    }

    if modified_validators > 0 {
        internal::set_bids(provider, validators)?;
    }

    Ok(())
}
