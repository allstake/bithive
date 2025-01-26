use std::str::FromStr;

use crate::*;
use account::Deposit;
use bitcoin::{consensus::encode::deserialize_hex, Psbt, PublicKey, Transaction, TxIn};
use consts::{CHAIN_SIGNATURES_KEY_VERSION_V1, CHAIN_SIGNATURES_PATH_V1};
use events::Event;
use ext::{
    ext_bip322_verifier, ext_btc_light_client, ext_chain_signatures, ProofArgs, SignRequest,
    SignatureResponse, GAS_LIGHT_CLIENT_VERIFY,
};
use near_sdk::{
    env::{self},
    json_types::U128,
    near_bindgen, require, Balance, Gas, Promise, PromiseError, PromiseOrValue, ONE_NEAR,
};
use serde::{Deserialize, Serialize};
use types::{DepositEmbedMsg, PendingSignPsbt, RedeemVersion, SubmitWithdrawTxArgs, TxId};
use utils::{
    assert_gas, current_timestamp_ms, get_hash_to_sign, verify_secp256k1_signature,
    verify_signed_message_ecdsa,
};

const GAS_CHAIN_SIG_SIGN: Gas = Gas(250 * Gas::ONE_TERA.0);
const GAS_CHAIN_SIG_SIGN_CB: Gas = Gas(10 * Gas::ONE_TERA.0);
const GAS_WITHDRAW_VERIFY_CB: Gas = Gas(80 * Gas::ONE_TERA.0);
const GAS_BIP322_VERIFY: Gas = Gas(20 * Gas::ONE_TERA.0);
const GAS_BIP322_VERIFY_CB: Gas = Gas(20 * Gas::ONE_TERA.0);

// queue withdrawal errors
const ERR_BIP322_NOT_ENABLED: &str = "BIP322 is not enabled";
const ERR_INVALID_WITHDRAWAL_AMOUNT: &str = "Withdrawal amount must be greater than 0";
// sign withdrawal errors
const ERR_INVALID_STORAGE_DEPOSIT: &str = "Invalid storage deposit amount";
const ERR_INSUFFICIENT_STORAGE_DEPOSIT: &str = "Insufficient storage deposit";
const ERR_INVALID_PSBT_HEX: &str = "Invalid PSBT hex";
const ERR_NO_WITHDRAW_REQUESTED: &str = "No withdrawal request made";
const ERR_WITHDRAW_NOT_READY: &str = "Not ready to withdraw now";
const ERR_MISSING_PARTIAL_SIG: &str = "Missing partial sig for given input";
const ERR_INVALID_PARTIAL_SIG: &str = "Invalid partial signature for withdrawal PSBT";
const ERR_BAD_WITHDRAWAL_AMOUNT: &str = "Withdrawal amount is larger than queued amount";
const ERR_PSBT_INPUT_LEN_MISMATCH: &str = "PSBT input length mismatch";
const ERR_PSBT_INPUT_MISMATCH: &str = "PSBT input mismatch";
const ERR_PSBT_REINVEST_PUBKEY_MISMATCH: &str = "PSBT reinvest pubkey mismatch";
const ERR_PSBT_REINVEST_OUTPUT_MISMATCH: &str = "PSBT reinvest output mismatch";
// submit withdrawal errors
const ERR_INVALID_TX_HEX: &str = "Invalid txn hex";
const ERR_NOT_WITHDRAW_TXN: &str = "Not a withdrawal transaction";

const REFUND_THRESHOLD: Balance = ONE_NEAR / 100; // 0.01 NEAR

/// in case different wallet signs message in different form,
/// the signer needs to explicitly specify the type
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub enum SigType {
    #[allow(clippy::upper_case_acronyms)]
    ECDSA,
    Bip322Full {
        address: String,
    },
}

