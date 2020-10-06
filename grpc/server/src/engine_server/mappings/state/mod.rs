//! Functions for converting between Casper types and their Protobuf equivalents which are
//! defined in protobuf/io/casperlabs/casper/consensus/state.proto

mod account;
pub(crate) mod big_int;
mod cl_type;
mod cl_value;
mod contract;
mod contract_package;
mod contract_wasm;
mod key;
mod named_key;
mod protocol_version;
mod semver;
mod stored_value;
mod uref;

pub(crate) use named_key::NamedKeyMap;
