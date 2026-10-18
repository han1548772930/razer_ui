//! Current740/746 module76435 wrapper and its custom mapping transform.
//! Unlike module1101, this wrapper does not restore consumed snapshots.
use super::*;
use crate::mapping_engine::{EngineContext, MappingEngine};
use crate::mapping_engine_hash::canonical_hash_input;
use std::collections::BTreeSet;

pub(crate) fn apply(
    document: &Value,
    running_games: Option<usize>,
    compose_f4: bool,
    context: &mut EngineContext,
) -> Result<Value> {
    ensure!(
        matches!(document["productId"].as_u64(), Some(740 | 746)),
        "Magnetic gaming product differs"
    );
    canonical_hash_input(document, &[])?;
    let profiles = document["profiles"]
        .as_array()
        .context("Magnetic gaming profiles are absent")?;
    let Some(index) = profiles
        .iter()
        .rposition(|profile| profile["guid"] == document["activeProfile"])
    else {
        return Ok(document.clone());
    };
    let original = &profiles[index];
    let raw = original["gamingMode"]
        .as_object()
        .context("Magnetic gaming settings are absent")?;
    let settings: GamingModeSettings = serde_json::from_value(Value::Object(
        raw.iter()
            .filter(|(key, _)| {
                [
                    "state",
                    "isWindowsKeyDisabled",
                    "isAltTabDisabled",
                    "isAltF4Disabled",
                    "isAltSpaceDisabled",
                    "isAltEnterDisabled",
                    "isFnSleepEnabled",
                    "scancode",
                ]
                .contains(&key.as_str())
            })
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    ))?;
    ensure!(settings.state <= 2, "Magnetic gaming state is invalid");
    let game_context = GamingMappingContext {
        game_mode_enabled: document
            .get("gameModeEnabled")
            .map(|value| {
                value
                    .as_bool()
                    .context("Magnetic global gaming mode is invalid")
            })
            .transpose()?,
        running_games,
        compose_f4,
        mod_tap: original.get("modTap").is_some_and(|value| value == true),
        firmware_analog_mapping: true,
    };
    // The source returns its original argument in this branch, including hashes.
    if !game_context.active(settings) {
        return Ok(document.clone());
    }
    let factory = MappingEngine::new(document["productId"].as_u64().unwrap() as u32)?;
    let mut draft = context.clone();
    let mut profile = original.clone();
    if profile.get("appEngine").is_none() {
        profile["appEngine"] = json!({"hash":"","mappings":[]});
    }
    let mut rows = profile["appEngine"]["mappings"]
        .as_array()
        .context("Magnetic gaming engine mappings are absent")?
        .clone();
    let mut result = document.clone();
    let mut default_engine = result["defaultMappings"]["appEngine"].clone();
    let mut defaults = match default_engine.get("mappings") {
        None => Vec::new(),
        Some(value) => value
            .as_array()
            .context("Magnetic gaming defaults are invalid")?
            .clone(),
    };
    apply_pass(
        &mut rows,
        &mut defaults,
        &profile["appEngine"],
        settings,
        true,
        true,
        game_context,
    )?;
    apply_pass(
        &mut rows,
        &mut defaults,
        &profile["appEngine"],
        settings,
        false,
        false,
        game_context,
    )?;
    apply_custom(&mut rows, &original["gamingMode"], &factory, &mut draft);
    profile["appEngine"]["mappings"] = json!(rows);
    profile["appEngine"]["gamemode"] = json!(true);
    profile["appEngine"]["hash"] = json!(source_hash(&profile["appEngine"])?);
    default_engine["mappings"] = json!(defaults);
    default_engine["hash"] = json!(source_hash(&default_engine)?);
    result["defaultMappings"]["appEngine"] = default_engine;
    let profiles = result["profiles"].as_array_mut().unwrap();
    profiles.remove(index);
    profiles.push(profile);
    *context = draft;
    Ok(result)
}

fn selected(settings: &Value) -> Vec<Value> {
    let Some(sets) = settings["customMappingSets"]
        .as_array()
        .filter(|sets| !sets.is_empty())
    else {
        return Vec::new();
    };
    let number = settings
        .get("customMappingSet")
        .and_then(Value::as_f64)
        .unwrap_or(1.0)
        - 1.0;
    let index = number.clamp(0.0, 2.0);
    let set = if index.fract() == 0.0 {
        sets.get(index as usize)
    } else {
        None
    }
    .unwrap_or(&sets[0]);
    set.as_array()
        .or_else(|| set["mappings"].as_array())
        .cloned()
        .unwrap_or_default()
}

fn combos(settings: &Value, engine: &MappingEngine) -> Vec<Value> {
    let Some(values) = [
        &settings["customGameMode"]["payload"]["combos"],
        &settings["customGameMode"]["combos"],
        &settings["payload"]["combos"],
        &settings["combos"],
    ]
    .into_iter()
    .find(|value| !value.is_null())
    .and_then(Value::as_array) else {
        return Vec::new();
    };
    values.iter().filter_map(|value|{
        if value["enabled"]!=true{return None;}
        let input=value["key1"].as_str().filter(|key|!key.is_empty())?;
        engine.source_key_metadata(input).ok()?;
        let target=value["key2"].as_str().filter(|key|!key.is_empty()).filter(|key|engine.source_key_metadata(key).is_ok());
        let mut row=json!({"inputID":input,"inputType":"KeyInput","isHyperShift":false,"customCombo":true});
        if let Some(target)=target {row["outputType"]=json!("keyboardGroup");row["keyboardGroup"]=json!({"key":target,"modifiers":[]});}
        else{row["disableOnly"]=json!(true);}
        Some(row)
    }).collect()
}

