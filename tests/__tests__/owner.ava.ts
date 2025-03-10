import {
  proposeChangeOwner,
  acceptChangeOwner,
  getSummary,
  setBtcLightClientId,
  setNConfirmation,
  setPaused,
  setWithdrawWaitingTime,
  submitDepositTx,
} from "./helpers/bithive";
import { initUnit } from "./helpers/context";
import { assertFailure } from "./helpers/utils";

const test = initUnit();

test("non-privileged account cannot call owner methods", async (t) => {
  const { contract, alice } = t.context.accounts;
  await assertFailure(
    t,
    proposeChangeOwner(contract, alice, alice),
    "Not owner",
  );

  await assertFailure(
    t,
    setBtcLightClientId(contract, alice, alice),
    "Not owner",
  );

  await assertFailure(t, setNConfirmation(contract, alice, 1), "Not owner");

  await assertFailure(
    t,
    setWithdrawWaitingTime(contract, alice, 1),
    "Not owner",
  );

  await assertFailure(t, setPaused(contract, alice, true), "Not owner");
});

test("change owner", async (t) => {
  const { contract, owner, alice } = t.context.accounts;

  await proposeChangeOwner(contract, owner, alice);
  await acceptChangeOwner(contract, alice);

  const summary = await getSummary(contract);
  t.is(summary.owner_id, alice.accountId);
});

test("cannot accept change owner if not pending", async (t) => {
  const { contract, owner } = t.context.accounts;
  await assertFailure(
    t,
    acceptChangeOwner(contract, owner),
    "No pending owner",
  );
});

test("only pending owner can accept change owner", async (t) => {
  const { contract, owner, alice, bob } = t.context.accounts;
  await proposeChangeOwner(contract, owner, alice);
  await assertFailure(t, acceptChangeOwner(contract, bob), "Not pending owner");
});

test("set n confirmation", async (t) => {
  const { contract, owner } = t.context.accounts;

  await setNConfirmation(contract, owner, 1);

  const summary = await getSummary(contract);
  t.is(summary.n_confirmation, 1);
});

test("set withdrawal waiting time", async (t) => {
  const { contract, owner } = t.context.accounts;

  await setWithdrawWaitingTime(contract, owner, 111);

  const summary = await getSummary(contract);
  t.is(summary.withdrawal_waiting_time_ms, 111);
});

test("pause contract", async (t) => {
  const { contract, owner } = t.context.accounts;
  await setPaused(contract, owner, true);

  const summary = await getSummary(contract);
  t.is(summary.paused, true);

  await assertFailure(
    t,
    submitDepositTx(contract, owner, {
      tx_hex: "00",
      embed_vout: 0,
      tx_block_hash: "00",
      tx_index: 0,
      merkle_proof: [],
    }),
    "Contract is paused",
  );
});
