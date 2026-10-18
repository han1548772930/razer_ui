//! Current magnetic-keyboard Qc/ft cleanup and oi/ri single-key Snap Tap tasks.
use super::{Receipt, Submission};
use crate::{
    backend::FeatureTransport, feature_exchange, keyboard_analog::KeyboardAnalogTransportCapability,
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Settings {
    pub enabled: bool,
    /// Source getKeyByInputID: make uses analogKeyID, break uses its HID usage.
    pub keys: Vec<[u8; 2]>,
}

pub(super) fn settings(product: u32, value: &Value) -> anyhow::Result<Settings> {
    ensure!(
        crate::keyboard_magnetic_mapping::supports(product),
        "Single-key Snap Tap product differs"
    );
    let enabled = value["isEnabled"]
        .as_bool()
        .context("Single-key Snap Tap enabled state is absent")?;
    let keys = match value.get("keyList") {
        None => Vec::new(),
        Some(Value::Array(keys)) => keys.clone(),
        Some(_) => anyhow::bail!("Single-key Snap Tap keys are not a list"),
    };
    let metadata =
        crate::mapping_engine::source_keys(product).context("Magnetic source keys are absent")?;
    let keys = keys
        .iter()
        .map(|key| {
            let make = key["keyMapMake"]
                .as_str()
                .context("Single-key Snap Tap make input is absent")?;
            let release = key["keyMapBreak"]
                .as_str()
                .context("Single-key Snap Tap break input is absent")?;
            let make = metadata
                .iter()
                .find(|key| key.input_id == Some(make))
                .map(|key| key.analog_key_id)
                .context("Single-key Snap Tap make key has no source analog identity")?;
            let release = metadata
                .iter()
                .find(|key| key.input_id == Some(release))
                .and_then(|key| key.hid)
                .context("Single-key Snap Tap break key has no source HID")?
                .parse::<u8>()?;
            Ok([u8::try_from(make)?, release])
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    ensure!(
        keys.len() <= 4,
        "Single-key Snap Tap exceeds four source entries"
    );
    Ok(Settings { enabled, keys })
}

/// Qc removes DKS conflicts from the persisted list, renumbers the retained
/// entries, and queues ft with the same enabled state before OBM mapping work.
pub(super) fn remove_dks_conflicts(
    product: u32,
    profile: &mut Value,
) -> anyhow::Result<Option<Settings>> {
    let Some(snap) = profile.get("singleKeySnapTap") else {
        return Ok(None);
    };
    let Some(keys) = snap.get("keyList") else {
        return Ok(None);
    };
    let keys = keys
        .as_array()
        .context("Single-key Snap Tap keys are not a list")?;
    if keys.is_empty() {
        return Ok(None);
    }
    let mappings = profile["mappings"]
        .as_array()
        .context("Magnetic mapping list is absent")?;
    let dynamic = mappings
        .iter()
        .filter(|row| {
            row["inputType"] == "AnalogInput"
                && row
                    .pointer("/mapping/0/outputType")
                    .is_some_and(|kind| kind == "dynamicKeyStrokeGroup")
        })
        .filter_map(|row| row["inputID"].as_str())
        .collect::<Vec<_>>();
    let mut remaining = keys
        .iter()
        .filter(|key| {
            !dynamic
                .iter()
                .any(|input| key["keyMapMake"] == *input || key["keyMapBreak"] == *input)
        })
        .cloned()
        .collect::<Vec<_>>();
    if remaining.len() == keys.len() {
        return Ok(None);
    }
    for (index, key) in remaining.iter_mut().enumerate() {
        ensure!(
            key.is_object(),
            "Single-key Snap Tap entry is not an object"
        );
        key["id"] = json!(index + 1);
    }
    profile["singleKeySnapTap"]["keyList"] = json!(remaining);
    Ok(Some(settings(product, &profile["singleKeySnapTap"])?))
}

pub(super) fn submit(
    device: &dyn FeatureTransport,
    cap: &KeyboardAnalogTransportCapability,
    settings: &Settings,
    global: bool,
    receipt: &mut Receipt,
    next: &mut impl FnMut() -> anyhow::Result<u8>,
    guard: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    ensure!(
        crate::keyboard_magnetic_mapping::supports(cap.product_id),
        "Single-key Snap Tap product differs"
    );
    ensure!(
        !global || !settings.enabled || !settings.keys.is_empty(),
        "Enabled single-key Snap Tap requires at least one key entry"
    );
    let first = if global { 0 } else { 1 };
    let slots = std::iter::once(first)
        .chain(
            receipt
                .matched_slots
                .iter()
                .copied()
                .filter(|slot| *slot != first),
        )
        .collect::<std::collections::BTreeSet<_>>();
    for slot in slots {
        let payload = crate::onboard_assignment::single_key_snap_tap_payload(
            slot,
            settings.enabled,
            &settings.keys,
        )?;
        receipt.submissions.push(Submission {
            profile_id: slot,
            command: crate::onboard_assignment::SET_SINGLE_KEY_SNAP_TAP,
            payload,
            attempted: false,
            acknowledgement: None,
        });
        let step = receipt.submissions.last_mut().unwrap();
        let data = feature_exchange::exchange_with_report_data_size_observing_send(
            device,
            &cap.device_read_capability(),
            step.command,
            &step.payload,
            next()?,
            11,
            guard,
            &mut || step.attempted = true,
        )?;
        step.acknowledgement = Some(data);
    }
    Ok(())
}
