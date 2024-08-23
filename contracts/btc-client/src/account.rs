use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    collections::UnorderedMap,
    require, Timestamp,
};

use crate::{
    types::{output_id, OutputId, StorageKey},
    utils::current_timestamp_ms,
};

const ERR_DEPOSIT_ALREADY_ACTIVE: &str = "Deposit already in active set";
const ERR_DEPOSIT_NOT_ACTIVE: &str = "Deposit is not active";
const ERR_DEPOSIT_CANNOT_QUEUE_WITHDRAW: &str = "Cannot queue withdraw";
const ERR_DEPOSIT_ALREADY_QUEUED: &str = "Deposit already in queued set";
const ERR_DEPOSIT_NOT_IN_QUEUE: &str = "Deposit is not in queue";
const ERR_DEPOSIT_ALREADY_WITHDRAWN: &str = "Deposit already withdrawn";

#[derive(BorshDeserialize, BorshSerialize)]
pub struct Account {
    pubkey: String,
    /// set of deposits that are not known to be withdrawed
    active_deposits: UnorderedMap<OutputId, Deposit>,
    /// set of deposits that are queued for withdraw
    queue_withdraw_deposits: UnorderedMap<OutputId, Deposit>,
    /// set of deposits that are confirmed to have been withdrawn
    withdrawn_deposits: UnorderedMap<OutputId, Deposit>,
}

impl Account {
    pub fn new(pubkey: String) -> Account {
        Account {
            pubkey: pubkey.clone(),
            active_deposits: UnorderedMap::new(StorageKey::ActiveDeposits(pubkey.clone())),
            queue_withdraw_deposits: UnorderedMap::new(StorageKey::QueueWithdrawDeposits(
                pubkey.clone(),
            )),
            withdrawn_deposits: UnorderedMap::new(StorageKey::WithdrawnDeposits(pubkey)),
        }
    }

    pub fn pubkey(&self) -> String {
        self.pubkey.clone()
    }

    pub fn insert_active_deposit(&mut self, deposit: &Deposit) {
        let deposit_id = deposit.id();
        require!(
            !self.is_deposit_active(&deposit.deposit_tx_id, deposit.deposit_vout),
            ERR_DEPOSIT_ALREADY_ACTIVE
        );
        self.active_deposits.insert(&deposit_id, deposit);
    }

    pub fn is_deposit_active(&self, tx_id: &String, vout: u64) -> bool {
        let deposit_id = output_id(tx_id, vout);
        self.active_deposits.get(&deposit_id).is_some()
    }

    pub fn remove_active_deposit(&mut self, deposit: &Deposit) {
        self.active_deposits.remove(&deposit.id());
    }

    pub fn get_active_deposit(&self, tx_id: &String, vout: u64) -> Deposit {
        self.active_deposits
            .get(&output_id(tx_id, vout))
            .expect(ERR_DEPOSIT_NOT_ACTIVE)
    }

    pub fn insert_queue_withdraw_deposit(&mut self, deposit: &Deposit) {
        let deposit_id = &deposit.id();
        require!(
            self.queue_withdraw_deposits.get(deposit_id).is_none(),
            ERR_DEPOSIT_ALREADY_QUEUED
        );
        self.queue_withdraw_deposits.insert(deposit_id, deposit);
    }

    pub fn get_queue_withdraw_deposit(&self, tx_id: &String, vout: u64) -> Deposit {
        self.queue_withdraw_deposits
            .get(&output_id(tx_id, vout))
            .expect(ERR_DEPOSIT_NOT_IN_QUEUE)
    }

    pub fn remove_queue_withdraw_deposit(&mut self, deposit: &Deposit) {
        self.queue_withdraw_deposits.remove(&deposit.id());
    }

    pub fn insert_withdrawn_deposit(&mut self, deposit: &Deposit) {
        let deposit_id = &deposit.id();
        require!(
            self.withdrawn_deposits.get(deposit_id).is_none(),
            ERR_DEPOSIT_ALREADY_WITHDRAWN
        );
        self.withdrawn_deposits.insert(deposit_id, deposit);
    }
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct Deposit {
    deposit_tx_id: String,
    deposit_vout: u64,
    value: u64,
    /// queue withdraw start time in ms
    queue_withdraw_ts: Timestamp,
    /// complete withdraw time in ms
    complete_withdraw_ts: Timestamp,
    /// withdraw txn ID
    withdraw_tx_id: Option<String>,
}

impl Deposit {
    pub fn new(tx_id: String, vout: u64, value: u64) -> Deposit {
        Deposit {
            deposit_tx_id: tx_id,
            deposit_vout: vout,
            value,
            queue_withdraw_ts: 0,
            complete_withdraw_ts: 0,
            withdraw_tx_id: None,
        }
    }

    pub fn id(&self) -> OutputId {
        output_id(&self.deposit_tx_id, self.deposit_vout)
    }

    pub fn queue_withdraw(&mut self) {
        require!(
            self.queue_withdraw_ts == 0 && self.complete_withdraw_ts == 0,
            ERR_DEPOSIT_CANNOT_QUEUE_WITHDRAW
        );
        self.queue_withdraw_ts = current_timestamp_ms();
    }

    pub fn can_complete_withdraw(&self, waiting_time_ms: u64) -> bool {
        self.complete_withdraw_ts == 0
            && self.queue_withdraw_ts > 0
            && self.queue_withdraw_ts + waiting_time_ms <= current_timestamp_ms()
    }

    pub fn complete_withdraw(&mut self, withdraw_tx_id: String) {
        self.complete_withdraw_ts = current_timestamp_ms();
        self.withdraw_tx_id = Some(withdraw_tx_id);
    }
}
