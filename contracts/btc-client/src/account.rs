use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    collections::UnorderedMap,
    require, Timestamp,
};
use serde::Serialize;

use crate::{
    events::Event,
    types::{output_id, OutputId, PubKey, RedeemVersion, StorageKey, TxId},
    utils::current_timestamp_ms,
};

const ERR_DEPOSIT_ALREADY_ACTIVE: &str = "Deposit already in active set";
const ERR_DEPOSIT_NOT_ACTIVE: &str = "Deposit is not active";
const ERR_DEPOSIT_ALREADY_WITHDRAWN: &str = "Deposit already withdrawn";

const ERR_INVALID_QUEUE_WITHDRAWAL: &str = "Invalid queue withdrawal amount";

#[derive(BorshDeserialize, BorshSerialize)]
pub struct Account {
    pub pubkey: PubKey,
    /// total deposit amount in full BTC decimals
    pub total_deposit: u64,
    /// set of deposits that are not known to be withdrawed
    active_deposits: UnorderedMap<OutputId, VersionedDeposit>,
    /// set of deposits that are confirmed to have been withdrawn
    withdrawn_deposits: UnorderedMap<OutputId, VersionedDeposit>,
    /// amount of deposits queued for withdrawl in full BTC decimals
    pub queue_withdrawal_amount: u64,
    /// timestamp when the queue withdrawal started in ms
    pub queue_withdrawal_start_ts: Timestamp,
    /// nonce is used in signing messages to prevent replay attacks
    pub nonce: u64,
    /// ID of the withdraw txn that needs to be signed via chain signature
    pub pending_withdraw_tx_id: Option<TxId>,
    /// number of unsigned inputs in the above txn
    pub pending_withdraw_unsigned_count: u16,
}

impl Account {
    pub fn new(pubkey: PubKey) -> Account {
        Account {
            pubkey: pubkey.clone(),
            total_deposit: 0,
            active_deposits: UnorderedMap::new(StorageKey::ActiveDeposits(pubkey.clone())),
            withdrawn_deposits: UnorderedMap::new(StorageKey::WithdrawnDeposits(pubkey)),
            queue_withdrawal_amount: 0,
            queue_withdrawal_start_ts: 0,
            nonce: 0,
            pending_withdraw_tx_id: None,
            pending_withdraw_unsigned_count: 0,
        }
    }

    pub fn active_deposits_len(&self) -> u64 {
        self.active_deposits.len()
    }

    pub fn get_active_deposit_by_index(&self, idx: u64) -> Option<Deposit> {
        self.active_deposits
            .values()
            .nth(idx as usize)
            .map(|d| d.into())
    }

    pub fn insert_active_deposit(&mut self, deposit: Deposit) {
        let deposit_id = deposit.id();
        require!(
            !self.is_deposit_active(&deposit.deposit_tx_id, deposit.deposit_vout),
            ERR_DEPOSIT_ALREADY_ACTIVE
        );
        self.active_deposits.insert(&deposit_id, &deposit.into());
    }

    pub fn is_deposit_active(&self, tx_id: &TxId, vout: u64) -> bool {
        let deposit_id = output_id(tx_id, vout);
        self.active_deposits.get(&deposit_id).is_some()
    }

    pub fn remove_active_deposit(&mut self, tx_id: &TxId, vout: u64) -> Deposit {
        self.active_deposits
            .remove(&output_id(tx_id, vout))
            .expect(ERR_DEPOSIT_NOT_ACTIVE)
            .into()
    }

    pub fn try_get_active_deposit(&self, tx_id: &TxId, vout: u64) -> Option<Deposit> {
        self.active_deposits
            .get(&output_id(tx_id, vout))
            .map(|d| d.into())
    }

    pub fn get_active_deposit(&self, tx_id: &TxId, vout: u64) -> Deposit {
        self.active_deposits
            .get(&output_id(tx_id, vout))
            .expect(ERR_DEPOSIT_NOT_ACTIVE)
            .into()
    }

    pub fn withdrawn_deposits_len(&self) -> u64 {
        self.withdrawn_deposits.len()
    }

    pub fn try_get_withdrawn_deposit(&self, tx_id: &TxId, vout: u64) -> Option<Deposit> {
        self.withdrawn_deposits
            .get(&output_id(tx_id, vout))
            .map(|d| d.into())
    }

    pub fn get_withdrawn_deposit_by_index(&self, idx: u64) -> Option<Deposit> {
        self.withdrawn_deposits
            .values()
            .nth(idx as usize)
            .map(|d| d.into())
    }

    fn insert_withdrawn_deposit(&mut self, deposit: Deposit) {
        let deposit_id = &deposit.id();
        require!(
            self.withdrawn_deposits.get(deposit_id).is_none(),
            ERR_DEPOSIT_ALREADY_WITHDRAWN
        );
        self.withdrawn_deposits.insert(deposit_id, &deposit.into());
    }

