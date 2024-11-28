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

export const queueWithdrawal: CommandModule<unknown, Args> = {
  command: "queue",
  describe: "Submit a BTC queue withdrawal request",
  builder: {
    env: envBuilder,
    pubkey: {
      describe: "User public key",
      type: "string",
      demandOption: true,
    },
    amount: {
      describe: "Amount to withdraw in satoshis",
      type: "number",
      demandOption: true,
    },
    sig: {
      describe: "Signature of the withdrawal message",
      type: "string",
      demandOption: false,
    },
  },
  async handler({ env, pubkey, amount, sig }) {
    const config = await getConfig(env);
    const { signer } = await initNear(env);

    if (!sig) {
      // print the withdrawal message that needs to be signed
      const msg = await signer.viewFunction({
        contractId: config.accountIds.bithive,
        methodName: "get_v1_withdrawal_constants",
        args: {
          user_pubkey: pubkey,
          amount,
        },
      });
      console.log("Withdrawal message to sign", msg.queue_withdrawal_msg);
      console.log(msg.queue_withdrawal_msg);
      return;
    }

    const args = {
      user_pubkey: pubkey,
      withdraw_amount: amount,
      msg_sig: sig,
      sig_type: "ECDSA",
    };

    await signer.functionCall({
      contractId: config.accountIds.bithive,
      methodName: "queue_withdrawal",
      args: args,
      gas: nearTGas(100),
    });

    console.log("Queued withdrawal");
  },
};
