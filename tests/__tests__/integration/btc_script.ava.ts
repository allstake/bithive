import * as bitcoin from "bitcoinjs-lib";
import ECPairFactory from "ecpair";
import { RegtestUtils } from "regtest-client";
import * as ecc from "tiny-secp256k1";
import {
  depositScriptV1,
  getWitnessUtxo,
  idToHash,
  multisigWithdrawScript,
  soloWithdrawScript,
  toOutputScript,
} from "../helpers/btc";
import { initUnit } from "../helpers/context";

const bip68 = require("bip68"); // eslint-disable-line
const regtestUtils = new RegtestUtils({ APIURL: "http://localhost:8080/1" });
const network = regtestUtils.network;
const ECPair = ECPairFactory(ecc);

const test = initUnit(false); // although this is integration test, but we don't need all the contracts

const SEQUENCE_TIMELOCK = 0xfffffffd; // sequence that enables time-lock and RBF

async function testCase(
  t: any,
  opts: {
    mineBlocksBeforeBroadcast: number;
    waitBlocks: number;
    mineBlocks: number;
    soloWithdraw: boolean;
    expectStakeFailure: boolean;
    expectWithdrawFailure: boolean;
  },
) {
  const user = ECPair.makeRandom();
  const bithive = ECPair.makeRandom();

  const fundAmount = 3e5;
  const stakeAmount = 2e5;
  const withdrawAmount = 1e5;

  // deposit transactions can only be broadcast after (current height + 3) blocks
  const lockTimeBlocks = 3;

  /// -- 0. Init
  // fund user's wallet first
  const userP2wpkhAddress = bitcoin.payments.p2wpkh({
    pubkey: user.publicKey,
    network,
  });
  const fundUnspent = await regtestUtils.faucet(
    userP2wpkhAddress.address!,
    fundAmount,
  );
  const fundUtx = await regtestUtils.fetch(fundUnspent.txId);

  /// -- 1. Stake
  // user transfer BTC from his wallet to his staking vault address
  const currentBlockHeight = await regtestUtils.height();
  const sequence = bip68.encode({ blocks: opts.waitBlocks });
  const p2wsh = bitcoin.payments.p2wsh({
    redeem: {
      output: depositScriptV1(user.publicKey, bithive.publicKey, sequence),
    },
    network,
  });

  const stakeEmbed = bitcoin.payments.embed({
    data: [Buffer.from("bithive.deposit.v1")],
  });
  const stakePsbt = new bitcoin.Psbt({ network })
    .addInput({
      hash: fundUnspent.txId,
      index: fundUnspent.vout,
      witnessUtxo: getWitnessUtxo(fundUtx.outs[fundUnspent.vout]),
      sequence: SEQUENCE_TIMELOCK,
    })
    .addOutput({
      address: p2wsh.address!,
      value: stakeAmount,
    })
    .addOutput({
      script: stakeEmbed.output!,
      value: 0,
    })
    .setLocktime(currentBlockHeight + lockTimeBlocks)
    .signInput(0, user);
  stakePsbt.finalizeAllInputs();
  const stakeTx = stakePsbt.extractTransaction();

  await regtestUtils.mine(opts.mineBlocksBeforeBroadcast);
  await broadcastTransaction(t, stakeTx, opts.expectStakeFailure);
  if (opts.expectStakeFailure) {
    return;
  }
  await regtestUtils.verify({
    txId: stakeTx.getId(),
    address: p2wsh.address!,
    vout: 0,
    value: stakeAmount,
  });

  /// -- 2. Withdraw
  // user withdraw BTC in his staking vault to his wallet
  const stakeUnspent = {
    txId: stakeTx.getId(),
    vout: 0,
  };
  const stakeUtx = await regtestUtils.fetch(stakeUnspent.txId);
  const withdrawMsg = Buffer.from("bithive.withdraw");

  // construct PSBT, which needs be sent to user and bithive to sign
  let psbt = new bitcoin.Psbt({ network });
  if (opts.soloWithdraw) {
    psbt = psbt.addInput({
      hash: stakeUnspent.txId,
      index: stakeUnspent.vout,
      witnessUtxo: getWitnessUtxo(stakeUtx.outs[stakeUnspent.vout]),
      witnessScript: p2wsh.redeem!.output!,
      sequence,
    });
  } else {
    psbt = psbt.addInput({
      hash: stakeUnspent.txId,
      index: stakeUnspent.vout,
      witnessUtxo: getWitnessUtxo(stakeUtx.outs[stakeUnspent.vout]),
      witnessScript: p2wsh.redeem!.output!,
    });
  }
  psbt = psbt
    .addOutput({
      address: userP2wpkhAddress.address!,
      value: withdrawAmount,
    })
    .addOutput({
      script: bitcoin.script.compile([bitcoin.opcodes.OP_RETURN, withdrawMsg]),
      value: 0,
    });

  // user and bithive both signs
  const userSignedPsbt = psbt.clone().signInput(0, user);
  const userPartialSig = userSignedPsbt.data.inputs[0].partialSig!;
  const userSig = userPartialSig[0].signature;

  const bithiveSignedPsbt = psbt.clone().signInput(0, bithive);
  const bithivePartialSig = bithiveSignedPsbt.data.inputs[0].partialSig!;
  const bithiveSig = bithivePartialSig[0].signature;

  // combine both signatures and build transaction
  const withdrawTx = new bitcoin.Transaction();
  withdrawTx.version = 2;
  if (opts.soloWithdraw) {
    withdrawTx.addInput(
      idToHash(stakeUnspent.txId),
      stakeUnspent.vout,
      sequence,
    );
  } else {
    withdrawTx.addInput(idToHash(stakeUnspent.txId), stakeUnspent.vout);
  }
  // withdraw to user's address
  withdrawTx.addOutput(
    toOutputScript(userP2wpkhAddress.address!, network),
    withdrawAmount,
  );
  // embed extra data via OP_RETURN
  const withdrawEmbed = bitcoin.payments.embed({ data: [withdrawMsg] });
  withdrawTx.addOutput(withdrawEmbed.output!, 0);

  const redeemWitness = bitcoin.payments.p2wsh({
    network,
    redeem: {
      network,
      output: p2wsh.redeem!.output!,
      input: opts.soloWithdraw
        ? soloWithdrawScript(userSig)
        : multisigWithdrawScript(userSig, bithiveSig),
    },
  }).witness!;
  withdrawTx.setWitness(0, redeemWitness);

  await regtestUtils.mine(opts.mineBlocks);

  await broadcastTransaction(t, withdrawTx, opts.expectWithdrawFailure);
  if (!opts.expectWithdrawFailure) {
    await regtestUtils.verify({
      txId: withdrawTx.getId(),
      address: userP2wpkhAddress.address!,
      vout: 0,
      value: withdrawAmount,
    });
  }
}

