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

pub fn source() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../assets/data/keyboard-latest-restore.json"
        ))
        .expect("validated current756 restore literals")
    })
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
        let firmware = &source()["firmware_keyboard"]["NormalMode"];
        let mut keys = std::collections::BTreeMap::new();
        for row in rows.iter().filter(|row| !truthy(&row["isHyperShift"])) {
            let Some(input) = row["inputID"].as_str() else { continue };
            let Some(id) = firmware[input]["fwID"].as_u64() else { continue };
            // The source skips unknown IDs and deduplicates by firmware ID.
            keys.entry(u8::try_from(id)?).or_insert_with(|| input.to_owned());
        }
        ensure!(!keys.is_empty(), "756 OBM assignment identities are empty");
        Ok(Self { keys: keys.into_iter().collect() })
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

fn disabled_duplicate(row: &Value) -> Value {
    let mut result = row.clone();
    result["inputType"] = json!("KeyInput");
    result["outputType"] = json!("disableGroup");
    // Source15691 deliberately has this nested disableGroup shape.
    result["disableGroup"] = json!({"disableGroup":{"disableAssignment":"Disable"}});
    let object = result.as_object_mut().unwrap();
    object.remove("interrupts");
    object.remove("mapping");
    result
}
pub fn firmware_mappings(rows: &[Value]) -> Vec<Value> {
    let mut result = rows.to_vec();
    let mut disabled = Vec::new();
    for row in &mut result {
        let stages = row["mapping"].as_array().cloned().unwrap_or_default();
        if row["inputType"] == "AnalogInput"
            && stages.first().is_some_and(|first| {
                first["outputType"] == "multimediaGroup"
                    && first["multimediaGroup"]["multimediaAssignment"] == "MuteAll"
            })
        {
            disabled.push(disabled_duplicate(row));
            continue;
        }
        let analog = row["inputType"] == "AnalogInput"
            && stages.iter().any(|s| {
                s["outputType"] == "joystickGroup"
                    && matches!(
                        s["joystickGroup"]["joystickMode"].as_str(),
                        Some("axes" | "raxes")
                    )
                    || s["outputType"] == "controllerGroup"
                        && truthy(&s["controllerGroup"]["controllerMode"]["isAnalog"])
            });
        let other = stages.iter().any(|s| {
            !matches!(
                s["outputType"].as_str(),
                Some(
                    "defaultGroup" | "joystickGroup" | "controllerGroup" | "dynamicKeyStrokeGroup"
                )
            )
        });
        if analog && other {
            disabled.push(disabled_duplicate(row));
            continue;
        }
        if matches!(row["inputType"].as_str(), Some("DKMInput" | "MouseInput")) {
            continue;
        }
        let contains = |kind: &str| stages.iter().any(|s| s["outputType"] == kind);
        let keyboard = contains("keyboardGroup");
        let joystick = contains("joystickGroup");
        let controller = contains("controllerGroup");
        if row["inputType"] == "AnalogInput"
            && stages.len() > 1
            && (stages.iter().all(|s| s["outputType"] == "keyboardGroup")
                || keyboard && controller
                || keyboard && joystick
                || joystick && controller)
        {
            disabled.push(disabled_duplicate(row));
            continue;
        }
        let mut needs_disable = false;
        for stage in stages {
            if stage["outputType"] == "defaultGroup" {
                continue;
            }
            if matches!(
                stage["outputType"].as_str(),
                Some("joystickGroup" | "controllerGroup" | "dynamicKeyStrokeGroup")
            ) {
                needs_disable = true;
                continue;
            }
            if let Some(kind) = stage["outputType"].as_str() {
                row["outputType"] = json!(kind);
                row[kind] = stage[kind].clone();
                row["inputType"] = json!("KeyInput");
                row.as_object_mut().unwrap().remove("mapping");
            }
        }
        if needs_disable {
            disabled.push(disabled_duplicate(row));
        }
    }
    result.extend(disabled);
    result
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

/// Current main26061 local defaults followed by So/default-engine preparation.
/// Local settings survive recompilation; transport identity is supplied by the
/// retained connection owner rather than inferred from a profile.
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
    let normalized = profiles
        .iter()
        .map(|profile| -> anyhow::Result<Value> {
            let mut value = default_profile();
            value.as_object_mut().unwrap().extend(
                profile
                    .as_object()
                    .context("756 local profile is not an object")?
                    .clone(),
            );
            ensure!(
                value["guid"].as_str().is_some_and(|guid| !guid.is_empty()),
                "756 local profile GUID is absent"
            );
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
    let mut document=previous.cloned().unwrap_or_else(||json!({
        "version":1,"vendorId":5426,"productId":756,"category":"KEYBOARD",
        "analogData":{"defaultMakeActuation":19656,"defaultBreakActuation":1638},
        "turnOffAnalogToKey":true,"gameModeEnabled":false,
        "reportIDs":{"11":"analogKeyboardV2ReportID","4":"razerKeyReportID"},
        "dimLighting":{"enabled":true,"value":5},"powerSaving":{"enabled":true,"value":15},"lowPowerMode":"Disable"
    }));
    for &field in features::DEVICE_FIELDS {
        if let Some(value) = device_settings.and_then(|settings| settings.get(field)) {
            document[field] = value.clone();
        }
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
            && features::DEVICE_FIELDS
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
    let rows = materialize(rows, false)?;
    Ok(rows.into_iter().filter(|row|truthy(&row["inputID"])).map(|mut row| {
        let input=row["inputID"].clone();
        let fill=|stage:&mut Value| {
            if stage["outputType"]=="keyboardGroup"&&stage["keyboardGroup"].as_object().is_none_or(|group|group.is_empty()) {
                stage["keyboardGroup"]=json!({"key":input,"keyGroup":0,"modifiers":[]});
            }
        };
        if let Some(stages)=row["mapping"].as_array_mut(){for stage in stages.iter_mut() {fill(stage);}}
        fill(&mut row);
        // Current6258 zm keeps one controller primary and the first custom
        // secondary. Keyboard products use the default-group secondary branch.
        if row["inputType"]=="AnalogInput"&&row["mapping"][0]["outputType"]=="controllerGroup" {
            let stages=row["mapping"].as_array().unwrap();
            let actuation=stages.iter().skip(1).find_map(|stage|stage.get("actuationPoint")).cloned().unwrap_or_else(||json!({"0":19656,"1":18018}));
            let secondary=stages.iter().skip(1).find(|stage|stage["outputType"]!="defaultGroup").cloned().unwrap_or_else(||json!({"actuationPoint":actuation,"isDefault":false,"outputType":"defaultGroup","isHyperShift":false}));
            row["mapping"]=json!([stages[0],secondary]);
        }
        row
    }).collect())
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
    let table = source()["firmware_keyboard"]["NormalMode"]
        .as_object()
        .context("756 firmware keyboard is absent")?;
    let act = defaults(guid, "preset_actuation", "default_actuation");
    let rt = defaults(guid, "preset_rapid_trigger", "default_rapid_trigger");
    let mut pair = AnalogPair {
        slot,
        guid: guid.into(),
        actuation: Vec::new(),
        rapid_trigger: Vec::new(),
    };
    for row in source()["physical_keys"]
        .as_array()
        .context("756 physical keys are absent")?
    {
        let input = row["inputID"]
            .as_str()
            .context("756 physical input ID is invalid")?;
        let Some(key) = row["fwID"]
            .as_u64()
            .and_then(|v| u8::try_from(v).ok())
            .filter(|id| *id != 255)
        else {
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
                .and_then(|v| v["fwID"].as_u64())
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
