use casper_engine_test_support::{
    internal::{
        utils, ExecuteRequestBuilder, InMemoryWasmTestBuilder, DEFAULT_ACCOUNTS,
        DEFAULT_ACCOUNT_PUBLIC_KEY, DEFAULT_PAYMENT, DEFAULT_RUN_GENESIS_REQUEST,
    },
    DEFAULT_ACCOUNT_ADDR,
};
use casper_execution_engine::{core::engine_state::genesis::GenesisAccount, shared::motes::Motes};
use casper_types::{
    account::AccountHash,
    auction::{
        BidPurses, DelegationRate, UnbondingPurses, ARG_UNBOND_PURSE, ARG_VALIDATOR_PUBLIC_KEYS,
        BID_PURSES_KEY, DEFAULT_UNBONDING_DELAY, INITIAL_ERA_ID, METHOD_RUN_AUCTION, METHOD_SLASH,
        UNBONDING_PURSES_KEY,
    },
    runtime_args,
    system_contract_errors::auction,
    ApiError, PublicKey, RuntimeArgs, URef, U512,
};

const CONTRACT_TRANSFER_TO_ACCOUNT: &str = "transfer_to_account_u512.wasm";
const CONTRACT_ADD_BID: &str = "add_bid.wasm";
const CONTRACT_WITHDRAW_BID: &str = "withdraw_bid.wasm";
const CONTRACT_AUCTION_BIDDING: &str = "auction_bidding.wasm";
const CONTRACT_AUCTION_BIDS: &str = "auction_bids.wasm";
const CONTRACT_CREATE_PURSE_01: &str = "create_purse_01.wasm";

const GENESIS_VALIDATOR_STAKE: u64 = 50_000;
const GENESIS_ACCOUNT_STAKE: u64 = 100_000;
const TRANSFER_AMOUNT: u64 = 500_000_000;

const TEST_BOND_FROM_MAIN_PURSE: &str = "bond-from-main-purse";
const TEST_SEED_NEW_ACCOUNT: &str = "seed_new_account";

const ARG_AMOUNT: &str = "amount";
const ARG_PUBLIC_KEY: &str = "public_key";
const ARG_ENTRY_POINT: &str = "entry_point";
const ARG_ACCOUNT_HASH: &str = "account_hash";
const ARG_RUN_AUCTION: &str = "run_auction";
const ARG_DELEGATION_RATE: &str = "delegation_rate";
const ARG_PURSE_NAME: &str = "purse_name";

const SYSTEM_ADDR: AccountHash = AccountHash::new([0u8; 32]);
const UNBONDING_PURSE_NAME: &str = "unbonding_purse";

