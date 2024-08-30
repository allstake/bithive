use std::str::FromStr;

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, near_bindgen, Gas, Promise, PromiseOrValue};

#[derive(Serialize, Deserialize)]
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

const GAS_FOR_SIGN_CALL: Gas = Gas(250 * Gas::ONE_TERA.0);

#[near_bindgen]
#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct Contract {
    big_r: String,
    s: String,
    recovery_id: u8,
}

#[near_bindgen]
impl Contract {
    #[init]
    pub fn init() -> Self {
        Self {
            big_r: "02E14D22E30DF1F02A3C46C52EB2B999AB009600FA945CACD3242AD66480E26EA7".to_string(),
            s: "7E7ADD7EF49E871C41EDF56BDF5C93B44E21A83CD55FA656318A1F0E6CD17CE9".to_string(),
            recovery_id: 0,
        }
    }

    pub fn set_sig(&mut self, big_r: String, s: String, recovery_id: u8) {
        self.big_r = big_r;
        self.s = s;
        self.recovery_id = recovery_id;
    }

    pub fn public_key(&self) -> near_sdk::PublicKey {
        // from v1.signer-prod.testnet
        let pk = "secp256k1:4NfTiv3UsGahebgTaHyD9vF8KYKMBnfd6kh94mK6xv8fGBiJB8TBtFMP5WWXz6B89Ac1fbpzPwAvoyQebemHFwx3";
        near_sdk::PublicKey::from_str(pk).unwrap()
    }

    #[allow(unused_variables)]
    #[payable]
    pub fn sign(&mut self, request: SignRequest) -> Promise {
        assert!(
            env::prepaid_gas() >= GAS_FOR_SIGN_CALL,
            "Insufficient gas provided. Provided: {:?} Required: {:?}",
            env::prepaid_gas(),
            GAS_FOR_SIGN_CALL
        );
        Self::ext(env::current_account_id()).sign_helper(request.payload, 0)
    }

    #[allow(unused_variables)]
    #[private]
    pub fn sign_helper(
        &mut self,
        payload: [u8; 32],
        depth: usize,
    ) -> PromiseOrValue<SignatureResponse> {
        // log!("payload to sign:");
        // log!("{:?}", payload);

        PromiseOrValue::Value(SignatureResponse {
            big_r: BigR {
                affine_point: self.big_r.clone(),
            },
            s: S {
                scalar: self.s.clone(),
            },
            recovery_id: self.recovery_id,
        })
    }

    pub fn pubkey_helper(&self, pubkey: near_sdk::PublicKey) -> String {
        hex::encode(pubkey.as_bytes())
    }
}
