use std::str::FromStr;

use account::Deposit;
use bitcoin::{
    absolute::LockTime,
    consensus::encode::deserialize_hex,
    opcodes::all::{
        OP_CHECKMULTISIG, OP_CHECKSIG, OP_CSV, OP_DROP, OP_ELSE, OP_ENDIF, OP_IF, OP_PUSHNUM_2,
    },
    script::Builder,
    PublicKey, Sequence, Transaction, TxOut,
};
use events::Event;
use ext::{ext_btc_lightclient, ProofArgs, GAS_LIGHTCLIENT_VERIFY};
use near_sdk::{env, near_bindgen, require, Gas, Promise, PromiseError, PromiseOrValue};
use types::{output_id, TxId};
use utils::{assert_gas, get_embed_message};

use crate::*;

const DEPOSIT_V1_MSG_HEX: &str = "616c6c7374616b652e6465706f7369742e7631"; // "allstake.deposit.v1"

const ERR_INVALID_TX_HEX: &str = "Invalid hex transaction";
const ERR_BAD_TX_LOCKTIME: &str = "Invalid transaction locktime";
const ERR_BAD_PUBKER_HEX: &str = "Invalid pubkey hex";

const ERR_BAD_DEPOSIT_IDX: &str = "Bad deposit output index";
const ERR_BAD_EMBED_IDX: &str = "Bad embed output index";

const ERR_EMBED_INVALID_MSG: &str = "Invalid embed output msg";

const ERR_DEPOSIT_NOT_P2WSH: &str = "Deposit output is not P2WSH";
const ERR_DEPOSIT_BAD_SCRIPT_HASH: &str = "Deposit output bad script hash";

const ERR_DEPOSIT_ALREADY_SAVED: &str = "Deposit already saved";

const GAS_DEPOSIT_VERIFY_CB: Gas = Gas(30 * Gas::ONE_TERA.0);

#[near_bindgen]
impl Contract {
    /// Submit a BTC deposit transaction
    /// ### Arguments
    /// * `tx_hex` - hex encoded transaction body
    /// * `deposit_vout` - index of deposit (p2wsh) output
    /// * `embed_vout` - index of embed (OP_RETURN) output
    /// * `user_pubkey_hex` - user pubkey hex encoded
    /// * `sequence_height` - sequence in height // TODO from config?
    /// * `tx_block_hash` - block hash in which the transaction is included
    /// * `tx_index` - transaction index in the block
    /// * `merkle_proof` - merkle proof of transaction in the block
    #[payable]
    pub fn submit_deposit_tx(
        &mut self,
        tx_hex: String,
        deposit_vout: u64,
        embed_vout: u64,
        user_pubkey_hex: String,
        sequence_height: u16,
        tx_block_hash: String,
        tx_index: u64,
        merkle_proof: Vec<String>,
    ) -> Promise {
        assert_gas(Gas(40 * Gas::ONE_TERA.0) + GAS_LIGHTCLIENT_VERIFY + GAS_DEPOSIT_VERIFY_CB); // 100 Tgas

        // TODO assert storage fee.
        // it's the caller's responsibility to ensure there is an output to cover his NEAR cost

        let tx = deserialize_hex::<Transaction>(&tx_hex).expect(ERR_INVALID_TX_HEX);
        let txid = tx.compute_txid();

        require!(
            LockTime::ZERO.partial_cmp(&tx.lock_time).unwrap().is_eq(),
            ERR_BAD_TX_LOCKTIME
        );

        // verify embed output
        let embed_output = tx.output.get(embed_vout as usize).expect(ERR_BAD_EMBED_IDX);
        let msg = get_embed_message(embed_output);

        // verify deposit output
        let deposit_output = tx
            .output
            .get(deposit_vout as usize)
            .expect(ERR_BAD_DEPOSIT_IDX);
        let user_pubkey = PublicKey::from_str(&user_pubkey_hex).expect(ERR_BAD_PUBKER_HEX);
        let sequence = Sequence::from_height(sequence_height);
        match msg.as_str() {
            DEPOSIT_V1_MSG_HEX => {
                self.verify_deposit_output_v1(deposit_output, &user_pubkey, sequence);
            }
            _ => panic!("{}", ERR_EMBED_INVALID_MSG),
        }

        let value = deposit_output.value;

        // set stake transaction(output) as confirmed now to prevent duplicate verification
        self.set_deposit_confirmed(&txid.to_string().into(), deposit_vout);

        // verify confirmation through btc light client
        ext_btc_lightclient::ext(self.btc_lightclient_id.clone())
            .with_static_gas(GAS_LIGHTCLIENT_VERIFY)
            .verify_transaction_inclusion(ProofArgs::new(
                txid.to_string(),
                tx_block_hash,
                tx_index,
                merkle_proof,
                self.n_confirmation,
            ))
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_DEPOSIT_VERIFY_CB)
                    .on_verify_deposit_tx(
                        txid.to_string(),
                        deposit_vout,
                        value.to_sat(),
                        user_pubkey_hex,
                    ),
            )
    }

    #[private]
    pub fn on_verify_deposit_tx(
        &mut self,
        tx_id: String,
        deposit_vout: u64,
        value: u64,
        user_pubkey: String,
        #[callback_result] result: Result<bool, PromiseError>,
    ) -> PromiseOrValue<bool> {
        let valid = result.unwrap_or(false);
        let txid: TxId = tx_id.clone().into();
        if valid {
            // append to user's active deposits
            let mut account = self.get_account(&user_pubkey.clone().into());
            let deposit = Deposit::new(txid.clone(), deposit_vout, value);
            account.insert_active_deposit(deposit);
            self.set_account(account);

            Event::Deposit {
                user_pubkey: &user_pubkey,
                tx_id: &tx_id,
                deposit_vout: deposit_vout.into(),
                value: value.into(),
            }
            .emit();
        } else {
            self.unset_deposit_confirmed(&txid, deposit_vout);
            Event::DepositFailed {
                user_pubkey: &user_pubkey,
                tx_id: &tx_id,
                deposit_vout: deposit_vout.into(),
                value: value.into(),
            }
            .emit();
        }

        PromiseOrValue::Value(valid)
    }
}