#[ignore]
#[test]
fn should_run_successful_bond_and_unbond_and_slashing() {
    let default_public_key_arg = *DEFAULT_ACCOUNT_PUBLIC_KEY;
    let mut builder = InMemoryWasmTestBuilder::default();
    builder.run_genesis(&DEFAULT_RUN_GENESIS_REQUEST);

    let exec_request = ExecuteRequestBuilder::standard(
        *DEFAULT_ACCOUNT_ADDR,
        CONTRACT_TRANSFER_TO_ACCOUNT,
        runtime_args! {
            "target" => SYSTEM_ADDR,
            "amount" => U512::from(TRANSFER_AMOUNT)
        },
    )
    .build();

    builder.exec(exec_request).expect_success().commit();

    let _default_account = builder
        .get_account(*DEFAULT_ACCOUNT_ADDR)
        .expect("should get account 1");

    let auction = builder.get_auction_contract_hash();

    let exec_request_1 = ExecuteRequestBuilder::standard(
        *DEFAULT_ACCOUNT_ADDR,
        CONTRACT_ADD_BID,
        runtime_args! {
            ARG_AMOUNT => U512::from(GENESIS_ACCOUNT_STAKE),
            ARG_PUBLIC_KEY => default_public_key_arg,
            ARG_DELEGATION_RATE => DelegationRate::from(42u8),
        },
    )
    .build();

    builder.exec(exec_request_1).expect_success().commit();

    let bid_purses: BidPurses = builder.get_value(auction, BID_PURSES_KEY);
    let bid_purse = bid_purses
        .get(&*DEFAULT_ACCOUNT_PUBLIC_KEY)
        .expect("should have bid purse");
    assert_eq!(
        builder.get_purse_balance(*bid_purse),
        GENESIS_ACCOUNT_STAKE.into()
    );

    let unbond_purses: UnbondingPurses = builder.get_value(auction, UNBONDING_PURSES_KEY);
    assert_eq!(unbond_purses.len(), 0);

    //
    // Partial unbond
    //

    let unbond_amount = U512::from(GENESIS_ACCOUNT_STAKE) - 1;

    let exec_request_2 = ExecuteRequestBuilder::standard(
        *DEFAULT_ACCOUNT_ADDR,
        CONTRACT_CREATE_PURSE_01,
        runtime_args! {
            ARG_PURSE_NAME => UNBONDING_PURSE_NAME,
        },
    )
    .build();

    builder.exec(exec_request_2).expect_success().commit();
    let unbonding_purse = builder
        .get_account(*DEFAULT_ACCOUNT_ADDR)
        .expect("should have default account")
        .named_keys()
        .get(UNBONDING_PURSE_NAME)
        .expect("should have unbonding purse")
        .into_uref()
        .expect("unbonding purse should be an uref");

    let exec_request_3 = ExecuteRequestBuilder::standard(
        *DEFAULT_ACCOUNT_ADDR,
        CONTRACT_WITHDRAW_BID,
        runtime_args! {
            ARG_AMOUNT => unbond_amount,
            ARG_PUBLIC_KEY => default_public_key_arg,
            ARG_UNBOND_PURSE => Some(unbonding_purse),
        },
    )
    .build();

    builder.exec(exec_request_3).expect_success().commit();

    let unbond_purses: UnbondingPurses = builder.get_value(auction, UNBONDING_PURSES_KEY);
    assert_eq!(unbond_purses.len(), 1);

    let unbond_list = unbond_purses
        .get(&*DEFAULT_ACCOUNT_PUBLIC_KEY)
        .expect("should have unbond");
    assert_eq!(unbond_list.len(), 1);
    assert_eq!(unbond_list[0].origin, default_public_key_arg,);
    assert_eq!(
        builder.get_purse_balance(unbond_list[0].purse),
        U512::zero(),
    );

    assert_eq!(
        unbond_list[0].era_of_withdrawal as usize,
        INITIAL_ERA_ID as usize + DEFAULT_UNBONDING_DELAY as usize
    );

    let unbond_era_1 = unbond_list[0].era_of_withdrawal;

    let exec_request_3 = ExecuteRequestBuilder::contract_call_by_hash(
        SYSTEM_ADDR,
        auction,
        METHOD_RUN_AUCTION,
        runtime_args! {},
    )
    .build();

    builder.exec(exec_request_3).expect_success().commit();

    let unbond_purses: UnbondingPurses = builder.get_value(auction, UNBONDING_PURSES_KEY);
    assert_eq!(unbond_purses.len(), 1);

    let unbond_list = unbond_purses
        .get(&*DEFAULT_ACCOUNT_PUBLIC_KEY)
        .expect("should have unbond");
    assert_eq!(unbond_list.len(), 1);
    assert_eq!(unbond_list[0].origin, default_public_key_arg,);
    assert_eq!(
        builder.get_purse_balance(unbond_list[0].purse),
        U512::zero(),
    );
    assert_eq!(unbond_list[0].amount, unbond_amount,);

    let unbond_era_2 = unbond_list[0].era_of_withdrawal;

    assert_eq!(unbond_era_2, unbond_era_1);

    let exec_request_4 = ExecuteRequestBuilder::contract_call_by_hash(
        SYSTEM_ADDR,
        auction,
        METHOD_SLASH,
        runtime_args! {
            ARG_VALIDATOR_PUBLIC_KEYS => vec![
               default_public_key_arg,
            ]
        },
    )
    .build();

    builder.exec(exec_request_4).expect_success().commit();

    let unbond_purses: UnbondingPurses = builder.get_value(auction, UNBONDING_PURSES_KEY);
    let unbond_list = unbond_purses
        .get(&*DEFAULT_ACCOUNT_PUBLIC_KEY)
        .expect("should have unbond");
    assert_eq!(unbond_list.len(), 0); // removed unbonds

    let bid_purses: BidPurses = builder.get_value(auction, BID_PURSES_KEY);

    assert!(bid_purses.is_empty());
}

