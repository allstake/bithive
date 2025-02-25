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
    signer: "bithive.testnet",
    owner: "bithive.testnet",
    bip322Verifier: "bip322.testnet",
    bithive: "incentivized.bithive.testnet",
    chainSignatures: "v1.signer-prod.testnet",
    btcLightClient: "btclc.testnet",
  },
  params: {
    nConfirmation: 3,
    withdrawalWaitingTimeMs: 2 * 24 * 60 * 60 * 1000, // 2 days
    minDepositSatoshi: 0,
    earliestDepositBlockHeight: 81500,
    soloWithdrawSeqHeights: [64000],
  },
};
