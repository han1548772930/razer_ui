//! PID756 current MW 42656 (Copilot), 15691 (firmware filtering), 72233
//! (preset analog values) and 6737 Df/Cf/wf. Preparation performs no write.
use crate::{
    keyboard_analog::{ActuationPoint, RapidTriggerKey},
    mapping_engine::{EngineContext, MappingEngine},
    mapping_engine_hash::{canonical_hash_input, source_hash},
    mapping_submission::next_source_version,
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeSet, sync::OnceLock};

#[path = "keyboard_latest_engine_runtime.rs"]
mod engine_runtime;
#[path = "keyboard_latest_features.rs"]
pub mod features;
#[path = "keyboard_latest_travel.rs"]
pub mod travel;

#[path = "keyboard_latest_restore_schema.rs"]
mod source_data;
use source_data::*;

fn typed_source() -> &'static LatestKeyboardSource {
    static DATA: OnceLock<LatestKeyboardSource> = OnceLock::new();
    DATA.get_or_init(|| make_keyboard_latest_restore_table())
}

/// Mapping engine and vendor profile interchange document.
pub fn source() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::to_value(typed_source()).expect("typed keyboard interchange document")
    })
}

pub fn firmware_key_id(input: &str) -> Option<u64> {
    typed_source()
        .firmware_keyboard
        .normal_mode
        .get(input)
        .and_then(|key| key.fw_id)
}

pub fn is_onboard_preset(guid: &str) -> bool {
    typed_source()
        .onboard_presets
        .iter()
        .any(|profile| profile.guid == guid)
}

pub fn guest_profile_owner() -> &'static str {
    &typed_source().guest_profile_owner
}

/// Source-generated OBM assignment identities, retained by the connected
/// owner. Current26192.NA ->obmEngine.init/getButtonIdFromUIMappingList uses
/// the product BUTTON_LIST and NormalMode firmware table. This is not a
/// device read. setSingleButtonMapping changes each assignment's payload
/// after ACK, preserving these key/input identities across profile edits.
#[derive(Clone, Debug)]
pub struct ObmButtonAssignments {
    keys: Vec<(u8, String)>,
}
impl ObmButtonAssignments {
    pub fn from_source() -> anyhow::Result<Self> {
        Self::from_mapping_list(
            source()["button_list"]
                .as_array()
                .context("756 OBM source button list is absent")?,
        )
    }
    pub fn from_mapping_list(rows: &[Value]) -> anyhow::Result<Self> {
        let mut keys = std::collections::BTreeMap::new();
        for row in rows.iter().filter(|row| !truthy(&row["isHyperShift"])) {
            let Some(input) = row["inputID"].as_str() else {
                continue;
            };
            let Some(id) = firmware_key_id(input) else {
                continue;
            };
            // The source skips unknown IDs and deduplicates by firmware ID.
            keys.entry(u8::try_from(id)?)
                .or_insert_with(|| input.to_owned());
        }
        ensure!(!keys.is_empty(), "756 OBM assignment identities are empty");
        Ok(Self {
            keys: keys.into_iter().collect(),
        })
    }
    pub fn keys(&self) -> &[(u8, String)] {
        &self.keys
    }
}
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(v) => v.as_f64().is_some_and(|v| v != 0.),
        Value::String(v) => !v.is_empty(),
        _ => true,
    }
}
fn hs(row: &Value) -> bool {
    row["isHyperShift"] == true
}
fn object(value: &Value) -> bool {
    value.is_object()
}
fn empty_object(value: &Value) -> bool {
    value.as_object().is_some_and(|v| v.is_empty())
}
fn missing(value: &Value) -> bool {
    value.is_null() || value.as_str().is_some_and(|v| v.trim().is_empty())
}
fn same_scalar(left: &Value, right: &Value) -> bool {
    let text = |v: &Value| {
        v.as_str()
            .map(str::to_owned)
            .or_else(|| v.as_number().map(ToString::to_string))
    };
    text(left).is_some() && text(left) == text(right)
}
fn modifiers(group: &Value, names: &[&str]) -> bool {
    group["modifiers"].as_array().is_some_and(|list| {
        list.len() == names.len() && names.iter().all(|name| list.iter().any(|v| v == name))
    })
}
fn copilot_group(group: &Value) -> bool {
    object(group)
        && group["key"] == "KEY_F23"
        && modifiers(group, &["KEY_LEFT_GUI", "KEY_LEFT_SHIFT"])
        && (group.get("keyGroup").is_none() || group["keyGroup"] == 0)
        && group.as_object().is_some_and(|o| {
            o.keys()
                .all(|k| matches!(k.as_str(), "key" | "keyGroup" | "modifiers"))
        })
}
fn default_for(row: &Value) -> Option<&'static Value> {
    let defaults = source()["default_mappings"].as_array()?;
    let compatible = |candidate: &&Value| {
        candidate["inputID"] == row["inputID"]
            && hs(candidate) == hs(row)
            && candidate["outputType"] == "keyboardGroup"
            && candidate["keyboardGroup"]["key"] == "KEY_F23"
            && modifiers(
                &candidate["keyboardGroup"],
                &["KEY_LEFT_GUI", "KEY_LEFT_SHIFT"],
            )
            && match row["inputType"].as_str() {
                Some("DKMInput") => candidate["inputType"] == "DKMInput",
                Some("KeyInput" | "AnalogInput") => matches!(
                    candidate["inputType"].as_str(),
                    Some("KeyInput" | "AnalogInput")
                ),
                _ => candidate["inputType"] == row["inputType"],
            }
    };
    defaults
        .iter()
        .filter(compatible)
        .find(|candidate| candidate["inputType"] == row["inputType"])
        .or_else(|| defaults.iter().find(compatible))
}
fn copilot_output(default: &Value, obm: bool) -> anyhow::Result<Value> {
    if !obm {
        return Ok(json!({"outputType":"keyboardGroup","keyboardGroup":default["keyboardGroup"]}));
    }
    let key = MappingEngine::new(756)?.source_key_metadata("KEY_F23")?;
    ensure!(
        !key["HID"].is_null() && !key["flag"].is_null() && !key["pageID"].is_null(),
        "Copilot F23 key metadata is incomplete"
    );
    Ok(
        json!({"outputType":"keyboardGroup","keyboardGroup":{"key":{"HID":key["HID"],"flag":key["flag"],"pageID":key["pageID"]},"modifiers":["Left_GUI","Left_Shift"]}}),
    )
}
fn has_custom(row: &Value) -> bool {
    row.as_object().is_some_and(|object| {
        object.keys().any(|key| {
            let key = key.to_lowercase();
            key != "keyboardgroup"
                && key != "defaultgroup"
                && (key.contains("turbo")
                    || key.contains("repeat")
                    || key.contains("mouse")
                    || key.contains("macro")
                    || key.contains("controller")
                    || key.ends_with("group"))
        })
    })
}
fn self_mapping(group: &Value, default: &Value) -> bool {
    if default["inputType"] == "DKMInput" {
        return false;
    }
    let meaningful = |value: &Value| match value {
        Value::String(v) => !v.trim().is_empty() && v != "Extended_Key",
        Value::Null => false,
        _ => true,
    };
    let mods = &group["modifiers"];
    if mods
        .as_array()
        .map(|v| v.iter().any(meaningful))
        .unwrap_or_else(|| meaningful(mods))
    {
        return false;
    }
    let key = &group["key"];
    if key
        .as_str()
        .is_some_and(|id| Some(id) == default["inputID"].as_str())
    {
        return true;
    }
    source()["button_list"].as_array().is_some_and(|list| {
        list.iter().any(|button| {
            button["inputID"] == default["inputID"]
                && hs(button) == hs(default)
                && object(key)
                && object(&button["keyInput"])
                && same_scalar(&key["HID"], &button["keyInput"]["HID"])
                && same_scalar(&key["pageID"], &button["keyInput"]["pageID"])
        })
    })
}
fn repair_output(output: &Value, default: &Value, obm: bool) -> anyhow::Result<Value> {
    if !object(output) {
        return Ok(output.clone());
    }
    let key = &output["keyboardGroup"]["key"];
    let invalid_key = key.is_null()
        || key.as_str().is_some_and(|v| v.trim().is_empty())
        || key.is_array()
        || key.is_number()
        || key.is_boolean()
        || (object(key) && missing(&key["HID"]));
    let broken180 = output["outputType"] == "perfect180Group"
        && object(&output["perfect180Group"])
        && output["perfect180Group"]["mouseAssignment"] == "Perfect180"
        && output["perfect180Group"]["reportCount"] == 0
        && output["perfect180Group"].get("direction").is_none()
        && (output["perfect180Group"].get("isEnabled").is_none()
            || output["perfect180Group"]["isEnabled"] == 0);
    if !(missing(&output["outputType"])
        || broken180
        || output["outputType"] == "keyboardGroup"
            && (!object(&output["keyboardGroup"])
                || invalid_key
                || self_mapping(&output["keyboardGroup"], default)))
    {
        return Ok(output.clone());
    }
    let replacement = copilot_output(default, obm)?;
    let mut repaired = output.clone();
    if let Some(old) = output["outputType"]
        .as_str()
        .filter(|s| !s.trim().is_empty() && *s != "keyboardGroup")
    {
        repaired.as_object_mut().unwrap().remove(old);
    }
    if missing(&output["outputType"]) {
        repaired.as_object_mut().unwrap().remove("perfect180Group");
    }
    repaired["outputType"] = replacement["outputType"].clone();
    repaired["keyboardGroup"] = replacement["keyboardGroup"].clone();
    Ok(repaired)
}

/// Current k/$: repair malformed output then materialize default Copilot. The
/// UI-only representation is kept separate by `ui_mappings` below.
pub fn materialize(rows: &[Value], obm: bool) -> anyhow::Result<Vec<Value>> {
    rows.iter()
        .map(|row| {
            if !object(row) || row["defaultGroup"] == true {
                return Ok(row.clone());
            }
            let Some(default) = default_for(row) else {
                return Ok(row.clone());
            };
            let mut result = row.clone();
            if row["inputType"] == "AnalogInput" {
                let Some(stages) = row["mapping"].as_array().filter(|v| !v.is_empty()) else {
                    return Ok(result);
                };
                let mut output = repair_output(&stages[0], default, obm)?;
                if output["outputType"] == "keyboardGroup" && empty_object(&output["keyboardGroup"])
                {
                    output["keyboardGroup"] =
                        copilot_output(default, obm)?["keyboardGroup"].clone();
                }
                result["mapping"][0] = output;
            } else {
                result = repair_output(row, default, obm)?;
                if result["outputType"] == "defaultGroup"
                    || result["outputType"] == "keyboardGroup"
                        && empty_object(&result["keyboardGroup"])
                {
                    result["outputType"] = json!("keyboardGroup");
                    result["keyboardGroup"] =
                        copilot_output(default, obm)?["keyboardGroup"].clone();
                }
            }
            Ok(result)
        })
        .collect()
}

pub fn ui_mappings(rows: &[Value]) -> Vec<Value> {
    rows.iter()
        .map(|row| {
            let mut result = row.clone();
            if !object(row)
                || row["defaultGroup"] == true
                || has_custom(row)
                || !default_for(row).is_some_and(|v| copilot_group(&v["keyboardGroup"]))
            {
                return result;
            }
            if row["inputType"] == "AnalogInput" {
                let Some(stages) = row["mapping"].as_array().filter(|v| !v.is_empty()) else {
                    return result;
                };
                if stages[0]["outputType"] == "keyboardGroup"
                    && copilot_group(&stages[0]["keyboardGroup"])
                    && !has_custom(&stages[0])
                    && stages[1..].iter().all(|v| {
                        v.is_null()
                            || object(v) && !has_custom(v) && v["outputType"] == "defaultGroup"
                    })
                {
                    result["mapping"][0]["keyboardGroup"] = json!({});
                }
            } else if row["outputType"] == "keyboardGroup" && copilot_group(&row["keyboardGroup"]) {
                result.as_object_mut().unwrap().remove("keyboardGroup");
                result["outputType"] = json!("defaultGroup");
            }
            result
        })
        .collect()
}

fn mapping_path(profile: &Value) -> anyhow::Result<Vec<String>> {
    if let Some(keymaps) = profile["keymaps"]
        .as_array()
        .filter(|list| !list.is_empty())
    {
        let index = keymaps
            .iter()
            .position(|keymap| keymap["guid"] == profile["activeKeymapId"])
            .unwrap_or(0);
        if keymaps[index]["mappings"].is_array() {
            return Ok(vec!["keymaps".into(), index.to_string(), "mappings".into()]);
        }
    }
    ensure!(
        profile["mappings"].is_array(),
        "Software Copilot active mappings are absent"
    );
    Ok(vec!["mappings".into()])
}
fn at<'a>(value: &'a Value, path: &[String]) -> &'a Value {
    path.iter().fold(value, |value, key| {
        if let Ok(i) = key.parse::<usize>() {
            &value[i]
        } else {
            &value[key]
        }
    })
}
fn at_mut<'a>(value: &'a mut Value, path: &[String]) -> &'a mut Value {
    let Some((first, rest)) = path.split_first() else {
        return value;
    };
    let child = if let Ok(i) = first.parse::<usize>() {
        &mut value[i]
    } else {
        &mut value[first]
    };
    at_mut(child, rest)
}
pub fn active_mappings(profile: &Value) -> anyhow::Result<&[Value]> {
    Ok(at(profile, &mapping_path(profile)?)
        .as_array()
        .context("Active mappings are invalid")?)
}

/// Recovery replaces only matching cached engine inputs and preserves every
/// unrelated engine field. A malformed cache fails before any device write.
pub fn recover_document(document: &Value, context: &mut EngineContext) -> anyhow::Result<Value> {
    let profiles = document["profiles"]
        .as_array()
        .context("Software Copilot profiles are missing")?;
    let Some(index) = profiles
        .iter()
        .position(|profile| profile["guid"] == document["activeProfile"])
    else {
        return Ok(document.clone());
    };
    let profile = &profiles[index];
    let path = mapping_path(profile)?;
    let old = at(profile, &path).as_array().unwrap();
    let repaired = materialize(old, false)?;
    let changed = old
        .iter()
        .zip(&repaired)
        .filter_map(|(old, new)| (old != new).then_some(new))
        .collect::<Vec<_>>();
    if changed.is_empty() {
        return Ok(document.clone());
    }
    let cached = profile["appEngine"]["mappings"]
        .as_array()
        .filter(|m| !m.is_empty())
        .context("Software Copilot cannot recover without cached appEngine mappings")?;
    let mut draft_context = context.clone();
    let engine = MappingEngine::new(756)?;
    let mut outputs = Vec::new();
    for row in changed {
        let secondary = row["inputType"] == "AnalogInput"
            && row["mapping"].as_array().is_some_and(|stages| {
                stages[1..].iter().any(|stage| {
                    object(stage)
                        && (stage["outputType"] != "defaultGroup"
                            || stage.get("turboMode").is_some()
                            || stage.get("repeat").is_some())
                })
            });
        let mut primary = row.clone();
        if row["inputType"] == "AnalogInput" {
            let first = row["mapping"][0]
                .as_object()
                .context("Software Copilot analog primary output is absent")?;
            primary.as_object_mut().unwrap().extend(first.clone());
            primary.as_object_mut().unwrap().remove("mapping");
        }
        let generated = engine.record(&primary, &mut draft_context)?;
        ensure!(
            !generated.is_empty(),
            "Software Copilot recovery returned no engine output"
        );
        outputs.extend(generated.into_iter().map(|value| (value, secondary)));
    }
    let mut used = BTreeSet::new();
    let mut mappings = Vec::with_capacity(cached.len());
    for cached in cached {
        let input = canonical_hash_input(&cached["input"], &[])?;
        let found = outputs.iter().enumerate().find(|(index, (output, _))| {
            !used.contains(index)
                && canonical_hash_input(&output["input"], &[]).is_ok_and(|value| value == input)
        });
        let Some((index, (output, preserve))) = found else {
            mappings.push(cached.clone());
            continue;
        };
        used.insert(index);
        let mut updated = cached.clone();
        updated["input"] = output["input"].clone();
        if *preserve {
            ensure!(
                updated["output"]["type"] == "triggers"
                    && object(&updated["output"]["outputs"]["1"])
                    && updated["output"]["outputs"]["1"].get("mapping").is_some(),
                "Software Copilot cannot preserve secondary engine output"
            );
            updated["output"]["outputs"]["1"]["mapping"] = output["output"].clone();
        } else {
            updated["output"] = output["output"].clone();
        }
        mappings.push(updated);
    }
    ensure!(
        used.len() == outputs.len(),
        "Software Copilot cannot match every repaired cached engine input"
    );
    let mut result = document.clone();
    let active = &mut result["profiles"][index];
    *at_mut(active, &path) = json!(repaired);
    active["appEngine"]["mappings"] = json!(mappings);
    active["appEngine"]["hash"] = json!(source_hash(&active["appEngine"])?);
    result["version"] = json!(next_source_version(
        document
            .get("version")
            .map(|v| v.as_u64().context("Invalid source version"))
            .transpose()?
    )?);
    *context = draft_context;
    Ok(result)
}

pub fn firmware_mappings(rows: &[Value]) -> Vec<Value> {
    crate::keyboard_firmware_mapping::firmware_mappings_with_options(rows, true)
}

pub fn default_profile() -> Value {
    source()["default_profile"].clone()
}

pub fn source_context(
    previous: Option<&Value>,
    fresh_guids: impl IntoIterator<Item = String>,
) -> EngineContext {
    let mut context = EngineContext {
        default_turbos: previous
            .and_then(|value| value["defaultTurbos"].as_array())
            .cloned(),
        synapse_turbos: Some(
            previous
                .and_then(|value| value["synapseTurbos"].as_array())
                .cloned()
                .unwrap_or_default(),
        ),
        fresh_guids: fresh_guids.into_iter().collect(),
        is_analog_gen2: true,
        analog_binary_range: Some(65535),
        analog_generation: None,
        ..Default::default()
    };
    // Current main26061 R: the mini keyboard exposes only this dial press.
    context.dkm_keys.insert(
        "DKM_KBMK_04".into(),
        json!({"inputID":"DKM_KBMK_04","buttonKey":"DIAL_CLICK","key":82}),
    );
    context
}

/// Compile complete source profiles with explicitly supplied device settings.
/// Transport identity belongs to the retained connection owner.
pub fn prepare_document(
    profiles: &[Value],
    active_profile: &str,
    layout: u32,
    previous: Option<&Value>,
    fresh_guids: impl IntoIterator<Item = String>,
) -> anyhow::Result<Value> {
    prepare_document_with_device_settings(
        profiles,
        active_profile,
        layout,
        previous,
        None,
        fresh_guids,
    )
}