#[ignore]
#[test]
fn should_fail_bonding_with_insufficient_funds() {
    let account_1_public_key: PublicKey = PublicKey::Ed25519([123; 32]);
    let account_1_hash = AccountHash::from(account_1_public_key);

    let exec_request_1 = ExecuteRequestBuilder::standard(
        *DEFAULT_ACCOUNT_ADDR,
        CONTRACT_AUCTION_BIDDING,
        runtime_args! {
            ARG_ENTRY_POINT => TEST_SEED_NEW_ACCOUNT,
            ARG_ACCOUNT_HASH => account_1_hash,
            ARG_AMOUNT => *DEFAULT_PAYMENT + GENESIS_ACCOUNT_STAKE,
        },
    )
    .build();
    let exec_request_2 = ExecuteRequestBuilder::standard(
        account_1_hash,
        CONTRACT_AUCTION_BIDDING,
        runtime_args! {
            ARG_ENTRY_POINT => TEST_BOND_FROM_MAIN_PURSE,
            ARG_AMOUNT => *DEFAULT_PAYMENT + GENESIS_ACCOUNT_STAKE,
            ARG_PUBLIC_KEY => account_1_public_key,
        },
    )
    .build();

    let mut builder = InMemoryWasmTestBuilder::default();

    builder
        .run_genesis(&DEFAULT_RUN_GENESIS_REQUEST)
        .exec(exec_request_1)
        .commit();

    builder.exec(exec_request_2).commit();

    let response = builder
        .get_exec_response(1)
        .expect("should have a response")
        .to_owned();

    let error_message = utils::get_error_message(response);

    assert!(
        error_message.contains(&format!("{:?}", ApiError::from(auction::Error::Transfer))),
        "error: {:?}",
        error_message
    );
}

#[ignore]
#[test]
fn should_fail_unbonding_validator_with_locked_funds() {
    let account_1_public_key = PublicKey::Ed25519([42; 32]);
    let account_1_hash = AccountHash::from(account_1_public_key);
    let account_1_balance = U512::from(1_000_000_000);

    let accounts = {
        let mut tmp: Vec<GenesisAccount> = DEFAULT_ACCOUNTS.clone();
        let account = GenesisAccount::new(
            account_1_public_key,
            account_1_hash,
            Motes::new(account_1_balance),
            Motes::new(GENESIS_VALIDATOR_STAKE.into()),
        );
        tmp.push(account);
        tmp
    };

    let run_genesis_request = utils::create_run_genesis_request(accounts);

    let exec_request_1 = ExecuteRequestBuilder::standard(
        *DEFAULT_ACCOUNT_ADDR,
        CONTRACT_CREATE_PURSE_01,
        runtime_args! {
            ARG_PURSE_NAME => UNBONDING_PURSE_NAME,
        },
    )
    .build();

    let mut builder = InMemoryWasmTestBuilder::default();

    builder.run_genesis(&run_genesis_request);

    builder.exec(exec_request_1).expect_success().commit();

    let unbonding_purse = builder
        .get_account(*DEFAULT_ACCOUNT_ADDR)
        .expect("should have default account")
        .named_keys()
        .get(UNBONDING_PURSE_NAME)
        .expect("should have unbonding purse")
        .into_uref()
        .expect("unbonding purse should be an uref");

    let exec_request_2 = ExecuteRequestBuilder::standard(
        account_1_hash,
        CONTRACT_WITHDRAW_BID,
        runtime_args! {
            ARG_AMOUNT => U512::from(42),
            ARG_PUBLIC_KEY => account_1_public_key,
            ARG_UNBOND_PURSE => Some(unbonding_purse)
        },
    )
    .build();

    builder.exec(exec_request_2).commit();

    let response = builder
        .get_exec_response(1)
        .expect("should have a response")
        .to_owned();

    let error_message = utils::get_error_message(response);

    // pos::Error::NotBonded => 0
    assert!(
        error_message.contains(&format!(
            "{:?}",
            ApiError::from(auction::Error::ValidatorFundsLocked)
        )),
        "error {:?}",
        error_message
    );
}

