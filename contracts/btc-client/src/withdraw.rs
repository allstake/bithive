use crate::*;
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
use types::{output_id, PubKey, RedeemVersion, TxId};
use utils::{assert_gas, get_hash_to_sign, verify_signed_message_unisat};

const GAS_CHAIN_SIG_SIGN: Gas = Gas(250 * Gas::ONE_TERA.0);
const GAS_CHAIN_SIG_SIGN_CB: Gas = Gas(10 * Gas::ONE_TERA.0);
const GAS_WITHDRAW_VERIFY_CB: Gas = Gas(30 * Gas::ONE_TERA.0);

const ERR_INVALID_PSBT_HEX: &str = "Invalid PSBT hex";
const ERR_WITHDRAW_NOT_READY: &str = "Not ready to withdraw now";
const ERR_CHAIN_SIG_FAILED: &str = "Failed to sign via chain signature";

const ERR_INVALID_TX_HEX: &str = "Invalid txn hex";
const ERR_BAD_DEPOSIT_VIN: &str = "Deposit vin not exist";

/// in case different wallet signs message in different form,
/// the signer needs to explicitly specify the type
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub enum SigType {
    Unisat,
}

#[near_bindgen]
impl Contract {
    /// Submit a queue withdrawal request for a user
    /// ### Arguments
    /// * `user_pubkey` - hex encoded user pub key
    /// * `deposit_tx_id` - id of transaction that contains the deposit to withdraw
    /// * `deposit_vout` - deposit output vout to withdraw
    /// * `msg_sig` - hex encoded signature of queue withdraw message that shoud match `user_pubkey`
    /// * `sig_type` - signature type
    pub fn queue_withdrawal(
        &mut self,
        user_pubkey: String,
        deposit_tx_id: String,
        deposit_vout: u64,
        msg_sig: String,
        sig_type: SigType,
    ) {
        let tx_id: TxId = deposit_tx_id.clone().into();
        // verify msg signature
        let expected_withdraw_msg = self.withdrawal_message(&tx_id, deposit_vout);
        let msg = match sig_type {
            SigType::Unisat => verify_signed_message_unisat(
                &expected_withdraw_msg.into_bytes(),
                &hex::decode(&msg_sig).unwrap(),
                &hex::decode(&user_pubkey).unwrap(),
            ),
        };

        let mut account = self.get_account(&user_pubkey.clone().into());
        let mut deposit = account.remove_active_deposit(&tx_id, deposit_vout);
        deposit.queue_withdrawal(hex::encode(msg), msg_sig);
        account.insert_queue_withdrawal_deposit(deposit);
        self.set_account(account);

        Event::QueueWithdrawal {
            user_pubkey: &user_pubkey,
            deposit_tx_id: &deposit_tx_id,
            deposit_vout: deposit_vout.into(),
        }
        .emit();
    }