#[near_bindgen]
impl Contract {
    /// Submit a queue withdrawal request for a user
    /// NOTE: this will reset the queue withdrawal count down
    /// ### Arguments
    /// * `user_pubkey` - hex encoded user pub key
    /// * `withdraw_amount` - amount to withdraw
    /// * `msg_sig` - hex encoded signature of queue withdrawal message that should match `user_pubkey`
    /// * `sig_type` - signature type
    pub fn queue_withdrawal(
        &mut self,
        user_pubkey: String,
        withdraw_amount: u64,
        msg_sig: String,
        sig_type: SigType,
    ) -> PromiseOrValue<bool> {
        self.assert_running();
        assert_gas(Gas(40 * Gas::ONE_TERA.0) + GAS_BIP322_VERIFY + GAS_BIP322_VERIFY_CB); // 80 Tgas
        require!(withdraw_amount > 0, ERR_INVALID_WITHDRAWAL_AMOUNT);

        let mut account = self.get_account(&user_pubkey.clone().into());

        // verify msg signature
        let expected_withdraw_msg = withdrawal_message(account.nonce, withdraw_amount);
        match sig_type {
            SigType::ECDSA => {
                let msg = verify_signed_message_ecdsa(
                    &expected_withdraw_msg.into_bytes(),
                    &hex::decode(&msg_sig).unwrap(),
                    &hex::decode(&user_pubkey).unwrap(),
                );
                account.queue_withdrawal(withdraw_amount, msg, &msg_sig);
                self.set_account(account);
                PromiseOrValue::Value(true)
            }
            SigType::Bip322Full { address } => {
                require!(self.bip322_verifier_id.is_some(), ERR_BIP322_NOT_ENABLED);
                ext_bip322_verifier::ext(self.bip322_verifier_id.clone().unwrap())
                    .with_static_gas(GAS_BIP322_VERIFY)
                    .verify_bip322_full(
                        user_pubkey.clone(),
                        address,
                        expected_withdraw_msg.clone(),
                        msg_sig.clone(),
                    )
                    .then(
                        Self::ext(env::current_account_id())
                            .with_static_gas(GAS_BIP322_VERIFY_CB)
                            .on_bip322_verify(
                                user_pubkey,
                                withdraw_amount,
                                expected_withdraw_msg,
                                msg_sig,
                            ),
                    )
                    .into()
            }
        }
    }

    #[private]
    pub fn on_bip322_verify(
        &mut self,
        user_pubkey: String,
        withdraw_amount: u64,
        msg: String,
        msg_sig: String,
        #[callback_result] result: Result<bool, PromiseError>,
    ) -> PromiseOrValue<bool> {
        let valid = result.unwrap_or(false);
        if !valid {
            return PromiseOrValue::Value(false);
        }

        let mut account = self.get_account(&user_pubkey.clone().into());
        account.queue_withdrawal(withdraw_amount, msg.into_bytes(), &msg_sig);
        self.set_account(account);
        PromiseOrValue::Value(true)
    }

