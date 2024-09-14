use std::cmp::min;

use crate::*;
use account::Deposit;
use consts::{CHAIN_SIGNATURE_PATH_V1, DEPOSIT_MSG_HEX_V1};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct ContractSummary {
    owner_id: AccountId,
    btc_lightclient_id: AccountId,
    chain_signature_id: AccountId,
    chain_signature_root_pubkey: Option<near_sdk::PublicKey>,
    n_confirmation: u64,
    withdraw_waiting_time_ms: u64,
    min_deposit_satoshi: u64,
    solo_withdraw_sequence_heights: Vec<u16>,
}

/// Constants for version 1 of the deposit script
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct ConstantsV1 {
    /// allstake pubkey used in the deposit script
    allstake_pubkey: String,
    /// message that should be embedded in the deposit transaction
    deposit_embed_msg: String,
    /// raw message that needs to be signed by the user for queueing withdraw
    queue_withdrawl_msg: Option<String>,
}

#[near_bindgen]
impl Contract {
    pub fn get_summary(&self) -> ContractSummary {
        ContractSummary {
            owner_id: self.owner_id.clone(),
            btc_lightclient_id: self.btc_lightclient_id.clone(),
            chain_signature_id: self.chain_signature_id.clone(),
            chain_signature_root_pubkey: self.chain_signature_root_pubkey.clone(),
            n_confirmation: self.n_confirmation,
            withdraw_waiting_time_ms: self.withdraw_waiting_time_ms,
            min_deposit_satoshi: self.min_deposit_satoshi,
            solo_withdraw_sequence_heights: self.solo_withdraw_seq_heights.clone(),
        }
    }

    /// Return constants that will be used by deposit/withdraw scripts of version 1
    /// ### Arguments
    /// * `deposit_tx_id` - (needed for queue withdraw) deposit transaction id
    /// * `deposit_vout` - (needed for queue withdraw) deposit vout index
    pub fn get_v1_constants(
        &self,
        deposit_tx_id: Option<String>,
        deposit_vout: Option<u64>,
    ) -> ConstantsV1 {
        ConstantsV1 {
            allstake_pubkey: self
                .generate_btc_pubkey(CHAIN_SIGNATURE_PATH_V1)
                .to_string(),
            deposit_embed_msg: DEPOSIT_MSG_HEX_V1.to_string(),
            queue_withdrawl_msg: deposit_tx_id
                .map(|tx_id| self.withdrawal_message(&tx_id.into(), deposit_vout.unwrap())),
        }
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

    pub fn user_queue_withdrawal_deposits_len(&self, user_pubkey: String) -> u64 {
        let account = self.get_account(&user_pubkey.into());
        account.queue_withdrawal_deposits_len()
    }

    pub fn list_user_queue_withdrawal_deposits(
        &self,
        user_pubkey: String,
        offset: u64,
        limit: u64,
    ) -> Vec<Deposit> {
        let account = self.get_account(&user_pubkey.into());
        (offset..min(account.queue_withdrawal_deposits_len(), offset + limit))
            .map(|idx| account.get_queue_withdrawal_deposit_by_index(idx).unwrap())
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
}
