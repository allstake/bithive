import {
  fastForward,
  getUserActiveDepositsLen,
  getUserQueueWithdrawalDepositsLen,
  getUserWithdrawnDepositsLen,
  listUserWithdrawnDeposits,
  submitWithdrawalTx,
} from "./helpers/btc_client";
import { initUnit } from "./helpers/context";
import { TestTransactionBuilder } from "./helpers/txn_builder";
import { assertFailure, daysToMs, someH256 } from "./helpers/utils";

const test = initUnit();

test("submit withdraw invalid txn hex", async (t) => {
  const { contract, alice } = t.context.accounts;
  const userPubkey = t.context.unisatPubkey;
  const allstakePubkey = t.context.allstakePubkey;
  const builder = new TestTransactionBuilder(contract, alice, {
    userPubkey,
    allstakePubkey,
  });
  await builder.submit();

  const withdrawTx = builder.generateWithdrawTx();

  await assertFailure(
    t,
    submitWithdrawalTx(
      contract,
      alice,
      `${withdrawTx.toHex()}fff`, // wrong
      userPubkey.toString("hex"),
      0,
      someH256,
      1,
      [someH256],
    ),
    "Invalid txn hex",
  );
});

test("submit withdraw invalid deposit", async (t) => {
  const { contract, alice } = t.context.accounts;
  const userPubkey = t.context.unisatPubkey;
  const allstakePubkey = t.context.allstakePubkey;
  const builder = new TestTransactionBuilder(contract, alice, {
    userPubkey,
    allstakePubkey,
  });

  builder.generateWithdrawTx();

  await assertFailure(t, builder.submitWithdraw(), "Deposit is not active");
});

test("submit withdraw txn not confirmed", async (t) => {
  const { contract, alice } = t.context.accounts;
  const userPubkey = t.context.unisatPubkey;
  const allstakePubkey = t.context.allstakePubkey;
  const builder = new TestTransactionBuilder(contract, alice, {
    userPubkey,
    allstakePubkey,
  });
  await builder.submit();

  const withdrawTx = builder.generateWithdrawTx();

  await submitWithdrawalTx(
    contract,
    alice,
    withdrawTx.toHex(),
    builder.userPubkeyHex,
    0,
    someH256,
    0, // wrong
    [someH256],
  );

  t.is(await getUserWithdrawnDepositsLen(contract, builder.userPubkeyHex), 0);
  t.is(await getUserActiveDepositsLen(contract, builder.userPubkeyHex), 1);
});

test("submit solo withdraw", async (t) => {
  const { contract, alice } = t.context.accounts;
  const userPubkey = t.context.unisatPubkey;
  const allstakePubkey = t.context.allstakePubkey;
  const builder = new TestTransactionBuilder(contract, alice, {
    userPubkey,
    allstakePubkey,
  });
  await builder.submit();
  await builder.submitWithdraw();

  t.is(await getUserWithdrawnDepositsLen(contract, builder.userPubkeyHex), 1);
  t.is(await getUserActiveDepositsLen(contract, builder.userPubkeyHex), 0);

  const deposits = await listUserWithdrawnDeposits(
    contract,
    builder.userPubkeyHex,
    0,
    1,
  );
  t.is(deposits[0].deposit_tx_id, builder.tx.getId());
  t.is(deposits[0].deposit_vout, 0);
  t.is(deposits[0].value, builder.depositAmount);
  t.is(deposits[0].queue_withdraw_ts, 0);
  t.is(deposits[0].queue_withdraw_message, null);
  t.assert(deposits[0].complete_withdraw_ts > 0);
  t.is(deposits[0].withdrawal_tx_id, builder.withdrawTx!.getId());
});

test("submit multisig withdraw", async (t) => {
  const { contract, alice } = t.context.accounts;
  const userPubkey = t.context.unisatPubkey;
  const allstakePubkey = t.context.allstakePubkey;
  const builder = new TestTransactionBuilder(contract, alice, {
    userPubkey,
    allstakePubkey,
  });
  await builder.submit();
  await builder.queueWithdraw(t.context.unisatSig);

  await fastForward(contract, daysToMs(2) + 1);
  await builder.submitWithdraw();

  t.is(await getUserWithdrawnDepositsLen(contract, builder.userPubkeyHex), 1);
  t.is(
    await getUserQueueWithdrawalDepositsLen(contract, builder.userPubkeyHex),
    0,
  );

  const deposits = await listUserWithdrawnDeposits(
    contract,
    builder.userPubkeyHex,
    0,
    1,
  );
  t.is(deposits[0].deposit_tx_id, builder.tx.getId());
  t.is(deposits[0].deposit_vout, 0);
  t.is(deposits[0].value, builder.depositAmount);
  t.assert(deposits[0].complete_withdraw_ts > 0);
  t.is(deposits[0].withdrawal_tx_id, builder.withdrawTx!.getId());
});
