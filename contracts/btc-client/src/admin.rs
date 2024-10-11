use near_sdk::assert_one_yocto;

use crate::*;

#[near_bindgen]
impl Contract {
    #[payable]
    pub fn change_owner(&mut self, new_owner_id: AccountId) {
        self.assert_owner();
        self.owner_id = new_owner_id;
    }

    #[payable]
    pub fn set_btc_lightclient_id(&mut self, new_contract_id: AccountId) {
        self.assert_owner();
        self.btc_lightclient_id = new_contract_id;
    }

    #[payable]
    pub fn set_chain_signature_id(&mut self, new_contract_id: AccountId) {
        self.assert_owner();
        self.chain_signature_id = new_contract_id;
    }

    #[payable]
    pub fn set_n_confirmation(&mut self, n: u64) {
        self.assert_owner();
        self.n_confirmation = n;
    }

    #[payable]
    pub fn set_withdraw_waiting_time(&mut self, ms: u64) {
        self.assert_owner();
        self.withdraw_waiting_time_ms = ms;
    }

    #[payable]
    pub fn set_min_deposit_satoshi(&mut self, min_deposit_satoshi: u64) {
        self.assert_owner();
        self.min_deposit_satoshi = min_deposit_satoshi;
    }

    #[payable]
    pub fn set_earliest_deposit_block_height(&mut self, height: u32) {
        self.assert_owner();
        self.earliest_deposit_block_height = height;
    }

    #[payable]
    pub fn set_solo_withdraw_sequence_heights(&mut self, values: Vec<u16>) {
        self.assert_owner();
        self.solo_withdraw_seq_heights = values;
    }
}

impl Contract {
    fn assert_owner(&self) {
        assert_one_yocto();
        require!(env::predecessor_account_id() == self.owner_id, "Not owner");
    }
}
