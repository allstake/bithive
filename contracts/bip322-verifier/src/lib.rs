#[cfg(not(feature = "test"))]
use std::str::FromStr;

#[cfg(not(feature = "test"))]
use bip322_rs::bitcoin::consensus::deserialize;
#[cfg(not(feature = "test"))]
use bip322_rs::bitcoin::{Address, PublicKey, Witness};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::{near_bindgen, PanicOnDefault};

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {}

#[near_bindgen]
impl Contract {
    #[init]
    #[private]
    pub fn new() -> Self {
        Self {}
    }

    /// Verify a BIP322 full signature
    /// ### Arguments
    /// * `pubkey_hex` - hex encoded public key
    /// * `address` - BTC address (either P2TR or P2WPKH), derived from the public key
    /// * `message` - message to verify
    /// * `signature_hex` - hex encoded signature, should be the witness of to_sign txn
    #[cfg(not(feature = "test"))]
    pub fn verify_bip322_full(
        &self,
        pubkey_hex: String,
        address: String,
        message: String,
        signature_hex: String,
    ) -> bool {
        let pubkey = PublicKey::from_str(&pubkey_hex).unwrap();
        let address = Address::from_str(&address).unwrap().assume_checked();
        let msg = message.as_bytes();
        let signature = deserialize::<Witness>(&hex::decode(signature_hex).unwrap()).unwrap();
        bip322_rs::verify_full_witness(&pubkey, &address, msg, signature).is_ok()
    }

    /// This is to reduce the contract size in integration tests
    #[cfg(feature = "test")]
    #[allow(unused_variables)]
    pub fn verify_bip322_full(
        &self,
        pubkey_hex: String,
        address: String,
        message: String,
        signature_hex: String,
    ) -> bool {
        signature_hex.starts_with("00")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PUBKEY_HEX: &str = "02c7f12003196442943d8588e01aee840423cc54fc1521526a3b85c2b0cbd58872";
    const ADDRESS: &str = "bc1ppv609nr0vr25u07u95waq5lucwfm6tde4nydujnu8npg4q75mr5sxq8lt3";
    const MESSAGE: &str = "Hello World";
    const SIGNATURE: &str = "0141ddebd3eb25012ffa82937d9f25f9644e047bb2f472ab6c5089bbb53588ada2884cb5bcc53911f32d8dcf9548733b694d120db6a4e485194559e8d8fe668d269f01";

    fn get_contract() -> Contract {
        Contract::new()
    }

    #[test]
    fn test_verify_bip322_full() {
        let contract = get_contract();
        assert!(contract.verify_bip322_full(
            PUBKEY_HEX.to_string(),
            ADDRESS.to_string(),
            MESSAGE.to_string(),
            SIGNATURE.to_string()
        ));
    }

    #[test]
    fn test_verify_bip322_full_wrong_pubkey() {
        let contract = get_contract();
        let pubkey = "02999d8a64c41b29ba32790af1eb220adfb8cd038c758d0a2a59dcc3ec13bdac84";
        assert!(!contract.verify_bip322_full(
            pubkey.to_string(),
            ADDRESS.to_string(),
            MESSAGE.to_string(),
            SIGNATURE.to_string(),
        ));
    }

    #[test]
    fn test_verify_bip322_full_wrong_message() {
        let contract = get_contract();
        let message = "Hello world 1";
        assert!(!contract.verify_bip322_full(
            PUBKEY_HEX.to_string(),
            ADDRESS.to_string(),
            message.to_string(),
            SIGNATURE.to_string(),
        ));
    }
}