test("btc script test", async (t) => {
  // after waiting period, solo withdrawal
  await testCase(t, {
    mineBlocksBeforeBroadcast: 4,
    waitBlocks: 5,
    mineBlocks: 6,
    soloWithdraw: true,
    expectStakeFailure: false,
    expectWithdrawFailure: false,
  });
  // after waiting period, multisig withdrawal
  await testCase(t, {
    mineBlocksBeforeBroadcast: 4,
    waitBlocks: 5,
    mineBlocks: 6,
    soloWithdraw: false,
    expectStakeFailure: false,
    expectWithdrawFailure: false,
  });
  // within waiting period, solo withdrawal
  await testCase(t, {
    mineBlocksBeforeBroadcast: 4,
    waitBlocks: 5,
    mineBlocks: 4,
    soloWithdraw: true,
    expectStakeFailure: false,
    expectWithdrawFailure: true,
  });
  // within waiting period, multisig withdrawal
  await testCase(t, {
    mineBlocksBeforeBroadcast: 4,
    waitBlocks: 5,
    mineBlocks: 4,
    soloWithdraw: false,
    expectStakeFailure: false,
    expectWithdrawFailure: false,
  });
  // broadcast before locktime
  await testCase(t, {
    mineBlocksBeforeBroadcast: 2,
    waitBlocks: 5,
    mineBlocks: 6,
    soloWithdraw: true,
    expectStakeFailure: true,
    expectWithdrawFailure: true,
  });
});

async function broadcastTransaction(
  t: any,
  tx: bitcoin.Transaction,
  expectToFail: boolean,
) {
  let failed = false;
  let err: Error | null = null;
  try {
    await regtestUtils.broadcast(tx.toHex());
  } catch (error: any) {
    failed = true;
    err = error;
  }

  if (expectToFail) {
    t.is(failed, true, "❗txn didn't fail!");
    // console.log('✅ Txn failed as expected, error:', err!.toString());
  } else {
    if (err) {
      console.log(err);
    }
    t.is(failed, false, "❗txn failed!");
    // console.log('✅ Txn went ok as expected');
  }
}
