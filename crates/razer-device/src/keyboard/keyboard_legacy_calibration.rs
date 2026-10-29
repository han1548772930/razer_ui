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
pub struct ExchangeReceipt {
    pub command: [u8; 3],
    pub payload: Vec<u8>,
    pub acknowledgement: Option<Vec<u8>>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Report {
    pub messages: Vec<Message>,
    #[serde(default)]
    pub exchanges: Vec<ExchangeReceipt>,
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
#[derive(Deserialize)]
struct Product {
    product_id: u32,
    keys: Vec<Key>,
}

#[derive(Deserialize)]
struct Catalog {
    products: Vec<Product>,
}

fn key(product_id: u32, input: &str) -> anyhow::Result<&'static Key> {
    capability(product_id)?;
    let input = if input == "KEY_WINDOWS" {
        "KEY_LEFT_GUI"
    } else {
        input
    };
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG
        .get_or_init(|| make_keyboard_analog_keys_legacy_table())
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
        exchanges: vec![],
        verified_top: false,
        completed: false,
        error: None,
    };
    let mut send = |command, payload: &[u8]| {
        let result = next().and_then(|transaction| {
            feature_exchange::exchange(device, cap, command, payload, transaction, validate)
        });
        report.exchanges.push(ExchangeReceipt {
            command,
            payload: payload.to_vec(),
            acknowledgement: feature_exchange::acknowledged_reply(&result).map(<[u8]>::to_vec),
            error: result.as_ref().err().map(|error| format!("{error:#}")),
        });
        result
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

// Generated typed table: razer-device/keyboard_analog_keys_legacy.
#[rustfmt::skip]
fn make_keyboard_analog_keys_legacy_table() -> Catalog {
    Catalog{
        products: vec![
            Product{
                product_id: 679u32,
                keys: vec![
                    Key{input_id:"KEY_0".into(),analog_id:11u8,input_type:Default::default()},
                    Key{input_id:"KEY_1".into(),analog_id:2u8,input_type:Default::default()},
                    Key{input_id:"KEY_2".into(),analog_id:3u8,input_type:Default::default()},
                    Key{input_id:"KEY_3".into(),analog_id:4u8,input_type:Default::default()},
                    Key{input_id:"KEY_4".into(),analog_id:5u8,input_type:Default::default()},
                    Key{input_id:"KEY_5".into(),analog_id:6u8,input_type:Default::default()},
                    Key{input_id:"KEY_6".into(),analog_id:7u8,input_type:Default::default()},
                    Key{input_id:"KEY_7".into(),analog_id:8u8,input_type:Default::default()},
                    Key{input_id:"KEY_8".into(),analog_id:9u8,input_type:Default::default()},
                    Key{input_id:"KEY_9".into(),analog_id:10u8,input_type:Default::default()},
                    Key{input_id:"KEY_A".into(),analog_id:31u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_APOSTROPHE".into(),
                        analog_id: 41u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_APPLICATION".into(),
                        analog_id: 129u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_B".into(),analog_id:50u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_BACKSLASH".into(),
                        analog_id: 29u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_BACKSPACE".into(),
                        analog_id: 15u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_C".into(),analog_id:48u8,input_type:Default::default()},
                    Key{input_id:"KEY_CANCEL".into(),analog_id:125u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_CAPS_LOCK".into(),
                        analog_id: 30u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_CLOSE_SQUARE_BRACKET".into(),
                        analog_id: 28u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_COMMA".into(),analog_id:53u8,input_type:Default::default()},
                    Key{input_id:"KEY_D".into(),analog_id:33u8,input_type:Default::default()},
                    Key{input_id:"KEY_DELETE".into(),analog_id:76u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_DOWN_ARROW".into(),
                        analog_id: 84u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_DOWN_DPAD".into(),analog_id:0u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_DOWN_LEFT_ARROW".into(),
                        analog_id: 0u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_DOWN_RIGHT_ARROW".into(),
                        analog_id: 0u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_E".into(),analog_id:19u8,input_type:Default::default()},
                    Key{input_id:"KEY_END".into(),analog_id:81u8,input_type:Default::default()},
                    Key{input_id:"KEY_ENTER".into(),analog_id:43u8,input_type:Default::default()},
                    Key{input_id:"KEY_EQUAL".into(),analog_id:13u8,input_type:Default::default()},
                    Key{input_id:"KEY_ESC".into(),analog_id:110u8,input_type:Default::default()},
                    Key{input_id:"KEY_F".into(),analog_id:34u8,input_type:Default::default()},
                    Key{input_id:"KEY_F1".into(),analog_id:112u8,input_type:Default::default()},
                    Key{input_id:"KEY_F10".into(),analog_id:121u8,input_type:Default::default()},
                    Key{input_id:"KEY_F11".into(),analog_id:122u8,input_type:Default::default()},
                    Key{input_id:"KEY_F12".into(),analog_id:123u8,input_type:Default::default()},
                    Key{input_id:"KEY_F13".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F14".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F15".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F16".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F17".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F18".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F19".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F2".into(),analog_id:113u8,input_type:Default::default()},
                    Key{input_id:"KEY_F20".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F21".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F22".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F23".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F24".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F3".into(),analog_id:114u8,input_type:Default::default()},
                    Key{input_id:"KEY_F4".into(),analog_id:115u8,input_type:Default::default()},
                    Key{input_id:"KEY_F5".into(),analog_id:116u8,input_type:Default::default()},
                    Key{input_id:"KEY_F6".into(),analog_id:117u8,input_type:Default::default()},
                    Key{input_id:"KEY_F7".into(),analog_id:118u8,input_type:Default::default()},
                    Key{input_id:"KEY_F8".into(),analog_id:119u8,input_type:Default::default()},
                    Key{input_id:"KEY_F9".into(),analog_id:120u8,input_type:Default::default()},
                    Key{input_id:"KEY_G".into(),analog_id:35u8,input_type:Default::default()},
                    Key{input_id:"KEY_H".into(),analog_id:36u8,input_type:Default::default()},
                    Key{input_id:"KEY_HENKAN".into(),analog_id:154u8,input_type:Default::default()},
                    Key{input_id:"KEY_HOME".into(),analog_id:80u8,input_type:Default::default()},
                    Key{input_id:"KEY_HYPEN".into(),analog_id:12u8,input_type:Default::default()},
                    Key{input_id:"KEY_I".into(),analog_id:24u8,input_type:Default::default()},
                    Key{input_id:"KEY_INSERT".into(),analog_id:75u8,input_type:Default::default()},
                    Key{input_id:"KEY_J".into(),analog_id:37u8,input_type:Default::default()},
                    Key{input_id:"KEY_K".into(),analog_id:38u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_KATAKANA_HIRAGANA".into(),
                        analog_id: 151u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_L".into(),analog_id:39u8,input_type:Default::default()},
                    Key{input_id:"KEY_LEFT_ALT".into(),analog_id:60u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_LEFT_ARROW".into(),
                        analog_id: 79u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_LEFT_CTRL".into(),
                        analog_id: 58u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_LEFT_DPAD".into(),analog_id:0u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_LEFT_GUI".into(),
                        analog_id: 127u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_LEFT_SHIFT".into(),
                        analog_id: 44u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_M".into(),analog_id:52u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_MUHENKAN".into(),
                        analog_id: 150u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_N".into(),analog_id:51u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_NON_US_BACKSLASH".into(),
                        analog_id: 45u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_NON_US_POUND".into(),
                        analog_id: 42u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_NUMPAD_0".into(),analog_id:99u8,input_type:Default::default()},
                    Key{input_id:"KEY_NUMPAD_1".into(),analog_id:93u8,input_type:Default::default()},
                    Key{input_id:"KEY_NUMPAD_2".into(),analog_id:98u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_NUMPAD_3".into(),
                        analog_id: 103u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_NUMPAD_4".into(),analog_id:92u8,input_type:Default::default()},
                    Key{input_id:"KEY_NUMPAD_5".into(),analog_id:97u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_NUMPAD_6".into(),
                        analog_id: 102u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_NUMPAD_7".into(),analog_id:91u8,input_type:Default::default()},
                    Key{input_id:"KEY_NUMPAD_8".into(),analog_id:96u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_NUMPAD_9".into(),
                        analog_id: 101u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_ASTERISK".into(),
                        analog_id: 100u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_DASH".into(),
                        analog_id: 105u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_ENTER".into(),
                        analog_id: 108u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_NUM_LOCK".into(),
                        analog_id: 90u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_PERIOD".into(),
                        analog_id: 104u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_PLUS".into(),
                        analog_id: 106u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_SLASH".into(),
                        analog_id: 95u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_O".into(),analog_id:25u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_OPEN_SQUARE_BRACKET".into(),
                        analog_id: 27u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_P".into(),analog_id:26u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_PAGE_DOWN".into(),
                        analog_id: 86u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_PAGE_UP".into(),analog_id:85u8,input_type:Default::default()},
                    Key{input_id:"KEY_PAUSE".into(),analog_id:126u8,input_type:Default::default()},
                    Key{input_id:"KEY_PERIOD".into(),analog_id:54u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_PRINT_SCREEN".into(),
                        analog_id: 124u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_Q".into(),analog_id:17u8,input_type:Default::default()},
                    Key{input_id:"KEY_R".into(),analog_id:20u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_RIGHT_ALT".into(),
                        analog_id: 62u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_RIGHT_ARROW".into(),
                        analog_id: 89u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_RIGHT_CTRL".into(),
                        analog_id: 64u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_RIGHT_DPAD".into(),
                        analog_id: 0u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_RIGHT_SHIFT".into(),
                        analog_id: 57u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_RO".into(),analog_id:56u8,input_type:Default::default()},
                    Key{input_id:"KEY_S".into(),analog_id:32u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_SCROLL_LOCK".into(),
                        analog_id: 125u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_SEMICOLON".into(),
                        analog_id: 40u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_SLASH".into(),analog_id:55u8,input_type:Default::default()},
                    Key{input_id:"KEY_SPACEBAR".into(),analog_id:61u8,input_type:Default::default()},
                    Key{input_id:"KEY_T".into(),analog_id:21u8,input_type:Default::default()},
                    Key{input_id:"KEY_TAB".into(),analog_id:16u8,input_type:Default::default()},
                    Key{input_id:"KEY_TILDE".into(),analog_id:1u8,input_type:Default::default()},
                    Key{input_id:"KEY_U".into(),analog_id:23u8,input_type:Default::default()},
                    Key{input_id:"KEY_UP_ARROW".into(),analog_id:83u8,input_type:Default::default()},
                    Key{input_id:"KEY_UP_DPAD".into(),analog_id:0u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_UP_LEFT_ARROW".into(),
                        analog_id: 0u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_UP_RIGHT_ARROW".into(),
                        analog_id: 0u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_V".into(),analog_id:49u8,input_type:Default::default()},
                    Key{input_id:"KEY_W".into(),analog_id:18u8,input_type:Default::default()},
                    Key{input_id:"KEY_X".into(),analog_id:47u8,input_type:Default::default()},
                    Key{input_id:"KEY_Y".into(),analog_id:22u8,input_type:Default::default()},
                    Key{input_id:"KEY_YEN".into(),analog_id:14u8,input_type:Default::default()},
                    Key{input_id:"KEY_Z".into(),analog_id:46u8,input_type:Default::default()}
                ]
            },
            Product{
                product_id: 740u32,
                keys: vec![
                    Key{input_id:"KEY_0".into(),analog_id:11u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_1".into(),analog_id:2u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_2".into(),analog_id:3u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_3".into(),analog_id:4u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_4".into(),analog_id:5u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_5".into(),analog_id:6u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_6".into(),analog_id:7u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_7".into(),analog_id:8u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_8".into(),analog_id:9u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_9".into(),analog_id:10u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_A".into(),analog_id:31u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_APOSTROPHE".into(),analog_id:41u8,input_type:"Symbols".into()},
                    Key{
                        input_id: "KEY_APPLICATION".into(),
                        analog_id: 129u8,
                        input_type: "Navigation".into()
                    },
                    Key{input_id:"KEY_B".into(),analog_id:50u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_BACKSLASH".into(),analog_id:29u8,input_type:"Symbols".into()},
                    Key{
                        input_id: "KEY_BACKSPACE".into(),
                        analog_id: 15u8,
                        input_type: "Navigation".into()
                    },
                    Key{input_id:"KEY_C".into(),analog_id:48u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_CANCEL".into(),analog_id:125u8,input_type:"Navigation".into()},
                    Key{
                        input_id: "KEY_CAPS_LOCK".into(),
                        analog_id: 30u8,
                        input_type: "Modifiers".into()
                    },
                    Key{
                        input_id: "KEY_CLOSE_SQUARE_BRACKET".into(),
                        analog_id: 28u8,
                        input_type: "Symbols".into()
                    },
                    Key{input_id:"KEY_COMMA".into(),analog_id:53u8,input_type:"Symbols".into()},
                    Key{input_id:"KEY_D".into(),analog_id:33u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_DELETE".into(),analog_id:76u8,input_type:"Navigation".into()},
                    Key{
                        input_id: "KEY_DOWN_ARROW".into(),
                        analog_id: 84u8,
                        input_type: "Navigation".into()
                    },
                    Key{
                        input_id: "KEY_DOWN_DPAD".into(),
                        analog_id: 0u8,
                        input_type: "Navigation".into()
                    },
                    Key{
                        input_id: "KEY_DOWN_LEFT_ARROW".into(),
                        analog_id: 0u8,
                        input_type: "Navigation".into()
                    },
                    Key{
                        input_id: "KEY_DOWN_RIGHT_ARROW".into(),
                        analog_id: 0u8,
                        input_type: "Navigation".into()
                    },
                    Key{input_id:"KEY_E".into(),analog_id:19u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_END".into(),analog_id:81u8,input_type:"Navigation".into()},
                    Key{input_id:"KEY_ENTER".into(),analog_id:43u8,input_type:"Navigation".into()},
                    Key{input_id:"KEY_EQUAL".into(),analog_id:13u8,input_type:"Symbols".into()},
                    Key{input_id:"KEY_ESC".into(),analog_id:110u8,input_type:"Navigation".into()},
                    Key{input_id:"KEY_F".into(),analog_id:34u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_F1".into(),analog_id:112u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F10".into(),analog_id:121u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F11".into(),analog_id:122u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F12".into(),analog_id:123u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F13".into(),analog_id:0u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F14".into(),analog_id:0u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F15".into(),analog_id:0u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F16".into(),analog_id:0u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F17".into(),analog_id:0u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F18".into(),analog_id:0u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F19".into(),analog_id:0u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F2".into(),analog_id:113u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F20".into(),analog_id:0u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F21".into(),analog_id:0u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F22".into(),analog_id:0u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F23".into(),analog_id:0u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F24".into(),analog_id:0u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F3".into(),analog_id:114u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F4".into(),analog_id:115u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F5".into(),analog_id:116u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F6".into(),analog_id:117u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F7".into(),analog_id:118u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F8".into(),analog_id:119u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F9".into(),analog_id:120u8,input_type:"Function".into()},
                    Key{input_id:"KEY_G".into(),analog_id:35u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_H".into(),analog_id:36u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_HENKAN".into(),analog_id:154u8,input_type:"".into()},
                    Key{input_id:"KEY_HOME".into(),analog_id:80u8,input_type:"Navigation".into()},
                    Key{input_id:"KEY_HYPEN".into(),analog_id:12u8,input_type:"Symbols".into()},
                    Key{input_id:"KEY_I".into(),analog_id:24u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_INSERT".into(),analog_id:75u8,input_type:"Navigation".into()},
                    Key{input_id:"KEY_J".into(),analog_id:37u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_K".into(),analog_id:38u8,input_type:"Alphanumeric".into()},
                    Key{
                        input_id: "KEY_KATAKANA_HIRAGANA".into(),
                        analog_id: 151u8,
                        input_type: "".into()
                    },
                    Key{input_id:"KEY_L".into(),analog_id:39u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_LEFT_ALT".into(),analog_id:60u8,input_type:"Modifiers".into()},
                    Key{
                        input_id: "KEY_LEFT_ARROW".into(),
                        analog_id: 79u8,
                        input_type: "Navigation".into()
                    },
                    Key{
                        input_id: "KEY_LEFT_CTRL".into(),
                        analog_id: 58u8,
                        input_type: "Modifiers".into()
                    },
                    Key{
                        input_id: "KEY_LEFT_DPAD".into(),
                        analog_id: 0u8,
                        input_type: "Navigation".into()
                    },
                    Key{
                        input_id: "KEY_LEFT_GUI".into(),
                        analog_id: 127u8,
                        input_type: "Modifiers".into()
                    },
                    Key{
                        input_id: "KEY_LEFT_SHIFT".into(),
                        analog_id: 44u8,
                        input_type: "Modifiers".into()
                    },
                    Key{input_id:"KEY_M".into(),analog_id:52u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_MUHENKAN".into(),analog_id:150u8,input_type:"".into()},
                    Key{input_id:"KEY_N".into(),analog_id:51u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_NON_US_BACKSLASH".into(),analog_id:45u8,input_type:"".into()},
                    Key{
                        input_id: "KEY_NON_US_POUND".into(),
                        analog_id: 42u8,
                        input_type: "Symbols".into()
                    },
                    Key{input_id:"KEY_NUMPAD_0".into(),analog_id:99u8,input_type:"Numpad".into()},
                    Key{input_id:"KEY_NUMPAD_1".into(),analog_id:93u8,input_type:"Numpad".into()},
                    Key{input_id:"KEY_NUMPAD_2".into(),analog_id:98u8,input_type:"Numpad".into()},
                    Key{input_id:"KEY_NUMPAD_3".into(),analog_id:103u8,input_type:"Numpad".into()},
                    Key{input_id:"KEY_NUMPAD_4".into(),analog_id:92u8,input_type:"Numpad".into()},
                    Key{input_id:"KEY_NUMPAD_5".into(),analog_id:97u8,input_type:"Numpad".into()},
                    Key{input_id:"KEY_NUMPAD_6".into(),analog_id:102u8,input_type:"Numpad".into()},
                    Key{input_id:"KEY_NUMPAD_7".into(),analog_id:91u8,input_type:"Numpad".into()},
                    Key{input_id:"KEY_NUMPAD_8".into(),analog_id:96u8,input_type:"Numpad".into()},
                    Key{input_id:"KEY_NUMPAD_9".into(),analog_id:101u8,input_type:"Numpad".into()},
                    Key{
                        input_id: "KEY_NUMPAD_ASTERISK".into(),
                        analog_id: 100u8,
                        input_type: "Numpad".into()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_DASH".into(),
                        analog_id: 105u8,
                        input_type: "Numpad".into()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_ENTER".into(),
                        analog_id: 108u8,
                        input_type: "Numpad".into()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_NUM_LOCK".into(),
                        analog_id: 90u8,
                        input_type: "Numpad".into()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_PERIOD".into(),
                        analog_id: 104u8,
                        input_type: "Numpad".into()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_PLUS".into(),
                        analog_id: 106u8,
                        input_type: "Numpad".into()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_SLASH".into(),
                        analog_id: 95u8,
                        input_type: "Numpad".into()
                    },
                    Key{input_id:"KEY_O".into(),analog_id:25u8,input_type:"Alphanumeric".into()},
                    Key{
                        input_id: "KEY_OPEN_SQUARE_BRACKET".into(),
                        analog_id: 27u8,
                        input_type: "Symbols".into()
                    },
                    Key{input_id:"KEY_P".into(),analog_id:26u8,input_type:"Alphanumeric".into()},
                    Key{
                        input_id: "KEY_PAGE_DOWN".into(),
                        analog_id: 86u8,
                        input_type: "Navigation".into()
                    },
                    Key{input_id:"KEY_PAGE_UP".into(),analog_id:85u8,input_type:"Navigation".into()},
                    Key{input_id:"KEY_PAUSE".into(),analog_id:126u8,input_type:"Navigation".into()},
                    Key{input_id:"KEY_PERIOD".into(),analog_id:54u8,input_type:"Symbols".into()},
                    Key{
                        input_id: "KEY_PRINT_SCREEN".into(),
                        analog_id: 124u8,
                        input_type: "Navigation".into()
                    },
                    Key{input_id:"KEY_Q".into(),analog_id:17u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_R".into(),analog_id:20u8,input_type:"Alphanumeric".into()},
                    Key{
                        input_id: "KEY_RIGHT_ALT".into(),
                        analog_id: 62u8,
                        input_type: "Modifiers".into()
                    },
                    Key{
                        input_id: "KEY_RIGHT_ARROW".into(),
                        analog_id: 89u8,
                        input_type: "Navigation".into()
                    },
                    Key{
                        input_id: "KEY_RIGHT_CTRL".into(),
                        analog_id: 64u8,
                        input_type: "Modifiers".into()
                    },
                    Key{
                        input_id: "KEY_RIGHT_DPAD".into(),
                        analog_id: 0u8,
                        input_type: "Navigation".into()
                    },
                    Key{
                        input_id: "KEY_RIGHT_SHIFT".into(),
                        analog_id: 57u8,
                        input_type: "Modifiers".into()
                    },
                    Key{input_id:"KEY_RO".into(),analog_id:56u8,input_type:"".into()},
                    Key{input_id:"KEY_S".into(),analog_id:32u8,input_type:"Alphanumeric".into()},
                    Key{
                        input_id: "KEY_SCROLL_LOCK".into(),
                        analog_id: 125u8,
                        input_type: "Modifiers".into()
                    },
                    Key{input_id:"KEY_SEMICOLON".into(),analog_id:40u8,input_type:"Symbols".into()},
                    Key{input_id:"KEY_SLASH".into(),analog_id:55u8,input_type:"Symbols".into()},
                    Key{
                        input_id: "KEY_SPACEBAR".into(),
                        analog_id: 61u8,
                        input_type: "Navigation".into()
                    },
                    Key{input_id:"KEY_T".into(),analog_id:21u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_TAB".into(),analog_id:16u8,input_type:"Navigation".into()},
                    Key{input_id:"KEY_TILDE".into(),analog_id:1u8,input_type:"Symbols".into()},
                    Key{input_id:"KEY_U".into(),analog_id:23u8,input_type:"Alphanumeric".into()},
                    Key{
                        input_id: "KEY_UP_ARROW".into(),
                        analog_id: 83u8,
                        input_type: "Navigation".into()
                    },
                    Key{input_id:"KEY_UP_DPAD".into(),analog_id:0u8,input_type:"Navigation".into()},
                    Key{
                        input_id: "KEY_UP_LEFT_ARROW".into(),
                        analog_id: 0u8,
                        input_type: "Navigation".into()
                    },
                    Key{
                        input_id: "KEY_UP_RIGHT_ARROW".into(),
                        analog_id: 0u8,
                        input_type: "Navigation".into()
                    },
                    Key{input_id:"KEY_V".into(),analog_id:49u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_W".into(),analog_id:18u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_X".into(),analog_id:47u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_Y".into(),analog_id:22u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_YEN".into(),analog_id:14u8,input_type:"".into()},
                    Key{input_id:"KEY_Z".into(),analog_id:46u8,input_type:"Alphanumeric".into()}
                ]
            },
            Product{
                product_id: 742u32,
                keys: vec![
                    Key{input_id:"KEY_0".into(),analog_id:11u8,input_type:Default::default()},
                    Key{input_id:"KEY_1".into(),analog_id:2u8,input_type:Default::default()},
                    Key{input_id:"KEY_2".into(),analog_id:3u8,input_type:Default::default()},
                    Key{input_id:"KEY_3".into(),analog_id:4u8,input_type:Default::default()},
                    Key{input_id:"KEY_4".into(),analog_id:5u8,input_type:Default::default()},
                    Key{input_id:"KEY_5".into(),analog_id:6u8,input_type:Default::default()},
                    Key{input_id:"KEY_6".into(),analog_id:7u8,input_type:Default::default()},
                    Key{input_id:"KEY_7".into(),analog_id:8u8,input_type:Default::default()},
                    Key{input_id:"KEY_8".into(),analog_id:9u8,input_type:Default::default()},
                    Key{input_id:"KEY_9".into(),analog_id:10u8,input_type:Default::default()},
                    Key{input_id:"KEY_A".into(),analog_id:31u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_APOSTROPHE".into(),
                        analog_id: 41u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_APPLICATION".into(),
                        analog_id: 129u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_B".into(),analog_id:50u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_BACKSLASH".into(),
                        analog_id: 29u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_BACKSPACE".into(),
                        analog_id: 15u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_C".into(),analog_id:48u8,input_type:Default::default()},
                    Key{input_id:"KEY_CANCEL".into(),analog_id:125u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_CAPS_LOCK".into(),
                        analog_id: 30u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_CLOSE_SQUARE_BRACKET".into(),
                        analog_id: 28u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_COMMA".into(),analog_id:53u8,input_type:Default::default()},
                    Key{input_id:"KEY_D".into(),analog_id:33u8,input_type:Default::default()},
                    Key{input_id:"KEY_DELETE".into(),analog_id:76u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_DOWN_ARROW".into(),
                        analog_id: 84u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_DOWN_DPAD".into(),analog_id:0u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_DOWN_LEFT_ARROW".into(),
                        analog_id: 0u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_DOWN_RIGHT_ARROW".into(),
                        analog_id: 0u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_E".into(),analog_id:19u8,input_type:Default::default()},
                    Key{input_id:"KEY_END".into(),analog_id:81u8,input_type:Default::default()},
                    Key{input_id:"KEY_ENTER".into(),analog_id:43u8,input_type:Default::default()},
                    Key{input_id:"KEY_EQUAL".into(),analog_id:13u8,input_type:Default::default()},
                    Key{input_id:"KEY_ESC".into(),analog_id:110u8,input_type:Default::default()},
                    Key{input_id:"KEY_F".into(),analog_id:34u8,input_type:Default::default()},
                    Key{input_id:"KEY_F1".into(),analog_id:112u8,input_type:Default::default()},
                    Key{input_id:"KEY_F10".into(),analog_id:121u8,input_type:Default::default()},
                    Key{input_id:"KEY_F11".into(),analog_id:122u8,input_type:Default::default()},
                    Key{input_id:"KEY_F12".into(),analog_id:123u8,input_type:Default::default()},
                    Key{input_id:"KEY_F13".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F14".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F15".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F16".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F17".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F18".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F19".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F2".into(),analog_id:113u8,input_type:Default::default()},
                    Key{input_id:"KEY_F20".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F21".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F22".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F23".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F24".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F3".into(),analog_id:114u8,input_type:Default::default()},
                    Key{input_id:"KEY_F4".into(),analog_id:115u8,input_type:Default::default()},
                    Key{input_id:"KEY_F5".into(),analog_id:116u8,input_type:Default::default()},
                    Key{input_id:"KEY_F6".into(),analog_id:117u8,input_type:Default::default()},
                    Key{input_id:"KEY_F7".into(),analog_id:118u8,input_type:Default::default()},
                    Key{input_id:"KEY_F8".into(),analog_id:119u8,input_type:Default::default()},
                    Key{input_id:"KEY_F9".into(),analog_id:120u8,input_type:Default::default()},
                    Key{input_id:"KEY_G".into(),analog_id:35u8,input_type:Default::default()},
                    Key{input_id:"KEY_H".into(),analog_id:36u8,input_type:Default::default()},
                    Key{input_id:"KEY_HENKAN".into(),analog_id:154u8,input_type:Default::default()},
                    Key{input_id:"KEY_HOME".into(),analog_id:80u8,input_type:Default::default()},
                    Key{input_id:"KEY_HYPEN".into(),analog_id:12u8,input_type:Default::default()},
                    Key{input_id:"KEY_I".into(),analog_id:24u8,input_type:Default::default()},
                    Key{input_id:"KEY_INSERT".into(),analog_id:75u8,input_type:Default::default()},
                    Key{input_id:"KEY_J".into(),analog_id:37u8,input_type:Default::default()},
                    Key{input_id:"KEY_K".into(),analog_id:38u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_KATAKANA_HIRAGANA".into(),
                        analog_id: 151u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_L".into(),analog_id:39u8,input_type:Default::default()},
                    Key{input_id:"KEY_LEFT_ALT".into(),analog_id:60u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_LEFT_ARROW".into(),
                        analog_id: 79u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_LEFT_CTRL".into(),
                        analog_id: 58u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_LEFT_DPAD".into(),analog_id:0u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_LEFT_GUI".into(),
                        analog_id: 127u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_LEFT_SHIFT".into(),
                        analog_id: 44u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_M".into(),analog_id:52u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_MUHENKAN".into(),
                        analog_id: 150u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_N".into(),analog_id:51u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_NON_US_BACKSLASH".into(),
                        analog_id: 45u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_NON_US_POUND".into(),
                        analog_id: 42u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_NUMPAD_0".into(),analog_id:99u8,input_type:Default::default()},
                    Key{input_id:"KEY_NUMPAD_1".into(),analog_id:93u8,input_type:Default::default()},
                    Key{input_id:"KEY_NUMPAD_2".into(),analog_id:98u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_NUMPAD_3".into(),
                        analog_id: 103u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_NUMPAD_4".into(),analog_id:92u8,input_type:Default::default()},
                    Key{input_id:"KEY_NUMPAD_5".into(),analog_id:97u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_NUMPAD_6".into(),
                        analog_id: 102u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_NUMPAD_7".into(),analog_id:91u8,input_type:Default::default()},
                    Key{input_id:"KEY_NUMPAD_8".into(),analog_id:96u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_NUMPAD_9".into(),
                        analog_id: 101u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_ASTERISK".into(),
                        analog_id: 100u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_DASH".into(),
                        analog_id: 105u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_ENTER".into(),
                        analog_id: 108u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_NUM_LOCK".into(),
                        analog_id: 90u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_PERIOD".into(),
                        analog_id: 104u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_PLUS".into(),
                        analog_id: 106u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_SLASH".into(),
                        analog_id: 95u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_O".into(),analog_id:25u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_OPEN_SQUARE_BRACKET".into(),
                        analog_id: 27u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_P".into(),analog_id:26u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_PAGE_DOWN".into(),
                        analog_id: 86u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_PAGE_UP".into(),analog_id:85u8,input_type:Default::default()},
                    Key{input_id:"KEY_PAUSE".into(),analog_id:126u8,input_type:Default::default()},
                    Key{input_id:"KEY_PERIOD".into(),analog_id:54u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_PRINT_SCREEN".into(),
                        analog_id: 124u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_Q".into(),analog_id:17u8,input_type:Default::default()},
                    Key{input_id:"KEY_R".into(),analog_id:20u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_RIGHT_ALT".into(),
                        analog_id: 62u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_RIGHT_ARROW".into(),
                        analog_id: 89u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_RIGHT_CTRL".into(),
                        analog_id: 64u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_RIGHT_DPAD".into(),
                        analog_id: 0u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_RIGHT_SHIFT".into(),
                        analog_id: 57u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_RO".into(),analog_id:56u8,input_type:Default::default()},
                    Key{input_id:"KEY_S".into(),analog_id:32u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_SCROLL_LOCK".into(),
                        analog_id: 125u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_SEMICOLON".into(),
                        analog_id: 40u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_SLASH".into(),analog_id:55u8,input_type:Default::default()},
                    Key{input_id:"KEY_SPACEBAR".into(),analog_id:61u8,input_type:Default::default()},
                    Key{input_id:"KEY_T".into(),analog_id:21u8,input_type:Default::default()},
                    Key{input_id:"KEY_TAB".into(),analog_id:16u8,input_type:Default::default()},
                    Key{input_id:"KEY_TILDE".into(),analog_id:1u8,input_type:Default::default()},
                    Key{input_id:"KEY_U".into(),analog_id:23u8,input_type:Default::default()},
                    Key{input_id:"KEY_UP_ARROW".into(),analog_id:83u8,input_type:Default::default()},
                    Key{input_id:"KEY_UP_DPAD".into(),analog_id:0u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_UP_LEFT_ARROW".into(),
                        analog_id: 0u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_UP_RIGHT_ARROW".into(),
                        analog_id: 0u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_V".into(),analog_id:49u8,input_type:Default::default()},
                    Key{input_id:"KEY_W".into(),analog_id:18u8,input_type:Default::default()},
                    Key{input_id:"KEY_X".into(),analog_id:47u8,input_type:Default::default()},
                    Key{input_id:"KEY_Y".into(),analog_id:22u8,input_type:Default::default()},
                    Key{input_id:"KEY_YEN".into(),analog_id:14u8,input_type:Default::default()},
                    Key{input_id:"KEY_Z".into(),analog_id:46u8,input_type:Default::default()}
                ]
            },
            Product{
                product_id: 746u32,
                keys: vec![
                    Key{input_id:"KEY_0".into(),analog_id:11u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_1".into(),analog_id:2u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_2".into(),analog_id:3u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_3".into(),analog_id:4u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_4".into(),analog_id:5u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_5".into(),analog_id:6u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_6".into(),analog_id:7u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_7".into(),analog_id:8u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_8".into(),analog_id:9u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_9".into(),analog_id:10u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_A".into(),analog_id:31u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_APOSTROPHE".into(),analog_id:41u8,input_type:"Symbols".into()},
                    Key{
                        input_id: "KEY_APPLICATION".into(),
                        analog_id: 129u8,
                        input_type: "Navigation".into()
                    },
                    Key{input_id:"KEY_B".into(),analog_id:50u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_BACKSLASH".into(),analog_id:29u8,input_type:"Symbols".into()},
                    Key{
                        input_id: "KEY_BACKSPACE".into(),
                        analog_id: 15u8,
                        input_type: "Navigation".into()
                    },
                    Key{input_id:"KEY_C".into(),analog_id:48u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_CANCEL".into(),analog_id:125u8,input_type:"Navigation".into()},
                    Key{
                        input_id: "KEY_CAPS_LOCK".into(),
                        analog_id: 30u8,
                        input_type: "Modifiers".into()
                    },
                    Key{
                        input_id: "KEY_CLOSE_SQUARE_BRACKET".into(),
                        analog_id: 28u8,
                        input_type: "Symbols".into()
                    },
                    Key{input_id:"KEY_COMMA".into(),analog_id:53u8,input_type:"Symbols".into()},
                    Key{input_id:"KEY_D".into(),analog_id:33u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_DELETE".into(),analog_id:76u8,input_type:"Navigation".into()},
                    Key{
                        input_id: "KEY_DOWN_ARROW".into(),
                        analog_id: 84u8,
                        input_type: "Navigation".into()
                    },
                    Key{
                        input_id: "KEY_DOWN_DPAD".into(),
                        analog_id: 0u8,
                        input_type: "Navigation".into()
                    },
                    Key{
                        input_id: "KEY_DOWN_LEFT_ARROW".into(),
                        analog_id: 0u8,
                        input_type: "Navigation".into()
                    },
                    Key{
                        input_id: "KEY_DOWN_RIGHT_ARROW".into(),
                        analog_id: 0u8,
                        input_type: "Navigation".into()
                    },
                    Key{input_id:"KEY_E".into(),analog_id:19u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_END".into(),analog_id:81u8,input_type:"Navigation".into()},
                    Key{input_id:"KEY_ENTER".into(),analog_id:43u8,input_type:"Navigation".into()},
                    Key{input_id:"KEY_EQUAL".into(),analog_id:13u8,input_type:"Symbols".into()},
                    Key{input_id:"KEY_ESC".into(),analog_id:110u8,input_type:"Navigation".into()},
                    Key{input_id:"KEY_F".into(),analog_id:34u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_F1".into(),analog_id:112u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F10".into(),analog_id:121u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F11".into(),analog_id:122u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F12".into(),analog_id:123u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F13".into(),analog_id:0u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F14".into(),analog_id:0u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F15".into(),analog_id:0u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F16".into(),analog_id:0u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F17".into(),analog_id:0u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F18".into(),analog_id:0u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F19".into(),analog_id:0u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F2".into(),analog_id:113u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F20".into(),analog_id:0u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F21".into(),analog_id:0u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F22".into(),analog_id:0u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F23".into(),analog_id:0u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F24".into(),analog_id:0u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F3".into(),analog_id:114u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F4".into(),analog_id:115u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F5".into(),analog_id:116u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F6".into(),analog_id:117u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F7".into(),analog_id:118u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F8".into(),analog_id:119u8,input_type:"Function".into()},
                    Key{input_id:"KEY_F9".into(),analog_id:120u8,input_type:"Function".into()},
                    Key{input_id:"KEY_G".into(),analog_id:35u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_H".into(),analog_id:36u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_HENKAN".into(),analog_id:154u8,input_type:"".into()},
                    Key{input_id:"KEY_HOME".into(),analog_id:80u8,input_type:"Navigation".into()},
                    Key{input_id:"KEY_HYPEN".into(),analog_id:12u8,input_type:"Symbols".into()},
                    Key{input_id:"KEY_I".into(),analog_id:24u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_INSERT".into(),analog_id:75u8,input_type:"Navigation".into()},
                    Key{input_id:"KEY_J".into(),analog_id:37u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_K".into(),analog_id:38u8,input_type:"Alphanumeric".into()},
                    Key{
                        input_id: "KEY_KATAKANA_HIRAGANA".into(),
                        analog_id: 151u8,
                        input_type: "".into()
                    },
                    Key{input_id:"KEY_L".into(),analog_id:39u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_LEFT_ALT".into(),analog_id:60u8,input_type:"Modifiers".into()},
                    Key{
                        input_id: "KEY_LEFT_ARROW".into(),
                        analog_id: 79u8,
                        input_type: "Navigation".into()
                    },
                    Key{
                        input_id: "KEY_LEFT_CTRL".into(),
                        analog_id: 58u8,
                        input_type: "Modifiers".into()
                    },
                    Key{
                        input_id: "KEY_LEFT_DPAD".into(),
                        analog_id: 0u8,
                        input_type: "Navigation".into()
                    },
                    Key{
                        input_id: "KEY_LEFT_GUI".into(),
                        analog_id: 127u8,
                        input_type: "Modifiers".into()
                    },
                    Key{
                        input_id: "KEY_LEFT_SHIFT".into(),
                        analog_id: 44u8,
                        input_type: "Modifiers".into()
                    },
                    Key{input_id:"KEY_M".into(),analog_id:52u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_MUHENKAN".into(),analog_id:150u8,input_type:"".into()},
                    Key{input_id:"KEY_N".into(),analog_id:51u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_NON_US_BACKSLASH".into(),analog_id:45u8,input_type:"".into()},
                    Key{
                        input_id: "KEY_NON_US_POUND".into(),
                        analog_id: 42u8,
                        input_type: "Symbols".into()
                    },
                    Key{input_id:"KEY_NUMPAD_0".into(),analog_id:99u8,input_type:"Numpad".into()},
                    Key{input_id:"KEY_NUMPAD_1".into(),analog_id:93u8,input_type:"Numpad".into()},
                    Key{input_id:"KEY_NUMPAD_2".into(),analog_id:98u8,input_type:"Numpad".into()},
                    Key{input_id:"KEY_NUMPAD_3".into(),analog_id:103u8,input_type:"Numpad".into()},
                    Key{input_id:"KEY_NUMPAD_4".into(),analog_id:92u8,input_type:"Numpad".into()},
                    Key{input_id:"KEY_NUMPAD_5".into(),analog_id:97u8,input_type:"Numpad".into()},
                    Key{input_id:"KEY_NUMPAD_6".into(),analog_id:102u8,input_type:"Numpad".into()},
                    Key{input_id:"KEY_NUMPAD_7".into(),analog_id:91u8,input_type:"Numpad".into()},
                    Key{input_id:"KEY_NUMPAD_8".into(),analog_id:96u8,input_type:"Numpad".into()},
                    Key{input_id:"KEY_NUMPAD_9".into(),analog_id:101u8,input_type:"Numpad".into()},
                    Key{
                        input_id: "KEY_NUMPAD_ASTERISK".into(),
                        analog_id: 100u8,
                        input_type: "Numpad".into()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_DASH".into(),
                        analog_id: 105u8,
                        input_type: "Numpad".into()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_ENTER".into(),
                        analog_id: 108u8,
                        input_type: "Numpad".into()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_NUM_LOCK".into(),
                        analog_id: 90u8,
                        input_type: "Numpad".into()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_PERIOD".into(),
                        analog_id: 104u8,
                        input_type: "Numpad".into()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_PLUS".into(),
                        analog_id: 106u8,
                        input_type: "Numpad".into()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_SLASH".into(),
                        analog_id: 95u8,
                        input_type: "Numpad".into()
                    },
                    Key{input_id:"KEY_O".into(),analog_id:25u8,input_type:"Alphanumeric".into()},
                    Key{
                        input_id: "KEY_OPEN_SQUARE_BRACKET".into(),
                        analog_id: 27u8,
                        input_type: "Symbols".into()
                    },
                    Key{input_id:"KEY_P".into(),analog_id:26u8,input_type:"Alphanumeric".into()},
                    Key{
                        input_id: "KEY_PAGE_DOWN".into(),
                        analog_id: 86u8,
                        input_type: "Navigation".into()
                    },
                    Key{input_id:"KEY_PAGE_UP".into(),analog_id:85u8,input_type:"Navigation".into()},
                    Key{input_id:"KEY_PAUSE".into(),analog_id:126u8,input_type:"Navigation".into()},
                    Key{input_id:"KEY_PERIOD".into(),analog_id:54u8,input_type:"Symbols".into()},
                    Key{
                        input_id: "KEY_PRINT_SCREEN".into(),
                        analog_id: 124u8,
                        input_type: "Navigation".into()
                    },
                    Key{input_id:"KEY_Q".into(),analog_id:17u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_R".into(),analog_id:20u8,input_type:"Alphanumeric".into()},
                    Key{
                        input_id: "KEY_RIGHT_ALT".into(),
                        analog_id: 62u8,
                        input_type: "Modifiers".into()
                    },
                    Key{
                        input_id: "KEY_RIGHT_ARROW".into(),
                        analog_id: 89u8,
                        input_type: "Navigation".into()
                    },
                    Key{
                        input_id: "KEY_RIGHT_CTRL".into(),
                        analog_id: 64u8,
                        input_type: "Modifiers".into()
                    },
                    Key{
                        input_id: "KEY_RIGHT_DPAD".into(),
                        analog_id: 0u8,
                        input_type: "Navigation".into()
                    },
                    Key{
                        input_id: "KEY_RIGHT_SHIFT".into(),
                        analog_id: 57u8,
                        input_type: "Modifiers".into()
                    },
                    Key{input_id:"KEY_RO".into(),analog_id:56u8,input_type:"".into()},
                    Key{input_id:"KEY_S".into(),analog_id:32u8,input_type:"Alphanumeric".into()},
                    Key{
                        input_id: "KEY_SCROLL_LOCK".into(),
                        analog_id: 125u8,
                        input_type: "Modifiers".into()
                    },
                    Key{input_id:"KEY_SEMICOLON".into(),analog_id:40u8,input_type:"Symbols".into()},
                    Key{input_id:"KEY_SLASH".into(),analog_id:55u8,input_type:"Symbols".into()},
                    Key{
                        input_id: "KEY_SPACEBAR".into(),
                        analog_id: 61u8,
                        input_type: "Navigation".into()
                    },
                    Key{input_id:"KEY_T".into(),analog_id:21u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_TAB".into(),analog_id:16u8,input_type:"Navigation".into()},
                    Key{input_id:"KEY_TILDE".into(),analog_id:1u8,input_type:"Symbols".into()},
                    Key{input_id:"KEY_U".into(),analog_id:23u8,input_type:"Alphanumeric".into()},
                    Key{
                        input_id: "KEY_UP_ARROW".into(),
                        analog_id: 83u8,
                        input_type: "Navigation".into()
                    },
                    Key{input_id:"KEY_UP_DPAD".into(),analog_id:0u8,input_type:"Navigation".into()},
                    Key{
                        input_id: "KEY_UP_LEFT_ARROW".into(),
                        analog_id: 0u8,
                        input_type: "Navigation".into()
                    },
                    Key{
                        input_id: "KEY_UP_RIGHT_ARROW".into(),
                        analog_id: 0u8,
                        input_type: "Navigation".into()
                    },
                    Key{input_id:"KEY_V".into(),analog_id:49u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_W".into(),analog_id:18u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_X".into(),analog_id:47u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_Y".into(),analog_id:22u8,input_type:"Alphanumeric".into()},
                    Key{input_id:"KEY_YEN".into(),analog_id:14u8,input_type:"".into()},
                    Key{input_id:"KEY_Z".into(),analog_id:46u8,input_type:"Alphanumeric".into()}
                ]
            },
            Product{
                product_id: 756u32,
                keys: vec![
                    Key{input_id:"KEY_0".into(),analog_id:11u8,input_type:Default::default()},
                    Key{input_id:"KEY_1".into(),analog_id:2u8,input_type:Default::default()},
                    Key{input_id:"KEY_2".into(),analog_id:3u8,input_type:Default::default()},
                    Key{input_id:"KEY_3".into(),analog_id:4u8,input_type:Default::default()},
                    Key{input_id:"KEY_4".into(),analog_id:5u8,input_type:Default::default()},
                    Key{input_id:"KEY_5".into(),analog_id:6u8,input_type:Default::default()},
                    Key{input_id:"KEY_6".into(),analog_id:7u8,input_type:Default::default()},
                    Key{input_id:"KEY_7".into(),analog_id:8u8,input_type:Default::default()},
                    Key{input_id:"KEY_8".into(),analog_id:9u8,input_type:Default::default()},
                    Key{input_id:"KEY_9".into(),analog_id:10u8,input_type:Default::default()},
                    Key{input_id:"KEY_A".into(),analog_id:31u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_APOSTROPHE".into(),
                        analog_id: 41u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_APPLICATION".into(),
                        analog_id: 129u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_B".into(),analog_id:50u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_BACKSLASH".into(),
                        analog_id: 29u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_BACKSPACE".into(),
                        analog_id: 15u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_C".into(),analog_id:48u8,input_type:Default::default()},
                    Key{input_id:"KEY_CANCEL".into(),analog_id:125u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_CAPS_LOCK".into(),
                        analog_id: 30u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_CLOSE_SQUARE_BRACKET".into(),
                        analog_id: 28u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_COMMA".into(),analog_id:53u8,input_type:Default::default()},
                    Key{input_id:"KEY_D".into(),analog_id:33u8,input_type:Default::default()},
                    Key{input_id:"KEY_DELETE".into(),analog_id:76u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_DOWN_ARROW".into(),
                        analog_id: 84u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_DOWN_DPAD".into(),analog_id:0u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_DOWN_LEFT_ARROW".into(),
                        analog_id: 0u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_DOWN_RIGHT_ARROW".into(),
                        analog_id: 0u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_E".into(),analog_id:19u8,input_type:Default::default()},
                    Key{input_id:"KEY_END".into(),analog_id:81u8,input_type:Default::default()},
                    Key{input_id:"KEY_ENTER".into(),analog_id:43u8,input_type:Default::default()},
                    Key{input_id:"KEY_EQUAL".into(),analog_id:13u8,input_type:Default::default()},
                    Key{input_id:"KEY_ESC".into(),analog_id:110u8,input_type:Default::default()},
                    Key{input_id:"KEY_F".into(),analog_id:34u8,input_type:Default::default()},
                    Key{input_id:"KEY_F1".into(),analog_id:112u8,input_type:Default::default()},
                    Key{input_id:"KEY_F10".into(),analog_id:121u8,input_type:Default::default()},
                    Key{input_id:"KEY_F11".into(),analog_id:122u8,input_type:Default::default()},
                    Key{input_id:"KEY_F12".into(),analog_id:123u8,input_type:Default::default()},
                    Key{input_id:"KEY_F13".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F14".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F15".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F16".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F17".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F18".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F19".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F2".into(),analog_id:113u8,input_type:Default::default()},
                    Key{input_id:"KEY_F20".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F21".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F22".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F23".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F24".into(),analog_id:0u8,input_type:Default::default()},
                    Key{input_id:"KEY_F3".into(),analog_id:114u8,input_type:Default::default()},
                    Key{input_id:"KEY_F4".into(),analog_id:115u8,input_type:Default::default()},
                    Key{input_id:"KEY_F5".into(),analog_id:116u8,input_type:Default::default()},
                    Key{input_id:"KEY_F6".into(),analog_id:117u8,input_type:Default::default()},
                    Key{input_id:"KEY_F7".into(),analog_id:118u8,input_type:Default::default()},
                    Key{input_id:"KEY_F8".into(),analog_id:119u8,input_type:Default::default()},
                    Key{input_id:"KEY_F9".into(),analog_id:120u8,input_type:Default::default()},
                    Key{input_id:"KEY_G".into(),analog_id:35u8,input_type:Default::default()},
                    Key{input_id:"KEY_H".into(),analog_id:36u8,input_type:Default::default()},
                    Key{input_id:"KEY_HENKAN".into(),analog_id:154u8,input_type:Default::default()},
                    Key{input_id:"KEY_HOME".into(),analog_id:80u8,input_type:Default::default()},
                    Key{input_id:"KEY_HYPEN".into(),analog_id:12u8,input_type:Default::default()},
                    Key{input_id:"KEY_I".into(),analog_id:24u8,input_type:Default::default()},
                    Key{input_id:"KEY_INSERT".into(),analog_id:75u8,input_type:Default::default()},
                    Key{input_id:"KEY_J".into(),analog_id:37u8,input_type:Default::default()},
                    Key{input_id:"KEY_K".into(),analog_id:38u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_KATAKANA_HIRAGANA".into(),
                        analog_id: 151u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_L".into(),analog_id:39u8,input_type:Default::default()},
                    Key{input_id:"KEY_LEFT_ALT".into(),analog_id:60u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_LEFT_ARROW".into(),
                        analog_id: 79u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_LEFT_CTRL".into(),
                        analog_id: 58u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_LEFT_DPAD".into(),analog_id:0u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_LEFT_GUI".into(),
                        analog_id: 127u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_LEFT_SHIFT".into(),
                        analog_id: 44u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_M".into(),analog_id:52u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_MUHENKAN".into(),
                        analog_id: 150u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_N".into(),analog_id:51u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_NON_US_BACKSLASH".into(),
                        analog_id: 45u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_NON_US_POUND".into(),
                        analog_id: 42u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_NUMPAD_0".into(),analog_id:99u8,input_type:Default::default()},
                    Key{input_id:"KEY_NUMPAD_1".into(),analog_id:93u8,input_type:Default::default()},
                    Key{input_id:"KEY_NUMPAD_2".into(),analog_id:98u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_NUMPAD_3".into(),
                        analog_id: 103u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_NUMPAD_4".into(),analog_id:92u8,input_type:Default::default()},
                    Key{input_id:"KEY_NUMPAD_5".into(),analog_id:97u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_NUMPAD_6".into(),
                        analog_id: 102u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_NUMPAD_7".into(),analog_id:91u8,input_type:Default::default()},
                    Key{input_id:"KEY_NUMPAD_8".into(),analog_id:96u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_NUMPAD_9".into(),
                        analog_id: 101u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_ASTERISK".into(),
                        analog_id: 100u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_DASH".into(),
                        analog_id: 105u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_ENTER".into(),
                        analog_id: 108u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_NUM_LOCK".into(),
                        analog_id: 90u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_PERIOD".into(),
                        analog_id: 104u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_PLUS".into(),
                        analog_id: 106u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_NUMPAD_SLASH".into(),
                        analog_id: 95u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_O".into(),analog_id:25u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_OPEN_SQUARE_BRACKET".into(),
                        analog_id: 27u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_P".into(),analog_id:26u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_PAGE_DOWN".into(),
                        analog_id: 86u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_PAGE_UP".into(),analog_id:85u8,input_type:Default::default()},
                    Key{input_id:"KEY_PAUSE".into(),analog_id:126u8,input_type:Default::default()},
                    Key{input_id:"KEY_PERIOD".into(),analog_id:54u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_PRINT_SCREEN".into(),
                        analog_id: 124u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_Q".into(),analog_id:17u8,input_type:Default::default()},
                    Key{input_id:"KEY_R".into(),analog_id:20u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_RIGHT_ALT".into(),
                        analog_id: 62u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_RIGHT_ARROW".into(),
                        analog_id: 89u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_RIGHT_CTRL".into(),
                        analog_id: 64u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_RIGHT_DPAD".into(),
                        analog_id: 0u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_RIGHT_SHIFT".into(),
                        analog_id: 57u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_RO".into(),analog_id:56u8,input_type:Default::default()},
                    Key{input_id:"KEY_S".into(),analog_id:32u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_SCROLL_LOCK".into(),
                        analog_id: 125u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_SEMICOLON".into(),
                        analog_id: 40u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_SLASH".into(),analog_id:55u8,input_type:Default::default()},
                    Key{input_id:"KEY_SPACEBAR".into(),analog_id:61u8,input_type:Default::default()},
                    Key{input_id:"KEY_T".into(),analog_id:21u8,input_type:Default::default()},
                    Key{input_id:"KEY_TAB".into(),analog_id:16u8,input_type:Default::default()},
                    Key{input_id:"KEY_TILDE".into(),analog_id:1u8,input_type:Default::default()},
                    Key{input_id:"KEY_U".into(),analog_id:23u8,input_type:Default::default()},
                    Key{input_id:"KEY_UP_ARROW".into(),analog_id:83u8,input_type:Default::default()},
                    Key{input_id:"KEY_UP_DPAD".into(),analog_id:0u8,input_type:Default::default()},
                    Key{
                        input_id: "KEY_UP_LEFT_ARROW".into(),
                        analog_id: 0u8,
                        input_type: Default::default()
                    },
                    Key{
                        input_id: "KEY_UP_RIGHT_ARROW".into(),
                        analog_id: 0u8,
                        input_type: Default::default()
                    },
                    Key{input_id:"KEY_V".into(),analog_id:49u8,input_type:Default::default()},
                    Key{input_id:"KEY_W".into(),analog_id:18u8,input_type:Default::default()},
                    Key{input_id:"KEY_X".into(),analog_id:47u8,input_type:Default::default()},
                    Key{input_id:"KEY_Y".into(),analog_id:22u8,input_type:Default::default()},
                    Key{input_id:"KEY_YEN".into(),analog_id:14u8,input_type:Default::default()},
                    Key{input_id:"KEY_Z".into(),analog_id:46u8,input_type:Default::default()}
                ]
            }
        ]
    }
}
