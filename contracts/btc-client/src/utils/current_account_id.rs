use crate::*;
use near_sdk::{near_bindgen, AccountId};

#[cfg(feature = "test")]
pub const ACCOUNT_ID_STORAGE_KEY: &[u8] = "__test_account_id__".as_bytes();

/// Returns the deployed contract ID when in production
/// or a configurable value in integration tests
pub fn current_account_id() -> AccountId {
    #[cfg(feature = "test")]
    {
        near_sdk::env::storage_read(ACCOUNT_ID_STORAGE_KEY)
            .as_deref()
            .map(near_sdk::borsh::BorshDeserialize::try_from_slice)
            .map(Result::unwrap)
            .unwrap_or_else(near_sdk::env::current_account_id)
    }

    #[cfg(not(feature = "test"))]
    near_sdk::env::current_account_id()
}

#[near_bindgen]
impl Contract {
    #[cfg(feature = "test")]
    pub fn set_current_account_id(&mut self, id: AccountId) {
        near_sdk::env::storage_write(
            ACCOUNT_ID_STORAGE_KEY,
            &near_sdk::borsh::BorshSerialize::try_to_vec(&id).unwrap(),
        );
    }
}
