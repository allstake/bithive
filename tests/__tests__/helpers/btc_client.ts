import { Gas, NEAR, NearAccount } from "near-workspaces";
import { ChainSignatureResponse } from "./utils";

interface SubmitDepositArg {
  tx_hex: string;
  deposit_vout: number;
  embed_vout: number;
  user_pubkey_hex: string;
  allstake_pubkey_hex: string;
  sequence_height: number;
  tx_block_hash: string;
  tx_index: number;
  merkle_proof: string[];
}

export async function submitDepositTx(
  btcClient: NearAccount,
  caller: NearAccount,
  args: SubmitDepositArg,
): Promise<boolean> {
  return caller.call(btcClient, "submit_deposit_tx", args as any, {
    gas: Gas.parse("200 Tgas"),
  });
}

export async function queueWithdraw(
  btcClient: NearAccount,
  caller: NearAccount,
  user_pubkey: string,
  deposit_tx_id: string,
  deposit_vout: number,
  msg_sig: string,
  sig_type: string,
) {
  return caller.call(btcClient, "queue_withdraw", {
    user_pubkey,
    deposit_tx_id,
    deposit_vout,
    msg_sig,
    sig_type,
  });
}

export async function signWithdraw(
  btcClient: NearAccount,
  caller: NearAccount,
  psbtHex: string,
  userPubkey: string,
  embedVout: number,
): Promise<ChainSignatureResponse> {
  return caller.call(
    btcClient.accountId,
    "sign_withdraw",
    {
      psbt_hex: psbtHex,
      user_pubkey: userPubkey,
      embed_vout: embedVout,
    },
    {
      attachedDeposit: NEAR.parse("0.5"),
      gas: Gas.parse("300 Tgas"),
    },
  );
}

export async function submitWithdrawTx(
  btcClient: NearAccount,
  caller: NearAccount,
  tx_hex: string,
  user_pubkey: string,
  embed_vout: number,
  tx_block_hash: string,
  tx_index: number,
  merkle_proof: string[],
) {
  return caller.call(
    btcClient,
    "submit_withdraw_tx",
    {
      tx_hex,
      user_pubkey,
      embed_vout,
      tx_block_hash,
      tx_index,
      merkle_proof,
    },
    {
      gas: Gas.parse("200 Tgas"),
    },
  );
}

export async function fastForward(btcClient: NearAccount, duration: number) {
  return btcClient.call(btcClient, "fast_forward", {
    duration,
  });
}
