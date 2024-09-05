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

## About redeem script
Redeem scripts, along with const values used by them, are strictly versioned. Once a specific version of the redeem script goes alive it should **NEVER** be changed.    
In order to use a new deposit redeem script (the script itself or its consts), please follow these steps:
1. Define a new deposit message `DEPOSIT_MSG_HEX_Vx` and put the hex encoded string in `consts.rs`
2. Specify chain signature path `CHAIN_SIGNATURE_PATH_Vx` and key version `CHAIN_SIGNATURE_KEY_VERSION_Vx` in `consts.rs`
3. Add a new entry for `RedeemVersion` in `types.rs`
4. Define a function that could verify if a deposit txn is of the newly-created version in `deposit.rs`, like `verify_deposit_output_v1`
5. Update the `redeem_version` match in `submit_deposit_tx` with the new verify function above
6. Update the `deposit.redeem_version()` match in `sign_withdrawal` with new consts defined in step 2
