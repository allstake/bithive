use near_sdk::{ext_contract, Gas};

pub const GAS_LIGHTCLIENT_VERIFY: Gas = Gas(30 * Gas::ONE_TERA.0);

#[ext_contract(ext_btc_lightclient)]
#[allow(dead_code)]
pub trait BtcLightClient {
    fn verify_transaction_inclusion(
        &self,
        tx_id: String,
        tx_block_blockhash: String,
        tx_index: u64,
        merkle_proof: Vec<String>,
        confirmations: u64,
    ) -> bool;
}
