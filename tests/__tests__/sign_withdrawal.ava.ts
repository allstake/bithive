import * as bitcoin from "bitcoinjs-lib";
import { fastForward, signWithdrawal, viewAccount } from "./helpers/btc_client";
import { initUnit } from "./helpers/context";
import { TestTransactionBuilder } from "./helpers/txn_builder";
import { assertFailure, buildDepositEmbedMsg, daysToMs } from "./helpers/utils";

const test = initUnit();

async function makeDeposit(t: any, amount = 1e8) {
  const { contract, alice } = t.context.accounts;
  const userPubkey = t.context.unisatPubkey;
  const allstakePubkey = t.context.allstakePubkey;
  const builder = new TestTransactionBuilder(contract, alice, {
    userPubkey,
    allstakePubkey,
    depositAmount: amount,
  });
  await builder.submit();

  return {
    builder,
    contract,
    userPubkey,
    account: alice,
  };
}

test("sign withdraw with invalid PSBT", async (t) => {
  const {
    builder,
    contract,
    userPubkey,
    account: alice,
  } = await makeDeposit(t);
  await builder.queueWithdraw(100, t.context.unisatSig);
  const psbt = builder.generateWithdrawPsbt();
  const psbtHex = psbt.toHex();

  await assertFailure(
    t,
    signWithdrawal(
      contract,
      alice,
      `11${psbtHex}`, // wrong
      userPubkey.toString("hex"),
      1,
    ),
    "Invalid PSBT hex",
  );
});

test("sign withdraw without queueing first", async (t) => {
  const { builder } = await makeDeposit(t);
  builder.generateWithdrawPsbt();
  await assertFailure(t, builder.signWithdraw(0), "No withdraw request made");
});

test("sign withdraw with invalid deposit vin", async (t) => {
  const { builder } = await makeDeposit(t);
  await builder.queueWithdraw(100, t.context.unisatSig);

  builder.generateWithdrawPsbt({
    hash: "0000000000000000000000000000000000000000000000000000000000000000",
    index: 1,
  });

  await assertFailure(t, builder.signWithdraw(1), "Deposit is not active");
});

test("sign withdraw within waiting period", async (t) => {
  const { builder } = await makeDeposit(t);
  await builder.queueWithdraw(100, t.context.unisatSig);
  builder.generateWithdrawPsbt();

  await assertFailure(t, builder.signWithdraw(0), "Not ready to withdraw now");
});

test("sign withdraw with one deposit should clear request after signed", async (t) => {
  const { builder, contract } = await makeDeposit(t, 1e8);

  await builder.queueWithdraw(100, t.context.unisatSig);
  await fastForward(contract, daysToMs(2));

  builder.generateWithdrawPsbt(undefined, 1e8 - 100);
  await builder.signWithdraw(0);

  const account = await viewAccount(contract, builder.userPubkeyHex);
  t.is(account.pending_withdraw_tx_id, null);
  t.is(account.pending_withdraw_unsigned_count, 0);
});

test("sign withdraw with multiple deposits should clear request after fully signed", async (t) => {
  const { builder: builder1, contract } = await makeDeposit(t, 1e8);
  const { builder: builder2 } = await makeDeposit(t, 100);

  await builder1.queueWithdraw(100, t.context.unisatSig);
  await fastForward(contract, daysToMs(2));

  builder1.generateWithdrawPsbt(
    {
      hash: builder2.tx.getId(),
      index: 0,
    },
    1e8,
  );
  await builder1.signWithdraw(0);

  let account = await viewAccount(contract, builder1.userPubkeyHex);
  t.is(
    account.pending_withdraw_tx_id,
    (builder1.psbt! as any).__CACHE.__TX.getId(),
  );
  t.is(account.pending_withdraw_unsigned_count, 1);

  await builder1.signWithdraw(1);
  account = await viewAccount(contract, builder1.userPubkeyHex);
  t.is(account.pending_withdraw_tx_id, null);
  t.is(account.pending_withdraw_unsigned_count, 0);
});

test("sign withdraw twice but with different PSBT", async (t) => {
  const { builder: builder1, contract } = await makeDeposit(t, 1e8);
  const { builder: builder2 } = await makeDeposit(t, 100);

  await builder1.queueWithdraw(100, t.context.unisatSig);
  await fastForward(contract, daysToMs(2));

  builder1.generateWithdrawPsbt(
    {
      hash: builder2.tx.getId(),
      index: 0,
    },
    1e8,
  );
  await builder1.signWithdraw(0);

  builder2.generateWithdrawPsbt();
  await assertFailure(t, builder2.signWithdraw(0), "A pending withdrawal tx");
});

test("sign withdraw without reinvestment", async (t) => {
  const { builder, contract } = await makeDeposit(t, 1e8);

  await builder.queueWithdraw(1e8, t.context.unisatSig1BTC);
  await fastForward(contract, daysToMs(2));

  builder.generateWithdrawPsbt();
  await builder.signWithdraw(0);

  const account = await viewAccount(contract, builder.userPubkeyHex);
  t.is(account.pending_withdraw_tx_id, null);
  t.is(account.pending_withdraw_unsigned_count, 0);
});

test("sign withdraw with invalid reinvestment type", async (t) => {
  const { builder, contract, userPubkey, account } = await makeDeposit(t, 1e8);

  await builder.queueWithdraw(100, t.context.unisatSig);
  await fastForward(contract, daysToMs(2));

  const psbt = builder.generateWithdrawPsbt();
  const reinvestEmbedMsg = buildDepositEmbedMsg(1, builder.userPubkeyHex, 5);
  const embedOutput = bitcoin.payments.embed({
    data: [reinvestEmbedMsg],
  }).output!;
  psbt
    .addOutput({
      address: "1AtX4bUXcyMnZsKP3NpqRtr5YQ8bg7Q6Lk", // a random P2PKH address
      value: 1e8 - 100,
    })
    .addOutput({
      script: embedOutput,
      value: 0,
    });

  await assertFailure(
    t,
    signWithdrawal(
      contract,
      account,
      psbt.toHex(),
      userPubkey.toString("hex"),
      0,
      2,
    ),
    "Deposit output is not P2WSH",
  );
});

test("sign withdraw with with bad amount of reinvestment", async (t) => {
  const { builder, contract } = await makeDeposit(t, 1e8);

  await builder.queueWithdraw(100, t.context.unisatSig);
  await fastForward(contract, daysToMs(2));

  // should reinvest 1e8 - 100 sats
  builder.generateWithdrawPsbt(undefined, 1e8 - 101);
  await assertFailure(
    t,
    builder.signWithdraw(0),
    "Withdraw amount is larger than queued amount",
  );
});
