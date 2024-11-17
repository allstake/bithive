# Integration Tests
Tests in this folder are supposed to run against a local BTC regtest network and request signatures from the actual testnet chain signatures contract (`v1.signer-prod.testnet`)

To run integration tests, you must first:
1. Run a bitcoin node by: `docker run -d -p 8080:8080 --name regtest junderw/bitcoinjs-regtest-server`
2. Prepare a NEAR testnet account to call chain signatures contract. Put the account ID in `.env`

Then run `make test-integration`