    /// Sign a BTC withdrawal PSBT via chain signatures for multisig withdrawal
    /// ### Arguments
    /// * `psbt_hex` - hex encoded PSBT to sign, must be partially signed by the user first
    /// * `user_pubkey` - user public key
    /// * `vin_to_sign` - vin to sign, must be an active deposit UTXO
    /// * `reinvest_embed_vout` - vout of the reinvestment deposit embed UTXO
    /// * `storage_deposit` - attached NEAR amount as storage deposit for pending sign PSBT
    #[payable]
    pub fn sign_withdrawal(
        &mut self,
        psbt_hex: String,
        user_pubkey: String,
        vin_to_sign: u64,
        reinvest_embed_vout: Option<u64>,
        storage_deposit: Option<U128>,
    ) -> Promise {
        self.assert_running();

        assert_gas(Gas(40 * Gas::ONE_TERA.0) + GAS_CHAIN_SIG_SIGN + GAS_CHAIN_SIG_SIGN_CB); // 300 Tgas

        let mut attached_near_for_storage = 0u128;

        let psbt_bytes = hex::decode(psbt_hex).unwrap();
        let psbt = Psbt::deserialize(&psbt_bytes).expect(ERR_INVALID_PSBT_HEX);

        let mut account = self.get_account(&user_pubkey.clone().into());

        let input_to_sign = psbt.unsigned_tx.input.get(vin_to_sign as usize).unwrap();
        let deposit = account.get_active_deposit(
            &input_to_sign.previous_output.txid.to_string().into(),
            input_to_sign.previous_output.vout.into(),
        );

        if account.pending_sign_psbt.is_some() {
            // if the user has previously requested to sign a withdrawal tx, he cannot request to
            // sign another one until the previous one is completed or replaced by fee
            verify_sign_withdrawal_psbt(account.pending_sign_psbt.as_ref().unwrap(), &psbt);
        } else {
            // if not, verify the withdrawal PSBT and save it for signing
            verify_pending_sign_partial_sig(&psbt, vin_to_sign, &user_pubkey);
            let reinvest_deposit_vout =
                self.verify_pending_sign_request_amount(&account, &psbt, reinvest_embed_vout);

            // if there is more than one input in PSBT, we charge the user for PSBT storage deposit
            if psbt.unsigned_tx.input.len() > 1 {
                attached_near_for_storage = storage_deposit.unwrap_or(U128::from(0)).into();
                require!(
                    env::attached_deposit() >= attached_near_for_storage,
                    ERR_INVALID_STORAGE_DEPOSIT
                );
                let storage_needed = psbt_bytes.len() as u128 * env::storage_byte_cost();
                require!(
                    account.pending_sign_deposit + attached_near_for_storage >= storage_needed,
                    ERR_INSUFFICIENT_STORAGE_DEPOSIT
                );
            }

            // update account state
            account.pending_sign_psbt = Some(PendingSignPsbt {
                psbt: psbt.clone().into(),
                reinvest_deposit_vout,
            });
            account.pending_sign_deposit += attached_near_for_storage;
            // reset queue withdrawal amount
            account.queue_withdrawal_amount = 0;
            account.queue_withdrawal_start_ts = 0;

            self.set_account(account);
        }

        // request signature from chain signatures
        let payload = get_hash_to_sign(&psbt, vin_to_sign);
        let (path, key_version) = match deposit.redeem_version {
            RedeemVersion::V1 => (
                CHAIN_SIGNATURES_PATH_V1.to_string(),
                CHAIN_SIGNATURES_KEY_VERSION_V1,
            ),
        };
        let req = SignRequest {
            payload,
            path,
            key_version,
        };
        // the rest of the attached NEAR will be used for chain signatures
        let chain_signatures_deposit = env::attached_deposit() - attached_near_for_storage;
        ext_chain_signatures::ext(self.chain_signatures_id.clone())
            .with_static_gas(GAS_CHAIN_SIG_SIGN)
            .with_attached_deposit(chain_signatures_deposit)
            .sign(req)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_CHAIN_SIG_SIGN_CB)
                    .on_sign_withdrawal(
                        user_pubkey,
                        env::predecessor_account_id(),
                        chain_signatures_deposit.into(),
                    ),
            )
    }

    #[private]
    pub fn on_sign_withdrawal(
        &mut self,
        user_pubkey: String,
        caller_id: AccountId,
        attached_deposit: U128,
        #[callback_result] result: Result<SignatureResponse, PromiseError>,
    ) -> Option<SignatureResponse> {
        if let Ok(sig) = result {
            Event::SignWithdrawal {
                user_pubkey: &user_pubkey,
            }
            .emit();

            Some(sig)
        } else {
            // refund
            if attached_deposit.0 >= REFUND_THRESHOLD {
                Promise::new(caller_id).transfer(attached_deposit.into());
            }
            None
        }
    }

    /// Submit a BTC withdrawal (either solo or multisig) transaction
    /// ### Arguments
    /// * `args.tx_hex` - hex encoded transaction body
    /// * `args.user_pubkey` - user public key
    /// * `args.tx_block_hash` - block hash in which the transaction is included
    /// * `args.tx_index` - transaction index in the block
    /// * `args.merkle_proof` - merkle proof of transaction in the block
    pub fn submit_withdrawal_tx(&mut self, args: SubmitWithdrawTxArgs) -> Promise {
        self.assert_running();

        assert_gas(Gas(30 * Gas::ONE_TERA.0) + GAS_LIGHT_CLIENT_VERIFY + GAS_WITHDRAW_VERIFY_CB); // 140 Tgas

        let tx = deserialize_hex::<Transaction>(&args.tx_hex).expect(ERR_INVALID_TX_HEX);
        let txid = tx.compute_txid();

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
                    .with_static_gas(GAS_WITHDRAW_VERIFY_CB)
                    .on_verify_withdrawal_tx(args.user_pubkey, args.tx_hex),
            )
    }

    #[private]
    pub fn on_verify_withdrawal_tx(
        &mut self,
        user_pubkey: String,
        tx_hex: String,
        #[callback_result] result: Result<bool, PromiseError>,
    ) -> bool {
        let valid = result.unwrap_or(false);
        if !valid {
            return false;
        }

        let tx = deserialize_hex::<Transaction>(&tx_hex).expect(ERR_INVALID_TX_HEX);
        let tx_id: TxId = tx.compute_txid().to_string().into();

        let mut account = self.get_account(&user_pubkey.clone().into());
        let deposit_inputs = filter_deposit_inputs(&account, &tx.input);
        require!(!deposit_inputs.is_empty(), ERR_NOT_WITHDRAW_TXN);

        for deposit_input in deposit_inputs {
            let deposit = account.get_active_deposit(
                &deposit_input.previous_output.txid.to_string().into(),
                deposit_input.previous_output.vout.into(),
            );
            let is_multisig = is_multisig_withdrawal(&deposit, deposit_input);
            account.complete_withdrawal(deposit, &tx_id, is_multisig);
        }
        self.set_account(account);

        true
    }
}

