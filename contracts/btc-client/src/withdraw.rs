use crate::*;
use bitcoin::{consensus::encode::deserialize_hex, Psbt, Transaction};
use events::Event;
use ext::{
    ext_btc_lightclient, ext_chain_signature, SignRequest, SignatureResponse,
    GAS_LIGHTCLIENT_VERIFY,
};
use near_sdk::{
    env::{self},
    near_bindgen, promise_result_as_success, require, Gas, Promise, PromiseError,
};
use serde::{Deserialize, Serialize};
use types::output_id;
use utils::{get_embed_message, get_hash_to_sign, verify_signed_message_unisat};

const CHAIN_SIGNATURE_PATH: &str = "btc/v1";
const CHAIN_SIGNATURE_KEY_VERSION: u32 = 0; // TODO ??

const WITHDRAW_MSG_HEX: &str = "616c6c7374616b652e7769746864726177"; // "allstake.withdraw"

const GAS_CHAIN_SIG_SIGN: Gas = Gas(250 * Gas::ONE_TERA.0);
const GAS_CHAIN_SIG_SIGN_CB: Gas = Gas(30 * Gas::ONE_TERA.0);
const GAS_WITHDRAW_VERIFY_CB: Gas = Gas(30 * Gas::ONE_TERA.0);

const ERR_INVALID_PSBT_HEX: &str = "Invalid PSBT hex";
const ERR_NOT_ONLY_ONE_INPUT: &str = "Withdraw txn must have only 1 input";
const ERR_WITHDRAW_NOT_READY: &str = "Not ready to withdraw now";
const ERR_INVALID_EMBED_VOUT: &str = "Invalid embed output vout";
const ERR_BAD_EMBED_MSG: &str = "Wrong embed message";
const ERR_CHAIN_SIG_FAILED: &str = "Failed to sign via chain signature";
const ERR_INVALID_SIGNATURE: &str = "Invalid signature result";
const ERR_INVALID_WITHDRAW_SIG: &str = "Invalid signature for withdraw";

const ERR_INVALID_TX_HEX: &str = "Invalid txn hex";

/// in case different wallet signs message in different form,
/// the signer needs to explicitly specify the type
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub enum SigType {
    Unisat,
}

#[near_bindgen]
impl Contract {
    /// Submit a queue withdraw request for a user
    /// ### Arguments
    /// * `user_pubkey` - hex encoded user pub key
    /// * `deposit_tx_id` - id of transaction that contains the deposit to withdraw
    /// * `deposit_vout` - deposit output vout to withdraw
    /// * `msg_sig` - hex encoded signature of queue withdraw message that shoud match `user_pubkey`
    /// * `sig_type` - signature type
    pub fn queue_withdraw(
        &mut self,
        user_pubkey: String,
        deposit_tx_id: String,
        deposit_vout: u64,
        msg_sig: String,
        sig_type: SigType,
    ) {
        // verify msg signature
        let expected_withdraw_msg = self.withdraw_message(&deposit_tx_id, deposit_vout);
        let (msg, valid) = match sig_type {
            SigType::Unisat => verify_signed_message_unisat(
                &expected_withdraw_msg.into_bytes(),
                &hex::decode(&msg_sig).unwrap(),
                &hex::decode(&user_pubkey).unwrap(),
            ),
        };
        require!(valid, ERR_INVALID_WITHDRAW_SIG);

        let mut account = self.get_account(&user_pubkey);
        let mut deposit = account.get_active_deposit(&deposit_tx_id, deposit_vout);
        deposit.queue_withdraw(hex::encode(msg), msg_sig);
        account.remove_active_deposit(&deposit);
        account.insert_queue_withdraw_deposit(deposit);
        self.set_account(account);

        Event::QueueWithdraw {
            user_pubkey: &user_pubkey,
            deposit_tx_id: &deposit_tx_id,
            deposit_vout: deposit_vout.into(),
        }
        .emit();
    }

