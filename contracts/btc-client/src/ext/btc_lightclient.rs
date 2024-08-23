use near_sdk::ext_contract;

#[ext_contract(ext_btc_lightclient)]
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
