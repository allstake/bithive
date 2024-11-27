import { CommandModule } from "yargs";
import * as bitcoin from "bitcoinjs-lib";
import { getConfig } from "../config";
import { envBuilder, nearTGas } from "../helper";
import { initNear } from "../near";
import { getTransactionProof, getTransactionStatus } from "../blocksteam";

interface Args {
  env: string;
  tx: string;
  pubkey: string;
  vin: number;
}

export const submitWithdrawal: CommandModule<unknown, Args> = {
  command: "withdraw",
  describe: "Submit a BTC withdrawal transaction",
  builder: {
    env: envBuilder,
    tx: {
      type: "string",
      describe: "Hex encoded withdrawal transaction",
      demandOption: true,
    },
    pubkey: {
      type: "string",
      describe: "User public key",
      demandOption: true,
    },
    vin: {
      type: "number",
      describe: "Vin of the deposit in the withdrawal tx",
      demandOption: true,
    },
  },
  async handler({ env, tx, pubkey, vin }) {
    const config = await getConfig(env);
    const { signer } = await initNear(env);

    const txId = bitcoin.Transaction.fromHex(tx).getId();
    const status = await getTransactionStatus(txId, config.bitcoin.network);
    const proof = await getTransactionProof(txId, config.bitcoin.network);

    const args = {
      tx_hex: tx,
      user_pubkey: pubkey,
      deposit_vin: vin,
      tx_block_hash: status.block_hash,
      tx_index: proof.pos,
      merkle_proof: proof.merkle,
    };

    await signer.functionCall({
      contractId: config.accountIds.btcClient,
      methodName: "submit_withdrawal_tx",
      args: args,
      gas: nearTGas(100),
    });

    console.log("Submitted withdrawal tx");
  },
};
