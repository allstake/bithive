RUSTFLAGS = "-C link-arg=-s"

all: lint btc-client

clean:
	rm -rf res

btc-client: contracts/btc-client
	$(call compile_release,btc-client)
	@mkdir -p res
	@cp target/wasm32-unknown-unknown/release/btc_client.wasm ./res/btc_client.wasm

btc-client-test: contracts/btc-client
	$(call compile_test,btc-client)
	@mkdir -p res
	@cp target/wasm32-unknown-unknown/release/btc_client.wasm ./res/btc_client_test.wasm

bip322-verifier: contracts/bip322-verifier
	$(call compile_release,bip322-verifier)
	@mkdir -p res
	@cp target/wasm32-unknown-unknown/release/bip322_verifier.wasm ./res/bip322_verifier.wasm

bip322-verifier-test: contracts/bip322-verifier
	$(call compile_test,bip322-verifier)
	@mkdir -p res
	@cp target/wasm32-unknown-unknown/release/bip322_verifier.wasm ./res/bip322_verifier_test.wasm

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
	@cargo test -- --nocapture

TEST_FILE ?= **
ifndef LOGS
	export NEAR_WORKSPACES_NO_LOGS=1
else
	export NEAR_PRINT_LOGS=1 
endif

test-ava: btc-client-test mock-btc-lightclient mock-chain-signature bip322-verifier-test
	npx ava -c 2 --timeout=5m tests/__tests__/$(TEST_FILE).ava.ts --verbose


test-integration: btc-client-test mock-btc-lightclient mock-chain-signature bip322-verifier-test
	npx ava -c 2 --timeout=5m tests/__tests__/integration/$(TEST_FILE).ava.ts --verbose

UNAME_S := $(shell uname -s)
ifeq ($(UNAME_S),Darwin)
	export AR=/opt/homebrew/opt/llvm/bin/llvm-ar
	export CC=/opt/homebrew/opt/llvm/bin/clang
endif

define compile_release
	@rustup target add wasm32-unknown-unknown
	RUSTFLAGS=$(RUSTFLAGS) cargo build --package $(1) --target wasm32-unknown-unknown --release
endef

define compile_test
	@rustup target add wasm32-unknown-unknown
	RUSTFLAGS=$(RUSTFLAGS) cargo build --package $(1) --target wasm32-unknown-unknown --release --features=test
endef
