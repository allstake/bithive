use std::cmp::min;

use crate::*;
use account::Deposit;
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

    pub fn user_queue_withdraw_deposits_len(&self, user_pubkey: String) -> u64 {
        let account = self.get_account(&user_pubkey.into());
        account.queue_withdraw_deposits_len()
    }

    pub fn list_user_queue_withdraw_deposits(
        &self,
        user_pubkey: String,
        offset: u64,
        limit: u64,
    ) -> Vec<Deposit> {
        let account = self.get_account(&user_pubkey.into());
        (offset..min(account.queue_withdraw_deposits_len(), offset + limit))
            .map(|idx| account.get_queue_withdraw_deposit_by_index(idx).unwrap())
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
