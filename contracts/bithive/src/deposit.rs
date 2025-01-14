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
use consts::CHAIN_SIGNATURES_PATH_V1;
use ext::{ext_btc_light_client, ProofArgs, GAS_LIGHT_CLIENT_VERIFY};
use near_sdk::{
    json_types::U128, near_bindgen, require, Balance, Gas, Promise, PromiseError, ONE_NEAR,
};
use types::{output_id, DepositEmbedMsg, RedeemVersion, SubmitDepositTxArgs, TxId};
use utils::{assert_gas, get_embed_message};

use crate::*;

const ERR_NOT_ENOUGH_STORAGE_DEPOSIT: &str = "Not enough NEAR attached.";
const ERR_INVALID_TX_HEX: &str = "Invalid hex transaction";
const ERR_BAD_PUBKEY_HEX: &str = "Invalid pubkey hex";
const ERR_BAD_DEPOSIT_AMOUNT: &str = "Deposit amount is less than minimum deposit amount";
const ERR_NOT_ABS_TIMELOCK: &str = "Transaction absolute timelock not enabled";

const ERR_BAD_DEPOSIT_IDX: &str = "Bad deposit output index";
const ERR_BAD_EMBED_IDX: &str = "Bad embed output index";

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
    /// * `args.embed_vout` - index of embed (OP_RETURN) output
    /// * `args.tx_block_hash` - block hash in which the transaction is included
    /// * `args.tx_index` - transaction index in the block
    /// * `args.merkle_proof` - merkle proof of transaction in the block
    #[payable]
    pub fn submit_deposit_tx(&mut self, args: SubmitDepositTxArgs) -> Promise {
        self.assert_running();
        assert_gas(Gas(40 * Gas::ONE_TERA.0) + GAS_LIGHT_CLIENT_VERIFY + GAS_DEPOSIT_VERIFY_CB); // 100 Tgas

        // assert storage fee.
        // it's the caller's responsibility to ensure there is an output to cover his NEAR cost
        require!(
            env::attached_deposit() >= STORAGE_DEPOSIT_ACCOUNT,
            ERR_NOT_ENOUGH_STORAGE_DEPOSIT
        );

        let tx = deserialize_hex::<Transaction>(&args.tx_hex).expect(ERR_INVALID_TX_HEX);
        let txid = tx.compute_txid();
        let deposit_vout = match self.verify_embed_output(&tx, args.embed_vout) {
            DepositEmbedMsg::V1 { deposit_vout, .. } => deposit_vout,
        };

        // set deposit transaction(output) as confirmed now to prevent duplicate verification
        // NOTE that deposit vout should be used instead of embed vout!
        self.set_deposit_confirmed(&txid.to_string().into(), deposit_vout);

        // verify confirmation through btc light client
        ext_btc_light_client::ext(self.btc_light_client_id.clone())
            .with_static_gas(GAS_LIGHT_CLIENT_VERIFY)
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
                        args.tx_hex,
                        args.embed_vout,
                        deposit_vout,
                        env::predecessor_account_id(),
                        env::attached_deposit().into(),
                    ),
            )
    }

    #[allow(clippy::too_many_arguments)]
    #[private]
    pub fn on_verify_deposit_tx(
        &mut self,
        tx_hex: String,
        embed_vout: u64,
        deposit_vout: u64,
        caller_id: AccountId,
        refund_amount: U128,
        #[callback_result] result: Result<bool, PromiseError>,
    ) -> bool {
        let valid = result.unwrap_or(false);
        let tx = deserialize_hex::<Transaction>(&tx_hex).expect(ERR_INVALID_TX_HEX);
        let txid = tx.compute_txid();
        if !valid {
            self.unset_deposit_confirmed(&txid.to_string().into(), deposit_vout);
            // refund storage deposit
            Promise::new(caller_id).transfer(refund_amount.into());

            return false;
        }

        self.save_deposit_txn(&tx, embed_vout);

        true
    }
}

