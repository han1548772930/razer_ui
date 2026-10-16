//! Current756 restore dispatch: 6737 yo/taskRunner, generic56839/31982 and
//! custom-game feature ee. Plans contain requests, never fabricated readings.
use super::*;
use crate::keyboard_latest_protocol::Setting;
use crate::keyboard_latest_restore::Slot;

/// Current26061 device storage fields. Profile snapshots carry these only to
/// project the mounted UI; the device document remains their storage owner.
pub const DEVICE_FIELDS: &[&str] = &[
    "gameModeEnabled",
    "dimLighting",
    "powerSaving",
    "lowPowerMode",
    "analogData",
];

/// Current87142 notification5/OLED_MENU82 ->13881 ->50574 Rg/Lg.
/// Getter values reach the source reducer only through these hardware events;
/// the comparison getters used by ordinary setters have no UI side effects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PowerChange {
    DimLighting,
    PowerSaving,
}
impl PowerChange {
    pub fn from_packet(packet: &[u8]) -> Option<Self> {
        if packet.first() != Some(&5) || packet.get(1) != Some(&82) {
            return None;
        }
        match packet.get(2) {
            Some(12) => Some(Self::DimLighting),
            Some(13) => Some(Self::PowerSaving),
            _ => None,
        }
    }
    pub fn operations(self) -> Vec<crate::keyboard_latest_protocol::Operation> {
        use crate::keyboard_latest_protocol::Operation;
        match self {
            Self::DimLighting => vec![Operation::ReadTimeToDim { slot: 0 }],
            Self::PowerSaving => vec![
                Operation::ReadAutoSleepMode { slot: 0 },
                Operation::ReadTimeToSleep,
            ],
        }
    }
    pub fn observe(
        self,
        document: &Value,
        readings: &[crate::keyboard_latest_protocol::Reply],
    ) -> anyhow::Result<Value> {
        use crate::keyboard_latest_protocol::Reply;
        let (field, enabled, seconds) = match (self, readings) {
            (
                Self::DimLighting,
                [
                    Reply::TimeToDim {
                        slot: 0,
                        enabled,
                        seconds,
                        ..
                    },
                ],
            ) => ("dimLighting", *enabled, *seconds),
            (
                Self::PowerSaving,
                [
                    Reply::AutoSleepMode {
                        slot: 0, enabled, ..
                    },
                    Reply::TimeToSleep { seconds, .. },
                ],
            ) => ("powerSaving", *enabled, *seconds),
            _ => anyhow::bail!("756 power event getter sequence differs"),
        };
        ensure!(document.is_object(), "756 power event document is invalid");
        let mut result = document.clone();
        result[field] = json!({"isEnabled":enabled,"value":f64::from(seconds)/60.});
        // Rg/Lg advance the source version even for an unchanged reading.
        result["version"] = json!(next_source_version(
            document
                .get("version")
                .map(|version| version
                    .as_u64()
                    .context("756 power event version is invalid"))
                .transpose()?
        )?);
        Ok(result)
    }
}

