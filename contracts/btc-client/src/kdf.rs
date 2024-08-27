use crate::*;

use k256::{
    elliptic_curve::{
        bigint::ArrayEncoding,
        sec1::{FromEncodedPoint, ToEncodedPoint},
        CurveArithmetic, PrimeField,
    },
    AffinePoint, EncodedPoint, Scalar, Secp256k1, U256,
};
use sha3::{Digest, Sha3_256};
use utils::{compress_pub_key, current_account_id};

pub trait ScalarExt: Sized {
    fn from_bytes(bytes: [u8; 32]) -> Option<Self>;
    fn from_non_biased(bytes: [u8; 32]) -> Self;
}

impl ScalarExt for Scalar {
    /// Returns nothing if the bytes are greater than the field size of Secp256k1.
    /// This will be very rare with random bytes as the field size is 2^256 - 2^32 - 2^9 - 2^8 - 2^7 - 2^6 - 2^4 - 1
    fn from_bytes(bytes: [u8; 32]) -> Option<Self> {
        let bytes = U256::from_be_slice(bytes.as_slice());
        Scalar::from_repr(bytes.to_be_byte_array()).into_option()
    }

    /// When the user can't directly select the value, this will always work
    /// Use cases are things that we know have been hashed
    fn from_non_biased(hash: [u8; 32]) -> Self {
        // This should never happen.
        // The space of inputs is 2^256, the space of the field is ~2^256 - 2^32.
        // This mean that you'd have to run 2^224 hashes to find a value that causes this to fail.
        Scalar::from_bytes(hash).expect("Derived epsilon value falls outside of the field")
    }
}

type KdfPublicKey = <Secp256k1 as CurveArithmetic>::AffinePoint;

// near-mpc-recovery with key derivation protocol vX.Y.Z.
const EPSILON_DERIVATION_PREFIX: &str = "near-mpc-recovery v0.1.0 epsilon derivation:";

pub fn derive_epsilon(predecessor_id: &AccountId, path: &str) -> Scalar {
    let derivation_path = format!("{EPSILON_DERIVATION_PREFIX}{},{}", predecessor_id, path);
    let mut hasher = Sha3_256::new();
    hasher.update(derivation_path.clone());
    let hash: [u8; 32] = hasher.finalize().into();
    Scalar::from_non_biased(hash)
}

pub fn derive_key(public_key: KdfPublicKey, epsilon: Scalar) -> KdfPublicKey {
    (<Secp256k1 as CurveArithmetic>::ProjectivePoint::GENERATOR * epsilon + public_key).to_affine()
}

impl Contract {
    fn get_chain_signatures_root_public_key_bytes(&self) -> Vec<u8> {
        let mut root_public_key_bytes = vec![0x04];
        root_public_key_bytes.extend_from_slice(
            &self
                .chain_signature_root_pubkey
                .as_ref()
                .expect("Missing chain_signatures_root_public_key")
                .as_bytes()[1..],
        );
        root_public_key_bytes
    }

    fn generate_public_key(&self, path: &str) -> Vec<u8> {
        let mpc_point = EncodedPoint::from_bytes(self.get_chain_signatures_root_public_key_bytes())
            .expect("Invalid root public key bytes");
        let mpc_pk = AffinePoint::from_encoded_point(&mpc_point).unwrap();
        let account_id = current_account_id();
        let epsilon = derive_epsilon(&account_id, path);
        let user_pk = derive_key(mpc_pk, epsilon);
        let user_pk_encoded_point = user_pk.to_encoded_point(false);
        user_pk_encoded_point.as_bytes()[1..65].to_vec()
    }

    /// returns the derived BTC compressed public key for path
    pub fn generate_btc_pubkey(&self, path: &str) -> bitcoin::PublicKey {
        let public_key_bytes = self.generate_public_key(path);
        let compressed = compress_pub_key(&public_key_bytes[..].try_into().unwrap());
        bitcoin::PublicKey::from_slice(&compressed).expect("Invalid compressed pubkey bytes")
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::{tests::test_contract_instance, utils::compress_pub_key};

    #[test]
    fn test_generate_btc_public_key() {
        let contract = test_contract_instance();
        // current account id is `alice.near`
        let generated_btc_pk = contract.generate_btc_pubkey("/btc");

        // direct call chain signature
        let expected_pk = near_sdk::PublicKey::from_str(
            "secp256k1:39pFLm2p3eph57BFEBVGXmwzH14B5nJVbC1qBw9vkbSsafU9e69VNDQvYn2diAcKtwYBoTwJ6ZL4DYKZckPGEz8n"
        ).unwrap();
        let compressed_slice = compress_pub_key(&expected_pk.as_bytes()[1..].try_into().unwrap());
        let expected_pk = bitcoin::PublicKey::from_slice(&compressed_slice).unwrap();

        assert_eq!(generated_btc_pk, expected_pk);
    }

    #[test]
    fn test_generate_btc_public_key_wrong_path() {
        let contract = test_contract_instance();
        // current account id is `alice.near`
        let generated_btc_pk = contract.generate_btc_pubkey("/foo");

        // direct call chain signature
        let expected_pk = near_sdk::PublicKey::from_str(
            "secp256k1:39pFLm2p3eph57BFEBVGXmwzH14B5nJVbC1qBw9vkbSsafU9e69VNDQvYn2diAcKtwYBoTwJ6ZL4DYKZckPGEz8n"
        ).unwrap();
        let compressed_slice = compress_pub_key(&expected_pk.as_bytes()[1..].try_into().unwrap());
        let expected_pk = bitcoin::PublicKey::from_slice(&compressed_slice).unwrap();

        assert_ne!(generated_btc_pk, expected_pk);
    }
}
