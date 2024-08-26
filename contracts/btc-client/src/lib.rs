use account::{Account, VersionedAccount};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, LookupSet};
use near_sdk::{near_bindgen, AccountId, PanicOnDefault};
use types::{OutputId, PubKey, StorageKey};

mod account;
mod deposit;
mod events;
mod ext;
mod types;
mod utils;
mod withdraw;

#[near_bindgen]
#[derive(BorshSerialize, BorshDeserialize, PanicOnDefault)]
pub struct Contract {
    /// contract owner ID
    owner_id: AccountId,
    /// btc light client contract ID
    btc_lightclient_id: AccountId,
    /// chain signature contract ID
    chain_signature_id: AccountId,
    /// number of confirmations in BTC
    n_confirmation: u64,
    /// how long the withdraw request needs to be queued
    withdraw_waiting_time_ms: u64,
    /// set of all confirmed deposit txns
    confirmed_deposit_txns: LookupSet<OutputId>,
    /// user accounts: pubkey -> account
    accounts: LookupMap<PubKey, VersionedAccount>,
}

#[near_bindgen]
impl Contract {
    #[init]
    #[private]
    pub fn init(
        owner_id: AccountId,
        btc_lightclient_id: AccountId,
        chain_signature_id: AccountId,
        n_confirmation: u64,
        withdraw_waiting_time_ms: u64,
    ) -> Self {
        Self {
            owner_id,
            btc_lightclient_id,
            chain_signature_id,
            n_confirmation,
            withdraw_waiting_time_ms,
            confirmed_deposit_txns: LookupSet::new(StorageKey::ConfirmedDeposits),
            accounts: LookupMap::new(StorageKey::Accounts),
        }
    }
}

impl Contract {
    fn get_account(&self, pubkey: &PubKey) -> Account {
        self.accounts
            .get(pubkey)
            .unwrap_or_else(|| Account::new(pubkey.clone()).into())
            .into()
    }

    fn set_account(&mut self, account: Account) {
        self.accounts.insert(&account.pubkey(), &account.into());
    }
}
