use account::Account;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, LookupSet};
use near_sdk::{near_bindgen, AccountId, PanicOnDefault};
use types::{OutputId, StorageKey};

mod account;
mod deposit;
mod ext;
mod types;

#[near_bindgen]
#[derive(BorshSerialize, BorshDeserialize, PanicOnDefault)]
pub struct Contract {
    /// contract owner ID
    owner_id: AccountId,
    /// btc light client contract ID
    btc_lightclient_id: AccountId,
    /// number of confirmations in BTC
    n_confirmation: u64,
    /// set of all confirmed deposit txns
    confirmed_deposit_txns: LookupSet<OutputId>,
    /// user accounts: pubkey -> account
    accounts: LookupMap<String, Account>,
}

#[near_bindgen]
impl Contract {
    #[init]
    #[private]
    pub fn init(owner_id: AccountId, btc_lightclient_id: AccountId, n_confirmation: u64) -> Self {
        Self {
            owner_id,
            btc_lightclient_id,
            n_confirmation,
            confirmed_deposit_txns: LookupSet::new(StorageKey::ConfirmedDeposits),
            accounts: LookupMap::new(StorageKey::Accounts),
        }
    }
}

impl Contract {
    fn get_account(&self, pubkey: &String) -> Account {
        self.accounts
            .get(pubkey)
            .unwrap_or_else(|| Account::new(pubkey.to_string()))
    }

    fn set_account(&mut self, account: &Account) {
        self.accounts.insert(&account.pubkey(), account);
    }
}