    /// Sign a BTC withdraw PSBT via chain signature
    /// ### Arguments
    /// * `psbt_hex` - hex encoded PSBT to sign
    /// * `user_pubkey` - user public key
    /// * `embed_vout` - vout of embed output (OP_RETURN)
    #[payable]
    pub fn sign_withdraw(
        &mut self,
        psbt_hex: String,
        user_pubkey: String,
        embed_vout: u64,
    ) -> Promise {
        let psbt_bytes = hex::decode(psbt_hex).unwrap();
        let psbt = Psbt::deserialize(&psbt_bytes).expect(ERR_INVALID_PSBT_HEX);

        // verify it is a valid withdraw transaction
        self.verify_withdraw_transaction(&psbt.unsigned_tx, &user_pubkey, embed_vout);

        let input = psbt.unsigned_tx.input.first().unwrap();

        // request signature from chain signature
        let payload = get_hash_to_sign(&psbt, 0);
        let req = SignRequest {
            payload,
            path: CHAIN_SIGNATURE_PATH.to_string(),
            key_version: CHAIN_SIGNATURE_KEY_VERSION,
        };
        ext_chain_signature::ext(self.chain_signature_id.clone())
            .with_static_gas(GAS_CHAIN_SIG_SIGN)
            .with_attached_deposit(env::attached_deposit())
            .sign(req)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_CHAIN_SIG_SIGN_CB)
                    .on_sign_withdraw(
                        user_pubkey,
                        input.previous_output.txid.to_string(),
                        input.previous_output.vout.into(),
                    ),
            )
    }

    #[private]
    pub fn on_sign_withdraw(
        &self,
        user_pubkey: String,
        deposit_tx_id: String,
        deposit_vout: u64,
    ) -> SignatureResponse {
        let result_bytes = promise_result_as_success().expect(ERR_CHAIN_SIG_FAILED);
        let sig = serde_json::from_slice::<SignatureResponse>(&result_bytes)
            .expect(ERR_INVALID_SIGNATURE);

        Event::SignWithdraw {
            user_pubkey: &user_pubkey,
            deposit_tx_id: &deposit_tx_id,
            deposit_vout: deposit_vout.into(),
        }
        .emit();

        sig
    }

    /// Submit a BTC withdraw (either solo or multisig) transaction
    /// ### Arguments
    /// * `tx_hex` - hex encoded transaction body
    /// * `user_pubkey` - user public key
    /// * `embed_vout` - vout of embed output (OP_RETURN)
    /// * `tx_block_hash` - block hash in which the transaction is included
    /// * `tx_index` - transaction index in the block
    /// * `merkle_proof` - merkle proof of transaction in the block
    pub fn submit_withdraw_tx(
        &mut self,
        tx_hex: String,
        user_pubkey: String,
        embed_vout: u64,
        tx_block_hash: String,
        tx_index: u64,
        merkle_proof: Vec<String>,
    ) -> Promise {
        let tx = deserialize_hex::<Transaction>(&tx_hex).expect(ERR_INVALID_TX_HEX);
        let txid = tx.compute_txid();
        self.verify_withdraw_transaction(&tx, &user_pubkey, embed_vout);

        let deposit_utxo = tx.input.first().unwrap().previous_output;
        let deposit_tx_id = deposit_utxo.txid;
        let deposit_vout = deposit_utxo.vout;

        // verify confirmation through btc light client
        ext_btc_lightclient::ext(self.btc_lightclient_id.clone())
            .with_static_gas(GAS_LIGHTCLIENT_VERIFY)
            .verify_transaction_inclusion(
                txid.to_string(),
                tx_block_hash,
                tx_index,
                merkle_proof,
                self.n_confirmation,
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_WITHDRAW_VERIFY_CB)
                    .on_verify_withdraw_tx(
                        user_pubkey,
                        txid.to_string(),
                        deposit_tx_id.to_string(),
                        deposit_vout.into(),
                    ),
            )
    }

    #[private]
    pub fn on_verify_withdraw_tx(
        &mut self,
        user_pubkey: String,
        withdraw_tx_id: String,
        deposit_tx_id: String,
        deposit_vout: u64,
        #[callback_result] result: Result<bool, PromiseError>,
    ) -> bool {
        let valid = result.unwrap_or(false);
        if valid {
            let mut account = self.get_account(&user_pubkey);
            let mut deposit = account.get_queue_withdraw_deposit(&deposit_tx_id, deposit_vout);
            deposit.complete_withdraw(withdraw_tx_id.clone());
            account.remove_queue_withdraw_deposit(&deposit);
            account.insert_withdrawn_deposit(deposit);
            self.set_account(account);

            Event::CompleteWithdraw {
                user_pubkey: &user_pubkey,
                withdraw_tx_id: &withdraw_tx_id,
                deposit_tx_id: &deposit_tx_id,
                deposit_vout: deposit_vout.into(),
            }
            .emit();
        } else {
            Event::CompleteWithdrawFailed {
                user_pubkey: &user_pubkey,
                withdraw_tx_id: &withdraw_tx_id,
                deposit_tx_id: &deposit_tx_id,
                deposit_vout: deposit_vout.into(),
            }
            .emit();
        }

        valid
    }
}

impl Contract {
    fn withdraw_message(&self, deposit_tx_id: &String, deposit_vout: u64) -> String {
        format!(
            "allstake.withdraw:{}",
            output_id(deposit_tx_id, deposit_vout)
        )
    }

    fn verify_withdraw_transaction(&self, tx: &Transaction, pubkey: &String, embed_vout: u64) {
        // right now we ask withdraw transactions to have only 1 input,
        // which is the deposit UTXO
        require!(tx.input.len() == 1, ERR_NOT_ONLY_ONE_INPUT);
        let input = tx.input.first().unwrap();

        // input UTXO must be in user's withdraw queue
        let account = self.get_account(pubkey);
        let deposit = account.get_queue_withdraw_deposit(
            &input.previous_output.txid.to_string(),
            input.previous_output.vout.into(),
        );
        // make sure queue waiting time has passed
        require!(
            deposit.can_complete_withdraw(self.withdraw_waiting_time_ms),
            ERR_WITHDRAW_NOT_READY
        );
        // embed message
        let msg = get_embed_message(
            tx.output
                .get(embed_vout as usize)
                .expect(ERR_INVALID_EMBED_VOUT),
        );
        require!(msg == WITHDRAW_MSG_HEX, ERR_BAD_EMBED_MSG);

        // we don't care how the input is spent, since the user has to sign it himself as well
    }
}

#[cfg(test)]
mod tests {
    // TODO unit test verify_withdraw_transaction
}
