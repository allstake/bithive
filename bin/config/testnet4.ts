import { UnencryptedFileSystemKeyStore } from "near-api-js/lib/key_stores";
import * as path from "path";
import * as os from "os";
import { Config } from "./index";

export const config: Config = {
  bitcoin: {
    network: "testnet",
    detailedName: "testnet4",
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
    bithive: "testnet4-3.allbtc.testnet",
    chainSignatures: "v1.signer-dev.testnet",
    btcLightClient: "btclc.testnet",
  },
  params: {
    nConfirmation: 2,
    withdrawalWaitingTimeMs: 5 * 60 * 1000, // 5 minutes
    minDepositSatoshi: 0,
    earliestDepositBlockHeight: 0,
    soloWithdrawSeqHeights: [2],
  },
};
