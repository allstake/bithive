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
    pub fn set_btc_light_client_id(&mut self, new_contract_id: AccountId) {
        self.assert_owner();
        self.btc_light_client_id = new_contract_id;
    }

    #[payable]
    pub fn set_bip322_verifier_id(&mut self, new_contract_id: Option<AccountId>) {
        self.assert_owner();
        self.bip322_verifier_id = new_contract_id;
    }

    #[payable]
    pub fn set_chain_signatures_id(&mut self, new_contract_id: AccountId) {
        self.assert_owner();
        self.chain_signatures_id = new_contract_id;
    }

    #[payable]
    pub fn set_n_confirmation(&mut self, n: u64) {
        self.assert_owner();
        require!(n > 0, "n_confirmation must be greater than 0");
        self.n_confirmation = n;
    }

    #[payable]
    pub fn set_withdrawal_waiting_time(&mut self, ms: u64) {
        self.assert_owner();
        require!(ms > 0, "withdrawal_waiting_time_ms must be greater than 0");
        self.withdrawal_waiting_time_ms = ms;
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
    pub fn set_solo_withdrawal_sequence_heights(&mut self, values: Vec<u16>) {
        self.assert_owner();
        require!(values.len() > 0, "values must be non-empty");
        self.solo_withdrawal_seq_heights = values;
    }
}

impl Contract {
    pub(crate) fn assert_owner(&self) {
        assert_one_yocto();
        require!(env::predecessor_account_id() == self.owner_id, "Not owner");
    }
}