impl Contract {
    /// Verify if the withdrawal amount in the PSBT is valid
    /// Returns the reinvest deposit vout if any
    pub(crate) fn verify_pending_sign_request_amount(
        &self,
        account: &Account,
        psbt: &Psbt,
        reinvest_embed_vout: Option<u64>,
    ) -> Option<u64> {
        require!(
            account.queue_withdrawal_amount > 0 && account.queue_withdrawal_start_ts > 0,
            ERR_NO_WITHDRAW_REQUESTED
        );

        // make sure queue waiting time has passed
        require!(
            current_timestamp_ms()
                >= account.queue_withdrawal_start_ts + self.withdrawal_waiting_time_ms,
            ERR_WITHDRAW_NOT_READY
        );

        // sum all known deposit inputs
        let deposit_input_sum = filter_deposit_inputs(account, &psbt.unsigned_tx.input)
            .iter()
            .map(|input| {
                let deposit = account.get_active_deposit(
                    &input.previous_output.txid.to_string().into(),
                    input.previous_output.vout.into(),
                );
                deposit.value
            })
            .sum::<u64>();

        // subtract reinvest amount if provided
        let reinvest_amount = reinvest_embed_vout
            .map(|embed_vout| {
                let deposit = self.verify_deposit_txn(&psbt.unsigned_tx, embed_vout);
                require!(
                    deposit.user_pubkey == account.pubkey,
                    ERR_PSBT_REINVEST_PUBKEY_MISMATCH
                );
                deposit.value
            })
            .unwrap_or(0);
        let actual_withdraw_amount = deposit_input_sum - reinvest_amount;

        // make sure the actual amount is less than or equal to the requested withdrawal amount
        require!(
            actual_withdraw_amount <= account.queue_withdrawal_amount,
            ERR_BAD_WITHDRAWAL_AMOUNT
        );

        // return the reinvest deposit vout if any
        reinvest_embed_vout.map(|embed_vout| {
            let embed_msg = self.verify_embed_output(&psbt.unsigned_tx, embed_vout);
            match embed_msg {
                DepositEmbedMsg::V1 { deposit_vout, .. } => deposit_vout,
            }
        })
    }
}

pub(crate) fn withdrawal_message(nonce: u64, amount: u64) -> String {
    format!("bithive.withdraw:{}:{}sats", nonce, amount)
}

/// Verify if the PSBT has a valid partial signature for the given input
/// This is to make sure the PSBT is submitted by the user himself
pub(crate) fn verify_pending_sign_partial_sig(psbt: &Psbt, vin_to_sign: u64, user_pubkey: &str) {
    let input = psbt.inputs.get(vin_to_sign as usize).unwrap();
    let hash_to_sign = get_hash_to_sign(psbt, vin_to_sign);

    let pubkey = PublicKey::from_str(user_pubkey).unwrap();
    let user_sig = input
        .partial_sigs
        .get(&pubkey)
        .expect(ERR_MISSING_PARTIAL_SIG)
        .signature
        .serialize_compact();

    // try with v = 0 and v = 1
    verify_secp256k1_signature(&pubkey.inner.serialize(), &hash_to_sign, &user_sig, 0u8)
        .or_else(|_| {
            verify_secp256k1_signature(&pubkey.inner.serialize(), &hash_to_sign, &user_sig, 1u8)
        })
        .expect(ERR_INVALID_PARTIAL_SIG);
}

