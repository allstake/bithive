use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    collections::UnorderedMap,
    require,
};

use crate::types::{output_id, OutputId, StorageKey};

const ERR_DEPOSIT_ALREADY_ACTIVE: &str = "Deposit already in active set";

#[derive(BorshDeserialize, BorshSerialize)]
pub struct Account {
    pubkey: String,
    /// set of deposit UTXOs that are not known to be withdrawed
    active_deposits: UnorderedMap<OutputId, Deposit>,
}

impl Account {
    pub fn new(pubkey: String) -> Account {
        Account {
            pubkey: pubkey.clone(),
            active_deposits: UnorderedMap::new(StorageKey::ActiveDeposits(pubkey)),
        }
    }

    pub fn pubkey(&self) -> String {
        self.pubkey.clone()
    }

    pub fn append_active_deposits(&mut self, deposit: &Deposit) {
        let deposit_id = deposit.id();
        require!(
            self.active_deposits.get(&deposit_id).is_none(),
            ERR_DEPOSIT_ALREADY_ACTIVE
        );
        self.active_deposits.insert(&deposit_id, deposit);
    }
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct Deposit {
    pub tx_id: String,
    pub vout: usize,
    pub value: u64,
}

impl Deposit {
    pub fn id(&self) -> OutputId {
        output_id(self.tx_id.clone(), self.vout)
    }
}
