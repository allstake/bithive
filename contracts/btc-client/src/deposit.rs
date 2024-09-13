use std::str::FromStr;

use account::Deposit;
use bitcoin::{
    absolute::LockTime,
    consensus::encode::deserialize_hex,
    opcodes::all::{
        OP_CHECKMULTISIG, OP_CHECKSIG, OP_CSV, OP_DROP, OP_ELSE, OP_ENDIF, OP_IF, OP_PUSHNUM_2,
    },
    script::Builder,
    PublicKey, ScriptBuf, Sequence, Transaction, TxOut,
};
use consts::{CHAIN_SIGNATURE_PATH_V1, DEPOSIT_MSG_HEX_V1};
use events::Event;
use ext::{ext_btc_lightclient, ProofArgs, GAS_LIGHTCLIENT_VERIFY};
use near_sdk::{
    near_bindgen, require, Balance, Gas, Promise, PromiseError, PromiseOrValue, ONE_NEAR,
};
use types::{output_id, RedeemVersion, SubmitDepositTxArgs, TxId};
use utils::{assert_gas, get_embed_message};

use crate::*;

const ERR_NOT_ENOUGH_STORAGE_DEPOSIT: &str = "Not enough NEAR attached.";
const ERR_INVALID_TX_HEX: &str = "Invalid hex transaction";
const ERR_BAD_TX_LOCKTIME: &str = "Invalid transaction locktime";
const ERR_BAD_PUBKER_HEX: &str = "Invalid pubkey hex";
const ERR_BAD_DEPOSIT_AMOUNT: &str = "Deposit amount is less than minimum deposit amount";

const ERR_BAD_DEPOSIT_IDX: &str = "Bad deposit output index";
const ERR_BAD_EMBED_IDX: &str = "Bad embed output index";

const ERR_EMBED_INVALID_MSG: &str = "Invalid embed output msg";

const ERR_DEPOSIT_NOT_P2WSH: &str = "Deposit output is not P2WSH";
const ERR_DEPOSIT_BAD_SCRIPT_HASH: &str = "Deposit output bad script hash";

const ERR_DEPOSIT_ALREADY_SAVED: &str = "Deposit already saved";

const GAS_DEPOSIT_VERIFY_CB: Gas = Gas(30 * Gas::ONE_TERA.0);

const STORAGE_DEPOSIT_ACCOUNT: Balance = 3 * ONE_NEAR / 100; // 0.03 NEAR

