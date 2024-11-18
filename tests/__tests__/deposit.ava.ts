import * as bitcoin from "bitcoinjs-lib";
import {
  getUserActiveDepositsLen,
  listUserActiveDeposits,
  setEarliestDepositBlockHeight,
  submitDepositTx,
  viewAccount,
} from "./helpers/btc_client";
import { initUnit } from "./helpers/context";
import { TestTransactionBuilder } from "./helpers/txn_builder";
import { assertFailure, buildDepositEmbedMsg, someH256 } from "./helpers/utils";
import { Gas, NEAR } from "near-workspaces";

const test = initUnit();

test("submit valid deposit txn", async (t) => {
  const { contract, alice } = t.context.accounts;

  const builder = new TestTransactionBuilder(contract, alice, {
    userKeyPair: t.context.aliceKeyPair,
    bithivePubkey: t.context.bithivePubkey,
  });
  await builder.submit();

  t.is(await getUserActiveDepositsLen(contract, builder.userPubkeyHex), 1);
  const activeDeposits = await listUserActiveDeposits(
    contract,
    builder.userPubkeyHex,
    0,
    1,
  );

  t.is(activeDeposits[0].user_pubkey, builder.userPubkeyHex);
  t.is(activeDeposits[0].status, "Active");
  t.is(activeDeposits[0].redeem_version, "V1");
  t.is(activeDeposits[0].deposit_tx_id, builder.tx.getId());
  t.is(activeDeposits[0].deposit_vout, 0);
  t.is(activeDeposits[0].value, builder.depositAmount);
  t.is(activeDeposits[0].sequence, builder.sequence);
  t.is(activeDeposits[0].complete_withdraw_ts, 0);
  t.is(activeDeposits[0].withdrawal_tx_id, null);

  const account = await viewAccount(contract, builder.userPubkeyHex);
  t.is(account.pubkey, builder.userPubkeyHex);
  t.is(account.total_deposit, builder.depositAmount);
  t.is(account.queue_withdrawal_amount, 0);
  t.is(account.queue_withdrawal_start_ts, 0);
  t.is(account.nonce, 0);
  t.is(account.pending_sign_psbt, null);
});

test("submit invalid embed msg", async (t) => {
  const { contract, alice } = t.context.accounts;

  const builder = new TestTransactionBuilder(contract, alice, {
    userKeyPair: t.context.aliceKeyPair,
    bithivePubkey: t.context.bithivePubkey,
  });
  const tx = builder.tx;
  // manually add an invalid embed output
  const embedMsg = Buffer.from("wrong"); // wrong
  const embed = bitcoin.payments.embed({
    data: [embedMsg],
  });
  tx.addOutput(embed.output!, 0);

  await assertFailure(
    t,
    submitDepositTx(contract, alice, {
      tx_hex: builder.tx.toHex(),
      embed_vout: 2,
      tx_block_hash: someH256,
      tx_index: 1,
      merkle_proof: [someH256],
    }),
    "Invalid magic header",
  );
});

test("submit invalid sequence height", async (t) => {
  const { contract, alice } = t.context.accounts;

  const builder = new TestTransactionBuilder(contract, alice, {
    userKeyPair: t.context.aliceKeyPair,
    bithivePubkey: t.context.bithivePubkey,
    seq: 2,
  });

  await assertFailure(t, builder.submit(), "Invalid seq height");
});

test("submit invalid deposit txn", async (t) => {
  const { contract, alice } = t.context.accounts;

  const builder = new TestTransactionBuilder(contract, alice, {
    userKeyPair: t.context.bobKeyPair, // wrong
    bithivePubkey: t.context.bithivePubkey,
  });
  const tx = builder.tx;
  // add an other embed output which refers to a different user pubkey
  const userPubkey = t.context.aliceKeyPair.publicKey.toString("hex");
  const embedMsg = buildDepositEmbedMsg(0, userPubkey, builder.sequence);
  const embed = bitcoin.payments.embed({
    data: [embedMsg],
  });
  tx.addOutput(embed.output!, 0);

  await assertFailure(
    t,
    submitDepositTx(contract, alice, {
      tx_hex: builder.tx.toHex(),
      embed_vout: 2,
      tx_block_hash: someH256,
      tx_index: 1,
      merkle_proof: [someH256],
    }),
    "Deposit output bad script hash",
  );

  t.is(await getUserActiveDepositsLen(contract, userPubkey), 0);
});

test("submit deposit txn not confirmed", async (t) => {
  const { contract, alice } = t.context.accounts;

  const builder = new TestTransactionBuilder(contract, alice, {
    userKeyPair: t.context.aliceKeyPair,
    bithivePubkey: t.context.bithivePubkey,
  });

  await submitDepositTx(contract, alice, {
    tx_hex: builder.tx.toHex(),
    embed_vout: 1,
    tx_block_hash: someH256,
    tx_index: 0, // this makes it unconfirmed
    merkle_proof: [someH256],
  });
  t.is(await getUserActiveDepositsLen(contract, builder.userPubkeyHex), 0);
});

test("submit duplicated deposit txn", async (t) => {
  const { contract, alice } = t.context.accounts;

  const builder = new TestTransactionBuilder(contract, alice, {
    userKeyPair: t.context.aliceKeyPair,
    bithivePubkey: t.context.bithivePubkey,
  });
  await builder.submit();

  await assertFailure(t, builder.submit(), "Deposit already saved");
  t.is(await getUserActiveDepositsLen(contract, builder.userPubkeyHex), 1);
});

test("submit deposit txn with too small deposit amount", async (t) => {
  const { contract, alice } = t.context.accounts;

  const builder = new TestTransactionBuilder(contract, alice, {
    userKeyPair: t.context.aliceKeyPair,
    bithivePubkey: t.context.bithivePubkey,
    depositAmount: 10,
  });

  await assertFailure(
    t,
    builder.submit(),
    "Deposit amount is less than minimum deposit amount",
  );

  t.is(await getUserActiveDepositsLen(contract, builder.userPubkeyHex), 0);
});

test("submit deposit txn with wrong timelock config", async (t) => {
  const { contract, alice, owner } = t.context.accounts;
  // enable timelock
  await setEarliestDepositBlockHeight(contract, owner, 101);

  const builder = new TestTransactionBuilder(contract, alice, {
    userKeyPair: t.context.aliceKeyPair,
    bithivePubkey: t.context.bithivePubkey,
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
    userKeyPair: t.context.aliceKeyPair,
    bithivePubkey: t.context.bithivePubkey,
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
    userKeyPair: t.context.aliceKeyPair,
    bithivePubkey: t.context.bithivePubkey,
    enableTimelock: true,
  });

  await builder.submit();
  t.is(await getUserActiveDepositsLen(contract, builder.userPubkeyHex), 1);
});

test("submit deposit txn with insufficient NEAR deposit", async (t) => {
  const { contract, alice } = t.context.accounts;
  const builder = new TestTransactionBuilder(contract, alice, {
    userKeyPair: t.context.aliceKeyPair,
    bithivePubkey: t.context.bithivePubkey,
  });
  const tx = builder.tx;

  await assertFailure(
    t,
    alice.call(
      contract,
      "submit_deposit_tx",
      {
        args: {
          tx_hex: tx.toHex(),
          embed_vout: 1,
          tx_block_hash: someH256,
          tx_index: 1,
          merkle_proof: [someH256],
        },
      },
      {
        gas: Gas.parse("200 Tgas"),
        attachedDeposit: NEAR.parse("0.01"),
      },
    ),
    "Not enough NEAR attached",
  );
});
