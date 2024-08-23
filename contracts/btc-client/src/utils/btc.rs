use bitcoin::{Amount, TxOut};
use near_sdk::require;

const ERR_EMBED_NOT_ZERO: &str = "Embed output should have 0 value";
const ERR_EMBED_NOT_OPRETURN: &str = "Embed output is not OP_RETURN";

pub fn get_embed_message(output: &TxOut) -> String {
    require!(output.script_pubkey.is_op_return(), ERR_EMBED_NOT_OPRETURN);
    require!(output.value == Amount::ZERO, ERR_EMBED_NOT_ZERO);
    // first 2 bytes are OP codes, msg starts from the 3rd byte (4th in hex)
    output.script_pubkey.to_hex_string()[4..].to_string()
}
