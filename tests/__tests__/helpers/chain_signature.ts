import { NearAccount } from "near-workspaces";
import { ChainSignatureResponse } from "./utils";

export async function setSignature(
  chainSignature: NearAccount,
  sig: ChainSignatureResponse,
) {
  return chainSignature.call(chainSignature.accountId, "set_sig", {
    big_r: sig.big_r.affine_point,
    s: sig.s.scalar,
    recovery_id: sig.recovery_id,
  });
}
