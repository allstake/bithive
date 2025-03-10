import { toXOnly } from "bitcoinjs-lib/src/psbt/bip371";
import { fastForward, viewAccount } from "./helpers/bithive";
import { initUnit } from "./helpers/context";
import { TestTransactionBuilder } from "./helpers/txn_builder";
import { assertFailure, daysToMs } from "./helpers/utils";
import * as ecc from "tiny-secp256k1";
import * as bitcoin from "bitcoinjs-lib";

const test = initUnit();

async function makeDeposit(t: any, inputTxIndex = 0) {
  const { contract, alice } = t.context.accounts;
  const bithivePubkey = t.context.bithivePubkey;

  // real user pubkey from unisat
  const builder = new TestTransactionBuilder(contract, alice, {
    userKeyPair: t.context.aliceKeyPair,
    bithivePubkey,
    inputTxIndex,
  });
  await builder.submit();

  return {
    builder,
    contract,
    alice,
  };
}

test("queue withdrawal with invalid signature", async (t) => {
  const bobKeyPair = t.context.bobKeyPair;
  const { builder: aliceBuilder, contract, alice } = await makeDeposit(t);
  const bobBuilder = new TestTransactionBuilder(contract, alice, {
    userKeyPair: bobKeyPair,
    bithivePubkey: t.context.bithivePubkey,
  });

  // generate signagure from bob
  const bobSig = bobBuilder.queueWithdrawSignature(100, 0);

  await assertFailure(
    t,
    aliceBuilder.queueWithdraw(100, bobSig),
    "Invalid bitcoin signature",
  );
});

test("valid queue withdrawal", async (t) => {
  const { builder, contract } = await makeDeposit(t);
  const sig = builder.queueWithdrawSignature(100, 0);
  await builder.queueWithdraw(100, sig);

  const account = await viewAccount(contract, builder.userPubkeyHex);
  t.is(account.queue_withdrawal_amount, 100);
  t.is(account.queue_withdrawal_start_ts, daysToMs(3));
  t.is(account.nonce, 1);
  t.is(account.pending_sign_psbt, null);
});

test("queue withdrawal with wrong amount in signature", async (t) => {
  const { builder } = await makeDeposit(t);
  // signature of msg: "bithive.withdraw:0:1000sats"
  const sig = builder.queueWithdrawSignature(1000, 0);
  await assertFailure(
    t,
    builder.queueWithdraw(100, sig),
    "Invalid bitcoin signature",
  );
});

test("queue withdrawal with wrong nonce (reuse signature)", async (t) => {
  const { builder } = await makeDeposit(t);
  const sig = builder.queueWithdrawSignature(100, 0);
  await builder.queueWithdraw(100, sig);

  await assertFailure(
    t,
    builder.queueWithdraw(100, sig),
    "Invalid bitcoin signature",
  );
});

test("queue withdrawal with bad amount", async (t) => {
  const { builder } = await makeDeposit(t);
  // signature of msg: "bithive.withdraw:0:200000000sats"
  const sig = builder.queueWithdrawSignature(2e8, 0);

  await assertFailure(
    t,
    builder.queueWithdraw(2e8, sig),
    " Invalid queue withdrawal amount",
  );
});

test("a second queue withdrawal request should reset the waiting period", async (t) => {
  const { builder, contract } = await makeDeposit(t);
  const sig = builder.queueWithdrawSignature(100, 0);
  await builder.queueWithdraw(100, sig);

  await fastForward(contract, daysToMs(1));
  const sig2 = builder.queueWithdrawSignature(1000, 1);
  await builder.queueWithdraw(1000, sig2);

  const account = await viewAccount(contract, builder.userPubkeyHex);
  t.is(account.queue_withdrawal_amount, 1100);
  t.is(account.queue_withdrawal_start_ts, daysToMs(4));
});

test("queue withdrawal again after deposit", async (t) => {
  const { builder, contract } = await makeDeposit(t);
  const sig = builder.queueWithdrawSignature(100, 0);
  await builder.queueWithdraw(100, sig);

  // make another deposit, which makes the total deposit to 2 BTC
  await makeDeposit(t, 1);

  await fastForward(contract, daysToMs(1));

  // queue withdrawal again
  // signature of msg: "bithive.withdraw:1:100000000sats"
  const sig2 = builder.queueWithdrawSignature(1e8, 1);
  await builder.queueWithdraw(1e8, sig2);

  const account = await viewAccount(contract, builder.userPubkeyHex);
  t.is(account.queue_withdrawal_amount, 1e8 + 100);
  t.is(account.queue_withdrawal_start_ts, daysToMs(4));
});

test("queue withdrawal should clear pending withdrawal psbt", async (t) => {
  const { builder, contract } = await makeDeposit(t, 1e8);

  const sig = builder.queueWithdrawSignature(100, 0);
  await builder.queueWithdraw(100, sig);
  await fastForward(contract, daysToMs(2));

  builder.generateWithdrawPsbt(undefined, 1e8 - 100);
  await builder.signWithdraw(0);

  let account = await viewAccount(contract, builder.userPubkeyHex);
  t.assert(account.pending_sign_psbt);
  t.assert(account.pending_sign_psbt!.psbt);
  t.assert(account.pending_sign_psbt!.reinvest_deposit_vout);

  const sig2 = builder.queueWithdrawSignature(100, 1);
  await builder.queueWithdraw(100, sig2);

  account = await viewAccount(contract, builder.userPubkeyHex);
  t.assert(account.pending_sign_psbt === null);
});

test("queue withdrawal with bip322 signature", async (t) => {
  bitcoin.initEccLib(ecc);
  const { builder } = await makeDeposit(t);
  const address = bitcoin.payments.p2tr({
    internalPubkey: toXOnly(builder.userPubkey),
    network: bitcoin.networks.bitcoin,
  }).address!;

  const sigOk = "00"; // dummy signature, just to test the function
  t.assert(await builder.queueWithdrawBip322(100, sigOk, address));

  const sigBad = "01"; // dummy signature, just to test the function
  t.assert(!(await builder.queueWithdrawBip322(100, sigBad, address)));
});
