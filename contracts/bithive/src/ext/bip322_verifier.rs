use near_sdk::ext_contract;

#[ext_contract(ext_bip322_verifier)]
#[allow(dead_code)]
pub trait Bip322Verifier {
    fn verify_bip322_full(
        &self,
        pubkey_hex: String,
        address: String,
        message: String,
        signature_hex: String,
    ) -> bool;
}
