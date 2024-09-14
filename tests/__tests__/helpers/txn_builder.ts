import * as bitcoin from "bitcoinjs-lib";
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
import { someH256 } from "./utils";
const bip68 = require("bip68"); // eslint-disable-line

const SEQUENCE_TIMELOCK = 0xfffffffd; // sequence that enables time-lock and RBF

export class TestTransactionBuilder {
  public tx: bitcoin.Transaction;
  public withdrawTx: bitcoin.Transaction | undefined;
  public userPubkey: Buffer;
  public sequence: any;
  public readonly depositAmount: number;

  private p2wsh: bitcoin.Payment;
  private psbt: bitcoin.Psbt | undefined;

  private btcClient: NearAccount;
  private caller: NearAccount;

  constructor(
    btcClient: NearAccount,
    caller: NearAccount,
    args: {
      userPubkey: Buffer;
      allstakePubkey: Buffer;
      seq?: number;
      depositAmount?: number;
      enableTimelock?: boolean;
    },
  ) {
    this.btcClient = btcClient;
    this.caller = caller;

    this.depositAmount = args.depositAmount ?? 1e8;
    this.sequence = bip68.encode({ blocks: args.seq ?? 5 });
    this.userPubkey = args.userPubkey;
    this.tx = new bitcoin.Transaction();
    this.p2wsh = bitcoin.payments.p2wsh({
      redeem: {
        output: depositScriptV1(
          args.userPubkey,
          args.allstakePubkey,
          this.sequence,
        ),
      },
    });
    this.tx.addInput(
      idToHash(
        "e813831dccfd1537517c0e62431c9a2a1ca2580b9401cb2274e3f2e06c43ae43",
      ),
      0,
      args.enableTimelock ? SEQUENCE_TIMELOCK : 0xffffffff,
    );
    if (args.enableTimelock) {
      this.tx.locktime = 100;
    }

    this.tx.addOutput(
      toOutputScript(this.p2wsh.address!, bitcoin.networks.bitcoin),
      this.depositAmount,
    );
    const embed = bitcoin.payments.embed({
      data: [Buffer.from("allstake.deposit.v1")],
    });
    this.tx.addOutput(embed.output!, 0);
  }

  get userPubkeyHex() {
    return this.userPubkey.toString("hex");
  }

  async submit() {
    return submitDepositTx(this.btcClient, this.caller, {
      tx_hex: this.tx.toHex(),
      deposit_vout: 0,
      embed_vout: 1,
      user_pubkey_hex: this.userPubkey.toString("hex"),
      sequence_height: this.sequence,
      tx_block_hash: someH256,
      tx_index: 1,
      merkle_proof: [someH256],
    });
  }

  async queueWithdraw(sig: string) {
    return queueWithdrawal(
      this.btcClient,
      this.caller,
      this.userPubkeyHex,
      this.tx.getId(),
      0,
      sig,
      "Unisat",
    );
  }

  generatePsbt(
    extraInput = false,
    embedMsg = "allstake.withdraw",
  ): bitcoin.Psbt {
    const withdrawMsg = Buffer.from(embedMsg);
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
        hash: this.tx.getId(),
        index: 1,
        witnessUtxo: getWitnessUtxo(this.tx.outs[0]),
        witnessScript: this.p2wsh.redeem!.output!,
      });
    }

    this.psbt = this.psbt
      .addOutput({
        address: userP2WPKHAddress.address!,
        value: this.depositAmount - 10,
      })
      .addOutput({
        script: bitcoin.script.compile([
          bitcoin.opcodes.OP_RETURN,
          withdrawMsg,
        ]),
        value: 0,
      });

    return this.psbt;
  }

  signWithdraw() {
    if (!this.psbt) {
      this.psbt = this.generatePsbt();
    }
    return signWithdrawal(
      this.btcClient,
      this.caller,
      this.psbt.toHex(),
      this.userPubkey.toString("hex"),
      0,
    );
  }

  generateWithdrawTx(extraInput = false, embedMsg = "allstake.withdraw") {
    const userP2WPKHAddress = bitcoin.payments.p2wpkh({
      pubkey: this.userPubkey,
      network: bitcoin.networks.bitcoin,
    });
    const withdrawMsg = Buffer.from(embedMsg);

    const withdrawTransaction = new bitcoin.Transaction();
    withdrawTransaction.version = 2;
    withdrawTransaction.addInput(idToHash(this.tx.getId()), 0);

    if (extraInput) {
      withdrawTransaction.addInput(idToHash(this.tx.getId()), 1);
    }

    withdrawTransaction.addOutput(
      toOutputScript(userP2WPKHAddress.address!, bitcoin.networks.bitcoin),
      this.depositAmount - 100,
    );
    const embedOutput = bitcoin.payments.embed({ data: [withdrawMsg] });
    withdrawTransaction.addOutput(embedOutput.output!, 0);

    this.withdrawTx = withdrawTransaction;
    return withdrawTransaction;
  }

  submitWithdraw() {
    if (!this.withdrawTx) {
      this.withdrawTx = this.generateWithdrawTx();
    }
    return submitWithdrawalTx(
      this.btcClient,
      this.caller,
      this.withdrawTx.toHex(),
      this.userPubkeyHex,
      0,
      someH256,
      1,
      [someH256],
    );
  }
}
