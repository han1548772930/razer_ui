//! Current740/746 generic feature reducers and their engine preparation mode.
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub(super) const ENGINE_PREPARATION: &str = "_magneticEnginePreparation";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EnginePreparation {
    Firmware,
    Gamepad,
}

pub(crate) fn preparation(profile: &Value) -> anyhow::Result<EnginePreparation> {
    match profile.get(ENGINE_PREPARATION) {
        Some(value) => {
            serde_json::from_value(value.clone()).context("Magnetic engine preparation is invalid")
        }
        // The source's restoreDeviceData invokes the enable/override generic
        // feature handlers. Older local profiles have that preparation mode.
        None => Ok(EnginePreparation::Gamepad),
    }
}

pub(crate) fn set_preparation(profile: &mut Value, mode: EnginePreparation) {
    profile[ENGINE_PREPARATION] = serde_json::to_value(mode).unwrap();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InterruptionKey {
    Any,
    Dedicated,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct InterruptSettings {
    pub is_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interruption_key: Option<InterruptionKey>,
    pub interrupts: Vec<String>,
}

impl InterruptSettings {
    pub(crate) fn parse(value: &Value) -> anyhow::Result<Self> {
        let mut settings: Self = serde_json::from_value(value.clone())
            .context("Magnetic interruption settings are invalid")?;
        let mut unique = Vec::new();
        for key in settings.interrupts {
            ensure!(!key.is_empty(), "Interruption key identity is empty");
            if !unique.contains(&key) {
                unique.push(key);
            }
        }
        settings.interrupts = unique;
        Ok(settings)
    }

    pub(crate) fn firmware_keys(&self, product: u32) -> anyhow::Result<(u8, Vec<u8>)> {
        if !self.is_enabled
            || self.interruption_key == Some(InterruptionKey::Any)
            || self.interrupts.iter().any(|key| key == "KEY_ANY")
        {
            return Ok((0, Vec::new()));
        }
        ensure!(
            self.interrupts.len() <= 4,
            "Interruption keys exceed four source entries"
        );
        let metadata = crate::mapping_engine::source_keys(product)
            .context("Magnetic interruption key metadata is absent")?;
        let keys = self
            .interrupts
            .iter()
            .map(|input| {
                let key = metadata
                    .iter()
                    .find(|key| key.input_id == Some(input.as_str()))
                    .context("Interruption key has no source identity")?;
                Ok(u8::try_from(key.analog_key_id)?)
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok((1, keys))
    }
}

pub(crate) fn persist_interrupt(profile: &mut Value, value: &Value) -> anyhow::Result<()> {
    let settings = InterruptSettings::parse(value)?;
    ensure!(
        settings.interruption_key.is_some(),
        "Requested interruption mode is absent"
    );
    ensure!(
        profile["interruptKey"].is_object(),
        "Magnetic interruption profile state is absent"
    );
    profile["interruptKey"]["isEnabled"] = json!(settings.is_enabled);
    profile["interruptKey"]["interruptionKey"] = json!(settings.interruption_key);
    if !settings.interrupts.iter().any(|key| key == "KEY_ANY") {
        profile["interruptKey"]["interrupts"] = json!(settings.interrupts);
    }
    Ok(())
}

pub(super) fn is_interruptible(row: &Value) -> bool {
    matches!(
        row["outputType"].as_str(),
        Some("macroGroup" | "dynamicKeyStrokeGroup")
    ) || row["inputType"] == "AnalogInput"
        && matches!(
            row.pointer("/mapping/0/outputType").and_then(Value::as_str),
            Some("macroGroup" | "dynamicKeyStrokeGroup")
        )
}

pub(crate) fn has_interruptible(profile: &Value) -> anyhow::Result<bool> {
    Ok(profile["mappings"]
        .as_array()
        .context("Magnetic mappings are absent")?
        .iter()
        .any(is_interruptible))
}

pub(super) fn engine_rows(
    profile: &Value,
    rows: &[Value],
    mode: EnginePreparation,
) -> anyhow::Result<Vec<Value>> {
    let mut rows = rows.to_vec();
    let settings = match profile.get("interruptKey") {
        Some(value) => Some(InterruptSettings::parse(value)?),
        None => None,
    };
    for row in rows.iter_mut().filter(|row| {
        is_interruptible(row)
            && (mode == EnginePreparation::Gamepad || row["inputType"] == "AnalogInput")
    }) {
        if let Some(settings) = &settings {
            if settings.is_enabled {
                let keys = if settings.interruption_key == Some(InterruptionKey::Any)
                    || mode == EnginePreparation::Gamepad
                        && settings.interrupts.iter().any(|key| key == "KEY_ANY")
                {
                    if mode == EnginePreparation::Firmware {
                        vec!["KEY_ANY".to_owned()]
                    } else {
                        Vec::new()
                    }
                } else {
                    settings.interrupts.clone()
                };
                row["interrupts"] = json!(keys);
                continue;
            }
        }
        row.as_object_mut()
            .context("Interruptible mapping is not an object")?
            .remove("interrupts");
    }
    Ok(rows)
}
