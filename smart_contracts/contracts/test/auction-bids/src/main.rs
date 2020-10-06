#![no_std]
#![no_main]

extern crate alloc;

use alloc::{collections::BTreeMap, string::String};

use casper_contract::contract_api::{account, runtime, storage, system};

use casper_types::{
    auction::{
        SeigniorageRecipients, ARG_DELEGATOR, ARG_DELEGATOR_PUBLIC_KEY, ARG_REWARD_FACTORS,
        ARG_SOURCE_PURSE, ARG_TARGET_PURSE, ARG_VALIDATOR, ARG_VALIDATOR_PUBLIC_KEY,
        METHOD_DELEGATE, METHOD_DISTRIBUTE, METHOD_READ_SEIGNIORAGE_RECIPIENTS, METHOD_RUN_AUCTION,
        METHOD_UNDELEGATE, METHOD_WITHDRAW_DELEGATOR_REWARD, METHOD_WITHDRAW_VALIDATOR_REWARD,
    },
    runtime_args, ApiError, PublicKey, RuntimeArgs, URef, U512,
};

const ARG_ENTRY_POINT: &str = "entry_point";
const ARG_AMOUNT: &str = "amount";
const ARG_DELEGATE: &str = "delegate";
const ARG_UNDELEGATE: &str = "undelegate";
const ARG_RUN_AUCTION: &str = "run_auction";
const ARG_READ_SEIGNIORAGE_RECIPIENTS: &str = "read_seigniorage_recipients";

const REWARD_PURSE: &str = "reward_purse";
const DELEGATE_PURSE: &str = "delegate_purse";
const UNDELEGATE_PURSE: &str = "undelegate_purse";

#[repr(u16)]
enum Error {
    UnknownCommand,
}

#[no_mangle]
pub extern "C" fn call() {
    let command: String = runtime::get_named_arg(ARG_ENTRY_POINT);

    match command.as_str() {
        ARG_DELEGATE => delegate(),
        ARG_UNDELEGATE => undelegate(),
        ARG_RUN_AUCTION => run_auction(),
        ARG_READ_SEIGNIORAGE_RECIPIENTS => read_seigniorage_recipients(),
        METHOD_DISTRIBUTE => distribute(),
        METHOD_WITHDRAW_DELEGATOR_REWARD => withdraw_delegator_reward(),
        METHOD_WITHDRAW_VALIDATOR_REWARD => withdraw_validator_reward(),
        _ => runtime::revert(ApiError::User(Error::UnknownCommand as u16)),
    }
}

fn delegate() {
    let auction = system::get_auction();
    let delegator: PublicKey = runtime::get_named_arg(ARG_DELEGATOR);
    let validator: PublicKey = runtime::get_named_arg(ARG_VALIDATOR);
    let amount: U512 = runtime::get_named_arg(ARG_AMOUNT);
    let args = runtime_args! {
        ARG_DELEGATOR => delegator,
        ARG_VALIDATOR => validator,
        ARG_SOURCE_PURSE => account::get_main_purse(),
        ARG_AMOUNT => amount,
    };

    let (purse, _amount): (URef, U512) = runtime::call_contract(auction, METHOD_DELEGATE, args);

    runtime::put_key(DELEGATE_PURSE, purse.into())
}

fn undelegate() {
    let auction = system::get_auction();
    let amount: U512 = runtime::get_named_arg(ARG_AMOUNT);
    let delegator: PublicKey = runtime::get_named_arg(ARG_DELEGATOR);
    let validator: PublicKey = runtime::get_named_arg(ARG_VALIDATOR);

    let args = runtime_args! {
        ARG_AMOUNT => amount,
        ARG_VALIDATOR => validator,
        ARG_DELEGATOR => delegator,
    };

    let (purse, _remaining_bid): (URef, U512) =
        runtime::call_contract(auction, METHOD_UNDELEGATE, args);

    runtime::put_key(UNDELEGATE_PURSE, purse.into());
}

fn run_auction() {
    let auction = system::get_auction();
    let args = runtime_args! {};
    runtime::call_contract::<()>(auction, METHOD_RUN_AUCTION, args);
}

fn read_seigniorage_recipients() {
    let auction = system::get_auction();
    let args = runtime_args! {};
    let result: SeigniorageRecipients =
        runtime::call_contract(auction, METHOD_READ_SEIGNIORAGE_RECIPIENTS, args);
    let uref = storage::new_uref(result);
    runtime::put_key("seigniorage_recipients_result", uref.into());
}

fn distribute() {
    let auction = system::get_auction();
    let reward_factors: BTreeMap<PublicKey, u64> = runtime::get_named_arg(ARG_REWARD_FACTORS);
    let args = runtime_args! {
        ARG_REWARD_FACTORS => reward_factors
    };
    runtime::call_contract::<()>(auction, METHOD_DISTRIBUTE, args);
}

fn withdraw_delegator_reward() {
    let auction = system::get_auction();
    let validator_public_key: PublicKey = runtime::get_named_arg(ARG_VALIDATOR_PUBLIC_KEY);
    let delegator_public_key: PublicKey = runtime::get_named_arg(ARG_DELEGATOR_PUBLIC_KEY);

    let reward_purse = system::create_purse();

    runtime::put_key(REWARD_PURSE, reward_purse.into());

    let args = runtime_args! {
        ARG_VALIDATOR_PUBLIC_KEY => validator_public_key,
        ARG_DELEGATOR_PUBLIC_KEY => delegator_public_key,
        ARG_TARGET_PURSE => reward_purse,
    };
    runtime::call_contract::<()>(auction, METHOD_WITHDRAW_DELEGATOR_REWARD, args);
}

fn withdraw_validator_reward() {
    let auction = system::get_auction();
    let validator_public_key: PublicKey = runtime::get_named_arg(ARG_VALIDATOR_PUBLIC_KEY);

    let reward_purse = system::create_purse();

    runtime::put_key(REWARD_PURSE, reward_purse.into());

    let args = runtime_args! {
        ARG_VALIDATOR_PUBLIC_KEY => validator_public_key,
        ARG_TARGET_PURSE => reward_purse,
    };
    runtime::call_contract::<()>(auction, METHOD_WITHDRAW_VALIDATOR_REWARD, args);
}
