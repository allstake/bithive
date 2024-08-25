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

const test = initUnit(); // although this is integration test, but we don't need all the contracts

async function testCase(
  t: any,
  opts: {
    waitBlocks: number;
    mineBlocks: number;
    soloWithdraw: boolean;
    expectFailure: boolean;
  },
) {
  const user = ECPair.makeRandom();
  const allstake = ECPair.makeRandom();

  const fundAmount = 3e5;
  const stakeAmount = 2e5;
  const withdrawAmount = 1e5;

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
  const sequence = bip68.encode({ blocks: opts.waitBlocks });
  const p2wsh = bitcoin.payments.p2wsh({
    redeem: {
      output: depositScriptV1(user.publicKey, allstake.publicKey, sequence),
    },
    network,
  });

  const stakeEmbed = bitcoin.payments.embed({
    data: [Buffer.from("allstake.deposit.v1")],
  });
  const stakePsbt = new bitcoin.Psbt({ network })
    .addInput({
      hash: fundUnspent.txId,
      index: fundUnspent.vout,
      nonWitnessUtxo: Buffer.from(fundUtx.txHex, "hex"),
    })
    .addOutput({
      address: p2wsh.address!,
      value: stakeAmount,
    })
    .addOutput({
      script: stakeEmbed.output!,
      value: 0,
    })
    .signInput(0, user);
  stakePsbt.finalizeAllInputs();
  const stakeTx = stakePsbt.extractTransaction();
  await regtestUtils.broadcast(stakeTx.toHex());
  // await regtestUtils.mine(1);
  await regtestUtils.verify({
    txId: stakeTx.getId(),
    address: p2wsh.address!,
    vout: 0,
    value: stakeAmount,
  });
  // console.log("- Stake OK");

  /// -- 2. Withdraw
  // user withdraw BTC in his staking vault to his wallet
  const stakeUnspent = (await regtestUtils.unspents(p2wsh.address!))[0];
  const stakeUtx = await regtestUtils.fetch(stakeUnspent.txId);
  const withdrawMsg = Buffer.from("allstake.withdraw");

  // construct PSBT, which needs be sent to user and allstake to sign
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

  // user and allstake both signs
  const userSignedPsbt = psbt.clone().signInput(0, user);
  const userPartialSig = userSignedPsbt.data.inputs[0].partialSig!;
  const userSig = userPartialSig[0].signature;

  const allstakeSignedPsbt = psbt.clone().signInput(0, allstake);
  const allstakePartialSig = allstakeSignedPsbt.data.inputs[0].partialSig!;
  const allstakeSig = allstakePartialSig[0].signature;

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
        : multisigWithdrawScript(userSig, allstakeSig),
    },
  }).witness!;
  withdrawTx.setWitness(0, redeemWitness);

  await regtestUtils.mine(opts.mineBlocks);

  let failed = false;
  let err: Error | null = null;
  try {
    await regtestUtils.broadcast(withdrawTx.toHex());
    // console.log("- Withdraw OK");
  } catch (error: any) {
    failed = true;
    err = error;
  }

  if (opts.expectFailure) {
    t.is(failed, true, "❗txn didn't fail!");
    // console.log('✅ Txn failed as expected, error:', err!.toString());
  } else {
    if (err) {
      console.log(err);
    }
    t.is(failed, false, "❗txn failed!");
    await regtestUtils.verify({
      txId: withdrawTx.getId(),
      address: userP2wpkhAddress.address!,
      vout: 0,
      value: withdrawAmount,
    });
    // console.log('✅ Txn went ok as expected');
  }
}

test("btc script test", async (t) => {
  // after waiting period, solo withdraw
  await testCase(t, {
    waitBlocks: 5,
    mineBlocks: 6,
    soloWithdraw: true,
    expectFailure: false,
  });
  // after waiting period, multisig withdraw
  await testCase(t, {
    waitBlocks: 5,
    mineBlocks: 6,
    soloWithdraw: false,
    expectFailure: false,
  });
  // within waiting period, solo withdraw
  await testCase(t, {
    waitBlocks: 5,
    mineBlocks: 4,
    soloWithdraw: true,
    expectFailure: true,
  });
  // within waiting period, multisig withdraw
  await testCase(t, {
    waitBlocks: 5,
    mineBlocks: 4,
    soloWithdraw: false,
    expectFailure: false,
  });
});
