import { connect } from "near-api-js";
import { NEAR } from "near-workspaces";
import { getConfig } from "./config";
import { nearTGas } from "./helper";

export async function initNear(env: string, signerId?: string) {
  const config = await getConfig(env);
  const near = await connect(config.near);
  const signer = await near.account(signerId ?? config.accountIds.signer);
  return { signer };
}

export async function getSummary(env: string): Promise<{
  solo_withdrawal_sequence_heights: number[];
}> {
  const config = await getConfig(env);
  const { signer } = await initNear(env);
  return signer.viewFunction({
    contractId: config.accountIds.bithive,
    methodName: "get_summary",
    args: {},
  });
}

export async function getV1DepositConstants(
  env: string,
  pubkey: string,
): Promise<{
  bithive_pubkey: string;
}> {
  const config = await getConfig(env);
  const { signer } = await initNear(env);
  return signer.viewFunction({
    contractId: config.accountIds.bithive,
    methodName: "get_v1_deposit_constants",
    args: {
      deposit_vout: 0,
      user_pubkey: pubkey,
      sequence_height: 0,
    },
  });
}

export async function signWithdrawal(
  env: string,
  psbtHex: string,
  userPubkey: string,
  depositVin: number,
): Promise<{
  big_r: {
    affine_point: string;
  };
  s: {
    scalar: string;
  };
  recovery_id: number;
}> {
  const config = await getConfig(env);
  const { signer } = await initNear(env);
  const res: any = await signer.functionCall({
    contractId: config.accountIds.bithive,
    methodName: "sign_withdrawal",
    args: {
      psbt_hex: psbtHex,
      user_pubkey: userPubkey,
      deposit_vin: depositVin,
    },
    gas: nearTGas(300),
    attachedDeposit: NEAR.parse("0.5").toBigInt(),
  });
  const successValue = res.status.SuccessValue;
  return JSON.parse(Buffer.from(successValue, "base64").toString("ascii"));
}
