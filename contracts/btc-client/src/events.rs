use near_sdk::json_types::U64;
use near_sdk::log;
use near_sdk::serde::Serialize;
use near_sdk::serde_json::json;

pub const EVENT_STANDARD: &str = "allstake.btc";
pub const EVENT_STANDARD_VERSION: &str = "1.0.0";

#[derive(Serialize)]
#[serde(
    crate = "near_sdk::serde",
    rename_all = "snake_case",
    tag = "event",
    content = "data"
)]
#[must_use = "Don't forget to `.emit()` this event"]
pub enum Event<'a> {
    Deposit {
        user_pubkey: &'a String,
        tx_id: &'a String,
        deposit_vout: U64,
        value: U64,
    },
    QueueWithdraw {
        user_pubkey: &'a String,
        deposit_tx_id: &'a String,
        deposit_vout: U64,
    },
    SignWithdraw {
        user_pubkey: &'a String,
        deposit_tx_id: &'a String,
        deposit_vout: U64,
    },
    CompleteWithdraw {
        user_pubkey: &'a String,
        withdraw_tx_id: &'a String,
        deposit_tx_id: &'a String,
        deposit_vout: U64,
    },
    CompleteWithdrawFailed {
        user_pubkey: &'a String,
        withdraw_tx_id: &'a String,
        deposit_tx_id: &'a String,
        deposit_vout: U64,
    },
}

impl<'a> Event<'a> {
    pub fn emit(&self) {
        let json = json!(self);
        let event_json = json!({
            "standard": EVENT_STANDARD,
            "version": EVENT_STANDARD_VERSION,
            "event": json["event"],
            "data": [json["data"]]
        })
        .to_string();
        log!("EVENT_JSON:{}", event_json);
    }
}
