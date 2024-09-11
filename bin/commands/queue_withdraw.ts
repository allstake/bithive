import { CommandModule } from "yargs";
import { getConfig } from "../config";
import { envBuilder, nearTGas } from "../helper";
import { initNear } from "../near";

interface Args {
  env: string;
}

export const queueWithdraw: CommandModule<unknown, Args> = {
  command: "queue",
  describe: "Submit a BTC queue withdraw request",
  builder: {
    env: envBuilder,
  },
  async handler({ env }) {
    const config = await getConfig(env);
    const { signer } = await initNear(env);

    const args = {
      user_pubkey:
        "0299b4097603b073aa2390203303fe0e60c87bd2af8e621a3df22818c40e3dd217",
      deposit_tx_id:
        "1750aacd94ab84aa70186685da6fc869a56807ca7ed3097c3f5c229d8644365a",
      deposit_vout: 0,
      msg_sig: "",
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
