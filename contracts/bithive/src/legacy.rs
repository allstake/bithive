use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::{
    collections::{LookupMap, LookupSet},
    AccountId,
};

use crate::{
    account::VersionedAccount,
    types::{OutputId, PubKey},
};

#[derive(BorshSerialize, BorshDeserialize)]
pub struct ContractV1 {
    /// contract owner ID
    pub owner_id: AccountId,
    /// btc light client contract ID
    pub btc_light_client_id: AccountId,
    /// bip322 verifier contract ID
    pub bip322_verifier_id: Option<AccountId>,
    /// chain signatures contract ID
    pub chain_signatures_id: AccountId,
    /// chain signatures root public key
    /// once set in contract initialization, this should not be changed
    /// otherwise we won't be able to sign previous txns
    pub chain_signatures_root_pubkey: Option<near_sdk::PublicKey>,
    /// number of confirmations in BTC
    pub n_confirmation: u64,
    /// for multisig withdrawal, how long the withdrawal request needs to be queued
    pub withdrawal_waiting_time_ms: u64,
    /// minimum deposit amount in satoshi
    pub min_deposit_satoshi: u64,
    /// earliest block height acceptable for deposit
    pub earliest_deposit_block_height: u32,
    /// list of available solo withdrawal sequence heights, used by redeem script
    pub solo_withdrawal_seq_heights: Vec<u16>,
    /// set of all confirmed deposit txns
    pub confirmed_deposit_txns: LookupSet<OutputId>,
    /// user accounts: pubkey -> account
    pub accounts: LookupMap<PubKey, VersionedAccount>,
    /// whether the contract is paused
    pub paused: bool,
}
