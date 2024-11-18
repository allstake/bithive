import * as bitcoin from "bitcoinjs-lib";
import { fastForward, signWithdrawal, viewAccount } from "./helpers/btc_client";
import { initUnit } from "./helpers/context";
import { TestTransactionBuilder } from "./helpers/txn_builder";
import { assertFailure, buildDepositEmbedMsg, daysToMs } from "./helpers/utils";
import { depositScriptV1 } from "./helpers/btc";

const test = initUnit();

async function makeDeposit(t: any, amount = 1e8) {
  const { contract, alice } = t.context.accounts;
  const userPubkey = t.context.aliceKeyPair.publicKey;
  const bithivePubkey = t.context.bithivePubkey;
  const builder = new TestTransactionBuilder(contract, alice, {
    userKeyPair: t.context.aliceKeyPair,
    bithivePubkey,
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
  const sig = builder.queueWithdrawSignature(100, 0);
  await builder.queueWithdraw(100, sig);
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
  await assertFailure(t, builder.signWithdraw(0), "No withdrawal request made");
});

test("sign withdraw with invalid deposit vin", async (t) => {
  const { builder } = await makeDeposit(t);
  const sig = builder.queueWithdrawSignature(100, 0);
  await builder.queueWithdraw(100, sig);

  builder.generateWithdrawPsbt({
    hash: "0000000000000000000000000000000000000000000000000000000000000000",
    index: 1,
  });

  await assertFailure(t, builder.signWithdraw(1), "Deposit is not active");
});

test("sign withdraw within waiting period", async (t) => {
  const { builder } = await makeDeposit(t);
  const sig = builder.queueWithdrawSignature(100, 0);
  await builder.queueWithdraw(100, sig);
  builder.generateWithdrawPsbt();

  await assertFailure(t, builder.signWithdraw(0), "Not ready to withdraw now");
});

test("sign withdraw should set pending withdraw psbt", async (t) => {
  const { builder, contract } = await makeDeposit(t, 1e8);

  const sig = builder.queueWithdrawSignature(100, 0);
  await builder.queueWithdraw(100, sig);
  await fastForward(contract, daysToMs(2));

  builder.generateWithdrawPsbt(undefined, 1e8 - 100);
  await builder.signWithdraw(0);

  const account = await viewAccount(contract, builder.userPubkeyHex);
  t.is(account.pending_sign_psbt!.psbt, builder.psbt!.toHex());
  t.is(account.pending_sign_psbt!.reinvest_deposit_vout, 1);
});

test("sign withdraw should reset queue withdraw amount", async (t) => {
  const { builder, contract } = await makeDeposit(t, 1e8);

  const sig = builder.queueWithdrawSignature(100, 0);
  await builder.queueWithdraw(100, sig);
  await fastForward(contract, daysToMs(2));

  builder.generateWithdrawPsbt(undefined, 1e8 - 100);
  await builder.signWithdraw(0);

  const account = await viewAccount(contract, builder.userPubkeyHex);
  t.is(account.queue_withdrawal_amount, 0);
  t.is(account.queue_withdrawal_start_ts, 0);
});

test("sign withdraw with multiple deposit inputs", async (t) => {
  const { builder: builder1, contract } = await makeDeposit(t, 1e8);
  const { builder: builder2 } = await makeDeposit(t, 2e8);

  const sig = builder1.queueWithdrawSignature(3e8, 0);
  await builder1.queueWithdraw(3e8, sig);
  await fastForward(contract, daysToMs(2));

  // withdraw psbt has two inputs to sign
  builder1.generateWithdrawPsbt(
    {
      hash: builder2.tx.getId(),
      index: 0,
    },
    0,
    3e8,
  );

  await builder1.signWithdraw(0);
  await builder1.signWithdraw(1);
});

test("sign withdraw RBF", async (t) => {
  const { builder, contract } = await makeDeposit(t, 1e8);

  const sig = builder.queueWithdrawSignature(100, 0);
  await builder.queueWithdraw(100, sig);
  await fastForward(contract, daysToMs(2));

  let withdrawAmount = 100;
  builder.generateWithdrawPsbt(undefined, 1e8 - 100, withdrawAmount);
  await builder.signWithdraw(0);

  // update withdraw psbt to override fee
  withdrawAmount -= 90;
  builder.generateWithdrawPsbt(undefined, 1e8 - 100, withdrawAmount); // actual withdraw amount is only 10 sats
  // sign the new psbt
  await builder.signWithdraw(0);
});

test("sign withdraw twice but with different PSBT", async (t) => {
  const { builder: builder1, contract } = await makeDeposit(t, 1e8);
  const { builder: builder2 } = await makeDeposit(t, 100);

  const sig = builder1.queueWithdrawSignature(100, 0);
  await builder1.queueWithdraw(100, sig);
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
  await assertFailure(
    t,
    builder2.signWithdraw(0),
    "PSBT input length mismatch",
  );
});

test("sign withdraw without reinvestment", async (t) => {
  const { builder, contract } = await makeDeposit(t, 1e8);

  const sig = builder.queueWithdrawSignature(1e8, 0);
  await builder.queueWithdraw(1e8, sig);
  await fastForward(contract, daysToMs(2));

  builder.generateWithdrawPsbt();
  await builder.signWithdraw(0);
});

test("sign withdraw with invalid reinvestment type", async (t) => {
  const { builder, contract, userPubkey, account } = await makeDeposit(t, 1e8);

  const sig = builder.queueWithdrawSignature(100, 0);
  await builder.queueWithdraw(100, sig);
  await fastForward(contract, daysToMs(2));

  const psbt = builder.generateWithdrawPsbt(
    undefined,
    undefined,
    undefined,
    false,
  );
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
  const partialSignedPsbt = builder.partialSignWithdrawPsbt(0);

  await assertFailure(
    t,
    signWithdrawal(
      contract,
      account,
      partialSignedPsbt.toHex(),
      userPubkey.toString("hex"),
      0,
      2,
    ),
    "Deposit output is not P2WSH",
  );
});

test("sign withdraw with with bad amount of reinvestment", async (t) => {
  const { builder, contract } = await makeDeposit(t, 1e8);

  const sig = builder.queueWithdrawSignature(100, 0);
  await builder.queueWithdraw(100, sig);
  await fastForward(contract, daysToMs(2));

  // should reinvest 1e8 - 100 sats
  builder.generateWithdrawPsbt(undefined, 1e8 - 101);
  await assertFailure(
    t,
    builder.signWithdraw(0),
    "Withdraw amount is larger than queued amount",
  );
});

test("sign withdraw without partial signature", async (t) => {
  const { builder, contract } = await makeDeposit(t, 1e8);

  const sig = builder.queueWithdrawSignature(100, 0);
  await builder.queueWithdraw(100, sig);
  await fastForward(contract, daysToMs(2));

  builder.generateWithdrawPsbt(undefined, undefined, undefined, false);
  await assertFailure(
    t,
    builder.signWithdraw(0),
    "Missing partial sig for given input",
  );
});

test("sign withdraw with invalid partial signature", async (t) => {
  const { builder, contract } = await makeDeposit(t, 1e8);

  const sig = builder.queueWithdrawSignature(100, 0);
  await builder.queueWithdraw(100, sig);
  await fastForward(contract, daysToMs(2));

  const psbt1 = builder.generateWithdrawPsbt(undefined, 1e8 - 100, 99).clone();
  // generate a new psbt that replaces the above one
  builder.generateWithdrawPsbt(undefined, 1e8 - 100, 98);
  // replace the partial signature with a different one
  builder.psbt!.data.inputs[0].partialSig = psbt1.data.inputs[0].partialSig;

  await assertFailure(
    t,
    builder.signWithdraw(0),
    "Invalid partial signature for withdraw PSBT",
  );
});

test("sign withdraw with reinvestment of a different pubkey", async (t) => {
  const { builder, contract } = await makeDeposit(t, 1e8);

  const sig = builder.queueWithdrawSignature(100, 0);
  await builder.queueWithdraw(100, sig);
  await fastForward(contract, daysToMs(2));

  const psbt = builder.generateWithdrawPsbt(undefined, 0, 100, false);

  // add a reinvest output for bob
  const bobP2wsh = bitcoin.payments.p2wsh({
    redeem: {
      output: depositScriptV1(
        t.context.bobKeyPair.publicKey,
        t.context.bithivePubkey,
        5,
      ),
    },
  });
  const embedReinvestMsg = buildDepositEmbedMsg(
    1,
    t.context.bobKeyPair.publicKey.toString("hex"),
    5,
  );
  const embed = bitcoin.payments.embed({
    data: [embedReinvestMsg],
  });

  psbt.addOutput({
    address: bobP2wsh.address!,
    value: 1e8 - 100,
  });
  psbt.addOutput({
    script: embed.output!,
    value: 0,
  });

  builder.psbt = psbt;
  builder.reinvest = true;
  builder.partialSignWithdrawPsbt(0);

  await assertFailure(
    t,
    builder.signWithdraw(0),
    "PSBT reinvest pubkey mismatch",
  );
});
