use crate::*;
use account::Deposit;
use bitcoin::{consensus::encode::deserialize_hex, Psbt, Transaction};
use consts::{CHAIN_SIGNATURE_KEY_VERSION_V1, CHAIN_SIGNATURE_PATH_V1};
use events::Event;
use ext::{
    ext_btc_lightclient, ext_chain_signature, ProofArgs, SignRequest, SignatureResponse,
    GAS_LIGHTCLIENT_VERIFY,
};
use near_sdk::{
    env::{self},
    json_types::U128,
    near_bindgen, require, Gas, Promise, PromiseError,
};
use serde::{Deserialize, Serialize};
use types::RedeemVersion;
use utils::{assert_gas, current_timestamp_ms, get_hash_to_sign, verify_signed_message_ecdsa};

const GAS_CHAIN_SIG_SIGN: Gas = Gas(250 * Gas::ONE_TERA.0);
const GAS_CHAIN_SIG_SIGN_CB: Gas = Gas(10 * Gas::ONE_TERA.0);
const GAS_WITHDRAW_VERIFY_CB: Gas = Gas(80 * Gas::ONE_TERA.0);

const ERR_INVALID_PSBT_HEX: &str = "Invalid PSBT hex";
const ERR_NO_WITHDRAW_REQUESTED: &str = "No withdraw request made";
const ERR_WITHDRAW_NOT_READY: &str = "Not ready to withdraw now";
const ERR_BAD_WITHDRAW_AMOUNT: &str = "Withdraw amount is larger than queued amount";
const ERR_CHAIN_SIG_FAILED: &str = "Failed to sign via chain signature";

const ERR_INVALID_TX_HEX: &str = "Invalid txn hex";
const ERR_NOT_WITHDRAW_TXN: &str = "Not a withdrawal transaction";

/// in case different wallet signs message in different form,
/// the signer needs to explicitly specify the type
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub enum SigType {
    #[allow(clippy::upper_case_acronyms)]
    ECDSA,
}

#[near_bindgen]
impl Contract {
    /// Submit a queue withdrawal request for a user
    /// NOTE: this will reset the queue withdrawal count down
    /// ### Arguments
    /// * `user_pubkey` - hex encoded user pub key
    /// * `withdraw_amount` - amount to withdraw
    /// * `msg_sig` - hex encoded signature of queue withdraw message that shoud match `user_pubkey`
    /// * `sig_type` - signature type
    pub fn queue_withdrawal(
        &mut self,
        user_pubkey: String,
        withdraw_amount: u64,
        msg_sig: String,
        sig_type: SigType,
    ) {
        let mut account = self.get_account(&user_pubkey.clone().into());

        // verify msg signature
        let expected_withdraw_msg = self.withdrawal_message(account.nonce, withdraw_amount);
        let msg = match sig_type {
            SigType::ECDSA => verify_signed_message_ecdsa(
                &expected_withdraw_msg.into_bytes(),
                &hex::decode(&msg_sig).unwrap(),
                &hex::decode(&user_pubkey).unwrap(),
            ),
        };

        account.queue_withdrawal(withdraw_amount, msg, &msg_sig);
        self.set_account(account);
    }

    /// Sign a BTC withdrawal PSBT via chain signature for multisig withdraw
    /// ### Arguments
    /// * `psbt_hex` - hex encoded PSBT to sign
    /// * `user_pubkey` - user public key
    /// * `vin_to_sign` - vin to sign, must be an active deposit UTXO
    /// * `reinvest_embed_vout` - vout of the reinvestment deposit embed UTXO
    #[payable]
    pub fn sign_withdrawal(
        &mut self,
        psbt_hex: String,
        user_pubkey: String,
        vin_to_sign: u64,
        reinvest_embed_vout: Option<u64>,
    ) -> Promise {
        assert_gas(Gas(40 * Gas::ONE_TERA.0) + GAS_CHAIN_SIG_SIGN + GAS_CHAIN_SIG_SIGN_CB); // 300 Tgas

        let psbt_bytes = hex::decode(psbt_hex).unwrap();
        let psbt = Psbt::deserialize(&psbt_bytes).expect(ERR_INVALID_PSBT_HEX);

        let mut account = self.get_account(&user_pubkey.clone().into());

        let input_to_sign = psbt.unsigned_tx.input.get(vin_to_sign as usize).unwrap();
        let deposit = account.get_active_deposit(
            &input_to_sign.previous_output.txid.to_string().into(),
            input_to_sign.previous_output.vout.into(),
        );

        if let Some(tx_id) = account.pending_withdraw_tx_id.clone() {
            // if the user has previously requested to sign a withdraw tx, he cannot request to
            // sign another one until the previous one is completed

            require!(
                tx_id.to_string() != psbt.unsigned_tx.compute_txid().to_string(),
                format!(
                    "A pending withdrawal tx {} already exists",
                    tx_id.to_string()
                )
            );
        } else {
            // if not , verify the withdraw PSBT and save it for signing

            self.verify_sign_withdrawal(&account, &psbt, reinvest_embed_vout);
            // update pending withdraw tx id and count
            account.set_pending_withdraw_tx(
                psbt.unsigned_tx.compute_txid().to_string().into(),
                self.filter_deposit_inputs(&account, &psbt.unsigned_tx.input)
                    .len() as u16,
            );
            // TODO reset queue withdrawal amount ?
            self.set_account(account);
        }

        // request signature from chain signature
        let payload = get_hash_to_sign(&psbt, vin_to_sign);
        let (path, key_version) = match deposit.redeem_version {
            RedeemVersion::V1 => (
                CHAIN_SIGNATURE_PATH_V1.to_string(),
                CHAIN_SIGNATURE_KEY_VERSION_V1,
            ),
        };
        let req = SignRequest {
            payload,
            path,
            key_version,
        };
        ext_chain_signature::ext(self.chain_signature_id.clone())
            .with_static_gas(GAS_CHAIN_SIG_SIGN)
            .with_attached_deposit(env::attached_deposit())
            .sign(req)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_CHAIN_SIG_SIGN_CB)
                    .on_sign_withdrawal(
                        user_pubkey,
                        env::predecessor_account_id(),
                        env::attached_deposit().into(),
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
    ) -> SignatureResponse {
        if let Ok(sig) = result {
            let mut account = self.get_account(&user_pubkey.clone().into());
            let pending_withdraw_tx_id = account.pending_withdraw_tx_id.clone().unwrap();
            account.on_sign_withdrawal();
            self.set_account(account);

            Event::SignWithdrawal {
                user_pubkey: &user_pubkey,
                pending_withdraw_tx_id: &pending_withdraw_tx_id.to_string(),
            }
            .emit();

            sig
        } else {
            // refund
            Promise::new(caller_id).transfer(attached_deposit.into());
            panic!("{}", ERR_CHAIN_SIG_FAILED);
        }
    }

