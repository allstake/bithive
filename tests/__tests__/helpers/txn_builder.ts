import * as bitcoin from "bitcoinjs-lib";
import { message } from "@okxweb3/coin-bitcoin";
import { NearAccount } from "near-workspaces";
import {
  depositScriptV1,
  getWitnessUtxo,
  idToHash,
  toOutputScript,
} from "./btc";
import {
  queueWithdrawal,
  signWithdrawal,
  submitDepositTx,
  submitWithdrawalTx,
} from "./btc_client";
import { buildDepositEmbedMsg, someH256 } from "./utils";
import { ECPairInterface } from "ecpair";
const bip68 = require("bip68"); // eslint-disable-line

const SEQUENCE_TIMELOCK = 0xfffffffd; // sequence that enables time-lock and RBF

export class TestTransactionBuilder {
  public tx: bitcoin.Transaction;
  public withdrawTx: bitcoin.Transaction | undefined;
  public userKeyPair: ECPairInterface;
  public userPubkey: Buffer;
  public sequence: any;
  public readonly depositAmount: number;
  public reinvest = false;

  private p2wsh: bitcoin.Payment;
  public psbt: bitcoin.Psbt | undefined;

  private btcClient: NearAccount;
  private caller: NearAccount;

  constructor(
    btcClient: NearAccount,
    caller: NearAccount,
    args: {
      userKeyPair: ECPairInterface;
      allstakePubkey: Buffer;
      inputTxIndex?: number;
      seq?: number;
      depositAmount?: number;
      enableTimelock?: boolean;
    },
  ) {
    this.btcClient = btcClient;
    this.caller = caller;

    this.depositAmount = args.depositAmount ?? 1e8;
    this.sequence = bip68.encode({ blocks: args.seq ?? 5 });
    this.userKeyPair = args.userKeyPair;
    this.userPubkey = args.userKeyPair.publicKey;

    this.tx = new bitcoin.Transaction();
    this.p2wsh = bitcoin.payments.p2wsh({
      redeem: {
        output: depositScriptV1(
          this.userPubkey,
          args.allstakePubkey,
          this.sequence,
        ),
      },
    });

    const inputTxId =
      "e813831dccfd1537517c0e62431c9a2a1ca2580b9401cb2274e3f2e06c43ae43";
    this.tx.addInput(
      idToHash(inputTxId),
      args.inputTxIndex ?? 0, // this allows us to generate deposit txn with different ID
      args.enableTimelock ? SEQUENCE_TIMELOCK : 0xffffffff,
    );
    if (args.enableTimelock) {
      this.tx.locktime = 100;
    }

    this.tx.addOutput(
      toOutputScript(this.p2wsh.address!, bitcoin.networks.bitcoin),
      this.depositAmount,
    );

    // add embed output
    const embedMsg = buildDepositEmbedMsg(0, this.userPubkeyHex, args.seq ?? 5);
    const embed = bitcoin.payments.embed({
      data: [embedMsg],
    });
    this.tx.addOutput(embed.output!, 0);
  }

  get userPubkeyHex() {
    return this.userPubkey.toString("hex");
  }

  async submit() {
    return submitDepositTx(this.btcClient, this.caller, {
      tx_hex: this.tx.toHex(),
      embed_vout: 1,
      tx_block_hash: someH256,
      tx_index: 1,
      merkle_proof: [someH256],
    });
  }

  queueWithdrawSignature(amount: number, nonce: number) {
    const withdrawMsgPlain = `bithive.withdraw:${nonce}:${amount}sats`;
    const sigBase64 = message.sign(this.userKeyPair.toWIF(), withdrawMsgPlain);
    const sigHex = Buffer.from(sigBase64, "base64").toString("hex");
    return sigHex;
  }

  async queueWithdraw(amount: number, sigHex: string) {
    return queueWithdrawal(
      this.btcClient,
      this.caller,
      this.userPubkeyHex,
      amount,
      sigHex,
      "ECDSA",
    );
  }

  generateWithdrawPsbt(
    extraInput?: {
      hash: string;
      index: number;
    },
    reinvestAmount = 0,
    withdrawAmount = 100,
  ): bitcoin.Psbt {
    const userP2WPKHAddress = bitcoin.payments.p2wpkh({
      pubkey: this.userPubkey,
      network: bitcoin.networks.bitcoin,
    });
    this.psbt = new bitcoin.Psbt({ network: bitcoin.networks.bitcoin });
    this.psbt = this.psbt.addInput({
      hash: this.tx.getId(),
      index: 0,
      witnessUtxo: getWitnessUtxo(this.tx.outs[0]),
      witnessScript: this.p2wsh.redeem!.output!,
    });

    if (extraInput) {
      this.psbt = this.psbt.addInput({
        hash: extraInput.hash,
        index: extraInput.index,
        witnessUtxo: getWitnessUtxo(this.tx.outs[0]),
        witnessScript: this.p2wsh.redeem!.output!,
      });
    }

    this.psbt = this.psbt.addOutput({
      address: userP2WPKHAddress.address!,
      value: withdrawAmount,
    });
    if (reinvestAmount > 0) {
      // this reinvest vout will be 1
      const embedReinvestMsg = buildDepositEmbedMsg(1, this.userPubkeyHex, 5);
      const reinvestEmbed = bitcoin.payments.embed({
        data: [embedReinvestMsg],
      });

      this.psbt = this.psbt
        .addOutput({
          address: this.p2wsh.address!,
          value: reinvestAmount,
        })
        .addOutput({
          script: reinvestEmbed.output!,
          value: 0,
        });

      this.reinvest = true;
    }

    return this.psbt;
  }

  extractWithdrawTx(): bitcoin.Transaction {
    if (!this.psbt) {
      throw new Error("Generate PSBT first");
    }
    return (this.psbt as any).__CACHE.__TX;
  }

  signWithdraw(vinToSign: number) {
    if (!this.psbt) {
      throw new Error("Generate PSBT first");
    }
    return signWithdrawal(
      this.btcClient,
      this.caller,
      this.psbt.toHex(),
      this.userPubkey.toString("hex"),
      vinToSign,
      this.reinvest ? 2 : undefined, // if reinvest, the embed ouput will be index 2
    );
  }

  submitWithdraw() {
    this.withdrawTx = this.extractWithdrawTx();
    return submitWithdrawalTx(this.btcClient, this.caller, {
      tx_hex: this.withdrawTx.toHex(),
      user_pubkey: this.userPubkeyHex,
      tx_block_hash: someH256,
      tx_index: 1,
      merkle_proof: [someH256],
    });
  }
}
