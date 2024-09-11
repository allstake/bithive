import { getConfig } from "./config";
import { connect } from "near-api-js";

export async function initNear(env: string, signerId?: string) {
  const config = await getConfig(env);
  const near = await connect(config.near);
  const signer = await near.account(signerId ?? config.accountIds.signer);
  return { signer };
}
