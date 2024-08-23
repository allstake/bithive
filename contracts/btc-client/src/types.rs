use near_sdk::{
    borsh::{self, BorshSerialize},
    BorshStorageKey,
};

#[derive(BorshSerialize, BorshStorageKey)]
pub(crate) enum StorageKey {
    ConfirmedDeposits,
    Accounts,
    ActiveDeposits(String),
}

/// unique ID for an output of a transaction
pub(crate) type OutputId = String;
pub(crate) fn output_id(tx_id: String, vout: usize) -> String {
    format!("{}:{}", tx_id, vout)
}
