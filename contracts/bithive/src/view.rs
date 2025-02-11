use std::cmp::min;

use crate::*;
use account::{Deposit, DepositStatus};
use bitcoin::{consensus::encode::deserialize_hex, Psbt, Transaction};
use consts::CHAIN_SIGNATURES_PATH_V1;
use near_sdk::{json_types::U128, Timestamp};
use serde::{Deserialize, Serialize};
use types::{output_id, DepositEmbedMsg, PendingSignPsbt};
use withdraw::{verify_pending_sign_partial_sig, verify_sign_withdrawal_psbt, withdrawal_message};

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct ContractSummary {
    owner_id: AccountId,
    btc_light_client_id: AccountId,
    bip322_verifier_id: Option<AccountId>,
    chain_signatures_id: AccountId,
    chain_signatures_root_pubkey: Option<near_sdk::PublicKey>,
    n_confirmation: u64,
    withdrawal_waiting_time_ms: u64,
    min_deposit_satoshi: u64,
    earliest_deposit_block_height: u32,
    solo_withdrawal_sequence_heights: Vec<u16>,
    paused: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct GetV1DepositConstantsArgs {
    deposit_vout: u64,
    user_pubkey: String,
}

/// Constants for version 1 of the deposit script
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct DepositConstantsV1 {
    /// bithive pubkey used in the deposit script
    bithive_pubkey: String,
    /// message that needs to be embedded in the deposit transaction via OP_RETURN
    deposit_embed_msg: Option<String>,
    /// minimum deposit amount in satoshi
    min_deposit_satoshi: u64,
    /// earliest deposit block height
    earliest_deposit_block_height: u32,
    /// the current active value of sequence height for solo withdrawal
    solo_withdrawal_sequence_height: u16,
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct AccountView {
    pub pubkey: PubKey,
    /// total deposit amount in full BTC decimals
    pub total_deposit: u64,
    /// amount of deposits queued for withdrawal in full BTC decimals
    pub queue_withdrawal_amount: u64,
    /// timestamp when the queue withdrawal started in ms
    pub queue_withdrawal_start_ts: Timestamp,
    /// timestamp when the queue withdrawal ends in ms
    pub queue_withdrawal_end_ts: Timestamp,
    /// nonce is used in signing messages to prevent replay attacks
    pub nonce: u64,
    /// PSBT of the withdrawal txn that needs to be signed via chain signatures
    pub pending_sign_psbt: Option<PendingSignPsbt>,
    /// deposit user paid to cover the storage of pending sign PSBT
    /// this should only be increased when needed
    pub pending_sign_deposit: U128,
}

/// Constants for withdrawing v1 deposits
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct WithdrawalConstantsV1 {
    /// raw message that needs to be signed by the user for queueing withdrawal
    queue_withdrawal_msg: String,
}

/// Deposit info
#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct DepositInfo {
    deposit: Deposit,
    status: DepositStatus,
}

#[near_bindgen]
impl Contract {
    pub fn get_summary(&self) -> ContractSummary {
        ContractSummary {
            owner_id: self.owner_id.clone(),
            btc_light_client_id: self.btc_light_client_id.clone(),
            bip322_verifier_id: self.bip322_verifier_id.clone(),
            chain_signatures_id: self.chain_signatures_id.clone(),
            chain_signatures_root_pubkey: self.chain_signatures_root_pubkey.clone(),
            n_confirmation: self.n_confirmation,
            withdrawal_waiting_time_ms: self.withdrawal_waiting_time_ms,
            min_deposit_satoshi: self.min_deposit_satoshi,
            earliest_deposit_block_height: self.earliest_deposit_block_height,
            solo_withdrawal_sequence_heights: self.solo_withdrawal_seq_heights.clone(),
            paused: self.paused,
        }
    }

    /// Return constants that will be used by deposit scripts of version 1
    /// ### Arguments
    /// * `deposit_vout` - deposit vout index
    /// * `user_pubkey` - user pubkey
    /// * `sequence_height` - sequence height
    pub fn get_v1_deposit_constants(
        &self,
        args: Option<GetV1DepositConstantsArgs>,
    ) -> DepositConstantsV1 {
        // the first item is the current active one
        let sequence_height = self.solo_withdrawal_seq_heights[0];

        let embed_msg = args.map(|args| DepositEmbedMsg::V1 {
            deposit_vout: args.deposit_vout,
            user_pubkey: hex::decode(args.user_pubkey).unwrap().try_into().unwrap(),
            sequence_height,
        });

        DepositConstantsV1 {
            bithive_pubkey: self
                .generate_btc_pubkey(CHAIN_SIGNATURES_PATH_V1)
                .to_string(),
            deposit_embed_msg: embed_msg.map(|embed_msg| hex::encode(embed_msg.encode())),
            min_deposit_satoshi: self.min_deposit_satoshi,
            earliest_deposit_block_height: self.earliest_deposit_block_height,
            solo_withdrawal_sequence_height: sequence_height,
        }
    }

    /// Return constants that will be used for withdrawing v1 deposits
    /// ### Arguments
    /// * `user_pubkey` - user pubkey
    /// * `amount` - amount to withdraw
    pub fn get_v1_withdrawal_constants(
        &self,
        user_pubkey: String,
        amount: u64,
    ) -> WithdrawalConstantsV1 {
        let account = self.get_account(&user_pubkey.into());
        let msg = withdrawal_message(account.nonce, amount);
        WithdrawalConstantsV1 {
            queue_withdrawal_msg: msg,
        }
    }

    pub fn accounts_len(&self) -> u64 {
        self.accounts.len()
    }

    pub fn list_accounts(&self, offset: u64, limit: u64) -> Vec<AccountView> {
        let limit = min(limit, self.accounts_len() - offset);
        self.accounts
            .iter()
            .skip(offset as usize)
            .take(limit as usize)
            .map(|(_, account)| self.get_account_view(&account.into()))
            .collect()
    }

    pub fn view_account(&self, user_pubkey: String) -> AccountView {
        let account = self.get_account(&user_pubkey.into());
        self.get_account_view(&account)
    }

    pub fn user_active_deposits_len(&self, user_pubkey: String) -> u64 {
        let account = self.get_account(&user_pubkey.into());
        account.active_deposits_len()
    }

    pub fn list_user_active_deposits(
        &self,
        user_pubkey: String,
        offset: u64,
        limit: u64,
    ) -> Vec<Deposit> {
        let account = self.get_account(&user_pubkey.into());
        (offset..min(account.active_deposits_len(), offset + limit))
            .map(|idx| account.get_active_deposit_by_index(idx).unwrap())
            .collect()
    }

    pub fn user_withdrawn_deposits_len(&self, user_pubkey: String) -> u64 {
        let account = self.get_account(&user_pubkey.into());
        account.withdrawn_deposits_len()
    }

    pub fn list_user_withdrawn_deposits(
        &self,
        user_pubkey: String,
        offset: u64,
        limit: u64,
    ) -> Vec<Deposit> {
        let account = self.get_account(&user_pubkey.into());
        (offset..min(account.withdrawn_deposits_len(), offset + limit))
            .map(|idx| account.get_withdrawn_deposit_by_index(idx).unwrap())
            .collect()
    }

    pub fn get_deposit(&self, user_pubkey: String, tx_id: String, vout: u64) -> Option<Deposit> {
        let account = self.get_account(&user_pubkey.into());
        account
            .try_get_active_deposit(&tx_id.clone().into(), vout)
            .or_else(|| account.try_get_withdrawn_deposit(&tx_id.into(), vout))
    }

    /// Dry run deposit txn to verify if it can be accepted or not
    /// ### Arguments
    /// * `tx_hex` - hex encoded transaction
    /// * `embed_vout` - vout index of the embed output
    pub fn dry_run_deposit(&self, tx_hex: String, embed_vout: u64) {
        self.assert_running();
        let tx = deserialize_hex::<Transaction>(&tx_hex).unwrap();
        let output_id = output_id(&tx.compute_txid().to_string().into(), embed_vout);

        require!(
            !self.confirmed_deposit_txns.contains(&output_id),
            "deposit txn already confirmed"
        );
        self.verify_deposit_txn(&tx, embed_vout);
    }

    /// Dry run sign withdrawal txn to verify if it can be accepted or not
    /// ### Arguments
    /// * `psbt_hex` - hex encoded PSBT
    /// * `user_pubkey` - user pubkey
    /// * `vin_to_sign` - input index to sign
    /// * `reinvest_embed_vout` - vout index of the reinvest embed output
    pub fn dry_run_sign_withdrawal(
        &self,
        psbt_hex: String,
        user_pubkey: String,
        vin_to_sign: u64,
        reinvest_embed_vout: Option<u64>,
    ) {
        self.assert_running();
        let psbt_bytes = hex::decode(psbt_hex).unwrap();
        let psbt = Psbt::deserialize(&psbt_bytes).unwrap();

        let account = self.get_account(&user_pubkey.clone().into());

        let input_to_sign = psbt.unsigned_tx.input.get(vin_to_sign as usize).unwrap();
        account.get_active_deposit(
            &input_to_sign.previous_output.txid.to_string().into(),
            input_to_sign.previous_output.vout.into(),
        );

        if account.pending_sign_psbt.is_some() {
            verify_sign_withdrawal_psbt(account.pending_sign_psbt.as_ref().unwrap(), &psbt);
        } else {
            verify_pending_sign_partial_sig(&psbt, vin_to_sign, &user_pubkey);
            self.verify_pending_sign_request_amount(&account, &psbt, reinvest_embed_vout);
        }
    }
}

impl Contract {
    fn get_account_view(&self, account: &Account) -> AccountView {
        AccountView {
            pubkey: account.pubkey.clone(),
            total_deposit: account.total_deposit,
            queue_withdrawal_amount: account.queue_withdrawal_amount,
            queue_withdrawal_start_ts: account.queue_withdrawal_start_ts,
            queue_withdrawal_end_ts: if account.queue_withdrawal_start_ts == 0 {
                0
            } else {
                account.queue_withdrawal_start_ts + self.withdrawal_waiting_time_ms
            },
            nonce: account.nonce,
            pending_sign_psbt: account.pending_sign_psbt.clone(),
            pending_sign_deposit: account.pending_sign_deposit.into(),
        }
    }
}
