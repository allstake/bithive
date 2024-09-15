import { KeyStore } from "near-api-js/lib/key_stores";

export interface Config {
  near: {
    networkId: string;
    keyStore: KeyStore;
    nodeUrl: string;
  };
  accountIds: {
    signer: string;
    owner: string;
    btcClient: string;
    chainSignature: string;
    btcLightClient: string;
  };
  params: {
    nConfirmation: number;
    withdrawWaitingTimeMs: number;
    minDepositSatoshi: number;
    earliestDepositBlockHeight: number;
    soloWithdrawSeqHeights: number[];
  };
}

export async function getConfig(env: string): Promise<Config> {
  const module = await import(`./${env}.ts`);
  return module.config;
}