impl Contract {
    pub(crate) fn verify_deposit_txn(&self, tx: &Transaction, embed_vout: u64) -> Deposit {
        let txid = tx.compute_txid();
        // verify embed output
        let embed_msg = self.verify_embed_output(tx, embed_vout);
        let (deposit_vout, user_pubkey_hex, sequence_height) = match embed_msg {
            DepositEmbedMsg::V1 {
                deposit_vout,
                user_pubkey,
                sequence_height,
            } => (deposit_vout, hex::encode(user_pubkey), sequence_height),
        };

        require!(
            self.solo_withdrawal_seq_heights.contains(&sequence_height),
            format!(
                "Invalid seq height. Available values are: {:?}",
                self.solo_withdrawal_seq_heights
            )
        );

        if self.earliest_deposit_block_height > 0 {
            self.verify_timelock(tx);
        }

        // verify deposit output
        let deposit_output = tx
            .output
            .get(deposit_vout as usize)
            .expect(ERR_BAD_DEPOSIT_IDX);
        let user_pubkey = PublicKey::from_str(&user_pubkey_hex).expect(ERR_BAD_PUBKEY_HEX);
        let sequence = Sequence::from_height(sequence_height);
        let redeem_version = match embed_msg {
            DepositEmbedMsg::V1 { .. } => {
                self.verify_deposit_output_v1(deposit_output, &user_pubkey, sequence);
                RedeemVersion::V1
            }
        };

        let value = deposit_output.value;
        require!(
            value.to_sat() >= self.min_deposit_satoshi,
            ERR_BAD_DEPOSIT_AMOUNT
        );

        Deposit::new(
            user_pubkey_hex.clone().into(),
            redeem_version,
            txid.to_string().into(),
            deposit_vout,
            value.to_sat(),
            sequence_height.into(),
        )
    }

    pub(crate) fn save_deposit_txn(&mut self, tx: &Transaction, embed_vout: u64) {
        let deposit = self.verify_deposit_txn(tx, embed_vout);
        let mut account = self.get_account(&deposit.user_pubkey.clone());
        account.create_deposit(deposit);
        self.set_account(account);
    }

    pub(crate) fn verify_embed_output(&self, tx: &Transaction, embed_vout: u64) -> DepositEmbedMsg {
        let embed_output = tx.output.get(embed_vout as usize).expect(ERR_BAD_EMBED_IDX);
        let msg = get_embed_message(embed_output);
        DepositEmbedMsg::decode_hex(&msg).unwrap()
    }

    /// Verify if transaction has absolute timelock enabled and set to the correct value.
    fn verify_timelock(&self, tx: &Transaction) {
        for input in &tx.input {
            require!(
                input.sequence.enables_absolute_lock_time(),
                ERR_NOT_ABS_TIMELOCK
            );
        }

        require!(
            LockTime::from_height(self.earliest_deposit_block_height)
                .unwrap()
                .is_implied_by(tx.lock_time),
            format!(
                "Transaction locktime should be set to {}",
                self.earliest_deposit_block_height
            )
        );
    }

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

        // derived pubkey from chain signatures
        // if path is changed, a new deposit output version MUST be used
        let bithive_pubkey = &self.generate_btc_pubkey(CHAIN_SIGNATURES_PATH_V1);

        let script = Self::deposit_script_v1(user_pubkey, bithive_pubkey, sequence);
        let expected_script_hash = env::sha256_array(script.as_bytes());

