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
    near_bindgen, promise_result_as_success, require, Gas, Promise, PromiseError,
};
use serde::{Deserialize, Serialize};
use types::{output_id, PubKey, RedeemVersion, TxId};
use utils::{assert_gas, get_embed_message, get_hash_to_sign, verify_signed_message_unisat};

const WITHDRAW_MSG_HEX: &str = "616c6c7374616b652e7769746864726177"; // "allstake.withdraw"

const GAS_CHAIN_SIG_SIGN: Gas = Gas(250 * Gas::ONE_TERA.0);
const GAS_CHAIN_SIG_SIGN_CB: Gas = Gas(10 * Gas::ONE_TERA.0);
const GAS_WITHDRAW_VERIFY_CB: Gas = Gas(30 * Gas::ONE_TERA.0);

const ERR_INVALID_PSBT_HEX: &str = "Invalid PSBT hex";
const ERR_NOT_ONLY_ONE_INPUT: &str = "Withdraw txn must have only 1 input";
const ERR_WITHDRAW_NOT_READY: &str = "Not ready to withdraw now";
const ERR_INVALID_EMBED_VOUT: &str = "Invalid embed output vout";
const ERR_BAD_EMBED_MSG: &str = "Wrong embed message";
const ERR_CHAIN_SIG_FAILED: &str = "Failed to sign via chain signature";
const ERR_INVALID_SIGNATURE: &str = "Invalid signature result";

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
        let tx_id: TxId = deposit_tx_id.clone().into();
        // verify msg signature
        let expected_withdraw_msg = self.withdraw_message(&tx_id, deposit_vout);
        let msg = match sig_type {
            SigType::Unisat => verify_signed_message_unisat(
                &expected_withdraw_msg.into_bytes(),
                &hex::decode(&msg_sig).unwrap(),
                &hex::decode(&user_pubkey).unwrap(),
            ),
        };

        let mut account = self.get_account(&user_pubkey.clone().into());
        let mut deposit = account.remove_active_deposit(&tx_id, deposit_vout);
        deposit.queue_withdraw(hex::encode(msg), msg_sig);
        account.insert_queue_withdraw_deposit(deposit);
        self.set_account(account);

        Event::QueueWithdraw {
            user_pubkey: &user_pubkey,
            deposit_tx_id: &deposit_tx_id,
            deposit_vout: deposit_vout.into(),
        }
        .emit();
    }

    /// Sign a BTC withdraw PSBT via chain signature for multisig withdraw
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
        assert_gas(Gas(40 * Gas::ONE_TERA.0) + GAS_CHAIN_SIG_SIGN + GAS_CHAIN_SIG_SIGN_CB); // 300 Tgas

        let psbt_bytes = hex::decode(psbt_hex).unwrap();
        let psbt = Psbt::deserialize(&psbt_bytes).expect(ERR_INVALID_PSBT_HEX);

        // verify it is a valid withdraw transaction
        self.verify_withdraw_transaction(&psbt.unsigned_tx, embed_vout);

        // for multisig withraw, input UTXO must be in user's withdraw queue
        let input = psbt.unsigned_tx.input.first().unwrap();
        let account = self.get_account(&user_pubkey.clone().into());
        let deposit = account.get_queue_withdraw_deposit(
            &input.previous_output.txid.to_string().into(),
            input.previous_output.vout.into(),
        );
        // make sure queue waiting time has passed
        require!(
            deposit.can_complete_withdraw(self.withdraw_waiting_time_ms),
            ERR_WITHDRAW_NOT_READY
        );

        // request signature from chain signature
        let payload = get_hash_to_sign(&psbt, 0);
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
    /// * `deposit_vin` - vin of the deposit UTXO
    /// * `tx_block_hash` - block hash in which the transaction is included
    /// * `tx_index` - transaction index in the block
    /// * `merkle_proof` - merkle proof of transaction in the block
    pub fn submit_withdraw_tx(
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
            .try_get_queue_withdraw_deposit(&deposit_txid, deposit_vout)
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
                    .on_verify_withdraw_tx(
                        user_pubkey,
                        txid.to_string(),
                        deposit_txid.to_string(),
                        deposit_vout,
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
            let pk: PubKey = user_pubkey.clone().into();
            let tx_id: TxId = deposit_tx_id.clone().into();
            let mut account = self.get_account(&pk);
            let mut deposit = account
                .try_remove_queue_withdraw_deposit(&tx_id, deposit_vout)
                .unwrap_or_else(|| account.remove_active_deposit(&tx_id, deposit_vout));

            deposit.complete_withdraw(withdraw_tx_id.clone());
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
    fn withdraw_message(&self, deposit_tx_id: &TxId, deposit_vout: u64) -> String {
        format!(
            "allstake.withdraw:{}",
            output_id(deposit_tx_id, deposit_vout)
        )
    }

    /// NOTE: in the future we can remove this function to allow the user spend his deposit in any way
    pub fn verify_withdraw_transaction(&self, tx: &Transaction, embed_vout: u64) {
        // right now we ask withdraw transactions to have only 1 input,
        // which is the deposit UTXO
        require!(tx.input.len() == 1, ERR_NOT_ONLY_ONE_INPUT);

        // verify embed message
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
    use bitcoin::Psbt;

    use crate::tests::test_contract_instance;

    fn test_verify_withdraw_tx(psbt_hex: &str, embed_vout: u64) {
        let contract = test_contract_instance();
        let psbt_bytes = hex::decode(psbt_hex).unwrap();
        let psbt = Psbt::deserialize(&psbt_bytes).expect("invalid psbt");

        contract.verify_withdraw_transaction(&psbt.unsigned_tx, embed_vout);
    }

    #[test]
    #[should_panic(expected = "Withdraw txn must have only 1 input")]
    fn test_verify_withdraw_txn_many_inputs() {
        let psbt_hex = "70736274ff01009a02000000021cc22b0a9be31b9d6b368c2497af275d93e495dadfd477df1b841c2bc5bf64700000000000fffffffff11f61d5ed2918d9d1d2fcab4b649259ad831d997b42c4aa513ee8ce66860ec800000000000500000002a0860100000000001600144f7277040129f03adebe9fac4a6d78eb6519f2080000000000000000166a14616c6c7374616b652e77697468647261772e763100000000000100de02000000000101a59464afb58400ae57b90f41f5f0599693913eba32d474a0576d4edb7d483c950000000000feffffff02e0930400000000001600144f7277040129f03adebe9fac4a6d78eb6519f2084d83f349000000001600149ad303910b681fb863e0a958952d9c7066927f7202473044022021e99bb47c160def7ef45d3d3ece23826e1d3383c3f37b067b56f01a026ad6d10220611aabc75b42f7668040d494592ea7b73c307f2d703cd2ce49d784dfc60a6de6012103da2e202cdd314b4f2d8307e61c65ca515b905d64d7d7a080ea7f701227995a65110200000001012b400d0300000000002200204882ff0d0c9be26d5c8e07f8cc75217643295b235ebf2fee178ce79d7f3e846d0105706355b275210309eb861d8b3315e7fef32693154f290cbd5ab2a218050d1167704cd72b8a41e2ac6752210309eb861d8b3315e7fef32693154f290cbd5ab2a218050d1167704cd72b8a41e22103a4ba81571a98b63a61d147bd4cf7e8b470c38d74c025876160718950644af86a52ae68000000";
        test_verify_withdraw_tx(psbt_hex, 1);
    }

    #[test]
    #[should_panic(expected = "Embed output is not OP_RETURN")]
    fn test_verify_withdraw_txn_invalid_embed() {
        let psbt_hex = "70736274ff01007102000000015cabce7e6ebcbea0902079ebd9b21fcb3f22f98f7040f04f979bf927d518be4b0000000000ffffffff02a0860100000000001600141239496b9c08c4ed6f4f1dd81a265364d369ae920000000000000000166a14616c6c7374616b652e77697468647261772e7631000000000001012b400d0300000000002200200db69ffd665670b0e98a6eaf2ac89d46f42975da7d34c4fa97d76ffe3bf7d70e0105706355b2752102f23fe849508454e2c619dafcd0fb8b9e0e8e71d87edabac14d8301c80a51ba78ac67522102f23fe849508454e2c619dafcd0fb8b9e0e8e71d87edabac14d8301c80a51ba782102204fd62ad9e4beffb97dce178fd19fe068d7330b310c59425d61137d2a881d5952ae68000000";
        test_verify_withdraw_tx(psbt_hex, 0);
    }

    #[test]
    #[should_panic(expected = "Wrong embed message")]
    fn test_verify_withdraw_txn_bad_embed_msg() {
        let psbt_hex = "70736274ff0100700200000001d2ed7dbd2449ab0167abb09fef40060baa1b97e03f8e4831b0417d209503c74600000000000500000002a086010000000000160014cf31baf1968efea9f5c67b103b5526d665b5dc680000000000000000156a13616c6c7374616b652e77697468647261772e76000000000001012b400d0300000000002200206d7373492084677836b475fd89e2159b71b0f29b90b035139334835b0d045f660105706355b2752103b89fbea33ae6f1993da2c528184f0a84c06f06f01110bc6eaaddc90dd8181465ac67522103b89fbea33ae6f1993da2c528184f0a84c06f06f01110bc6eaaddc90dd81814652102c19d7d28615247d83b14d10241ac2e67e640e590ba093d532d71b8ae252c738b52ae68000000";
        test_verify_withdraw_tx(psbt_hex, 1);
    }

    #[test]
    fn test_verify_withdraw_valid_tx() {
        let psbt_hex = "70736274ff01006e02000000018695e224b5666a3904d6d22656f4b2ebb07a768ce6745a1f53d865152da957ad0000000000ffffffff02a0860100000000001600145759e59230fa08911345d15d5e37405a7b833cb10000000000000000136a11616c6c7374616b652e7769746864726177000000000001012b400d030000000000220020c749cb3a3d45d4c29c534a55e7d3569ece87a8c1369b4b3da33ef2be102a9aa20105706355b2752102ff95f611148f2ffc6faf493b052e2e4383e2a9dba5fa8a2b400e954562e1989eac67522102ff95f611148f2ffc6faf493b052e2e4383e2a9dba5fa8a2b400e954562e1989e2102fa49b109a4f1decb4fae3676a8a1f35e201d49d71c24b52e004fb10cb0a0aa5b52ae68000000";
        test_verify_withdraw_tx(psbt_hex, 1);
    }
}