#[near_bindgen]
impl Contract {
    /// Submit a BTC deposit transaction
    /// ### Arguments
    /// * `args.tx_hex` - hex encoded transaction body
    /// * `args.deposit_vout` - index of deposit (p2wsh) output
    /// * `args.embed_vout` - index of embed (OP_RETURN) output
    /// * `args.user_pubkey_hex` - user pubkey hex encoded
    /// * `args.sequence_height` - sequence height used in the redeem script, must be a valid value from the configuration
    /// * `args.tx_block_hash` - block hash in which the transaction is included
    /// * `args.tx_index` - transaction index in the block
    /// * `args.merkle_proof` - merkle proof of transaction in the block
    #[payable]
    pub fn submit_deposit_tx(&mut self, args: SubmitDepositTxArgs) -> Promise {
        assert_gas(Gas(40 * Gas::ONE_TERA.0) + GAS_LIGHTCLIENT_VERIFY + GAS_DEPOSIT_VERIFY_CB); // 100 Tgas

        // assert storage fee.
        // it's the caller's responsibility to ensure there is an output to cover his NEAR cost
        require!(
            env::attached_deposit() >= STORAGE_DEPOSIT_ACCOUNT,
            ERR_NOT_ENOUGH_STORAGE_DEPOSIT
        );

        require!(
            self.solo_withdraw_seq_heights
                .contains(&args.sequence_height),
            format!(
                "Invalid seq height. Available values are: {:?}",
                self.solo_withdraw_seq_heights
            )
        );

        let tx = deserialize_hex::<Transaction>(&args.tx_hex).expect(ERR_INVALID_TX_HEX);
        let txid = tx.compute_txid();

        require!(
            LockTime::ZERO.partial_cmp(&tx.lock_time).unwrap().is_eq(),
            ERR_BAD_TX_LOCKTIME
        );

        // verify embed output
        let embed_output = tx
            .output
            .get(args.embed_vout as usize)
            .expect(ERR_BAD_EMBED_IDX);
        let msg = get_embed_message(embed_output);

        // verify deposit output
        let deposit_output = tx
            .output
            .get(args.deposit_vout as usize)
            .expect(ERR_BAD_DEPOSIT_IDX);
        let user_pubkey = PublicKey::from_str(&args.user_pubkey_hex).expect(ERR_BAD_PUBKER_HEX);
        let sequence = Sequence::from_height(args.sequence_height);
        let redeem_version = match msg.as_str() {
            DEPOSIT_MSG_HEX_V1 => {
                self.verify_deposit_output_v1(deposit_output, &user_pubkey, sequence);
                RedeemVersion::V1
            }
            _ => panic!("{}", ERR_EMBED_INVALID_MSG),
        };

        let value = deposit_output.value;
        require!(
            value.to_sat() >= self.min_deposit_satoshi,
            ERR_BAD_DEPOSIT_AMOUNT
        );

        // set deposit transaction(output) as confirmed now to prevent duplicate verification
        self.set_deposit_confirmed(&txid.to_string().into(), args.deposit_vout);

        // verify confirmation through btc light client
        ext_btc_lightclient::ext(self.btc_lightclient_id.clone())
            .with_static_gas(GAS_LIGHTCLIENT_VERIFY)
            .verify_transaction_inclusion(ProofArgs::new(
                txid.to_string(),
                args.tx_block_hash,
                args.tx_index,
                args.merkle_proof,
                self.n_confirmation,
            ))
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_DEPOSIT_VERIFY_CB)
                    .on_verify_deposit_tx(
                        redeem_version,
                        txid.to_string(),
                        args.deposit_vout,
                        value.to_sat(),
                        sequence.to_consensus_u32(),
                        args.user_pubkey_hex,
                    ),
            )
    }

    #[allow(clippy::too_many_arguments)]
    #[private]
    pub fn on_verify_deposit_tx(
        &mut self,
        redeem_version: RedeemVersion,
        tx_id: String,
        deposit_vout: u64,
        value: u64,
        sequence: u32,
        user_pubkey: String,
        #[callback_result] result: Result<bool, PromiseError>,
    ) -> PromiseOrValue<bool> {
        let valid = result.unwrap_or(false);
        let txid: TxId = tx_id.clone().into();
        if valid {
            // append to user's active deposits
            let mut account = self.get_account(&user_pubkey.clone().into());
            let deposit = Deposit::new(redeem_version, txid.clone(), deposit_vout, value, sequence);
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
        }

        PromiseOrValue::Value(valid)
    }
}

impl Contract {
    /// Verify if output is a valid deposit output.
    /// Note that this function should **NEVER** be changed once goes online!
    fn verify_deposit_output_v1(
        &self,
        output: &TxOut,
        user_pubkey: &PublicKey,
        sequence: Sequence,
    ) {
        require!(output.script_pubkey.is_p2wsh(), ERR_DEPOSIT_NOT_P2WSH);
        // first 2 bytes are OP_0 OP_PUSHBYTES_32, so we take from the 3rd byte (4th in hex)
        let p2wsh_script_hash = &output.script_pubkey.to_hex_string()[4..];

        // derived pubkey from chain signature
        // if path is changed, a new deposit output version MUST be used
        let allstake_pubkey = &self.generate_btc_pubkey(CHAIN_SIGNATURE_PATH_V1);

        let script = Self::deposit_script_v1(user_pubkey, allstake_pubkey, sequence);
        let expected_script_hash = env::sha256_array(script.as_bytes());

        // check if script hash == p2wsh
        require!(
            p2wsh_script_hash == hex::encode(expected_script_hash),
            ERR_DEPOSIT_BAD_SCRIPT_HASH
        );
    }

