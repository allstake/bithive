import { CommandModule } from "yargs";
import * as bitcoin from "bitcoinjs-lib";
import { getTransaction } from "../blocksteam";
import { getConfig } from "../config";
import { envBuilder } from "../helper";
import {
  buildBitHiveSignature,
  depositScriptV1,
  getWitnessUtxo,
  multisigWithdrawScript,
} from "../btc";
import { getSummary, getV1Consts, signWithdrawal } from "../near";

interface Args {
  env: string;
  txid: string;
  vout: number;
  pubkey: string;
  receiver: string;
  amount: number;
  bithiveSig: string;
  userSignedPsbt: string;
}

export const signWithdraw: CommandModule<unknown, Args> = {
  command: "sign",
  describe: "Build a BTC withdraw request to sign",
  builder: {
    env: envBuilder,
    txid: {
      describe: "Transaction ID",
      type: "string",
      demandOption: true,
    },
    vout: {
      describe: "Deposit output index",
      type: "number",
      demandOption: true,
    },
    pubkey: {
      describe: "Deposit user public key",
      type: "string",
      demandOption: true,
    },
    receiver: {
      describe: "Receiver address",
      type: "string",
      demandOption: true,
    },
    amount: {
      describe: "Amount to withdraw in satoshis",
      type: "number",
      demandOption: true,
    },
    userSignedPsbt: {
      describe: "User signed PSBT in hex",
      type: "string",
    },
    bithiveSig: {
      describe: "BitHive signature",
      type: "string",
    },
  },
  async handler({
    env,
    txid,
    vout,
    pubkey,
    receiver,
    amount,
    userSignedPsbt,
    bithiveSig,
  }) {
    const config = await getConfig(env);
    const txHex = await getTransaction(txid, config.bitcoin.network);
    const depositTxn = bitcoin.Transaction.fromHex(txHex);

    // read near contract configs
    const summary = await getSummary(env);
    const v1Consts = await getV1Consts(env);

    // construct withdraw psbt
    const network =
      config.bitcoin.network === "testnet"
        ? bitcoin.networks.testnet
        : bitcoin.networks.bitcoin;
    const depositScript = bitcoin.payments.p2wsh({
      redeem: {
        output: depositScriptV1(
          Buffer.from(pubkey, "hex"),
          Buffer.from(v1Consts.bithive_pubkey, "hex"),
          summary.solo_withdraw_sequence_heights[0],
        ),
      },
      network,
    });
    const psbt = new bitcoin.Psbt({ network });
    psbt.addInput({
      hash: depositTxn.getId(),
      index: vout,
      witnessUtxo: getWitnessUtxo(depositTxn.outs[vout]),
      witnessScript: depositScript.redeem!.output!,
    });

    // output
    psbt.addOutput({
      address: receiver,
      value: amount,
    });

    // -- path 1: both signatures provided, build the transaction

    if (userSignedPsbt && bithiveSig) {
      // extract user signature from signed PSBT
      const partialSignedPsbt = bitcoin.Psbt.fromHex(userSignedPsbt);
      const userSig = partialSignedPsbt.data.inputs[0].partialSig![0].signature;

      const withdrawTxn: bitcoin.Transaction = (psbt as any).__CACHE.__TX;
      const witness = bitcoin.payments.p2wsh({
        network,
        redeem: {
          network,
          output: depositScript.redeem!.output!,
          input: multisigWithdrawScript(
            userSig,
            Buffer.from(bithiveSig, "hex"),
          ),
        },
      });
      withdrawTxn.setWitness(0, witness.witness!);

      console.log("\n>>> Withdraw transaction to broadcast:");
      console.log(withdrawTxn.toHex());
      console.log(
        `\nYou can broadcast it via ${config.bitcoin.network === "testnet" ? "https://mempool.space/testnet/tx/push" : "https://mempool.space/tx/push"}`,
      );

      return;
    }

    // -- path 2: generate data required for signing

    // a) the signature from chain signatures
    const bithiveRes = await signWithdrawal(env, psbt.toHex(), pubkey, vout);
    const sig = buildBitHiveSignature(
      bithiveRes.big_r.affine_point,
      bithiveRes.s.scalar,
    );
    console.log("\n>>> BitHive signature:");
    console.log(sig.toString("hex"));

    // b) psbt for signing via wallet
    console.log("\n>>> PSBT to sign via wallet:");
    console.log(psbt.toHex());
    console.log();

    console.log("\nPlease provide the PSBT hex to Unisat wallet for signing.");
    console.log(
      "\nAfter signing, provide the signed PSBT hex along with the bithive signature above to this command (`--userSignedPsbt` and `--bithiveSig`) to build the transaction.",
    );
  },
};
