use std::{fmt, str::FromStr};

use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    ext_contract, Gas,
};
use serde::{
    de::{self, Visitor},
    Deserialize, Serialize,
};

pub const GAS_LIGHT_CLIENT_VERIFY: Gas = Gas(30 * Gas::ONE_TERA.0);

#[ext_contract(ext_btc_light_client)]
#[allow(dead_code)]
pub trait BtcLightClient {
    fn verify_transaction_inclusion(&self, #[serializer(borsh)] args: ProofArgs) -> bool;
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct ProofArgs {
    pub tx_id: H256,
    pub tx_block_blockhash: H256,
    pub tx_index: u64,
    pub merkle_proof: Vec<H256>,
    pub confirmations: u64,
}

impl ProofArgs {
    pub fn new(
        tx_id: String,
        tx_block_blockhash: String,
        tx_index: u64,
        merkle_proof: Vec<String>,
        confirmations: u64,
    ) -> Self {
        ProofArgs {
            tx_id: tx_id.parse().expect("Invalid tx_id"),
            tx_block_blockhash: tx_block_blockhash
                .parse()
                .expect("Invalid tx_block_blockhash"),
            tx_index,
            merkle_proof: merkle_proof
                .into_iter()
                .map(|v| {
                    v.parse()
                        .unwrap_or_else(|_| panic!("Invalid merkle_proof: {:?}", v))
                })
                .collect(),
            confirmations,
        }
    }
}

#[derive(
    BorshDeserialize, BorshSerialize, Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Default,
)]
pub struct H256(pub [u8; 32]);

impl From<[u8; 32]> for H256 {
    fn from(bytes: [u8; 32]) -> Self {
        H256(bytes)
    }
}

impl fmt::Display for H256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let reversed: Vec<u8> = self.0.into_iter().rev().collect();
        write!(f, "{}", hex::encode(reversed))
    }
}

impl TryFrom<Vec<u8>> for H256 {
    type Error = &'static str;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        Ok(H256(value.try_into().map_err(|_| "Invalid hex length")?))
    }
}

impl FromStr for H256 {
    type Err = hex::FromHexError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut result = [0; 32];
        hex::decode_to_slice(s, &mut result)?;
        result.reverse();
        Ok(H256(result))
    }
}

impl<'de> Deserialize<'de> for H256 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct HexVisitor;

        impl<'de> Visitor<'de> for HexVisitor {
            type Value = H256;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a hex string")
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let mut result = [0; 32];
                hex::decode_to_slice(s, &mut result).map_err(de::Error::custom)?;
                result.reverse();
                Ok(H256(result))
            }
        }

        deserializer.deserialize_str(HexVisitor)
    }
}

impl Serialize for H256 {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> Result<<S as serde::Serializer>::Ok, <S as serde::Serializer>::Error>
    where
        S: serde::Serializer,
    {
        let reversed: Vec<u8> = self.0.into_iter().rev().collect();
        serializer.serialize_str(&hex::encode(reversed))
    }
}
