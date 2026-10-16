//! PID679 current Mt -> ni KillSwitch task. The selected slot is the first
//! actual OBM GUID match, with source active-slot fallback 1. A matching GET
//! skips SET; setter ACKs remain independent from any readback observation.
use super::{Receipt, Submission, gaming};
use crate::{
    backend::FeatureTransport, feature_exchange, keyboard_analog::KeyboardAnalogTransportCapability,
};
use anyhow::{Context as _, ensure};
use serde_json::{Value, json};

const GET_KILL_SWITCH: [u8; 3] = [4, 2, 161];
const SET_KILL_SWITCH: [u8; 3] = [4, 2, 33];

pub(super) fn local_settings(payload: &Value) -> anyhow::Result<Value> {
    let enabled = payload["isEnabled"]
        .as_bool()
        .context("Snap Tap enabled state is missing")?;
    let pairs = payload["keyList"]
        .as_array()
        .context("Snap Tap key list is missing")?;
    // MW dispatcher destructures only these two fields. pressedKeys belongs
    // to the independent UI observation and is not persisted by Mt.
    Ok(json!({"isEnabled": enabled, "keyList": pairs}))
}

fn pairs(product_id: u32, settings: &Value) -> anyhow::Result<Vec<[u8; 2]>> {
    settings["keyList"]
        .as_array()
        .context("Snap Tap key list is missing")?
        .iter()
        .map(|pair| {
            let first = pair["key1"]
                .as_str()
                .context("First Snap Tap key is missing")?;
            let second = pair["key2"]
                .as_str()
                .context("Second Snap Tap key is missing")?;
            Ok([
                gaming::literal_key_id(product_id, first)?,
                gaming::literal_key_id(product_id, second)?,
            ])
        })
        .collect()
}

pub(super) fn validate(
    product_id: u32,
    cap: &KeyboardAnalogTransportCapability,
    settings: &Value,
) -> anyhow::Result<()> {
    ensure!(
        product_id == 679 && cap.kill_switch && !cap.snap_tap,
        "Product does not have the captured PID679 Snap Tap slot policy"
    );
    local_settings(settings)?;
    pairs(product_id, settings)?;
    Ok(())
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
    let sequence = next()?;
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
        sequence,
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
    validate(product_id, cap, settings)?;
    let enabled = u8::from(settings["isEnabled"] == true);
    let pairs = pairs(product_id, settings)?;
    let slot = receipt.matched_slots.first().copied().unwrap_or(1);
    let observed = exchange(
        device,
        cap,
        receipt,
        GET_KILL_SWITCH,
        vec![slot],
        next,
        guard,
    )?;
    ensure!(
        observed.len() >= 4 && observed[0] == slot,
        "Snap Tap getter is truncated or belongs to another profile"
    );
    // ni compares each pair against the same initial getter. Feature report
    // exchanges remain serialized by their owning service connection.
    for [first, second] in pairs {
        if observed[1..4] == [enabled, first, second] {
            continue;
        }
        exchange(
            device,
            cap,
            receipt,
            SET_KILL_SWITCH,
            vec![slot, enabled, first, second],
            next,
            guard,
        )?;
    }
    Ok(())
}
