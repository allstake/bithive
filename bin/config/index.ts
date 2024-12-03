import { KeyStore } from "near-api-js/lib/key_stores";

export interface Config {
  bitcoin: {
    network: "testnet" | "mainnet";
  };
  near: {
    networkId: string;
    keyStore: KeyStore;
    nodeUrl: string;
  };
  accountIds: {
    signer: string;
    owner: string;
    bithive: string;
    bip322Verifier?: string;
    chainSignatures: string;
    btcLightClient: string;
  };
  params: {
    nConfirmation: number;
    withdrawalWaitingTimeMs: number;
    minDepositSatoshi: number;
    earliestDepositBlockHeight: number;
    soloWithdrawSeqHeights: number[];
  };
}

export async function getConfig(env: string): Promise<Config> {
  const module = await import(`./${env}.ts`);
  return module.config;
}
