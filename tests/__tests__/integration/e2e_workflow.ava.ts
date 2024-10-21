import { message } from "@okxweb3/coin-bitcoin";
import * as bitcoin from "bitcoinjs-lib";
import { toXOnly } from "bitcoinjs-lib/src/psbt/bip371";
import { isP2TR } from "bitcoinjs-lib/src/psbt/psbtutils";
import { ECPairInterface } from "ecpair";
import { NearAccount } from "near-workspaces";
import { RegtestUtils } from "regtest-client";
import * as ecc from "tiny-secp256k1";
import {
  depositScriptV1,
  getWitnessUtxo,
  multisigWithdrawScript,
  partialSignPsbt,
  reconstructSignature,
} from "../helpers/btc";
import {
  fastForward,
  queueWithdrawal,
  signWithdrawal,
  submitDepositTx,
  submitWithdrawalTx,
  V1_PK_PATH,
} from "../helpers/btc_client";
import { setSignature } from "../helpers/chain_signature";
import { initIntegration } from "../helpers/context";
import { requestSigFromTestnet } from "../helpers/near_client";
import { buildDepositEmbedMsg, daysToMs, someH256 } from "../helpers/utils";

const bip68 = require("bip68"); // eslint-disable-line
bitcoin.initEccLib(ecc);
const test = initIntegration();

test("Deposit and withdraw workflow e2e", async (t) => {
  const aliceKp = t.context.keyPairs.alice;
  const regtestUtils = t.context.regtestUtils;
  const network = regtestUtils.network;
  const { contract, mockChainSignature, alice } = t.context.accounts;

  const fundAmount = 4e5;
  const depositAmount1 = 2e5;
  const depositAmount2 = 3e5;
  const withdrawAmount = 3e5;

  const waitBlocks = 5;
  const mineBlocks = 1;

  const userP2wpkhPayment = bitcoin.payments.p2wpkh({
    pubkey: aliceKp.publicKey,
    network,
  });

  // -- 1. deposit
  const { p2wsh } = await makeDeposit(
    contract,
    alice,
    regtestUtils,
    fundAmount,
    depositAmount1,
    waitBlocks,
    aliceKp,
    t.context.allstakePubkey,
    network,
  );
  await makeDeposit(
    contract,
    alice,
    regtestUtils,
    fundAmount,
    depositAmount2,
    waitBlocks,
    aliceKp,
    t.context.allstakePubkey,
    network,
  );

  /// -- 2. Withdraw both deposits at once
  // user request queue withdraw
  await makeQueueWithdrawal(contract, alice, withdrawAmount, aliceKp);

  // fast forward
  await fastForward(contract, daysToMs(3));

  const depositUnspent1 = (await regtestUtils.unspents(p2wsh.address!))[0];
  const depositUtx1 = await regtestUtils.fetch(depositUnspent1.txId);
  const depositUnspent2 = (await regtestUtils.unspents(p2wsh.address!))[1];
  const depositUtx2 = await regtestUtils.fetch(depositUnspent2.txId);

  // construct withdraw PSBT, which needs be sent to user and allstake to sign
  let psbt = new bitcoin.Psbt({ network });
  psbt = psbt
    .addInput({
      hash: depositUnspent1.txId,
      index: depositUnspent1.vout,
      witnessUtxo: getWitnessUtxo(depositUtx1.outs[depositUnspent1.vout]),
      witnessScript: p2wsh.redeem!.output!,
    })
    .addInput({
      hash: depositUnspent2.txId,
      index: depositUnspent2.vout,
      witnessUtxo: getWitnessUtxo(depositUtx2.outs[depositUnspent2.vout]),
      witnessScript: p2wsh.redeem!.output!,
    });

  const gasFee = 500;
  // reinvestment output
  const { p2wsh: reinvestOutput, depositEmbed } = buildDepositOutputs(
    aliceKp.publicKey,
    t.context.allstakePubkey,
    waitBlocks,
    network,
  );
  psbt = psbt
    .addOutput({
      address: reinvestOutput.address!,
      value: depositAmount1 + depositAmount2 - withdrawAmount,
    })
    .addOutput({
      script: depositEmbed.output!,
      value: 0,
    });
  // withdraw output
  psbt = psbt.addOutput({
    address: userP2wpkhPayment.address!,
    value: withdrawAmount - gasFee,
  });

  const psbtUnsignedTx: bitcoin.Transaction = (psbt as any).__CACHE.__TX;
  const psbtUnsignedTxId = psbtUnsignedTx.getId();

  // first, user signs both inputs of the psbt
  const { partialSignedPsbt: partialSignedPsbt1, hashToSign: hashToSign1 } =
    partialSignPsbt(psbt, aliceKp, 0);
  const userPartialSig1 = partialSignedPsbt1.data.inputs[0].partialSig!;
  const userSig1 = userPartialSig1[0].signature;
  const { partialSignedPsbt: partialSignedPsbt2, hashToSign: hashToSign2 } =
    partialSignPsbt(psbt, aliceKp, 1);
  const userPartialSig2 = partialSignedPsbt2.data.inputs[1].partialSig!;
  const userSig2 = userPartialSig2[0].signature;

  // then, allstake signs psbt
  const allstakeSig1 = await makeSignWithdrawal(
    contract,
    alice,
    mockChainSignature,
    psbt,
    0,
    1,
    aliceKp.publicKey.toString("hex"),
    hashToSign1,
  );
  const allstakeSig2 = await makeSignWithdrawal(
    contract,
    alice,
    mockChainSignature,
    psbt,
    1,
    1,
    aliceKp.publicKey.toString("hex"),
    hashToSign2,
  );

  // construct txn to broadcast

  // combine both signatures and build transaction
  const withdrawTx: bitcoin.Transaction = psbtUnsignedTx;

  // set witness for both inputs
  const redeemWitness1 = bitcoin.payments.p2wsh({
    network,
    redeem: {
      network,
      output: p2wsh.redeem!.output!,
      input: multisigWithdrawScript(userSig1, allstakeSig1),
    },
  }).witness!;
  withdrawTx.setWitness(0, redeemWitness1);
  const redeemWitness2 = bitcoin.payments.p2wsh({
    network,
    redeem: {
      network,
      output: p2wsh.redeem!.output!,
      input: multisigWithdrawScript(userSig2, allstakeSig2),
    },
  }).witness!;
  withdrawTx.setWitness(1, redeemWitness2);

  t.is(withdrawTx.getId(), psbtUnsignedTxId, "Withdraw txn id mismatch");

  await regtestUtils.mine(mineBlocks);
  await regtestUtils.broadcast(withdrawTx.toHex());
  await regtestUtils.mine(1);
  await regtestUtils.verify({
    txId: withdrawTx.getId(),
    address: userP2wpkhPayment.address!,
    vout: 2,
    value: withdrawAmount - gasFee,
  });

  // finally, submit withdraw tx for both withdrawals to allstake
  await submitWithdrawalTx(contract, alice, {
    tx_hex: withdrawTx.toHex(),
    user_pubkey: aliceKp.publicKey.toString("hex"),
    reinvest_embed_vout: 1,
    tx_block_hash: someH256,
    tx_index: 66,
    merkle_proof: [someH256, someH256],
  });

  console.log("Sign withdraw e2e workflow done!");
});

