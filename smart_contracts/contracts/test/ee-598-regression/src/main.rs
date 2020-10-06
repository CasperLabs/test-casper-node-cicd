#![no_std]
#![no_main]

use auction::DelegationRate;
use casper_contract::contract_api::{account, runtime, system};
use casper_types::{auction, runtime_args, ContractHash, PublicKey, RuntimeArgs, URef, U512};

const ARG_AMOUNT: &str = "amount";
const ARG_PUBLIC_KEY: &str = "public_key";

fn add_bid(
    contract_hash: ContractHash,
    public_key: PublicKey,
    bond_amount: U512,
    bonding_purse: URef,
) {
    let runtime_args = runtime_args! {
        auction::ARG_PUBLIC_KEY => public_key,
        auction::ARG_SOURCE_PURSE => bonding_purse,
        auction::ARG_DELEGATION_RATE => DelegationRate::from(42u8),
        auction::ARG_AMOUNT => bond_amount,
    };
    runtime::call_contract::<(URef, U512)>(contract_hash, auction::METHOD_ADD_BID, runtime_args);
}

fn withdraw_bid(
    contract_hash: ContractHash,
    public_key: PublicKey,
    unbond_amount: U512,
    unbond_purse: URef,
) -> U512 {
    let args = runtime_args! {
        auction::ARG_AMOUNT => unbond_amount,
        auction::ARG_PUBLIC_KEY => public_key,
        auction::ARG_UNBOND_PURSE => unbond_purse,
    };
    runtime::call_contract(contract_hash, auction::METHOD_WITHDRAW_BID, args)
}

#[no_mangle]
pub extern "C" fn call() {
    let amount: U512 = runtime::get_named_arg(ARG_AMOUNT);
    let public_key = runtime::get_named_arg(ARG_PUBLIC_KEY);
    // unbond attempt for more than is staked should fail
    let contract_hash = system::get_auction();
    add_bid(contract_hash, public_key, amount, account::get_main_purse());
    withdraw_bid(
        contract_hash,
        public_key,
        amount + 1,
        account::get_main_purse(),
    );
}