impl Contract {
    /// Verify if output is a valid stake output.
    /// Note that this function should **NEVER** be changed once goes online!
    fn verify_deposit_output_v1(
        &self,
        output: &TxOut,
        user_pubkey: &PublicKey,
        sequence: Sequence,
    ) {
        const V1_PATH: &str = "/btc/manage/v1";

        require!(output.script_pubkey.is_p2wsh(), ERR_DEPOSIT_NOT_P2WSH);
        // first 2 bytes are OP_0 OP_PUSHBYTES_32, so we take from the 3rd byte (4th in hex)
        let p2wsh_script_hash = &output.script_pubkey.to_hex_string()[4..];

        // derived pubkey from chain signature
        // if path is changed, a new deposit output version MUST be used
        let allstake_pubkey = &self.generate_btc_pubkey(V1_PATH);

        // build required deposit redeem script:
        // OP_IF
        //     {{sequence}}
        //     OP_CHECKSEQUENCEVERIFY
        //     OP_DROP
        //     {{user pubkey}}
        //     OP_CHECKSIG
        // OP_ELSE
        //     OP_2
        //     {{user pubkey}}
        //     {{allstake pubkey}}
        //     OP_2
        //     OP_CHECKMULTISIG
        // OP_ENDIF
        let script_sig = Builder::new()
            .push_opcode(OP_IF)
            .push_sequence(sequence)
            .push_opcode(OP_CSV)
            .push_opcode(OP_DROP)
            .push_key(user_pubkey)
            .push_opcode(OP_CHECKSIG)
            .push_opcode(OP_ELSE)
            .push_opcode(OP_PUSHNUM_2)
            .push_key(user_pubkey)
            .push_key(allstake_pubkey)
            .push_opcode(OP_PUSHNUM_2)
            .push_opcode(OP_CHECKMULTISIG)
            .push_opcode(OP_ENDIF)
            .into_script();
        let expected_script_hash = env::sha256_array(script_sig.as_bytes());

        // check if script hash == p2wsh
        require!(
            p2wsh_script_hash == hex::encode(expected_script_hash),
            ERR_DEPOSIT_BAD_SCRIPT_HASH
        );
    }

    fn set_deposit_confirmed(&mut self, tx_id: &TxId, vout: u64) {
        let output_id = output_id(tx_id, vout);
        require!(
            !self.confirmed_deposit_txns.contains(&output_id),
            ERR_DEPOSIT_ALREADY_SAVED
        );
        self.confirmed_deposit_txns.insert(&output_id);
    }

