import * as bitcoin from "bitcoinjs-lib";
import { depositScriptV1, idToHash, toOutputScript } from "./btc";
import { NearAccount } from "near-workspaces";
import { queueWithdraw, submitDepositTx } from "./btc_client";
import { someH256 } from "./utils";
const bip68 = require("bip68"); // eslint-disable-line

export class DepositTransactionBuilder {
  public tx: bitcoin.Transaction;
  public userPubkey: Buffer;
  public sequence: any;

  constructor(args: {
    userPubkey: Buffer;
    allstakePubkey: Buffer;
    amount: number;
    seq?: number;
  }) {
    this.sequence = bip68.encode({ blocks: args.seq ?? 5 });
    this.userPubkey = args.userPubkey;
    this.tx = new bitcoin.Transaction();
    const p2wsh = bitcoin.payments.p2wsh({
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
    );
    this.tx.addOutput(
      toOutputScript(p2wsh.address!, bitcoin.networks.bitcoin),
      args.amount,
    );
    const embed = bitcoin.payments.embed({
      data: [Buffer.from("allstake.deposit.v1")],
    });
    this.tx.addOutput(embed.output!, 0);
  }

  get userPubkeyHex() {
    return this.userPubkey.toString("hex");
  }

  async submit(btcClient: NearAccount, caller?: NearAccount) {
    return submitDepositTx(btcClient, caller ?? btcClient, {
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

  async queueWithdraw(
    btcClient: NearAccount,
    sig: string,
    caller?: NearAccount,
  ) {
    return queueWithdraw(
      btcClient,
      caller ?? btcClient,
      this.userPubkeyHex,
      this.tx.getId(),
      0,
      sig,
      "Unisat",
    );
  }
}
