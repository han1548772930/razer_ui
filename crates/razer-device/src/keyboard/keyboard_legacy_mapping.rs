//! Wired PID 515 current middleware document preparation. Original inputs:
//! main.8342a5d11f2dd0d29680.js DeviceInfo/defaults; 2520.0964e34572ce7e761fa8.js
//! factories; 2592.ca84d3908676d84b1779.js modules 72937/42731/31064.
//! Compiling a local profile never acknowledges mapping-engine publication or HID writes.
use crate::{
    keyboard_gaming_mapping::{
        GamingMappingContext, GamingModeSettings, KeyboardGamingMappingPlan,
    },
    mapping_engine::{EngineContext, MappingEngine},
    mapping_engine_hash::source_hash,
};
use anyhow::{Context as _, ensure};
use serde_json::{Value, json};

pub fn default_profile() -> Value {
    json!({"guid":"","name":"","switchOffLighting":{"isDisplayOn":false,"isIdleEnabled":false,"idleMinutes":1},
        "brightness":{"isEnabled":true,"value":100},"quickEffects":{"selectedEffectId":3},
        "gamingMode":{"state":0,"isWindowsKeyDisabled":false,"isAltTabDisabled":false,"isAltF4Disabled":false},
        "snapTap":{"isEnabled":false,"keyList":[{"key1":"KEY_A","key2":"KEY_D","id":1}]},"macro":{},"mappings":[]})
}

pub fn source_defaults() -> Vec<Value> {
    let mut records = Vec::new();
    for (input, assignment, turbo) in [
        ("KEY_F1", "MuteVolume", false),
        ("KEY_F2", "VolumeDown", true),
        ("KEY_F3", "VolumeUp", true),
        ("KEY_F5", "PrevTrack", false),
        ("KEY_F6", "Play", false),
        ("KEY_F7", "NextTrack", false),
    ] {
        let mut group = json!({"multimediaAssignment":assignment});
        if turbo {
            group["useTurbo"] = json!(true);
        }
        records.push(json!({"outputType":"multimediaGroup","isHyperShift":true,"inputType":"KeyInput","inputID":input,"multimediaGroup":group}));
    }
    for (input, output) in [("KEY_F9", "otfmGroup"), ("KEY_F10", "gameModeGroup")] {
        records.push(
            json!({"outputType":output,"isHyperShift":true,"inputType":"KeyInput","inputID":input}),
        );
    }
    for (input, assignment) in [("KEY_F11", "BrightnessDown"), ("KEY_F12", "BrightnessUp")] {
        records.push(json!({"outputType":"backlightGroup","isHyperShift":true,"inputType":"KeyInput","inputID":input,"backlightGroup":{"backlightAssignment":assignment}}));
    }
    records.push(json!({"outputType":"systemGroup","isHyperShift":true,"inputType":"KeyInput","inputID":"KEY_PAUSE","systemGroup":{"systemAssignment":"Sleep"}}));
    records
}

