use bitcoin::{sighash::SighashCache, Amount, Psbt, TxOut};
use near_sdk::{env, require};

const ERR_EMBED_NOT_ZERO: &str = "Embed output should have 0 value";
const ERR_EMBED_NOT_OPRETURN: &str = "Embed output is not OP_RETURN";

const BITCOIN_SIGNED_MSG_PREFIX_UNISAT: &[u8] = b"Bitcoin Signed Message:\n";

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

/// verifies a BTC signed message produced by
/// browser wallet extensions including unisat, okx
/// https://github.com/unisat-wallet/wallet-sdk/blob/master/src/message/deterministic-ecdsa.ts#L51
///
/// ### Returns
/// (0: plain text message that is hashed an signed, 1: is_valid)
pub fn verify_signed_message_unisat(
    plain_msg: &[u8],
    sig: &[u8],
    pubkey: &[u8],
) -> (Vec<u8>, bool) {
    // build message to hash from plain_msg and prefix
    let mut msg_to_hash: Vec<u8> = vec![];
    msg_to_hash.push(BITCOIN_SIGNED_MSG_PREFIX_UNISAT.len() as u8);
    msg_to_hash.append(&mut BITCOIN_SIGNED_MSG_PREFIX_UNISAT.to_vec());
    let msg_len = plain_msg.len();
    if msg_len < 253 {
        msg_to_hash.push(msg_len as u8);
    } else if msg_len < 0x10000 {
        msg_to_hash.push(253);
        msg_to_hash.append(&mut (msg_len as u16).to_le_bytes().to_vec());
    } else if (msg_len as u64) < 0x100000000 {
        msg_to_hash.push(254);
        msg_to_hash.append(&mut (msg_len as u32).to_le_bytes().to_vec());
    } else {
        msg_to_hash.push(255);
        msg_to_hash.append(&mut (msg_len as u64).to_le_bytes().to_vec());
    }
    msg_to_hash.append(&mut plain_msg.to_vec());

    // double hash
    let msg_hash = env::sha256_array(&env::sha256_array(&msg_to_hash));

    // https://github.com/okx/js-wallet-sdk/blob/main/packages/coin-bitcoin/src/message.ts#L78
    let actual_sig = &sig[1..];
    let flag = sig[0] - 27;
    let v = flag & 3;

    (
        msg_to_hash,
        verify_secp256k1_signature(pubkey, msg_hash.as_ref(), actual_sig, v),
    )
}

fn verify_secp256k1_signature(public_key: &[u8], message: &[u8], signature: &[u8], v: u8) -> bool {
    let recovered_uncompressed_pk = env::ecrecover(message, signature, v, true)
        .unwrap()
        .to_vec();
    let compressed_pk = compress_pub_key(&recovered_uncompressed_pk);
    compressed_pk == *public_key
}

fn compress_pub_key(uncompressed_pub_key_bytes: &[u8]) -> Vec<u8> {
    // Extract the x and y coordinates
    let x_coord = &uncompressed_pub_key_bytes[0..32]; // First 32 bytes after the prefix
    let y_coord = &uncompressed_pub_key_bytes[32..64]; // Next 32 bytes

    // Determine the prefix for the compressed key
    let y_coord_is_even = y_coord[31] % 2 == 0;
    let prefix = if y_coord_is_even { 0x02 } else { 0x03 };

    // Create the compressed public key
    let mut compressed_pub_key = Vec::with_capacity(33);
    compressed_pub_key.push(prefix);
    compressed_pub_key.extend_from_slice(x_coord);

    compressed_pub_key
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

    #[test]
    fn test_verify_signed_message_unisat() {
        let plain_msg = "hello:02405803ac0c989534cdd54d5e1215e4149dc11aee83c21097571150c633dbc1cc";
        let pubkey = "02405803ac0c989534cdd54d5e1215e4149dc11aee83c21097571150c633dbc1cc";
        let sig = "1f579cd70d3a244ad1d774eb8ef300e17172f62bdb3b4090c296c98ce5c94b54a95a5ba68a70b60dc3bf4a32e851cfc300b87a5de6571ba8c7fff75b0b5cc4d3e3";
        let (msg, valid) = verify_signed_message_unisat(
            plain_msg.as_bytes(),
            &hex::decode(sig).unwrap(),
            &hex::decode(pubkey).unwrap(),
        );

        assert_eq!(
            hex::encode(msg),
            "18426974636f696e205369676e6564204d6573736167653a0a4868656c6c6f3a303234303538303361633063393839353334636464353464356531323135653431343964633131616565383363323130393735373131353063363333646263316363"
        );
        assert!(valid);
    }

    #[test]
    fn test_verify_signed_message_unisat_wrong_msg() {
        let plain_msg = "heiio:02405803ac0c989534cdd54d5e1215e4149dc11aee83c21097571150c633dbc1cc";
        let pubkey = "02405803ac0c989534cdd54d5e1215e4149dc11aee83c21097571150c633dbc1cc";
        let sig = "1f579cd70d3a244ad1d774eb8ef300e17172f62bdb3b4090c296c98ce5c94b54a95a5ba68a70b60dc3bf4a32e851cfc300b87a5de6571ba8c7fff75b0b5cc4d3e3";
        let (_, valid) = verify_signed_message_unisat(
            plain_msg.as_bytes(),
            &hex::decode(sig).unwrap(),
            &hex::decode(pubkey).unwrap(),
        );
        assert!(!valid);
    }

    #[test]
    fn test_verify_signed_message_unisat_wrong_pubkey() {
        let plain_msg = "hello:02405803ac0c989534cdd54d5e1215e4149dc11aee83c21097571150c633dbc1cc";
        let pubkey = "02405803ac0c989534cdd54d5e1215e4149dc11aee83c21097571150c633dbc1cc";
        // sig is from the same plain_msg but signed by a different key
        let sig = "20c6340b918107d565ef9ff80995289ca19366b7280ceae99f1c6ce38f9e0822b74240f315ddf981dd19f7f7d18ef1ca7959a8a6e0544d1efef8376b7a5fe394a4";
        let (_, valid) = verify_signed_message_unisat(
            plain_msg.as_bytes(),
            &hex::decode(sig).unwrap(),
            &hex::decode(pubkey).unwrap(),
        );

        assert!(!valid);
    }

    #[test]
    fn test_verify_signed_message_unisat_bad_sig() {
        let plain_msg = "hello:02405803ac0c989534cdd54d5e1215e4149dc11aee83c21097571150c633dbc1cc";
        let pubkey = "02405803ac0c989534cdd54d5e1215e4149dc11aee83c21097571150c633dbc1cc";
        let sig = "1f579cd70d3a244ad1d774eb8ef300e27172f62bdb3b4090c296c98ce5c94b54a95a5ba68a70b60dc3bf4a32e851cfc300b87a5de6571ba8c7fff75b0b5cc4d3e3";
        let (_, valid) = verify_signed_message_unisat(
            plain_msg.as_bytes(),
            &hex::decode(sig).unwrap(),
            &hex::decode(pubkey).unwrap(),
        );

        assert!(!valid);
    }
}
