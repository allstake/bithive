import { UnencryptedFileSystemKeyStore } from "near-api-js/lib/key_stores";
import * as path from "path";
import * as os from "os";
import { Config } from "./index";

export const config: Config = {
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
    btcClient: "2.allbtc.testnet",
    chainSignature: "v1.signer-dev.testnet",
    btcLightClient: "btc-client.testnet",
  },
  params: {
    nConfirmation: 1,
    withdrawWaitingTimeMs: 5 * 60 * 1000, // 5 minutes
    minDepositSatoshi: 100,
    earliestDepositBlockHeight: 0,
    soloWithdrawSeqHeights: [2],
  },
};
