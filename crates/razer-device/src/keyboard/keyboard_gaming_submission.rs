//! Custom_GameMode setCustomGameMode: three native slots, active read/merge,
//! then actual GUID-matched OBM slots using the active selection byte.
use super::{Receipt, Submission};
use crate::{
    backend::FeatureTransport, feature_exchange, keyboard_analog::KeyboardAnalogTransportCapability,
};
use anyhow::{Context as _, ensure};
use serde::Deserialize;
use serde_json::Value;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct Catalog {
    schema_version: u8,
    products: Vec<ProductKeys>,
}
#[derive(Deserialize)]
struct ProductKeys {
    product_id: u32,
    keys: Vec<Key>,
}
#[derive(Deserialize)]
struct Key {
    input_id: String,
    analog_id: u8,
}

fn number(value: Option<&Value>) -> anyhow::Result<f64> {
    let value = match value {
        None | Some(Value::Null) => 0.,
        Some(Value::Bool(value)) => {
            if *value {
                1.
            } else {
                0.
            }
        }
        Some(Value::Number(value)) => value.as_f64().context("Gaming key number is invalid")?,
        Some(Value::String(value)) if value.trim().is_empty() => 0.,
        Some(Value::String(value)) => value
            .trim()
            .parse()
            .context("Gaming key has no literal analog ID or numeric fallback")?,
        _ => anyhow::bail!("Gaming key must be a current input ID or numeric source value"),
    };
    ensure!(value.is_finite(), "Gaming key numeric value is not finite");
    Ok(value)
}
fn key_id(product: &ProductKeys, value: Option<&Value>) -> anyhow::Result<u8> {
    if let Some(key) = value
        .and_then(Value::as_str)
        .and_then(|id| product.keys.iter().find(|key| key.input_id == id))
    {
        return Ok(key.analog_id);
    }
    let value = number(value)?;
    ensure!(
        (0. ..=255.).contains(&value),
        "Gaming key ID exceeds the current byte range"
    );
    Ok(value as u8)
}
fn product_keys(product_id: u32) -> anyhow::Result<&'static ProductKeys> {
    static KEYS: OnceLock<Catalog> = OnceLock::new();
    let catalog = KEYS.get_or_init(|| crate::data::keyboard_analog_keys::keyboard_analog_keys!());
    ensure!(
        catalog.schema_version == 1,
        "Unknown keyboard key catalog schema"
    );
    catalog
        .products
        .iter()
        .find(|row| row.product_id == product_id)
        .context("Current product KEYS capture is absent")
}

/// getKeyByInputID resolves the complete source KEYS table. A missing input
/// must not become a guessed analog ID or a successful device write.
pub(super) fn literal_key_id(product_id: u32, input: &str) -> anyhow::Result<u8> {
    product_keys(product_id)?
        .keys
        .iter()
        .find(|key| key.input_id == input)
        .map(|key| key.analog_id)
        .context("Snap Tap input has no captured analog key ID")
}

pub(super) fn custom_keys(product_id: u32, settings: &Value) -> anyhow::Result<[u8; 9]> {
    let product = product_keys(product_id)?;
    let combos = settings["combos"]
        .as_array()
        .context("Custom gaming combinations are absent")?;
    ensure!(
        combos.len() <= 3,
        "Custom gaming combinations exceed the three native slots"
    );
    let mut keys = [0; 9];
    for (ix, combo) in combos.iter().enumerate() {
        if combo["enabled"] != true {
            continue;
        }
        let default_count = if combo["key2"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
        {
            2.
        } else {
            1.
        };
        let count = if combo.get("keyNum").is_some_and(|value| !value.is_null()) {
            number(combo.get("keyNum"))?.clamp(0., 2.)
        } else {
            default_count
        };
        keys[3 * ix] = count as u8;
        keys[3 * ix + 1] = key_id(product, combo.get("key1"))?;
        if count == 2. {
            keys[3 * ix + 2] = key_id(product, combo.get("key2"))?;
        }
    }
    Ok(keys)
}

fn exchange(
    device: &dyn FeatureTransport,
    cap: &KeyboardAnalogTransportCapability,
    receipt: &mut Receipt,
    command: [u8; 3],
    payload: Vec<u8>,
    next: &mut impl FnMut() -> anyhow::Result<u8>,
    guard: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Vec<u8>> {
    guard()?;
    let transaction = next()?;
    receipt.submissions.push(Submission {
        profile_id: payload[0],
        command,
        payload,
        attempted: false,
        acknowledgement: None,
    });
    let step = receipt.submissions.last_mut().unwrap();
    let data = feature_exchange::exchange_with_report_data_size_observing_send(
        device,
        &cap.device_read_capability(),
        command,
        &step.payload,
        transaction,
        command[0],
        guard,
        &mut || step.attempted = true,
    )?;
    step.acknowledgement = Some(data.clone());
    Ok(data)
}
pub(super) fn submit(
    device: &dyn FeatureTransport,
    product_id: u32,
    cap: &KeyboardAnalogTransportCapability,
    settings: &Value,
    receipt: &mut Receipt,
    next: &mut impl FnMut() -> anyhow::Result<u8>,
    guard: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let keys = custom_keys(product_id, settings)?;
    let custom = |slot| std::iter::once(slot).chain(keys).collect::<Vec<_>>();
    exchange(device, cap, receipt, [10, 0, 93], custom(1), next, guard)?;
    let active = exchange(device, cap, receipt, [3, 0, 218], vec![1], next, guard)?;
    ensure!(
        active.len() >= 2 && active[0] == 1,
        "Custom gaming selection response is truncated or belongs to another profile"
    );
    let mut extended = active.get(2).copied().unwrap_or(0);
    for (ix, combo) in settings["combos"].as_array().unwrap().iter().enumerate() {
        if combo["enabled"] == true {
            extended |= 1 << (ix + 1);
        } else {
            extended &= !(1 << (ix + 1));
        }
    }
    exchange(
        device,
        cap,
        receipt,
        [3, 0, 90],
        vec![1, active[1], extended],
        next,
        guard,
    )?;
    for slot in receipt.matched_slots.clone() {
        if slot != 1 {
            exchange(device, cap, receipt, [10, 0, 93], custom(slot), next, guard)?;
        }
        exchange(
            device,
            cap,
            receipt,
            [3, 0, 90],
            vec![slot, active[1], extended],
            next,
            guard,
        )?;
    }
    Ok(())
}
