mod btc;
mod current_account_id;
mod time;

pub use btc::*;
pub use current_account_id::*;
use near_sdk::{env, require, Gas};
pub use time::*;

const ERR_NOT_ENOUGH_GAS: &str = "Not enough gas";

pub fn assert_gas(expected: Gas) {
    require!(env::prepaid_gas() >= expected, ERR_NOT_ENOUGH_GAS);
}