#[ignore]
#[test]
fn should_fail_unbonding_validator_without_bonding_first() {
    let exec_request = ExecuteRequestBuilder::standard(
        *DEFAULT_ACCOUNT_ADDR,
        CONTRACT_WITHDRAW_BID,
        runtime_args! {
            ARG_AMOUNT => U512::from(42),
            ARG_PUBLIC_KEY => *DEFAULT_ACCOUNT_PUBLIC_KEY,
            ARG_UNBOND_PURSE => Option::<URef>::None,
        },
    )
    .build();

    let mut builder = InMemoryWasmTestBuilder::default();

    builder.run_genesis(&DEFAULT_RUN_GENESIS_REQUEST);

    builder.exec(exec_request).commit();

    let response = builder
        .get_exec_response(0)
        .expect("should have a response")
        .to_owned();

    let error_message = utils::get_error_message(response);

    assert!(
        error_message.contains(&format!(
            "{:?}",
            ApiError::from(auction::Error::ValidatorNotFound)
        )),
        "error {:?}",
        error_message
    );
}

#[ignore]
#[test]
fn should_run_successful_bond_and_unbond_with_release() {
    let default_public_key_arg = *DEFAULT_ACCOUNT_PUBLIC_KEY;

    let mut builder = InMemoryWasmTestBuilder::default();
    builder.run_genesis(&DEFAULT_RUN_GENESIS_REQUEST);

    let create_purse_request_1 = ExecuteRequestBuilder::standard(
        *DEFAULT_ACCOUNT_ADDR,
        CONTRACT_CREATE_PURSE_01,
        runtime_args! {
            ARG_PURSE_NAME => UNBONDING_PURSE_NAME,
        },
    )
    .build();

    builder
        .exec(create_purse_request_1)
        .expect_success()
        .commit();
    let unbonding_purse = builder
        .get_account(*DEFAULT_ACCOUNT_ADDR)
        .expect("should have default account")
        .named_keys()
        .get(UNBONDING_PURSE_NAME)
        .expect("should have unbonding purse")
        .into_uref()
        .expect("unbonding purse should be an uref");

    let exec_request = ExecuteRequestBuilder::standard(
        *DEFAULT_ACCOUNT_ADDR,
        CONTRACT_TRANSFER_TO_ACCOUNT,
        runtime_args! {
            "target" => SYSTEM_ADDR,
            "amount" => U512::from(TRANSFER_AMOUNT)
        },
    )
    .build();

    builder.exec(exec_request).expect_success().commit();

    let _default_account = builder
        .get_account(*DEFAULT_ACCOUNT_ADDR)
        .expect("should get account 1");

    let auction = builder.get_auction_contract_hash();

    let exec_request_1 = ExecuteRequestBuilder::standard(
        *DEFAULT_ACCOUNT_ADDR,
        CONTRACT_ADD_BID,
        runtime_args! {
            ARG_AMOUNT => U512::from(GENESIS_ACCOUNT_STAKE),
            ARG_PUBLIC_KEY => default_public_key_arg,
            ARG_DELEGATION_RATE => DelegationRate::from(42u8),
        },
    )
    .build();

    builder.exec(exec_request_1).expect_success().commit();

    let bid_purses: BidPurses = builder.get_value(auction, BID_PURSES_KEY);
    let bid_purse = bid_purses
        .get(&default_public_key_arg)
        .expect("should have bid purse");
    assert_eq!(
        builder.get_purse_balance(*bid_purse),
        GENESIS_ACCOUNT_STAKE.into()
    );

    let unbond_purses: UnbondingPurses = builder.get_value(auction, UNBONDING_PURSES_KEY);
    assert_eq!(unbond_purses.len(), 0);

    //
    // Advance era by calling run_auction
    //
    let run_auction_request_1 = ExecuteRequestBuilder::standard(
        SYSTEM_ADDR,
        CONTRACT_AUCTION_BIDS,
        runtime_args! {
            ARG_ENTRY_POINT => ARG_RUN_AUCTION,
        },
    )
    .build();

    builder
        .exec(run_auction_request_1)
        .commit()
        .expect_success();

    //
    // Partial unbond
    //

    let unbond_amount = U512::from(GENESIS_ACCOUNT_STAKE) - 1;

    let exec_request_2 = ExecuteRequestBuilder::standard(
        *DEFAULT_ACCOUNT_ADDR,
        CONTRACT_WITHDRAW_BID,
        runtime_args! {
            ARG_AMOUNT => unbond_amount,
            ARG_PUBLIC_KEY => default_public_key_arg,
            ARG_UNBOND_PURSE => Some(unbonding_purse),
        },
    )
    .build();

    builder.exec(exec_request_2).expect_success().commit();

    let unbond_purses: UnbondingPurses = builder.get_value(auction, UNBONDING_PURSES_KEY);
    assert_eq!(unbond_purses.len(), 1);

    let unbond_list = unbond_purses
        .get(&*DEFAULT_ACCOUNT_PUBLIC_KEY)
        .expect("should have unbond");
    assert_eq!(unbond_list.len(), 1);
    assert_eq!(unbond_list[0].origin, default_public_key_arg,);
    assert_eq!(
        builder.get_purse_balance(unbond_list[0].purse),
        U512::zero(),
    );

    assert_eq!(
        unbond_list[0].era_of_withdrawal as usize,
        INITIAL_ERA_ID as usize + 1 + DEFAULT_UNBONDING_DELAY as usize
    );

    let unbond_era_1 = unbond_list[0].era_of_withdrawal;

    let exec_request_3 = ExecuteRequestBuilder::contract_call_by_hash(
        SYSTEM_ADDR,
        auction,
        METHOD_RUN_AUCTION,
        runtime_args! {},
    )
    .build();

    builder.exec(exec_request_3).expect_success().commit();

    let unbond_purses: UnbondingPurses = builder.get_value(auction, UNBONDING_PURSES_KEY);
    assert_eq!(unbond_purses.len(), 1);

    let unbond_list = unbond_purses
        .get(&default_public_key_arg)
        .expect("should have unbond");
    assert_eq!(unbond_list.len(), 1);
    assert_eq!(unbond_list[0].origin, default_public_key_arg,);

    assert_eq!(unbonding_purse, unbond_list[0].purse);
    assert_ne!(
        unbond_list[0].purse,
        *bid_purse // unbond purse is different than bid purse
    );
    assert_eq!(
        unbond_list[0].purse,
        unbonding_purse, // unbond purse is not changed
    );
    assert_eq!(
        builder.get_purse_balance(unbonding_purse),
        U512::zero(), // Not paid yet
    );

    let unbond_era_2 = unbond_list[0].era_of_withdrawal;

    assert_eq!(unbond_era_2, unbond_era_1); // era of withdrawal didn't change since first run

    //
    // Advance state to hit the unbonding period
    //

    for _ in 0..DEFAULT_UNBONDING_DELAY {
        let run_auction_request_1 = ExecuteRequestBuilder::standard(
            SYSTEM_ADDR,
            CONTRACT_AUCTION_BIDS,
            runtime_args! {
                ARG_ENTRY_POINT => ARG_RUN_AUCTION,
            },
        )
        .build();

        builder
            .exec(run_auction_request_1)
            .commit()
            .expect_success();
    }

    // Should pay out

    let exec_request_4 = ExecuteRequestBuilder::contract_call_by_hash(
        SYSTEM_ADDR,
        auction,
        METHOD_RUN_AUCTION,
        runtime_args! {},
    )
    .build();

    builder.exec(exec_request_4).expect_success().commit();

    assert_eq!(builder.get_purse_balance(unbonding_purse), unbond_amount);

    let unbond_purses: UnbondingPurses = builder.get_value(auction, UNBONDING_PURSES_KEY);
    assert!(
        !unbond_purses.contains_key(&*DEFAULT_ACCOUNT_PUBLIC_KEY),
        "Unbond entry should be removed"
    );

    let bid_purses: BidPurses = builder.get_value(auction, BID_PURSES_KEY);

    assert!(!bid_purses.is_empty());
    assert_eq!(
        builder.get_purse_balance(
            *bid_purses
                .get(&default_public_key_arg)
                .expect("should have unbond")
        ),
        U512::from(GENESIS_ACCOUNT_STAKE) - unbond_amount, // remaining funds
    );
}