    /// Sign a BTC withdrawal PSBT via chain signature for multisig withdraw
    /// ### Arguments
    /// * `psbt_hex` - hex encoded PSBT to sign
    /// * `user_pubkey` - user public key
    /// * `deposit_vin` - vin of deposit UTXO to be withdrawn
    #[payable]
    pub fn sign_withdrawal(
        &mut self,
        psbt_hex: String,
        user_pubkey: String,
        deposit_vin: u64,
    ) -> Promise {
        assert_gas(Gas(40 * Gas::ONE_TERA.0) + GAS_CHAIN_SIG_SIGN + GAS_CHAIN_SIG_SIGN_CB); // 300 Tgas

        let psbt_bytes = hex::decode(psbt_hex).unwrap();
        let psbt = Psbt::deserialize(&psbt_bytes).expect(ERR_INVALID_PSBT_HEX);

        // for multisig withraw, input UTXO must be in user's withdraw queue
        let input = psbt
            .unsigned_tx
            .input
            .get(deposit_vin as usize)
            .expect(ERR_BAD_DEPOSIT_VIN);
        let account = self.get_account(&user_pubkey.clone().into());
        let deposit = account.get_queue_withdrawal_deposit(
            &input.previous_output.txid.to_string().into(),
            input.previous_output.vout.into(),
        );
        // make sure queue waiting time has passed
        require!(
            deposit.can_complete_withdraw(self.withdraw_waiting_time_ms),
            ERR_WITHDRAW_NOT_READY
        );

        // request signature from chain signature
        let payload = get_hash_to_sign(&psbt, deposit_vin);
        let (path, key_version) = match deposit.redeem_version() {
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
                        input.previous_output.txid.to_string(),
                        input.previous_output.vout.into(),
                        env::predecessor_account_id(),
                        env::attached_deposit().into(),
                    ),
            )
    }

    #[private]
    pub fn on_sign_withdrawal(
        &self,
        user_pubkey: String,
        deposit_tx_id: String,
        deposit_vout: u64,
        caller_id: AccountId,
        attached_deposit: U128,
        #[callback_result] result: Result<SignatureResponse, PromiseError>,
    ) -> SignatureResponse {
        if let Ok(sig) = result {
            Event::SignWithdrawal {
                user_pubkey: &user_pubkey,
                deposit_tx_id: &deposit_tx_id,
                deposit_vout: deposit_vout.into(),
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
    /// * `deposit_vin` - vin of the deposit UTXO
    /// * `tx_block_hash` - block hash in which the transaction is included
    /// * `tx_index` - transaction index in the block
    /// * `merkle_proof` - merkle proof of transaction in the block
    pub fn submit_withdrawal_tx(
        &mut self,
        tx_hex: String,
        user_pubkey: String,
        deposit_vin: u64,
        tx_block_hash: String,
        tx_index: u64,
        merkle_proof: Vec<String>,
    ) -> Promise {
        assert_gas(Gas(40 * Gas::ONE_TERA.0) + GAS_LIGHTCLIENT_VERIFY + GAS_WITHDRAW_VERIFY_CB); // 100 Tgas

        let tx = deserialize_hex::<Transaction>(&tx_hex).expect(ERR_INVALID_TX_HEX);
        let txid = tx.compute_txid();

        let input = tx
            .input
            .get(deposit_vin as usize)
            .expect(ERR_BAD_DEPOSIT_VIN);
        let deposit_txid: TxId = input.previous_output.txid.to_string().into();
        let deposit_vout: u64 = input.previous_output.vout.into();
        let account = self.get_account(&user_pubkey.clone().into());
        // submitted txn could either be solo withdraw or multisig withdraw,
        // so we need to scan both sets
        let deposit = account
            .try_get_queue_withdrawal_deposit(&deposit_txid, deposit_vout)
            .unwrap_or_else(|| account.get_active_deposit(&deposit_txid, deposit_vout));
        require!(
            deposit.can_complete_withdraw(self.withdraw_waiting_time_ms),
            ERR_WITHDRAW_NOT_READY
        );

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
                    .on_verify_withdrawal_tx(
                        user_pubkey,
                        txid.to_string(),
                        deposit_txid.to_string(),
                        deposit_vout,
                    ),
            )
    }

    #[private]
    pub fn on_verify_withdrawal_tx(
        &mut self,
        user_pubkey: String,
        withdrawal_tx_id: String,
        deposit_tx_id: String,
        deposit_vout: u64,
        #[callback_result] result: Result<bool, PromiseError>,
    ) -> bool {
        let valid = result.unwrap_or(false);
        if valid {
            let pk: PubKey = user_pubkey.clone().into();
            let tx_id: TxId = deposit_tx_id.clone().into();
            let mut account = self.get_account(&pk);
            let mut deposit = account
                .try_remove_queue_withdrawal_deposit(&tx_id, deposit_vout)
                .unwrap_or_else(|| account.remove_active_deposit(&tx_id, deposit_vout));

            deposit.complete_withdraw(withdrawal_tx_id.clone());
            account.insert_withdrawn_deposit(deposit);
            self.set_account(account);

            Event::CompleteWithdrawal {
                user_pubkey: &user_pubkey,
                withdrawal_tx_id: &withdrawal_tx_id,
                deposit_tx_id: &deposit_tx_id,
                deposit_vout: deposit_vout.into(),
            }
            .emit();
        }

        valid
    }
}

impl Contract {
    pub(crate) fn withdrawal_message(&self, deposit_tx_id: &TxId, deposit_vout: u64) -> String {
        format!(
            "allstake.withdraw:{}",
            output_id(deposit_tx_id, deposit_vout)
        )
    }
}
