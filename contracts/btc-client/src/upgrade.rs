use crate::{view::ContractSummary, Contract, ContractExt};
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