    pub fn create_deposit(&mut self, deposit: Deposit) {
        // make sure the deposit is not in withdrawn set
        require!(
            self.try_get_withdrawn_deposit(&deposit.deposit_tx_id, deposit.deposit_vout)
                .is_none(),
            ERR_DEPOSIT_ALREADY_WITHDRAWN
        );
        let value = deposit.value;
        let vout = deposit.deposit_vout;
        let tx_id = deposit.deposit_tx_id.clone();

        self.total_deposit += value;
        // this makes sure the deposit is not in active set
        self.insert_active_deposit(deposit);

        Event::Deposit {
            user_pubkey: &self.pubkey.clone().into(),
            tx_id: &tx_id.into(),
            deposit_vout: vout.into(),
            value: value.into(),
        }
        .emit();
    }

    pub fn queue_withdrawal(&mut self, amount: u64, msg: Vec<u8>, msg_sig: &String) {
        require!(
            self.queue_withdrawal_amount + amount <= self.total_deposit,
            ERR_INVALID_QUEUE_WITHDRAWAL
        );
        self.queue_withdrawal_amount += amount;
        self.queue_withdrawal_start_ts = current_timestamp_ms();
        self.nonce += 1;
        self.pending_withdraw_tx_id = None;
        self.pending_withdraw_unsigned_count = 0;

        Event::QueueWithdrawal {
            user_pubkey: &self.pubkey.clone().into(),
            amount: amount.into(),
            withdraw_msg: &hex::encode(msg),
            withdraw_sig: msg_sig,
        }
        .emit();
    }

    pub fn set_pending_withdraw_tx(&mut self, tx_id: TxId, unsigned_count: u16) {
        self.pending_withdraw_tx_id = Some(tx_id);
        self.pending_withdraw_unsigned_count = unsigned_count;
    }

    pub fn on_sign_withdrawal(&mut self) {
        self.pending_withdraw_unsigned_count -= 1;
        // if all signatures are collected, clear all withdraw related data
        if self.pending_withdraw_unsigned_count == 0 {
            self.queue_withdrawal_amount = 0;
            self.queue_withdrawal_start_ts = 0;
            self.pending_withdraw_tx_id = None;
        }
    }

    pub fn complete_withdrawal(&mut self, mut deposit: Deposit, tx_id: TxId) {
        let deposit_tx_id = deposit.deposit_tx_id.clone();
        let deposit_vout = deposit.deposit_vout;

        deposit.complete_withdraw(tx_id.clone());
        self.total_deposit -= deposit.value;
        self.remove_active_deposit(&deposit_tx_id, deposit_vout);
        self.insert_withdrawn_deposit(deposit);

        Event::CompleteWithdrawal {
            user_pubkey: &self.pubkey.clone().into(),
            withdrawal_tx_id: &tx_id.into(),
            deposit_tx_id: &deposit_tx_id.into(),
            deposit_vout: deposit_vout.into(),
        }
        .emit();
    }
}

#[derive(BorshDeserialize, BorshSerialize)]
pub enum VersionedAccount {
    Current(Account),
}

impl From<VersionedAccount> for Account {
    fn from(value: VersionedAccount) -> Self {
        match value {
            VersionedAccount::Current(a) => a,
        }
    }
}

impl From<Account> for VersionedAccount {
    fn from(value: Account) -> Self {
        VersionedAccount::Current(value)
    }
}

#[derive(Serialize, BorshDeserialize, BorshSerialize)]
#[serde(crate = "near_sdk::serde")]
pub enum DepositStatus {
    Active,
    Withdrawn,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Deposit {
    /// user pubkey
    pub user_pubkey: PubKey,
    /// deposit status
    pub status: DepositStatus,
    /// redeem version allows us to use the correct params to sign withdraw txn
    pub redeem_version: RedeemVersion,
    /// deposit transaction ID
    pub deposit_tx_id: TxId,
    /// deposit UTXO vout in the above transaction
    pub deposit_vout: u64,
    /// deposit amount in full BTC decimals
    pub value: u64,
    /// encoded sequence number of the deposit
    pub sequence: u32,
    /// complete withdraw time in ms
    pub complete_withdraw_ts: Timestamp,
    /// withdraw txn ID
    pub withdrawal_tx_id: Option<TxId>,
}

impl Deposit {
    pub fn new(
        user_pubkey: PubKey,
        redeem_version: RedeemVersion,
        tx_id: TxId,
        vout: u64,
        value: u64,
        sequence: u32,
    ) -> Deposit {
        Deposit {
            user_pubkey,
            status: DepositStatus::Active,
            redeem_version,
            deposit_tx_id: tx_id,
            deposit_vout: vout,
            value,
            sequence,
            complete_withdraw_ts: 0,
            withdrawal_tx_id: None,
        }
    }

    pub fn id(&self) -> OutputId {
        output_id(&self.deposit_tx_id, self.deposit_vout)
    }

    pub fn complete_withdraw(&mut self, withdrawal_tx_id: TxId) {
        self.complete_withdraw_ts = current_timestamp_ms();
        self.withdrawal_tx_id = Some(withdrawal_tx_id);
        self.status = DepositStatus::Withdrawn;
    }
}

#[derive(BorshDeserialize, BorshSerialize)]
pub enum VersionedDeposit {
    Current(Deposit),
}

impl From<VersionedDeposit> for Deposit {
    fn from(value: VersionedDeposit) -> Self {
        match value {
            VersionedDeposit::Current(d) => d,
        }
    }
}

impl From<Deposit> for VersionedDeposit {
    fn from(value: Deposit) -> Self {
        VersionedDeposit::Current(value)
    }
}
