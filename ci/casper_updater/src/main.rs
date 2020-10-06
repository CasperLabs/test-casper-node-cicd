//! A tool to update versions of all published CasperLabs packages.

#![warn(unused, missing_copy_implementations, missing_docs)]
#![deny(
    deprecated_in_future,
    future_incompatible,
    macro_use_extern_crate,
    rust_2018_idioms,
    nonstandard_style,
    single_use_lifetimes,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unstable_features,
    unused_import_braces,
    unused_lifetimes,
    unused_qualifications,
    unused_results,
    warnings,
    clippy::all
)]
#![forbid(
    const_err,
    arithmetic_overflow,
    invalid_type_param_default,
    macro_expanded_macro_exports_accessed_by_absolute_paths,
    missing_fragment_specifier,
    mutable_transmutes,
    no_mangle_const_items,
    order_dependent_trait_objects,
    overflowing_literals,
    pub_use_of_private_extern_crate,
    unknown_crate_types
)]

mod dependent_file;
mod package;
mod regex_data;

use std::{
    env,
    path::{Path, PathBuf},
    str::FromStr,
};

use clap::{crate_version, App, Arg};
use lazy_static::lazy_static;

use package::Package;

const APP_NAME: &str = "Casper Updater";

const ROOT_DIR_ARG_NAME: &str = "root-dir";
const ROOT_DIR_ARG_SHORT: &str = "r";
const ROOT_DIR_ARG_VALUE_NAME: &str = "PATH";
const ROOT_DIR_ARG_HELP: &str =
    "Path to casper-node root directory.  If not supplied, assumes it is at ../..";

const BUMP_ARG_NAME: &str = "bump";
const BUMP_ARG_SHORT: &str = "b";
const BUMP_ARG_VALUE_NAME: &str = "VERSION-COMPONENT";
const BUMP_ARG_HELP: &str =
    "Increase all crates' versions automatically without asking for user input.  For a crate at \
    version x.y.z, the version will be bumped to (x+1).0.0, x.(y+1).0, or x.y.(z+1) depending on \
    which version component is specified";
const MAJOR: &str = "major";
const MINOR: &str = "minor";
const PATCH: &str = "patch";

const DRY_RUN_ARG_NAME: &str = "dry-run";
const DRY_RUN_ARG_SHORT: &str = "d";
const DRY_RUN_ARG_HELP: &str = "Check all regexes get matches in current casper-node repo";

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub(crate) enum BumpVersion {
    Major,
    Minor,
    Patch,
}

struct Args {
    root_dir: PathBuf,
    bump_version: Option<BumpVersion>,
    dry_run: bool,
}

/// The full path to the casper-node root directory.
pub(crate) fn root_dir() -> &'static Path {
    &ARGS.root_dir
}

/// The version component to bump, if any.
pub(crate) fn bump_version() -> Option<BumpVersion> {
    ARGS.bump_version
}

/// Whether we're doing a dry run or not.
pub(crate) fn is_dry_run() -> bool {
    ARGS.dry_run
}

lazy_static! {
    static ref ARGS: Args = get_args();
}

fn get_args() -> Args {
    let arg_matches = App::new(APP_NAME)
        .version(crate_version!())
        .arg(
            Arg::with_name(ROOT_DIR_ARG_NAME)
                .long(ROOT_DIR_ARG_NAME)
                .short(ROOT_DIR_ARG_SHORT)
                .value_name(ROOT_DIR_ARG_VALUE_NAME)
                .help(ROOT_DIR_ARG_HELP)
                .takes_value(true),
        )
        .arg(
            Arg::with_name(BUMP_ARG_NAME)
                .long(BUMP_ARG_NAME)
                .short(BUMP_ARG_SHORT)
                .value_name(BUMP_ARG_VALUE_NAME)
                .help(BUMP_ARG_HELP)
                .takes_value(true)
                .possible_values(&[MAJOR, MINOR, PATCH]),
        )
        .arg(
            Arg::with_name(DRY_RUN_ARG_NAME)
                .long(DRY_RUN_ARG_NAME)
                .short(DRY_RUN_ARG_SHORT)
                .help(DRY_RUN_ARG_HELP),
        )
        .get_matches();

    let root_dir = match arg_matches.value_of(ROOT_DIR_ARG_NAME) {
        Some(path) => PathBuf::from_str(path).expect("should be a valid unicode path"),
        None => env::current_dir()
            .expect("should be able to access current working dir")
            .parent()
            .expect("current working dir should have parent")
            .parent()
            .expect("current working dir should have two parents")
            .to_path_buf(),
    };

    let bump_version = arg_matches
        .value_of(BUMP_ARG_NAME)
        .map(|value| match value {
            MAJOR => BumpVersion::Major,
            MINOR => BumpVersion::Minor,
            PATCH => BumpVersion::Patch,
            _ => unreachable!(),
        });

    let dry_run = arg_matches.is_present(DRY_RUN_ARG_NAME);

    Args {
        root_dir,
        bump_version,
        dry_run,
    }
}

fn main() {
    let types = Package::cargo("types", &*regex_data::types::DEPENDENT_FILES);
    types.update();

    let execution_engine = Package::cargo(
        "execution_engine",
        &*regex_data::execution_engine::DEPENDENT_FILES,
    );
    execution_engine.update();

    let node = Package::cargo("node", &*regex_data::node::DEPENDENT_FILES);
    node.update();

    let grpc_server = Package::cargo("grpc/server", &*regex_data::grpc_server::DEPENDENT_FILES);
    grpc_server.update();

    let client = Package::cargo("client", &*regex_data::client::DEPENDENT_FILES);
    client.update();

    let smart_contracts_contract = Package::cargo(
        "smart_contracts/contract",
        &*regex_data::smart_contracts_contract::DEPENDENT_FILES,
    );
    smart_contracts_contract.update();

    let smart_contracts_contract_as = Package::assembly_script(
        "smart_contracts/contract_as",
        &*regex_data::smart_contracts_contract_as::DEPENDENT_FILES,
    );
    smart_contracts_contract_as.update();

    let grpc_test_support = Package::cargo(
        "grpc/test_support",
        &*regex_data::grpc_test_support::DEPENDENT_FILES,
    );
    grpc_test_support.update();

    let grpc_cargo_casper = Package::cargo(
        "grpc/cargo_casper",
        &*regex_data::grpc_cargo_casper::DEPENDENT_FILES,
    );
    grpc_cargo_casper.update();
}
