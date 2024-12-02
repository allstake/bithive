import { CommandModule } from "yargs";
import ECPairFactory from "ecpair";
import * as ecc from "tiny-secp256k1";
import * as bitcoin from "bitcoinjs-lib";
import { envBuilder } from "../helper";
import { getConfig } from "../config";
import {
  depositScriptV1,
  getWitnessUtxo,
  idToHash,
  soloWithdrawScript,
  toOutputScript,
} from "../btc";
import { getV1DepositConstants } from "../near";
import { getSummary } from "../near";
import { broadcastTransaction, getTransaction } from "../mempool";

const bip68 = require("bip68"); // eslint-disable-line
bitcoin.initEccLib(ecc);
const ECPair = ECPairFactory(ecc);

interface Args {
  env: string;
  txid: string;
  vout: number;
  receiver: string;
  gas: number;
  bithivePubkey?: string;
}

export const soloWithdraw: CommandModule<unknown, Args> = {
  command: "solo",
  describe: "Generate a BTC solo withdrawal request",
  builder: {
    env: envBuilder,
    txid: {
      describe: "Transaction ID",
      type: "string",
      demandOption: true,
    },
    vout: {
      describe: "Output index",
      type: "number",
      demandOption: true,
    },
    receiver: {
      describe: "Receiver address",
      type: "string",
      demandOption: true,
    },
    gas: {
      describe: "Gas in satoshis",
      type: "number",
      demandOption: false,
      default: 300,
    },
    bithivePubkey: {
      describe: "Bithive public key to override the one from contract",
      type: "string",
      demandOption: false,
    },
  },
  async handler({ env, txid, vout, receiver, gas, bithivePubkey }) {
    const config = await getConfig(env);

    const privateKey = process.env.BTC_PRIVATE_KEY;
    if (!privateKey) {
      throw new Error("BTC private key is not set via BTC_PRIVATE_KEY");
    }
    const btcSigner = ECPair.fromPrivateKey(Buffer.from(privateKey, "hex"));

    // read near contract configs
    const summary = await getSummary(env);
    const v1Consts = await getV1DepositConstants(
      env,
      btcSigner.publicKey.toString("hex"),
    );

    const txHex = await getTransaction(txid, config.bitcoin.detailedName);
    const depositTxn = bitcoin.Transaction.fromHex(txHex);

    const network =
      config.bitcoin.network === "testnet"
        ? bitcoin.networks.testnet
        : bitcoin.networks.bitcoin;
    const depositScript = bitcoin.payments.p2wsh({
      redeem: {
        output: depositScriptV1(
          btcSigner.publicKey,
          bithivePubkey
            ? Buffer.from(bithivePubkey, "hex")
            : Buffer.from(v1Consts.bithive_pubkey, "hex"),
          summary.solo_withdrawal_sequence_heights[0],
        ),
      },
      network,
    });

    const psbt = new bitcoin.Psbt({ network });

    psbt.addInput({
      hash: txid,
      index: vout,
      witnessUtxo: getWitnessUtxo(depositTxn.outs[vout]),
      witnessScript: depositScript.redeem!.output!,
      sequence: bip68.encode({
        blocks: summary.solo_withdrawal_sequence_heights[0],
      }),
    });

    psbt.addOutput({
      address: receiver,
      value: depositTxn.outs[vout].value - gas,
    });

    psbt.signInput(0, btcSigner);
    console.log("Withdraw psbt signed");

    const userPartialSig = psbt.data.inputs[0].partialSig![0].signature;
    const withdrawTx = new bitcoin.Transaction();
    withdrawTx.version = 2;
    withdrawTx.addInput(
      idToHash(txid),
      vout,
      bip68.encode({ blocks: summary.solo_withdrawal_sequence_heights[0] }),
    );
    withdrawTx.addOutput(
      toOutputScript(receiver, network),
      depositTxn.outs[vout].value - gas,
    );
    const witness = bitcoin.payments.p2wsh({
      network,
      redeem: {
        network,
        output: depositScript.redeem!.output!,
        input: soloWithdrawScript(userPartialSig),
      },
    }).witness!;
    withdrawTx.setWitness(0, witness);

    // broadcast
    const res = await broadcastTransaction(
      withdrawTx.toHex(),
      config.bitcoin.detailedName,
    );
    console.log("Withdraw tx broadcasted", withdrawTx.getId());
    console.log(res);
  },
};