fn key_number(key: &Value) -> Option<u16> {
    key.get("key")
        .unwrap_or(&key["scancode"])
        .as_u64()
        .and_then(|scan| u16::try_from(scan).ok())
}

fn apply_combo(
    rows: &mut Vec<Value>,
    record: &Value,
    engine: &MappingEngine,
    seen: &mut BTreeSet<(u16, u8)>,
) {
    let Some(key) = record["inputID"]
        .as_str()
        .and_then(|input| engine.source_key_metadata(input).ok())
    else {
        return;
    };
    let Some(scan) = key_number(&key) else {
        return;
    };
    let disabled_only = record["disableOnly"] == true;
    let target = record["keyboardGroup"]["key"]
        .as_str()
        .and_then(|input| engine.source_key_metadata(input).ok());
    if !disabled_only && target.is_none() {
        return;
    }
    let normal = engine
        .source_keys()
        .iter()
        .any(|key| key.scancode == u32::from(scan) && key.flag == 0);
    let extended = engine
        .source_keys()
        .iter()
        .any(|key| key.scancode == u32::from(scan) && key.flag == 2);
    let base = key["flag"].as_u64().unwrap_or(0) as u8 & !1;
    let mut variants = Vec::new();
    if base == 0 && normal {
        variants.push(0);
    }
    if base == 2 && extended {
        variants.push(2);
    }
    if normal && !variants.contains(&0) {
        variants.push(0);
    }
    if extended && !variants.contains(&2) {
        variants.push(2);
    }
    for flag in variants.iter().flat_map(|flag| [*flag, *flag | 1]) {
        if !seen.insert((scan, flag)) {
            continue;
        }
        let existing = rows.iter().position(|row| {
            row["input"]["type"] == "keyboard"
                && row["input"]["scancode"] == scan
                && row["input"]["flag"] == flag
                && (!disabled_only || row["input"]["gamemode"] == true)
        });
        let previous = existing.map(|index| rows.remove(index));
        let output = if disabled_only {
            disabled()
        } else {
            previous
                .and_then(|row| row.get("output").cloned())
                .filter(|value| !value.is_null())
                .unwrap_or_else(|| keyboard(scan, flag))
        };
        push(rows, input("keyboard", scan, flag, None, true), output);
    }
    if disabled_only {
        return;
    }
    let Some(target) = target else {
        return;
    };
    let Some(target_scan) = key_number(&target) else {
        return;
    };
    let target_flag = target["flag"].as_u64().unwrap_or(0) as u8;
    for flag in [target_flag, target_flag | 1] {
        if rows.iter().any(|row| {
            row["input"]["type"] == "keyboard"
                && row["input"]["scancode"] == target_scan
                && row["input"]["flag"] == flag
        }) {
            continue;
        }
        let mut guards = [normal.then_some(0), extended.then_some(2)]
            .into_iter()
            .flatten()
            .map(|flag| {
                entry(
                    Some(input("keyboard", scan, flag, None, false)),
                    disabled(),
                    false,
                )
            })
            .collect::<Vec<_>>();
        guards.push(entry(None, keyboard(target_scan, flag), false));
        push(
            rows,
            input("keyboard", target_scan, flag, None, true),
            cascade(guards),
        );
    }
}

fn apply_custom(
    rows: &mut Vec<Value>,
    settings: &Value,
    engine: &MappingEngine,
    context: &mut EngineContext,
) {
    let mut records = selected(settings);
    records.extend(combos(settings, engine));
    let mut seen = BTreeSet::new();
    for record in records {
        if record["customCombo"] == true {
            apply_combo(rows, &record, engine, &mut seen);
            continue;
        }
        // The source catches each record independently; generated entries are
        // appended directly, without module1101's cascade restoration layer.
        if let Ok(generated) = engine.record(&record, context) {
            for mut row in generated {
                if !row["input"].is_object() {
                    continue;
                }
                row["input"]["gamemode"] = json!(true);
                rows.push(row);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inactive_wrapper_preserves_original_and_active_moves_last_matching_profile() {
        let input = json!({"productId":740,"activeProfile":"a","gameModeEnabled":false,"profiles":[
            {"guid":"a","gamingMode":{"state":1},"appEngine":{"mappings":[],"hash":"original"}},
            {"guid":"b","gamingMode":{"state":0},"appEngine":{"mappings":[]}}
        ],"defaultMappings":{"appEngine":{"mappings":[]}}});
        let mut context = EngineContext::default();
        assert_eq!(apply(&input, None, true, &mut context).unwrap(), input);
        let mut active = input.clone();
        active["gameModeEnabled"] = json!(true);
        let output = apply(&active, None, true, &mut context).unwrap();
        assert_eq!(output["profiles"][1]["guid"], "a");
        assert!(
            output["defaultMappings"]["appEngine"]["mappings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["input"]["type"] == "analogKey")
        );
        assert!(
            output["profiles"][1]["appEngine"]
                .get("__gameModeConsumedMappings")
                .is_none()
        );
    }
}
