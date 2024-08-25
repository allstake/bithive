RUSTFLAGS = "-C link-arg=-s"

all: lint btc-client

btc-client: contracts/btc-client
	$(call compile_release,btc-client)
	@mkdir -p res
	@cp target/wasm32-unknown-unknown/release/btc_client.wasm ./res/btc_client.wasm

btc-client-test: contracts/btc-client
	$(call compile_test,btc-client)
	@mkdir -p res
	@cp target/wasm32-unknown-unknown/debug/btc_client.wasm ./res/btc_client_test.wasm

mock-btc-lightclient: contracts/mock-btc-lightclient
	$(call compile_release,mock-btc-lightclient)
	@mkdir -p res
	@cp target/wasm32-unknown-unknown/release/mock_btc_lightclient.wasm ./res/mock_btc_lightclient.wasm

mock-chain-signature: contracts/mock-chain-signature
	$(call compile_release,mock-chain-signature)
	@mkdir -p res
	@cp target/wasm32-unknown-unknown/release/mock_chain_signature.wasm ./res/mock_chain_signature.wasm

lint:
	@cargo fmt --all
	@cargo clippy --fix --allow-dirty --allow-staged --features=test

test: test-unit test-ava test-integration

test-unit:
	@cargo test --features=test -- --nocapture

TEST_FILE ?= **
LOGS ?=

test-ava: btc-client-test mock-btc-lightclient mock-chain-signature
	NEAR_PRINT_LOGS=$(LOGS) npx ava --timeout=5m tests/__tests__/$(TEST_FILE).ava.ts --verbose


test-integration: btc-client-test mock-btc-lightclient mock-chain-signature
	NEAR_PRINT_LOGS=$(LOGS) npx ava --timeout=5m tests/__tests__/integration/$(TEST_FILE).ava.ts --verbose

define compile_release
	@rustup target add wasm32-unknown-unknown
	AR=/opt/homebrew/opt/llvm/bin/llvm-ar CC=/opt/homebrew/opt/llvm/bin/clang RUSTFLAGS=$(RUSTFLAGS) cargo build --package $(1) --target wasm32-unknown-unknown --release
endef

define compile_test
	@rustup target add wasm32-unknown-unknown
	AR=/opt/homebrew/opt/llvm/bin/llvm-ar CC=/opt/homebrew/opt/llvm/bin/clang RUSTFLAGS=$(RUSTFLAGS) cargo build --package $(1) --target wasm32-unknown-unknown --features=test
endef
