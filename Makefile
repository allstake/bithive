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

mock-btc-light-client: contracts/mock-btc-light-client
	$(call compile_release,mock-btc-light-client)
	@mkdir -p res
	@cp target/wasm32-unknown-unknown/release/mock_btc_light_client.wasm ./res/mock_btc_light_client.wasm

mock-chain-signatures: contracts/mock-chain-signatures
	$(call compile_release,mock-chain-signatures)
	@mkdir -p res
	@cp target/wasm32-unknown-unknown/release/mock_chain_signatures.wasm ./res/mock_chain_signatures.wasm

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

test-assets: btc-client-test mock-btc-light-client mock-chain-signatures bip322-verifier-test

# by default you should run ava test through this command
test-ava: test-assets
	npx ava -c 2 --timeout=5m tests/__tests__/$(TEST_FILE).ava.ts --verbose

# by default you should run integration test through this command
test-integration: test-assets
	npx ava -c 2 --timeout=5m tests/__tests__/integration/$(TEST_FILE).ava.ts --verbose

# assets built by ARM mac may fail in tests with an error like `{"CompilationError":{"PrepareError":"Deserialization"}}`,
# you can try to build assets via docker and then run tests through commands below, see README.md for more details

test-ava-no-build:
	npx ava -c 2 --timeout=5m tests/__tests__/$(TEST_FILE).ava.ts --verbose

test-integration-no-build:
	npx ava -c 2 --timeout=5m tests/__tests__/integration/$(TEST_FILE).ava.ts --verbose

# build assets via docker
build-docker:
	-rm res/*.*
	docker build -t bithive-assets .
	docker run -v ./res:/app/res -v ./contracts:/app/contracts -it bithive-assets make btc-client test-assets

define compile_release
	@rustup target add wasm32-unknown-unknown
	RUSTFLAGS=$(RUSTFLAGS) cargo build --package $(1) --target wasm32-unknown-unknown --release
endef

define compile_test
	@rustup target add wasm32-unknown-unknown
	RUSTFLAGS=$(RUSTFLAGS) cargo build --package $(1) --target wasm32-unknown-unknown --release --features=test
endef
