use account::{Account, VersionedAccount};
use ext::ext_chain_signature;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, LookupSet};
use near_sdk::{env, near_bindgen, require, AccountId, Gas, PanicOnDefault, Promise, PromiseError};
use types::{OutputId, PubKey, StorageKey};

mod account;
mod deposit;
mod events;
mod ext;
mod kdf;
mod types;
mod utils;
mod withdraw;

const ERR_ROOT_PK_ALREADY_SYNCED: &str = "Root pubkey already synced";
const ERR_FAILED_TO_SYNC_KEY: &str = "Failed to sync root pubkey from chain sig";

const GAS_GET_ROOT_PUBKEY: Gas = Gas(30 * Gas::ONE_TERA.0);
const GAS_GET_ROOT_PUBKEY_CB: Gas = Gas(10 * Gas::ONE_TERA.0);

#[near_bindgen]
#[derive(BorshSerialize, BorshDeserialize, PanicOnDefault)]
pub struct Contract {
    /// contract owner ID
    owner_id: AccountId,
    /// btc light client contract ID
    btc_lightclient_id: AccountId,
    /// chain signature contract ID
    chain_signature_id: AccountId,
    /// chain signature root public key
    /// once set in contract initialization, this should not be changed
    /// otherwise we won't be able to sign previous txns
    chain_signature_root_pubkey: Option<near_sdk::PublicKey>,
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
            chain_signature_root_pubkey: None,
            n_confirmation,
            withdraw_waiting_time_ms,
            confirmed_deposit_txns: LookupSet::new(StorageKey::ConfirmedDeposits),
            accounts: LookupMap::new(StorageKey::Accounts),
        }
    }

    /// sync root public key from chain signature
    /// this should be called right after the init function is called
    pub fn sync_chain_signature_root_pubkey(&self) -> Promise {
        require!(
            self.chain_signature_root_pubkey.is_none(),
            ERR_ROOT_PK_ALREADY_SYNCED
        );
        ext_chain_signature::ext(self.chain_signature_id.clone())
            .with_static_gas(GAS_GET_ROOT_PUBKEY)
            .public_key()
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_GET_ROOT_PUBKEY_CB)
                    .on_sync_root_pubkey(),
            )
    }

    #[private]
    pub fn on_sync_root_pubkey(
        &mut self,
        #[callback_result] result: Result<near_sdk::PublicKey, PromiseError>,
    ) -> near_sdk::PublicKey {
        let pk = result.expect(ERR_FAILED_TO_SYNC_KEY);
        self.set_chain_signature_root_pubkey(pk.clone());
        pk
    }
}

impl Contract {
    /// this could be called by tests but not exposed on-chain
    pub fn set_chain_signature_root_pubkey(&mut self, pk: near_sdk::PublicKey) {
        require!(
            self.chain_signature_root_pubkey.is_none(),
            ERR_ROOT_PK_ALREADY_SYNCED
        );
        self.chain_signature_root_pubkey = Some(pk);
    }

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

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    pub(crate) fn test_contract_instance() -> Contract {
        let mut contract = Contract::init(
            AccountId::new_unchecked("owner".to_string()),
            AccountId::new_unchecked("lc".to_string()),
            AccountId::new_unchecked("cs".to_string()),
            6,
            0,
        );

        // from v1.signer-prod.testnet
        let pk = "secp256k1:4NfTiv3UsGahebgTaHyD9vF8KYKMBnfd6kh94mK6xv8fGBiJB8TBtFMP5WWXz6B89Ac1fbpzPwAvoyQebemHFwx3";
        let root_pk = near_sdk::PublicKey::from_str(pk).unwrap();
        contract.set_chain_signature_root_pubkey(root_pk);

        contract
    }

    pub(crate) fn user_pubkey_hex() -> String {
        "02f6b15f899fac9c7dc60dcac795291c70e50c3a2ee1d5070dee0d8020781584e5".to_string()
    }
}