/// The PSBT provided must be the same or RBF of the saved withdrawal PSBT
pub(crate) fn verify_sign_withdrawal_psbt(
    pending_sign_psbt: &PendingSignPsbt,
    request_psbt: &Psbt,
) {
    let expected_psbt: bitcoin::Psbt = pending_sign_psbt.psbt.clone().into();

    // each input must match the saved PSBT
    require!(
        request_psbt.unsigned_tx.input.len() == expected_psbt.unsigned_tx.input.len(),
        ERR_PSBT_INPUT_LEN_MISMATCH
    );
    for (i, input) in request_psbt.unsigned_tx.input.iter().enumerate() {
        let expected_input = expected_psbt.unsigned_tx.input.get(i).unwrap();
        require!(input == expected_input, ERR_PSBT_INPUT_MISMATCH);
    }

    // for outputs, we need to make sure the reinvest output is the same
    if let Some(reinvest_deposit_vout) = pending_sign_psbt.reinvest_deposit_vout {
        let expected_output = expected_psbt
            .unsigned_tx
            .output
            .get(reinvest_deposit_vout as usize)
            .unwrap();
        let request_output = request_psbt
            .unsigned_tx
            .output
            .get(reinvest_deposit_vout as usize)
            .unwrap();
        require!(
            request_output == expected_output,
            ERR_PSBT_REINVEST_OUTPUT_MISMATCH
        );
    }
}

fn filter_deposit_inputs<'a>(account: &Account, inputs: &'a [TxIn]) -> Vec<&'a TxIn> {
    inputs
        .iter()
        .filter(|input| {
            account.is_deposit_active(
                &input.previous_output.txid.to_string().into(),
                input.previous_output.vout.into(),
            )
        })
        .collect()
}

fn is_multisig_withdrawal(deposit: &Deposit, tx_in: &TxIn) -> bool {
    match deposit.redeem_version {
        RedeemVersion::V1 => {
            let witness = tx_in.witness.to_vec();
            // witness script should have 5 elements, and the second last one should be empty
            witness.len() == 5 && witness[witness.len() - 2].is_empty()
        }
    }
}

#[cfg(test)]
mod tests {
    use bitcoin::{hashes::Hash, Amount, OutPoint, ScriptBuf, Sequence, TxOut, Txid, Witness};

    use super::*;

    fn test_psbt(inputs: Vec<TxIn>, outputs: Vec<TxOut>) -> Psbt {
        let tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
            input: inputs,
            output: outputs,
        };
        Psbt::from_unsigned_tx(tx).unwrap()
    }

    fn test_input1() -> TxIn {
        TxIn {
            previous_output: OutPoint::new(Txid::all_zeros(), 0),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }
    }

    fn test_input2() -> TxIn {
        TxIn {
            previous_output: OutPoint::new(Txid::all_zeros(), 1),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }
    }

    fn test_output1() -> TxOut {
        TxOut {
            value: Amount::from_sat(1000),
            script_pubkey: ScriptBuf::new(),
        }
    }

    fn test_output2() -> TxOut {
        TxOut {
            value: Amount::from_sat(2000),
            script_pubkey: ScriptBuf::new(),
        }
    }

    #[test]
    #[should_panic(expected = "PSBT input length mismatch")]
    fn test_verify_sign_withdrawal_psbt_wrong_input_len() {
        let pending_sign_psbt = PendingSignPsbt {
            psbt: test_psbt(vec![test_input1(), test_input2()], vec![]).into(),
            reinvest_deposit_vout: None,
        };
        let request_psbt = test_psbt(vec![test_input1()], vec![]);
        verify_sign_withdrawal_psbt(&pending_sign_psbt, &request_psbt);
    }

    #[test]
    #[should_panic(expected = "PSBT input mismatch")]
    fn test_verify_sign_withdrawal_psbt_wrong_input() {
        let pending_sign_psbt = PendingSignPsbt {
            psbt: test_psbt(vec![test_input1()], vec![]).into(),
            reinvest_deposit_vout: None,
        };
        let request_psbt = test_psbt(vec![test_input2()], vec![]);
        verify_sign_withdrawal_psbt(&pending_sign_psbt, &request_psbt);
    }

    #[test]
    #[should_panic(expected = "PSBT reinvest output mismatch")]
    fn test_verify_sign_withdrawal_psbt_wrong_reinvest_output() {
        let pending_sign_psbt = PendingSignPsbt {
            psbt: test_psbt(vec![test_input1(), test_input2()], vec![test_output1()]).into(),
            reinvest_deposit_vout: Some(0),
        };
        let request_psbt = test_psbt(vec![test_input1(), test_input2()], vec![test_output2()]);
        verify_sign_withdrawal_psbt(&pending_sign_psbt, &request_psbt);
    }
}
