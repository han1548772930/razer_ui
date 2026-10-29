//! Current226 MW92148.Cn external-file schema migrations. JSON stays at this
//! boundary; the caller immediately validates the result as its typed Document.
use anyhow::{Context as _, Result, bail, ensure};
use serde_json::{Map, Value};

use crate::host::js_truthy;

const CURRENT_SCHEMA: u64 = 13;
type Object = Map<String, Value>;

/// Consuming the parsed value keeps a failed, partially applied migration private.
/// The original Chromium bytes remain the caller's provenance and are never saved.
pub(super) fn migrate(value: Value) -> Result<Value> {
    let Value::Object(root) = value else {
        bail!("BLE226 local profile document is not an object");
    };
    let schema = match root.get("schemaVersion") {
        None | Some(Value::Null) => 0,
        Some(Value::Number(number)) => number
            .as_u64()
            .filter(|version| *version <= CURRENT_SCHEMA)
            .context("BLE226 schemaVersion must be an integer from 0 through 13")?,
        Some(_) => bail!("BLE226 schemaVersion must be an integer from 0 through 13"),
    };
    if schema == CURRENT_SCHEMA {
        return Ok(Value::Object(root));
    }
    let mut candidate = Candidate::parse(root)?;
    for step in Step::ALL {
        if step.version() > schema {
            step.apply(&mut candidate)
                .with_context(|| format!("BLE226 schema {} migration failed", step.version()))?;
            candidate
                .root
                .insert("schemaVersion".into(), step.version().into());
        }
    }
    Ok(candidate.into_value())
}

struct Candidate {
    root: Object,
    profiles: Vec<Object>,
}

impl Candidate {
    fn parse(mut root: Object) -> Result<Self> {
        let Value::Array(profiles) = root
            .shift_remove("profiles")
            .context("BLE226 local profile document is missing profiles")?
        else {
            bail!("BLE226 local profile profiles is not an array");
        };
        let profiles = profiles
            .into_iter()
            .enumerate()
            .map(|(index, value)| match value {
                Value::Object(profile) => Ok(profile),
                _ => bail!("BLE226 local profile profiles[{index}] is not an object"),
            })
            .collect::<Result<_>>()?;
        Ok(Self { root, profiles })
    }

    fn into_value(mut self) -> Value {
        self.root.insert(
            "profiles".into(),
            Value::Array(self.profiles.into_iter().map(Value::Object).collect()),
        );
        Value::Object(self.root)
    }
}

#[derive(Clone, Copy)]
#[repr(u8)]
enum Step {
    StreamMixer = 1,
    RemoveVolume = 2,
    MoveBossKey = 3,
    RemoveIndicatorLed = 4,
    MoveMicrophoneSettings = 5,
    RemoveDpiMatcherNoneProfile = 6,
    RemoveCameraPreview = 7,
    RemoveContinuousRapidTriggerPerKey = 8,
    SnapTapKeyListArray = 9,
    AddSnapTapKeyListId = 10,
    RemoveStreamMixerSettings = 11,
    GamingModeFnSleep = 12,
    CameraAntiFlicker = 13,
}

impl Step {
    const ALL: [Self; 13] = [
        Self::StreamMixer,
        Self::RemoveVolume,
        Self::MoveBossKey,
        Self::RemoveIndicatorLed,
        Self::MoveMicrophoneSettings,
        Self::RemoveDpiMatcherNoneProfile,
        Self::RemoveCameraPreview,
        Self::RemoveContinuousRapidTriggerPerKey,
        Self::SnapTapKeyListArray,
        Self::AddSnapTapKeyListId,
        Self::RemoveStreamMixerSettings,
        Self::GamingModeFnSleep,
        Self::CameraAntiFlicker,
    ];

    fn version(self) -> u64 {
        self as u64
    }

