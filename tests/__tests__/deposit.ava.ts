import {
  getUserActiveDepositsLen,
  listUserActiveDeposits,
  submitDepositTx,
} from "./helpers/btc_client";
import { initUnit } from "./helpers/context";
import { TestTransactionBuilder } from "./helpers/txn_builder";
import { assertFailure, someH256 } from "./helpers/utils";

const test = initUnit();

test("submit valid deposit txn", async (t) => {
  const { contract, alice } = t.context.accounts;

  const builder = new TestTransactionBuilder(contract, alice, {
    userPubkey: t.context.aliceKeyPair.publicKey,
    allstakePubkey: t.context.allstakePubkey,
  });
  await builder.submit();

  t.is(await getUserActiveDepositsLen(contract, builder.userPubkeyHex), 1);
  const activeDeposits = await listUserActiveDeposits(
    contract,
    builder.userPubkeyHex,
    0,
    1,
  );

  t.is(activeDeposits[0].redeem_version, "V1");
  t.is(activeDeposits[0].deposit_tx_id, builder.tx.getId());
  t.is(activeDeposits[0].deposit_vout, 0);
  t.is(activeDeposits[0].value, builder.depositAmount);
  t.is(activeDeposits[0].queue_withdraw_ts, 0);
  t.is(activeDeposits[0].queue_withdraw_message, null);
  t.is(activeDeposits[0].complete_withdraw_ts, 0);
  t.is(activeDeposits[0].withdraw_tx_id, null);
});

test("submit invalid sequence height", async (t) => {
  const { contract, alice } = t.context.accounts;

  const builder = new TestTransactionBuilder(contract, alice, {
    userPubkey: t.context.aliceKeyPair.publicKey,
    allstakePubkey: t.context.allstakePubkey,
    seq: 2,
  });

  await assertFailure(t, builder.submit(), "Invalid seq height");
});

test("submit invalid deposit txn", async (t) => {
  const { contract, alice } = t.context.accounts;

  const builder = new TestTransactionBuilder(contract, alice, {
    userPubkey: t.context.bobKeyPair.publicKey, // wrong
    allstakePubkey: t.context.allstakePubkey,
  });

  const userPubkey = t.context.aliceKeyPair.publicKey.toString("hex");
  await assertFailure(
    t,
    submitDepositTx(contract, alice, {
      tx_hex: builder.tx.toHex(),
      deposit_vout: 0,
      embed_vout: 1,
      user_pubkey_hex: userPubkey,
      sequence_height: builder.sequence,
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

  const builder = new TestTransactionBuilder(contract, alice, {
    userPubkey: t.context.aliceKeyPair.publicKey,
    allstakePubkey: t.context.allstakePubkey,
  });

  await submitDepositTx(contract, alice, {
    tx_hex: builder.tx.toHex(),
    deposit_vout: 0,
    embed_vout: 1,
    user_pubkey_hex: builder.userPubkeyHex,
    sequence_height: builder.sequence,
    tx_block_hash: someH256,
    tx_index: 0, // this makes it unconfirmed
    merkle_proof: [someH256],
  });
  t.is(await getUserActiveDepositsLen(contract, builder.userPubkeyHex), 0);
});

test("submit duplicated deposit txn", async (t) => {
  const { contract, alice } = t.context.accounts;

  const builder = new TestTransactionBuilder(contract, alice, {
    userPubkey: t.context.aliceKeyPair.publicKey,
    allstakePubkey: t.context.allstakePubkey,
  });
  await builder.submit();

  await assertFailure(t, builder.submit(), "Deposit already saved");
  t.is(await getUserActiveDepositsLen(contract, builder.userPubkeyHex), 1);
});
