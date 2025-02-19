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
    signer: "bithive-staging.near",
    owner: "bithive-staging.near",
    // bip322Verifier: "bip322.near",
    bithive: "bithive-staging.near",
    chainSignatures: "v1.signer",
    btcLightClient: "btc-client.bridge.near",
  },
  params: {
    nConfirmation: 6,
    withdrawalWaitingTimeMs: 2 * 24 * 3600 * 1000, // 2 days
    minDepositSatoshi: 0,
    earliestDepositBlockHeight: 884168,
    soloWithdrawSeqHeights: [64000],
  },
};