async function prepareAllstakeSignature(
  chainSignature: NearAccount,
  hashToSign: Buffer,
) {
  // request actual signature from chain sig testnet
  const sigResponse = await requestSigFromTestnet(hashToSign, V1_PK_PATH);

  // upload the actual signature response to mocked chain sig contract
  // so that sign_withdrawal call could resolve
  await setSignature(chainSignature, hashToSign, sigResponse);
}

async function fundP2wpkh(
  regtestUtils: RegtestUtils,
  userPubkey: Buffer,
  fundAmount: number,
  network: bitcoin.Network,
) {
  const userP2wpkhPayment = bitcoin.payments.p2wpkh({
    pubkey: userPubkey,
    network,
  });
  return regtestUtils.faucet(userP2wpkhPayment.address!, fundAmount);
}

async function fundP2pkh(
  regtestUtils: RegtestUtils,
  userPubkey: Buffer,
  fundAmount: number,
  network: bitcoin.Network,
) {
  const userP2pkhPayment = bitcoin.payments.p2pkh({
    pubkey: userPubkey,
    network,
  });
  return regtestUtils.faucet(userP2pkhPayment.address!, fundAmount);
}

async function fundP2tr(
  regtestUtils: RegtestUtils,
  userPubkey: Buffer,
  fundAmount: number,
  network: bitcoin.Network,
) {
  const userP2trPayment = bitcoin.payments.p2tr({
    internalPubkey: toXOnly(userPubkey),
    network,
  });
  return regtestUtils.faucetComplex(
    Buffer.from(userP2trPayment.output!),
    fundAmount,
  );
}

