import {
  fastForward,
  getUserActiveDepositsLen,
  getUserWithdrawnDepositsLen,
  listUserWithdrawnDeposits,
  submitWithdrawalTx,
  viewAccount,
} from "./helpers/btc_client";
import { initUnit } from "./helpers/context";
import { TestTransactionBuilder } from "./helpers/txn_builder";
import { assertFailure, daysToMs, someH256 } from "./helpers/utils";

const test = initUnit();

test("submit withdraw invalid txn hex", async (t) => {
  const { contract, alice } = t.context.accounts;
  const allstakePubkey = t.context.allstakePubkey;
  const builder = new TestTransactionBuilder(contract, alice, {
    userKeyPair: t.context.aliceKeyPair,
    allstakePubkey,
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

test("submit withdraw invalid deposit", async (t) => {
  const { contract, alice } = t.context.accounts;
  const allstakePubkey = t.context.allstakePubkey;
  const builder = new TestTransactionBuilder(contract, alice, {
    userKeyPair: t.context.aliceKeyPair,
    allstakePubkey,
  });
  builder.generateWithdrawPsbt();

  await assertFailure(
    t,
    builder.submitWithdraw(),
    "Not a withdrawal transaction",
  );
});

test("submit withdraw txn not confirmed", async (t) => {
  const { contract, alice } = t.context.accounts;
  const allstakePubkey = t.context.allstakePubkey;
  const builder = new TestTransactionBuilder(contract, alice, {
    userKeyPair: t.context.aliceKeyPair,
    allstakePubkey,
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

test("submit solo withdraw", async (t) => {
  const { contract, alice } = t.context.accounts;
  const builder = new TestTransactionBuilder(contract, alice, {
    userKeyPair: t.context.aliceKeyPair,
    allstakePubkey: t.context.allstakePubkey,
  });
  await builder.submit();
  builder.generateWithdrawPsbt();
  await builder.submitWithdraw();

  t.is(await getUserWithdrawnDepositsLen(contract, builder.userPubkeyHex), 1);
  t.is(await getUserActiveDepositsLen(contract, builder.userPubkeyHex), 0);

  const deposits = await listUserWithdrawnDeposits(
    contract,
    builder.userPubkeyHex,
    0,
    1,
  );
  t.is(deposits[0].status, "Withdrawn");
  t.is(deposits[0].deposit_tx_id, builder.tx.getId());
  t.is(deposits[0].deposit_vout, 0);
  t.is(deposits[0].value, builder.depositAmount);
  t.assert(deposits[0].complete_withdraw_ts > 0);
  t.is(deposits[0].withdrawal_tx_id, builder.withdrawTx!.getId());
});

test("submit multisig withdraw", async (t) => {
  const { contract, alice } = t.context.accounts;
  const allstakePubkey = t.context.allstakePubkey;
  const builder = new TestTransactionBuilder(contract, alice, {
    userKeyPair: t.context.aliceKeyPair,
    allstakePubkey,
  });
  await builder.submit();
  const sig = builder.queueWithdrawSignature(100, 0);
  await builder.queueWithdraw(100, sig);

  await fastForward(contract, daysToMs(2) + 1);
  builder.generateWithdrawPsbt();
  await builder.submitWithdraw();

  t.is(await getUserWithdrawnDepositsLen(contract, builder.userPubkeyHex), 1);
  t.is(await getUserActiveDepositsLen(contract, builder.userPubkeyHex), 0);

  const deposits = await listUserWithdrawnDeposits(
    contract,
    builder.userPubkeyHex,
    0,
    1,
  );
  t.is(deposits[0].status, "Withdrawn");
  t.is(deposits[0].deposit_tx_id, builder.tx.getId());
  t.is(deposits[0].deposit_vout, 0);
  t.is(deposits[0].value, builder.depositAmount);
  t.assert(deposits[0].complete_withdraw_ts > 0);
  t.is(deposits[0].withdrawal_tx_id, builder.withdrawTx!.getId());
});

test.only("submit withdraw should decrease queue withdraw amount if necessary", async (t) => {
  const { contract, alice } = t.context.accounts;
  const allstakePubkey = t.context.allstakePubkey;
  const builder = new TestTransactionBuilder(contract, alice, {
    userKeyPair: t.context.aliceKeyPair,
    allstakePubkey,
  });

  await builder.submit();
  const sig = builder.queueWithdrawSignature(5e7, 0);
  await builder.queueWithdraw(5e7, sig);

  const accountBefore = await viewAccount(contract, builder.userPubkeyHex);

  builder.generateWithdrawPsbt(undefined, undefined, 9e7);
  await builder.submitWithdraw();

  const accountAfter = await viewAccount(contract, builder.userPubkeyHex);
  t.is(accountAfter.queue_withdrawal_amount, 1e7);
  // this should not change
  t.is(
    accountAfter.queue_withdrawal_start_ts,
    accountBefore.queue_withdrawal_start_ts,
  );
});