pub fn prepare_document(
    profiles: &[Value],
    active_profile: &str,
    previous: Option<&Value>,
    fresh_guids: impl IntoIterator<Item = String>,
) -> anyhow::Result<Value> {
    let mut context = EngineContext {
        default_turbos: previous
            .and_then(|p| p["defaultTurbos"].as_array())
            .cloned(),
        synapse_turbos: Some(
            previous
                .and_then(|p| p["synapseTurbos"].as_array())
                .cloned()
                .unwrap_or_default(),
        ),
        fresh_guids: fresh_guids.into_iter().collect(),
        ..Default::default()
    };
    let profiles = profiles
        .iter()
        .map(|profile| -> anyhow::Result<Value> {
            let mut normalized = default_profile();
            normalized.as_object_mut().unwrap().extend(
                profile
                    .as_object()
                    .context("Local keyboard profile is not an object")?
                    .clone(),
            );
            if let Some(old_engine) = previous
                .and_then(|p| p["profiles"].as_array())
                .and_then(|p| p.iter().find(|p| p["guid"] == normalized["guid"]))
                .and_then(|p| p.get("appEngine"))
            {
                normalized["appEngine"] = old_engine.clone();
            }
            Ok(normalized)
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let version = crate::mapping_submission::next_source_version(
        previous.and_then(|p| p["version"].as_u64()),
    )?;
    let mut document =
        json!({"version":version,"profiles":profiles,"activeProfile":active_profile});
    if let Some(enabled) = previous.and_then(|document| document.get("gameModeEnabled")) {
        document["gameModeEnabled"] = enabled.clone();
    }
    compile(&document, &source_defaults(), &mut context, None)
}

pub fn compile(
    document: &Value,
    source_defaults: &[Value],
    context: &mut EngineContext,
    running_games: Option<usize>,
) -> anyhow::Result<Value> {
    let factory = MappingEngine::new(515)?;
    let mut context_draft = context.clone();
    for (id, key) in [
        ("DKM_M_01", 32),
        ("DKM_M_02", 33),
        ("DKM_M_03", 34),
        ("DKM_M_04", 35),
        ("DKM_M_05", 36),
    ] {
        context_draft
            .dkm_keys
            .insert(id.into(), json!({"inputID":id,"key":key}));
    }
    if context_draft.default_turbos.is_none() {
        factory.generate_default_turbos(&mut context_draft)?;
    }
    context_draft.synapse_turbos.get_or_insert_with(Vec::new);
    let mut default_engine = factory
        .generate(source_defaults, &mut context_draft)?
        .app_engine;
    for hypershift in [false, true] {
        for flag in [0, 1] {
            default_engine["mappings"]
                .as_array_mut()
                .unwrap()
                .push(json!({
                "input":{"type":"razerKey","key":1,"hypershift":hypershift,"flag":flag},
                "output":{"type":"hypershift","flag":flag}}));
        }
    }
    default_engine["hash"] = json!(source_hash(&default_engine)?);
    let mut result = document.clone();
    let active = result["activeProfile"]
        .as_str()
        .context("No active local keyboard profile")?
        .to_owned();
    let profiles = result["profiles"]
        .as_array_mut()
        .context("No local keyboard profiles")?;
    ensure!(
        profiles.iter().filter(|p| p["guid"] == active).count() == 1,
        "Active local profile must be unique"
    );
    for profile in profiles.iter_mut() {
        let replaced = profile["appEngine"]["mappings"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|mapping| mapping["output"]["type"] == "turbo")
            .filter_map(|mapping| mapping["output"]["guid"].as_str().map(str::to_owned))
            .collect::<std::collections::BTreeSet<_>>();
        if let Some(turbos) = &mut context_draft.synapse_turbos {
            turbos.retain(|turbo| {
                !turbo["guid"]
                    .as_str()
                    .is_some_and(|guid| replaced.contains(guid))
            });
        }
        let records = profile["mappings"]
            .as_array()
            .context("Local keyboard mappings are missing")?;
        profile["appEngine"] = factory.generate(records, &mut context_draft)?.app_engine;
    }
    let active_index = profiles.iter().position(|p| p["guid"] == active).unwrap();
    let mut profile = profiles.remove(active_index);
    let settings = &profile["gamingMode"];
    let state = settings["state"]
        .as_u64()
        .context("Local gaming-mode state is missing")?;
    ensure!(state <= 2, "Invalid local gaming-mode state");
    let gaming = GamingModeSettings {
        state: state as u8,
        is_windows_key_disabled: settings["isWindowsKeyDisabled"] == true,
        is_alt_tab_disabled: settings["isAltTabDisabled"] == true,
        is_alt_f4_disabled: settings["isAltF4Disabled"] == true,
        ..Default::default()
    };
    let plan = KeyboardGamingMappingPlan::prepare(
        515,
        &profile["appEngine"],
        &default_engine,
        gaming,
        GamingMappingContext {
            running_games,
            game_mode_enabled: document.get("gameModeEnabled").and_then(Value::as_bool),
            ..Default::default()
        },
        || Ok(()),
    )?;
    profile["appEngine"] = plan.active_engine().clone();
    apply_snap_tap(&factory, &mut profile, &context_draft)?;
    profiles.push(profile);
    result["defaultMappings"] = json!({"appEngine":plan.default_engine()});
    result["version"] = document.get("version").cloned().unwrap_or(json!(1));
    result["vendorId"] = json!(5426);
    result["productId"] = json!(515);
    result["category"] = json!("KEYBOARD");
    result["bluetoothLE"] = json!(false);
    result["reportIDs"] = json!({"4":"razerKeyReportID","5":"hardwareEventReportID"});
    result["defaultTurbos"] = json!(context_draft.default_turbos);
    result["synapseTurbos"] = json!(context_draft.synapse_turbos);
    *context = context_draft;
    Ok(result)
}

fn apply_snap_tap(
    factory: &MappingEngine,
    profile: &mut Value,
    context: &EngineContext,
) -> anyhow::Result<()> {
    if profile["snapTap"]["isEnabled"] != true {
        return Ok(());
    }
    let pairs = profile["snapTap"]["keyList"]
        .as_array()
        .context("Snap Tap key list is missing")?
        .clone();
    let mappings = profile["appEngine"]["mappings"].as_array_mut().unwrap();
    for pair in pairs {
        let id = pair["id"]
            .as_u64()
            .context("Snap Tap pair identity is missing")?;
        for input in ["key1", "key2"] {
            let name = pair[input]
                .as_str()
                .context("Snap Tap key identity is missing")?;
            let dkm = context.dkm_keys.get(name);
            let key = if let Some(key) = dkm {
                key.clone()
            } else {
                factory.source_key_metadata(name)?
            };
            let field = if dkm.is_some() { "key" } else { "scancode" };
            let flag = key["flag"].as_u64().unwrap_or(0);
            let mut found = false;
            for mapping in mappings.iter_mut() {
                let input = &mapping["input"];
                let matched = if dkm.is_some() {
                    input["key"] == key["key"]
                } else {
                    input["hypershift"] != true
                        && input["scancode"] == key["scancode"]
                        && matches!(input["flag"].as_u64(), Some(value) if value == flag || value == flag + 1)
                };
                if matched {
                    mapping["input"]["snaptapId"] = json!(id);
                    found = true;
                }
            }
            if !found {
                for flag in [flag, flag + 1] {
                    let mut input = json!({"type":if dkm.is_some() {"razerKey"} else {"keyboard"},"flag":flag,"snaptapId":id});
                    input[field] = key[field].clone();
                    let mut output = json!({"type":"keyboard","flag":flag});
                    output[field] = key[field].clone();
                    mappings.push(json!({"input":input,"output":output}));
                }
            }
        }
    }
    profile["appEngine"]["hash"] = json!(source_hash(&profile["appEngine"])?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn profile() -> Value {
        let mut value = default_profile();
        value["guid"] = json!("profile");
        value["name"] = json!("Local");
        value
    }
    fn guids() -> impl Iterator<Item = String> {
        (1..257).map(|n| format!("00000000-0000-4000-8000-{n:012x}"))
    }
    #[test]
    fn default_fn_and_volume_make_release_match_current_factory() {
        let document = prepare_document(&[profile()], "profile", None, guids()).unwrap();
        let mappings = document["defaultMappings"]["appEngine"]["mappings"]
            .as_array()
            .unwrap();
        assert_eq!(
            mappings
                .iter()
                .filter(|m| m["input"]["type"] == "razerKey"
                    && m["input"]["key"] == 1
                    && m["output"]["type"] == "hypershift")
                .count(),
            4
        );
        // Current 515 MW2520 se returns keyboard make/release for ordinary
        // KeyInput volume. useTurbo does not turn this branch into a turbo.
        for (input_scancode, output_scancode) in [(60, 46), (61, 48)] {
            for (input_flag, output_flag) in [(0, 2), (1, 3)] {
                let entry = mappings
                    .iter()
                    .find(|mapping| {
                        mapping["input"]["scancode"] == input_scancode
                            && mapping["input"]["flag"] == input_flag
                            && mapping["input"]["hypershift"] == true
                    })
                    .unwrap();
                assert_eq!(
                    entry["output"],
                    json!({"type":"keyboard",
                    "scancode":output_scancode,"flag":output_flag})
                );
            }
        }
        assert_eq!(document["defaultTurbos"].as_array().unwrap().len(), 38);
        assert_eq!(document["reportIDs"]["4"], "razerKeyReportID");
    }
    #[test]
    fn ordinary_gaming_mode_filters_keys_and_retains_local_profile() {
        let mut p = profile();
        p["gamingMode"] = json!({"state":1,"isWindowsKeyDisabled":true,"isAltTabDisabled":true});
        let result = prepare_document(&[p.clone()], "profile", None, guids()).unwrap();
        assert_eq!(p["mappings"], json!([]));
        assert_eq!(result["profiles"][0]["appEngine"]["gamemode"], true);
        assert!(
            result["defaultMappings"]["appEngine"]["mappings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|m| m["input"]["scancode"] == 91 && m["output"]["type"] == "disabled")
        );
        assert_eq!(
            result["defaultMappings"]["appEngine"]["hash"],
            source_hash(&result["defaultMappings"]["appEngine"]).unwrap()
        );
    }
    #[test]
    fn game_observation_is_required_for_during_game_and_defaults_remain_stable() {
        let mut p = profile();
        p["gamingMode"]["state"] = json!(2);
        let previous = prepare_document(&[p.clone()], "profile", None, guids()).unwrap();
        assert!(
            previous["profiles"][0]["appEngine"]
                .get("gamemode")
                .is_none()
        );
        let next = prepare_document(&[p], "profile", Some(&previous), std::iter::empty()).unwrap();
        assert_eq!(previous["defaultTurbos"], next["defaultTurbos"]);
        let mut context = EngineContext {
            default_turbos: next["defaultTurbos"].as_array().cloned(),
            synapse_turbos: Some(vec![]),
            ..Default::default()
        };
        let active = compile(&next, &source_defaults(), &mut context, Some(1)).unwrap();
        assert_eq!(active["profiles"][0]["appEngine"]["gamemode"], true);
    }

    #[test]
    fn local_profile_save_preserves_an_observed_global_gaming_override() {
        let mut source = profile();
        source["gamingMode"]["state"] = json!(1);
        let mut previous = prepare_document(&[source.clone()], "profile", None, guids()).unwrap();
        previous["gameModeEnabled"] = json!(false);
        let saved =
            prepare_document(&[source], "profile", Some(&previous), std::iter::empty()).unwrap();
        assert_eq!(saved["gameModeEnabled"], false);
        assert!(saved["profiles"][0]["appEngine"].get("gamemode").is_none());
        assert!(!crate::keyboard_gaming_mapping::indicator_enabled(&saved, 515, Some(1)).unwrap());
    }
    #[test]
    fn snap_tap_tags_actual_scancodes_without_mutating_saved_mappings() {
        let mut p = profile();
        p["snapTap"]["isEnabled"] = json!(true);
        let document = prepare_document(&[p], "profile", None, guids()).unwrap();
        let active = &document["profiles"][0];
        assert_eq!(active["mappings"], json!([]));
        let mappings = active["appEngine"]["mappings"].as_array().unwrap();
        assert_eq!(mappings.len(), 4);
        assert!(mappings.iter().all(|m| m["input"]["snaptapId"] == 1));
        assert!(mappings.iter().any(|m| m["input"]["scancode"] == 30));
        assert!(mappings.iter().any(|m| m["input"]["scancode"] == 32));
    }
}
