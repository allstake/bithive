use std::fmt::Display;

use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    BorshStorageKey,
};

#[derive(BorshSerialize, BorshStorageKey)]
pub enum StorageKey {
    ConfirmedDeposits,
    Accounts,
    ActiveDeposits(PubKey),
    QueueWithdrawDeposits(PubKey),
    WithdrawnDeposits(PubKey),
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct SubmitDepositTxArgs {
    pub tx_hex: String,
    pub deposit_vout: u64,
    pub embed_vout: u64,
    pub user_pubkey_hex: String,
    pub sequence_height: u16,
    pub tx_block_hash: String,
    pub tx_index: u64,
    pub merkle_proof: Vec<String>,
}

/// Version of redeem script
#[derive(BorshSerialize, BorshDeserialize, serde::Serialize, serde::Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub enum RedeemVersion {
    V1,
}

/// public key (either compressed or not) in lower case
pub type PubKey = LowercaseString;

/// txn ID in lower case
pub type TxId = LowercaseString;

/// unique ID for an output of a transaction
pub type OutputId = LowercaseString;
pub fn output_id(tx_id: &TxId, vout: u64) -> LowercaseString {
    format!("{}:{}", tx_id, vout).into()
}

/// helper type which enforces lowercase strings
#[derive(serde::Serialize)]
pub struct LowercaseString(String);

impl LowercaseString {
    fn new(s: &str) -> Self {
        Self(s.to_lowercase())
    }
}

impl From<String> for LowercaseString {
    fn from(value: String) -> Self {
        LowercaseString::new(&value)
    }
}

impl From<LowercaseString> for String {
    fn from(value: LowercaseString) -> Self {
        value.0
    }
}

impl Display for LowercaseString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Clone for LowercaseString {
    fn clone(&self) -> Self {
        Self::new(&self.0)
    }
}

impl borsh::BorshDeserialize for LowercaseString {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
        String::deserialize(buf).map(|s| s.into())
    }
}

impl borsh::BorshSerialize for LowercaseString {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        String::serialize(&self.0, writer)
    }
}