pub fn prepare_document_with_device_settings(
    profiles: &[Value],
    active_profile: &str,
    layout: u32,
    previous: Option<&Value>,
    device_settings: Option<&Value>,
    fresh_guids: impl IntoIterator<Item = String>,
) -> anyhow::Result<Value> {
    let device_settings = device_settings
        .or(previous)
        .context("756 source device settings are absent")?;
    crate::keyboard_current_profile::high_speed::decode_latest_device_settings(device_settings)?;
    let normalized = profiles
        .iter()
        .map(|profile| -> anyhow::Result<Value> {
            crate::keyboard_current_profile::high_speed::decode_profile_for_product(756, profile)?;
            let mut value = profile.clone();
            if let Some(engine) = previous
                .and_then(|document| document["profiles"].as_array())
                .and_then(|profiles| profiles.iter().find(|old| old["guid"] == value["guid"]))
                .and_then(|profile| profile.get("appEngine"))
            {
                value["appEngine"] = engine.clone();
            }
            Ok(value)
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let mut document = previous.cloned().unwrap_or_else(|| json!({}));
    for &field in crate::keyboard_current_profile::high_speed::LatestDeviceSettings::FIELDS {
        document[field] = device_settings
            .get(field)
            .with_context(|| format!("756 source device field {field} is absent"))?
            .clone();
    }
    document["version"] = json!(next_source_version(
        previous.and_then(|document| document["version"].as_u64())
    )?);
    document["profiles"] = json!(normalized);
    document["activeProfile"] = json!(active_profile);
    document["vendorId"] = json!(5426);
    document["productId"] = json!(756);
    document["category"] = json!("KEYBOARD");
    document["_source_layout_id"] = json!(layout);
    // Ny excludes state from the versioned options hash. A global shortcut
    // changes the effective engine but must not version saved preferences.
    if let Some(previous) = previous {
        let comparable = |profile: &Value| {
            let mut value = profile.clone();
            if let Some(profile) = value.as_object_mut() {
                profile.remove("appEngine");
                if let Some(mode) = profile.get_mut("gamingMode").and_then(Value::as_object_mut) {
                    mode.remove("state");
                }
            }
            value
        };
        let unchanged_profiles = previous["profiles"]
            .as_array()
            .zip(document["profiles"].as_array())
            .is_some_and(|(old, new)| {
                old.len() == new.len()
                    && new.iter().all(|new| {
                        old.iter()
                            .find(|old| old["guid"] == new["guid"])
                            .is_some_and(|old| comparable(old) == comparable(new))
                    })
            });
        if unchanged_profiles
            && previous["activeProfile"] == document["activeProfile"]
            && previous["_source_layout_id"] == document["_source_layout_id"]
            && crate::keyboard_current_profile::high_speed::LatestDeviceSettings::FIELDS
                .iter()
                .filter(|&&field| field != "gameModeEnabled")
                .all(|&field| previous[field] == document[field])
        {
            document["version"] = previous["version"].clone();
        }
    }
    let mut context = source_context(previous, fresh_guids);
    compile_with_context(&document, &mut context, None)
}

pub fn compile(
    document: &Value,
    running_games: Option<usize>,
    fresh_guids: impl IntoIterator<Item = String>,
) -> anyhow::Result<Value> {
    compile_with_context(
        document,
        &mut source_context(Some(document), fresh_guids),
        running_games,
    )
}

/// The connected owner supplies the serial after a real metadata read. Local
/// previews have no connected travel-cap owner and use `compile` instead.
pub fn compile_for_serial(
    document: &Value,
    serial: &str,
    running_games: Option<usize>,
    fresh_guids: impl IntoIterator<Item = String>,
) -> anyhow::Result<Value> {
    compile_with_caps(
        document,
        &mut source_context(Some(document), fresh_guids),
        running_games,
        &travel::Caps::for_serial(document, serial)?,
        true,
    )
}

/// The firmware base publication precedes the generic feature restores.
pub fn compile_firmware_for_serial(
    document: &Value,
    serial: &str,
    running_games: Option<usize>,
    fresh_guids: impl IntoIterator<Item = String>,
) -> anyhow::Result<Value> {
    compile_with_caps(
        document,
        &mut source_context(Some(document), fresh_guids),
        running_games,
        &travel::Caps::for_serial(document, serial)?,
        false,
    )
}

/// FW analog publication skips4753 during ordinary restore. The source calls
/// it explicitly after a function-key-primary mutation, after gaming mode.
pub fn apply_function_key_snap_tap(document: &Value) -> anyhow::Result<Value> {
    let mut result = document.clone();
    let active = result["activeProfile"].clone();
    let defaults = result["defaultMappings"]["appEngine"].clone();
    let profiles = result["profiles"]
        .as_array_mut()
        .context("756 profiles are absent")?;
    let Some(index) = profiles
        .iter()
        .rposition(|profile| profile["guid"] == active)
    else {
        return Ok(result);
    };
    let mut profile = profiles.remove(index);
    engine_runtime::snap_tap(
        &MappingEngine::new(756)?,
        &mut profile,
        &defaults,
        &source_context(Some(document), []),
    )?;
    profiles.push(profile);
    Ok(result)
}

// Current6258 zm fills empty keyboard assignments before analog generation.
fn normalize_engine_rows(rows: &[Value]) -> anyhow::Result<Vec<Value>> {
    crate::keyboard_firmware_mapping::normalize_engine_rows(&materialize(rows, false)?)
}

pub fn compile_with_context(
    document: &Value,
    context: &mut EngineContext,
    running_games: Option<usize>,
) -> anyhow::Result<Value> {
    compile_with_caps(
        document,
        context,
        running_games,
        &travel::Caps::default(),
        true,
    )
}

fn compile_with_caps(
    document: &Value,
    context: &mut EngineContext,
    running_games: Option<usize>,
    caps: &travel::Caps,
    generic_enabled: bool,
) -> anyhow::Result<Value> {
    ensure!(
        document["productId"] == 756,
        "756 compiler document product differs"
    );
    crate::keyboard_current_profile::high_speed::decode_latest_device_settings(document)?;
    let layout = document["_source_layout_id"]
        .as_u64()
        .context("756 keyboard layout is absent")?;
    let active = document["activeProfile"]
        .as_str()
        .context("756 active profile is absent")?;
    let mut draft = context.clone();
    let factory = MappingEngine::new(756)?;
    if draft.default_turbos.is_none() {
        factory.generate_default_turbos(&mut draft)?;
    }
    draft.synapse_turbos.get_or_insert_with(Vec::new);
    let mut defaults = factory
        .generate(
            source()["default_mappings"]
                .as_array()
                .context("756 source defaults are absent")?,
            &mut draft,
        )?
        .app_engine;
    for hypershift in [false, true] {
        for flag in [0, 1] {
            defaults["mappings"].as_array_mut().unwrap().push(json!({
        "input":{"type":"razerKey","key":1,"hypershift":hypershift,"flag":flag},"output":{"type":"hypershift","flag":flag}
    }));
        }
    }
    defaults["hash"] = json!(source_hash(&defaults)?);
    let mut result = document.clone();
    let profiles = result["profiles"]
        .as_array_mut()
        .context("756 local profiles are absent")?;
    ensure!(
        profiles
            .iter()
            .filter(|profile| profile["guid"] == active)
            .count()
            == 1,
        "756 active profile is not unique"
    );
    let mut guids = BTreeSet::new();
    for profile in profiles.iter_mut() {
        crate::keyboard_current_profile::high_speed::decode_profile_for_product(756, profile)?;
        ensure!(
            guids.insert(
                profile["guid"]
                    .as_str()
                    .context("756 profile GUID is invalid")?
                    .to_owned()
            ),
            "756 local profile GUID is repeated"
        );
        let replaced = profile["appEngine"]["mappings"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|mapping| mapping["output"]["type"] == "turbo")
            .filter_map(|mapping| mapping["output"]["guid"].as_str().map(str::to_owned))
            .collect::<BTreeSet<_>>();
        draft.synapse_turbos.as_mut().unwrap().retain(|turbo| {
            !turbo["guid"]
                .as_str()
                .is_some_and(|guid| replaced.contains(guid))
        });
        // Generic61450 runs after the base firmware preparation and rebuilds
        // the active engine directly from its runtime rows. Re-filtering these
        // through15691 would discard the source's keyboard/gamepad overrides.
        let runtime_rows = if generic_enabled && profile["guid"] == active {
            engine_runtime::gamepad_rows(profile)?
        } else {
            None
        };
        let generic_gamepad = runtime_rows.is_some();
        let guid = profile["guid"]
            .as_str()
            .context("756 profile GUID is absent")?;
        let mut rows = runtime_rows
            .as_deref()
            .unwrap_or(active_mappings(profile)?)
            .to_vec();
        if profile["interruptKey"]["isEnabled"] == true {
            // The generic56839 restore path uses [] for ANY; the legacy
            // task-maker's KEY_ANY sentinel does not reach this current path.
            let interrupts = if profile["interruptKey"]["interruptionKey"] == "any"
                || profile["interruptKey"]["interrupts"]
                    .as_array()
                    .is_some_and(|keys| keys.iter().any(|key| key == "KEY_ANY"))
            {
                json!([])
            } else {
                profile["interruptKey"]["interrupts"].clone()
            };
            for row in &mut rows {
                if row["inputType"] == "AnalogInput"
                    && row["mapping"].as_array().is_some_and(|stages| {
                        stages.iter().any(|stage| {
                            matches!(
                                stage["outputType"].as_str(),
                                Some("macroGroup" | "dynamicKeyStrokeGroup")
                            )
                        })
                    })
                {
                    row["interrupts"] = interrupts.clone();
                }
            }
        }
        let rows = if generic_gamepad {
            rows
        } else {
            firmware_mappings(&rows)
        };
        let rows = normalize_engine_rows(&caps.rows(&rows, guid))?;
        let uses_backslash = layout == 12 || [1, 2, 5, 8, 9, 11, 19, 21].contains(&layout);
        let rows = rows
            .into_iter()
            .filter(|row| row["outputType"] != "modTapGroup")
            .filter(|row| {
                row["inputID"]
                    != if uses_backslash {
                        "KEY_NON_US_POUND"
                    } else {
                        "KEY_BACKSLASH"
                    }
            })
            .collect::<Vec<_>>();
        let (analog, ordinary): (Vec<_>, Vec<_>) = rows
            .into_iter()
            .partition(|row| row["inputType"] == "AnalogInput");
        let mut engine = factory.generate(&ordinary, &mut draft)?.app_engine;
        if !analog.is_empty() {
            let generated = factory.generate_analog(&analog, &mut draft)?.app_engine;
            for group in generated["mappings"]
                .as_array()
                .context("756 analog engine mappings are absent")?
            {
                let mut group = group
                    .as_array()
                    .context("756 analog engine group is invalid")?
                    .clone();
                caps.engine(&mut group)?;
                for mapping in &group {
                    // Current generateAppEngineFromProfileMapping suppresses an
                    // ordinary analog key mapped back to its own keyboard key.
                    if mapping["input"]["type"] == "analogKey"
                        && mapping["output"]["type"] == "keyboard"
                        && mapping["input"]["scancode"] == mapping["output"]["scancode"]
                        && mapping["input"]["hypershift"] != true
                    {
                        continue;
                    }
                    engine["mappings"]
                        .as_array_mut()
                        .unwrap()
                        .push(mapping.clone());
                }
            }
            for field in ["actuations", "rapidTriggers"] {
                if generic_gamepad && field == "rapidTriggers" {
                    continue;
                }
                if generated[field].as_array().is_some_and(|v| !v.is_empty())
                    || generated[field].as_object().is_some_and(|v| !v.is_empty())
                {
                    engine[field] = generated[field].clone();
                }
            }
        }
        engine["hash"] = json!(source_hash(&engine)?);
        profile["appEngine"] = engine;
    }
    result["defaultMappings"] = json!({"appEngine":defaults});
    result["defaultTurbos"] = json!(draft.default_turbos);
    result["synapseTurbos"] = json!(draft.synapse_turbos);
    let result = crate::keyboard_latest_gaming::apply(&result, running_games, true, &mut draft)?;
    *context = draft;
    Ok(result)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnalogPair {
    pub slot: u8,
    pub guid: String,
    pub actuation: Vec<ActuationPoint>,
    pub rapid_trigger: Vec<RapidTriggerKey>,
}
fn defaults(guid: &str, kind: &str, fallback: &str) -> &'static Value {
    source()[kind]
        .as_array()
        .and_then(|rows| rows.iter().find(|row| row[0] == guid))
        .map(|row| &row[1])
        .unwrap_or(&source()[fallback])
}
fn per_key<'a>(defaults: &'a Value, input: &str) -> &'a Value {
    defaults["specific"].get(input).unwrap_or(&defaults["rest"])
}
fn u16_value(value: &Value, label: &str) -> anyhow::Result<u16> {
    let value = value
        .as_f64()
        .with_context(|| format!("Invalid source {label}"))?;
    ensure!(
        value.is_finite() && value.fract() == 0. && (0.0..=65535.).contains(&value),
        "Source {label} exceeds u16 or is not integral"
    );
    Ok(value as u16)
}
fn raw_sensitivity(value: &Value, fallback: &Value) -> anyhow::Result<u16> {
    if let Some(value) = value.as_f64().filter(|v| *v != 0.) {
        let raw = (value * 1638.).floor();
        ensure!(
            raw.is_finite() && (0.0..=65535.0).contains(&raw),
            "Rapid trigger sensitivity is invalid"
        );
        Ok(raw as u16)
    } else {
        u16_value(fallback, "rapid trigger sensitivity")
    }
}
fn replace_key<T>(rows: &mut Vec<T>, value: T, key: impl Fn(&T) -> u8) {
    if let Some(old) = rows.iter_mut().find(|old| key(old) == key(&value)) {
        *old = value;
    } else {
        rows.push(value);
    }
}

/// Full physical defaults followed by removed then current mappings. Last
/// value wins while the first insertion position is retained (source Map).
pub fn analog_pair_with_caps(
    slot: u8,
    guid: &str,
    removed: &[Value],
    mappings: &[Value],
    caps: &travel::Caps,
) -> anyhow::Result<AnalogPair> {
    // Source X.dl bounds both stages before selecting the primary hardware
    // threshold. Clamping the final packet alone loses the two-stage gap.
    let bounded = caps.rows(mappings, guid);
    let mut pair = analog_pair(slot, guid, removed, &bounded)?;
    caps.analog_pair(&mut pair);
    Ok(pair)
}
pub fn analog_pair(
    slot: u8,
    guid: &str,
    removed: &[Value],
    mappings: &[Value],
) -> anyhow::Result<AnalogPair> {
    ensure!(
        slot > 0 && !guid.is_empty(),
        "Analog restore slot/GUID is absent"
    );
    let table = &typed_source().firmware_keyboard.normal_mode;
    let act = defaults(guid, "preset_actuation", "default_actuation");
    let rt = defaults(guid, "preset_rapid_trigger", "default_rapid_trigger");
    let mut pair = AnalogPair {
        slot,
        guid: guid.into(),
        actuation: Vec::new(),
        rapid_trigger: Vec::new(),
    };
    for row in &typed_source().physical_keys {
        let input = row.input_id.as_str();
        let Some(key) = u8::try_from(row.fw_id).ok().filter(|id| *id != 255) else {
            continue;
        };
        let d = &act["rest"];
        let r = per_key(rt, input);
        let primary = mappings
            .iter()
            .find(|row| row["inputID"] == input && row["inputType"] == "AnalogInput")
            .map(|row| &row["mapping"][0]);
        let threshold = |index: usize, field: &str| {
            primary
                .and_then(|primary| {
                    primary["actuationPoint"]
                        .get(index)
                        .or_else(|| primary["actuationPoint"].get(index.to_string()))
                })
                .filter(|value| !value.is_null())
                .unwrap_or(&d[field])
        };
        pair.actuation.push(ActuationPoint {
            key_id: key,
            threshold_high: u16_value(threshold(1, "thresholdH"), "default break actuation")?,
            threshold_low: u16_value(threshold(0, "thresholdL"), "default make actuation")?,
        });
        pair.rapid_trigger.push(if let Some(primary) = primary {
            RapidTriggerKey {
                key_id: key,
                enabled: u8::from(truthy(&primary["rapidTrigger"])),
                up_sensitivity: raw_sensitivity(
                    &primary["rapidTrigger"]["breakSensitivity"],
                    &r["upSensitivity"],
                )?,
                down_sensitivity: raw_sensitivity(
                    &primary["rapidTrigger"]["makeSensitivity"],
                    &r["downSensitivity"],
                )?,
            }
        } else {
            RapidTriggerKey {
                key_id: key,
                enabled: u8::from(truthy(&r["enabled"])),
                up_sensitivity: u16_value(&r["upSensitivity"], "default break sensitivity")?,
                down_sensitivity: u16_value(&r["downSensitivity"], "default make sensitivity")?,
            }
        });
    }
    for (rows, reset) in [(removed, true), (mappings, false)] {
        for row in rows {
            if row["inputType"] != "AnalogInput" || truthy(&row["isHyperShift"]) {
                continue;
            }
            let input = row["inputID"]
                .as_str()
                .context("Analog mapping input is missing")?;
            let Some(key) = table
                .get(input)
                .and_then(|v| v.fw_id)
                .and_then(|v| u8::try_from(v).ok())
            else {
                continue;
            };
            let d = &act["rest"];
            let r = per_key(rt, input);
            let primary = &row["mapping"][0];
            let (low, high) = if reset {
                (u16_value(&d["thresholdL"], "removed make actuation")?, 256)
            } else {
                (
                    u16_value(&primary["actuationPoint"]["0"], "mapping make actuation").or_else(
                        |_| u16_value(&primary["actuationPoint"][0], "mapping make actuation"),
                    )?,
                    u16_value(&primary["actuationPoint"]["1"], "mapping break actuation").or_else(
                        |_| u16_value(&primary["actuationPoint"][1], "mapping break actuation"),
                    )?,
                )
            };
            replace_key(
                &mut pair.actuation,
                ActuationPoint {
                    key_id: key,
                    threshold_high: high,
                    threshold_low: low,
                },
                |row| row.key_id,
            );
            let rapid = if reset {
                RapidTriggerKey {
                    key_id: key,
                    enabled: u8::from(truthy(&r["enabled"])),
                    up_sensitivity: u16_value(&r["upSensitivity"], "removed break sensitivity")?,
                    down_sensitivity: u16_value(&r["downSensitivity"], "removed make sensitivity")?,
                }
            } else {
                let rapid = &primary["rapidTrigger"];
                RapidTriggerKey {
                    key_id: key,
                    enabled: u8::from(truthy(rapid)),
                    up_sensitivity: raw_sensitivity(
                        &rapid["breakSensitivity"],
                        &r["upSensitivity"],
                    )?,
                    down_sensitivity: raw_sensitivity(
                        &rapid["makeSensitivity"],
                        &r["downSensitivity"],
                    )?,
                }
            };
            replace_key(&mut pair.rapid_trigger, rapid, |row| row.key_id);
        }
    }
    ensure!(
        !pair.actuation.is_empty(),
        "756 analog physical keyboard list is empty"
    );
    Ok(pair)
}

#[cfg(test)]
#[path = "keyboard_latest_mapping_tests.rs"]
mod tests;

