use bitcoin::{sighash::SighashCache, Amount, Psbt, TxOut};
use near_sdk::require;

const ERR_EMBED_NOT_ZERO: &str = "Embed output should have 0 value";
const ERR_EMBED_NOT_OPRETURN: &str = "Embed output is not OP_RETURN";

pub fn get_embed_message(output: &TxOut) -> String {
    require!(output.script_pubkey.is_op_return(), ERR_EMBED_NOT_OPRETURN);
    require!(output.value == Amount::ZERO, ERR_EMBED_NOT_ZERO);
    // first 2 bytes are OP codes, msg starts from the 3rd byte (4th in hex)
    output.script_pubkey.to_hex_string()[4..].to_string()
}

pub fn get_hash_to_sign(psbt: &Psbt, vin: u64) -> [u8; 32] {
    let tx = psbt.unsigned_tx.clone();
    let mut cache = SighashCache::new(tx);
    let payload = psbt.sighash_ecdsa(vin as usize, &mut cache).unwrap();
    *payload.0.as_ref()
}

#[cfg(test)]
mod tests {
    use bitcoin::hex::{Case, DisplayHex};

    use super::*;

    #[test]
    fn test_get_hash_to_sign() {
        let psbt_hex = "70736274ff01007e0200000002253b73f1450d6be67a16e46d05f62235f1728d737d9540f12b69f84f4cc5b5950100000000ffffffff2b9507dc02d7a805b8f825f2d4e21b2e8f8b2ae8c9efd7292dcc86e471495a240100000000ffffffff01801a0600000000001976a914f6064f024b21637d7fc244081d7839dbc452d2fe88ac000000000001012be093040000000000220020a8761ded7be3f15c37ef6a84344a94479519218506e18fdb5596c16cd0b61b23010524752103d695ad0a1f72cdd70ca873f84c50cbb428c8f3a61bf6078c2693f3025751903eac0001012be093040000000000220020a8761ded7be3f15c37ef6a84344a94479519218506e18fdb5596c16cd0b61b23010524752103d695ad0a1f72cdd70ca873f84c50cbb428c8f3a61bf6078c2693f3025751903eac0000";
        let psbt_bytes = hex::decode(psbt_hex).unwrap();
        let psbt = Psbt::deserialize(&psbt_bytes).unwrap();

        let msg = get_hash_to_sign(&psbt, 0);
        assert_eq!(
            msg.to_hex_string(Case::Lower),
            "1c26c901749bc7a4ccc8e9c278d3062dab98f39c3eaf459d11640546e4ef345d",
        );
    }
}
