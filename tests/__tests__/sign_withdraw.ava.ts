import { fastForward, signWithdraw } from "./helpers/btc_client";
import { initUnit } from "./helpers/context";
import { TestTransactionBuilder } from "./helpers/txn_builder";
import { assertFailure, daysToMs } from "./helpers/utils";

const test = initUnit();

test("sign withdraw with invalid PSBT", async (t) => {
  const { contract, alice } = t.context.accounts;
  const userPubkey = t.context.unisatPubkey;
  const allstakePubkey = t.context.allstakePubkey;
  const builder = new TestTransactionBuilder(contract, alice, {
    userPubkey,
    allstakePubkey,
  });
  await builder.submit();
  await builder.queueWithdraw(t.context.unisatSig);
  const psbt = builder.generatePsbt();
  const psbtHex = psbt.toHex();

  await assertFailure(
    t,
    signWithdraw(
      contract,
      alice,
      `11${psbtHex}`, // wrong
      userPubkey.toString("hex"),
      1,
    ),
    "Invalid PSBT hex",
  );
});

test("sign withdraw with wrong input number", async (t) => {
  const { contract, alice } = t.context.accounts;
  const userPubkey = t.context.unisatPubkey;
  const allstakePubkey = t.context.allstakePubkey;
  const builder = new TestTransactionBuilder(contract, alice, {
    userPubkey,
    allstakePubkey,
  });
  await builder.submit();
  await builder.queueWithdraw(t.context.unisatSig);
  builder.generatePsbt(true); // wrong

  await assertFailure(
    t,
    builder.signWithdraw(),
    "Withdraw txn must have only 1 input",
  );
});

test("sign withdraw with wrong embed vout", async (t) => {
  const { contract, alice } = t.context.accounts;
  const userPubkey = t.context.unisatPubkey;
  const allstakePubkey = t.context.allstakePubkey;
  const builder = new TestTransactionBuilder(contract, alice, {
    userPubkey,
    allstakePubkey,
  });
  await builder.submit();
  await builder.queueWithdraw(t.context.unisatSig);
  const psbt = builder.generatePsbt();

  await assertFailure(
    t,
    signWithdraw(
      contract,
      alice,
      psbt.toHex(),
      userPubkey.toString("hex"),
      0, // wrong
    ),
    "Embed output is not OP_RETURN",
  );
});

test("sign withdraw with wrong embed msg", async (t) => {
  const { contract, alice } = t.context.accounts;
  const userPubkey = t.context.unisatPubkey;
  const allstakePubkey = t.context.allstakePubkey;
  const builder = new TestTransactionBuilder(contract, alice, {
    userPubkey,
    allstakePubkey,
  });
  await builder.submit();
  await builder.queueWithdraw(t.context.unisatSig);
  builder.generatePsbt(false, "withdraw"); // wrong

  await assertFailure(t, builder.signWithdraw(), "Wrong embed message");
});

test("sign withdraw without queueing first", async (t) => {
  const { contract, alice } = t.context.accounts;
  const userPubkey = t.context.unisatPubkey;
  const allstakePubkey = t.context.allstakePubkey;
  const builder = new TestTransactionBuilder(contract, alice, {
    userPubkey,
    allstakePubkey,
  });
  await builder.submit();
  builder.generatePsbt();

  await assertFailure(t, builder.signWithdraw(), "Deposit is not in queue");
});

test("sign withdraw within waiting period", async (t) => {
  const { contract, alice } = t.context.accounts;
  const userPubkey = t.context.unisatPubkey;
  const allstakePubkey = t.context.allstakePubkey;
  const builder = new TestTransactionBuilder(contract, alice, {
    userPubkey,
    allstakePubkey,
  });
  await builder.submit();
  await builder.queueWithdraw(t.context.unisatSig);
  builder.generatePsbt();

  await fastForward(contract, daysToMs(2) - 1); // wrong

  await assertFailure(t, builder.signWithdraw(), "Not ready to withdraw now");
});

test("sign withdraw after waiting period", async (t) => {
  const { contract, alice } = t.context.accounts;
  const userPubkey = t.context.unisatPubkey;
  const allstakePubkey = t.context.allstakePubkey;
  const builder = new TestTransactionBuilder(contract, alice, {
    userPubkey,
    allstakePubkey,
  });
  await builder.submit();
  await builder.queueWithdraw(t.context.unisatSig);
  builder.generatePsbt();

  await fastForward(contract, daysToMs(2) + 1);

  const sig = await builder.signWithdraw();
  t.is(
    sig.big_r.affine_point,
    "02E14D22E30DF1F02A3C46C52EB2B999AB009600FA945CACD3242AD66480E26EA7",
  );
  t.is(
    sig.s.scalar,
    "7E7ADD7EF49E871C41EDF56BDF5C93B44E21A83CD55FA656318A1F0E6CD17CE9",
  );
  t.is(sig.recovery_id, 0);
});