/// Current34938 Ny global model and current UI gaming restore projection.
/// The effective UI state is separate from the saved profile preference.
pub fn effective_gaming_mode(profile: &Value, enabled: Option<bool>) -> Value {
    let mut mode = profile["gamingMode"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    if let Some(enabled) = enabled {
        let state = if !enabled {
            0
        } else if mode.get("state").and_then(Value::as_f64) == Some(2.) {
            2
        } else {
            1
        };
        mode.insert("state".into(), json!(state));
    }
    Value::Object(mode)
}

/// Source taskMakerSetGameMode's global branch. A device toggle preserves the
/// entire saved gamingMode object. Only changed options other than state bump
/// the source version; the global boolean still updates the engine/hash.
pub fn set_global_gaming_mode(
    document: &mut Value,
    incoming: &Value,
    from_ui: bool,
) -> anyhow::Result<Value> {
    let active = document["activeProfile"].clone();
    let index = document["profiles"]
        .as_array()
        .and_then(|profiles| {
            profiles
                .iter()
                .position(|profile| profile["guid"] == active)
        })
        .context("756 global game mode active profile is absent")?;
    let old = document["profiles"][index]["gamingMode"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    let mut rest = incoming
        .as_object()
        .cloned()
        .context("756 gaming mode payload is invalid")?;
    let explicit = rest
        .remove("gameModeEnabled")
        .and_then(|value| value.as_bool());
    let mut enabled = truthy(&document["gameModeEnabled"]);
    if from_ui {
        if let Some(value) = explicit {
            enabled = value;
        } else if let Some(state) = rest.get("state").and_then(Value::as_f64) {
            enabled = state > 0.;
        }
    } else if rest
        .get("state")
        .is_some_and(|state| state.is_number() && state.as_f64() != Some(2.))
    {
        enabled = !enabled;
    }
    let mut saved = old.clone();
    if from_ui {
        saved.extend(rest);
    }
    let mut old_options = old;
    old_options.remove("state");
    let mut new_options = saved.clone();
    new_options.remove("state");
    if source_hash(&Value::Object(old_options))? != source_hash(&Value::Object(new_options))? {
        document["version"] = json!(next_source_version(document["version"].as_u64())?);
    }
    document["gameModeEnabled"] = json!(enabled);
    let profile = &mut document["profiles"][index];
    profile["gamingMode"] = Value::Object(saved);
    if !profile["appEngine"].is_object() {
        profile["appEngine"] = json!({"hash":"","mappings":[]});
    }
    profile["appEngine"]["gamemode"] = json!(enabled);
    profile["appEngine"]["hash"] = json!(source_hash(&profile["appEngine"])?);
    let mut effective = effective_gaming_mode(profile, Some(enabled));
    if profile["guid"] == "af06d371-861f-4b98-8d78-bfaef5cfdebf" {
        effective["state"] = json!(0);
    }
    Ok(effective)
}

#[derive(Clone, Debug)]
pub enum Action {
    Write(Setting),
    Analog {
        slot: u8,
        guid: String,
        removed: Vec<Value>,
        mappings: Vec<Value>,
    },
    Gaming {
        slots: Vec<u8>,
        mode: Value,
    },
    SnapTap {
        slot: u8,
        enabled: bool,
        pairs: Vec<[u8; 3]>,
    },
    SingleKeySnapTap {
        slots: Vec<u8>,
        enabled: bool,
        keys: Vec<[u8; 2]>,
    },
    CustomGame {
        slots: Vec<u8>,
        combos: [[u8; 3]; 3],
        extension_updates: Vec<bool>,
    },
    Dim {
        enabled: bool,
        minutes: f64,
    },
    Sleep {
        enabled: bool,
        minutes: f64,
    },
}
pub fn profile(document: &Value) -> anyhow::Result<&Value> {
    document["profiles"]
        .as_array()
        .and_then(|profiles| {
            profiles
                .iter()
                .find(|profile| profile["guid"] == document["activeProfile"])
        })
        .context("756 restore active profile is missing")
}

/// Current50574 Nv/Uv ->46329 $i/zi. Global Sync writes indicator255 and
/// physical keys to actual matching OBM slots plus software slot1. It does
/// not save the preview value into profile mappings or analogData.
pub fn global_sync_operations(
    document: &Value,
    observed: &[Slot],
    assignments: &ObmButtonAssignments,
    message: &Value,
    serial: &str,
) -> anyhow::Result<Vec<crate::keyboard_latest_protocol::Operation>> {
    use crate::keyboard_latest_protocol::Operation;
    let profile = profile(document)?;
    let guid = profile["guid"]
        .as_str()
        .context("756 global Sync profile GUID is absent")?;
    let mappings = active_mappings(profile)?;
    let mut slots = Vec::new();
    for slot in observed
        .iter()
        .filter(|slot| slot.guid == guid)
        .map(|slot| slot.slot_id)
        .chain([1])
    {
        if !slots.contains(&slot) {
            slots.push(slot);
        }
    }
    let raw = |value: &Value| -> anyhow::Result<u16> {
        let value = value
            .as_f64()
            .context("756 global Sync value is not numeric")?
            .floor();
        ensure!(
            value.is_finite() && (0.0..=65535.).contains(&value),
            "756 global Sync value exceeds u16"
        );
        Ok(value as u16)
    };
    let rapid = |value: &Value| {
        raw(&json!(
            value
                .as_f64()
                .context("756 rapid trigger sensitivity is absent")?
                * 1638.
        ))
    };
    let mut result = Vec::new();
    match message["type"].as_str() {
        Some("ON_SYNC_GLOBAL_ACTUATION") => {
            let point = &message["payload"]["value"];
            let global = raw(point
                .get("0")
                .or_else(|| point.get(0))
                .context("756 global actuation point is absent")?)?;
            let mut points = vec![crate::keyboard_analog::ActuationPoint {
                key_id: 255,
                threshold_low: global,
                threshold_high: global.saturating_sub(1638).max(256),
            }];
            let default = defaults(guid, "preset_actuation", "default_actuation");
            for (key_id, input) in assignments.keys() {
                let input = input.as_str();
                let primary = mappings
                    .iter()
                    .rfind(|row| row["inputID"] == input && row["inputType"] == "AnalogInput");
                let normal = mappings.iter().find(|row| {
                    row["inputID"] == input
                        && row["inputType"] == "AnalogInput"
                        && !truthy(&row["isHyperShift"])
                });
                let blocked = normal.is_some_and(|row| {
                    let stage = if row["mapping"].is_array() {
                        &row["mapping"][0]
                    } else {
                        row
                    };
                    let secondary = &row["mapping"][1];
                    secondary
                        .as_object()
                        .is_some_and(|secondary| !secondary.is_empty())
                        && secondary["outputType"] != "defaultGroup"
                        || matches!(
                            stage["outputType"].as_str(),
                            Some("controllerGroup" | "joystickGroup" | "disableGroup")
                        )
                }) || source()["default_mappings"].as_array().is_some_and(|rows| {
                    rows.iter().any(|row| {
                        row["inputID"] == input && row["outputType"] == "profileNavigationGroup"
                    })
                });
                let (low, high) = if blocked {
                    let default = per_key(default, input);
                    (raw(&default["thresholdL"])?, raw(&default["thresholdH"])?)
                } else if let Some(row) = primary {
                    let point = &row["mapping"][0]["actuationPoint"];
                    let value = point
                        .get(0)
                        .or_else(|| point.get("0"))
                        .context("756 mapped actuation value is absent")?;
                    let low = raw(value)?;
                    (low, low.saturating_sub(1638).max(256))
                } else {
                    (global, global.saturating_sub(1638).max(256))
                };
                points.push(crate::keyboard_analog::ActuationPoint {
                    key_id: *key_id,
                    threshold_low: low,
                    threshold_high: high,
                });
            }
            let mut pair = AnalogPair {
                slot: 1,
                guid: guid.into(),
                actuation: points,
                rapid_trigger: vec![],
            };
            travel::Caps::for_serial(document, serial)?.analog_pair(&mut pair);
            for slot in slots {
                for chunk in pair.actuation.chunks(15) {
                    result.push(Operation::Actuation {
                        slot,
                        points: chunk.to_vec(),
                    });
                }
            }
        }
        Some("ON_SYNC_GLOBAL_RAPID_TRIGGER") => {
            let value = &message["payload"]["value"];
            let down = rapid(&value["makeSensitivity"])?;
            let up = if truthy(&value["isEnableUpStroke"]) {
                rapid(&value["breakSensitivity"])?
            } else {
                down.saturating_sub(1638).max(256)
            };
            let mut rows = vec![crate::keyboard_analog::RapidTriggerKey {
                key_id: 255,
                enabled: 1,
                down_sensitivity: down,
                up_sensitivity: up,
            }];
            let default = &defaults(guid, "preset_rapid_trigger", "default_rapid_trigger")["rest"];
            for (key_id, input) in assignments.keys() {
                let input = input.as_str();
                let saved = mappings.iter().rfind(|row| {
                    row["inputID"] == input
                        && row["inputType"] == "AnalogInput"
                        && truthy(&row["mapping"][0]["rapidTrigger"])
                });
                let (enabled, down, up) = if let Some(row) = saved {
                    let value = &row["mapping"][0]["rapidTrigger"];
                    let down = rapid(&value["makeSensitivity"])?;
                    (
                        1,
                        down,
                        if truthy(&value["isEnableUpStroke"]) {
                            rapid(&value["breakSensitivity"])?
                        } else {
                            down.saturating_sub(1638).max(256)
                        },
                    )
                } else {
                    (
                        u8::from(truthy(&default["enabled"])),
                        raw(&default["downSensitivity"])?,
                        raw(&default["upSensitivity"])?,
                    )
                };
                rows.push(crate::keyboard_analog::RapidTriggerKey {
                    key_id: *key_id,
                    enabled,
                    down_sensitivity: down,
                    up_sensitivity: up,
                });
            }
            for slot in slots {
                for chunk in rows.chunks(13) {
                    result.push(Operation::RapidTrigger {
                        slot,
                        keys: chunk.to_vec(),
                    });
                }
            }
        }
        _ => anyhow::bail!("756 global Sync message is unsupported"),
    }
    Ok(result)
}

/// Current50574 sy/cy: resolve the physical event using current93494 Rk,
/// including outputFlag's precedence over flag. Only the configured fallback
/// key is used for an unknown event; 756 has no fallback key ID.
pub fn navigation_feedback(
    guid: &str,
    input: &Value,
    observed: &[crate::keyboard_latest_protocol::ProfileMetadata],
) -> anyhow::Result<Option<Setting>> {
    let input = input
        .as_str()
        .context("756 navigation input JSON is absent")?;
    let input: Value = serde_json::from_str(input)?;
    let keys = source()["navigation_keys"]
        .as_array()
        .context("756 navigation keys are absent")?;
    let matched = keys.iter().find(|key| {
        key["scancode"] == input["scancode"]
            && key
                .get("outputFlag")
                .filter(|flag| !flag.is_null())
                .unwrap_or(&key["flag"])
                .as_u64()
                .and_then(|flag| flag.checked_add(1))
                == input["flag"].as_u64()
    });
    let config = &source()["profile_switch_effect"];
    let key = if let Some(input) = matched.and_then(|key| key["inputID"].as_str()) {
        source()["firmware_keyboard"]["NormalMode"][input]["fwID"].as_u64()
    } else {
        config["keyID"].as_u64()
    };
    let slot = observed
        .iter()
        .find(|profile| profile.guid == guid && !profile.name.is_empty())
        .map(|profile| profile.slot_id)
        .or_else(|| (config["alwaysTriggerSlot1Effect"] == true).then_some(1));
    match (slot, key.and_then(|key| u8::try_from(key).ok())) {
        (Some(slot), Some(key)) if slot > 0 && key > 0 => {
            Ok(Some(Setting::TriggerPresetEffect { slot, key }))
        }
        _ => Ok(None),
    }
}

/// Current50574 dy -> resetActuationObm/resetRapidTriggerObm. These use the
/// preset's rest values for every physical key, with a separate indicator
/// task before the physical-key task in each protocol. Saved mappings remain
/// untouched and the physical-key write begins a pending restore journal.
pub fn navigation_reset_operations(
    document: &Value,
    serial: &str,
) -> anyhow::Result<Vec<crate::keyboard_latest_protocol::Operation>> {
    use crate::keyboard_latest_protocol::Operation;
    let profile = profile(document)?;
    let guid = profile["guid"]
        .as_str()
        .context("756 navigation reset GUID is absent")?;
    let act = &defaults(guid, "preset_actuation", "default_actuation")["rest"];
    let rt = &defaults(guid, "preset_rapid_trigger", "default_rapid_trigger")["rest"];
    let mut pair = AnalogPair {
        slot: 1,
        guid: guid.into(),
        actuation: Vec::new(),
        rapid_trigger: Vec::new(),
    };
    let ids = std::iter::once(Ok(255)).chain(
        source()["physical_keys"]
            .as_array()
            .context("756 physical keys are absent")?
            .iter()
            .map(|key| {
                key["fwID"]
                    .as_u64()
                    .context("756 navigation reset key is absent")
                    .and_then(|id| u8::try_from(id).map_err(Into::into))
            }),
    );
    for id in ids {
        let key_id = id?;
        pair.actuation.push(crate::keyboard_analog::ActuationPoint {
            key_id,
            threshold_low: u16_value(&act["thresholdL"], "reset actuation make")?,
            threshold_high: u16_value(&act["thresholdH"], "reset actuation release")?,
        });
        pair.rapid_trigger
            .push(crate::keyboard_analog::RapidTriggerKey {
                key_id,
                enabled: u8::from(truthy(&rt["enabled"])),
                down_sensitivity: u16_value(&rt["downSensitivity"], "reset rapid trigger make")?,
                up_sensitivity: u16_value(&rt["upSensitivity"], "reset rapid trigger release")?,
            });
    }
    travel::Caps::for_serial(document, serial)?.analog_pair(&mut pair);
    let mut operations = vec![Operation::Actuation {
        slot: 1,
        points: pair.actuation[..1].to_vec(),
    }];
    operations.extend(
        pair.actuation[1..]
            .chunks(15)
            .map(|points| Operation::Actuation {
                slot: 1,
                points: points.to_vec(),
            }),
    );
    operations.push(Operation::RapidTrigger {
        slot: 1,
        keys: pair.rapid_trigger[..1].to_vec(),
    });
    operations.extend(
        pair.rapid_trigger[1..]
            .chunks(13)
            .map(|keys| Operation::RapidTrigger {
                slot: 1,
                keys: keys.to_vec(),
            }),
    );
    Ok(operations)
}

fn key(input: &Value) -> u8 {
    input
        .as_str()
        .and_then(|id| MappingEngine::new(756).ok()?.source_key_metadata(id).ok())
        .and_then(|metadata| metadata["analogKeyID"].as_u64())
        .and_then(|id| u8::try_from(id).ok())
        .unwrap_or(0)
}
fn key_or_number(input: &Value) -> u8 {
    let value = key(input);
    if value > 0 {
        value
    } else {
        input
            .as_u64()
            .or_else(|| input.as_str()?.parse().ok())
            .and_then(|value| u8::try_from(value).ok())
            .unwrap_or(0)
    }
}
fn hid(input: &Value) -> anyhow::Result<u8> {
    let metadata = MappingEngine::new(756)?.source_key_metadata(
        input
            .as_str()
            .context("756 single-key Snap Tap break key is invalid")?,
    )?;
    let number = metadata["HID"]
        .as_u64()
        .or_else(|| metadata["HID"].as_str()?.parse().ok())
        .context("756 single-key Snap Tap break HID is absent")?;
    Ok(u8::try_from(number)?)
}
fn unique_keys(values: impl IntoIterator<Item = Value>) -> Vec<Value> {
    let mut result = Vec::new();
    for value in values {
        if !result.contains(&value) {
            result.push(value);
        }
    }
    result
}
/// The source removes conflicting single-key pairs when either side becomes
/// DKS, renumbers the retained UI rows, then persists the candidate.
pub fn remove_dks_conflicts(document: &Value) -> anyhow::Result<Value> {
    let mut result = document.clone();
    let active = result["activeProfile"].clone();
    let Some(profile) = result["profiles"].as_array_mut().and_then(|profiles| {
        profiles
            .iter_mut()
            .find(|profile| profile["guid"] == active)
    }) else {
        return Ok(result);
    };
    let conflicting = active_mappings(profile)?
        .iter()
        .filter(|row| {
            row["mapping"].as_array().is_some_and(|stages| {
                stages
                    .iter()
                    .any(|stage| stage["outputType"] == "dynamicKeyStrokeGroup")
            })
        })
        .map(|row| row["inputID"].clone())
        .collect::<Vec<_>>();
    let Some(keys) = profile["singleKeySnapTap"]["keyList"].as_array_mut() else {
        return Ok(result);
    };
    let before = keys.len();
    keys.retain(|row| {
        !conflicting.contains(&row["keyMapMake"]) && !conflicting.contains(&row["keyMapBreak"])
    });
    if before != keys.len() {
        for (index, row) in keys.iter_mut().enumerate() {
            row["id"] = json!(index + 1);
        }
        result["version"] = json!(next_source_version(document["version"].as_u64())?);
    }
    Ok(result)
}
pub fn actions(
    document: &Value,
    previous: Option<&Value>,
    observed: &[Slot],
    running_games: Option<usize>,
    wireless: bool,
) -> anyhow::Result<Vec<Action>> {
    let profile = profile(document)?;
    let old = previous.and_then(|doc| self::profile(doc).ok());
    let changed = |field: &str| {
        old.is_none_or(|old| old["guid"] != profile["guid"] || old[field] != profile[field])
    };
    let guid = profile["guid"]
        .as_str()
        .context("756 active GUID is absent")?;
    // Slot1 is the software profile; OBM slots come only from actual inventory.
    let affected = std::iter::once(1)
        .chain(
            observed
                .iter()
                .filter(|slot| slot.guid == guid)
                .map(|slot| slot.slot_id),
        )
        .collect::<Vec<_>>();
    let unique = || {
        let mut slots = Vec::new();
        for slot in &affected {
            if !slots.contains(slot) {
                slots.push(*slot);
            }
        }
        slots
    };
    let mut result = Vec::new();
    if previous.is_none_or(|doc| doc["activeProfile"] != document["activeProfile"]) {
        result.push(Action::Write(Setting::AdjustmentMode {
            slot: 1,
            enabled: !source()["preset_guids"]
                .as_object()
                .is_some_and(|guids| guids.values().any(|preset| preset == guid)),
        }));
    }
    if changed("brightness") {
        if let Some(value) = profile["brightness"]["value"].as_f64() {
            result.push(Action::Write(Setting::Brightness {
                slot: 1,
                region: 0,
                percent: if truthy(&profile["brightness"]["isEnabled"]) {
                    value
                } else {
                    0.
                },
            }));
        }
    }
    let polling = if wireless {
        "pollingRateWireless"
    } else {
        "pollingRate"
    };
    if changed(polling) {
        if let Some(rate) = profile[polling].as_f64() {
            ensure!(
                rate.is_finite() && rate.fract() == 0.,
                "756 stored polling rate is invalid"
            );
            result.push(Action::Write(Setting::PollingRate {
                slot: 1,
                hertz: rate as u32,
            }));
        }
    }
    if changed("gamingMode")
        || previous.is_some_and(|doc| doc["gameModeEnabled"] != document["gameModeEnabled"])
    {
        let effective = effective_gaming_mode(profile, document["gameModeEnabled"].as_bool());
        let mode = &effective;
        let enabled = match mode["state"].as_u64().unwrap_or(0) {
            0 => false,
            1 => true,
            _ => running_games.is_some_and(|count| count > 0),
        };
        result.push(Action::Write(Setting::LedState {
            class: 0,
            led: 8,
            enabled,
        }));
        result.push(Action::Gaming {
            slots: unique(),
            mode: mode.clone(),
        });
    }
    if changed("continuousRapidTrigger") {
        for &slot in &affected {
            result.push(Action::Write(Setting::ContinuousRapidTrigger {
                slot,
                enabled: truthy(&profile["continuousRapidTrigger"]),
            }));
        }
    }
    if changed("mappings") || changed("keymaps") || changed("activeKeymapId") {
        let mappings = active_mappings(profile)?.to_vec();
        let removed = old
            .and_then(|old| active_mappings(old).ok())
            .unwrap_or_default()
            .iter()
            .filter(|row| !mappings.contains(row))
            .cloned()
            .collect::<Vec<_>>();
        for &slot in &affected {
            result.push(Action::Analog {
                slot,
                guid: guid.into(),
                removed: removed.clone(),
                mappings: mappings.clone(),
            });
        }
    }
    // The current8k analog feature restores these controls through analog_obm,
    // after the ordinary profile fields and before interrupt/custom features.
    if changed("snapTap") {
        let data = &profile["snapTap"];
        if let Some(keys) = data["keyList"].as_array().filter(|keys| !keys.is_empty()) {
            ensure!(
                keys.len() <= 4,
                "756 Snap Tap contains more than four pairs"
            );
            let pairs = keys
                .iter()
                .map(|row| {
                    [
                        key(&row["key1"]),
                        key(&row["key2"]),
                        [
                            "LAST_INPUT",
                            "PRIO_LEFT",
                            "PRIO_RIGHT",
                            "NEUTRAL",
                            "COMPARE_LEVEL",
                        ]
                        .iter()
                        .position(|mode| row["mode"] == *mode)
                        .unwrap_or(0) as u8,
                    ]
                })
                .collect();
            result.push(Action::SnapTap {
                slot: observed
                    .iter()
                    .find(|slot| slot.guid == guid)
                    .map(|slot| slot.slot_id)
                    .unwrap_or(1),
                enabled: truthy(&data["isEnabled"]),
                pairs,
            });
        }
    }
    if changed("bottomDeadzone") {
        let value = match profile["bottomDeadzone"]["value"]
            .as_str()
            .unwrap_or("medium")
            .to_ascii_lowercase()
            .as_str()
        {
            "responsive" => 3,
            "stable" => 8,
            _ => 5,
        };
        for &slot in &affected {
            result.push(Action::Write(Setting::BottomDeadzone { slot, value }));
        }
    }
    if changed("singleKeySnapTap") {
        let data = &profile["singleKeySnapTap"];
        let keys = data["keyList"]
            .as_array()
            .context("756 single-key Snap Tap list is invalid")?
            .iter()
            .map(|row| Ok([key(&row["keyMapMake"]), hid(&row["keyMapBreak"])?]))
            .collect::<anyhow::Result<Vec<_>>>()?;
        ensure!(
            keys.len() <= 4,
            "756 single-key Snap Tap list exceeds four pairs"
        );
        result.push(Action::SingleKeySnapTap {
            slots: unique(),
            enabled: truthy(&data["isEnabled"]),
            keys,
        });
    }
    if changed("interruptKey") {
        let data = &profile["interruptKey"];
        let keys = unique_keys(data["interrupts"].as_array().cloned().unwrap_or_default());
        let any = data["interruptionKey"] == "any" || keys.contains(&json!("KEY_ANY"));
        let keys = if any {
            vec![]
        } else {
            keys.iter().map(key).collect()
        };
        ensure!(keys.len() <= 4, "756 interrupt keys exceed four keys");
        for dynamic_keystroke in [false, true] {
            for &slot in &affected {
                result.push(Action::Write(Setting::InterruptKeys {
                    slot,
                    dynamic_keystroke,
                    enabled: truthy(&data["isEnabled"]),
                    mode: u8::from(!any),
                    keys: keys.clone(),
                }));
            }
        }
    }
    if changed("dualKeypressPriority") {
        let data = &profile["dualKeypressPriority"];
        let values = match &data["keys"] {
            Value::Array(keys) => keys.clone(),
            Value::Object(keys) => keys.values().cloned().collect(),
            _ => vec![],
        };
        let keys = unique_keys(values).iter().map(key).collect::<Vec<_>>();
        for &slot in &affected {
            result.push(Action::Write(Setting::DualKeypressPriority {
                slot,
                enabled: truthy(&data["isEnabled"]),
                keys: keys.clone(),
            }));
        }
    }
    if changed("gamingMode") {
        let custom = &profile["gamingMode"]["customGameMode"];
        let rows = custom["payload"]["combos"]
            .as_array()
            .or_else(|| custom["combos"].as_array());
        let mut combos = [[0; 3]; 3];
        let mut extension_updates = Vec::new();
        if let Some(rows) = rows {
            for (index, row) in rows.iter().take(3).enumerate() {
                extension_updates.push(truthy(&row["enabled"]));
                if row["enabled"] == true {
                    let first = key_or_number(&row["key1"]);
                    let second = key_or_number(&row["key2"]);
                    let count = row["keyNum"]
                        .as_u64()
                        .map(|n| n.min(2) as u8)
                        .unwrap_or(if second > 0 { 2 } else { 1 });
                    combos[index] = [count, first, if count == 2 { second } else { 0 }];
                }
            }
        }
        result.push(Action::CustomGame {
            slots: unique(),
            combos,
            extension_updates,
        });
    }
    for (field, dim) in [("dimLighting", true), ("powerSaving", false)] {
        // 756 has no singleProfileDevice flag. Full restore deliberately
        // skips device-wide power defaults; an actual UI mutation still sets.
        if previous.is_some_and(|old| old[field] != document[field]) {
            let data = &document[field];
            if let Some(minutes) = data["value"].as_f64() {
                let enabled = truthy(&data["isEnabled"]);
                result.push(if dim {
                    Action::Dim { enabled, minutes }
                } else {
                    Action::Sleep { enabled, minutes }
                });
            }
        }
    }
    Ok(result)
}
