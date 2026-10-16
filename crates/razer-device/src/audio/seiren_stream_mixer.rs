//! Current 1422/1446 MW80168 Audio_StreamMixer cache and native call ordering.
//! The cache is local data, never a receipt for native or OS audio writes.
use crate::seiren_eq::NativeOperation;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::OnceLock;

pub const STORAGE_KEY: &str = "stream-mixer";
pub const ACTIVE_KEY: &str = "activeStreamMixer";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Action {
    event: String,
    payload: Value,
}
impl Action {
    pub fn new(event: impl Into<String>, mut payload: Value) -> Result<Self> {
        // Generic33422 removes reducer bookkeeping before extracting the
        // descriptor paramKey; absent/null parameters fall back to the object.
        if let Some(fields) = payload.as_object_mut() {
            fields.remove("version");
            fields.remove("timertick");
        }
        let action = Self {
            event: event.into(),
            payload,
        };
        action.argument()?;
        Ok(action)
    }
    pub fn event(&self) -> &str {
        &self.event
    }
    pub fn payload(&self) -> &Value {
        &self.payload
    }
    pub fn argument(&self) -> Result<&Value> {
        let key = match self.event.as_str() {
            "ON_SET_STREAM_MIXER_ENABLE" => "isStreamMixerEnabled",
            "ON_SET_STREAM_MIX_VOLUME" => "streamMixVolume",
            "ON_SET_PLAYBACK_MIX_VOLUME" => "playbackMixVolume",
            "ON_SET_STREAM_MIX_MONITOR" => "isStreamMixMonitor",
            "ON_CHANGE_PLAY_BACK_MIX_DEVICE" => "changePlayBackMixDevice",
            "ON_MODIFY_INPUT_CHANNEL" => "inputChannel",
            "ON_ADD_STREAM_MIXER_PRESET" => "addSteamMixerPreset",
            "ON_DELETE_STREAM_MIXER_PRESET" => "deleteSteamMixerPreset",
            "ON_RESET_STREAM_MIXER_PRESET" => "resetSteamMixerPreset",
            "ON_RENAME_STREAM_MIXER_PRESET" => "renameSteamMixerPreset",
            "ON_DUPLICATE_STREAM_MIXER_PRESET" => "duplicateSteamMixerPreset",
            "ON_SELECTED_STREAM_MIXER_PRESET" => "selectedSteamMixerPreset",
            "ON_CHANGE_STREAM_MIXER_PRESET_MAPPING" => "changeStreamMixerPresetMapping",
            _ => anyhow::bail!("Unknown current Seiren mixer event: {}", self.event),
        };
        Ok(self
            .payload
            .get(key)
            .filter(|value| !value.is_null())
            .unwrap_or(&self.payload))
    }
    pub fn enabled(&self) -> Result<Option<bool>> {
        if self.event == "ON_SET_STREAM_MIXER_ENABLE" {
            Ok(Some(
                self.argument()?
                    .as_bool()
                    .context("Mixer enable is not a boolean")?,
            ))
        } else {
            Ok(None)
        }
    }
    pub fn applies_active_preset(&self) -> bool {
        matches!(
            self.event.as_str(),
            "ON_ADD_STREAM_MIXER_PRESET"
                | "ON_DELETE_STREAM_MIXER_PRESET"
                | "ON_RESET_STREAM_MIXER_PRESET"
                | "ON_DUPLICATE_STREAM_MIXER_PRESET"
                | "ON_SELECTED_STREAM_MIXER_PRESET"
        )
    }
    pub fn updates_device_document(&self) -> bool {
        matches!(
            self.event.as_str(),
            "ON_SET_STREAM_MIXER_ENABLE"
                | "ON_SET_STREAM_MIX_VOLUME"
                | "ON_SET_PLAYBACK_MIX_VOLUME"
                | "ON_SET_STREAM_MIX_MONITOR"
                | "ON_CHANGE_PLAY_BACK_MIX_DEVICE"
                | "ON_MODIFY_INPUT_CHANNEL"
        )
    }
}
/// Both verified startup defaults initialize the separate device document at
/// version 1. Preset operations have no Generic Storage_Mask and skip this.
pub fn next_device_version(previous: Option<&Value>) -> Result<u64> {
    let mut document = json!({"version":previous.cloned().unwrap_or(json!(1))});
    next_version(&mut document)?;
    document["version"]
        .as_u64()
        .context("Invalid next mixer device version")
}
fn defaults(product_id: u32) -> Result<&'static Value> {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../assets/data/seiren-stream-mixer.json"
        ))
        .expect("prepared current Seiren literals")
    })
    .get(product_id.to_string())
    .context("Current Seiren stream-mixer product is absent")
}
pub fn device_name(product_id: u32) -> Result<&'static str> {
    defaults(product_id)?["deviceName"]
        .as_str()
        .context("Source Seiren device name is absent")
}
/// MW44857 L lowercases its dictionary keys; the notification writer indexes
/// that map with the exact localStorage `currentLanguage` value. Neither the
/// source edition context nor this lookup supplies an English/name fallback.
pub fn product_name(product_id: u32, edition: u32, locale: &str) -> Result<&'static str> {
    defaults(product_id)?["productNames"]
        .get(edition.to_string())
        .with_context(|| {
            format!("Current Seiren PRODUCT edition {product_id}_{edition} is absent")
        })?
        .get(locale)
        .and_then(Value::as_str)
        .with_context(|| {
            format!("Current Seiren PRODUCT locale {locale:?} for {product_id}_{edition} is absent")
        })
}
pub fn shortcut_binding(product_id: u32, mapping: &Value) -> Result<Option<(u32, u32)>> {
    if mapping.as_object().is_some_and(serde_json::Map::is_empty) {
        return Ok(None);
    }
    let source = defaults(product_id)?;
    let input = mapping["inputID"]
        .as_str()
        .context("Mixer shortcut input ID is absent")?;
    let key = source["shortcutKeys"]
        .as_array()
        .context("Mixer shortcut source keys are absent")?
        .iter()
        .find(|key| key["inputID"] == input)
        .and_then(|key| key["keyCode"].as_str())
        .context("Mixer shortcut input has no source keyCode")?
        .parse::<u32>()?;
    let mut modifiers = 0;
    for modifier in mapping["inputModifiers"]
        .as_array()
        .context("Mixer shortcut inputModifiers is not an array")?
    {
        modifiers |= source["shortcutModifierBits"][modifier
            .as_str()
            .context("Mixer shortcut modifier is not a string")?]
        .as_u64()
        .unwrap_or(0) as u32;
    }
    Ok(Some((key, modifiers)))
}
pub fn default_preset(product_id: u32, guid: &str) -> Result<Value> {
    ensure!(!guid.is_empty(), "Mixer preset UUID is absent");
    let source = defaults(product_id)?;
    Ok(
        json!({"name":source["preset_name"],"guid":guid,"inputChannels":source["inputChannels"],"outputChannel":source["outputChannel"],"shortcutMapping":source["shortcutMapping"]}),
    )
}
pub fn prepare_cache(value: Option<Value>, product_id: u32, guid: &str) -> Result<(Value, bool)> {
    let source = defaults(product_id)?;
    let mut changed = false;
    let mut cache = match value.filter(|value| !value.is_null() && *value != false) {
        Some(value) => value,
        None => {
            changed = true;
            json!({"activePresetGuid":guid,"version":source["version"],"presets":[default_preset(product_id,guid)?],"devices":[],"userId":source["userId"]})
        }
    };
    let devices = cache["devices"]
        .as_array_mut()
        .context("Mixer cache devices is not an array")?;
    if !devices
        .iter()
        .any(|device| device["productId"] == product_id)
    {
        devices.push(
            json!({"isStreamMixerEnabled":source["isStreamMixerEnabled"],"productId":product_id}),
        );
        changed = true;
    }
    active_preset(&cache)?;
    Ok((cache, changed))
}
pub fn active_preset(cache: &Value) -> Result<&Value> {
    let guid = cache["activePresetGuid"]
        .as_str()
        .context("Mixer cache active preset is absent")?;
    cache["presets"]
        .as_array()
        .context("Mixer cache presets is not an array")?
        .iter()
        .find(|preset| preset["guid"] == guid)
        .context("Mixer active preset does not exist")
}
fn active_mut(cache: &mut Value) -> Result<&mut Value> {
    let guid = cache["activePresetGuid"]
        .as_str()
        .context("Mixer cache active preset is absent")?
        .to_owned();
    cache["presets"]
        .as_array_mut()
        .context("Mixer cache presets is not an array")?
        .iter_mut()
        .find(|preset| preset["guid"] == guid)
        .context("Mixer active preset does not exist")
}
fn next_version(cache: &mut Value) -> Result<()> {
    cache["version"] = json!(match cache.get("version") {
        None => 1,
        Some(version) => {
            let version = version
                .as_u64()
                .context("Mixer version is not a 53-bit integer")?;
            ensure!(
                version <= 9_007_199_254_740_991,
                "Mixer version exceeds source 53-bit range"
            );
            if version == 9_007_199_254_740_991 {
                0
            } else {
                version + 1
            }
        }
    });
    Ok(())
}
fn assign(target: &mut Value, extra: &Value) -> Result<()> {
    let target = target
        .as_object_mut()
        .context("Mixer target is not an object")?;
    for (key, value) in extra.as_object().context("Mixer source is not an object")? {
        target.insert(key.clone(), value.clone());
    }
    Ok(())
}
/// Original updateStorage runs before device work for ordinary controls.
/// Preset operations own their saves inside execute. Keep version increments
/// exactly at the source sites (add/rename/select/mapping do not increment).
pub fn apply_cache(
    cache: &mut Value,
    product_id: u32,
    action: &Action,
    reset_guid: &str,
) -> Result<bool> {
    let argument = action.argument()?;
    let event = action.event();
    if let Some(enabled) = action.enabled()? {
        let device = cache["devices"]
            .as_array_mut()
            .context("Mixer devices is not an array")?
            .iter_mut()
            .find(|device| device["productId"] == product_id)
            .context("Mixer current device is absent")?;
        device["isStreamMixerEnabled"] = json!(enabled);
        next_version(cache)?;
        return Ok(true);
    }
    if matches!(
        event,
        "ON_SET_STREAM_MIX_VOLUME"
            | "ON_SET_PLAYBACK_MIX_VOLUME"
            | "ON_SET_STREAM_MIX_MONITOR"
            | "ON_CHANGE_PLAY_BACK_MIX_DEVICE"
            | "ON_MODIFY_INPUT_CHANNEL"
    ) {
        let preset = active_mut(cache)?;
        match event {
            "ON_SET_STREAM_MIX_VOLUME" | "ON_SET_PLAYBACK_MIX_VOLUME" => {
                let field = if event == "ON_SET_STREAM_MIX_VOLUME" {
                    "streamMixVolume"
                } else {
                    "playbackMixVolume"
                };
                preset["outputChannel"][field] =
                    json!({"isEnabled":argument["isEnabled"],"value":argument["value"]});
            }
            "ON_SET_STREAM_MIX_MONITOR" => {
                preset["outputChannel"]["isStreamMixMonitor"] = argument.clone()
            }
            "ON_CHANGE_PLAY_BACK_MIX_DEVICE" => {
                preset["outputChannel"]["playbackMixDevice"] = argument.clone()
            }
            "ON_MODIFY_INPUT_CHANNEL" => {
                let change = &argument["modifyInputChannel"];
                for channel in preset["inputChannels"]
                    .as_array_mut()
                    .context("Mixer inputs is not an array")?
                {
                    if channel["name"] == change["name"] {
                        assign(channel, change)?;
                        for field in ["streamMixerInputVolume", "playbackMixInputVolume"] {
                            if let Some(value) = change.get(field) {
                                channel[field] =
                                    json!({"isEnabled":value["isEnabled"],"value":value["value"]});
                            }
                        }
                    }
                }
            }
            _ => unreachable!(),
        }
        next_version(cache)?;
        return Ok(true);
    }
    match event {
        "ON_ADD_STREAM_MIXER_PRESET" => {
            let mut preset = default_preset(product_id, reset_guid)?;
            assign(&mut preset, &argument["newPreset"])?;
            cache["activePresetGuid"] = preset["guid"].clone();
            cache["presets"]
                .as_array_mut()
                .context("Mixer presets is not an array")?
                .push(preset);
        }
        "ON_DELETE_STREAM_MIXER_PRESET" => {
            let presets = cache["presets"]
                .as_array_mut()
                .context("Mixer presets is not an array")?;
            if presets.len() <= 1 {
                return Ok(false);
            }
            presets.retain(|preset| preset["guid"] != argument["deletedPresetId"]);
            cache["activePresetGuid"] = argument["selectedPresetGuid"].clone();
            next_version(cache)?;
        }
        "ON_RESET_STREAM_MIXER_PRESET" => {
            for preset in cache["presets"]
                .as_array_mut()
                .context("Mixer presets is not an array")?
            {
                if preset["guid"] == argument["selectedPresetGuid"] {
                    let mut reset = default_preset(product_id, reset_guid)?;
                    reset["guid"] = preset["guid"].clone();
                    reset["name"] = preset["name"].clone();
                    *preset = reset;
                }
            }
            cache["activePresetGuid"] = argument["selectedPresetGuid"].clone();
            next_version(cache)?;
        }
        "ON_RENAME_STREAM_MIXER_PRESET" => {
            for preset in cache["presets"]
                .as_array_mut()
                .context("Mixer presets is not an array")?
            {
                if preset["guid"] == argument["selectedPresetGuid"] {
                    preset["name"] = argument["newName"].clone();
                }
            }
        }
        "ON_DUPLICATE_STREAM_MIXER_PRESET" => {
            let Some(mut preset) = cache["presets"]
                .as_array()
                .context("Mixer presets is not an array")?
                .iter()
                .find(|preset| preset["guid"] == argument["selectedPresetGuid"])
                .cloned()
            else {
                return Ok(false);
            };
            assign(&mut preset, &argument["newPreset"])?;
            cache["activePresetGuid"] = preset["guid"].clone();
            cache["presets"].as_array_mut().unwrap().push(preset);
            next_version(cache)?;
        }
        "ON_SELECTED_STREAM_MIXER_PRESET" => {
            cache["activePresetGuid"] = argument["selectedPresetGuid"].clone()
        }
        "ON_CHANGE_STREAM_MIXER_PRESET_MAPPING" => {
            for preset in cache["presets"]
                .as_array_mut()
                .context("Mixer presets is not an array")?
            {
                if preset["guid"] == argument["changePresetGUID"] {
                    preset["shortcutMapping"] =
                        if argument["newShortcutMapping"].get("inputID").is_none() {
                            json!({})
                        } else {
                            argument["newShortcutMapping"].clone()
                        };
                }
            }
        }
        _ => anyhow::bail!("Unimplemented current mixer cache event: {event}"),
    }
    active_preset(cache)?;
    Ok(true)
}
pub fn ui_payload(cache: &Value, product_id: u32, headphones: &str) -> Result<Value> {
    let preset = active_preset(cache)?;
    let current = cache["devices"]
        .as_array()
        .context("Mixer devices is not an array")?
        .iter()
        .find(|device| device["productId"] == product_id)
        .context("Mixer current device is absent")?;
    let mut output = json!({"presets":cache["presets"],"selectedPresetGuid":cache["activePresetGuid"],"isStreamMixerEnabled":current["isStreamMixerEnabled"],"inputChannels":preset["inputChannels"]});
    assign(&mut output, &preset["outputChannel"])?;
    if output["playbackMixDevice"]
        .as_str()
        .is_none_or(str::is_empty)
    {
        output["playbackMixDevice"] = json!(headphones);
    }
    Ok(output)
}

