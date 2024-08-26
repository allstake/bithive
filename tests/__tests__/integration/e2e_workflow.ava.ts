import { message } from "@okxweb3/coin-bitcoin";
import * as bitcoin from "bitcoinjs-lib";
import { NearAccount } from "near-workspaces";
import {
  depositScriptV1,
  getWitnessUtxo,
  idToHash,
  multisigWithdrawScript,
  partialSignPsbt,
  reconstructSignature,
  toOutputScript,
} from "../helpers/btc";
import {
  fastForward,
  queueWithdraw,
  signWithdraw,
  submitDepositTx,
  submitWithdrawTx,
} from "../helpers/btc_client";
import { setSignature } from "../helpers/chain_signature";
import { initIntegration } from "../helpers/context";
import { requestSigFromTestnet } from "../helpers/near_client";
import { daysToMs, someH256 } from "../helpers/utils";

const bip68 = require("bip68"); // eslint-disable-line
const test = initIntegration();

test("Deposit and withdraw workflow e2e", async (t) => {
  const aliceKp = t.context.keyPairs.alice;
  const regtestUtils = t.context.regtestUtils;
  const network = regtestUtils.network;
  const { contract, mockChainSignature, alice } = t.context.accounts;

  const fundAmount = 3e5;
  const depositAmount = 2e5;
  const withdrawAmount = 1e5;

  const waitBlocks = 5;
  const mineBlocks = 1;

  // make sure the timestamp is not 0 at first
  await fastForward(contract, daysToMs(3));

  // -- 0. init
  // fund user's wallet first
  const userP2wpkhAddress = bitcoin.payments.p2wpkh({
    pubkey: aliceKp.publicKey,
    network,
  });
  const fundUnspent = await regtestUtils.faucet(
    userP2wpkhAddress.address!,
    fundAmount,
  );
  const fundUtx = await regtestUtils.fetch(fundUnspent.txId);

  // -- 1. deposit
  // user transfer BTC from his wallet to his staking vault address
  const sequence = bip68.encode({ blocks: waitBlocks });
  const p2wsh = bitcoin.payments.p2wsh({
    redeem: {
      output: depositScriptV1(
        aliceKp.publicKey,
        t.context.allstakePubkey,
        sequence,
      ),
    },
    network,
  });

  const depositEmbed = bitcoin.payments.embed({
    data: [Buffer.from("allstake.deposit.v1")],
  });
  const depositPsbt = new bitcoin.Psbt({ network })
    .addInput({
      hash: fundUnspent.txId,
      index: fundUnspent.vout,
      nonWitnessUtxo: Buffer.from(fundUtx.txHex, "hex"),
    })
    .addOutput({
      address: p2wsh.address!,
      value: depositAmount,
    })
    .addOutput({
      script: depositEmbed.output!,
      value: 0,
    })
    .signInput(0, aliceKp);
  depositPsbt.finalizeAllInputs();
  const depositTx = depositPsbt.extractTransaction();
  await regtestUtils.broadcast(depositTx.toHex());
  await regtestUtils.verify({
    txId: depositTx.getId(),
    address: p2wsh.address!,
    vout: 0,
    value: depositAmount,
  });

  // submit deposit to allstake
  await submitDepositTx(contract, alice, {
    tx_hex: depositTx.toHex(),
    deposit_vout: 0,
    embed_vout: 1,
    user_pubkey_hex: aliceKp.publicKey.toString("hex"),
    allstake_pubkey_hex: t.context.allstakePubkey.toString("hex"),
    sequence_height: waitBlocks,
    tx_block_hash: someH256,
    tx_index: 1,
    merkle_proof: [someH256],
  });
  console.log("submitDepositTx ok");

  /// -- 2. Withdraw
  // user withdraw BTC in his staking vault to his wallet

  // user request queue withdraw
  const depositVout = 0;
  const withdrawMsgPlain = `allstake.withdraw:${depositTx.getId()}:${depositVout}`;
  console.log({ withdrawMsgPlain });
  const sigBase64 = message.sign(aliceKp.toWIF(), withdrawMsgPlain);
  const sigHex = Buffer.from(sigBase64, "base64").toString("hex");
  await queueWithdraw(
    contract,
    alice,
    aliceKp.publicKey.toString("hex"),
    depositTx.getId(),
    depositVout,
    sigHex,
    "Unisat",
  );
  console.log("queueWithdraw ok");

  // fast forward
  await fastForward(contract, daysToMs(3));

  const depositUnspent = (await regtestUtils.unspents(p2wsh.address!))[0];
  const depositUtx = await regtestUtils.fetch(depositUnspent.txId);
  const withdrawMsg = Buffer.from("allstake.withdraw");

  // construct PSBT, which needs be sent to user and allstake to sign
  let psbt = new bitcoin.Psbt({ network });
  psbt = psbt.addInput({
    hash: depositUnspent.txId,
    index: depositUnspent.vout,
    witnessUtxo: getWitnessUtxo(depositUtx.outs[depositUnspent.vout]),
    witnessScript: p2wsh.redeem!.output!,
  });
  psbt = psbt
    .addOutput({
      address: userP2wpkhAddress.address!,
      value: withdrawAmount,
    })
    .addOutput({
      script: bitcoin.script.compile([bitcoin.opcodes.OP_RETURN, withdrawMsg]),
      value: 0,
    });
  const psbtUnsignedTx: bitcoin.Transaction = (psbt as any).__CACHE.__TX;
  const psbtUnsignedTxId = psbtUnsignedTx.getId();

  // first, user signs psbt
  const { partialSignedPsbt, hashToSign } = partialSignPsbt(psbt, aliceKp);
  const userPartialSig = partialSignedPsbt.data.inputs[0].partialSig!;
  const userSig = userPartialSig[0].signature;
  console.log("user sig");
  console.log(userSig.toString("hex"));

  // then, allstake signs psbt

  // fetch real signature from testnet and upload to mock chain sig contract
  await prepareAllstakeSignature(mockChainSignature, hashToSign);

  // call btc client contract to sign withdraw PSBT
  const sig = await signWithdraw(
    contract,
    alice,
    psbt.toHex(),
    aliceKp.publicKey.toString("hex"),
    1,
  );
  const allstakeSig = bitcoin.script.signature.encode(
    reconstructSignature(sig.big_r.affine_point, sig.s.scalar),
    bitcoin.Transaction.SIGHASH_ALL,
  );
  console.log("allstake sig");
  console.log(allstakeSig.toString("hex"));

  // construct txn to broadcast

  // combine both signatures and build transaction
  const withdrawTx = new bitcoin.Transaction();
  withdrawTx.version = 2;
  withdrawTx.addInput(idToHash(depositUnspent.txId), depositUnspent.vout);
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
      input: multisigWithdrawScript(userSig, allstakeSig),
    },
  }).witness!;
  withdrawTx.setWitness(0, redeemWitness);

  t.is(withdrawTx.getId(), psbtUnsignedTxId, "Withdraw txn id mismatch");

  await regtestUtils.mine(mineBlocks);
  await regtestUtils.broadcast(withdrawTx.toHex());
  await regtestUtils.mine(1);
  await regtestUtils.verify({
    txId: withdrawTx.getId(),
    address: userP2wpkhAddress.address!,
    vout: 0,
    value: withdrawAmount,
  });

  // finally, submit withdraw tx to allstake
  await submitWithdrawTx(
    contract,
    alice,
    withdrawTx.toHex(),
    aliceKp.publicKey.toString("hex"),
    1,
    someH256,
    66,
    [someH256, someH256],
  );

  console.log("Sign withdraw e2e workflow done!");
});

async function prepareAllstakeSignature(
  chainSignature: NearAccount,
  hashToSign: Buffer,
) {
  // request actual signature from chain sig testnet
  const sigResponse = await requestSigFromTestnet(hashToSign);

  // upload the actual signature response to mocked chain sig contract
  // so that sign_withdraw call could resolve
  await setSignature(chainSignature, sigResponse);
}
