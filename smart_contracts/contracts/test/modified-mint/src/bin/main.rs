#![no_std]
#![no_main]

#[no_mangle]
pub extern "C" fn mint() {
    modified_mint::mint();
}

#[no_mangle]
pub extern "C" fn create() {
    modified_mint::create();
}

#[no_mangle]
pub extern "C" fn balance() {
    modified_mint::balance();
}

#[no_mangle]
pub extern "C" fn transfer() {
    modified_mint::transfer();
}

#[no_mangle]
pub extern "C" fn read_base_round_reward() {
    modified_mint::read_base_round_reward()
}
