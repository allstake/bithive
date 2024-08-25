# Allstake BTC Client Contracts

## Build
**NOTE**: If you are building on an ARM mac, run `brew install llvm` first!
- `make all`

## Test
Prepare:
1. have `nodejs` version >= 20
2. complete `Build` preparations
3. if you want to run integration tests, follow `tests/__tests__/integration/README.md` to setup for integration tests

- `make test-unit`: run rust unit tests
- `make test-ava`: run near workspace tests
- `make test-integration`: run integration tests
- `make test`: run all tests
