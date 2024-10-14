/// This is used to make calls to actual NEAR testnet contracts,
/// which is need to retrieve real chain signature responses
import { Base64 } from "js-base64";
import { connect } from "near-api-js";
import { UnencryptedFileSystemKeyStore } from "near-api-js/lib/key_stores";
import { Gas, NEAR } from "near-workspaces";
import * as os from "os";
import path from "path";
import { ChainSignatureResponse } from "./utils";

export async function initNearClient(signerId: string) {
  const config = {
    networkId: "testnet",
    keyStore: new UnencryptedFileSystemKeyStore(
      path.join(os.homedir(), ".near-credentials"),
    ),
    nodeUrl: "https://rpc.testnet.pagoda.co",
  };

  const near = await connect(config);
  const signer = await near.account(signerId);

  return {
    near,
    signer,
  };
}

export async function requestSigFromTestnet(
  payload: Buffer,
  path: string,
): Promise<ChainSignatureResponse> {
  const nearTestnetAccountId = process.env.TESTNET_ACCOUNT_ID;
  if (!nearTestnetAccountId) {
    throw new Error("missing env TESTNET_ACCOUNT_ID");
  }
  const { signer } = await initNearClient(nearTestnetAccountId);

  const args = {
    request: {
      key_version: 0,
      path,
      payload: payload.toJSON().data,
    },
  };
  const res = await signer.functionCall({
    contractId: "v1.signer-dev.testnet",
    methodName: "sign",
    args,
    attachedDeposit: NEAR.parse("0.5").toBigInt(),
    gas: Gas.parse("250 Tgas").toBigInt(),
  });
  const base64Encoded = (res.status as any).SuccessValue;
  const sig: ChainSignatureResponse = JSON.parse(Base64.decode(base64Encoded));
  return sig;
}