    pub(crate) fn deposit_script_v1(
        user_pubkey: &PublicKey,
        allstake_pubkey: &PublicKey,
        sequence: Sequence,
    ) -> ScriptBuf {
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
        Builder::new()
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
            .into_script()
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
    use bitcoin::{
        consensus::encode::serialize_hex, opcodes::OP_0, transaction::Version, Amount, TxIn,
    };
    use near_sdk::{test_utils::VMContextBuilder, testing_env};

    use super::*;
    use crate::tests::*;

    fn sequence_height() -> Sequence {
        Sequence::from_height(5)
    }

    fn user_pubkey() -> PublicKey {
        PublicKey::from_str("02f6b15f899fac9c7dc60dcac795291c70e50c3a2ee1d5070dee0d8020781584e5")
            .unwrap()
    }

    fn submit_deposit(contract: &mut Contract, tx_hex: String, deposit_vout: u64, embed_vout: u64) {
        let mut builder = VMContextBuilder::new();
        builder.attached_deposit(STORAGE_DEPOSIT_ACCOUNT);
        testing_env!(builder.build());

        contract.submit_deposit_tx(SubmitDepositTxArgs {
            tx_hex,
            deposit_vout,
            embed_vout,
            user_pubkey_hex: user_pubkey().to_string(),
            sequence_height: sequence_height().0 as u16,
            tx_block_hash: "00000000000000000000088feef67bf3addee2624be0da65588c032192368de8"
                .to_string(),
            tx_index: 0,
            merkle_proof: vec![],
        });
    }

    fn build_tx(
        contract: &Contract,
        user_pubkey: &PublicKey,
        sequence: Sequence,
        embed_msg: &str,
        embed_value: u64,
    ) -> String {
        let mut tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: vec![],
            output: vec![],
        };

        // Add a dummy input
        tx.input.push(TxIn::default());

        // Add the deposit output
        let allstake_pubkey = contract.generate_btc_pubkey(CHAIN_SIGNATURE_PATH_V1);
        let deposit_script = Contract::deposit_script_v1(user_pubkey, &allstake_pubkey, sequence);
        let witness_script_hash = env::sha256_array(deposit_script.as_bytes());

        let p2wsh_script = Builder::new()
            .push_opcode(OP_0) // Witness version 0 (P2WSH)
            .push_slice(witness_script_hash) // Push the SHA256 hash of the witness script
            .into_script();

        tx.output.push(TxOut {
            value: Amount::from_sat(100),
            script_pubkey: p2wsh_script,
        });

        // Add the embed output
        let mut embed_msg_bytes = [0u8; 19];
        embed_msg_bytes.copy_from_slice(&hex::decode(embed_msg).expect("Invalid embed message"));
        let embed_script = ScriptBuf::new_op_return(embed_msg_bytes);
        tx.output.push(TxOut {
            value: Amount::from_sat(embed_value),
            script_pubkey: embed_script,
        });

        serialize_hex(&tx)
    }

    #[test]
    #[should_panic(expected = "Invalid hex transaction")]
    fn test_invalid_tx_hex() {
        let mut contract = test_contract_instance();
        let tx_hex = build_tx(
            &contract,
            &user_pubkey(),
            sequence_height(),
            DEPOSIT_MSG_HEX_V1,
            0,
        );
        submit_deposit(&mut contract, tx_hex[2..].to_string(), 0, 1);
    }

    #[test]
    #[should_panic(expected = "Embed output should have 0 value")]
    fn test_embed_output_not_zero() {
        let mut contract = test_contract_instance();
        let tx_hex = build_tx(
            &contract,
            &user_pubkey(),
            sequence_height(),
            DEPOSIT_MSG_HEX_V1,
            1,
        );
        submit_deposit(&mut contract, tx_hex.to_string(), 0, 1);
    }

    #[test]
    #[should_panic(expected = "Embed output is not OP_RETURN")]
    fn test_embed_output_not_opreturn() {
        let mut contract = test_contract_instance();
        let tx_hex = build_tx(
            &contract,
            &user_pubkey(),
            sequence_height(),
            DEPOSIT_MSG_HEX_V1,
            0,
        );
        submit_deposit(&mut contract, tx_hex.to_string(), 0, 0);
    }

    #[test]
    #[should_panic(expected = "Invalid embed output msg")]
    fn test_embed_output_bad_msg() {
        let mut contract = test_contract_instance();
        let tx_hex = build_tx(
            &contract,
            &user_pubkey(),
            sequence_height(),
            "016c6c7374616b652e6465706f7369742e7631",
            0,
        );
        submit_deposit(&mut contract, tx_hex.to_string(), 0, 1);
    }

    #[test]
    #[should_panic(expected = "Deposit output bad script hash")]
    fn test_invalid_stake_script() {
        let mut contract = test_contract_instance();
        let pubkey = PublicKey::from_str(
            "02f6b15f899fac9c7dc60dcac795291c70e50c3a2ee1d5070dee0d8020781584e6",
        )
        .unwrap();
        let tx_hex = build_tx(&contract, &pubkey, sequence_height(), DEPOSIT_MSG_HEX_V1, 0);
        submit_deposit(&mut contract, tx_hex.to_string(), 0, 1);
    }

    #[test]
    fn test_valid_stake_output() {
        let mut contract = test_contract_instance();
        let tx_hex = build_tx(
            &contract,
            &user_pubkey(),
            sequence_height(),
            DEPOSIT_MSG_HEX_V1,
            0,
        );
        submit_deposit(&mut contract, tx_hex.to_string(), 0, 1);
    }
}
