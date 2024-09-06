import { Gas, NEAR, NearAccount } from "near-workspaces";
import { ChainSignatureResponse } from "./utils";

export const V1_PK_PATH = "/btc/manage/v1"; // this should be equal to the one defined in contract

interface SubmitDepositArg {
  tx_hex: string;
  deposit_vout: number;
  embed_vout: number;
  user_pubkey_hex: string;
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

export async function queueWithdrawal(
  btcClient: NearAccount,
  caller: NearAccount,
  user_pubkey: string,
  deposit_tx_id: string,
  deposit_vout: number,
  msg_sig: string,
  sig_type: string,
) {
  return caller.call(btcClient, "queue_withdrawal", {
    user_pubkey,
    deposit_tx_id,
    deposit_vout,
    msg_sig,
    sig_type,
  });
}

export async function signWithdrawal(
  btcClient: NearAccount,
  caller: NearAccount,
  psbtHex: string,
  userPubkey: string,
  depositVin: number,
): Promise<ChainSignatureResponse> {
  return caller.call(
    btcClient.accountId,
    "sign_withdrawal",
    {
      psbt_hex: psbtHex,
      user_pubkey: userPubkey,
      deposit_vin: depositVin,
    },
    {
      attachedDeposit: NEAR.parse("0.5"),
      gas: Gas.parse("300 Tgas"),
    },
  );
}

export async function submitWithdrawalTx(
  btcClient: NearAccount,
  caller: NearAccount,
  tx_hex: string,
  user_pubkey: string,
  deposit_vin: number,
  tx_block_hash: string,
  tx_index: number,
  merkle_proof: string[],
) {
  return caller.call(
    btcClient,
    "submit_withdrawal_tx",
    {
      tx_hex,
      user_pubkey,
      deposit_vin,
      tx_block_hash,
      tx_index,
      merkle_proof,
    },
    {
      gas: Gas.parse("200 Tgas"),
    },
  );
}

export async function syncChainSignatureRootPubkey(btcClient: NearAccount) {
  return btcClient.call(
    btcClient,
    "sync_chain_signature_root_pubkey",
    {},
    {
      gas: Gas.parse("60 Tgas"),
    },
  );
}

export async function changeOwner(
  btcClient: NearAccount,
  caller: NearAccount,
  newOwner: NearAccount,
) {
  return caller.call(
    btcClient,
    "change_owner",
    {
      new_owner_id: newOwner.accountId,
    },
    {
      attachedDeposit: "1",
    },
  );
}

export async function setBtcLightclientId(
  btcClient: NearAccount,
  caller: NearAccount,
  contract: NearAccount,
) {
  return caller.call(
    btcClient,
    "set_btc_lightclient_id",
    {
      new_contract_id: contract.accountId,
    },
    {
      attachedDeposit: "1",
    },
  );
}

export async function setNConfirmation(
  btcClient: NearAccount,
  caller: NearAccount,
  n: number,
) {
  return caller.call(
    btcClient,
    "set_n_confirmation",
    {
      n,
    },
    {
      attachedDeposit: "1",
    },
  );
}

export async function setWithdrawWaitingTime(
  btcClient: NearAccount,
  caller: NearAccount,
  ms: number,
) {
  return caller.call(
    btcClient,
    "set_withdraw_waiting_time",
    {
      ms,
    },
    {
      attachedDeposit: "1",
    },
  );
}

interface ContractSummary {
  owner_id: string;
  btc_lightclient_id: string;
  chain_signature_id: string;
  chain_signature_root_pubkey: string;
  n_confirmation: number;
  withdraw_waiting_time_ms: number;
}

export async function getSummary(
  btcClient: NearAccount,
): Promise<ContractSummary> {
  return btcClient.view("get_summary");
}

export async function fastForward(btcClient: NearAccount, duration: number) {
  return btcClient.call(btcClient, "fast_forward", {
    duration,
  });
}

export async function setCurrentAccountId(btcClient: NearAccount, id: string) {
  return btcClient.call(btcClient, "set_current_account_id", { id });
}

function buildGetUserLenFunction(name: string) {
  return (btcClient: NearAccount, userPubkey: string): Promise<number> => {
    return btcClient.view(`user_${name}_len`, { user_pubkey: userPubkey });
  };
}

export const getUserActiveDepositsLen =
  buildGetUserLenFunction("active_deposits");
export const getUserQueueWithdrawalDepositsLen = buildGetUserLenFunction(
  "queue_withdrawal_deposits",
);
export const getUserWithdrawnDepositsLen =
  buildGetUserLenFunction("withdrawn_deposits");

interface Deposit {
  redeem_version: string;
  deposit_tx_id: string;
  deposit_vout: number;
  value: number;
  queue_withdraw_ts: number;
  queue_withdraw_message: string | null;
  queue_withdraw_sig: string | null;
  complete_withdraw_ts: number;
  withdrawal_tx_id: string | null;
}

function buildListUserDepositFunction(name: string) {
  return (
    btcClient: NearAccount,
    userPubkey: string,
    offset: number,
    limit: number,
  ): Promise<Deposit[]> => {
    return btcClient.view(`list_user_${name}`, {
      user_pubkey: userPubkey,
      offset,
      limit,
    });
  };
}

export const listUserActiveDeposits =
  buildListUserDepositFunction("active_deposits");
export const listUserQueueWithdrawalDeposits = buildListUserDepositFunction(
  "queue_withdrawal_deposits",
);
export const listUserWithdrawnDeposits =
  buildListUserDepositFunction("withdrawn_deposits");
