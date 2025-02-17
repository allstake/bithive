import { UnencryptedFileSystemKeyStore } from "near-api-js/lib/key_stores";
import * as path from "path";
import * as os from "os";
import { Config } from "./index";

/**
 * dev env in testnet3 is deprecated, use testnet4 or signet instead
 */
export const config: Config = {
  bitcoin: {
    network: "testnet",
    detailedName: "testnet3",
  },
  near: {
    networkId: "testnet",
    keyStore: new UnencryptedFileSystemKeyStore(
      path.join(os.homedir(), ".near-credentials"),
    ),
    nodeUrl:
      process.env.NEAR_CLI_TESTNET_RPC_SERVER_URL ??
      "https://rpc.testnet.near.org",
  },
  accountIds: {
    signer: "allbtc.testnet",
    owner: "allbtc.testnet",
    bip322Verifier: "bip322.testnet",
    bithive: "testnet3.allbtc.testnet",
    chainSignatures: "v1.signer-dev.testnet",
    btcLightClient: "btc-client.testnet",
  },
  params: {
    nConfirmation: 2,
    withdrawalWaitingTimeMs: 5 * 60 * 1000, // 5 minutes
    minDepositSatoshi: 0,
    earliestDepositBlockHeight: 0,
    soloWithdrawSeqHeights: [2],
  },
};
