import * as bitcoin from "bitcoinjs-lib";
import { initUnit } from "./helpers/context";
import { depositScriptV1, idToHash, toOutputScript } from "./helpers/btc";
import {
  getUserActiveDepositsLen,
  listUserActiveDeposits,
  submitDepositTx,
} from "./helpers/btc_client";
import { assertFailure, someH256 } from "./helpers/utils";

const bip68 = require("bip68"); // eslint-disable-line

const test = initUnit();

test("submit valid deposit txn", async (t) => {
  const { contract, alice } = t.context.accounts;
  const amount = 10e8;
  const seq = 5;

  const depositTx = depositTransaction(
    t.context.aliceKeyPair.publicKey,
    t.context.allstakePubkey,
    amount,
  );

  const userPubkey = t.context.aliceKeyPair.publicKey.toString("hex");
  await submitDepositTx(contract, alice, {
    tx_hex: depositTx.toHex(),
    deposit_vout: 0,
    embed_vout: 1,
    user_pubkey_hex: userPubkey,
    sequence_height: seq,
    tx_block_hash: someH256,
    tx_index: 1,
    merkle_proof: [someH256],
  });

  t.is(await getUserActiveDepositsLen(contract, userPubkey), 1);
  const activeDeposits = await listUserActiveDeposits(
    contract,
    userPubkey,
    0,
    1,
  );
  t.is(activeDeposits[0].deposit_tx_id, depositTx.getId());
  t.is(activeDeposits[0].deposit_vout, 0);
  t.is(activeDeposits[0].value, amount);
  t.is(activeDeposits[0].queue_withdraw_ts, 0);
  t.is(activeDeposits[0].queue_withdraw_message, null);
  t.is(activeDeposits[0].complete_withdraw_ts, 0);
  t.is(activeDeposits[0].withdraw_tx_id, null);
});

test("submit invalid deposit txn", async (t) => {
  const { contract, alice } = t.context.accounts;
  const amount = 10e8;
  const seq = 5;

  const depositTx = depositTransaction(
    t.context.aliceKeyPair.publicKey,
    t.context.allstakePubkey,
    amount,
  );

  const userPubkey = t.context.bobKeyPair.publicKey.toString("hex");
  await assertFailure(
    t,
    submitDepositTx(contract, alice, {
      tx_hex: depositTx.toHex(),
      deposit_vout: 0,
      embed_vout: 1,
      user_pubkey_hex: userPubkey,
      sequence_height: seq,
      tx_block_hash: someH256,
      tx_index: 1,
      merkle_proof: [someH256],
    }),
    "Deposit output bad script hash",
  );

  t.is(await getUserActiveDepositsLen(contract, userPubkey), 0);
});

test("submit deposit txn not confirmed", async (t) => {
  const { contract, alice } = t.context.accounts;
  const amount = 10e8;
  const seq = 5;

  const depositTx = depositTransaction(
    t.context.aliceKeyPair.publicKey,
    t.context.allstakePubkey,
    amount,
  );

  const userPubkey = t.context.aliceKeyPair.publicKey.toString("hex");
  await submitDepositTx(contract, alice, {
    tx_hex: depositTx.toHex(),
    deposit_vout: 0,
    embed_vout: 1,
    user_pubkey_hex: userPubkey,
    sequence_height: seq,
    tx_block_hash: someH256,
    tx_index: 0, // this makes it unconfirmed
    merkle_proof: [someH256],
  });
  t.is(await getUserActiveDepositsLen(contract, userPubkey), 0);
});

test("submit duplicated deposit txn", async (t) => {
  const { contract, alice } = t.context.accounts;
  const amount = 10e8;
  const seq = 5;

  const depositTx = depositTransaction(
    t.context.aliceKeyPair.publicKey,
    t.context.allstakePubkey,
    amount,
  );

  const userPubkey = t.context.aliceKeyPair.publicKey.toString("hex");
  await submitDepositTx(contract, alice, {
    tx_hex: depositTx.toHex(),
    deposit_vout: 0,
    embed_vout: 1,
    user_pubkey_hex: userPubkey,
    sequence_height: seq,
    tx_block_hash: someH256,
    tx_index: 1,
    merkle_proof: [someH256],
  });
  await assertFailure(
    t,
    submitDepositTx(contract, alice, {
      tx_hex: depositTx.toHex(),
      deposit_vout: 0,
      embed_vout: 1,
      user_pubkey_hex: userPubkey,
      sequence_height: seq,
      tx_block_hash: someH256,
      tx_index: 1,
      merkle_proof: [someH256],
    }),
    "Deposit already saved",
  );
  t.is(await getUserActiveDepositsLen(contract, userPubkey), 1);
});

// --
// helper methods

function depositTransaction(
  userPubkey: Buffer,
  allstakePubkey: Buffer,
  amount: number,
  seq = 5,
) {
  const sequence = bip68.encode({ blocks: seq });

  const tx = new bitcoin.Transaction();
  const p2wsh = bitcoin.payments.p2wsh({
    redeem: {
      output: depositScriptV1(userPubkey, allstakePubkey, sequence),
    },
  });
  tx.addInput(
    idToHash(
      "e813831dccfd1537517c0e62431c9a2a1ca2580b9401cb2274e3f2e06c43ae43",
    ),
    0,
  );
  tx.addOutput(
    toOutputScript(p2wsh.address!, bitcoin.networks.bitcoin),
    amount,
  );
  const embed = bitcoin.payments.embed({
    data: [Buffer.from("allstake.deposit.v1")],
  });
  tx.addOutput(embed.output!, 0);

  return tx;
}
