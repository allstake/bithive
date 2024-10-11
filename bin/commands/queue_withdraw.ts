import { CommandModule } from "yargs";
import { getConfig } from "../config";
import { envBuilder, nearTGas } from "../helper";
import { initNear } from "../near";

interface Args {
  env: string;
  pubkey: string;
  txid: string;
  vout: number;
  sig: string;
}

export const queueWithdraw: CommandModule<unknown, Args> = {
  command: "queue",
  describe: "Submit a BTC queue withdraw request",
  builder: {
    env: envBuilder,
    pubkey: {
      describe: "User public key",
      type: "string",
      demandOption: true,
    },
    txid: {
      describe: "Deposit txid",
      type: "string",
      demandOption: true,
    },
    vout: {
      describe: "Deposit vout",
      type: "number",
      demandOption: true,
    },
    sig: {
      describe: "Signature",
      type: "string",
      demandOption: true,
    },
  },
  async handler({ env, pubkey, txid, vout, sig }) {
    const config = await getConfig(env);
    const { signer } = await initNear(env);

    const args = {
      user_pubkey: pubkey,
      deposit_tx_id: txid,
      deposit_vout: vout,
      msg_sig: sig,
      sig_type: "Unisat",
    };

    await signer.functionCall({
      contractId: config.accountIds.btcClient,
      methodName: "queue_withdrawal",
      args: args,
      gas: nearTGas(100),
    });

    console.log(
      "Queued withdraw",
      `${args.deposit_tx_id}:${args.deposit_vout}`,
    );
  },
};
