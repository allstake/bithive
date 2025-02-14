import { Gas, NEAR, NearAccount } from "near-workspaces";
import { ChainSignatureResponse } from "./utils";

export const V1_PK_PATH = "/bithive/v1"; // this should be equal to the one defined in contract

interface SubmitDepositArg {
  tx_hex: string;
  embed_vout: number;
  tx_block_hash: string;
  tx_index: number;
  merkle_proof: string[];
}

export async function submitDepositTx(
  bithive: NearAccount,
  caller: NearAccount,
  args: SubmitDepositArg,
): Promise<boolean> {
  return caller.call(
    bithive,
    "submit_deposit_tx",
    { args },
    {
      gas: Gas.parse("200 Tgas"),
      attachedDeposit: NEAR.parse("0.03"),
    },
  );
}

type SigType =
  | "ECDSA"
  | {
      Bip322Full: {
        address: string;
      };
    };

export async function queueWithdrawal(
  bithive: NearAccount,
  caller: NearAccount,
  user_pubkey: string,
  withdraw_amount: number,
  msg_sig: string,
  sig_type: SigType,
) {
  return caller.call(
    bithive,
    "queue_withdrawal",
    {
      user_pubkey,
      withdraw_amount,
      msg_sig,
      sig_type,
    },
    {
      gas: Gas.parse("80 Tgas"),
    },
  );
}

export async function signWithdrawal(
  bithive: NearAccount,
  caller: NearAccount,
  psbtHex: string,
  userPubkey: string,
  vinToSign: number,
  reinvestEmbedVout?: number,
  storageDeposit?: NEAR,
): Promise<ChainSignatureResponse | null> {
  const attachedDeposit = NEAR.parse("0.5").add(
    storageDeposit ?? NEAR.parse("0"),
  );

  return caller.call(
    bithive.accountId,
    "sign_withdrawal",
    {
      psbt_hex: psbtHex,
      user_pubkey: userPubkey,
      vin_to_sign: vinToSign,
      reinvest_embed_vout: reinvestEmbedVout,
      storage_deposit: storageDeposit ? storageDeposit.toString() : null,
    },
    {
      attachedDeposit,
      gas: Gas.parse("300 Tgas"),
    },
  );
}

export async function submitWithdrawalTx(
  bithive: NearAccount,
  caller: NearAccount,
  args: {
    tx_hex: string;
    user_pubkey: string;
    tx_block_hash: string;
    tx_index: number;
    merkle_proof: string[];
  },
) {
  return caller.call(
    bithive,
    "submit_withdrawal_tx",
    {
      args,
    },
    {
      gas: Gas.parse("200 Tgas"),
    },
  );
}

export async function syncChainSignaturesRootPubkey(bithive: NearAccount) {
  return bithive.call(
    bithive,
    "sync_chain_signatures_root_pubkey",
    {},
    {
      gas: Gas.parse("60 Tgas"),
    },
  );
}

export async function proposeChangeOwner(
  bithive: NearAccount,
  caller: NearAccount,
  newOwner: NearAccount,
) {
  return caller.call(
    bithive,
    "propose_change_owner",
    {
      new_owner_id: newOwner.accountId,
    },
    {
      attachedDeposit: "1",
    },
  );
}

export async function acceptChangeOwner(
  bithive: NearAccount,
  caller: NearAccount,
) {
  return caller.call(
    bithive,
    "accept_change_owner",
    {},
    {
      attachedDeposit: "1",
    },
  );
}

export async function setBtcLightClientId(
  bithive: NearAccount,
  caller: NearAccount,
  contract: NearAccount,
) {
  return caller.call(
    bithive,
    "set_btc_light_client_id",
    {
      new_contract_id: contract.accountId,
    },
    {
      attachedDeposit: "1",
    },
  );
}

export async function setNConfirmation(
  bithive: NearAccount,
  caller: NearAccount,
  n: number,
) {
  return caller.call(
    bithive,
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
  bithive: NearAccount,
  caller: NearAccount,
  ms: number,
) {
  return caller.call(
    bithive,
    "set_withdrawal_waiting_time",
    {
      ms,
    },
    {
      attachedDeposit: "1",
    },
  );
}

export async function setEarliestDepositBlockHeight(
  bithive: NearAccount,
  caller: NearAccount,
  height: number,
) {
  return caller.call(
    bithive,
    "set_earliest_deposit_block_height",
    { height },
    {
      attachedDeposit: "1",
    },
  );
}

export async function setPaused(
  bithive: NearAccount,
  caller: NearAccount,
  paused: boolean,
) {
  return caller.call(
    bithive,
    "set_paused",
    { paused },
    {
      attachedDeposit: "1",
    },
  );
}

interface ContractSummary {
  owner_id: string;
  btc_light_client_id: string;
  chain_signatures_id: string;
  chain_signatures_root_pubkey: string;
  n_confirmation: number;
  withdrawal_waiting_time_ms: number;
  paused: boolean;
}

export async function getSummary(
  bithive: NearAccount,
): Promise<ContractSummary> {
  return bithive.view("get_summary");
}

export async function fastForward(bithive: NearAccount, duration: number) {
  return bithive.call(bithive, "fast_forward", {
    duration,
  });
}

export async function setCurrentAccountId(bithive: NearAccount, id: string) {
  return bithive.call(bithive, "set_current_account_id", { id });
}

function buildGetUserLenFunction(name: string) {
  return (bithive: NearAccount, userPubkey: string): Promise<number> => {
    return bithive.view(`user_${name}_len`, { user_pubkey: userPubkey });
  };
}

export const getUserActiveDepositsLen =
  buildGetUserLenFunction("active_deposits");
export const getUserWithdrawnDepositsLen =
  buildGetUserLenFunction("withdrawn_deposits");

interface Deposit {
  user_pubkey: string;
  status: "Active" | "Withdrawn";
  redeem_version: string;
  deposit_tx_id: string;
  deposit_vout: number;
  value: number;
  sequence: number;
  complete_withdrawal_ts: number;
  withdrawal_tx_id: string | null;
}

function buildListUserDepositFunction(name: string) {
  return (
    bithive: NearAccount,
    userPubkey: string,
    offset: number,
    limit: number,
  ): Promise<Deposit[]> => {
    return bithive.view(`list_user_${name}`, {
      user_pubkey: userPubkey,
      offset,
      limit,
    });
  };
}

export const listUserActiveDeposits =
  buildListUserDepositFunction("active_deposits");
export const listUserWithdrawnDeposits =
  buildListUserDepositFunction("withdrawn_deposits");

interface Account {
  pubkey: string;
  total_deposit: number;
  queue_withdrawal_amount: number;
  queue_withdrawal_start_ts: number;
  nonce: number;
  pending_sign_psbt: {
    psbt: string;
    reinvest_deposit_vout: number | null;
  } | null;
}

export async function viewAccount(
  bithive: NearAccount,
  userPubkey: string,
): Promise<Account> {
  return bithive.view("view_account", { user_pubkey: userPubkey });
}

export async function accountsLen(bithive: NearAccount): Promise<number> {
  return bithive.view("accounts_len");
}

export async function listAccounts(
  bithive: NearAccount,
  offset: number,
  limit: number,
): Promise<Account[]> {
  return bithive.view("list_accounts", { offset, limit });
}
