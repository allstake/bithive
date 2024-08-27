use near_sdk::{ext_contract, Promise};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct SignRequest {
    pub payload: [u8; 32],
    pub path: String,
    pub key_version: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct BigR {
    pub affine_point: String,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct S {
    pub scalar: String,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct SignatureResponse {
    pub big_r: BigR,
    pub s: S,
    pub recovery_id: u8,
}

#[ext_contract(ext_chain_signature)]
#[allow(dead_code)]
pub trait ChainSignature {
    fn sign(&mut self, request: SignRequest) -> Promise;

    /// returns the root public key
    fn public_key(&self) -> near_sdk::PublicKey;
}
