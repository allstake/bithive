use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::near_bindgen;

#[near_bindgen]
#[derive(BorshSerialize, BorshDeserialize, Default)]
struct Contract {}

#[near_bindgen]
impl Contract {
    #[allow(unused_variables, dead_code)]
    pub fn verify_transaction_inclusion(
        &self,
        tx_id: String,
        tx_block_blockhash: String,
        tx_index: u64,
        merkle_proof: Vec<String>,
        confirmations: u64,
    ) -> bool {
        // dummy return based on tx_index
        tx_index != 0
    }
}
