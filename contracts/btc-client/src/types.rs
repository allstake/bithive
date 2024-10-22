use std::fmt::Display;

use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    AccountId, BorshStorageKey,
};

#[derive(BorshSerialize, BorshStorageKey)]
pub enum StorageKey {
    ConfirmedDeposits,
    Accounts,
    ActiveDeposits(PubKey),
    WithdrawnDeposits(PubKey),
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct InitArgs {
    pub owner_id: AccountId,
    pub btc_lightclient_id: AccountId,
    pub chain_signature_id: AccountId,
    pub n_confirmation: u64,
    pub withdraw_waiting_time_ms: u64,
    pub min_deposit_satoshi: u64,
    pub earliest_deposit_block_height: u32,
    pub solo_withdraw_seq_heights: Vec<u16>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct SubmitDepositTxArgs {
    pub tx_hex: String,
    pub embed_vout: u64,
    pub tx_block_hash: String,
    pub tx_index: u64,
    pub merkle_proof: Vec<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct SubmitWithdrawTxArgs {
    pub tx_hex: String,
    pub user_pubkey: String,
    pub tx_block_hash: String,
    pub tx_index: u64,
    pub merkle_proof: Vec<String>,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, PartialEq, Debug)]
pub enum DepositEmbedMsg {
    V1 {
        deposit_vout: u64,
        user_pubkey: [u8; 33],
        sequence_height: u16,
    },
}

impl DepositEmbedMsg {
    const MAGIC_HEADER: &'static str = "bithive";

    pub fn encode(&self) -> Vec<u8> {
        let mut encoded = Self::MAGIC_HEADER.as_bytes().to_vec();
        encoded.extend(self.try_to_vec().unwrap());
        encoded
    }

    pub fn decode_hex(data: &str) -> Result<Self, String> {
        let raw_data = hex::decode(data).map_err(|e| format!("Failed to decode hex: {}", e))?;
        if raw_data.starts_with(Self::MAGIC_HEADER.as_bytes()) {
            let mut slice = &raw_data[Self::MAGIC_HEADER.len()..];
            Self::deserialize(&mut slice).map_err(|e| format!("Failed to deserialize: {}", e))
        } else {
            Err("Invalid magic header".to_string())
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embed_msg_encode_decode() {
        let pubkey =
            hex::decode("02f6b15f899fac9c7dc60dcac795291c70e50c3a2ee1d5070dee0d8020781584e5")
                .unwrap();
        let msg = DepositEmbedMsg::V1 {
            deposit_vout: 1,
            user_pubkey: pubkey.try_into().unwrap(),
            sequence_height: 1,
        };
        let encoded = msg.encode();
        let hex_encoded = hex::encode(encoded);
        let decoded = DepositEmbedMsg::decode_hex(&hex_encoded).unwrap();
        assert_eq!(msg, decoded);
    }
}
