import {
  fastForward,
  getUserActiveDepositsLen,
  getUserWithdrawnDepositsLen,
  listAccounts,
  listUserWithdrawnDeposits,
  submitWithdrawalTx,
  viewAccount,
} from "./helpers/bithive";
import { initUnit } from "./helpers/context";
import { TestTransactionBuilder } from "./helpers/txn_builder";
import { assertFailure, daysToMs, someH256 } from "./helpers/utils";

const test = initUnit();

test("submit withdrawal invalid txn hex", async (t) => {
  const { contract, alice } = t.context.accounts;
  const bithivePubkey = t.context.bithivePubkey;
  const builder = new TestTransactionBuilder(contract, alice, {
    userKeyPair: t.context.aliceKeyPair,
    bithivePubkey,
  });
  await builder.submit();
  builder.generateWithdrawPsbt();
  const withdrawTx = builder.extractWithdrawTx();

  await assertFailure(
    t,
    submitWithdrawalTx(contract, alice, {
      tx_hex: withdrawTx.toHex() + "fff", // wrong
      user_pubkey: builder.userPubkeyHex,
      tx_block_hash: someH256,
      tx_index: 1,
      merkle_proof: [someH256],
    }),
    "Invalid txn hex",
  );
});

test("submit withdrawal invalid deposit", async (t) => {
  const { contract, alice } = t.context.accounts;
  const bithivePubkey = t.context.bithivePubkey;
  const builder = new TestTransactionBuilder(contract, alice, {
    userKeyPair: t.context.aliceKeyPair,
    bithivePubkey,
  });
  builder.generateWithdrawPsbt();

  await assertFailure(
    t,
    builder.submitWithdraw(),
    "Not a withdrawal transaction",
  );
});

test("submit withdrawal txn not confirmed", async (t) => {
  const { contract, alice } = t.context.accounts;
  const bithivePubkey = t.context.bithivePubkey;
  const builder = new TestTransactionBuilder(contract, alice, {
    userKeyPair: t.context.aliceKeyPair,
    bithivePubkey,
  });
  await builder.submit();

  builder.generateWithdrawPsbt();
  const withdrawTx = builder.extractWithdrawTx();

  await submitWithdrawalTx(contract, alice, {
    tx_hex: withdrawTx.toHex(),
    user_pubkey: builder.userPubkeyHex,
    tx_block_hash: someH256,
    tx_index: 0, // wrong
    merkle_proof: [someH256],
  });

  t.is(await getUserWithdrawnDepositsLen(contract, builder.userPubkeyHex), 0);
  t.is(await getUserActiveDepositsLen(contract, builder.userPubkeyHex), 1);
});

test("submit solo withdrawal", async (t) => {
  const { contract, alice } = t.context.accounts;

  // create two deposits
  const builder1 = new TestTransactionBuilder(contract, alice, {
    userKeyPair: t.context.aliceKeyPair,
    bithivePubkey: t.context.bithivePubkey,
    depositAmount: 1e8,
  });
  await builder1.submit();

  const builder2 = new TestTransactionBuilder(contract, alice, {
    userKeyPair: t.context.aliceKeyPair,
    bithivePubkey: t.context.bithivePubkey,
    depositAmount: 2e8,
  });
  await builder2.submit();

  // queue withdrawal 3e8
  const sig = builder1.queueWithdrawSignature(3e8, 0);
  await builder1.queueWithdraw(3e8, sig);
  const accountBefore = await viewAccount(contract, builder1.userPubkeyHex);

  builder1.generateWithdrawPsbt();
  await builder1.submitWithdraw();

  t.is(await getUserWithdrawnDepositsLen(contract, builder1.userPubkeyHex), 1);
  t.is(await getUserActiveDepositsLen(contract, builder1.userPubkeyHex), 1);

  const deposits = await listUserWithdrawnDeposits(
    contract,
    builder1.userPubkeyHex,
    0,
    1,
  );
  t.is(deposits[0].status, "Withdrawn");
  t.is(deposits[0].deposit_tx_id, builder1.tx.getId());
  t.is(deposits[0].deposit_vout, 0);
  t.is(deposits[0].value, builder1.depositAmount);
  t.assert(deposits[0].complete_withdrawal_ts > 0);
  t.is(deposits[0].withdrawal_tx_id, builder1.withdrawTx!.getId());

  // account queue amount should be updated
  const accountAfter = await viewAccount(contract, builder1.userPubkeyHex);
  t.is(accountAfter.queue_withdrawal_amount, 2e8);
  t.is(
    accountAfter.queue_withdrawal_start_ts,
    accountBefore.queue_withdrawal_start_ts,
  );
});

