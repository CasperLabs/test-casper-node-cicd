# This supports environments where $HOME/.cargo/env has not been sourced (CI, CLion Makefile runner)
CARGO  = $(or $(shell which cargo),  $(HOME)/.cargo/bin/cargo)
RUSTUP = $(or $(shell which rustup), $(HOME)/.cargo/bin/rustup)
NPM    = $(or $(shell which npm),    /usr/bin/npm)

RUST_TOOLCHAIN := $(shell cat rust-toolchain)

CARGO_OPTS := --locked
CARGO := $(CARGO) $(CARGO_TOOLCHAIN) $(CARGO_OPTS)

DISABLE_LOGGING = RUST_LOG=MatchesNothing

# Rust Contracts
# Directory names should match crate names
BENCH       = $(shell find ./smart_contracts/contracts/bench       -mindepth 1 -maxdepth 1 -type d -exec basename {} \;)
CLIENT      = $(shell find ./smart_contracts/contracts/client      -mindepth 1 -maxdepth 1 -type d -exec basename {} \;)
EXPLORER    = $(shell find ./smart_contracts/contracts/explorer    -mindepth 1 -maxdepth 1 -type d -exec basename {} \;)
INTEGRATION = $(shell find ./smart_contracts/contracts/integration -mindepth 1 -maxdepth 1 -type d -exec basename {} \;)
PROFILING   = $(shell find ./smart_contracts/contracts/profiling   -mindepth 1 -maxdepth 1 -type d -exec basename {} \;)
SRE         = $(shell find ./smart_contracts/contracts/SRE         -mindepth 1 -maxdepth 1 -type d -exec basename {} \;)
SYSTEM      = $(shell find ./smart_contracts/contracts/system      -mindepth 1 -maxdepth 1 -type d -exec basename {} \;)
TEST        = $(shell find ./smart_contracts/contracts/test        -mindepth 1 -maxdepth 1 -type d -exec basename {} \;)

BENCH_CONTRACTS     := $(patsubst %, build-contract-rs/%, $(BENCH))
CLIENT_CONTRACTS    := $(patsubst %, build-contract-rs/%, $(CLIENT))
EXPLORER_CONTRACTS  := $(patsubst %, build-contract-rs/%, $(EXPLORER))
PROFILING_CONTRACTS := $(patsubst %, build-contract-rs/%, $(PROFILING))
SRE_CONTRACTS       := $(patsubst %, build-contract-rs/%, $(SRE))
TEST_CONTRACTS      := $(patsubst %, build-contract-rs/%, $(TEST))

# AssemblyScript Contracts
CLIENT_CONTRACTS_AS  = $(shell find ./smart_contracts/contracts_as/client   -mindepth 1 -maxdepth 1 -type d)
TEST_CONTRACTS_AS    = $(shell find ./smart_contracts/contracts_as/test     -mindepth 1 -maxdepth 1 -type d)

CLIENT_CONTRACTS_AS  := $(patsubst %, build-contract-as/%, $(CLIENT_CONTRACTS_AS))
TEST_CONTRACTS_AS    := $(patsubst %, build-contract-as/%, $(TEST_CONTRACTS_AS))

INTEGRATION += \
	endless-loop \
	local-state \
	modified-system-upgrader \
	pos-bonding \
	remove-associated-key \
	standard-payment \
	transfer-to-account-u512

HIGHWAY_CONTRACTS += \
	pos-install \
	pos

SYSTEM_CONTRACTS          := $(patsubst %, build-contract-rs/%,                 $(SYSTEM))
SYSTEM_CONTRACTS_FEATURED := $(patsubst %, build-system-contract-featured-rs/%, $(SYSTEM))

CONTRACT_TARGET_DIR       = target/wasm32-unknown-unknown/release
CONTRACT_TARGET_DIR_AS    = target_as
PACKAGED_SYSTEM_CONTRACTS = mint_install.wasm pos_install.wasm standard_payment_install.wasm auction_install.wasm
TOOL_TARGET_DIR           = grpc/cargo_casper/target
TOOL_WASM_DIR             = grpc/cargo_casper/wasm

CRATES_WITH_DOCS_RS_MANIFEST_TABLE = \
	grpc/server \
	grpc/test_support \
	node \
	smart_contracts/contract \
	types

CRATES_WITH_DOCS_RS_MANIFEST_TABLE := $(patsubst %, doc-stable/%, $(CRATES_WITH_DOCS_RS_MANIFEST_TABLE))

.PHONY: all
all: build build-contracts

.PHONY: build
build:
	$(CARGO) build $(CARGO_FLAGS)

build-contract-rs/%:
	$(CARGO) build \
	        --release $(filter-out --release, $(CARGO_FLAGS)) \
	        --package $* \
	        --target wasm32-unknown-unknown

build-system-contract-featured-rs/%:
	$(CARGO) build \
	        --release $(filter-out --release, $(CARGO_FLAGS)) \
	        --manifest-path "smart_contracts/contracts/system/$*/Cargo.toml" $(if $(FEATURES),$(if $(filter $(HIGHWAY_CONTRACTS), $*),--features $(FEATURES))) \
	        --target wasm32-unknown-unknown