async function makeDeposit(
  contract: NearAccount,
  caller: NearAccount,
  regtestUtils: RegtestUtils,
  fundAmount: number,
  depositAmount: number,
  waitBlocks: number,
  userKeyPair: ECPairInterface,
  allstakePubkey: Buffer,
  network: bitcoin.Network,
) {
  // fund user's wallet first
  const utxos = [
    await fundP2wpkh(regtestUtils, userKeyPair.publicKey, fundAmount, network), // segwit
    await fundP2pkh(regtestUtils, userKeyPair.publicKey, fundAmount, network), // non-segwit
    await fundP2tr(regtestUtils, userKeyPair.publicKey, fundAmount, network), // taproot
  ];

  const depositPsbt = new bitcoin.Psbt({ network });
  const isTaproot: boolean[] = [];

  // inputs
  for (const utxo of utxos) {
    const fundUtx = await regtestUtils.fetch(utxo.txId);
    if (isP2TR(Buffer.from(fundUtx.outs[utxo.vout].script, "hex"))) {
      depositPsbt.addInput({
        hash: utxo.txId,
        index: utxo.vout,
        witnessUtxo: getWitnessUtxo(fundUtx.outs[utxo.vout]),
        tapInternalKey: toXOnly(userKeyPair.publicKey),
      });
      isTaproot.push(true);
    } else {
      depositPsbt.addInput({
        hash: utxo.txId,
        index: utxo.vout,
        nonWitnessUtxo: Buffer.from(fundUtx.txHex, "hex"), // works with both segwit and non-segwit
      });
      isTaproot.push(false);
    }
  }

  // outputs
  const { p2wsh, depositEmbed } = buildDepositOutputs(
    userKeyPair.publicKey,
    allstakePubkey,
    waitBlocks,
    network,
  );
  depositPsbt
    .addOutput({
      address: p2wsh.address!,
      value: depositAmount,
    })
    .addOutput({
      script: depositEmbed.output!,
      value: 0,
    });

  // sign inputs

  // tweaked key pair is used for taproot inputs
  // when using browser extension, this is automatically handled by signPsbt
  const tweakedKeyPair = userKeyPair.tweak(
    bitcoin.crypto.taggedHash("TapTweak", toXOnly(userKeyPair.publicKey)),
  );
  for (let i = 0; i < utxos.length; i++) {
    if (isTaproot[i]) {
      depositPsbt.signInput(i, tweakedKeyPair);
    } else {
      depositPsbt.signInput(i, userKeyPair);
    }
  }

  depositPsbt.finalizeAllInputs();

  // broadcast transaction
  const depositTx = depositPsbt.extractTransaction();
  await regtestUtils.broadcast(depositTx.toHex());
  await regtestUtils.verify({
    txId: depositTx.getId(),
    address: p2wsh.address!,
    vout: 0,
    value: depositAmount,
  });

  // submit transaction to NEAR
  await submitDepositTx(contract, caller, {
    tx_hex: depositTx.toHex(),
    embed_vout: 1,
    tx_block_hash: someH256,
    tx_index: 1,
    merkle_proof: [someH256],
  });
  console.log("submitDepositTx ok");

  return {
    depositTx,
    p2wsh,
  };
}

function buildDepositOutputs(
  publicKey: Buffer,
  allstakePubkey: Buffer,
  waitBlocks: number,
  network: bitcoin.Network,
) {
  const sequence = bip68.encode({ blocks: waitBlocks });
  const p2wsh = bitcoin.payments.p2wsh({
    redeem: {
      output: depositScriptV1(publicKey, allstakePubkey, sequence),
    },
    network,
  });

  const embedDepositMsg = buildDepositEmbedMsg(
    0,
    publicKey.toString("hex"),
    waitBlocks,
  );
  const depositEmbed = bitcoin.payments.embed({
    data: [embedDepositMsg],
  });

  return {
    p2wsh,
    depositEmbed,
  };
}

async function makeQueueWithdrawal(
  contract: NearAccount,
  caller: NearAccount,
  withdrawAmount: number,
  userKeyPair: ECPairInterface,
) {
  const withdrawMsgPlain = `bithive.withdraw:0:${withdrawAmount}sats`;
  console.log("withdrawMsgPlain", withdrawMsgPlain);
  const sigBase64 = message.sign(userKeyPair.toWIF(), withdrawMsgPlain);
  const sigHex = Buffer.from(sigBase64, "base64").toString("hex");
  await queueWithdrawal(
    contract,
    caller,
    userKeyPair.publicKey.toString("hex"),
    withdrawAmount,
    sigHex,
    "ECDSA",
  );
  console.log("queueWithdraw ok");
}

async function makeSignWithdrawal(
  contract: NearAccount,
  caller: NearAccount,
  mockChainSignature: NearAccount,
  psbt: bitcoin.Psbt,
  depositVin: number,
  reinvestVout: number,
  userPubkey: string,
  hashToSign: Buffer,
) {
  // fetch real signature from testnet and upload to mock chain sig contract
  await prepareAllstakeSignature(mockChainSignature, hashToSign);

  // call btc client contract to sign withdraw PSBT
  const sig = await signWithdrawal(
    contract,
    caller,
    psbt.toHex(),
    userPubkey,
    depositVin,
    reinvestVout,
  );

  return bitcoin.script.signature.encode(
    reconstructSignature(sig.big_r.affine_point, sig.s.scalar),
    bitcoin.Transaction.SIGHASH_ALL,
  );
}
