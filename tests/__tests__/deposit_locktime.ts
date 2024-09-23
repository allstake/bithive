import {
  getUserActiveDepositsLen,
  setEarliestDepositBlockHeight,
} from "./helpers/btc_client";
import { initUnit } from "./helpers/context";
import { TestTransactionBuilder } from "./helpers/txn_builder";
import { assertFailure } from "./helpers/utils";

const test = initUnit();

test("submit deposit txn with wrong timelock config", async (t) => {
  const { contract, alice, owner } = t.context.accounts;
  // enable timelock
  await setEarliestDepositBlockHeight(contract, owner, 101);

  const builder = new TestTransactionBuilder(contract, alice, {
    userPubkey: t.context.aliceKeyPair.publicKey,
    allstakePubkey: t.context.allstakePubkey,
    enableTimelock: true, // this sets locktime to 100
  });

  await assertFailure(
    t,
    builder.submit(),
    "Transaction locktime should be set to 101",
  );
});

test("submit deposit txn without timelock", async (t) => {
  const { contract, alice, owner } = t.context.accounts;
  // enable timelock
  await setEarliestDepositBlockHeight(contract, owner, 101);

  const builder = new TestTransactionBuilder(contract, alice, {
    userPubkey: t.context.aliceKeyPair.publicKey,
    allstakePubkey: t.context.allstakePubkey,
    enableTimelock: false, // wrong
  });

  await assertFailure(
    t,
    builder.submit(),
    "Transaction absolute timelock not enabled",
  );
});

test("submit deposit txn with timelock", async (t) => {
  const { contract, alice, owner } = t.context.accounts;
  // enable timelock
  await setEarliestDepositBlockHeight(contract, owner, 100);

  const builder = new TestTransactionBuilder(contract, alice, {
    userPubkey: t.context.aliceKeyPair.publicKey,
    allstakePubkey: t.context.allstakePubkey,
    enableTimelock: true,
  });

  await builder.submit();
  t.is(await getUserActiveDepositsLen(contract, builder.userPubkeyHex), 1);
});
