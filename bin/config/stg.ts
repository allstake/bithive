import { UnencryptedFileSystemKeyStore } from "near-api-js/lib/key_stores";
import * as path from "path";
import * as os from "os";
import { Config } from "./index";

export const config: Config = {
  bitcoin: {
    network: "mainnet",
    detailedName: "mainnet",
  },
  near: {
    networkId: "mainnet",
    keyStore: new UnencryptedFileSystemKeyStore(
      path.join(os.homedir(), ".near-credentials"),
    ),
    nodeUrl:
      process.env.NEAR_CLI_MAINNET_RPC_SERVER_URL ??
      "https://rpc.mainnet.near.org",
  },
  accountIds: {
    signer: "btchive0.mainnet",
    owner: "btchive0.mainnet",
    // bip322Verifier: "bip322.mainnet",
    bithive: "stg1.btchive0.near",
    chainSignatures: "v1.signer",
    btcLightClient: "btc-client.bridge.near",
  },
  params: {
    nConfirmation: 2,
    withdrawalWaitingTimeMs: 5 * 60 * 1000, // 5 minutes
    minDepositSatoshi: 0,
    earliestDepositBlockHeight: 0,
    soloWithdrawSeqHeights: [2],
  },
};