        // check if script hash == p2wsh
        require!(
            p2wsh_script_hash == hex::encode(expected_script_hash),
            ERR_DEPOSIT_BAD_SCRIPT_HASH
        );
    }

    pub(crate) fn deposit_script_v1(
        user_pubkey: &PublicKey,
        bithive_pubkey: &PublicKey,
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
        //     {{bithive pubkey}}
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
            .push_key(bithive_pubkey)
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

    fn submit_deposit_tx(contract: &mut Contract, tx_hex: String, embed_vout: u64) {
        let mut builder = VMContextBuilder::new();
        builder.attached_deposit(STORAGE_DEPOSIT_ACCOUNT);
        testing_env!(builder.build());

        contract.submit_deposit_tx(SubmitDepositTxArgs {
            tx_hex,
            embed_vout,
            tx_block_hash: "00000000000000000000088feef67bf3addee2624be0da65588c032192368de8"
                .to_string(),
            tx_index: 0,
            merkle_proof: vec![],
        });
    }

    fn verify_deposit_tx(contract: &mut Contract, tx_hex: String, embed_vout: u64) {
        let tx = deserialize_hex::<Transaction>(&tx_hex).expect(ERR_INVALID_TX_HEX);
        contract.verify_deposit_txn(&tx, embed_vout);
    }

    fn build_tx(
        contract: &Contract,
        embed_pubkey: &PublicKey,
        sequence: Sequence,
        embed_value: u64,
        locktime: Option<LockTime>,
    ) -> String {
        let mut tx = Transaction {
            version: Version::TWO,
            lock_time: locktime.unwrap_or(LockTime::from_height(0).unwrap()),
            input: vec![],
            output: vec![],
        };

        // Add a dummy input
        let mut input = TxIn::default();
        if locktime.is_some() {
            input.sequence = Sequence::ENABLE_LOCKTIME_NO_RBF;
        }
        tx.input.push(input);

        // Add the deposit output
        let bithive_pubkey = contract.generate_btc_pubkey(CHAIN_SIGNATURES_PATH_V1);
        let deposit_script = Contract::deposit_script_v1(&user_pubkey(), &bithive_pubkey, sequence);
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
        let embed_msg = DepositEmbedMsg::V1 {
            deposit_vout: 0,
            user_pubkey: hex::decode(embed_pubkey.to_string())
                .unwrap()
                .try_into()
                .unwrap(),
            sequence_height: sequence.to_consensus_u32() as u16,
        };
        let msg: [u8; 51] = embed_msg.encode().as_slice().try_into().unwrap();
        let embed_script = ScriptBuf::new_op_return(msg);
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
        let tx_hex = build_tx(&contract, &user_pubkey(), sequence_height(), 0, None);
        submit_deposit_tx(&mut contract, tx_hex[2..].to_string(), 1);
    }

    #[test]
    #[should_panic(expected = "Embed output should have 0 value")]
    fn test_embed_output_not_zero() {
        let mut contract = test_contract_instance();
        let tx_hex = build_tx(&contract, &user_pubkey(), sequence_height(), 1, None);
        verify_deposit_tx(&mut contract, tx_hex.to_string(), 1);
    }

    #[test]
    #[should_panic(expected = "Embed output is not OP_RETURN")]
    fn test_embed_output_not_opreturn() {
        let mut contract = test_contract_instance();
        let tx_hex = build_tx(&contract, &user_pubkey(), sequence_height(), 0, None);
        verify_deposit_tx(&mut contract, tx_hex.to_string(), 0);
    }

    #[test]
    #[should_panic(expected = "Deposit output bad script hash")]
    fn test_invalid_deposit_script() {
        let mut contract = test_contract_instance();
        let pubkey = PublicKey::from_str(
            "02f6b15f899fac9c7dc60dcac795291c70e50c3a2ee1d5070dee0d8020781584e6",
        )
        .unwrap();
        let tx_hex = build_tx(&contract, &pubkey, sequence_height(), 0, None);
        verify_deposit_tx(&mut contract, tx_hex.to_string(), 1);
    }

    #[test]
    #[should_panic(expected = "Transaction absolute timelock not enabled")]
    fn test_locktime_not_set() {
        let mut contract = test_contract_instance();
        contract.earliest_deposit_block_height = 100;
        let tx_hex = build_tx(
            &contract,
            &user_pubkey(),
            sequence_height(),
            0,
            None, // wrong
        );
        verify_deposit_tx(&mut contract, tx_hex.to_string(), 1);
    }

    #[test]
    #[should_panic(expected = "Transaction locktime should be set to 100")]
    fn test_wrong_locktime_value() {
        let mut contract = test_contract_instance();
        contract.earliest_deposit_block_height = 100;
        let tx_hex = build_tx(
            &contract,
            &user_pubkey(),
            sequence_height(),
            0,
            Some(LockTime::from_height(99).unwrap()), // wrong
        );
        verify_deposit_tx(&mut contract, tx_hex.to_string(), 1);
    }

    #[test]
    #[should_panic(expected = "Transaction locktime should be set to 100")]
    fn test_wrong_locktime_type() {
        let mut contract = test_contract_instance();
        contract.earliest_deposit_block_height = 100;
        let tx_hex = build_tx(
            &contract,
            &user_pubkey(),
            sequence_height(),
            0,
            Some(LockTime::from_time(1653195600).unwrap()), // wrong
        );
        verify_deposit_tx(&mut contract, tx_hex.to_string(), 1);
    }

    #[test]
    fn test_valid_deposit_output() {
        let mut contract = test_contract_instance();
        contract.earliest_deposit_block_height = 100;
        let tx_hex = build_tx(
            &contract,
            &user_pubkey(),
            sequence_height(),
            0,
            Some(LockTime::from_height(100).unwrap()),
        );
        verify_deposit_tx(&mut contract, tx_hex.to_string(), 1);
    }
}
