import { CommandModule } from "yargs";
import { envBuilder, nearTGas } from "../helper";
import { initNear } from "../near";
import { getConfig } from "../config";

interface Args {
  env: string;
}

export const init: CommandModule<unknown, Args> = {
  command: "init",
  describe: "Initialize contracts",
  builder: {
    env: envBuilder,
  },
  async handler({ env }) {
    const config = await getConfig(env);
    const { signer } = await initNear(env, config.accountIds.btcClient);

    const args = {
      owner_id: config.accountIds.owner,
      bip322_verifier_id: config.accountIds.bip322Verifier,
      btc_lightclient_id: config.accountIds.btcLightClient,
      chain_signature_id: config.accountIds.chainSignature,
      n_confirmation: config.params.nConfirmation,
      withdraw_waiting_time_ms: config.params.withdrawWaitingTimeMs,
      min_deposit_satoshi: config.params.minDepositSatoshi,
      earliest_deposit_block_height: config.params.earliestDepositBlockHeight,
      solo_withdraw_seq_heights: config.params.soloWithdrawSeqHeights,
    };

    await signer.functionCall({
      contractId: config.accountIds.btcClient,
      methodName: "init",
      args: { args },
    });
    console.log("Called init method");

    // sync root public key
    await signer.functionCall({
      contractId: config.accountIds.btcClient,
      methodName: "sync_chain_signature_root_pubkey",
      gas: nearTGas(100),
    });
    console.log("Called sync_chain_signature_root_pubkey method");

    process.exit(0);
  },
};
