use account::{Account, VersionedAccount};
use ext::ext_chain_signatures;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, LookupSet};
use near_sdk::{env, near_bindgen, require, AccountId, Gas, PanicOnDefault, Promise, PromiseError};
use types::{InitArgs, OutputId, PubKey, StorageKey};

mod account;
mod admin;
mod consts;
mod deposit;
mod events;
mod ext;
mod kdf;
mod legacy;
mod types;
mod upgrade;
mod utils;
mod view;
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
    btc_light_client_id: AccountId,
    /// bip322 verifier contract ID
    bip322_verifier_id: Option<AccountId>,
    /// chain signatures contract ID
    chain_signatures_id: AccountId,
    /// chain signatures root public key
    /// once set in contract initialization, this should not be changed
    /// otherwise we won't be able to sign previous txns
    chain_signatures_root_pubkey: Option<near_sdk::PublicKey>,
    /// number of confirmations in BTC
    n_confirmation: u64,
    /// for multisig withdrawal, how long the withdrawal request needs to be queued
    withdrawal_waiting_time_ms: u64,
    /// minimum deposit amount in satoshi
    min_deposit_satoshi: u64,
    /// earliest block height acceptable for deposit
    earliest_deposit_block_height: u32,
    /// list of available solo withdrawal sequence heights, used by redeem script
    solo_withdrawal_seq_heights: Vec<u16>,
    /// set of all confirmed deposit txns
    confirmed_deposit_txns: LookupSet<OutputId>,
    /// user accounts: pubkey -> account
    accounts: LookupMap<PubKey, VersionedAccount>,
    /// whether the contract is paused
    paused: bool,
}

#[near_bindgen]
impl Contract {
    #[init]
    #[private]
    pub fn init(args: InitArgs) -> Self {
        Self {
            owner_id: args.owner_id,
            btc_light_client_id: args.btc_light_client_id,
            bip322_verifier_id: args.bip322_verifier_id,
            chain_signatures_id: args.chain_signatures_id,
            chain_signatures_root_pubkey: None,
            n_confirmation: args.n_confirmation,
            withdrawal_waiting_time_ms: args.withdrawal_waiting_time_ms,
            min_deposit_satoshi: args.min_deposit_satoshi,
            earliest_deposit_block_height: args.earliest_deposit_block_height,
            solo_withdrawal_seq_heights: args.solo_withdrawal_seq_heights,
            confirmed_deposit_txns: LookupSet::new(StorageKey::ConfirmedDeposits),
            accounts: LookupMap::new(StorageKey::Accounts),
            paused: false,
        }
    }

    /// sync root public key from chain signatures
    /// this should be called right after the init function is called
    pub fn sync_chain_signatures_root_pubkey(&self) -> Promise {
        require!(
            self.chain_signatures_root_pubkey.is_none(),
            ERR_ROOT_PK_ALREADY_SYNCED
        );
        ext_chain_signatures::ext(self.chain_signatures_id.clone())
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
    pub(crate) fn set_chain_signature_root_pubkey(&mut self, pk: near_sdk::PublicKey) {
        require!(
            self.chain_signatures_root_pubkey.is_none(),
            ERR_ROOT_PK_ALREADY_SYNCED
        );
        self.chain_signatures_root_pubkey = Some(pk);
    }

    fn get_account(&self, pubkey: &PubKey) -> Account {
        self.accounts
            .get(pubkey)
            .unwrap_or_else(|| Account::new(pubkey.clone()).into())
            .into()
    }

    fn set_account(&mut self, account: Account) {
        self.accounts
            .insert(&account.pubkey.clone(), &account.into());
    }

    pub(crate) fn assert_running(&self) {
        require!(!self.paused, "Contract is paused");
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    pub(crate) fn test_contract_instance() -> Contract {
        let mut contract = Contract::init(InitArgs {
            owner_id: AccountId::new_unchecked("owner".to_string()),
            btc_light_client_id: AccountId::new_unchecked("lc".to_string()),
            bip322_verifier_id: Some(AccountId::new_unchecked("bv".to_string())),
            chain_signatures_id: AccountId::new_unchecked("cs".to_string()),
            n_confirmation: 6,
            withdrawal_waiting_time_ms: 0,
            min_deposit_satoshi: 0,
            earliest_deposit_block_height: 0,
            solo_withdrawal_seq_heights: vec![5],
        });

        // from v1.signer-prod.testnet
        let pk = "secp256k1:4NfTiv3UsGahebgTaHyD9vF8KYKMBnfd6kh94mK6xv8fGBiJB8TBtFMP5WWXz6B89Ac1fbpzPwAvoyQebemHFwx3";
        let root_pk = near_sdk::PublicKey::from_str(pk).unwrap();
        contract.set_chain_signature_root_pubkey(root_pk);

        contract
    }
}
