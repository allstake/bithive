import { NearAccount } from "near-workspaces";
import { ChainSignatureResponse } from "./utils";

export async function setSignature(
  chainSignature: NearAccount,
  payload: Buffer,
  sig: ChainSignatureResponse,
) {
  return chainSignature.call(chainSignature.accountId, "set_sig", {
    payload: payload.toJSON().data,
    big_r: sig.big_r.affine_point,
    s: sig.s.scalar,
    recovery_id: sig.recovery_id,
  });
}