build-contracts-rs: \
	$(BENCH_CONTRACTS) \
	$(CLIENT_CONTRACTS) \
	$(EXPLORER_CONTRACTS) \
	$(INTEGRATION_CONTRACTS) \
	$(PROFILING_CONTRACTS) \
	$(SRE_CONTRACTS) \
	$(SYSTEM_CONTRACTS) \
	$(TEST_CONTRACTS)

.PHONY: build-system-contracts
build-system-contracts: $(SYSTEM_CONTRACTS)

build-contract-as/%:
	cd $* && $(NPM) run asbuild

.PHONY: build-contracts-as
build-contracts-as: \
	$(CLIENT_CONTRACTS_AS) \
	$(TEST_CONTRACTS_AS) \
	$(EXAMPLE_CONTRACTS_AS)

.PHONY: build-contracts
build-contracts: build-contracts-rs build-contracts-as

.PHONY: test-rs
test-rs: build-system-contracts
	$(DISABLE_LOGGING) $(CARGO) test $(CARGO_FLAGS) --workspace

.PHONY: test-as
test-as: setup-as
	cd smart_contracts/contract_as && npm run asbuild && npm run test

.PHONY: test
test: test-rs test-as

.PHONY: test-contracts-rs
test-contracts-rs: build-contracts-rs
	$(DISABLE_LOGGING) $(CARGO) test $(CARGO_FLAGS) -p casper-engine-tests -- --ignored
	$(DISABLE_LOGGING) $(CARGO) test $(CARGO_FLAGS) --manifest-path "grpc/tests/Cargo.toml" --features "use-system-contracts" -- --ignored

.PHONY: test-contracts_as
test-contracts_as: build-contracts-rs build-contracts-as
	@# see https://github.com/rust-lang/cargo/issues/5015#issuecomment-515544290
	$(DISABLE_LOGGING) $(CARGO) test $(CARGO_FLAGS) --manifest-path "grpc/tests/Cargo.toml" --features "use-as-wasm" -- --ignored

.PHONY: test-contracts
test-contracts: test-contracts-rs test-contracts_as

.PHONY: check-format
check-format:
	$(CARGO) fmt --all -- --check

.PHONY: format
format:
	$(CARGO) fmt --all

.PHONY: lint
lint:
	$(CARGO) clippy --all-targets --all-features --workspace -- -D warnings -A renamed_and_removed_lints

.PHONY: audit
audit:
	$(CARGO) audit

.PHONY: build-docs-stable-rs
build-docs-stable-rs: $(CRATES_WITH_DOCS_RS_MANIFEST_TABLE)

doc-stable/%: CARGO_TOOLCHAIN += +stable
doc-stable/%:
	$(CARGO) doc $(CARGO_FLAGS) --manifest-path "$*/Cargo.toml" --features "no-unstable-features" --no-deps

.PHONY: check-rs
check-rs: \
	build-docs-stable-rs \
	build \
	check-format \
	lint \
	audit \
	test-rs \
	test-contracts-rs

.PHONY: check
check: \
	build-docs-stable-rs \
	build \
	check-format \
	lint \
	audit \
	test \
	test-contracts

.PHONY: clean
clean:
	rm -rf $(CONTRACT_TARGET_DIR_AS)
	rm -rf $(TOOL_TARGET_DIR)
	rm -rf $(TOOL_WASM_DIR)
	$(CARGO) clean

.PHONY: build-for-packaging
build-for-packaging: build-system-contracts
	$(CARGO) build --release

.PHONY: deb
deb: build-for-packaging
	cd grpc/server && $(CARGO) deb -p casper-engine-grpc-server --no-build
	cd node && $(CARGO) deb -p casper-node --no-build

grpc/server/.rpm:
	cd grpc/server && $(CARGO) rpm init

.PHONY: rpm
rpm: grpc/server/.rpm
	cd grpc/server && $(CARGO) rpm build

target/system-contracts.tar.gz: $(SYSTEM_CONTRACTS)
	tar -czf $@ -C $(CONTRACT_TARGET_DIR) $(PACKAGED_SYSTEM_CONTRACTS)

.PHONY: package-system-contracts
package-system-contracts: target/system-contracts.tar.gz

.PHONY: package
package:
	cd contract && $(CARGO) package

.PHONY: publish
publish:
	./publish.sh

.PHONY: bench
bench: build-contracts-rs
	$(CARGO) bench

.PHONY: setup-cargo-packagers
setup-cargo-packagers:
	$(CARGO) install cargo-rpm || exit 0
	$(CARGO) install cargo-deb || exit 0

.PHONY: setup-audit
setup-audit:
	$(CARGO) install cargo-audit

.PHONY: setup-rs
setup-rs: rust-toolchain
	$(RUSTUP) update --no-self-update
	$(RUSTUP) toolchain install --no-self-update $(RUST_TOOLCHAIN)
	$(RUSTUP) target add --toolchain $(RUST_TOOLCHAIN) wasm32-unknown-unknown

.PHONY: setup-stable-rs
setup-stable-rs: RUST_TOOLCHAIN := stable
setup-stable-rs: setup-rs

.PHONY: setup-nightly-rs
setup-nightly-rs: RUST_TOOLCHAIN := nightly
setup-nightly-rs: setup-rs

.PHONY: setup-as
setup-as: smart_contracts/contract_as/package.json
	cd smart_contracts/contract_as && $(NPM) ci

.PHONY: setup
setup: setup-rs setup-as