    /// Submit a BTC withdrawal (either solo or multisig) transaction
    /// ### Arguments
    /// * `tx_hex` - hex encoded transaction body
    /// * `user_pubkey` - user public key
    /// * `reinvest_embed_vout` - vout of the reinvestment deposit embed UTXO
    /// * `tx_block_hash` - block hash in which the transaction is included
    /// * `tx_index` - transaction index in the block
    /// * `merkle_proof` - merkle proof of transaction in the block
    pub fn submit_withdrawal_tx(
        &mut self,
        tx_hex: String,
        user_pubkey: String,
        reinvest_embed_vout: Option<u64>,
        tx_block_hash: String,
        tx_index: u64,
        merkle_proof: Vec<String>,
    ) -> Promise {
        assert_gas(Gas(30 * Gas::ONE_TERA.0) + GAS_LIGHTCLIENT_VERIFY + GAS_WITHDRAW_VERIFY_CB); // 140 Tgas

        let tx = deserialize_hex::<Transaction>(&tx_hex).expect(ERR_INVALID_TX_HEX);
        let txid = tx.compute_txid();

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
                    .with_static_gas(GAS_WITHDRAW_VERIFY_CB)
                    .on_verify_withdrawal_tx(user_pubkey, tx_hex, reinvest_embed_vout),
            )
    }

    #[private]
    pub fn on_verify_withdrawal_tx(
        &mut self,
        user_pubkey: String,
        tx_hex: String,
        reinvest_embed_vout: Option<u64>,
        #[callback_result] result: Result<bool, PromiseError>,
    ) -> bool {
        let valid = result.unwrap_or(false);
        if !valid {
            return false;
        }

        let tx = deserialize_hex::<Transaction>(&tx_hex).expect(ERR_INVALID_TX_HEX);
        let mut account = self.get_account(&user_pubkey.clone().into());
        let deposits = self.filter_deposit_inputs(&account, &tx.input);
        require!(!deposits.is_empty(), ERR_NOT_WITHDRAW_TXN);

        for deposit in deposits {
            account.complete_withdrawal(deposit, tx.compute_txid().to_string().into());
        }
        self.set_account(account);

        if let Some(reinvest_embed_vout) = reinvest_embed_vout {
            self.save_deposit_txn(&tx, reinvest_embed_vout);
        }

        true
    }
}

impl Contract {
    pub(crate) fn withdrawal_message(&self, nonce: u64, amount: u64) -> String {
        format!("allstake.withdraw:{}:{}sats", nonce, amount)
    }

    fn filter_deposit_inputs(&self, account: &Account, inputs: &[bitcoin::TxIn]) -> Vec<Deposit> {
        inputs
            .iter()
            .filter_map(|input| {
                account.try_get_active_deposit(
                    &input.previous_output.txid.to_string().into(),
                    input.previous_output.vout.into(),
                )
            })
            .collect()
    }

    pub(crate) fn verify_sign_withdrawal(
        &self,
        account: &Account,
        psbt: &Psbt,
        reinvest_embed_vout: Option<u64>,
    ) {
        require!(
            account.queue_withdrawal_amount > 0 && account.queue_withdrawal_start_ts > 0,
            ERR_NO_WITHDRAW_REQUESTED
        );

        // make sure queue waiting time has passed
        require!(
            current_timestamp_ms()
                >= account.queue_withdrawal_start_ts + self.withdraw_waiting_time_ms,
            ERR_WITHDRAW_NOT_READY
        );

        // sum all known deposit inputs
        let deposit_input_sum = self
            .filter_deposit_inputs(account, &psbt.unsigned_tx.input)
            .iter()
            .map(|deposit| deposit.value)
            .sum::<u64>();

        // subtract reinvest amount if provided
        let reinvest_amount = reinvest_embed_vout
            .map(|vout| {
                let deposit = self.verify_deposit_txn(&psbt.unsigned_tx, vout);
                deposit.value
            })
            .unwrap_or(0);
        let actual_withdraw_amount = deposit_input_sum - reinvest_amount;

        // make sure the actual amount is less than the requested withdraw amount
        require!(
            actual_withdraw_amount <= account.queue_withdrawal_amount,
            ERR_BAD_WITHDRAW_AMOUNT
        );
    }
}
