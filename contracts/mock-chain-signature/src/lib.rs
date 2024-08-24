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
pub struct Contract {}

#[near_bindgen]
impl Contract {
    #[allow(unused_variables)]
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
        PromiseOrValue::Value(SignatureResponse {
            big_r: BigR {
                affine_point: "025802983164945D1C3E40818FF569E275451CC33613EDDFA0E54D23710DFAF3C8"
                    .to_string(),
            },
            s: S {
                scalar: "07511DF9E947BC61F88011A3166AA0E60E2D45BFCACD61AD35DB4340941C84DE"
                    .to_string(),
            },
            recovery_id: 1,
        })
    }
}