#[derive(Clone, Debug, PartialEq)]
pub enum Operation {
    Native(NativeOperation),
    Playback(String),
    External { channel: String, device: String },
}

/// Current updateMixInput assigns each remove/edit native return to `o`.
/// External routing then ANDs that last value; add uses Promise.all().every().
/// Keep the final source return separate from diagnostics for earlier calls.
pub fn operation_result(action: &Action, results: &[(Operation, bool)]) -> Result<bool> {
    let add = action.event() == "ON_MODIFY_INPUT_CHANNEL" && action.argument()?["action"] == "add";
    let mut result = add && !results.is_empty();
    for (operation, succeeded) in results {
        result = if add || matches!(operation, Operation::External { .. }) {
            result && *succeeded
        } else {
            *succeeded
        };
    }
    Ok(result)
}
fn level(value: &Value) -> Result<u32> {
    let number = value.as_f64().context("Mixer level is not numeric")?;
    ensure!(
        number.is_finite() && (0. ..=100.).contains(&number),
        "Mixer level outside source range"
    );
    Ok(number as u32)
}
fn flag(value: &Value) -> Result<bool> {
    value.as_bool().context("Mixer flag is not a boolean")
}
fn input_id(product_id: u32, name: &str) -> Result<u32> {
    let source = defaults(product_id)?;
    let key = source["nameToDeviceId"][name]
        .as_str()
        .context("Mixer input name has no source native ID")?;
    Ok(source["nativeDeviceIds"][key]
        .as_u64()
        .context("Mixer input native ID is absent")? as u32)
}
/// Source initDevice uses playbackMixInputVolume for StreamMix and the reverse
/// for PlaybackMix. Preserve this observed source ordering, including its swap.
pub fn preset_operations(product_id: u32, preset: &Value) -> Result<Vec<Operation>> {
    let source = defaults(product_id)?;
    let stream = source["nativeDeviceIds"]["DeviceId_StreamMix"]
        .as_u64()
        .unwrap() as u32;
    let playback = source["nativeDeviceIds"]["DeviceId_PlaybackMix"]
        .as_u64()
        .unwrap() as u32;
    let output = &preset["outputChannel"];
    let mut operations = vec![Operation::Native(NativeOperation::SetStreamMixMonitor {
        enabled: flag(&output["isStreamMixMonitor"])?,
    })];
    for (id, field) in [(stream, "streamMixVolume"), (playback, "playbackMixVolume")] {
        operations.push(Operation::Native(NativeOperation::SetDeviceVolume {
            device_id: id,
            level: level(&output[field]["value"])?,
        }));
        operations.push(Operation::Native(NativeOperation::SetDeviceMute {
            device_id: id,
            muted: !flag(&output[field]["isEnabled"])?,
        }));
    }
    operations.push(Operation::Playback(
        output["playbackMixDevice"]
            .as_str()
            .context("Mixer output name is not a string")?
            .into(),
    ));
    for input in preset["inputChannels"]
        .as_array()
        .context("Mixer inputs is not an array")?
    {
        let name = input["name"]
            .as_str()
            .context("Mixer input name is absent")?;
        let input_id = input_id(product_id, name)?;
        let visible = flag(&input["isVisible"])?;
        for (output_id, field) in [
            (stream, "playbackMixInputVolume"),
            (playback, "streamMixerInputVolume"),
        ] {
            operations.push(Operation::Native(NativeOperation::SetMixLevel {
                output_id,
                input_id,
                level: level(&input[field]["value"])?,
            }));
            operations.push(Operation::Native(NativeOperation::SetMixEnable {
                output_id,
                input_id,
                enabled: visible && flag(&input[field]["isEnabled"])?,
            }));
        }
        if visible && input["inputChannelType"] == "configurable" {
            let device = input["exterInputDevice"]
                .as_str()
                .context("Mixer external input name is absent")?;
            if !device.is_empty() {
                operations.push(Operation::External {
                    channel: name.into(),
                    device: device.into(),
                });
            }
        }
    }
    Ok(operations)
}
pub fn action_operations(product_id: u32, action: &Action) -> Result<Vec<Operation>> {
    let value = action.argument()?;
    let mut operations = Vec::new();
    let ids = &defaults(product_id)?["nativeDeviceIds"];
    let stream = ids["DeviceId_StreamMix"].as_u64().unwrap() as u32;
    let playback = ids["DeviceId_PlaybackMix"].as_u64().unwrap() as u32;
    match action.event() {
        "ON_SET_STREAM_MIX_VOLUME" | "ON_SET_PLAYBACK_MIX_VOLUME" => {
            let device_id = if action.event() == "ON_SET_STREAM_MIX_VOLUME" {
                stream
            } else {
                playback
            };
            operations.push(Operation::Native(if value["isToggle"] == true {
                NativeOperation::SetDeviceMute {
                    device_id,
                    muted: !flag(&value["isEnabled"])?,
                }
            } else {
                NativeOperation::SetDeviceVolume {
                    device_id,
                    level: level(&value["value"])?,
                }
            }));
        }
        "ON_SET_STREAM_MIX_MONITOR" => {
            operations.push(Operation::Native(NativeOperation::SetStreamMixMonitor {
                enabled: flag(value)?,
            }))
        }
        "ON_CHANGE_PLAY_BACK_MIX_DEVICE" => operations.push(Operation::Playback(
            value
                .as_str()
                .context("Mixer output name is not a string")?
                .into(),
        )),
        "ON_MODIFY_INPUT_CHANNEL" => {
            // The mounted drag action carries an order list, not a channel
            // object. Current updateMixInput has no reorder case and returns
            // false without native writes; custom storage still versions it.
            if value["action"] == "reorder" {
                return Ok(operations);
            }
            let input = &value["modifyInputChannel"];
            let name = input["name"]
                .as_str()
                .context("Mixer input name is absent")?;
            let input_id = input_id(product_id, name)?;
            match value["action"]
                .as_str()
                .context("Mixer input action is absent")?
            {
                "add" => {
                    for output_id in [stream, playback] {
                        operations.push(Operation::Native(NativeOperation::SetMixEnable {
                            output_id,
                            input_id,
                            enabled: true,
                        }));
                        operations.push(Operation::Native(NativeOperation::SetMixLevel {
                            output_id,
                            input_id,
                            level: level(&input["streamMixerInputVolume"]["value"])?,
                        }));
                    }
                }
                "remove" => {
                    for output_id in [stream, playback] {
                        operations.push(Operation::Native(NativeOperation::SetMixEnable {
                            output_id,
                            input_id,
                            enabled: false,
                        }));
                    }
                }
                "edit" => {
                    for (output_id, field) in [
                        (stream, "streamMixerInputVolume"),
                        (playback, "playbackMixInputVolume"),
                    ] {
                        if let Some(volume) = input.get(field) {
                            operations.push(Operation::Native(if volume["isToggle"] == true {
                                NativeOperation::SetMixEnable {
                                    output_id,
                                    input_id,
                                    enabled: flag(&volume["isEnabled"])?,
                                }
                            } else {
                                NativeOperation::SetMixLevel {
                                    output_id,
                                    input_id,
                                    level: level(&volume["value"])?,
                                }
                            }));
                        }
                    }
                }
                other => anyhow::bail!("Unknown source mixer input action: {other}"),
            }
            if input["inputChannelType"] == "configurable" {
                let device = if value["action"] == "remove" {
                    ""
                } else {
                    input["exterInputDevice"].as_str().unwrap_or("")
                };
                if value["action"] != "add" || !device.is_empty() {
                    operations.push(Operation::External {
                        channel: name.into(),
                        device: device.into(),
                    });
                }
            }
        }
        _ => {}
    }
    Ok(operations)
}
pub fn endpoint_name<'a>(
    device_name: &str,
    role: &str,
    playback: &'a [String],
    record: &'a [String],
) -> &'a str {
    let (names, part) = match role {
        "Headphones" => (playback, "Headphones"),
        "PlaybackMix" => (record, "Playback Mix"),
        "Aux1" => (playback, "Aux 1"),
        "Aux2" => (playback, "Aux 2"),
        "Aux3" => (playback, "Aux 3"),
        _ => return "",
    };
    names
        .iter()
        .find(|name| name.contains(part) && name.contains(device_name))
        .map(String::as_str)
        .unwrap_or("")
}

#[cfg(test)]
#[path = "seiren_stream_mixer_tests.rs"]
mod tests;
