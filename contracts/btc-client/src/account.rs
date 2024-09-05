use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    collections::UnorderedMap,
    require, Timestamp,
};
use serde::Serialize;

use crate::{
    types::{output_id, OutputId, PubKey, RedeemVersion, StorageKey, TxId},
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
    pubkey: PubKey,
    /// set of deposits that are not known to be withdrawed
    active_deposits: UnorderedMap<OutputId, VersionedDeposit>,
    /// set of deposits that are queued for withdraw
    queue_withdrawal_deposits: UnorderedMap<OutputId, VersionedDeposit>,
    /// set of deposits that are confirmed to have been withdrawn
    withdrawn_deposits: UnorderedMap<OutputId, VersionedDeposit>,
}

impl Account {
    pub fn new(pubkey: PubKey) -> Account {
        Account {
            pubkey: pubkey.clone(),
            active_deposits: UnorderedMap::new(StorageKey::ActiveDeposits(pubkey.clone())),
            queue_withdrawal_deposits: UnorderedMap::new(StorageKey::QueueWithdrawDeposits(
                pubkey.clone(),
            )),
            withdrawn_deposits: UnorderedMap::new(StorageKey::WithdrawnDeposits(pubkey)),
        }
    }

    pub fn pubkey(&self) -> PubKey {
        self.pubkey.clone()
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

    pub fn get_active_deposit(&self, tx_id: &TxId, vout: u64) -> Deposit {
        self.active_deposits
            .get(&output_id(tx_id, vout))
            .expect(ERR_DEPOSIT_NOT_ACTIVE)
            .into()
    }

    pub fn queue_withdrawal_deposits_len(&self) -> u64 {
        self.queue_withdrawal_deposits.len()
    }

    pub fn get_queue_withdrawal_deposit_by_index(&self, idx: u64) -> Option<Deposit> {
        self.queue_withdrawal_deposits
            .values()
            .nth(idx as usize)
            .map(|d| d.into())
    }

    pub fn insert_queue_withdrawal_deposit(&mut self, deposit: Deposit) {
        let deposit_id = &deposit.id();
        require!(
            self.queue_withdrawal_deposits.get(deposit_id).is_none(),
            ERR_DEPOSIT_ALREADY_QUEUED
        );
        self.queue_withdrawal_deposits
            .insert(deposit_id, &deposit.into());
    }

    pub fn try_get_queue_withdrawal_deposit(&self, tx_id: &TxId, vout: u64) -> Option<Deposit> {
        self.queue_withdrawal_deposits
            .get(&output_id(tx_id, vout))
            .map(|d| d.into())
    }

    pub fn get_queue_withdrawal_deposit(&self, tx_id: &TxId, vout: u64) -> Deposit {
        self.try_get_queue_withdrawal_deposit(tx_id, vout)
            .expect(ERR_DEPOSIT_NOT_IN_QUEUE)
    }

    pub fn try_remove_queue_withdrawal_deposit(
        &mut self,
        tx_id: &TxId,
        vout: u64,
    ) -> Option<Deposit> {
        self.queue_withdrawal_deposits
            .remove(&output_id(tx_id, vout))
            .map(|d| d.into())
    }

    pub fn remove_queue_withdrawal_deposit(&mut self, tx_id: &TxId, vout: u64) -> Deposit {
        self.try_remove_queue_withdrawal_deposit(tx_id, vout)
            .expect(ERR_DEPOSIT_NOT_IN_QUEUE)
    }

    pub fn withdrawn_deposits_len(&self) -> u64 {
        self.withdrawn_deposits.len()
    }

    pub fn get_withdrawn_deposit_by_index(&self, idx: u64) -> Option<Deposit> {
        self.withdrawn_deposits
            .values()
            .nth(idx as usize)
            .map(|d| d.into())
    }

    pub fn insert_withdrawn_deposit(&mut self, deposit: Deposit) {
        let deposit_id = &deposit.id();
        require!(
            self.withdrawn_deposits.get(deposit_id).is_none(),
            ERR_DEPOSIT_ALREADY_WITHDRAWN
        );
        self.withdrawn_deposits.insert(deposit_id, &deposit.into());
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

#[derive(BorshDeserialize, BorshSerialize, Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Deposit {
    /// redeem version allows us to use the correct params to sign withdraw txn
    redeem_version: RedeemVersion,
    /// deposit transaction ID
    deposit_tx_id: TxId,
    /// deposit UTXO vout in the above transaction
    deposit_vout: u64,
    /// deposit amount in full BTC decimals
    value: u64,
    /// queue withdraw start time in ms
    queue_withdraw_ts: Timestamp,
    /// the message that the user signed when requesting queue withdraw
    queue_withdraw_message: Option<String>,
    /// signature of the above message.
    /// withdraw msg and sig are saved to make the action indisputable
    queue_withdraw_sig: Option<String>,
    /// complete withdraw time in ms
    complete_withdraw_ts: Timestamp,
    /// withdraw txn ID
    withdrawal_tx_id: Option<TxId>,
}

impl Deposit {
    pub fn new(redeem_version: RedeemVersion, tx_id: TxId, vout: u64, value: u64) -> Deposit {
        Deposit {
            redeem_version,
            deposit_tx_id: tx_id,
            deposit_vout: vout,
            value,
            queue_withdraw_ts: 0,
            queue_withdraw_message: None,
            queue_withdraw_sig: None,
            complete_withdraw_ts: 0,
            withdrawal_tx_id: None,
        }
    }

    pub fn redeem_version(&self) -> RedeemVersion {
        self.redeem_version.clone()
    }

    pub fn id(&self) -> OutputId {
        output_id(&self.deposit_tx_id, self.deposit_vout)
    }

    pub fn queue_withdrawal(&mut self, withdraw_msg: String, msg_sig: String) {
        require!(
            self.queue_withdraw_ts == 0 && self.complete_withdraw_ts == 0,
            ERR_DEPOSIT_CANNOT_QUEUE_WITHDRAW
        );
        self.queue_withdraw_ts = current_timestamp_ms();
        self.queue_withdraw_message = Some(withdraw_msg);
        self.queue_withdraw_sig = Some(msg_sig);
    }

    pub fn can_complete_withdraw(&self, waiting_time_ms: u64) -> bool {
        self.complete_withdraw_ts == 0
            && (self.queue_withdraw_ts == 0
                || self.queue_withdraw_ts + waiting_time_ms <= current_timestamp_ms())
    }

    pub fn complete_withdraw(&mut self, withdrawal_tx_id: String) {
        self.complete_withdraw_ts = current_timestamp_ms();
        self.withdrawal_tx_id = Some(withdrawal_tx_id.into());
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
