RUSTFLAGS = "-C link-arg=-s"

all: lint btc-client

btc-client: contracts/btc-client
	$(call compile_release,btc-client)
	@mkdir -p res
	@cp target/wasm32-unknown-unknown/release/btc_client.wasm ./res/btc_client.wasm

mock-btc-lightclient: contracts/mock-btc-lightclient
	$(call compile_release,mock-btc-lightclient)
	@mkdir -p res
	@cp target/wasm32-unknown-unknown/release/mock_btc_lightclient.wasm ./res/mock_btc_lightclient.wasm

lint:
	@cargo fmt --all
	@cargo clippy --fix --allow-dirty --allow-staged --features=test

test: test-unit

test-unit:
	@cargo test --features=test -- --nocapture


define compile_release
	@rustup target add wasm32-unknown-unknown
	AR=/opt/homebrew/opt/llvm/bin/llvm-ar CC=/opt/homebrew/opt/llvm/bin/clang RUSTFLAGS=$(RUSTFLAGS) cargo build --package $(1) --target wasm32-unknown-unknown --release
endef

define compile_test
	@rustup target add wasm32-unknown-unknown
	AR=/opt/homebrew/opt/llvm/bin/llvm-ar CC=/opt/homebrew/opt/llvm/bin/clang RUSTFLAGS=$(RUSTFLAGS) cargo build --package $(1) --target wasm32-unknown-unknown --features=test
endef
