use crate::{
    auction::{
        providers::StorageProvider, Bids, DelegatorRewardMap, Delegators, EraId, EraValidators,
        RuntimeProvider, SeigniorageRecipientsSnapshot, ValidatorRewardMap, BIDS_KEY,
        DELEGATORS_KEY, DELEGATOR_REWARD_MAP, ERA_ID_KEY, ERA_VALIDATORS_KEY,
        SEIGNIORAGE_RECIPIENTS_SNAPSHOT_KEY, VALIDATOR_REWARD_MAP,
    },
    bytesrepr::{FromBytes, ToBytes},
    system_contract_errors::auction::{Error, Result},
    CLTyped,
};

fn read_from<P, T>(provider: &mut P, name: &str) -> Result<T>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
    T: FromBytes + CLTyped,
{
    let key = provider.get_key(name).ok_or(Error::MissingKey)?;
    let uref = key.into_uref().ok_or(Error::InvalidKeyVariant)?;
    let value: T = provider.read(uref)?.ok_or(Error::MissingValue)?;
    Ok(value)
}

fn write_to<P, T>(provider: &mut P, name: &str, value: T) -> Result<()>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
    T: ToBytes + CLTyped,
{
    let key = provider.get_key(name).ok_or(Error::MissingKey)?;
    let uref = key.into_uref().ok_or(Error::InvalidKeyVariant)?;
    provider.write(uref, value)?;
    Ok(())
}

pub fn get_bids<P>(provider: &mut P) -> Result<Bids>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
{
    Ok(read_from(provider, BIDS_KEY)?)
}

pub fn set_bids<P>(provider: &mut P, validators: Bids) -> Result<()>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
{
    write_to(provider, BIDS_KEY, validators)
}

pub fn get_delegators<P>(provider: &mut P) -> Result<Delegators>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
{
    read_from(provider, DELEGATORS_KEY)
}

pub fn set_delegators<P>(provider: &mut P, delegators: Delegators) -> Result<()>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
{
    write_to(provider, DELEGATORS_KEY, delegators)
}

pub fn get_delegator_reward_map<P>(provider: &mut P) -> Result<DelegatorRewardMap>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
{
    read_from(provider, DELEGATOR_REWARD_MAP)
}

pub fn set_delegator_reward_map<P>(
    provider: &mut P,
    delegator_reward_map: DelegatorRewardMap,
) -> Result<()>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
{
    write_to(provider, DELEGATOR_REWARD_MAP, delegator_reward_map)
}

pub fn get_validator_reward_map<P>(provider: &mut P) -> Result<ValidatorRewardMap>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
{
    read_from(provider, VALIDATOR_REWARD_MAP)
}

pub fn set_validator_reward_map<P>(
    provider: &mut P,
    validator_reward_map: ValidatorRewardMap,
) -> Result<()>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
{
    write_to(provider, VALIDATOR_REWARD_MAP, validator_reward_map)
}

pub fn get_era_validators<P>(provider: &mut P) -> Result<EraValidators>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
{
    Ok(read_from(provider, ERA_VALIDATORS_KEY)?)
}

pub fn set_era_validators<P>(provider: &mut P, era_validators: EraValidators) -> Result<()>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
{
    write_to(provider, ERA_VALIDATORS_KEY, era_validators)
}

pub fn get_era_id<P>(provider: &mut P) -> Result<EraId>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
{
    Ok(read_from(provider, ERA_ID_KEY)?)
}

pub fn set_era_id<P>(provider: &mut P, era_id: u64) -> Result<()>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
{
    write_to(provider, ERA_ID_KEY, era_id)
}

pub fn get_seigniorage_recipients_snapshot<P>(
    provider: &mut P,
) -> Result<SeigniorageRecipientsSnapshot>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
{
    Ok(read_from(provider, SEIGNIORAGE_RECIPIENTS_SNAPSHOT_KEY)?)
}

pub fn set_seigniorage_recipients_snapshot<P>(
    provider: &mut P,
    snapshot: SeigniorageRecipientsSnapshot,
) -> Result<()>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
{
    write_to(provider, SEIGNIORAGE_RECIPIENTS_SNAPSHOT_KEY, snapshot)
}