test("submit multisig withdrawal", async (t) => {
  const { contract, alice } = t.context.accounts;
  const bithivePubkey = t.context.bithivePubkey;

  // make two deposits
  const builder1 = new TestTransactionBuilder(contract, alice, {
    userKeyPair: t.context.aliceKeyPair,
    bithivePubkey,
    depositAmount: 1e8,
  });
  await builder1.submit();
  const builder2 = new TestTransactionBuilder(contract, alice, {
    userKeyPair: t.context.aliceKeyPair,
    bithivePubkey,
    depositAmount: 2e8,
  });
  await builder2.submit();

  // queue withdrawal 1e7 first
  const sig1 = builder1.queueWithdrawSignature(1e7, 0);
  await builder1.queueWithdraw(1e7, sig1);

  await fastForward(contract, daysToMs(2) + 1);
  builder1.generateWithdrawPsbt(undefined, 9e7);
  // queue withdrawal amount 1e7 is cleared here
  await builder1.signWithdraw(0);

  // queue withdrawal another 2e7 before submitting the first withdrawal txn
  const sig2 = builder1.queueWithdrawSignature(2e7, 1);
  await builder2.queueWithdraw(2e7, sig2);
  const accountBefore = await viewAccount(contract, builder1.userPubkeyHex);

  // submit the first withdrawal txn
  // in order to make the contract to treat it as a multisig withdrawal, we manully set the witness
  const withdrawTx = builder1.extractWithdrawTx();
  withdrawTx.setWitness(
    0,
    Array.from({ length: 5 }, () => Buffer.alloc(0)),
  );
  await submitWithdrawalTx(contract, alice, {
    tx_hex: withdrawTx.toHex(),
    user_pubkey: builder1.userPubkeyHex,
    tx_block_hash: someH256,
    tx_index: 1,
    merkle_proof: [someH256],
  });

  t.is(await getUserWithdrawnDepositsLen(contract, builder1.userPubkeyHex), 1);
  t.is(await getUserActiveDepositsLen(contract, builder1.userPubkeyHex), 1);

  const deposits = await listUserWithdrawnDeposits(
    contract,
    builder1.userPubkeyHex,
    0,
    1,
  );
  t.is(deposits[0].status, "Withdrawn");
  t.is(deposits[0].deposit_tx_id, builder1.tx.getId());
  t.is(deposits[0].deposit_vout, 0);
  t.is(deposits[0].value, builder1.depositAmount);
  t.assert(deposits[0].complete_withdrawal_ts > 0);
  t.is(deposits[0].withdrawal_tx_id, withdrawTx.getId());

  // queue amount should not be affected
  const accountAfter = await viewAccount(contract, builder1.userPubkeyHex);
  t.is(
    accountAfter.queue_withdrawal_amount,
    accountBefore.queue_withdrawal_amount,
  );
  t.is(
    accountAfter.queue_withdrawal_start_ts,
    accountBefore.queue_withdrawal_start_ts,
  );
});

test("submit withdrawal with an already withdrawn deposit input", async (t) => {
  const { contract, alice } = t.context.accounts;
  const bithivePubkey = t.context.bithivePubkey;

  // make two deposits
  const builder1 = new TestTransactionBuilder(contract, alice, {
    userKeyPair: t.context.aliceKeyPair,
    bithivePubkey,
    depositAmount: 1e8,
  });
  await builder1.submit();
  const builder2 = new TestTransactionBuilder(contract, alice, {
    userKeyPair: t.context.aliceKeyPair,
    bithivePubkey,
    depositAmount: 2e8,
  });
  // deposit 2 is not submitted yet

  // make a withdrawal transaction to withdraw both
  builder1.generateWithdrawPsbt({
    hash: builder2.tx.getId(),
    index: 0,
  });
  await builder1.submitWithdraw();

  // submit the second deposit
  await builder2.submit();

  // try to submit the withdrawal again
  // only the second deposit should be withdrawn, the first one is already withdrawn
  await builder1.submitWithdraw();

  t.is(await getUserWithdrawnDepositsLen(contract, builder1.userPubkeyHex), 2);
  t.is(await getUserActiveDepositsLen(contract, builder1.userPubkeyHex), 0);
});

test("list accounts should contain withdrawn account", async (t) => {
  const { contract, alice } = t.context.accounts;

  // create two deposits
  const builder = new TestTransactionBuilder(contract, alice, {
    userKeyPair: t.context.aliceKeyPair,
    bithivePubkey: t.context.bithivePubkey,
    depositAmount: 1e8,
  });
  await builder.submit();

  const sig = builder.queueWithdrawSignature(1e8, 0);
  await builder.queueWithdraw(1e8, sig);

  builder.generateWithdrawPsbt();
  await builder.submitWithdraw();

  const accounts = await listAccounts(contract, 0, 10);
  t.is(accounts.length, 1);
  t.is(accounts[0].pubkey, builder.userPubkeyHex);
});