    fn apply(self, candidate: &mut Candidate) -> Result<()> {
        match self {
            Self::StreamMixer => move_stream_mixer(candidate),
            Self::RemoveVolume => {
                for profile in &mut candidate.profiles {
                    profile.shift_remove("volume");
                    profile.shift_remove("micVolume");
                }
            }
            Self::MoveBossKey => {
                if let Some(value) = candidate
                    .root
                    .get("bossKey")
                    .filter(|value| value.is_number() && js_truthy(value))
                    .cloned()
                {
                    candidate.root.shift_remove("bossKey");
                    for profile in &mut candidate.profiles {
                        profile.insert("bossKey".into(), value.clone());
                    }
                }
            }
            Self::RemoveIndicatorLed => {
                if candidate
                    .root
                    .get("indicatorLedStatus")
                    .is_some_and(|value| value.is_number() && js_truthy(value))
                {
                    candidate.root.shift_remove("indicatorLedStatus");
                }
            }
            Self::MoveMicrophoneSettings => move_microphone_settings(candidate),
            Self::RemoveDpiMatcherNoneProfile => remove_matcher_none(candidate)?,
            Self::RemoveCameraPreview => {
                candidate.root.shift_remove("isPreview");
            }
            Self::RemoveContinuousRapidTriggerPerKey => move_continuous_rapid_trigger(candidate)?,
            Self::SnapTapKeyListArray => snap_tap_key_list_array(candidate)?,
            Self::AddSnapTapKeyListId => add_snap_tap_key_list_id(candidate)?,
            Self::RemoveStreamMixerSettings => {
                for profile in &mut candidate.profiles {
                    profile.shift_remove("streamMixerSettings");
                }
                // MW92148.Cn case 11 has no break and runs case 10 as well.
                add_snap_tap_key_list_id(candidate)?;
            }
            Self::GamingModeFnSleep => gaming_mode_fn_sleep(candidate),
            Self::CameraAntiFlicker => camera_anti_flicker(candidate)?,
        }
        Ok(())
    }
}

fn move_stream_mixer(candidate: &mut Candidate) {
    const FIELDS: [&str; 6] = [
        "isStreamMixerEnabled",
        "streamMixVolume",
        "playbackMixVolume",
        "isStreamMixMonitor",
        "playbackMixDevice",
        "otherCategories",
    ];
    for profile in &mut candidate.profiles {
        if profile.contains_key("streamMixerSettings") {
            continue;
        }
        let mut settings = Object::new();
        for field in FIELDS {
            if let Some(value) = profile.shift_remove(field) {
                settings.insert(field.into(), value);
            }
        }
        if !settings.is_empty() {
            profile.insert("streamMixerSettings".into(), Value::Object(settings));
        }
    }
}

fn move_microphone_settings(candidate: &mut Candidate) {
    const FIELDS: [&str; 3] = [
        "samplingRate",
        "isHighPassFilterEnabled",
        "isAnalogGainLimiterEnabled",
    ];
    for profile in &mut candidate.profiles {
        for field in FIELDS {
            if let Some(value) = profile.shift_remove(field) {
                // An existing null is defined in the source and blocks adoption.
                candidate.root.entry(field).or_insert(value);
            }
        }
    }
}

fn remove_matcher_none(candidate: &mut Candidate) -> Result<()> {
    let Some(matcher) = candidate.root.get_mut("sensitivityMatcher") else {
        return Ok(());
    };
    let profiles = matcher
        .as_object_mut()
        .context("BLE226 sensitivityMatcher is not an object")?
        .get_mut("profiles")
        .and_then(Value::as_array_mut)
        .context("BLE226 sensitivityMatcher.profiles is not an array")?;
    ensure!(
        profiles.iter().all(Value::is_object),
        "BLE226 sensitivityMatcher.profiles contains a non-object"
    );
    profiles.retain(|profile| profile.get("name").and_then(Value::as_str) != Some("None"));
    Ok(())
}

