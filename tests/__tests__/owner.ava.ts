import {
  changeOwner,
  getSummary,
  setBtcLightClientId,
  setNConfirmation,
  setWithdrawWaitingTime,
} from "./helpers/btc_client";
import { initUnit } from "./helpers/context";
import { assertFailure } from "./helpers/utils";

const test = initUnit();

test("non-privileged account cannot call owner methods", async (t) => {
  const { contract, alice } = t.context.accounts;
  await assertFailure(t, changeOwner(contract, alice, alice), "Not owner");

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
});

test("change owner", async (t) => {
  const { contract, owner, alice } = t.context.accounts;

  await changeOwner(contract, owner, alice);

  const summary = await getSummary(contract);
  t.is(summary.owner_id, alice.accountId);
});

test("set n confirmation", async (t) => {
  const { contract, owner } = t.context.accounts;

  await setNConfirmation(contract, owner, 1);

  const summary = await getSummary(contract);
  t.is(summary.n_confirmation, 1);
});

test("set withdraw waiting time", async (t) => {
  const { contract, owner } = t.context.accounts;

  await setWithdrawWaitingTime(contract, owner, 111);

  const summary = await getSummary(contract);
  t.is(summary.withdraw_waiting_time_ms, 111);
});