// Generated typed table: razer-device/keyboard_latest_restore.
#[rustfmt::skip]
fn make_keyboard_latest_restore_table() -> LatestKeyboardSource {
    LatestKeyboardSource{
        product_id: 756u64,
        device_name: "Razer Reclusa Mini 65%".into(),
        guest_profile_owner: "5ed8944a85a9763fd315852f448cb7de36c5e928e13b3be427f98f7dc455f141".into(
        ),
        default_profile: LatestKeyboardSourceDefaultProfile{
            guid: "".into(),
            name: "".into(),
            actuation_point: vec![("0".into(),1.2f64),("1".into(),1.2f64)].into_iter().collect(),
            switch_off_lighting: LatestKeyboardSourceDefaultProfileSwitchofflighting{
                is_display_on: false,
                is_idle_enabled: false,
                idle_minutes: 1u64
            },
            brightness: LatestKeyboardSourceDefaultProfileBrightness{is_enabled:true,value:50u64},
            quick_effects: LatestKeyboardSourceDefaultProfileQuickeffects{
                selected_effect_id: 4u64,
                selected_effect_setting: LatestKeyboardSourceDefaultProfileQuickeffectsSelectedeffectsetting{
                    direction: 2u64
                }
            },
            polling_rate: 8000.0f64,
            polling_rate_wireless: 1000.0f64,
            gaming_mode: LatestKeyboardSourceDefaultProfileGamingmode{
                state: 0u64,
                is_windows_key_disabled: false,
                is_alt_tab_disabled: false,
                is_alt_4_disabled: false
            },
            snap_tap: LatestKeyboardSourceDefaultProfileSnaptap{
                is_enabled: false,
                key_list: vec![
                    LatestKeyboardSourceDefaultProfileSnaptapKeylistitem{
                        key_1: "KEY_A".into(),
                        key_2: "KEY_D".into(),
                        id: 1u64,
                        mode: "LAST_INPUT".into()
                    }
                ]
            },
            bottom_deadzone: LatestKeyboardSourceDefaultProfileBottomdeadzone{value:"medium".into()},
            continuous_rapid_trigger: false,
            r#macro: vec![].into_iter().collect(),
            mappings: vec![],
            calibrated_keys: vec![],
            is_enable_keyboard_keys: true,
            is_gamepad_override: true,
            single_key_snap_tap: LatestKeyboardSourceDefaultProfileSinglekeysnaptap{
                is_enabled: false,
                key_list: vec![]
            },
            interrupt_key: LatestKeyboardSourceDefaultProfileInterruptkey{
                is_enabled: false,
                interrupts: vec!["KEY_ANY".into()]
            },
            dual_keypress_priority: LatestKeyboardSourceDefaultProfileDualkeypresspriority{
                is_enabled: false,
                keys: vec![]
            }
        },
        default_mappings: vec![
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "keyboardGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_ESC".into(),
                keyboard_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemKeyboardgroup{
                        key: "KEY_TILDE".into(),
                        key_assignment: None,
                        modifiers: None
                    }
                ),
                multimedia_group: None,
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "keyboardGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_1".into(),
                keyboard_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemKeyboardgroup{
                        key: "KEY_F1".into(),
                        key_assignment: None,
                        modifiers: None
                    }
                ),
                multimedia_group: None,
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "keyboardGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_2".into(),
                keyboard_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemKeyboardgroup{
                        key: "KEY_F2".into(),
                        key_assignment: None,
                        modifiers: None
                    }
                ),
                multimedia_group: None,
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "keyboardGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_3".into(),
                keyboard_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemKeyboardgroup{
                        key: "KEY_F3".into(),
                        key_assignment: None,
                        modifiers: None
                    }
                ),
                multimedia_group: None,
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "keyboardGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_4".into(),
                keyboard_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemKeyboardgroup{
                        key: "KEY_F4".into(),
                        key_assignment: None,
                        modifiers: None
                    }
                ),
                multimedia_group: None,
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "keyboardGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_5".into(),
                keyboard_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemKeyboardgroup{
                        key: "KEY_F5".into(),
                        key_assignment: None,
                        modifiers: None
                    }
                ),
                multimedia_group: None,
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "keyboardGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_6".into(),
                keyboard_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemKeyboardgroup{
                        key: "KEY_F6".into(),
                        key_assignment: None,
                        modifiers: None
                    }
                ),
                multimedia_group: None,
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "keyboardGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_7".into(),
                keyboard_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemKeyboardgroup{
                        key: "KEY_F7".into(),
                        key_assignment: None,
                        modifiers: None
                    }
                ),
                multimedia_group: None,
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "keyboardGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_8".into(),
                keyboard_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemKeyboardgroup{
                        key: "KEY_F8".into(),
                        key_assignment: None,
                        modifiers: None
                    }
                ),
                multimedia_group: None,
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "keyboardGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_9".into(),
                keyboard_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemKeyboardgroup{
                        key: "KEY_F9".into(),
                        key_assignment: None,
                        modifiers: None
                    }
                ),
                multimedia_group: None,
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "keyboardGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_0".into(),
                keyboard_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemKeyboardgroup{
                        key: "KEY_F10".into(),
                        key_assignment: None,
                        modifiers: None
                    }
                ),
                multimedia_group: None,
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "keyboardGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_HYPEN".into(),
                keyboard_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemKeyboardgroup{
                        key: "KEY_F11".into(),
                        key_assignment: None,
                        modifiers: None
                    }
                ),
                multimedia_group: None,
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "keyboardGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_EQUAL".into(),
                keyboard_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemKeyboardgroup{
                        key: "KEY_F12".into(),
                        key_assignment: None,
                        modifiers: None
                    }
                ),
                multimedia_group: None,
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "multimediaGroup".into(),
                is_hyper_shift: false,
                input_type: "MouseInput".into(),
                input_id: "ScrollDown".into(),
                keyboard_group: None,
                multimedia_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemMultimediagroup{
                        multimedia_assignment: "VolumeUp".into(),
                        use_turbo: Some(false)
                    }
                ),
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "multimediaGroup".into(),
                is_hyper_shift: false,
                input_type: "DKMInput".into(),
                input_id: "DKM_KBMK_04".into(),
                keyboard_group: None,
                multimedia_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemMultimediagroup{
                        multimedia_assignment: "MuteVolume".into(),
                        use_turbo: None
                    }
                ),
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "multimediaGroup".into(),
                is_hyper_shift: false,
                input_type: "MouseInput".into(),
                input_id: "ScrollUp".into(),
                keyboard_group: None,
                multimedia_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemMultimediagroup{
                        multimedia_assignment: "VolumeDown".into(),
                        use_turbo: Some(false)
                    }
                ),
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "multimediaGroup".into(),
                is_hyper_shift: true,
                input_type: "MouseInput".into(),
                input_id: "ScrollDown".into(),
                keyboard_group: None,
                multimedia_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemMultimediagroup{
                        multimedia_assignment: "VolumeUp".into(),
                        use_turbo: Some(false)
                    }
                ),
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "multimediaGroup".into(),
                is_hyper_shift: true,
                input_type: "DKMInput".into(),
                input_id: "DKM_KBMK_04".into(),
                keyboard_group: None,
                multimedia_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemMultimediagroup{
                        multimedia_assignment: "MuteVolume".into(),
                        use_turbo: None
                    }
                ),
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "multimediaGroup".into(),
                is_hyper_shift: true,
                input_type: "MouseInput".into(),
                input_id: "ScrollUp".into(),
                keyboard_group: None,
                multimedia_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemMultimediagroup{
                        multimedia_assignment: "VolumeDown".into(),
                        use_turbo: Some(false)
                    }
                ),
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "systemGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_U".into(),
                keyboard_group: None,
                multimedia_group: None,
                system_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemSystemgroup{
                        system_assignment: "Sleep".into()
                    }
                ),
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "otfmGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_I".into(),
                keyboard_group: None,
                multimedia_group: None,
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "gameModeGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_O".into(),
                keyboard_group: None,
                multimedia_group: None,
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "keyboardGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_P".into(),
                keyboard_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemKeyboardgroup{
                        key: "KEY_PRINT_SCREEN".into(),
                        key_assignment: Some("Prnt Scrn".into()),
                        modifiers: Some(vec![])
                    }
                ),
                multimedia_group: None,
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "backlightGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_OPEN_SQUARE_BRACKET".into(),
                keyboard_group: None,
                multimedia_group: None,
                system_group: None,
                backlight_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemBacklightgroup{
                        backlight_assignment: "BrightnessDown".into()
                    }
                ),
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "backlightGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_CLOSE_SQUARE_BRACKET".into(),
                keyboard_group: None,
                multimedia_group: None,
                system_group: None,
                backlight_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemBacklightgroup{
                        backlight_assignment: "BrightnessUp".into()
                    }
                ),
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "keyboardGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_PAGE_UP".into(),
                keyboard_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemKeyboardgroup{
                        key: "KEY_HOME".into(),
                        key_assignment: None,
                        modifiers: None
                    }
                ),
                multimedia_group: None,
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "profileNavigationGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_J".into(),
                keyboard_group: None,
                multimedia_group: None,
                system_group: None,
                backlight_group: None,
                profile_navigation_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemProfilenavigationgroup{
                        profile_navigation_assignment: "Specific".into(),
                        guid: Some("af06d371-861f-4b98-8d78-bfaef5cfdebf".into())
                    }
                ),
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "profileNavigationGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_K".into(),
                keyboard_group: None,
                multimedia_group: None,
                system_group: None,
                backlight_group: None,
                profile_navigation_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemProfilenavigationgroup{
                        profile_navigation_assignment: "CycleUp".into(),
                        guid: None
                    }
                ),
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "multimediaGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_L".into(),
                keyboard_group: None,
                multimedia_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemMultimediagroup{
                        multimedia_assignment: "VolumeDown".into(),
                        use_turbo: None
                    }
                ),
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "multimediaGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_SEMICOLON".into(),
                keyboard_group: None,
                multimedia_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemMultimediagroup{
                        multimedia_assignment: "VolumeUp".into(),
                        use_turbo: None
                    }
                ),
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "multimediaGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_APOSTROPHE".into(),
                keyboard_group: None,
                multimedia_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemMultimediagroup{
                        multimedia_assignment: "MuteVolume".into(),
                        use_turbo: None
                    }
                ),
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "keyboardGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_PAGE_DOWN".into(),
                keyboard_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemKeyboardgroup{
                        key: "KEY_END".into(),
                        key_assignment: None,
                        modifiers: None
                    }
                ),
                multimedia_group: None,
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "disableGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_Z".into(),
                keyboard_group: None,
                multimedia_group: None,
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemDisablegroup{
                        disable_assignment: "Disable".into()
                    }
                ),
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "disableGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_X".into(),
                keyboard_group: None,
                multimedia_group: None,
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemDisablegroup{
                        disable_assignment: "Disable".into()
                    }
                ),
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "disableGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_C".into(),
                keyboard_group: None,
                multimedia_group: None,
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemDisablegroup{
                        disable_assignment: "Disable".into()
                    }
                ),
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "disableGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_V".into(),
                keyboard_group: None,
                multimedia_group: None,
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemDisablegroup{
                        disable_assignment: "Disable".into()
                    }
                ),
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "multimediaGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_COMMA".into(),
                keyboard_group: None,
                multimedia_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemMultimediagroup{
                        multimedia_assignment: "PrevTrack".into(),
                        use_turbo: None
                    }
                ),
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "multimediaGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_PERIOD".into(),
                keyboard_group: None,
                multimedia_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemMultimediagroup{
                        multimedia_assignment: "NextTrack".into(),
                        use_turbo: None
                    }
                ),
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "multimediaGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_SLASH".into(),
                keyboard_group: None,
                multimedia_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemMultimediagroup{
                        multimedia_assignment: "Play".into(),
                        use_turbo: None
                    }
                ),
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "snapTapGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_LEFT_SHIFT".into(),
                keyboard_group: None,
                multimedia_group: None,
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemSnaptapgroup{
                        snap_tap_assignment: "toggle".into()
                    }
                )
            },
            LatestKeyboardSourceDefaultMappingsitem{
                output_type: "keyboardGroup".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                input_id: "KEY_RIGHT_CTRL".into(),
                keyboard_group: Some(
                    LatestKeyboardSourceDefaultMappingsitemKeyboardgroup{
                        key: "KEY_F23".into(),
                        key_assignment: None,
                        modifiers: Some(vec!["KEY_LEFT_GUI".into(),"KEY_LEFT_SHIFT".into()])
                    }
                ),
                multimedia_group: None,
                system_group: None,
                backlight_group: None,
                profile_navigation_group: None,
                disable_group: None,
                snap_tap_group: None
            }
        ],
        full_mapping_list: vec![
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_ESC".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_1".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_2".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_3".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_4".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_5".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_6".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_7".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_8".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_9".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_0".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_HYPEN".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_EQUAL".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_BACKSPACE".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_PAGE_UP".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_TAB".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_Q".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_W".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_E".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_R".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_T".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_Y".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_U".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_I".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_O".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_P".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_OPEN_SQUARE_BRACKET".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_CLOSE_SQUARE_BRACKET".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_BACKSLASH".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_DELETE".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_PAGE_DOWN".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_CAPS_LOCK".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_A".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_S".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_D".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_F".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_G".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_H".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_J".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_K".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_L".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_SEMICOLON".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_APOSTROPHE".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_ENTER".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_LEFT_SHIFT".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_Z".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_X".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_C".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_V".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_B".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_N".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_M".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_COMMA".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_PERIOD".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_SLASH".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_RIGHT_SHIFT".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_UP_ARROW".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_LEFT_CTRL".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_LEFT_GUI".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_LEFT_ALT".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_SPACEBAR".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_RIGHT_ALT".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_RIGHT_CTRL".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_APPLICATION".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_LEFT_ARROW".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_DOWN_ARROW".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_RIGHT_ARROW".into(),
                is_hyper_shift: false,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_ESC".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_1".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_2".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_3".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_4".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_5".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_6".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_7".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_8".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_9".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_0".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_HYPEN".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_EQUAL".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_BACKSPACE".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_PAGE_UP".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_TAB".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_Q".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_W".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_E".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_R".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_T".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_Y".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_U".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_I".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_O".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_P".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_OPEN_SQUARE_BRACKET".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_CLOSE_SQUARE_BRACKET".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_BACKSLASH".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_DELETE".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_PAGE_DOWN".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_CAPS_LOCK".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_A".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_S".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_D".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_F".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_G".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_H".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_J".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_K".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_L".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_SEMICOLON".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_APOSTROPHE".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_ENTER".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_LEFT_SHIFT".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_Z".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_X".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_C".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_V".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_B".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_N".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_M".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_COMMA".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_PERIOD".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_SLASH".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_RIGHT_SHIFT".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_UP_ARROW".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_LEFT_CTRL".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_LEFT_GUI".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_LEFT_ALT".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_SPACEBAR".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_RIGHT_ALT".into(),
                is_hyper_shift: true,
                input_type: "KeyInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_RIGHT_CTRL".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_APPLICATION".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_LEFT_ARROW".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_DOWN_ARROW".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            },
            LatestKeyboardSourceFullMappingListitem{
                input_id: "KEY_RIGHT_ARROW".into(),
                is_hyper_shift: true,
                input_type: "AnalogInput".into(),
                output_type: "disableGroup".into(),
                disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup{
                    disable_assignment: "Disable".into()
                }
            }
        ],
        button_list: vec![
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_A".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "4".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "4".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_B".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "5".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "5".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_C".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "6".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "6".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_D".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "7".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "7".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_E".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "8".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "8".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_F".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "9".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "9".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_G".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "10".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "10".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_H".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "11".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "11".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_I".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "12".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "12".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_J".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "13".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "13".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_K".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "14".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "14".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_L".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "15".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "15".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_M".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "16".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "16".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_N".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "17".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "17".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_O".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "18".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "18".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_P".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "19".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "19".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_Q".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "20".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "20".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_R".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "21".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "21".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_S".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "22".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "22".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_T".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "23".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "23".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_U".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "24".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "24".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_V".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "25".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "25".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_W".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "26".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "26".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_X".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "27".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "27".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_Y".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "28".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "28".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_Z".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "29".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "29".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_1".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "30".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "30".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_2".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "31".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "31".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_3".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "32".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "32".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_4".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "33".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "33".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_5".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "34".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "34".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_6".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "35".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "35".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_7".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "36".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "36".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_8".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "37".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "37".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_9".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "38".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "38".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_0".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "39".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "39".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_ENTER".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "40".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "40".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_ESC".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "41".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "41".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_BACKSPACE".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "42".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "42".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_TAB".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "43".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "43".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_SPACEBAR".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "44".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "44".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_HYPEN".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "45".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "45".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_EQUAL".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "46".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "46".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_OPEN_SQUARE_BRACKET".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "47".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "47".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_CLOSE_SQUARE_BRACKET".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "48".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "48".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_BACKSLASH".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "49".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "49".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_SEMICOLON".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "51".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "51".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_APOSTROPHE".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "52".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "52".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_COMMA".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "54".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "54".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_PERIOD".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "55".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "55".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_SLASH".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "56".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "56".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_CAPS_LOCK".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "57".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "57".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_PAGE_UP".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "75".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "75".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec!["Extended_Key".into()],
                        extended_key: Some("Extended_00".into())
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_DELETE".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "76".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "76".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec!["Extended_Key".into()],
                        extended_key: Some("Extended_00".into())
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_PAGE_DOWN".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "78".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "78".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec!["Extended_Key".into()],
                        extended_key: Some("Extended_00".into())
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_RIGHT_ARROW".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "79".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "79".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec!["Extended_Key".into()],
                        extended_key: Some("Extended_00".into())
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_LEFT_ARROW".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "80".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "80".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec!["Extended_Key".into()],
                        extended_key: Some("Extended_00".into())
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_DOWN_ARROW".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "81".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "81".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec!["Extended_Key".into()],
                        extended_key: Some("Extended_00".into())
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_UP_ARROW".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "82".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "82".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec!["Extended_Key".into()],
                        extended_key: Some("Extended_00".into())
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_LEFT_CTRL".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "224".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "224".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_LEFT_SHIFT".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "225".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "225".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_LEFT_ALT".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "226".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "226".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_RIGHT_CTRL".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "228".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "228".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec!["Extended_Key".into()],
                        extended_key: Some("Extended_00".into())
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_RIGHT_SHIFT".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "229".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "229".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_RIGHT_ALT".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "230".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "230".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec!["Extended_Key".into()],
                        extended_key: Some("Extended_00".into())
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_RO".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "135".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "135".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: None,
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_A".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "4".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "4".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_B".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "5".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "5".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_C".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "6".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "6".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_D".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "7".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "7".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_E".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "8".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "8".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_F".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "9".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "9".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_G".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "10".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "10".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_H".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "11".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "74".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec!["Extended_Key".into()],
                        extended_key: Some("Extended_00".into())
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_I".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "12".into()
                },
                keyboard_group: None,
                output_type: Some("razerKeyGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: Some(
                    LatestKeyboardSourceButtonListitemRazerkeygroup{
                        razer_key_assignment: Some("MacroRecordKey".into())
                    }
                ),
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_J".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "13".into()
                },
                keyboard_group: None,
                output_type: Some("profileNavigationGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: Some(
                    LatestKeyboardSourceButtonListitemProfilenavigationgroup{
                        profile_navigation_assignment: "Specific".into(),
                        profile_id: Some(2u64)
                    }
                ),
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_K".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "14".into()
                },
                keyboard_group: None,
                output_type: Some("profileNavigationGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: Some(
                    LatestKeyboardSourceButtonListitemProfilenavigationgroup{
                        profile_navigation_assignment: "CycleUp".into(),
                        profile_id: None
                    }
                ),
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_L".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "15".into()
                },
                keyboard_group: None,
                output_type: Some("multimediaGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: Some(
                    LatestKeyboardSourceButtonListitemMultimediagroup{
                        multimedia_assignment: "VolumeDown".into()
                    }
                ),
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_M".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "16".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "16".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_N".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "17".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "17".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_O".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "18".into()
                },
                keyboard_group: None,
                output_type: Some("razerKeyGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: Some(
                    LatestKeyboardSourceButtonListitemRazerkeygroup{
                        razer_key_assignment: Some("GameModeKey".into())
                    }
                ),
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_P".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "19".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "70".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec!["Extended_Key".into()],
                        extended_key: Some("Extended_00".into())
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_Q".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "20".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "20".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_R".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "21".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "21".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_S".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "22".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "22".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_T".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "23".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "23".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_U".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "24".into()
                },
                keyboard_group: None,
                output_type: Some("powerKeyGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: Some(
                    LatestKeyboardSourceButtonListitemPowerkeygroup{
                        power_key_assignment: "SystemSleep".into()
                    }
                )
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_V".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "25".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "25".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_W".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "26".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "26".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_X".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "27".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "27".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_Y".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "28".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "73".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec!["Extended_Key".into()],
                        extended_key: Some("Extended_00".into())
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_Z".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "29".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "29".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_1".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "30".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "58".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_2".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "31".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "59".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_3".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "32".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "60".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_4".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "33".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "61".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_5".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "34".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "62".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_6".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "35".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "63".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_7".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "36".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "64".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_8".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "37".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "65".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_9".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "38".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "66".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_0".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "39".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "67".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_ENTER".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "40".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "40".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_ESC".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "41".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "53".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_BACKSPACE".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "42".into()
                },
                keyboard_group: None,
                output_type: Some("razerKeyGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: Some(
                    LatestKeyboardSourceButtonListitemRazerkeygroup{razer_key_assignment:None}
                ),
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_TAB".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "43".into()
                },
                keyboard_group: None,
                output_type: Some("razerKeyGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: Some(
                    LatestKeyboardSourceButtonListitemRazerkeygroup{
                        razer_key_assignment: Some("ActuationAdjustmentMode".into())
                    }
                ),
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_SPACEBAR".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "44".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "44".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_HYPEN".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "45".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "68".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_EQUAL".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "46".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "69".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_OPEN_SQUARE_BRACKET".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "47".into()
                },
                keyboard_group: None,
                output_type: Some("razerKeyGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: Some(
                    LatestKeyboardSourceButtonListitemRazerkeygroup{
                        razer_key_assignment: Some("KeyboardBacklightDown".into())
                    }
                ),
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_CLOSE_SQUARE_BRACKET".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "48".into()
                },
                keyboard_group: None,
                output_type: Some("razerKeyGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: Some(
                    LatestKeyboardSourceButtonListitemRazerkeygroup{
                        razer_key_assignment: Some("KeyboardBacklightUp".into())
                    }
                ),
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_BACKSLASH".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "49".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "49".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: None,
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_SEMICOLON".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "51".into()
                },
                keyboard_group: None,
                output_type: Some("multimediaGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: Some(
                    LatestKeyboardSourceButtonListitemMultimediagroup{
                        multimedia_assignment: "VolumeUp".into()
                    }
                ),
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_APOSTROPHE".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "52".into()
                },
                keyboard_group: None,
                output_type: Some("multimediaGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: Some(
                    LatestKeyboardSourceButtonListitemMultimediagroup{
                        multimedia_assignment: "MuteVolume".into()
                    }
                ),
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_COMMA".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "54".into()
                },
                keyboard_group: None,
                output_type: Some("multimediaGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: Some(
                    LatestKeyboardSourceButtonListitemMultimediagroup{
                        multimedia_assignment: "PrevTrack".into()
                    }
                ),
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_PERIOD".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "55".into()
                },
                keyboard_group: None,
                output_type: Some("multimediaGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: Some(
                    LatestKeyboardSourceButtonListitemMultimediagroup{
                        multimedia_assignment: "NextTrack".into()
                    }
                ),
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_SLASH".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "56".into()
                },
                keyboard_group: None,
                output_type: Some("multimediaGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: Some(
                    LatestKeyboardSourceButtonListitemMultimediagroup{
                        multimedia_assignment: "Play".into()
                    }
                ),
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_CAPS_LOCK".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "57".into()
                },
                keyboard_group: None,
                output_type: Some("razerKeyGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: Some(
                    LatestKeyboardSourceButtonListitemRazerkeygroup{
                        razer_key_assignment: Some("RapidTriggerAdjustmentMode".into())
                    }
                ),
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_PAGE_UP".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "75".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "74".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec!["Extended_Key".into()],
                        extended_key: Some("Extended_00".into())
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_DELETE".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "76".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "76".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec!["Extended_Key".into()],
                        extended_key: Some("Extended_00".into())
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_PAGE_DOWN".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "78".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "77".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec!["Extended_Key".into()],
                        extended_key: Some("Extended_00".into())
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_RIGHT_ARROW".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "79".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "79".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec!["Extended_Key".into()],
                        extended_key: Some("Extended_00".into())
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_LEFT_ARROW".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "80".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "80".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec!["Extended_Key".into()],
                        extended_key: Some("Extended_00".into())
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_DOWN_ARROW".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "81".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "81".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec!["Extended_Key".into()],
                        extended_key: Some("Extended_00".into())
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_UP_ARROW".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "82".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "82".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec!["Extended_Key".into()],
                        extended_key: Some("Extended_00".into())
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_LEFT_CTRL".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "224".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "224".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_LEFT_SHIFT".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "225".into()
                },
                keyboard_group: None,
                output_type: Some("razerKeyGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: Some(
                    LatestKeyboardSourceButtonListitemRazerkeygroup{
                        razer_key_assignment: Some("SnapTapToggle".into())
                    }
                ),
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_LEFT_ALT".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "226".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "226".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_RIGHT_CTRL".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "228".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "228".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec!["Extended_Key".into()],
                        extended_key: Some("Extended_00".into())
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_RIGHT_SHIFT".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "229".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "229".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec![],
                        extended_key: None
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            },
            LatestKeyboardSourceButtonListitem{
                input_id: "KEY_RIGHT_ALT".into(),
                input_type: "KeyInput".into(),
                key_input: LatestKeyboardSourceButtonListitemKeyinput{
                    page_id: "7".into(),
                    hid: "230".into()
                },
                keyboard_group: Some(
                    LatestKeyboardSourceButtonListitemKeyboardgroup{
                        key: LatestKeyboardSourceButtonListitemKeyboardgroupKey{
                            hid: "230".into(),
                            flag: 0u64,
                            page_id: "7".into()
                        },
                        modifiers: vec!["Extended_Key".into()],
                        extended_key: Some("Extended_00".into())
                    }
                ),
                output_type: Some("keyboardGroup".into()),
                is_hyper_shift: Some(true),
                razer_key_group: None,
                profile_navigation_group: None,
                multimedia_group: None,
                power_key_group: None
            }
        ],
        firmware_keyboard: LatestKeyboardSourceFirmwareKeyboard{
            normal_mode: vec![
                (
                    "KEY_A".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(31u64),
                        input_id: "KEY_A".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "4".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "4".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_B".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(50u64),
                        input_id: "KEY_B".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "5".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "5".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_C".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(48u64),
                        input_id: "KEY_C".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "6".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "6".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_D".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(33u64),
                        input_id: "KEY_D".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "7".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "7".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_E".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(19u64),
                        input_id: "KEY_E".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "8".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "8".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(34u64),
                        input_id: "KEY_F".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "9".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "9".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_G".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(35u64),
                        input_id: "KEY_G".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "10".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "10".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_H".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(36u64),
                        input_id: "KEY_H".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "11".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "11".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_I".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(24u64),
                        input_id: "KEY_I".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "12".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "12".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_J".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(37u64),
                        input_id: "KEY_J".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "13".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "13".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_K".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(38u64),
                        input_id: "KEY_K".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "14".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "14".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_L".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(39u64),
                        input_id: "KEY_L".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "15".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "15".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_M".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(52u64),
                        input_id: "KEY_M".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "16".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "16".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_N".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(51u64),
                        input_id: "KEY_N".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "17".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "17".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_O".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(25u64),
                        input_id: "KEY_O".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "18".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "18".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_P".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(26u64),
                        input_id: "KEY_P".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "19".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "19".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_Q".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(17u64),
                        input_id: "KEY_Q".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "20".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "20".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_R".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(20u64),
                        input_id: "KEY_R".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "21".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "21".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_S".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(32u64),
                        input_id: "KEY_S".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "22".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "22".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_T".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(21u64),
                        input_id: "KEY_T".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "23".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "23".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_U".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(23u64),
                        input_id: "KEY_U".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "24".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "24".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_V".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(49u64),
                        input_id: "KEY_V".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "25".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "25".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_W".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(18u64),
                        input_id: "KEY_W".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "26".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "26".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_X".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(47u64),
                        input_id: "KEY_X".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "27".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "27".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_Y".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(22u64),
                        input_id: "KEY_Y".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "28".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "28".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_Z".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(46u64),
                        input_id: "KEY_Z".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "29".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "29".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_1".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(2u64),
                        input_id: "KEY_1".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "30".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "30".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_2".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(3u64),
                        input_id: "KEY_2".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "31".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "31".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_3".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(4u64),
                        input_id: "KEY_3".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "32".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "32".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_4".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(5u64),
                        input_id: "KEY_4".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "33".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "33".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_5".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(6u64),
                        input_id: "KEY_5".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "34".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "34".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_6".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(7u64),
                        input_id: "KEY_6".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "35".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "35".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_7".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(8u64),
                        input_id: "KEY_7".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "36".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "36".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_8".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(9u64),
                        input_id: "KEY_8".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "37".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "37".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_9".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(10u64),
                        input_id: "KEY_9".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "38".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "38".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_0".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(11u64),
                        input_id: "KEY_0".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "39".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "39".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_ENTER".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(43u64),
                        input_id: "KEY_ENTER".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "40".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "40".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_ESC".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(110u64),
                        input_id: "KEY_ESC".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "41".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "41".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_BACKSPACE".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(15u64),
                        input_id: "KEY_BACKSPACE".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "42".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "42".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_TAB".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(16u64),
                        input_id: "KEY_TAB".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "43".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "43".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_SPACEBAR".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(61u64),
                        input_id: "KEY_SPACEBAR".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "44".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "44".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_HYPEN".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(12u64),
                        input_id: "KEY_HYPEN".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "45".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "45".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_EQUAL".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(13u64),
                        input_id: "KEY_EQUAL".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "46".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "46".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_OPEN_SQUARE_BRACKET".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(27u64),
                        input_id: "KEY_OPEN_SQUARE_BRACKET".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "47".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "47".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_CLOSE_SQUARE_BRACKET".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(28u64),
                        input_id: "KEY_CLOSE_SQUARE_BRACKET".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "48".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "48".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_BACKSLASH".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(29u64),
                        input_id: "KEY_BACKSLASH".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "49".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "49".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NON_US_POUND".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(42u64),
                        input_id: "KEY_NON_US_POUND".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "50".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "50".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_SEMICOLON".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(40u64),
                        input_id: "KEY_SEMICOLON".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "51".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "51".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_APOSTROPHE".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(41u64),
                        input_id: "KEY_APOSTROPHE".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "52".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "52".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_TILDE".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(1u64),
                        input_id: "KEY_TILDE".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "53".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "53".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_COMMA".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(53u64),
                        input_id: "KEY_COMMA".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "54".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "54".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_PERIOD".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(54u64),
                        input_id: "KEY_PERIOD".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "55".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "55".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_SLASH".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(55u64),
                        input_id: "KEY_SLASH".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "56".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "56".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_CAPS_LOCK".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(30u64),
                        input_id: "KEY_CAPS_LOCK".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "57".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "57".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F1".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(112u64),
                        input_id: "KEY_F1".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "58".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "58".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F2".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(113u64),
                        input_id: "KEY_F2".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "59".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "59".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F3".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(114u64),
                        input_id: "KEY_F3".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "60".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "60".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F4".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(115u64),
                        input_id: "KEY_F4".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "61".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "61".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F5".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(116u64),
                        input_id: "KEY_F5".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "62".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "62".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F6".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(117u64),
                        input_id: "KEY_F6".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "63".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "63".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F7".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(118u64),
                        input_id: "KEY_F7".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "64".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "64".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F8".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(119u64),
                        input_id: "KEY_F8".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "65".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "65".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F9".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(120u64),
                        input_id: "KEY_F9".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "66".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "66".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F10".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(121u64),
                        input_id: "KEY_F10".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "67".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "67".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F11".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(122u64),
                        input_id: "KEY_F11".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "68".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "68".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F12".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(123u64),
                        input_id: "KEY_F12".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "69".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "69".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_PRINT_SCREEN".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(124u64),
                        input_id: "KEY_PRINT_SCREEN".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "70".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "70".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_SCROLL_LOCK".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(125u64),
                        input_id: "KEY_SCROLL_LOCK".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "71".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "71".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_PAUSE".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(126u64),
                        input_id: "KEY_PAUSE".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "72".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "72".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_INSERT".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(75u64),
                        input_id: "KEY_INSERT".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "73".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "73".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_HOME".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(80u64),
                        input_id: "KEY_HOME".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "74".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "74".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_PAGE_UP".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(85u64),
                        input_id: "KEY_PAGE_UP".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "75".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "75".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_DELETE".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(76u64),
                        input_id: "KEY_DELETE".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "76".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "76".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_END".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(81u64),
                        input_id: "KEY_END".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "77".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "77".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_PAGE_DOWN".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(86u64),
                        input_id: "KEY_PAGE_DOWN".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "78".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "78".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_RIGHT_ARROW".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(89u64),
                        input_id: "KEY_RIGHT_ARROW".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "79".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "79".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_LEFT_ARROW".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(79u64),
                        input_id: "KEY_LEFT_ARROW".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "80".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "80".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_DOWN_ARROW".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(84u64),
                        input_id: "KEY_DOWN_ARROW".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "81".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "81".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_UP_ARROW".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(83u64),
                        input_id: "KEY_UP_ARROW".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "82".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "82".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_NUM_LOCK".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(90u64),
                        input_id: "KEY_NUMPAD_NUM_LOCK".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "83".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "83".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_SLASH".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(95u64),
                        input_id: "KEY_NUMPAD_SLASH".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "84".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "84".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_ASTERISK".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(100u64),
                        input_id: "KEY_NUMPAD_ASTERISK".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "85".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "85".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_DASH".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(105u64),
                        input_id: "KEY_NUMPAD_DASH".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "86".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "86".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_PLUS".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(106u64),
                        input_id: "KEY_NUMPAD_PLUS".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "87".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "87".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_ENTER".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(108u64),
                        input_id: "KEY_NUMPAD_ENTER".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "88".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "88".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_1".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(93u64),
                        input_id: "KEY_NUMPAD_1".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "89".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "89".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_2".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(98u64),
                        input_id: "KEY_NUMPAD_2".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "90".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "90".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_3".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(103u64),
                        input_id: "KEY_NUMPAD_3".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "91".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "91".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_4".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(92u64),
                        input_id: "KEY_NUMPAD_4".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "92".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "92".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_5".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(97u64),
                        input_id: "KEY_NUMPAD_5".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "93".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "93".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_6".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(102u64),
                        input_id: "KEY_NUMPAD_6".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "94".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "94".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_7".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(91u64),
                        input_id: "KEY_NUMPAD_7".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "95".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "95".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_8".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(96u64),
                        input_id: "KEY_NUMPAD_8".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "96".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "96".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_9".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(101u64),
                        input_id: "KEY_NUMPAD_9".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "97".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "97".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_0".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(99u64),
                        input_id: "KEY_NUMPAD_0".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "98".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "98".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_PERIOD".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(104u64),
                        input_id: "KEY_NUMPAD_PERIOD".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "99".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "99".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NON_US_BACKSLASH".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(45u64),
                        input_id: "KEY_NON_US_BACKSLASH".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "100".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "100".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_APPLICATION".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(129u64),
                        input_id: "KEY_APPLICATION".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "101".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "101".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_EQUAL".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(107u64),
                        input_id: "KEY_NUMPAD_EQUAL".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "103".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "103".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_LEFT_CTRL".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(58u64),
                        input_id: "KEY_LEFT_CTRL".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "224".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "224".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_LEFT_SHIFT".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(44u64),
                        input_id: "KEY_LEFT_SHIFT".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "225".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "225".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_LEFT_ALT".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(60u64),
                        input_id: "KEY_LEFT_ALT".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "226".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "226".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_RIGHT_CTRL".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(64u64),
                        input_id: "KEY_RIGHT_CTRL".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "228".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "228".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_RIGHT_SHIFT".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(57u64),
                        input_id: "KEY_RIGHT_SHIFT".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "229".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "229".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_RIGHT_ALT".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(62u64),
                        input_id: "KEY_RIGHT_ALT".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "230".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "230".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F13".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: None,
                        input_id: "KEY_F13".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "104".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "104".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F14".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: None,
                        input_id: "KEY_F14".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "105".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "105".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F15".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: None,
                        input_id: "KEY_F15".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "106".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "106".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F16".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: None,
                        input_id: "KEY_F16".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "107".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "107".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F17".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: None,
                        input_id: "KEY_F17".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "108".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "108".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F18".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: None,
                        input_id: "KEY_F18".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "109".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "109".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F19".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: None,
                        input_id: "KEY_F19".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "110".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "110".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F20".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: None,
                        input_id: "KEY_F20".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "111".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "111".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F21".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: None,
                        input_id: "KEY_F21".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "112".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "112".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F22".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: None,
                        input_id: "KEY_F22".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "113".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "113".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F23".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: None,
                        input_id: "KEY_F23".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "114".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "114".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F24".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: None,
                        input_id: "KEY_F24".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "115".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "115".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_RO".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(56u64),
                        input_id: "KEY_RO".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "135".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "135".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_KATAKANA_HIRAGANA".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(151u64),
                        input_id: "KEY_KATAKANA_HIRAGANA".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "136".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "136".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_YEN".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(14u64),
                        input_id: "KEY_YEN".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "137".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "137".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_HENKAN".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(154u64),
                        input_id: "KEY_HENKAN".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "138".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "138".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_MUHENKAN".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(150u64),
                        input_id: "KEY_MUHENKAN".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "139".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "139".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_HANGEUL".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(153u64),
                        input_id: "KEY_HANGEUL".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "144".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "144".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_HANJA".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(152u64),
                        input_id: "KEY_HANJA".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "145".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "145".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "DKM_KBMK_01".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(201u64),
                        input_id: "DKM_KBMK_01".into(),
                        input_type: "DKMInput".into(),
                        key_input: None,
                        keyboard_group: None,
                        output_type: "multimediaGroup".into(),
                        dkm_input: Some("DKM_KBMK_01".into()),
                        multimedia_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryMultimediagroup{
                                multimedia_assignment: "PrevTrack".into()
                            }
                        ),
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "DKM_KBMK_02".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(205u64),
                        input_id: "DKM_KBMK_02".into(),
                        input_type: "DKMInput".into(),
                        key_input: None,
                        keyboard_group: None,
                        output_type: "multimediaGroup".into(),
                        dkm_input: Some("DKM_KBMK_02".into()),
                        multimedia_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryMultimediagroup{
                                multimedia_assignment: "Play".into()
                            }
                        ),
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "DKM_KBMK_03".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(202u64),
                        input_id: "DKM_KBMK_03".into(),
                        input_type: "DKMInput".into(),
                        key_input: None,
                        keyboard_group: None,
                        output_type: "multimediaGroup".into(),
                        dkm_input: Some("DKM_KBMK_03".into()),
                        multimedia_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryMultimediagroup{
                                multimedia_assignment: "NextTrack".into()
                            }
                        ),
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "DKM_KBMK_04".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(200u64),
                        input_id: "DKM_KBMK_04".into(),
                        input_type: "DKMInput".into(),
                        key_input: None,
                        keyboard_group: None,
                        output_type: "multimediaGroup".into(),
                        dkm_input: Some("DKM_KBMK_04".into()),
                        multimedia_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryMultimediagroup{
                                multimedia_assignment: "MuteVolume".into()
                            }
                        ),
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "DKM_KB_SIDE1".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(207u64),
                        input_id: "DKM_KB_SIDE1".into(),
                        input_type: "DKMInput".into(),
                        key_input: None,
                        keyboard_group: None,
                        output_type: "win8ShortcutsGroup".into(),
                        dkm_input: Some("DKM_KB_SIDE1".into()),
                        multimedia_group: None,
                        win_8_shortcuts_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryWin8Shortcutsgroup{
                                windows_shortcut_assignment: "Screenshot".into()
                            }
                        )
                    }
                ),
                (
                    "DKM_KB_SIDE2".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(208u64),
                        input_id: "DKM_KB_SIDE2".into(),
                        input_type: "DKMInput".into(),
                        key_input: None,
                        keyboard_group: None,
                        output_type: "win8ShortcutsGroup".into(),
                        dkm_input: Some("DKM_KB_SIDE2".into()),
                        multimedia_group: None,
                        win_8_shortcuts_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryWin8Shortcutsgroup{
                                windows_shortcut_assignment: "XboxGameBar".into()
                            }
                        )
                    }
                ),
                (
                    "DKM_KB_SIDE3".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(209u64),
                        input_id: "DKM_KB_SIDE3".into(),
                        input_type: "DKMInput".into(),
                        key_input: None,
                        keyboard_group: None,
                        output_type: "win8ShortcutsGroup".into(),
                        dkm_input: Some("DKM_KB_SIDE3".into()),
                        multimedia_group: None,
                        win_8_shortcuts_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryWin8Shortcutsgroup{
                                windows_shortcut_assignment: "CycleApps".into()
                            }
                        )
                    }
                ),
                (
                    "DKM_M_01".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(160u64),
                        input_id: "DKM_M_01".into(),
                        input_type: "DKMInput".into(),
                        key_input: None,
                        keyboard_group: None,
                        output_type: "disableGroup".into(),
                        dkm_input: Some("DKM_M_01".into()),
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "DKM_M_02".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(161u64),
                        input_id: "DKM_M_02".into(),
                        input_type: "DKMInput".into(),
                        key_input: None,
                        keyboard_group: None,
                        output_type: "disableGroup".into(),
                        dkm_input: Some("DKM_M_02".into()),
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "DKM_M_03".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(162u64),
                        input_id: "DKM_M_03".into(),
                        input_type: "DKMInput".into(),
                        key_input: None,
                        keyboard_group: None,
                        output_type: "disableGroup".into(),
                        dkm_input: Some("DKM_M_03".into()),
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "DKM_M_04".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(163u64),
                        input_id: "DKM_M_04".into(),
                        input_type: "DKMInput".into(),
                        key_input: None,
                        keyboard_group: None,
                        output_type: "disableGroup".into(),
                        dkm_input: Some("DKM_M_04".into()),
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "DKM_M_05".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(164u64),
                        input_id: "DKM_M_05".into(),
                        input_type: "DKMInput".into(),
                        key_input: None,
                        keyboard_group: None,
                        output_type: "disableGroup".into(),
                        dkm_input: Some("DKM_M_05".into()),
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_LEFT_GUI".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(127u64),
                        input_id: "KEY_LEFT_GUI".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "227".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "227".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_RIGHT_GUI".into(),
                    LatestKeyboardSourceFirmwareKeyboardNormalmodeentry{
                        fw_id: Some(59u64),
                        input_id: "KEY_RIGHT_GUI".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "231".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey{
                                    hid: "231".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        dkm_input: None,
                        multimedia_group: None,
                        win_8_shortcuts_group: None
                    }
                )
            ].into_iter().collect(),
            hypershift_mode: vec![
                (
                    "KEY_A".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(31u64),
                        input_id: "KEY_A".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "4".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "4".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_B".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(50u64),
                        input_id: "KEY_B".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "5".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "5".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_C".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(48u64),
                        input_id: "KEY_C".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "6".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "6".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_D".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(33u64),
                        input_id: "KEY_D".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "7".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "7".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_E".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(19u64),
                        input_id: "KEY_E".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "8".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "8".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(34u64),
                        input_id: "KEY_F".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "9".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "9".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_G".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(35u64),
                        input_id: "KEY_G".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "10".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "10".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_H".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(36u64),
                        input_id: "KEY_H".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "11".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "11".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_I".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(24u64),
                        input_id: "KEY_I".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "12".into()
                            }
                        ),
                        keyboard_group: None,
                        output_type: "razerKeyGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryRazerkeygroup{
                                razer_key_assignment: Some("MacroRecordKey".into())
                            }
                        ),
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_J".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(37u64),
                        input_id: "KEY_J".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "13".into()
                            }
                        ),
                        keyboard_group: None,
                        output_type: "profileNavigationGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryProfilenavigationgroup{
                                profile_navigation_assignment: "Specific".into(),
                                profile_id: Some(2u64)
                            }
                        ),
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_K".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(38u64),
                        input_id: "KEY_K".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "14".into()
                            }
                        ),
                        keyboard_group: None,
                        output_type: "profileNavigationGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryProfilenavigationgroup{
                                profile_navigation_assignment: "CycleUp".into(),
                                profile_id: None
                            }
                        ),
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_L".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(39u64),
                        input_id: "KEY_L".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "15".into()
                            }
                        ),
                        keyboard_group: None,
                        output_type: "multimediaGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryMultimediagroup{
                                multimedia_assignment: "VolumeDown".into()
                            }
                        ),
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_M".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(52u64),
                        input_id: "KEY_M".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "16".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "16".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_N".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(51u64),
                        input_id: "KEY_N".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "17".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "17".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_O".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(25u64),
                        input_id: "KEY_O".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "18".into()
                            }
                        ),
                        keyboard_group: None,
                        output_type: "razerKeyGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryRazerkeygroup{
                                razer_key_assignment: Some("GameModeKey".into())
                            }
                        ),
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_P".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(26u64),
                        input_id: "KEY_P".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "19".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "70".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_Q".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(17u64),
                        input_id: "KEY_Q".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "20".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "20".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_R".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(20u64),
                        input_id: "KEY_R".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "21".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "21".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_S".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(32u64),
                        input_id: "KEY_S".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "22".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "22".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_T".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(21u64),
                        input_id: "KEY_T".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "23".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "23".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_U".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(23u64),
                        input_id: "KEY_U".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "24".into()
                            }
                        ),
                        keyboard_group: None,
                        output_type: "powerKeyGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryPowerkeygroup{
                                power_key_assignment: "SystemSleep".into()
                            }
                        ),
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_V".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(49u64),
                        input_id: "KEY_V".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "25".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "25".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_W".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(18u64),
                        input_id: "KEY_W".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "26".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "26".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_X".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(47u64),
                        input_id: "KEY_X".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "27".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "27".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_Y".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(22u64),
                        input_id: "KEY_Y".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "28".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "73".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_Z".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(46u64),
                        input_id: "KEY_Z".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "29".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "29".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_1".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(2u64),
                        input_id: "KEY_1".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "30".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "58".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_2".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(3u64),
                        input_id: "KEY_2".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "31".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "59".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_3".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(4u64),
                        input_id: "KEY_3".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "32".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "60".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_4".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(5u64),
                        input_id: "KEY_4".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "33".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "61".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_5".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(6u64),
                        input_id: "KEY_5".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "34".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "62".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_6".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(7u64),
                        input_id: "KEY_6".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "35".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "63".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_7".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(8u64),
                        input_id: "KEY_7".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "36".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "64".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_8".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(9u64),
                        input_id: "KEY_8".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "37".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "65".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_9".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(10u64),
                        input_id: "KEY_9".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "38".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "66".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_0".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(11u64),
                        input_id: "KEY_0".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "39".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "67".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_ENTER".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(43u64),
                        input_id: "KEY_ENTER".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "40".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "40".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_ESC".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(110u64),
                        input_id: "KEY_ESC".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "41".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "53".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_BACKSPACE".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(15u64),
                        input_id: "KEY_BACKSPACE".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "42".into()
                            }
                        ),
                        keyboard_group: None,
                        output_type: "razerKeyGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryRazerkeygroup{
                                razer_key_assignment: None
                            }
                        ),
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_TAB".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(16u64),
                        input_id: "KEY_TAB".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "43".into()
                            }
                        ),
                        keyboard_group: None,
                        output_type: "razerKeyGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryRazerkeygroup{
                                razer_key_assignment: Some("ActuationAdjustmentMode".into())
                            }
                        ),
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_SPACEBAR".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(61u64),
                        input_id: "KEY_SPACEBAR".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "44".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "44".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_HYPEN".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(12u64),
                        input_id: "KEY_HYPEN".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "45".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "68".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_EQUAL".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(13u64),
                        input_id: "KEY_EQUAL".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "46".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "69".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_OPEN_SQUARE_BRACKET".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(27u64),
                        input_id: "KEY_OPEN_SQUARE_BRACKET".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "47".into()
                            }
                        ),
                        keyboard_group: None,
                        output_type: "razerKeyGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryRazerkeygroup{
                                razer_key_assignment: Some("KeyboardBacklightDown".into())
                            }
                        ),
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_CLOSE_SQUARE_BRACKET".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(28u64),
                        input_id: "KEY_CLOSE_SQUARE_BRACKET".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "48".into()
                            }
                        ),
                        keyboard_group: None,
                        output_type: "razerKeyGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryRazerkeygroup{
                                razer_key_assignment: Some("KeyboardBacklightUp".into())
                            }
                        ),
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_BACKSLASH".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(29u64),
                        input_id: "KEY_BACKSLASH".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "49".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "49".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NON_US_POUND".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(42u64),
                        input_id: "KEY_NON_US_POUND".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "50".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "50".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_SEMICOLON".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(40u64),
                        input_id: "KEY_SEMICOLON".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "51".into()
                            }
                        ),
                        keyboard_group: None,
                        output_type: "multimediaGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryMultimediagroup{
                                multimedia_assignment: "VolumeUp".into()
                            }
                        ),
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_APOSTROPHE".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(41u64),
                        input_id: "KEY_APOSTROPHE".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "52".into()
                            }
                        ),
                        keyboard_group: None,
                        output_type: "multimediaGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryMultimediagroup{
                                multimedia_assignment: "MuteVolume".into()
                            }
                        ),
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_TILDE".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(1u64),
                        input_id: "KEY_TILDE".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "53".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "53".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_COMMA".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(53u64),
                        input_id: "KEY_COMMA".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "54".into()
                            }
                        ),
                        keyboard_group: None,
                        output_type: "multimediaGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryMultimediagroup{
                                multimedia_assignment: "PrevTrack".into()
                            }
                        ),
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_PERIOD".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(54u64),
                        input_id: "KEY_PERIOD".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "55".into()
                            }
                        ),
                        keyboard_group: None,
                        output_type: "multimediaGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryMultimediagroup{
                                multimedia_assignment: "NextTrack".into()
                            }
                        ),
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_SLASH".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(55u64),
                        input_id: "KEY_SLASH".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "56".into()
                            }
                        ),
                        keyboard_group: None,
                        output_type: "multimediaGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryMultimediagroup{
                                multimedia_assignment: "Play".into()
                            }
                        ),
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_CAPS_LOCK".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(30u64),
                        input_id: "KEY_CAPS_LOCK".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "57".into()
                            }
                        ),
                        keyboard_group: None,
                        output_type: "razerKeyGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryRazerkeygroup{
                                razer_key_assignment: Some("RapidTriggerAdjustmentMode".into())
                            }
                        ),
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F1".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(112u64),
                        input_id: "KEY_F1".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "58".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "58".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F2".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(113u64),
                        input_id: "KEY_F2".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "59".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "59".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F3".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(114u64),
                        input_id: "KEY_F3".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "60".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "60".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F4".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(115u64),
                        input_id: "KEY_F4".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "61".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "61".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F5".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(116u64),
                        input_id: "KEY_F5".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "62".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "62".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F6".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(117u64),
                        input_id: "KEY_F6".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "63".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "63".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F7".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(118u64),
                        input_id: "KEY_F7".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "64".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "64".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F8".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(119u64),
                        input_id: "KEY_F8".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "65".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "65".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F9".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(120u64),
                        input_id: "KEY_F9".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "66".into()
                            }
                        ),
                        keyboard_group: None,
                        output_type: "razerKeyGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryRazerkeygroup{
                                razer_key_assignment: Some("MacroRecordKey".into())
                            }
                        ),
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F10".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(121u64),
                        input_id: "KEY_F10".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "67".into()
                            }
                        ),
                        keyboard_group: None,
                        output_type: "razerKeyGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryRazerkeygroup{
                                razer_key_assignment: Some("GameModeKey".into())
                            }
                        ),
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F11".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(122u64),
                        input_id: "KEY_F11".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "68".into()
                            }
                        ),
                        keyboard_group: None,
                        output_type: "razerKeyGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryRazerkeygroup{
                                razer_key_assignment: Some("KeyboardBacklightDown".into())
                            }
                        ),
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F12".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(123u64),
                        input_id: "KEY_F12".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "69".into()
                            }
                        ),
                        keyboard_group: None,
                        output_type: "razerKeyGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryRazerkeygroup{
                                razer_key_assignment: Some("KeyboardBacklightUp".into())
                            }
                        ),
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_PRINT_SCREEN".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(124u64),
                        input_id: "KEY_PRINT_SCREEN".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "70".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "70".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_SCROLL_LOCK".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(125u64),
                        input_id: "KEY_SCROLL_LOCK".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "71".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "71".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_PAUSE".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(126u64),
                        input_id: "KEY_PAUSE".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "72".into()
                            }
                        ),
                        keyboard_group: None,
                        output_type: "powerKeyGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryPowerkeygroup{
                                power_key_assignment: "SystemSleep".into()
                            }
                        ),
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_INSERT".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(75u64),
                        input_id: "KEY_INSERT".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "73".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "73".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_HOME".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(80u64),
                        input_id: "KEY_HOME".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "74".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "74".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_PAGE_UP".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(85u64),
                        input_id: "KEY_PAGE_UP".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "75".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "74".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_DELETE".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(76u64),
                        input_id: "KEY_DELETE".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "76".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "76".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_END".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(81u64),
                        input_id: "KEY_END".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "77".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "77".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_PAGE_DOWN".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(86u64),
                        input_id: "KEY_PAGE_DOWN".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "78".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "77".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_RIGHT_ARROW".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(89u64),
                        input_id: "KEY_RIGHT_ARROW".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "79".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "79".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_LEFT_ARROW".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(79u64),
                        input_id: "KEY_LEFT_ARROW".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "80".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "80".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_DOWN_ARROW".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(84u64),
                        input_id: "KEY_DOWN_ARROW".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "81".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "81".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_UP_ARROW".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(83u64),
                        input_id: "KEY_UP_ARROW".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "82".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "82".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_NUM_LOCK".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(90u64),
                        input_id: "KEY_NUMPAD_NUM_LOCK".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "83".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "83".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_SLASH".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(95u64),
                        input_id: "KEY_NUMPAD_SLASH".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "84".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "84".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_ASTERISK".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(100u64),
                        input_id: "KEY_NUMPAD_ASTERISK".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "85".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "85".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_DASH".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(105u64),
                        input_id: "KEY_NUMPAD_DASH".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "86".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "86".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_PLUS".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(106u64),
                        input_id: "KEY_NUMPAD_PLUS".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "87".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "87".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_ENTER".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(108u64),
                        input_id: "KEY_NUMPAD_ENTER".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "88".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "88".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_1".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(93u64),
                        input_id: "KEY_NUMPAD_1".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "89".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "89".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_2".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(98u64),
                        input_id: "KEY_NUMPAD_2".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "90".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "90".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_3".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(103u64),
                        input_id: "KEY_NUMPAD_3".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "91".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "91".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_4".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(92u64),
                        input_id: "KEY_NUMPAD_4".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "92".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "92".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_5".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(97u64),
                        input_id: "KEY_NUMPAD_5".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "93".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "93".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_6".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(102u64),
                        input_id: "KEY_NUMPAD_6".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "94".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "94".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_7".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(91u64),
                        input_id: "KEY_NUMPAD_7".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "95".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "95".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_8".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(96u64),
                        input_id: "KEY_NUMPAD_8".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "96".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "96".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_9".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(101u64),
                        input_id: "KEY_NUMPAD_9".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "97".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "97".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_0".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(99u64),
                        input_id: "KEY_NUMPAD_0".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "98".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "98".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_PERIOD".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(104u64),
                        input_id: "KEY_NUMPAD_PERIOD".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "99".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "99".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NON_US_BACKSLASH".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(45u64),
                        input_id: "KEY_NON_US_BACKSLASH".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "100".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "100".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_APPLICATION".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(129u64),
                        input_id: "KEY_APPLICATION".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "101".into()
                            }
                        ),
                        keyboard_group: None,
                        output_type: "profileNavigationGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryProfilenavigationgroup{
                                profile_navigation_assignment: "CycleUp".into(),
                                profile_id: None
                            }
                        ),
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_NUMPAD_EQUAL".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(107u64),
                        input_id: "KEY_NUMPAD_EQUAL".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "103".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "103".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_LEFT_CTRL".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(58u64),
                        input_id: "KEY_LEFT_CTRL".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "224".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "224".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_LEFT_SHIFT".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(44u64),
                        input_id: "KEY_LEFT_SHIFT".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "225".into()
                            }
                        ),
                        keyboard_group: None,
                        output_type: "razerKeyGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryRazerkeygroup{
                                razer_key_assignment: Some("SnapTapToggle".into())
                            }
                        ),
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_LEFT_ALT".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(60u64),
                        input_id: "KEY_LEFT_ALT".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "226".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "226".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_RIGHT_CTRL".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(64u64),
                        input_id: "KEY_RIGHT_CTRL".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "228".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "228".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_RIGHT_SHIFT".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(57u64),
                        input_id: "KEY_RIGHT_SHIFT".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "229".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "229".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_RIGHT_ALT".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(62u64),
                        input_id: "KEY_RIGHT_ALT".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "230".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "230".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F13".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: None,
                        input_id: "KEY_F13".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "104".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "104".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F14".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: None,
                        input_id: "KEY_F14".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "105".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "105".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F15".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: None,
                        input_id: "KEY_F15".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "106".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "106".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F16".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: None,
                        input_id: "KEY_F16".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "107".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "107".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F17".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: None,
                        input_id: "KEY_F17".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "108".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "108".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F18".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: None,
                        input_id: "KEY_F18".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "109".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "109".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F19".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: None,
                        input_id: "KEY_F19".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "110".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "110".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F20".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: None,
                        input_id: "KEY_F20".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "111".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "111".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F21".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: None,
                        input_id: "KEY_F21".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "112".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "112".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F22".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: None,
                        input_id: "KEY_F22".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "113".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "113".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F23".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: None,
                        input_id: "KEY_F23".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "114".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "114".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_F24".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: None,
                        input_id: "KEY_F24".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "115".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "115".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_RO".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(56u64),
                        input_id: "KEY_RO".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "135".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "135".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_KATAKANA_HIRAGANA".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(151u64),
                        input_id: "KEY_KATAKANA_HIRAGANA".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "136".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "136".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_YEN".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(14u64),
                        input_id: "KEY_YEN".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "137".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "137".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_HENKAN".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(154u64),
                        input_id: "KEY_HENKAN".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "138".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "138".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_MUHENKAN".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(150u64),
                        input_id: "KEY_MUHENKAN".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "139".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "139".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_HANGEUL".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(153u64),
                        input_id: "KEY_HANGEUL".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "144".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "144".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_HANJA".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(152u64),
                        input_id: "KEY_HANJA".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "145".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "145".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec![],
                                extended_key: None
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "DKM_KBMK_01".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(201u64),
                        input_id: "DKM_KBMK_01".into(),
                        input_type: "DKMInput".into(),
                        key_input: None,
                        keyboard_group: None,
                        output_type: "multimediaGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryMultimediagroup{
                                multimedia_assignment: "PrevTrack".into()
                            }
                        ),
                        power_key_group: None,
                        dkm_input: Some("DKM_KBMK_01".into()),
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "DKM_KBMK_02".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(205u64),
                        input_id: "DKM_KBMK_02".into(),
                        input_type: "DKMInput".into(),
                        key_input: None,
                        keyboard_group: None,
                        output_type: "multimediaGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryMultimediagroup{
                                multimedia_assignment: "Play".into()
                            }
                        ),
                        power_key_group: None,
                        dkm_input: Some("DKM_KBMK_02".into()),
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "DKM_KBMK_03".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(202u64),
                        input_id: "DKM_KBMK_03".into(),
                        input_type: "DKMInput".into(),
                        key_input: None,
                        keyboard_group: None,
                        output_type: "multimediaGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryMultimediagroup{
                                multimedia_assignment: "NextTrack".into()
                            }
                        ),
                        power_key_group: None,
                        dkm_input: Some("DKM_KBMK_03".into()),
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "DKM_KBMK_04".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(200u64),
                        input_id: "DKM_KBMK_04".into(),
                        input_type: "DKMInput".into(),
                        key_input: None,
                        keyboard_group: None,
                        output_type: "multimediaGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryMultimediagroup{
                                multimedia_assignment: "MuteVolume".into()
                            }
                        ),
                        power_key_group: None,
                        dkm_input: Some("DKM_KBMK_04".into()),
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "DKM_KB_SIDE1".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(207u64),
                        input_id: "DKM_KB_SIDE1".into(),
                        input_type: "DKMInput".into(),
                        key_input: None,
                        keyboard_group: None,
                        output_type: "win8ShortcutsGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: Some("DKM_KB_SIDE1".into()),
                        win_8_shortcuts_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryWin8Shortcutsgroup{
                                windows_shortcut_assignment: "Screenshot".into()
                            }
                        )
                    }
                ),
                (
                    "DKM_KB_SIDE2".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(208u64),
                        input_id: "DKM_KB_SIDE2".into(),
                        input_type: "DKMInput".into(),
                        key_input: None,
                        keyboard_group: None,
                        output_type: "win8ShortcutsGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: Some("DKM_KB_SIDE2".into()),
                        win_8_shortcuts_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryWin8Shortcutsgroup{
                                windows_shortcut_assignment: "XboxGameBar".into()
                            }
                        )
                    }
                ),
                (
                    "DKM_KB_SIDE3".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(209u64),
                        input_id: "DKM_KB_SIDE3".into(),
                        input_type: "DKMInput".into(),
                        key_input: None,
                        keyboard_group: None,
                        output_type: "win8ShortcutsGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: Some("DKM_KB_SIDE3".into()),
                        win_8_shortcuts_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryWin8Shortcutsgroup{
                                windows_shortcut_assignment: "CycleApps".into()
                            }
                        )
                    }
                ),
                (
                    "DKM_M_01".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(160u64),
                        input_id: "DKM_M_01".into(),
                        input_type: "DKMInput".into(),
                        key_input: None,
                        keyboard_group: None,
                        output_type: "disableGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: Some("DKM_M_01".into()),
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "DKM_M_02".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(161u64),
                        input_id: "DKM_M_02".into(),
                        input_type: "DKMInput".into(),
                        key_input: None,
                        keyboard_group: None,
                        output_type: "disableGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: Some("DKM_M_02".into()),
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "DKM_M_03".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(162u64),
                        input_id: "DKM_M_03".into(),
                        input_type: "DKMInput".into(),
                        key_input: None,
                        keyboard_group: None,
                        output_type: "disableGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: Some("DKM_M_03".into()),
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "DKM_M_04".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(163u64),
                        input_id: "DKM_M_04".into(),
                        input_type: "DKMInput".into(),
                        key_input: None,
                        keyboard_group: None,
                        output_type: "disableGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: Some("DKM_M_04".into()),
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "DKM_M_05".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(164u64),
                        input_id: "DKM_M_05".into(),
                        input_type: "DKMInput".into(),
                        key_input: None,
                        keyboard_group: None,
                        output_type: "disableGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: Some("DKM_M_05".into()),
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_LEFT_GUI".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(127u64),
                        input_id: "KEY_LEFT_GUI".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "227".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "227".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                ),
                (
                    "KEY_RIGHT_GUI".into(),
                    LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry{
                        fw_id: Some(59u64),
                        input_id: "KEY_RIGHT_GUI".into(),
                        input_type: "KeyInput".into(),
                        key_input: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput{
                                page_id: "7".into(),
                                hid: "231".into()
                            }
                        ),
                        keyboard_group: Some(
                            LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup{
                                key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey{
                                    hid: "231".into(),
                                    flag: 0u64,
                                    page_id: "7".into()
                                },
                                modifiers: vec!["Extended_Key".into()],
                                extended_key: Some("Extended_00".into())
                            }
                        ),
                        output_type: "keyboardGroup".into(),
                        is_hyper_shift: true,
                        razer_key_group: None,
                        profile_navigation_group: None,
                        multimedia_group: None,
                        power_key_group: None,
                        dkm_input: None,
                        win_8_shortcuts_group: None
                    }
                )
            ].into_iter().collect()
        },
        profile_switch_effect: LatestKeyboardSourceProfileSwitchEffect{
            always_trigger_slot_1_effect: true
        },
        navigation_keys: vec![
            LatestKeyboardSourceNavigationKeysitem{
                name: "A".into(),
                input_id: Some("KEY_A".into()),
                r#type: "Alphanumeric".into(),
                scancode: 30u64,
                virtual_key: "65".into(),
                key_code: "65".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("4".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "B".into(),
                input_id: Some("KEY_B".into()),
                r#type: "Alphanumeric".into(),
                scancode: 48u64,
                virtual_key: "66".into(),
                key_code: "66".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("5".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "C".into(),
                input_id: Some("KEY_C".into()),
                r#type: "Alphanumeric".into(),
                scancode: 46u64,
                virtual_key: "67".into(),
                key_code: "67".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("6".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "D".into(),
                input_id: Some("KEY_D".into()),
                r#type: "Alphanumeric".into(),
                scancode: 32u64,
                virtual_key: "68".into(),
                key_code: "68".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("7".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "E".into(),
                input_id: Some("KEY_E".into()),
                r#type: "Alphanumeric".into(),
                scancode: 18u64,
                virtual_key: "69".into(),
                key_code: "69".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("8".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "F".into(),
                input_id: Some("KEY_F".into()),
                r#type: "Alphanumeric".into(),
                scancode: 33u64,
                virtual_key: "70".into(),
                key_code: "70".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("9".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "G".into(),
                input_id: Some("KEY_G".into()),
                r#type: "Alphanumeric".into(),
                scancode: 34u64,
                virtual_key: "71".into(),
                key_code: "71".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("10".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "H".into(),
                input_id: Some("KEY_H".into()),
                r#type: "Alphanumeric".into(),
                scancode: 35u64,
                virtual_key: "72".into(),
                key_code: "72".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("11".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "I".into(),
                input_id: Some("KEY_I".into()),
                r#type: "Alphanumeric".into(),
                scancode: 23u64,
                virtual_key: "73".into(),
                key_code: "73".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("12".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "J".into(),
                input_id: Some("KEY_J".into()),
                r#type: "Alphanumeric".into(),
                scancode: 36u64,
                virtual_key: "74".into(),
                key_code: "74".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("13".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "K".into(),
                input_id: Some("KEY_K".into()),
                r#type: "Alphanumeric".into(),
                scancode: 37u64,
                virtual_key: "75".into(),
                key_code: "75".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("14".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "L".into(),
                input_id: Some("KEY_L".into()),
                r#type: "Alphanumeric".into(),
                scancode: 38u64,
                virtual_key: "76".into(),
                key_code: "76".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("15".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "M".into(),
                input_id: Some("KEY_M".into()),
                r#type: "Alphanumeric".into(),
                scancode: 50u64,
                virtual_key: "77".into(),
                key_code: "77".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("16".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "N".into(),
                input_id: Some("KEY_N".into()),
                r#type: "Alphanumeric".into(),
                scancode: 49u64,
                virtual_key: "78".into(),
                key_code: "78".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("17".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "O".into(),
                input_id: Some("KEY_O".into()),
                r#type: "Alphanumeric".into(),
                scancode: 24u64,
                virtual_key: "79".into(),
                key_code: "79".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("18".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "P".into(),
                input_id: Some("KEY_P".into()),
                r#type: "Alphanumeric".into(),
                scancode: 25u64,
                virtual_key: "80".into(),
                key_code: "80".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("19".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Q".into(),
                input_id: Some("KEY_Q".into()),
                r#type: "Alphanumeric".into(),
                scancode: 16u64,
                virtual_key: "81".into(),
                key_code: "81".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("20".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "R".into(),
                input_id: Some("KEY_R".into()),
                r#type: "Alphanumeric".into(),
                scancode: 19u64,
                virtual_key: "82".into(),
                key_code: "82".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("21".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "S".into(),
                input_id: Some("KEY_S".into()),
                r#type: "Alphanumeric".into(),
                scancode: 31u64,
                virtual_key: "83".into(),
                key_code: "83".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("22".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "T".into(),
                input_id: Some("KEY_T".into()),
                r#type: "Alphanumeric".into(),
                scancode: 20u64,
                virtual_key: "84".into(),
                key_code: "84".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("23".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "U".into(),
                input_id: Some("KEY_U".into()),
                r#type: "Alphanumeric".into(),
                scancode: 22u64,
                virtual_key: "85".into(),
                key_code: "85".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("24".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "V".into(),
                input_id: Some("KEY_V".into()),
                r#type: "Alphanumeric".into(),
                scancode: 47u64,
                virtual_key: "86".into(),
                key_code: "86".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("25".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "W".into(),
                input_id: Some("KEY_W".into()),
                r#type: "Alphanumeric".into(),
                scancode: 17u64,
                virtual_key: "87".into(),
                key_code: "87".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("26".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "X".into(),
                input_id: Some("KEY_X".into()),
                r#type: "Alphanumeric".into(),
                scancode: 45u64,
                virtual_key: "88".into(),
                key_code: "88".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("27".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Y".into(),
                input_id: Some("KEY_Y".into()),
                r#type: "Alphanumeric".into(),
                scancode: 21u64,
                virtual_key: "89".into(),
                key_code: "89".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("28".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Z".into(),
                input_id: Some("KEY_Z".into()),
                r#type: "Alphanumeric".into(),
                scancode: 44u64,
                virtual_key: "90".into(),
                key_code: "90".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("29".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "0".into(),
                input_id: Some("KEY_0".into()),
                r#type: "Alphanumeric".into(),
                scancode: 11u64,
                virtual_key: "48".into(),
                key_code: "48".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("39".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "1".into(),
                input_id: Some("KEY_1".into()),
                r#type: "Alphanumeric".into(),
                scancode: 2u64,
                virtual_key: "49".into(),
                key_code: "49".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("30".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "2".into(),
                input_id: Some("KEY_2".into()),
                r#type: "Alphanumeric".into(),
                scancode: 3u64,
                virtual_key: "50".into(),
                key_code: "50".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("31".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "3".into(),
                input_id: Some("KEY_3".into()),
                r#type: "Alphanumeric".into(),
                scancode: 4u64,
                virtual_key: "51".into(),
                key_code: "51".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("32".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "4".into(),
                input_id: Some("KEY_4".into()),
                r#type: "Alphanumeric".into(),
                scancode: 5u64,
                virtual_key: "52".into(),
                key_code: "52".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("33".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "5".into(),
                input_id: Some("KEY_5".into()),
                r#type: "Alphanumeric".into(),
                scancode: 6u64,
                virtual_key: "53".into(),
                key_code: "53".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("34".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "6".into(),
                input_id: Some("KEY_6".into()),
                r#type: "Alphanumeric".into(),
                scancode: 7u64,
                virtual_key: "54".into(),
                key_code: "54".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("35".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "7".into(),
                input_id: Some("KEY_7".into()),
                r#type: "Alphanumeric".into(),
                scancode: 8u64,
                virtual_key: "55".into(),
                key_code: "55".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("36".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "8".into(),
                input_id: Some("KEY_8".into()),
                r#type: "Alphanumeric".into(),
                scancode: 9u64,
                virtual_key: "56".into(),
                key_code: "56".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("37".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "9".into(),
                input_id: Some("KEY_9".into()),
                r#type: "Alphanumeric".into(),
                scancode: 10u64,
                virtual_key: "57".into(),
                key_code: "57".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("38".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "F1".into(),
                input_id: Some("KEY_F1".into()),
                r#type: "Function".into(),
                scancode: 59u64,
                virtual_key: "112".into(),
                key_code: "112".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("58".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "F2".into(),
                input_id: Some("KEY_F2".into()),
                r#type: "Function".into(),
                scancode: 60u64,
                virtual_key: "113".into(),
                key_code: "113".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("59".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "F3".into(),
                input_id: Some("KEY_F3".into()),
                r#type: "Function".into(),
                scancode: 61u64,
                virtual_key: "114".into(),
                key_code: "114".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("60".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "F4".into(),
                input_id: Some("KEY_F4".into()),
                r#type: "Function".into(),
                scancode: 62u64,
                virtual_key: "115".into(),
                key_code: "115".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("61".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "F5".into(),
                input_id: Some("KEY_F5".into()),
                r#type: "Function".into(),
                scancode: 63u64,
                virtual_key: "116".into(),
                key_code: "116".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("62".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "F6".into(),
                input_id: Some("KEY_F6".into()),
                r#type: "Function".into(),
                scancode: 64u64,
                virtual_key: "117".into(),
                key_code: "117".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("63".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "F7".into(),
                input_id: Some("KEY_F7".into()),
                r#type: "Function".into(),
                scancode: 65u64,
                virtual_key: "118".into(),
                key_code: "118".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("64".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "F8".into(),
                input_id: Some("KEY_F8".into()),
                r#type: "Function".into(),
                scancode: 66u64,
                virtual_key: "119".into(),
                key_code: "119".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("65".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "F9".into(),
                input_id: Some("KEY_F9".into()),
                r#type: "Function".into(),
                scancode: 67u64,
                virtual_key: "120".into(),
                key_code: "120".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("66".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "F10".into(),
                input_id: Some("KEY_F10".into()),
                r#type: "Function".into(),
                scancode: 68u64,
                virtual_key: "121".into(),
                key_code: "121".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("67".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "F11".into(),
                input_id: Some("KEY_F11".into()),
                r#type: "Function".into(),
                scancode: 87u64,
                virtual_key: "122".into(),
                key_code: "122".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("68".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "F12".into(),
                input_id: Some("KEY_F12".into()),
                r#type: "Function".into(),
                scancode: 88u64,
                virtual_key: "123".into(),
                key_code: "123".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("69".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "F13".into(),
                input_id: Some("KEY_F13".into()),
                r#type: "Function".into(),
                scancode: 100u64,
                virtual_key: "124".into(),
                key_code: "124".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "F14".into(),
                input_id: Some("KEY_F14".into()),
                r#type: "Function".into(),
                scancode: 101u64,
                virtual_key: "125".into(),
                key_code: "125".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "F15".into(),
                input_id: Some("KEY_F15".into()),
                r#type: "Function".into(),
                scancode: 102u64,
                virtual_key: "126".into(),
                key_code: "126".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "F16".into(),
                input_id: Some("KEY_F16".into()),
                r#type: "Function".into(),
                scancode: 103u64,
                virtual_key: "127".into(),
                key_code: "127".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "F17".into(),
                input_id: Some("KEY_F17".into()),
                r#type: "Function".into(),
                scancode: 104u64,
                virtual_key: "128".into(),
                key_code: "128".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "F18".into(),
                input_id: Some("KEY_F18".into()),
                r#type: "Function".into(),
                scancode: 105u64,
                virtual_key: "129".into(),
                key_code: "129".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "F19".into(),
                input_id: Some("KEY_F19".into()),
                r#type: "Function".into(),
                scancode: 106u64,
                virtual_key: "130".into(),
                key_code: "130".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "F20".into(),
                input_id: Some("KEY_F20".into()),
                r#type: "Function".into(),
                scancode: 107u64,
                virtual_key: "131".into(),
                key_code: "131".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "F21".into(),
                input_id: Some("KEY_F21".into()),
                r#type: "Function".into(),
                scancode: 108u64,
                virtual_key: "132".into(),
                key_code: "132".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "F22".into(),
                input_id: Some("KEY_F22".into()),
                r#type: "Function".into(),
                scancode: 109u64,
                virtual_key: "133".into(),
                key_code: "133".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "F23".into(),
                input_id: Some("KEY_F23".into()),
                r#type: "Function".into(),
                scancode: 110u64,
                virtual_key: "134".into(),
                key_code: "134".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "F24".into(),
                input_id: Some("KEY_F24".into()),
                r#type: "Function".into(),
                scancode: 118u64,
                virtual_key: "135".into(),
                key_code: "135".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Num Lock".into(),
                input_id: Some("KEY_NUMPAD_NUM_LOCK".into()),
                r#type: "Numpad".into(),
                scancode: 69u64,
                virtual_key: "144".into(),
                key_code: "144".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("83".into()),
                extended_key: Some("Extended_00".into()),
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Num 0".into(),
                input_id: Some("KEY_NUMPAD_0".into()),
                r#type: "Numpad".into(),
                scancode: 82u64,
                virtual_key: "96".into(),
                key_code: "96".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("98".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Num 1".into(),
                input_id: Some("KEY_NUMPAD_1".into()),
                r#type: "Numpad".into(),
                scancode: 79u64,
                virtual_key: "97".into(),
                key_code: "97".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("89".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Num 2".into(),
                input_id: Some("KEY_NUMPAD_2".into()),
                r#type: "Numpad".into(),
                scancode: 80u64,
                virtual_key: "98".into(),
                key_code: "98".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("90".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Num 3".into(),
                input_id: Some("KEY_NUMPAD_3".into()),
                r#type: "Numpad".into(),
                scancode: 81u64,
                virtual_key: "99".into(),
                key_code: "99".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("91".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Num 4".into(),
                input_id: Some("KEY_NUMPAD_4".into()),
                r#type: "Numpad".into(),
                scancode: 75u64,
                virtual_key: "100".into(),
                key_code: "100".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("92".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Num 5".into(),
                input_id: Some("KEY_NUMPAD_5".into()),
                r#type: "Numpad".into(),
                scancode: 76u64,
                virtual_key: "101".into(),
                key_code: "101".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("93".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Num 6".into(),
                input_id: Some("KEY_NUMPAD_6".into()),
                r#type: "Numpad".into(),
                scancode: 77u64,
                virtual_key: "102".into(),
                key_code: "102".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("94".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Num 7".into(),
                input_id: Some("KEY_NUMPAD_7".into()),
                r#type: "Numpad".into(),
                scancode: 71u64,
                virtual_key: "103".into(),
                key_code: "103".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("95".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Num 8".into(),
                input_id: Some("KEY_NUMPAD_8".into()),
                r#type: "Numpad".into(),
                scancode: 72u64,
                virtual_key: "104".into(),
                key_code: "104".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("96".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Num 9".into(),
                input_id: Some("KEY_NUMPAD_9".into()),
                r#type: "Numpad".into(),
                scancode: 73u64,
                virtual_key: "105".into(),
                key_code: "105".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("97".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Num /".into(),
                input_id: Some("KEY_NUMPAD_SLASH".into()),
                r#type: "Numpad".into(),
                scancode: 53u64,
                virtual_key: "111".into(),
                key_code: "111".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("84".into()),
                extended_key: Some("Extended_00".into()),
                output_flag: Some(2u64),
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Num *".into(),
                input_id: Some("KEY_NUMPAD_ASTERISK".into()),
                r#type: "Numpad".into(),
                scancode: 55u64,
                virtual_key: "106".into(),
                key_code: "106".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("85".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Num -".into(),
                input_id: Some("KEY_NUMPAD_DASH".into()),
                r#type: "Numpad".into(),
                scancode: 74u64,
                virtual_key: "109".into(),
                key_code: "109".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("86".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Num +".into(),
                input_id: Some("KEY_NUMPAD_PLUS".into()),
                r#type: "Numpad".into(),
                scancode: 78u64,
                virtual_key: "107".into(),
                key_code: "107".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("87".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Num .".into(),
                input_id: Some("KEY_NUMPAD_PERIOD".into()),
                r#type: "Numpad".into(),
                scancode: 83u64,
                virtual_key: "46".into(),
                key_code: "110".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("99".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Num Enter".into(),
                input_id: Some("KEY_NUMPAD_ENTER".into()),
                r#type: "Numpad".into(),
                scancode: 28u64,
                virtual_key: "13".into(),
                key_code: "13".into(),
                flag: 1u64,
                page_id: Some("7".into()),
                hid: Some("88".into()),
                extended_key: Some("Extended_00".into()),
                output_flag: Some(2u64),
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Up".into(),
                input_id: Some("KEY_UP_ARROW".into()),
                r#type: "Navigation".into(),
                scancode: 72u64,
                virtual_key: "38".into(),
                key_code: "38".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("82".into()),
                extended_key: Some("Extended_00".into()),
                output_flag: Some(2u64),
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Down".into(),
                input_id: Some("KEY_DOWN_ARROW".into()),
                r#type: "Navigation".into(),
                scancode: 80u64,
                virtual_key: "40".into(),
                key_code: "40".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("81".into()),
                extended_key: Some("Extended_00".into()),
                output_flag: Some(2u64),
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Left".into(),
                input_id: Some("KEY_LEFT_ARROW".into()),
                r#type: "Navigation".into(),
                scancode: 75u64,
                virtual_key: "37".into(),
                key_code: "37".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("80".into()),
                extended_key: Some("Extended_00".into()),
                output_flag: Some(2u64),
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Right".into(),
                input_id: Some("KEY_RIGHT_ARROW".into()),
                r#type: "Navigation".into(),
                scancode: 77u64,
                virtual_key: "39".into(),
                key_code: "39".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("79".into()),
                extended_key: Some("Extended_00".into()),
                output_flag: Some(2u64),
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Page Up".into(),
                input_id: Some("KEY_PAGE_UP".into()),
                r#type: "Navigation".into(),
                scancode: 73u64,
                virtual_key: "33".into(),
                key_code: "33".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("75".into()),
                extended_key: Some("Extended_00".into()),
                output_flag: Some(2u64),
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Page Down".into(),
                input_id: Some("KEY_PAGE_DOWN".into()),
                r#type: "Navigation".into(),
                scancode: 81u64,
                virtual_key: "34".into(),
                key_code: "34".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("78".into()),
                extended_key: Some("Extended_00".into()),
                output_flag: Some(2u64),
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Backspace".into(),
                input_id: Some("KEY_BACKSPACE".into()),
                r#type: "Navigation".into(),
                scancode: 14u64,
                virtual_key: "8".into(),
                key_code: "8".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("42".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Tab".into(),
                input_id: Some("KEY_TAB".into()),
                r#type: "Navigation".into(),
                scancode: 15u64,
                virtual_key: "9".into(),
                key_code: "9".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("43".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Space".into(),
                input_id: Some("KEY_SPACEBAR".into()),
                r#type: "Navigation".into(),
                scancode: 57u64,
                virtual_key: "32".into(),
                key_code: "32".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("44".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Enter".into(),
                input_id: Some("KEY_ENTER".into()),
                r#type: "Navigation".into(),
                scancode: 28u64,
                virtual_key: "13".into(),
                key_code: "13".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("40".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Esc".into(),
                input_id: Some("KEY_ESC".into()),
                r#type: "Navigation".into(),
                scancode: 1u64,
                virtual_key: "27".into(),
                key_code: "27".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("41".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Insert".into(),
                input_id: Some("KEY_INSERT".into()),
                r#type: "Navigation".into(),
                scancode: 82u64,
                virtual_key: "45".into(),
                key_code: "45".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("73".into()),
                extended_key: Some("Extended_00".into()),
                output_flag: Some(2u64),
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Home".into(),
                input_id: Some("KEY_HOME".into()),
                r#type: "Navigation".into(),
                scancode: 71u64,
                virtual_key: "36".into(),
                key_code: "36".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("74".into()),
                extended_key: Some("Extended_00".into()),
                output_flag: Some(2u64),
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Delete".into(),
                input_id: Some("KEY_DELETE".into()),
                r#type: "Navigation".into(),
                scancode: 83u64,
                virtual_key: "46".into(),
                key_code: "46".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("76".into()),
                extended_key: Some("Extended_00".into()),
                output_flag: Some(2u64),
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "End".into(),
                input_id: Some("KEY_END".into()),
                r#type: "Navigation".into(),
                scancode: 79u64,
                virtual_key: "35".into(),
                key_code: "35".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("77".into()),
                extended_key: Some("Extended_00".into()),
                output_flag: Some(2u64),
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Print Screen".into(),
                input_id: Some("KEY_PRINT_SCREEN".into()),
                r#type: "Navigation".into(),
                scancode: 55u64,
                virtual_key: "44".into(),
                key_code: "44".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("70".into()),
                extended_key: Some("Extended_00".into()),
                output_flag: Some(2u64),
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Pause".into(),
                input_id: Some("KEY_PAUSE".into()),
                r#type: "Navigation".into(),
                scancode: 29u64,
                virtual_key: "19".into(),
                key_code: "19".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("72".into()),
                extended_key: None,
                output_flag: Some(4u64),
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Invalid/cancel".into(),
                input_id: Some("KEY_CANCEL".into()),
                r#type: "".into(),
                scancode: 70u64,
                virtual_key: "70".into(),
                key_code: "3".into(),
                flag: 0u64,
                page_id: Some("".into()),
                hid: Some("".into()),
                extended_key: None,
                output_flag: Some(2u64),
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Menu".into(),
                input_id: Some("KEY_APPLICATION".into()),
                r#type: "Navigation".into(),
                scancode: 93u64,
                virtual_key: "93".into(),
                key_code: "93".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("101".into()),
                extended_key: Some("Extended_00".into()),
                output_flag: Some(2u64),
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Ctrl".into(),
                input_id: Some("KEY_LEFT_CTRL".into()),
                r#type: "Modifiers".into(),
                scancode: 29u64,
                virtual_key: "162".into(),
                key_code: "17".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("224".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Right Ctrl".into(),
                input_id: Some("KEY_RIGHT_CTRL".into()),
                r#type: "Modifiers".into(),
                scancode: 29u64,
                virtual_key: "163".into(),
                key_code: "17".into(),
                flag: 2u64,
                page_id: Some("7".into()),
                hid: Some("228".into()),
                extended_key: Some("Extended_00".into()),
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Shift".into(),
                input_id: Some("KEY_LEFT_SHIFT".into()),
                r#type: "Modifiers".into(),
                scancode: 42u64,
                virtual_key: "160".into(),
                key_code: "16".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("225".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Right Shift".into(),
                input_id: Some("KEY_RIGHT_SHIFT".into()),
                r#type: "Modifiers".into(),
                scancode: 54u64,
                virtual_key: "161".into(),
                key_code: "16".into(),
                flag: 2u64,
                page_id: Some("7".into()),
                hid: Some("229".into()),
                extended_key: None,
                output_flag: Some(0u64),
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Alt".into(),
                input_id: Some("KEY_LEFT_ALT".into()),
                r#type: "Modifiers".into(),
                scancode: 56u64,
                virtual_key: "164".into(),
                key_code: "18".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("226".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Right Alt".into(),
                input_id: Some("KEY_RIGHT_ALT".into()),
                r#type: "Modifiers".into(),
                scancode: 56u64,
                virtual_key: "165".into(),
                key_code: "18".into(),
                flag: 2u64,
                page_id: Some("7".into()),
                hid: Some("230".into()),
                extended_key: Some("Extended_00".into()),
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Windows".into(),
                input_id: Some("KEY_LEFT_GUI".into()),
                r#type: "Modifiers".into(),
                scancode: 91u64,
                virtual_key: "91".into(),
                key_code: "91".into(),
                flag: 0u64,
                page_id: None,
                hid: Some("227".into()),
                extended_key: Some("Extended_00".into()),
                output_flag: Some(2u64),
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Caps Lock".into(),
                input_id: Some("KEY_CAPS_LOCK".into()),
                r#type: "Modifiers".into(),
                scancode: 58u64,
                virtual_key: "20".into(),
                key_code: "20".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("57".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Scroll Lock".into(),
                input_id: Some("KEY_SCROLL_LOCK".into()),
                r#type: "Modifiers".into(),
                scancode: 70u64,
                virtual_key: "145".into(),
                key_code: "145".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("71".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "Num Lock".into(),
                input_id: Some("KEY_NUMPAD_NUM_LOCK".into()),
                r#type: "Modifiers".into(),
                scancode: 69u64,
                virtual_key: "144".into(),
                key_code: "144".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("83".into()),
                extended_key: Some("Extended_00".into()),
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "`".into(),
                input_id: Some("KEY_TILDE".into()),
                r#type: "Symbols".into(),
                scancode: 41u64,
                virtual_key: "192".into(),
                key_code: "192".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("53".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "~".into(),
                input_id: None,
                r#type: "Symbols".into(),
                scancode: 41u64,
                virtual_key: "192".into(),
                key_code: "192".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: Some("16".into())
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "!".into(),
                input_id: None,
                r#type: "Symbols".into(),
                scancode: 2u64,
                virtual_key: "49".into(),
                key_code: "49".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: Some("16".into())
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "@".into(),
                input_id: None,
                r#type: "Symbols".into(),
                scancode: 3u64,
                virtual_key: "50".into(),
                key_code: "50".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: Some("16".into())
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "#".into(),
                input_id: Some("KEY_NON_US_POUND".into()),
                r#type: "Symbols".into(),
                scancode: 43u64,
                virtual_key: "51".into(),
                key_code: "51".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("50".into()),
                extended_key: None,
                output_flag: None,
                modifier: Some("16".into())
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "$".into(),
                input_id: None,
                r#type: "Symbols".into(),
                scancode: 5u64,
                virtual_key: "52".into(),
                key_code: "52".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: Some("16".into())
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "%".into(),
                input_id: None,
                r#type: "Symbols".into(),
                scancode: 6u64,
                virtual_key: "53".into(),
                key_code: "53".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: Some("16".into())
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "^".into(),
                input_id: None,
                r#type: "Symbols".into(),
                scancode: 7u64,
                virtual_key: "54".into(),
                key_code: "54".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: Some("16".into())
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "&".into(),
                input_id: None,
                r#type: "Symbols".into(),
                scancode: 8u64,
                virtual_key: "55".into(),
                key_code: "55".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: Some("16".into())
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "*".into(),
                input_id: None,
                r#type: "Symbols".into(),
                scancode: 9u64,
                virtual_key: "56".into(),
                key_code: "56".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: Some("16".into())
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "(".into(),
                input_id: None,
                r#type: "Symbols".into(),
                scancode: 10u64,
                virtual_key: "57".into(),
                key_code: "57".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: Some("16".into())
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: ")".into(),
                input_id: None,
                r#type: "Symbols".into(),
                scancode: 11u64,
                virtual_key: "48".into(),
                key_code: "48".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: Some("16".into())
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "-".into(),
                input_id: Some("KEY_HYPEN".into()),
                r#type: "Symbols".into(),
                scancode: 12u64,
                virtual_key: "189".into(),
                key_code: "189".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("45".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "=".into(),
                input_id: Some("KEY_EQUAL".into()),
                r#type: "Symbols".into(),
                scancode: 13u64,
                virtual_key: "187".into(),
                key_code: "187".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("46".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "_".into(),
                input_id: None,
                r#type: "Symbols".into(),
                scancode: 12u64,
                virtual_key: "189".into(),
                key_code: "189".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: Some("16".into())
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "+".into(),
                input_id: None,
                r#type: "Symbols".into(),
                scancode: 13u64,
                virtual_key: "187".into(),
                key_code: "187".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: Some("16".into())
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "[".into(),
                input_id: Some("KEY_OPEN_SQUARE_BRACKET".into()),
                r#type: "Symbols".into(),
                scancode: 26u64,
                virtual_key: "219".into(),
                key_code: "219".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("47".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "]".into(),
                input_id: Some("KEY_CLOSE_SQUARE_BRACKET".into()),
                r#type: "Symbols".into(),
                scancode: 27u64,
                virtual_key: "221".into(),
                key_code: "221".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("48".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "\\".into(),
                input_id: Some("KEY_BACKSLASH".into()),
                r#type: "Symbols".into(),
                scancode: 43u64,
                virtual_key: "220".into(),
                key_code: "220".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("49".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "{".into(),
                input_id: None,
                r#type: "Symbols".into(),
                scancode: 26u64,
                virtual_key: "219".into(),
                key_code: "219".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: Some("16".into())
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "}".into(),
                input_id: None,
                r#type: "Symbols".into(),
                scancode: 27u64,
                virtual_key: "221".into(),
                key_code: "221".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: Some("16".into())
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "|".into(),
                input_id: None,
                r#type: "Symbols".into(),
                scancode: 43u64,
                virtual_key: "220".into(),
                key_code: "220".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: Some("16".into())
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: ";".into(),
                input_id: Some("KEY_SEMICOLON".into()),
                r#type: "Symbols".into(),
                scancode: 39u64,
                virtual_key: "186".into(),
                key_code: "186".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("51".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "'".into(),
                input_id: Some("KEY_APOSTROPHE".into()),
                r#type: "Symbols".into(),
                scancode: 40u64,
                virtual_key: "222".into(),
                key_code: "222".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("52".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: ":".into(),
                input_id: None,
                r#type: "Symbols".into(),
                scancode: 39u64,
                virtual_key: "186".into(),
                key_code: "186".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: Some("16".into())
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "\"".into(),
                input_id: None,
                r#type: "Symbols".into(),
                scancode: 40u64,
                virtual_key: "222".into(),
                key_code: "222".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: Some("16".into())
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: ",".into(),
                input_id: Some("KEY_COMMA".into()),
                r#type: "Symbols".into(),
                scancode: 51u64,
                virtual_key: "188".into(),
                key_code: "188".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("54".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: ".".into(),
                input_id: Some("KEY_PERIOD".into()),
                r#type: "Symbols".into(),
                scancode: 52u64,
                virtual_key: "190".into(),
                key_code: "190".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("55".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "/".into(),
                input_id: Some("KEY_SLASH".into()),
                r#type: "Symbols".into(),
                scancode: 53u64,
                virtual_key: "191".into(),
                key_code: "191".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("56".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "<".into(),
                input_id: None,
                r#type: "Symbols".into(),
                scancode: 51u64,
                virtual_key: "188".into(),
                key_code: "188".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("100".into()),
                extended_key: None,
                output_flag: None,
                modifier: Some("16".into())
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: ">".into(),
                input_id: None,
                r#type: "Symbols".into(),
                scancode: 52u64,
                virtual_key: "190".into(),
                key_code: "190".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: Some("16".into())
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "?".into(),
                input_id: None,
                r#type: "Symbols".into(),
                scancode: 53u64,
                virtual_key: "191".into(),
                key_code: "191".into(),
                flag: 0u64,
                page_id: None,
                hid: None,
                extended_key: None,
                output_flag: None,
                modifier: Some("16".into())
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "KEY_YEN".into(),
                input_id: Some("KEY_YEN".into()),
                r#type: "".into(),
                scancode: 125u64,
                virtual_key: "".into(),
                key_code: "".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("137".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "KEY_BEFORE_LEFT_SHIFT".into(),
                input_id: Some("KEY_RO".into()),
                r#type: "".into(),
                scancode: 115u64,
                virtual_key: "".into(),
                key_code: "".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("135".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "KEY_BEFORE_SPACE".into(),
                input_id: Some("KEY_MUHENKAN".into()),
                r#type: "".into(),
                scancode: 123u64,
                virtual_key: "235".into(),
                key_code: "".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("139".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "KEY_AFTER_SPACE".into(),
                input_id: Some("KEY_HENKAN".into()),
                r#type: "".into(),
                scancode: 121u64,
                virtual_key: "255".into(),
                key_code: "".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("138".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "KEY_BEFORE_RIGHT_ALT".into(),
                input_id: Some("KEY_KATAKANA_HIRAGANA".into()),
                r#type: "".into(),
                scancode: 112u64,
                virtual_key: "255".into(),
                key_code: "".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("136".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            },
            LatestKeyboardSourceNavigationKeysitem{
                name: "\\".into(),
                input_id: Some("KEY_NON_US_BACKSLASH".into()),
                r#type: "".into(),
                scancode: 86u64,
                virtual_key: "226".into(),
                key_code: "226".into(),
                flag: 0u64,
                page_id: Some("7".into()),
                hid: Some("100".into()),
                extended_key: None,
                output_flag: None,
                modifier: None
            }
        ],
        physical_keys: vec![
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_A".into(),fw_id:31u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_B".into(),fw_id:50u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_C".into(),fw_id:48u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_D".into(),fw_id:33u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_E".into(),fw_id:19u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_F".into(),fw_id:34u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_G".into(),fw_id:35u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_H".into(),fw_id:36u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_I".into(),fw_id:24u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_J".into(),fw_id:37u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_K".into(),fw_id:38u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_L".into(),fw_id:39u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_M".into(),fw_id:52u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_N".into(),fw_id:51u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_O".into(),fw_id:25u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_P".into(),fw_id:26u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_Q".into(),fw_id:17u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_R".into(),fw_id:20u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_S".into(),fw_id:32u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_T".into(),fw_id:21u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_U".into(),fw_id:23u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_V".into(),fw_id:49u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_W".into(),fw_id:18u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_X".into(),fw_id:47u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_Y".into(),fw_id:22u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_Z".into(),fw_id:46u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_1".into(),fw_id:2u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_2".into(),fw_id:3u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_3".into(),fw_id:4u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_4".into(),fw_id:5u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_5".into(),fw_id:6u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_6".into(),fw_id:7u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_7".into(),fw_id:8u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_8".into(),fw_id:9u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_9".into(),fw_id:10u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_0".into(),fw_id:11u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_ENTER".into(),fw_id:43u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_ESC".into(),fw_id:110u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_BACKSPACE".into(),fw_id:15u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_TAB".into(),fw_id:16u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_SPACEBAR".into(),fw_id:61u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_HYPEN".into(),fw_id:12u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_EQUAL".into(),fw_id:13u64},
            LatestKeyboardSourcePhysicalKeysitem{
                input_id: "KEY_OPEN_SQUARE_BRACKET".into(),
                fw_id: 27u64
            },
            LatestKeyboardSourcePhysicalKeysitem{
                input_id: "KEY_CLOSE_SQUARE_BRACKET".into(),
                fw_id: 28u64
            },
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_BACKSLASH".into(),fw_id:29u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_SEMICOLON".into(),fw_id:40u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_APOSTROPHE".into(),fw_id:41u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_COMMA".into(),fw_id:53u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_PERIOD".into(),fw_id:54u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_SLASH".into(),fw_id:55u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_CAPS_LOCK".into(),fw_id:30u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_PAGE_UP".into(),fw_id:85u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_DELETE".into(),fw_id:76u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_PAGE_DOWN".into(),fw_id:86u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_RIGHT_ARROW".into(),fw_id:89u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_LEFT_ARROW".into(),fw_id:79u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_DOWN_ARROW".into(),fw_id:84u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_UP_ARROW".into(),fw_id:83u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_LEFT_CTRL".into(),fw_id:58u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_LEFT_SHIFT".into(),fw_id:44u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_LEFT_ALT".into(),fw_id:60u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_RIGHT_CTRL".into(),fw_id:64u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_RIGHT_SHIFT".into(),fw_id:57u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_RIGHT_ALT".into(),fw_id:62u64},
            LatestKeyboardSourcePhysicalKeysitem{input_id:"KEY_RO".into(),fw_id:56u64}
        ],
        preset_guids: vec![
            ("FACTORY_DEFAULT".into(),"af06d371-861f-4b98-8d78-bfaef5cfdebf".into()),
            ("ANALOG_WASD".into(),"2c481cef-62e0-4fe6-85a1-4d134c279c23".into()),
            ("FPS_RAPID_TRIGGER".into(),"d363ba48-79a8-422c-b405-29a07eff7e8e".into()),
            ("RACING".into(),"474c90e3-7eab-4aa0-8030-34c43cbc707a".into()),
            ("HIGH_SENSITIVITY".into(),"177a20d5-f30a-4a52-923e-4fadb7db8392".into()),
            ("GENERAL".into(),"31718350-ae43-11f0-b558-0800200c9a66".into())
        ].into_iter().collect(),
        preset_profiles: vec![
            LatestKeyboardSourcePresetProfilesitem{
                name: "Factory Default".into(),
                guid: "af06d371-861f-4b98-8d78-bfaef5cfdebf".into(),
                slot_id: 2u64,
                is_factory_preset: Some(true),
                is_default_selected_profile: None
            },
            LatestKeyboardSourcePresetProfilesitem{
                name: "FPS Rapid Trigger".into(),
                guid: "d363ba48-79a8-422c-b405-29a07eff7e8e".into(),
                slot_id: 3u64,
                is_factory_preset: None,
                is_default_selected_profile: None
            },
            LatestKeyboardSourcePresetProfilesitem{
                name: "Analog WASD".into(),
                guid: "2c481cef-62e0-4fe6-85a1-4d134c279c23".into(),
                slot_id: 4u64,
                is_factory_preset: None,
                is_default_selected_profile: None
            },
            LatestKeyboardSourcePresetProfilesitem{
                name: "Racing".into(),
                guid: "474c90e3-7eab-4aa0-8030-34c43cbc707a".into(),
                slot_id: 5u64,
                is_factory_preset: None,
                is_default_selected_profile: None
            },
            LatestKeyboardSourcePresetProfilesitem{
                name: "High Sensitivity".into(),
                guid: "177a20d5-f30a-4a52-923e-4fadb7db8392".into(),
                slot_id: 6u64,
                is_factory_preset: None,
                is_default_selected_profile: None
            },
            LatestKeyboardSourcePresetProfilesitem{
                name: "General".into(),
                guid: "31718350-ae43-11f0-b558-0800200c9a66".into(),
                slot_id: 6u64,
                is_factory_preset: None,
                is_default_selected_profile: Some(true)
            }
        ],
        preset_actuation: vec![
            (
                "af06d371-861f-4b98-8d78-bfaef5cfdebf".into(),
                LatestKeyboardSourcePresetActuationitempayload{
                    specific: LatestKeyboardSourcePresetActuationitempayloadSpecific{
                        lock: None,
                        insert: None,
                        delete: None,
                        up: None,
                        home: None,
                        end: None,
                        down: None
                    },
                    rest: LatestKeyboardSourcePresetActuationitempayloadRest{
                        threshold_l: 32760u64,
                        threshold_h: 31122u64,
                        ui_high: 1.9f64,
                        ui_low: LatestKeyboardSourcePresetActuationitempayloadRestUilowchoice::Integer(
                            2u64
                        )
                    }
                },
            ),
            (
                "d363ba48-79a8-422c-b405-29a07eff7e8e".into(),
                LatestKeyboardSourcePresetActuationitempayload{
                    specific: LatestKeyboardSourcePresetActuationitempayloadSpecific{
                        lock: Some(
                            LatestKeyboardSourcePresetActuationitempayloadSpecificKeyCapsLock{
                                ui_low: 2u64,
                                ui_high: 1.9f64,
                                threshold_l: 32760u64,
                                threshold_h: 31122u64
                            }
                        ),
                        insert: Some(
                            LatestKeyboardSourcePresetActuationitempayloadSpecificKeyInsert{
                                ui_low: 2u64,
                                ui_high: 1.9f64,
                                threshold_l: 32760u64,
                                threshold_h: 31122u64
                            }
                        ),
                        delete: Some(
                            LatestKeyboardSourcePresetActuationitempayloadSpecificKeyDelete{
                                ui_low: 2u64,
                                ui_high: 1.9f64,
                                threshold_l: 32760u64,
                                threshold_h: 31122u64
                            }
                        ),
                        up: Some(
                            LatestKeyboardSourcePresetActuationitempayloadSpecificKeyPageUp{
                                ui_low: 2u64,
                                ui_high: 1.9f64,
                                threshold_l: 32760u64,
                                threshold_h: 31122u64
                            }
                        ),
                        home: Some(
                            LatestKeyboardSourcePresetActuationitempayloadSpecificKeyHome{
                                ui_low: 2u64,
                                ui_high: 1.9f64,
                                threshold_l: 32760u64,
                                threshold_h: 31122u64
                            }
                        ),
                        end: Some(
                            LatestKeyboardSourcePresetActuationitempayloadSpecificKeyEnd{
                                ui_low: 2u64,
                                ui_high: 1.9f64,
                                threshold_l: 32760u64,
                                threshold_h: 31122u64
                            }
                        ),
                        down: Some(
                            LatestKeyboardSourcePresetActuationitempayloadSpecificKeyPageDown{
                                ui_low: 2u64,
                                ui_high: 1.9f64,
                                threshold_l: 32760u64,
                                threshold_h: 31122u64
                            }
                        )
                    },
                    rest: LatestKeyboardSourcePresetActuationitempayloadRest{
                        threshold_l: 19656u64,
                        threshold_h: 18018u64,
                        ui_high: 1.1f64,
                        ui_low: LatestKeyboardSourcePresetActuationitempayloadRestUilowchoice::Decimal(
                            1.2f64
                        )
                    }
                },
            ),
            (
                "2c481cef-62e0-4fe6-85a1-4d134c279c23".into(),
                LatestKeyboardSourcePresetActuationitempayload{
                    specific: LatestKeyboardSourcePresetActuationitempayloadSpecific{
                        lock: Some(
                            LatestKeyboardSourcePresetActuationitempayloadSpecificKeyCapsLock{
                                ui_low: 2u64,
                                ui_high: 1.9f64,
                                threshold_l: 32760u64,
                                threshold_h: 31122u64
                            }
                        ),
                        insert: Some(
                            LatestKeyboardSourcePresetActuationitempayloadSpecificKeyInsert{
                                ui_low: 2u64,
                                ui_high: 1.9f64,
                                threshold_l: 32760u64,
                                threshold_h: 31122u64
                            }
                        ),
                        delete: Some(
                            LatestKeyboardSourcePresetActuationitempayloadSpecificKeyDelete{
                                ui_low: 2u64,
                                ui_high: 1.9f64,
                                threshold_l: 32760u64,
                                threshold_h: 31122u64
                            }
                        ),
                        up: Some(
                            LatestKeyboardSourcePresetActuationitempayloadSpecificKeyPageUp{
                                ui_low: 2u64,
                                ui_high: 1.9f64,
                                threshold_l: 32760u64,
                                threshold_h: 31122u64
                            }
                        ),
                        home: Some(
                            LatestKeyboardSourcePresetActuationitempayloadSpecificKeyHome{
                                ui_low: 2u64,
                                ui_high: 1.9f64,
                                threshold_l: 32760u64,
                                threshold_h: 31122u64
                            }
                        ),
                        end: Some(
                            LatestKeyboardSourcePresetActuationitempayloadSpecificKeyEnd{
                                ui_low: 2u64,
                                ui_high: 1.9f64,
                                threshold_l: 32760u64,
                                threshold_h: 31122u64
                            }
                        ),
                        down: Some(
                            LatestKeyboardSourcePresetActuationitempayloadSpecificKeyPageDown{
                                ui_low: 2u64,
                                ui_high: 1.9f64,
                                threshold_l: 32760u64,
                                threshold_h: 31122u64
                            }
                        )
                    },
                    rest: LatestKeyboardSourcePresetActuationitempayloadRest{
                        threshold_l: 19656u64,
                        threshold_h: 18018u64,
                        ui_high: 1.1f64,
                        ui_low: LatestKeyboardSourcePresetActuationitempayloadRestUilowchoice::Decimal(
                            1.2f64
                        )
                    }
                },
            ),
            (
                "474c90e3-7eab-4aa0-8030-34c43cbc707a".into(),
                LatestKeyboardSourcePresetActuationitempayload{
                    specific: LatestKeyboardSourcePresetActuationitempayloadSpecific{
                        lock: Some(
                            LatestKeyboardSourcePresetActuationitempayloadSpecificKeyCapsLock{
                                ui_low: 2u64,
                                ui_high: 1.9f64,
                                threshold_l: 32760u64,
                                threshold_h: 31122u64
                            }
                        ),
                        insert: Some(
                            LatestKeyboardSourcePresetActuationitempayloadSpecificKeyInsert{
                                ui_low: 2u64,
                                ui_high: 1.9f64,
                                threshold_l: 32760u64,
                                threshold_h: 31122u64
                            }
                        ),
                        delete: Some(
                            LatestKeyboardSourcePresetActuationitempayloadSpecificKeyDelete{
                                ui_low: 2u64,
                                ui_high: 1.9f64,
                                threshold_l: 32760u64,
                                threshold_h: 31122u64
                            }
                        ),
                        up: Some(
                            LatestKeyboardSourcePresetActuationitempayloadSpecificKeyPageUp{
                                ui_low: 2u64,
                                ui_high: 1.9f64,
                                threshold_l: 32760u64,
                                threshold_h: 31122u64
                            }
                        ),
                        home: Some(
                            LatestKeyboardSourcePresetActuationitempayloadSpecificKeyHome{
                                ui_low: 2u64,
                                ui_high: 1.9f64,
                                threshold_l: 32760u64,
                                threshold_h: 31122u64
                            }
                        ),
                        end: Some(
                            LatestKeyboardSourcePresetActuationitempayloadSpecificKeyEnd{
                                ui_low: 2u64,
                                ui_high: 1.9f64,
                                threshold_l: 32760u64,
                                threshold_h: 31122u64
                            }
                        ),
                        down: Some(
                            LatestKeyboardSourcePresetActuationitempayloadSpecificKeyPageDown{
                                ui_low: 2u64,
                                ui_high: 1.9f64,
                                threshold_l: 32760u64,
                                threshold_h: 31122u64
                            }
                        )
                    },
                    rest: LatestKeyboardSourcePresetActuationitempayloadRest{
                        threshold_l: 19656u64,
                        threshold_h: 18018u64,
                        ui_high: 1.1f64,
                        ui_low: LatestKeyboardSourcePresetActuationitempayloadRestUilowchoice::Decimal(
                            1.2f64
                        )
                    }
                },
            ),
            (
                "177a20d5-f30a-4a52-923e-4fadb7db8392".into(),
                LatestKeyboardSourcePresetActuationitempayload{
                    specific: LatestKeyboardSourcePresetActuationitempayloadSpecific{
                        lock: Some(
                            LatestKeyboardSourcePresetActuationitempayloadSpecificKeyCapsLock{
                                ui_low: 2u64,
                                ui_high: 1.9f64,
                                threshold_l: 32760u64,
                                threshold_h: 31122u64
                            }
                        ),
                        insert: Some(
                            LatestKeyboardSourcePresetActuationitempayloadSpecificKeyInsert{
                                ui_low: 2u64,
                                ui_high: 1.9f64,
                                threshold_l: 32760u64,
                                threshold_h: 31122u64
                            }
                        ),
                        delete: Some(
                            LatestKeyboardSourcePresetActuationitempayloadSpecificKeyDelete{
                                ui_low: 2u64,
                                ui_high: 1.9f64,
                                threshold_l: 32760u64,
                                threshold_h: 31122u64
                            }
                        ),
                        up: Some(
                            LatestKeyboardSourcePresetActuationitempayloadSpecificKeyPageUp{
                                ui_low: 2u64,
                                ui_high: 1.9f64,
                                threshold_l: 32760u64,
                                threshold_h: 31122u64
                            }
                        ),
                        home: Some(
                            LatestKeyboardSourcePresetActuationitempayloadSpecificKeyHome{
                                ui_low: 2u64,
                                ui_high: 1.9f64,
                                threshold_l: 32760u64,
                                threshold_h: 31122u64
                            }
                        ),
                        end: Some(
                            LatestKeyboardSourcePresetActuationitempayloadSpecificKeyEnd{
                                ui_low: 2u64,
                                ui_high: 1.9f64,
                                threshold_l: 32760u64,
                                threshold_h: 31122u64
                            }
                        ),
                        down: Some(
                            LatestKeyboardSourcePresetActuationitempayloadSpecificKeyPageDown{
                                ui_low: 2u64,
                                ui_high: 1.9f64,
                                threshold_l: 32760u64,
                                threshold_h: 31122u64
                            }
                        )
                    },
                    rest: LatestKeyboardSourcePresetActuationitempayloadRest{
                        threshold_l: 13104u64,
                        threshold_h: 11466u64,
                        ui_high: 0.7f64,
                        ui_low: LatestKeyboardSourcePresetActuationitempayloadRestUilowchoice::Decimal(
                            0.8f64
                        )
                    }
                },
            ),
            (
                "31718350-ae43-11f0-b558-0800200c9a66".into(),
                LatestKeyboardSourcePresetActuationitempayload{
                    specific: LatestKeyboardSourcePresetActuationitempayloadSpecific{
                        lock: None,
                        insert: None,
                        delete: None,
                        up: None,
                        home: None,
                        end: None,
                        down: None
                    },
                    rest: LatestKeyboardSourcePresetActuationitempayloadRest{
                        threshold_l: 13104u64,
                        threshold_h: 11466u64,
                        ui_high: 0.7f64,
                        ui_low: LatestKeyboardSourcePresetActuationitempayloadRestUilowchoice::Decimal(
                            0.8f64
                        )
                    }
                },
            )
        ],
        preset_rapid_trigger: vec![
            (
                "af06d371-861f-4b98-8d78-bfaef5cfdebf".into(),
                LatestKeyboardSourcePresetRapidTriggeritempayload{
                    specific: LatestKeyboardSourcePresetRapidTriggeritempayloadSpecific{
                        a: None,
                        w: None,
                        s: None,
                        d: None
                    },
                    rest: LatestKeyboardSourcePresetRapidTriggeritempayloadRest{
                        enabled: true,
                        ui_make: 0.4f64,
                        ui_break: 0.4f64,
                        down_sensitivity: 6552u64,
                        up_sensitivity: 6552u64
                    }
                },
            ),
            (
                "d363ba48-79a8-422c-b405-29a07eff7e8e".into(),
                LatestKeyboardSourcePresetRapidTriggeritempayload{
                    specific: LatestKeyboardSourcePresetRapidTriggeritempayloadSpecific{
                        a: Some(
                            LatestKeyboardSourcePresetRapidTriggeritempayloadSpecificKeyA{
                                enabled: true,
                                ui_make: 0.3f64,
                                ui_break: 0.3f64,
                                down_sensitivity: 4914u64,
                                up_sensitivity: 4914u64
                            }
                        ),
                        w: Some(
                            LatestKeyboardSourcePresetRapidTriggeritempayloadSpecificKeyW{
                                enabled: true,
                                ui_make: 0.3f64,
                                ui_break: 0.3f64,
                                down_sensitivity: 4914u64,
                                up_sensitivity: 4914u64
                            }
                        ),
                        s: Some(
                            LatestKeyboardSourcePresetRapidTriggeritempayloadSpecificKeyS{
                                enabled: true,
                                ui_make: 0.3f64,
                                ui_break: 0.3f64,
                                down_sensitivity: 4914u64,
                                up_sensitivity: 4914u64
                            }
                        ),
                        d: Some(
                            LatestKeyboardSourcePresetRapidTriggeritempayloadSpecificKeyD{
                                enabled: true,
                                ui_make: 0.3f64,
                                ui_break: 0.3f64,
                                down_sensitivity: 4914u64,
                                up_sensitivity: 4914u64
                            }
                        )
                    },
                    rest: LatestKeyboardSourcePresetRapidTriggeritempayloadRest{
                        enabled: false,
                        ui_make: 0.3f64,
                        ui_break: 0.3f64,
                        down_sensitivity: 4914u64,
                        up_sensitivity: 4914u64
                    }
                },
            ),
            (
                "2c481cef-62e0-4fe6-85a1-4d134c279c23".into(),
                LatestKeyboardSourcePresetRapidTriggeritempayload{
                    specific: LatestKeyboardSourcePresetRapidTriggeritempayloadSpecific{
                        a: None,
                        w: None,
                        s: None,
                        d: None
                    },
                    rest: LatestKeyboardSourcePresetRapidTriggeritempayloadRest{
                        enabled: false,
                        ui_make: 0.4f64,
                        ui_break: 0.4f64,
                        down_sensitivity: 6552u64,
                        up_sensitivity: 6552u64
                    }
                },
            ),
            (
                "474c90e3-7eab-4aa0-8030-34c43cbc707a".into(),
                LatestKeyboardSourcePresetRapidTriggeritempayload{
                    specific: LatestKeyboardSourcePresetRapidTriggeritempayloadSpecific{
                        a: None,
                        w: None,
                        s: None,
                        d: None
                    },
                    rest: LatestKeyboardSourcePresetRapidTriggeritempayloadRest{
                        enabled: false,
                        ui_make: 0.4f64,
                        ui_break: 0.4f64,
                        down_sensitivity: 6552u64,
                        up_sensitivity: 6552u64
                    }
                },
            ),
            (
                "177a20d5-f30a-4a52-923e-4fadb7db8392".into(),
                LatestKeyboardSourcePresetRapidTriggeritempayload{
                    specific: LatestKeyboardSourcePresetRapidTriggeritempayloadSpecific{
                        a: Some(
                            LatestKeyboardSourcePresetRapidTriggeritempayloadSpecificKeyA{
                                enabled: true,
                                ui_make: 0.3f64,
                                ui_break: 0.3f64,
                                down_sensitivity: 4914u64,
                                up_sensitivity: 4914u64
                            }
                        ),
                        w: Some(
                            LatestKeyboardSourcePresetRapidTriggeritempayloadSpecificKeyW{
                                enabled: true,
                                ui_make: 0.3f64,
                                ui_break: 0.3f64,
                                down_sensitivity: 4914u64,
                                up_sensitivity: 4914u64
                            }
                        ),
                        s: Some(
                            LatestKeyboardSourcePresetRapidTriggeritempayloadSpecificKeyS{
                                enabled: true,
                                ui_make: 0.3f64,
                                ui_break: 0.3f64,
                                down_sensitivity: 4914u64,
                                up_sensitivity: 4914u64
                            }
                        ),
                        d: Some(
                            LatestKeyboardSourcePresetRapidTriggeritempayloadSpecificKeyD{
                                enabled: true,
                                ui_make: 0.3f64,
                                ui_break: 0.3f64,
                                down_sensitivity: 4914u64,
                                up_sensitivity: 4914u64
                            }
                        )
                    },
                    rest: LatestKeyboardSourcePresetRapidTriggeritempayloadRest{
                        enabled: false,
                        ui_make: 0.3f64,
                        ui_break: 0.3f64,
                        down_sensitivity: 4914u64,
                        up_sensitivity: 4914u64
                    }
                },
            ),
            (
                "31718350-ae43-11f0-b558-0800200c9a66".into(),
                LatestKeyboardSourcePresetRapidTriggeritempayload{
                    specific: LatestKeyboardSourcePresetRapidTriggeritempayloadSpecific{
                        a: None,
                        w: None,
                        s: None,
                        d: None
                    },
                    rest: LatestKeyboardSourcePresetRapidTriggeritempayloadRest{
                        enabled: false,
                        ui_make: 0.4f64,
                        ui_break: 0.4f64,
                        down_sensitivity: 6552u64,
                        up_sensitivity: 6552u64
                    }
                },
            )
        ],
        preset_mappings: vec![
            ("af06d371-861f-4b98-8d78-bfaef5cfdebf".into(),vec![],),
            (
                "2c481cef-62e0-4fe6-85a1-4d134c279c23".into(),
                vec![
                    LatestKeyboardSourcePresetMappingsitempayloaditem{
                        input_type: "AnalogInput".into(),
                        is_hyper_shift: false,
                        input_id: "KEY_W".into(),
                        mapping: vec![
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "controllerGroup".into(),
                                controller_group: Some(
                                    LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroup{
                                        controller_assignment: "LEFT_JS_UP".into(),
                                        controller_mode: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupControllermode{
                                            is_analog: true
                                        },
                                        analog_sensitivity_type: "STANDARD".into(),
                                        analog_sensitivity_list: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylist{
                                            analog_sensitivity_assignment: vec![
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Decimal(
                                                        0.1f64
                                                    ),
                                                    y: 0u64
                                                },
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Decimal(
                                                        1.4f64
                                                    ),
                                                    y: 86u64
                                                },
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Decimal(
                                                        2.8f64
                                                    ),
                                                    y: 176u64
                                                },
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Integer(
                                                        4u64
                                                    ),
                                                    y: 255u64
                                                }
                                            ]
                                        }
                                    }
                                ),
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 19656u64,
                                    field_1: 256u64
                                },
                                keyboard_group: None,
                                rapid_trigger: None
                            },
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "defaultGroup".into(),
                                controller_group: None,
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 19656u64,
                                    field_1: 256u64
                                },
                                keyboard_group: None,
                                rapid_trigger: None
                            }
                        ],
                        output_type: "keyboardGroup".into()
                    },
                    LatestKeyboardSourcePresetMappingsitempayloaditem{
                        input_type: "AnalogInput".into(),
                        is_hyper_shift: false,
                        input_id: "KEY_S".into(),
                        mapping: vec![
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "controllerGroup".into(),
                                controller_group: Some(
                                    LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroup{
                                        controller_assignment: "LEFT_JS_DOWN".into(),
                                        controller_mode: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupControllermode{
                                            is_analog: true
                                        },
                                        analog_sensitivity_type: "STANDARD".into(),
                                        analog_sensitivity_list: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylist{
                                            analog_sensitivity_assignment: vec![
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Decimal(
                                                        0.1f64
                                                    ),
                                                    y: 0u64
                                                },
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Decimal(
                                                        1.4f64
                                                    ),
                                                    y: 86u64
                                                },
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Decimal(
                                                        2.8f64
                                                    ),
                                                    y: 176u64
                                                },
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Integer(
                                                        4u64
                                                    ),
                                                    y: 255u64
                                                }
                                            ]
                                        }
                                    }
                                ),
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 19656u64,
                                    field_1: 256u64
                                },
                                keyboard_group: None,
                                rapid_trigger: None
                            },
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "defaultGroup".into(),
                                controller_group: None,
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 19656u64,
                                    field_1: 256u64
                                },
                                keyboard_group: None,
                                rapid_trigger: None
                            }
                        ],
                        output_type: "keyboardGroup".into()
                    },
                    LatestKeyboardSourcePresetMappingsitempayloaditem{
                        input_type: "AnalogInput".into(),
                        is_hyper_shift: false,
                        input_id: "KEY_A".into(),
                        mapping: vec![
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "controllerGroup".into(),
                                controller_group: Some(
                                    LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroup{
                                        controller_assignment: "LEFT_JS_LEFT".into(),
                                        controller_mode: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupControllermode{
                                            is_analog: true
                                        },
                                        analog_sensitivity_type: "STANDARD".into(),
                                        analog_sensitivity_list: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylist{
                                            analog_sensitivity_assignment: vec![
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Decimal(
                                                        0.1f64
                                                    ),
                                                    y: 0u64
                                                },
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Decimal(
                                                        1.4f64
                                                    ),
                                                    y: 86u64
                                                },
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Decimal(
                                                        2.8f64
                                                    ),
                                                    y: 176u64
                                                },
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Integer(
                                                        4u64
                                                    ),
                                                    y: 255u64
                                                }
                                            ]
                                        }
                                    }
                                ),
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 19656u64,
                                    field_1: 256u64
                                },
                                keyboard_group: None,
                                rapid_trigger: None
                            },
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "defaultGroup".into(),
                                controller_group: None,
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 19656u64,
                                    field_1: 256u64
                                },
                                keyboard_group: None,
                                rapid_trigger: None
                            }
                        ],
                        output_type: "keyboardGroup".into()
                    },
                    LatestKeyboardSourcePresetMappingsitempayloaditem{
                        input_type: "AnalogInput".into(),
                        is_hyper_shift: false,
                        input_id: "KEY_D".into(),
                        mapping: vec![
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "controllerGroup".into(),
                                controller_group: Some(
                                    LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroup{
                                        controller_assignment: "LEFT_JS_RIGHT".into(),
                                        controller_mode: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupControllermode{
                                            is_analog: true
                                        },
                                        analog_sensitivity_type: "STANDARD".into(),
                                        analog_sensitivity_list: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylist{
                                            analog_sensitivity_assignment: vec![
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Decimal(
                                                        0.1f64
                                                    ),
                                                    y: 0u64
                                                },
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Decimal(
                                                        1.4f64
                                                    ),
                                                    y: 86u64
                                                },
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Decimal(
                                                        2.8f64
                                                    ),
                                                    y: 176u64
                                                },
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Integer(
                                                        4u64
                                                    ),
                                                    y: 255u64
                                                }
                                            ]
                                        }
                                    }
                                ),
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 19656u64,
                                    field_1: 256u64
                                },
                                keyboard_group: None,
                                rapid_trigger: None
                            },
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "defaultGroup".into(),
                                controller_group: None,
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 19656u64,
                                    field_1: 256u64
                                },
                                keyboard_group: None,
                                rapid_trigger: None
                            }
                        ],
                        output_type: "keyboardGroup".into()
                    }
                ],
            ),
            (
                "474c90e3-7eab-4aa0-8030-34c43cbc707a".into(),
                vec![
                    LatestKeyboardSourcePresetMappingsitempayloaditem{
                        input_type: "AnalogInput".into(),
                        is_hyper_shift: false,
                        input_id: "KEY_W".into(),
                        mapping: vec![
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "controllerGroup".into(),
                                controller_group: Some(
                                    LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroup{
                                        controller_assignment: "RIGHTTRIGGER".into(),
                                        controller_mode: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupControllermode{
                                            is_analog: true
                                        },
                                        analog_sensitivity_type: "STANDARD".into(),
                                        analog_sensitivity_list: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylist{
                                            analog_sensitivity_assignment: vec![
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Decimal(
                                                        0.1f64
                                                    ),
                                                    y: 0u64
                                                },
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Decimal(
                                                        1.4f64
                                                    ),
                                                    y: 86u64
                                                },
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Decimal(
                                                        2.8f64
                                                    ),
                                                    y: 176u64
                                                },
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Integer(
                                                        4u64
                                                    ),
                                                    y: 255u64
                                                }
                                            ]
                                        }
                                    }
                                ),
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 19656u64,
                                    field_1: 256u64
                                },
                                keyboard_group: None,
                                rapid_trigger: None
                            },
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "defaultGroup".into(),
                                controller_group: None,
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 19656u64,
                                    field_1: 256u64
                                },
                                keyboard_group: None,
                                rapid_trigger: None
                            }
                        ],
                        output_type: "keyboardGroup".into()
                    },
                    LatestKeyboardSourcePresetMappingsitempayloaditem{
                        input_type: "AnalogInput".into(),
                        is_hyper_shift: false,
                        input_id: "KEY_A".into(),
                        mapping: vec![
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "controllerGroup".into(),
                                controller_group: Some(
                                    LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroup{
                                        controller_assignment: "LEFT_JS_LEFT".into(),
                                        controller_mode: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupControllermode{
                                            is_analog: true
                                        },
                                        analog_sensitivity_type: "STANDARD".into(),
                                        analog_sensitivity_list: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylist{
                                            analog_sensitivity_assignment: vec![
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Decimal(
                                                        0.1f64
                                                    ),
                                                    y: 0u64
                                                },
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Decimal(
                                                        1.4f64
                                                    ),
                                                    y: 86u64
                                                },
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Decimal(
                                                        2.8f64
                                                    ),
                                                    y: 176u64
                                                },
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Integer(
                                                        4u64
                                                    ),
                                                    y: 255u64
                                                }
                                            ]
                                        }
                                    }
                                ),
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 19656u64,
                                    field_1: 256u64
                                },
                                keyboard_group: None,
                                rapid_trigger: None
                            },
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "defaultGroup".into(),
                                controller_group: None,
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 19656u64,
                                    field_1: 256u64
                                },
                                keyboard_group: None,
                                rapid_trigger: None
                            }
                        ],
                        output_type: "keyboardGroup".into()
                    },
                    LatestKeyboardSourcePresetMappingsitempayloaditem{
                        input_type: "AnalogInput".into(),
                        is_hyper_shift: false,
                        input_id: "KEY_S".into(),
                        mapping: vec![
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "controllerGroup".into(),
                                controller_group: Some(
                                    LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroup{
                                        controller_assignment: "LEFTTRIGGER".into(),
                                        controller_mode: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupControllermode{
                                            is_analog: true
                                        },
                                        analog_sensitivity_type: "STANDARD".into(),
                                        analog_sensitivity_list: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylist{
                                            analog_sensitivity_assignment: vec![
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Decimal(
                                                        0.1f64
                                                    ),
                                                    y: 0u64
                                                },
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Decimal(
                                                        1.4f64
                                                    ),
                                                    y: 86u64
                                                },
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Decimal(
                                                        2.8f64
                                                    ),
                                                    y: 176u64
                                                },
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Integer(
                                                        4u64
                                                    ),
                                                    y: 255u64
                                                }
                                            ]
                                        }
                                    }
                                ),
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 19656u64,
                                    field_1: 256u64
                                },
                                keyboard_group: None,
                                rapid_trigger: None
                            },
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "defaultGroup".into(),
                                controller_group: None,
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 19656u64,
                                    field_1: 256u64
                                },
                                keyboard_group: None,
                                rapid_trigger: None
                            }
                        ],
                        output_type: "keyboardGroup".into()
                    },
                    LatestKeyboardSourcePresetMappingsitempayloaditem{
                        input_type: "AnalogInput".into(),
                        is_hyper_shift: false,
                        input_id: "KEY_D".into(),
                        mapping: vec![
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "controllerGroup".into(),
                                controller_group: Some(
                                    LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroup{
                                        controller_assignment: "LEFT_JS_RIGHT".into(),
                                        controller_mode: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupControllermode{
                                            is_analog: true
                                        },
                                        analog_sensitivity_type: "STANDARD".into(),
                                        analog_sensitivity_list: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylist{
                                            analog_sensitivity_assignment: vec![
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Decimal(
                                                        0.1f64
                                                    ),
                                                    y: 0u64
                                                },
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Decimal(
                                                        1.4f64
                                                    ),
                                                    y: 86u64
                                                },
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Decimal(
                                                        2.8f64
                                                    ),
                                                    y: 176u64
                                                },
                                                LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem{
                                                    x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice::Integer(
                                                        4u64
                                                    ),
                                                    y: 255u64
                                                }
                                            ]
                                        }
                                    }
                                ),
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 19656u64,
                                    field_1: 256u64
                                },
                                keyboard_group: None,
                                rapid_trigger: None
                            },
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "defaultGroup".into(),
                                controller_group: None,
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 19656u64,
                                    field_1: 256u64
                                },
                                keyboard_group: None,
                                rapid_trigger: None
                            }
                        ],
                        output_type: "keyboardGroup".into()
                    }
                ],
            ),
            (
                "177a20d5-f30a-4a52-923e-4fadb7db8392".into(),
                vec![
                    LatestKeyboardSourcePresetMappingsitempayloaditem{
                        input_type: "AnalogInput".into(),
                        is_hyper_shift: false,
                        input_id: "KEY_A".into(),
                        mapping: vec![
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "keyboardGroup".into(),
                                controller_group: None,
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 13104u64,
                                    field_1: 256u64
                                },
                                keyboard_group: Some(vec![].into_iter().collect()),
                                rapid_trigger: Some(
                                    LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemRapidtrigger{
                                        make_sensitivity: 3u64,
                                        break_sensitivity: 3u64,
                                        is_enable_up_stroke: false,
                                        continuous: false
                                    }
                                )
                            },
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "defaultGroup".into(),
                                controller_group: None,
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 13104u64,
                                    field_1: 256u64
                                },
                                keyboard_group: None,
                                rapid_trigger: None
                            }
                        ],
                        output_type: "keyboardGroup".into()
                    },
                    LatestKeyboardSourcePresetMappingsitempayloaditem{
                        input_type: "AnalogInput".into(),
                        is_hyper_shift: false,
                        input_id: "KEY_W".into(),
                        mapping: vec![
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "keyboardGroup".into(),
                                controller_group: None,
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 13104u64,
                                    field_1: 256u64
                                },
                                keyboard_group: Some(vec![].into_iter().collect()),
                                rapid_trigger: Some(
                                    LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemRapidtrigger{
                                        make_sensitivity: 3u64,
                                        break_sensitivity: 3u64,
                                        is_enable_up_stroke: false,
                                        continuous: false
                                    }
                                )
                            },
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "defaultGroup".into(),
                                controller_group: None,
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 13104u64,
                                    field_1: 256u64
                                },
                                keyboard_group: None,
                                rapid_trigger: None
                            }
                        ],
                        output_type: "keyboardGroup".into()
                    },
                    LatestKeyboardSourcePresetMappingsitempayloaditem{
                        input_type: "AnalogInput".into(),
                        is_hyper_shift: false,
                        input_id: "KEY_S".into(),
                        mapping: vec![
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "keyboardGroup".into(),
                                controller_group: None,
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 13104u64,
                                    field_1: 256u64
                                },
                                keyboard_group: Some(vec![].into_iter().collect()),
                                rapid_trigger: Some(
                                    LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemRapidtrigger{
                                        make_sensitivity: 3u64,
                                        break_sensitivity: 3u64,
                                        is_enable_up_stroke: false,
                                        continuous: false
                                    }
                                )
                            },
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "defaultGroup".into(),
                                controller_group: None,
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 13104u64,
                                    field_1: 256u64
                                },
                                keyboard_group: None,
                                rapid_trigger: None
                            }
                        ],
                        output_type: "keyboardGroup".into()
                    },
                    LatestKeyboardSourcePresetMappingsitempayloaditem{
                        input_type: "AnalogInput".into(),
                        is_hyper_shift: false,
                        input_id: "KEY_D".into(),
                        mapping: vec![
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "keyboardGroup".into(),
                                controller_group: None,
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 13104u64,
                                    field_1: 256u64
                                },
                                keyboard_group: Some(vec![].into_iter().collect()),
                                rapid_trigger: Some(
                                    LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemRapidtrigger{
                                        make_sensitivity: 3u64,
                                        break_sensitivity: 3u64,
                                        is_enable_up_stroke: false,
                                        continuous: false
                                    }
                                )
                            },
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "defaultGroup".into(),
                                controller_group: None,
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 13104u64,
                                    field_1: 256u64
                                },
                                keyboard_group: None,
                                rapid_trigger: None
                            }
                        ],
                        output_type: "keyboardGroup".into()
                    }
                ],
            ),
            (
                "d363ba48-79a8-422c-b405-29a07eff7e8e".into(),
                vec![
                    LatestKeyboardSourcePresetMappingsitempayloaditem{
                        input_type: "AnalogInput".into(),
                        is_hyper_shift: false,
                        input_id: "KEY_W".into(),
                        mapping: vec![
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "keyboardGroup".into(),
                                controller_group: None,
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 19656u64,
                                    field_1: 18018u64
                                },
                                keyboard_group: Some(vec![].into_iter().collect()),
                                rapid_trigger: Some(
                                    LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemRapidtrigger{
                                        make_sensitivity: 3u64,
                                        break_sensitivity: 3u64,
                                        is_enable_up_stroke: false,
                                        continuous: false
                                    }
                                )
                            },
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "defaultGroup".into(),
                                controller_group: None,
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 19656u64,
                                    field_1: 18018u64
                                },
                                keyboard_group: None,
                                rapid_trigger: None
                            }
                        ],
                        output_type: "keyboardGroup".into()
                    },
                    LatestKeyboardSourcePresetMappingsitempayloaditem{
                        input_type: "AnalogInput".into(),
                        is_hyper_shift: false,
                        input_id: "KEY_A".into(),
                        mapping: vec![
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "keyboardGroup".into(),
                                controller_group: None,
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 19656u64,
                                    field_1: 18018u64
                                },
                                keyboard_group: Some(vec![].into_iter().collect()),
                                rapid_trigger: Some(
                                    LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemRapidtrigger{
                                        make_sensitivity: 3u64,
                                        break_sensitivity: 3u64,
                                        is_enable_up_stroke: false,
                                        continuous: false
                                    }
                                )
                            },
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "defaultGroup".into(),
                                controller_group: None,
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 19656u64,
                                    field_1: 18018u64
                                },
                                keyboard_group: None,
                                rapid_trigger: None
                            }
                        ],
                        output_type: "keyboardGroup".into()
                    },
                    LatestKeyboardSourcePresetMappingsitempayloaditem{
                        input_type: "AnalogInput".into(),
                        is_hyper_shift: false,
                        input_id: "KEY_S".into(),
                        mapping: vec![
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "keyboardGroup".into(),
                                controller_group: None,
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 19656u64,
                                    field_1: 18018u64
                                },
                                keyboard_group: Some(vec![].into_iter().collect()),
                                rapid_trigger: Some(
                                    LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemRapidtrigger{
                                        make_sensitivity: 3u64,
                                        break_sensitivity: 3u64,
                                        is_enable_up_stroke: false,
                                        continuous: false
                                    }
                                )
                            },
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "defaultGroup".into(),
                                controller_group: None,
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 19656u64,
                                    field_1: 18018u64
                                },
                                keyboard_group: None,
                                rapid_trigger: None
                            }
                        ],
                        output_type: "keyboardGroup".into()
                    },
                    LatestKeyboardSourcePresetMappingsitempayloaditem{
                        input_type: "AnalogInput".into(),
                        is_hyper_shift: false,
                        input_id: "KEY_D".into(),
                        mapping: vec![
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "keyboardGroup".into(),
                                controller_group: None,
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 19656u64,
                                    field_1: 18018u64
                                },
                                keyboard_group: Some(vec![].into_iter().collect()),
                                rapid_trigger: Some(
                                    LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemRapidtrigger{
                                        make_sensitivity: 3u64,
                                        break_sensitivity: 3u64,
                                        is_enable_up_stroke: false,
                                        continuous: false
                                    }
                                )
                            },
                            LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem{
                                output_type: "defaultGroup".into(),
                                controller_group: None,
                                actuation_point: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint{
                                    field_0: 19656u64,
                                    field_1: 18018u64
                                },
                                keyboard_group: None,
                                rapid_trigger: None
                            }
                        ],
                        output_type: "keyboardGroup".into()
                    }
                ],
            ),
            ("31718350-ae43-11f0-b558-0800200c9a66".into(),vec![],)
        ],
        onboard_presets: vec![
            LatestKeyboardSourceOnboardPresetsitem{
                name: "Factory Default".into(),
                guid: "af06d371-861f-4b98-8d78-bfaef5cfdebf".into(),
                slot_id: 2u64,
                is_factory_preset: Some(true)
            },
            LatestKeyboardSourceOnboardPresetsitem{
                name: "FPS Rapid Trigger".into(),
                guid: "d363ba48-79a8-422c-b405-29a07eff7e8e".into(),
                slot_id: 3u64,
                is_factory_preset: None
            },
            LatestKeyboardSourceOnboardPresetsitem{
                name: "Analog WASD".into(),
                guid: "2c481cef-62e0-4fe6-85a1-4d134c279c23".into(),
                slot_id: 4u64,
                is_factory_preset: None
            },
            LatestKeyboardSourceOnboardPresetsitem{
                name: "Racing".into(),
                guid: "474c90e3-7eab-4aa0-8030-34c43cbc707a".into(),
                slot_id: 5u64,
                is_factory_preset: None
            },
            LatestKeyboardSourceOnboardPresetsitem{
                name: "High Sensitivity".into(),
                guid: "177a20d5-f30a-4a52-923e-4fadb7db8392".into(),
                slot_id: 6u64,
                is_factory_preset: None
            }
        ],
        profile_transfer_target: LatestKeyboardSourceProfileTransferTarget{
            product_id: 756u64,
            dongle_id: 757u64,
            ble_id: 758u64,
            device_name: "Razer Reclusa Mini 65%".into(),
            category: "KEYBOARD".into(),
            is_8_k_analog_device: true,
            preset_profiles: vec![
                LatestKeyboardSourceProfileTransferTargetPresetprofilesitem{
                    name: "Factory Default".into(),
                    guid: "af06d371-861f-4b98-8d78-bfaef5cfdebf".into(),
                    slot_id: 2u64,
                    is_factory_preset: Some(true)
                },
                LatestKeyboardSourceProfileTransferTargetPresetprofilesitem{
                    name: "FPS Rapid Trigger".into(),
                    guid: "d363ba48-79a8-422c-b405-29a07eff7e8e".into(),
                    slot_id: 3u64,
                    is_factory_preset: None
                },
                LatestKeyboardSourceProfileTransferTargetPresetprofilesitem{
                    name: "Analog WASD".into(),
                    guid: "2c481cef-62e0-4fe6-85a1-4d134c279c23".into(),
                    slot_id: 4u64,
                    is_factory_preset: None
                },
                LatestKeyboardSourceProfileTransferTargetPresetprofilesitem{
                    name: "Racing".into(),
                    guid: "474c90e3-7eab-4aa0-8030-34c43cbc707a".into(),
                    slot_id: 5u64,
                    is_factory_preset: None
                },
                LatestKeyboardSourceProfileTransferTargetPresetprofilesitem{
                    name: "High Sensitivity".into(),
                    guid: "177a20d5-f30a-4a52-923e-4fadb7db8392".into(),
                    slot_id: 6u64,
                    is_factory_preset: None
                }
            ],
            actuation_info: LatestKeyboardSourceProfileTransferTargetActuationinfo{
                default_value: 19656u64,
                max: 65520u64,
                min: 1638u64,
                unit: 1638u64,
                min_break: 1638u64
            },
            actuation_high_sensitivity_info: LatestKeyboardSourceProfileTransferTargetActuationhighsensitivityinfo{
                default_value: 13104u64,
                max: 65520u64,
                min: 1638u64,
                unit: 1638u64,
                min_break: 1638u64
            }
        },
        import_non_analog_ids: vec![
            "LEFT_FN".into(),
            "RIGHT_FN".into(),
            "DKM_KB_LEFTKNOB".into(),
            "DKM_KBMK_01".into(),
            "DKM_KBMK_02".into(),
            "DKM_KBMK_03".into(),
            "DKM_KBMK_04".into(),
            "DKM_KB_SIDE2".into(),
            "DKM_KB_SIDE3".into(),
            "ScrollDown".into(),
            "ScrollUp".into(),
            "ScrollButton".into(),
            "DIAL_CLICK".into(),
            "DIAL_LEFT_ROTATION".into(),
            "DIAL_RIGHT_ROTATION".into(),
            "mediaVolume".into(),
            "pauseandplay".into()
        ],
        import_keymap_passthrough_ids: vec![
            "KEY_UP_ARROW".into(),
            "KEY_DOWN_ARROW".into(),
            "KEY_LEFT_ARROW".into(),
            "KEY_RIGHT_ARROW".into(),
            "KEY_UP_DPAD".into(),
            "KEY_DOWN_DPAD".into(),
            "KEY_LEFT_DPAD".into(),
            "KEY_RIGHT_DPAD".into(),
            "KEY_UP_LEFT_ARROW".into(),
            "KEY_UP_RIGHT_ARROW".into(),
            "KEY_DOWN_LEFT_ARROW".into(),
            "KEY_DOWN_RIGHT_ARROW".into()
        ],
        import_restricted_layouts: vec![6u64,4u64,3u64,17u64,7u64,16u64,15u64,10u64,18u64],
        import_restricted_input_ids: vec!["KEY_BACKSLASH".into()],
        default_actuation: LatestKeyboardSourceDefaultActuation{
            specific: vec![].into_iter().collect(),
            rest: LatestKeyboardSourceDefaultActuationRest{
                threshold_h: 18018u64,
                threshold_l: 19656u64,
                ui_high: 1.2f64,
                ui_low: 1.2f64
            }
        },
        default_rapid_trigger: LatestKeyboardSourceDefaultRapidTrigger{
            specific: vec![].into_iter().collect(),
            rest: LatestKeyboardSourceDefaultRapidTriggerRest{
                enabled: false,
                ui_make: 0.4f64,
                ui_break: 0.4f64,
                down_sensitivity: 1638u64,
                up_sensitivity: 1638u64
            }
        }
    }
}