fn move_continuous_rapid_trigger(candidate: &mut Candidate) -> Result<()> {
    let active_profile = candidate.root.get("activeProfile").cloned();
    let Some(analog_data) = candidate
        .root
        .get_mut("analogData")
        .filter(|value| js_truthy(value))
    else {
        return Ok(());
    };
    let analog_data = analog_data
        .as_object_mut()
        .context("BLE226 analogData is not an object")?;
    let mut active_continuous = false;
    for profile in &mut candidate.profiles {
        // Optional chaining skips null/missing appEngine; other JSON primitives
        // have no rapidTriggers property either.
        let Some(triggers) = profile
            .get_mut("appEngine")
            .and_then(Value::as_object_mut)
            .and_then(|engine| engine.get_mut("rapidTriggers"))
        else {
            continue;
        };
        let continuous = match triggers {
            Value::Object(entries) => take_continuous(entries.values_mut())?,
            Value::Array(entries) => take_continuous(entries.iter_mut())?,
            _ => bail!("BLE226 appEngine.rapidTriggers is not an object or array"),
        };
        if active_profile.as_ref() == profile.get("guid") {
            active_continuous = continuous;
        }
        profile.insert("continuousRapidTrigger".into(), continuous.into());
    }
    // The source replaces rapidTrigger entirely, preserving only its parent fields.
    analog_data.insert(
        "rapidTrigger".into(),
        Value::Object(Object::from_iter([(
            "continuous".into(),
            active_continuous.into(),
        )])),
    );
    Ok(())
}

fn take_continuous<'a>(entries: impl Iterator<Item = &'a mut Value>) -> Result<bool> {
    let mut continuous = false;
    for entry in entries {
        let entry = entry
            .as_object_mut()
            .context("BLE226 rapidTriggers entry is not an object")?;
        if let Some(value) = entry.shift_remove("continuous") {
            continuous |= js_truthy(&value);
        }
    }
    Ok(continuous)
}

fn snap_tap_key_list_array(candidate: &mut Candidate) -> Result<()> {
    for profile in &mut candidate.profiles {
        let Some(snap_tap) = profile.get_mut("snapTap").filter(|value| js_truthy(value)) else {
            continue;
        };
        let snap_tap = snap_tap
            .as_object_mut()
            .context("BLE226 snapTap is not an object")?;
        if let Some(key_list) = snap_tap.get_mut("keyList") {
            // typeof null is "object" in case 9; the next step rejects [null].
            if matches!(key_list, Value::Null | Value::Object(_)) {
                *key_list = Value::Array(vec![std::mem::take(key_list)]);
            }
        }
    }
    Ok(())
}

fn add_snap_tap_key_list_id(candidate: &mut Candidate) -> Result<()> {
    for profile in &mut candidate.profiles {
        let Some(snap_tap) = profile.get_mut("snapTap").filter(|value| js_truthy(value)) else {
            continue;
        };
        let key_list = snap_tap
            .as_object_mut()
            .context("BLE226 snapTap is not an object")?
            .get_mut("keyList")
            .and_then(Value::as_array_mut)
            .context("BLE226 snapTap.keyList is not an array")?;
        let mut missing_id = false;
        for entry in key_list.iter() {
            let entry = entry
                .as_object()
                .context("BLE226 snapTap.keyList entry is not an object")?;
            missing_id |= !entry.contains_key("id");
        }
        if missing_id {
            for (index, entry) in key_list.iter_mut().enumerate() {
                entry
                    .as_object_mut()
                    .context("BLE226 snapTap.keyList entry is not an object")?
                    .insert("id".into(), (index + 1).into());
            }
        }
    }
    Ok(())
}

fn gaming_mode_fn_sleep(candidate: &mut Candidate) {
    for profile in &mut candidate.profiles {
        // Only objects can define this named property in an external JSON value.
        // Falsy values and truthy primitives without it remain untouched.
        if let Some(gaming_mode) = profile.get_mut("gamingMode").and_then(Value::as_object_mut) {
            if let Some(value) = gaming_mode.shift_remove("isFnZDisabled") {
                gaming_mode.insert("isFnSleepEnabled".into(), value);
            }
        }
    }
}

fn camera_anti_flicker(candidate: &mut Candidate) -> Result<()> {
    for profile in &mut candidate.profiles {
        let Some(camera) = profile.get_mut("camera").filter(|value| js_truthy(value)) else {
            continue;
        };
        let camera = camera
            .as_object_mut()
            .context("BLE226 camera is not an object")?;
        if !camera
            .get("antiFlicker")
            .and_then(Value::as_f64)
            .is_some_and(|value| [0.0, 1.0, 2.0, 3.0].contains(&value))
        {
            camera.insert("antiFlicker".into(), 0.into());
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "ble226_migration_tests.rs"]
mod tests;
