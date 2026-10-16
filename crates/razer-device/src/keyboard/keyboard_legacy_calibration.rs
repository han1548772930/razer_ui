//! Current mounted 740/746 magnetic calibration; the older optical ADC
//! implementation shares event names but is not the selected feature owner.
pub use crate::keyboard_submission::Target;
use crate::{backend::FeatureTransport, device_reads::DeviceReadCapability, feature_exchange};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::OnceLock;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Operation {
    Bottom(u8),
    Top(u8),
    VerifyBottom(u8),
    VerifyTop(u8),
    Stop,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Message {
    pub action: String,
    pub payload: Value,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Report {
    pub messages: Vec<Message>,
    pub verified_top: bool,
    pub completed: bool,
    pub error: Option<String>,
}
pub fn capability(product_id: u32) -> anyhow::Result<DeviceReadCapability> {
    ensure!(
        [740, 746].contains(&product_id),
        "Current magnetic calibration caller is absent"
    );
    crate::keyboard_metadata::capability(product_id)
        .cloned()
        .context("Current magnetic calibration transport factory is absent")
}
#[derive(Deserialize)]
struct Key {
    input_id: String,
    analog_id: u8,
    #[serde(default)]
    input_type: String,
}
fn key(product_id: u32, input: &str) -> anyhow::Result<&'static Key> {
    capability(product_id)?;
    let input = if input == "KEY_WINDOWS" {
        "KEY_LEFT_GUI"
    } else {
        input
    };
    #[derive(Deserialize)]
    struct Product {
        product_id: u32,
        keys: Vec<Key>,
    }
    #[derive(Deserialize)]
    struct Catalog {
        products: Vec<Product>,
    }
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG
        .get_or_init(|| {
            serde_json::from_str(include_str!(
                "../../../../assets/data/keyboard-analog-keys.json"
            ))
            .expect("current literal analog keys")
        })
        .products
        .iter()
        .find(|product| product.product_id == product_id)
        .and_then(|product| product.keys.iter().find(|key| key.input_id == input))
        .context("Calibration input has no current analog key identity")
}
pub fn key_id(product_id: u32, input: &str, japanese: bool) -> anyhow::Result<u8> {
    capability(product_id)?;
    if input == "KEY_FN" {
        return Ok(59);
    }
    if japanese && input == "KEY_BACKSLASH" {
        return Ok(42);
    }
    Ok(key(product_id, input)?.analog_id)
}
pub fn key_record(product_id: u32, input: &str) -> anyhow::Result<(&'static str, &'static str)> {
    capability(product_id)?;
    if input == "KEY_FN" {
        return Ok(("KEY_FN", "KeyInput"));
    }
    let key = key(product_id, input)?;
    Ok((
        &key.input_id,
        if key.input_type.is_empty() {
            "KeyInput"
        } else {
            &key.input_type
        },
    ))
}
pub fn operation(
    product_id: u32,
    message: &Value,
    japanese: bool,
) -> anyhow::Result<Option<Operation>> {
    let Some(action) = message["type"].as_str() else {
        return Ok(None);
    };
    if action == "ON_STOP_CALIBRATION" {
        capability(product_id)?;
        return Ok(Some(Operation::Stop));
    }
    let make: fn(u8) -> Operation = match action {
        "ON_START_BOTTOM_ANALOG_CALIBRATION" => Operation::Bottom,
        "ON_START_TOP_ANALOG_CALIBRATION" => Operation::Top,
        "ON_VERIFY_BOTTOM_ANALOG_CALIBRATION" => Operation::VerifyBottom,
        "ON_VERIFY_TOP_ANALOG_CALIBRATION" => Operation::VerifyTop,
        _ => return Ok(None),
    };
    let input = message["payload"][0]
        .as_str()
        .context("Calibration input ID is missing")?;
    Ok(Some(make(key_id(product_id, input, japanese)?)))
}
pub fn execute(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    operation: &Operation,
    mut next: impl FnMut() -> anyhow::Result<u8>,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Report> {
    capability(cap.product_id)?;
    let mut report = Report {
        messages: vec![],
        verified_top: false,
        completed: false,
        error: None,
    };
    let mut send = |command, payload: &[u8]| {
        feature_exchange::exchange(device, cap, command, payload, next()?, validate)
    };
    let result = (|| -> anyhow::Result<bool> {
        match *operation {
            Operation::Bottom(key) => {
                send([2, 0, 4], &[2, 0])?;
                send([1, 254, 46], &[key])?;
                Ok(true)
            }
            Operation::Top(key) => {
                send([1, 254, 45], &[key])?;
                Ok(true)
            }
            Operation::VerifyBottom(key) | Operation::VerifyTop(key) => {
                let command = if matches!(operation, Operation::VerifyBottom(_)) {
                    [2, 254, 174]
                } else {
                    [2, 254, 173]
                };
                let data = send(command, &[key, 0])?;
                // Current getter creates jsonData only for >=2 returned bytes.
                Ok(data.get(1) == Some(&0))
            }
            Operation::Stop => {
                send([2, 0, 4], &[3, 0])?;
                Ok(true)
            }
        }
    })();
    let success = match result {
        Ok(success) => success,
        Err(error) => {
            report.error = Some(format!("{error:#}"));
            false
        }
    };
    if !success && !matches!(operation, Operation::Stop) {
        match send([2, 0, 4], &[3, 0]) {
            Ok(_) => report.messages.push(Message {
                action: "MW_UPDATE_ANALOG_CALIBRATION_STATUS_TO_UI".into(),
                payload: json!({"status":"stop","isSuccess":true}),
            }),
            Err(error) => {
                let error = format!("Stop calibration: {error:#}");
                report.error = Some(match report.error.take() {
                    Some(previous) => format!("{previous}; {error}"),
                    None => error,
                });
                report.messages.push(Message {
                    action: "MW_UPDATE_ANALOG_CALIBRATION_STATUS_TO_UI".into(),
                    payload: json!({"status":"stop","isSuccess":false}),
                });
            }
        }
    }
    let (action, payload) = match operation {
        Operation::Bottom(_) => (
            "MW_UPDATE_BOTTOM_ANALOG_CALIBRATION_STATUS_TO_UI",
            json!(success),
        ),
        Operation::Top(_) => (
            "MW_UPDATE_TOP_ANALOG_CALIBRATION_STATUS_TO_UI",
            json!(success),
        ),
        Operation::VerifyBottom(_) => (
            "MW_UPDATE_ANALOG_ADC_VERIFICATION_STATUS_TO_UI",
            json!({"direction":"bottom","isSuccess":success}),
        ),
        Operation::VerifyTop(_) => (
            "MW_UPDATE_ANALOG_ADC_VERIFICATION_STATUS_TO_UI",
            json!({"direction":"top","isSuccess":success}),
        ),
        Operation::Stop => (
            "MW_UPDATE_ANALOG_CALIBRATION_STATUS_TO_UI",
            json!({"status":"stop","isSuccess":success}),
        ),
    };
    report.messages.push(Message {
        action: action.into(),
        payload,
    });
    report.verified_top = success && matches!(operation, Operation::VerifyTop(_));
    report.completed = report.error.is_none();
    Ok(report)
}

#[cfg(test)]
#[path = "keyboard_legacy_calibration_tests.rs"]
mod tests;
