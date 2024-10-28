use std::cmp::min;

use crate::*;
use account::{Deposit, DepositStatus};
use consts::CHAIN_SIGNATURE_PATH_V1;
use serde::{Deserialize, Serialize};
use types::DepositEmbedMsg;

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
    earliest_deposit_block_height: u32,
    solo_withdraw_sequence_heights: Vec<u16>,
}

/// Constants for version 1 of the deposit script
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct DepositConstantsV1 {
    /// allstake pubkey used in the deposit script
    allstake_pubkey: String,
    /// message that needs to be embedded in the deposit transaction via OP_RETURN
    deposit_embed_msg: String,
}

/// Constants for withdrawing v1 deposits
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct WithdrawalConstantsV1 {
    /// raw message that needs to be signed by the user for queueing withdraw
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
            btc_lightclient_id: self.btc_lightclient_id.clone(),
            chain_signature_id: self.chain_signature_id.clone(),
            chain_signature_root_pubkey: self.chain_signature_root_pubkey.clone(),
            n_confirmation: self.n_confirmation,
            withdraw_waiting_time_ms: self.withdraw_waiting_time_ms,
            min_deposit_satoshi: self.min_deposit_satoshi,
            earliest_deposit_block_height: self.earliest_deposit_block_height,
            solo_withdraw_sequence_heights: self.solo_withdraw_seq_heights.clone(),
        }
    }

    /// Return constants that will be used by deposit scripts of version 1
    /// ### Arguments
    /// * `deposit_vout` - deposit vout index
    /// * `user_pubkey` - user pubkey
    /// * `sequence_height` - sequence height
    pub fn get_v1_deposit_constants(
        &self,
        deposit_vout: u64,
        user_pubkey: String,
        sequence_height: u16,
    ) -> DepositConstantsV1 {
        let embed_msg = DepositEmbedMsg::V1 {
            deposit_vout,
            user_pubkey: hex::decode(user_pubkey).unwrap().try_into().unwrap(),
            sequence_height,
        };

        DepositConstantsV1 {
            allstake_pubkey: self
                .generate_btc_pubkey(CHAIN_SIGNATURE_PATH_V1)
                .to_string(),
            deposit_embed_msg: hex::encode(embed_msg.encode()),
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
        let msg = self.withdrawal_message(account.nonce, amount);
        WithdrawalConstantsV1 {
            queue_withdrawal_msg: msg,
        }
    }

    pub fn view_account(&self, user_pubkey: String) -> Account {
        self.get_account(&user_pubkey.into())
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
}
