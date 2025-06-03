use crate::{account::VersionedAccount, view::ContractSummary, Contract, ContractExt};
use near_sdk::{
    assert_one_yocto, env, near_bindgen, Gas, GasWeight, Promise, PromiseOrValue, ONE_YOCTO,
};

#[near_bindgen]
impl Contract {
    #[init(ignore_state)]
    #[payable]
    #[private]
    pub fn migrate() -> Self {
        assert_one_yocto();
        env::state_read::<Self>().expect("Failed to read contract state")
    }

    #[payable]
    pub fn upgrade(&mut self) -> PromiseOrValue<ContractSummary> {
        self.assert_owner();
        let code = env::input().expect("Code not found");
        Promise::new(env::current_account_id())
            .deploy_contract(code)
            .function_call_weight("migrate".into(), vec![], ONE_YOCTO, Gas(0), GasWeight(1))
            .function_call_weight(
                "get_summary".into(),
                vec![],
                0,
                Gas(10 * Gas::ONE_TERA.0),
                GasWeight(0),
            )
            .into()
    }
}

#[near_bindgen]
impl Contract {
    /// Whether the account needs to be migrated before doing view calls.
    /// To migrate, call `migrate_account` function.
    pub fn need_migrate_account(&self, user_pubkey: String) -> bool {
        if let Some(account) = self.accounts.get(&user_pubkey.into()) {
            return match account {
                VersionedAccount::V1(v1) => v1.pending_sign_psbt.is_some(),
                VersionedAccount::Current(_) => false,
            };
        }

        false
    }

    /// This helps to migrate old accounts to the latest version.
    /// If the function returns false, it means there is no need to migrate.
    pub fn migrate_account(&mut self, user_pubkey: String) {
        if self.need_migrate_account(user_pubkey.clone()) {
            let account = self.get_account(&user_pubkey.into());
            self.set_account(account);
        }
    }
}
