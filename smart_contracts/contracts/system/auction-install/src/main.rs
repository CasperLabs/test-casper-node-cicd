#![no_std]
#![no_main]

extern crate alloc;

use alloc::collections::BTreeMap;
use casperlabs_contract::{
    contract_api::{runtime, storage},
    unwrap_or_revert::UnwrapOrRevert,
};
use casperlabs_types::{
    auction::{
        Bid, BidPurses, Bids, Delegators, EraValidators, SeigniorageRecipient,
        SeigniorageRecipients, SeigniorageRecipientsSnapshot, UnbondingPurses, ValidatorWeights,
        AUCTION_DELAY, BID_PURSES_KEY, ERA_ID_KEY, INITIAL_ERA_ID,
        SEIGNIORAGE_RECIPIENTS_SNAPSHOT_KEY, UNBONDING_PURSES_KEY,
    },
    auction::{BIDS_KEY, DELEGATORS_KEY, ERA_VALIDATORS_KEY},
    contracts::{NamedKeys, CONTRACT_INITIAL_VERSION},
    runtime_args,
    system_contract_errors::mint,
    CLValue, ContractPackageHash, PublicKey, RuntimeArgs, URef, U512,
};

const HASH_KEY_NAME: &str = "auction_hash";
const ACCESS_KEY_NAME: &str = "auction_access";
const ENTRY_POINT_MINT: &str = "mint";
const ARG_AMOUNT: &str = "amount";
const ARG_GENESIS_VALIDATORS: &str = "genesis_validators";

#[no_mangle]
pub extern "C" fn install() {
    let mint_package_hash: ContractPackageHash =
        runtime::get_named_arg("mint_contract_package_hash");

    let entry_points = auction::get_entry_points();
    let (contract_package_hash, access_uref) = storage::create_contract_package_at_hash();
    runtime::put_key(HASH_KEY_NAME, contract_package_hash.into());
    runtime::put_key(ACCESS_KEY_NAME, access_uref.into());

    let named_keys = {
        let mut named_keys = NamedKeys::new();

        let mut validators = Bids::new();

        let genesis_validators: BTreeMap<PublicKey, U512> =
            runtime::get_named_arg(ARG_GENESIS_VALIDATORS);

        // List of validators for initial era.
        let mut initial_validator_weights = ValidatorWeights::new();

        for (validator_account_hash, amount) in genesis_validators {
            let bonding_purse = create_purse(mint_package_hash, amount);
            let founding_validator = Bid::new(bonding_purse, amount);
            validators.insert(validator_account_hash, founding_validator);
            initial_validator_weights.insert(validator_account_hash, amount);
        }

        let initial_snapshot_range = INITIAL_ERA_ID..=INITIAL_ERA_ID + AUCTION_DELAY;

        // Starting era validators
        named_keys.insert(ERA_ID_KEY.into(), storage::new_uref(INITIAL_ERA_ID).into());

        let mut era_validators = EraValidators::new();
        for era_index in initial_snapshot_range.clone() {
            era_validators.insert(era_index, initial_validator_weights.clone());
        }

        let seigniorage_recipients = compute_seigniorage_recipients(&validators);

        let mut initial_seigniorage_recipients = SeigniorageRecipientsSnapshot::new();
        for era_id in initial_snapshot_range {
            initial_seigniorage_recipients.insert(era_id, seigniorage_recipients.clone());
        }
        named_keys.insert(
            SEIGNIORAGE_RECIPIENTS_SNAPSHOT_KEY.into(),
            storage::new_uref(initial_seigniorage_recipients).into(),
        );
        named_keys.insert(BIDS_KEY.into(), storage::new_uref(validators).into());
        named_keys.insert(
            DELEGATORS_KEY.into(),
            storage::new_uref(Delegators::new()).into(),
        );
        named_keys.insert(
            ERA_VALIDATORS_KEY.into(),
            storage::new_uref(era_validators).into(),
        );
        named_keys.insert(
            BID_PURSES_KEY.into(),
            storage::new_uref(BidPurses::new()).into(),
        );
        named_keys.insert(
            UNBONDING_PURSES_KEY.into(),
            storage::new_uref(UnbondingPurses::new()).into(),
        );

        named_keys
    };

    let (contract_key, _contract_version) =
        storage::add_contract_version(contract_package_hash, entry_points, named_keys);

    let return_value = CLValue::from_t(contract_key).unwrap_or_revert();
    runtime::ret(return_value);
}

fn compute_seigniorage_recipients(founding_validators: &Bids) -> SeigniorageRecipients {
    let mut seigniorage_recipients = SeigniorageRecipients::new();
    for (era_validator, founding_validator) in founding_validators {
        let seigniorage_recipient = SeigniorageRecipient::from(founding_validator);
        seigniorage_recipients.insert(*era_validator, seigniorage_recipient);
    }
    seigniorage_recipients
}

fn create_purse(contract_package_hash: ContractPackageHash, amount: U512) -> URef {
    let args = runtime_args! {
        ARG_AMOUNT => amount,
    };

    let result: Result<URef, mint::Error> = runtime::call_versioned_contract(
        contract_package_hash,
        Some(CONTRACT_INITIAL_VERSION),
        ENTRY_POINT_MINT,
        args,
    );

    result.unwrap_or_revert()
}