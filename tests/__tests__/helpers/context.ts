import anyTest, { TestFn } from "ava";
import { NearAccount, Worker } from "near-workspaces";
import { daysToMs } from "./utils";
import ECPairFactory, { ECPairInterface } from "ecpair";
import { RegtestUtils } from "regtest-client";
import * as ecc from "tiny-secp256k1";
import { compressPubKey, deriveAddress } from "./btc";
import "dotenv/config";
import {
  fastForward,
  setCurrentAccountId,
  syncChainSignatureRootPubkey,
  V1_PK_PATH,
} from "./btc_client";

const ECPair = ECPairFactory(ecc);

export function initUnit() {
  const test = anyTest as TestFn<{
    worker: Worker;
    accounts: Record<string, NearAccount>;
    aliceKeyPair: ECPairInterface;
    bobKeyPair: ECPairInterface;
    allstakePubkey: Buffer;
  }>;

  test.beforeEach(async (t) => {
    // Init the worker and start a Sandbox server
    const worker = await Worker.init();

    // --
    // contract related

    // Create accounts
    const root = worker.rootAccount;
    const fixtures = await createFixtures(root);

    // btc
    const aliceKeyPair = ECPair.makeRandom();
    const bobKeyPair = ECPair.makeRandom();
    const allstakePubkey = Buffer.from(
      "037c3f573f302b20d0d53fe7b7940d63c966ffc1579cacdbcad700e4a401fee10d",
      "hex",
    );

    t.context.worker = worker;
    t.context.accounts = {
      root,
      ...fixtures,
    };
    t.context.aliceKeyPair = aliceKeyPair;
    t.context.bobKeyPair = bobKeyPair;
    t.context.allstakePubkey = allstakePubkey;
  });

  test.afterEach(async (t) => {
    // Stop Sandbox server
    await t.context.worker.tearDown().catch((error) => {
      console.log("Failed to stop the Sandbox:", error);
    });
  });

  return test;
}

export function initIntegration() {
  const test = anyTest as TestFn<{
    worker: Worker;
    accounts: Record<string, NearAccount>;
    keyPairs: Record<string, ECPairInterface>;
    regtestUtils: RegtestUtils;
    allstakePubkey: Buffer;
    nearTestnetAccountId: string;
  }>;

  // since tests could run in parallel, be careful when calling regtestUtils because
  // they can overlap with each other
  const regtestUtils = new RegtestUtils({ APIURL: "http://localhost:8080/1" });

  test.beforeEach(async (t) => {
    // Init the worker and start a Sandbox server
    const worker = await Worker.init();

    // --
    // contract related

    // Create accounts
    const root = worker.rootAccount;
    const fixtures = await createFixtures(root);

    // --
    // btc related
    // hardcode every key so that the mocked chain signature can produce the correct result
    const aliceKeyPair = ECPair.makeRandom();
    console.log("alice    pubkey", aliceKeyPair.publicKey.toString("hex"));

    const nearTestnetAccountId = process.env.TESTNET_ACCOUNT_ID;
    if (!nearTestnetAccountId) {
      throw new Error("missing env TESTNET_ACCOUNT_ID");
    }

    // this is to make sure the derived allstake pubkey matches
    // the actual signature generated by nearTestnetAccountId
    await setCurrentAccountId(fixtures.contract, nearTestnetAccountId);

    const allstakePkUncompressed = (
      await deriveAddress(nearTestnetAccountId, V1_PK_PATH, "testnet")
    ).publicKey;
    const allstakePubkey = compressPubKey(allstakePkUncompressed);

    t.context.worker = worker;
    t.context.accounts = {
      root,
      ...fixtures,
    };
    t.context.keyPairs = {
      alice: aliceKeyPair,
    };
    t.context.regtestUtils = regtestUtils;
    t.context.allstakePubkey = allstakePubkey;
    t.context.nearTestnetAccountId = nearTestnetAccountId;
  });

  test.afterEach(async (t) => {
    // Stop Sandbox server
    await t.context.worker.tearDown().catch((error) => {
      console.log("Failed to stop the Sandbox:", error);
    });
  });

  return test;
}

async function createFixtures(root: NearAccount) {
  const owner = await root.createSubAccount("owner");
  const alice = await root.createSubAccount("alice");

  const mockLightclient = await deployAndInit({
    root,
    subContractId: "lightclient",
    code: "res/mock_btc_lightclient.wasm",
    init: {
      methodName: "init",
      args: {},
    },
  });

  const mockChainSignature = await deployAndInit({
    root,
    subContractId: "chain-sig",
    code: "res/mock_chain_signature.wasm",
    init: {
      methodName: "init",
      args: {},
    },
  });

  const contract = await deployAndInit({
    root,
    subContractId: "btc-client",
    code: "res/btc_client_test.wasm",
    init: {
      methodName: "init",
      args: {
        owner_id: owner.accountId,
        btc_lightclient_id: mockLightclient.accountId,
        chain_signature_id: mockChainSignature.accountId,
        n_confirmation: 6,
        withdraw_waiting_time_ms: daysToMs(2),
      },
    },
  });
  await syncChainSignatureRootPubkey(contract);

  // make sure the timestamp is not 0 at first
  await fastForward(contract, daysToMs(3));

  return {
    owner,
    alice,
    contract,
    mockChainSignature,
    mockLightclient,
  };
}

async function deployAndInit({
  root,
  subContractId,
  code,
  init,
  initialBalance,
}: {
  root: NearAccount;
  subContractId: string;
  code: Uint8Array | string;
  init?: {
    methodName: string;
    args?: Record<string, unknown>;
  };
  initialBalance?: string;
}): Promise<NearAccount> {
  const contract = await root.createSubAccount(subContractId, {
    initialBalance,
  });
  const result = await contract.deploy(code);
  if (result.failed) {
    throw result.Failure;
  }
  if (init) {
    await contract.call(contract, init.methodName, init.args ?? {});
  }
  return contract;
}