    fn unset_deposit_confirmed(&mut self, tx_id: &TxId, vout: u64) {
        let output_id = output_id(tx_id, vout);
        self.confirmed_deposit_txns.remove(&output_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::*;

    fn sequence_height() -> u16 {
        5
    }

    fn submit_deposit(contract: &mut Contract, tx_hex: String, deposit_vout: u64, embed_vout: u64) {
        contract.submit_deposit_tx(
            tx_hex,
            deposit_vout,
            embed_vout,
            user_pubkey_hex(),
            sequence_height(),
            "00000000000000000000088feef67bf3addee2624be0da65588c032192368de8".to_string(),
            0,
            vec![],
        );
    }

    #[test]
    #[should_panic(expected = "Invalid hex transaction")]
    fn test_invalid_tx_hex() {
        let mut contract = test_contract_instance();
        let tx_hex = "01000000001018d9a4c46d52d9b49bd9a93b150ab1aae14952455292e793adadf4d66a97a4d4d0100000000ffffffff150db3c06000000001600142cfaf802afc0a8796a7268c4a0485203e67e4edd024730440220303748c5bfc291e3cf9a69915f8104b18cb9d9cc1363c30d4e2904ddcbe50d2a0220106d6cf6561a1bf27b95a8e55cade3312f0074d8fac1b021963e02b77675be2801210371147ce272e5ecb0856fb1eab9b36bbfe4e9fa376bc2a5de09b5c3004808cd4c00000000";
        submit_deposit(&mut contract, tx_hex.to_string(), 0, 1);
    }

    #[test]
    #[should_panic(expected = "Embed output should have 0 value")]
    fn test_embed_output_not_zero() {
        let mut contract = test_contract_instance();
        let tx_hex = "02000000000101fb030ee85bd29dae3a0bd3de05e68d70f45982650d74308214101770762306b70000000000ffffffff02400d030000000000220020314bcd8388cb2c6802b7024a0ac736a684d4ca60a6b376b8b565a948f78eb4740100000000000000106a0e616c6c7374616b652e7374616b6502483045022100e0faab10e035aac586cb4676b148afa37d7b958bfd069b607a39f5376cbe205c022077447da4415dab6120941a119e337e6771ff620ce8b85ab5e16d8ff2d870edaa012103aeb311069705d0c9500eb514f5f7ebf93be76127bcdb261a68359fda8ee57a1900000000";
        submit_deposit(&mut contract, tx_hex.to_string(), 0, 1);
    }

    #[test]
    #[should_panic(expected = "Embed output is not OP_RETURN")]
    fn test_embed_output_not_opreturn() {
        let mut contract = test_contract_instance();
        let tx_hex = "02000000000101fb030ee85bd29dae3a0bd3de05e68d70f45982650d74308214101770762306b70000000000ffffffff02400d030000000000220020314bcd8388cb2c6802b7024a0ac736a684d4ca60a6b376b8b565a948f78eb4740100000000000000106a0e616c6c7374616b652e7374616b6502483045022100e0faab10e035aac586cb4676b148afa37d7b958bfd069b607a39f5376cbe205c022077447da4415dab6120941a119e337e6771ff620ce8b85ab5e16d8ff2d870edaa012103aeb311069705d0c9500eb514f5f7ebf93be76127bcdb261a68359fda8ee57a1900000000";
        submit_deposit(&mut contract, tx_hex.to_string(), 0, 0);
    }

    #[test]
    #[should_panic(expected = "Invalid embed output msg")]
    fn test_embed_output_bad_msg() {
        let mut contract = test_contract_instance();
        // msg is "allstake.stake"
        let tx_hex = "020000000001015dd10d6e3ff800e0846c67ca29e31287df62f8b5297b40d805bced13da1b8b0a0000000000ffffffff02400d0300000000002200205ecb5f9f7793e117f15e51257f05b31c81185549247afcec324c12687158ecbf0000000000000000106a0e616c6c7374616b652e7374616b650247304402202a3f9d7e532cfcfe33c42d91a2c1363aabd3fa7cd335fe1c5eae1c2627db5dec02206fa2e6d4fdbc8e9b1b59db7c3fc638e5e6351b34dc4ebdf401aa3668cb86316a012103af4030c4ff989dcaca468c23e9995e49dc4e7458b9f06d252249c4efc5baac6900000000";
        submit_deposit(&mut contract, tx_hex.to_string(), 0, 1);
    }

    #[test]
    #[should_panic(expected = "Deposit output bad script hash")]
    fn test_invalid_stake_script() {
        let mut contract = test_contract_instance();
        let tx_hex = "020000000001018593bf29cbe328b53d597e3f9baa86b5990aabc35f9fe47a5a2a242213a8f3070000000000ffffffff02400d030000000000220020166f0bab6ab51e8e820391e3ad393ac075821a836025665cb8467dac2036ebd60000000000000000156a13616c6c7374616b652e6465706f7369742e7631024830450221009c9872421e911a866e8c0d9eb2bcb132c5af4e26eb10f366929f90f554e078b9022001448e4753242277801150f880e7525b99392d7a5f31ce7eaa0e11515f62361c012103011f6b6b0b70ce62b7c7575f661f17c141686055d1eb6c8e85ccc1ded99f15de00000000";
        submit_deposit(&mut contract, tx_hex.to_string(), 0, 1);
    }

    #[test]
    fn test_valid_stake_output() {
        let mut contract = test_contract_instance();
        let tx_hex = "020000000001011fcd48a529b464bef4a49b850579bd62237265eb61887b698e10ee31b568f5ab0000000000ffffffff02400d03000000000022002076efdc4231206f9fdc475e69b79a71201f37b1ed6ead63ecccd22cc89874ef720000000000000000156a13616c6c7374616b652e6465706f7369742e7631024830450221008c5f918d06bad07231152cd645e18115694a741fea07e03ba4887f68dbb123df0220038be69b5e638aa22e3f225d26d90296ac4471ccb14d1bc9a64925e0d28853fe012102f6b15f899fac9c7dc60dcac795291c70e50c3a2ee1d5070dee0d8020781584e500000000";
        submit_deposit(&mut contract, tx_hex.to_string(), 0, 1);
    }
}
