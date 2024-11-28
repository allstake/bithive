use near_sdk::{near_bindgen, Timestamp};

use crate::*;

#[cfg(feature = "test")]
pub const TIMESTAMP_STORAGE_KEY: &[u8] = "__test_ts__".as_bytes();

pub fn current_timestamp_ms() -> Timestamp {
    #[cfg(feature = "test")]
    {
        near_sdk::env::storage_read(TIMESTAMP_STORAGE_KEY)
            .as_deref()
            .map(near_sdk::borsh::BorshDeserialize::try_from_slice)
            .map(Result::unwrap)
            .unwrap_or_default()
    }

    #[cfg(not(feature = "test"))]
    near_sdk::env::block_timestamp_ms()
}

#[cfg(feature = "test")]
pub fn fast_forward_ms(duration: u64) {
    near_sdk::env::storage_write(
        TIMESTAMP_STORAGE_KEY,
        &near_sdk::borsh::BorshSerialize::try_to_vec(&(current_timestamp_ms() + duration)).unwrap(),
    );
}

#[near_bindgen]
impl Contract {
    #[cfg(feature = "test")]
    pub fn fast_forward(&mut self, duration: u64) {
        fast_forward_ms(duration);
    }

    #[cfg(feature = "test")]
    pub fn get_current_timestamp(&self) -> Timestamp {
        current_timestamp_ms()
    }
}
