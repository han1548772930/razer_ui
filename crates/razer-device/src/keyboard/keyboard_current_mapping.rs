//! Current PID679 Mt/ut -> storage hooks -> mapping_engine publication data.
//! Original MW main.2e96b673a06ae6ca25a1 module26061, 6737.d9169f212a8242741503
//! modules4753/89423/51765, and 2291.6b75539a8c608f561c1a factories.
//! Compilation is local; the service callback separately acknowledges publication.
use crate::{
    keyboard_gaming_mapping::{
        GamingMappingContext, GamingModeSettings, KeyboardGamingMappingPlan,
    },
    mapping_engine::{EngineContext, MappingEngine},
    mapping_engine_hash::source_hash,
};
use anyhow::{Context as _, ensure};
use serde_json::{Value, json};

pub const STORAGE_FIELD: &str = "_keyboard679_mapping_document";

fn source_defaults() -> Vec<Value> {
    let mut records = Vec::new();
    for hypershift in [false, true] {
        records.push(json!({"inputID":"DKM_KB_SIDE2","inputType":"DKMInput","isHyperShift":hypershift,
            "outputType":"win8ShortcutsGroup","win8ShortcutsGroup":{"windowsShortcutAssignment":"XboxGameBar"}}));
        for (key, action) in [
            ("DKM_KBMK_01", "PrevTrack"),
            ("DKM_KBMK_02", "Play"),
            ("DKM_KBMK_03", "NextTrack"),
            ("DKM_KBMK_04", "MuteVolume"),
        ] {
            records.push(
                json!({"inputID":key,"inputType":"DKMInput","isHyperShift":hypershift,
                "outputType":"multimediaGroup","multimediaGroup":{"multimediaAssignment":action}}),
            );
        }
    }
    for hypershift in [false, true] {
        for (input, action) in [("ScrollUp", "VolumeUp"), ("ScrollDown", "VolumeDown")] {
            records.push(
                json!({"inputID":input,"inputType":"MouseInput","isHyperShift":hypershift,
                "outputType":"multimediaGroup","multimediaGroup":{"multimediaAssignment":action}}),
            );
        }
    }
    records.push(
        json!({"inputID":"KEY_LEFT_SHIFT","inputType":"AnalogInput","isHyperShift":true,
        "outputType":"snapTapGroup","snapTapGroup":{"snapTapAssignment":"toggle"}}),
    );
    for (input, key) in [
        ("KEY_F6", "KEY_PRINT_SCREEN"),
        ("KEY_F7", "KEY_SCROLL_LOCK"),
    ] {
        let mut group = json!({"key":key});
        if input == "KEY_F6" {
            group["keyAssignment"] = json!("Prnt Scrn");
            group["modifiers"] = json!([]);
        }
        records.push(
            json!({"inputID":input,"inputType":"AnalogInput","isHyperShift":true,
            "outputType":"keyboardGroup","keyboardGroup":group}),
        );
    }
    records.push(
        json!({"inputID":"KEY_F8","inputType":"AnalogInput","isHyperShift":true,
        "outputType":"systemGroup","systemGroup":{"systemAssignment":"Sleep"}}),
    );
    for (input, output) in [("KEY_F9", "otfmGroup"), ("KEY_F10", "gameModeGroup")] {
        records.push(json!({"inputID":input,"inputType":"AnalogInput","isHyperShift":true,"outputType":output}));
    }
    for (input, action) in [("KEY_F11", "BrightnessDown"), ("KEY_F12", "BrightnessUp")] {
        records.push(
            json!({"inputID":input,"inputType":"AnalogInput","isHyperShift":true,
            "outputType":"backlightGroup","backlightGroup":{"backlightAssignment":action}}),
        );
    }
    for (input, guid) in [
        ("KEY_HOME", "af06d371-861f-4b98-8d78-bfaef5cfdebf"),
        ("KEY_INSERT", "af06d371-861f-4b98-8d78-bfaef5cfdebf"),
        ("KEY_PAGE_UP", "d363ba48-79a8-422c-b405-29a07eff7e8e"),
        ("KEY_DELETE", "2c481cef-62e0-4fe6-85a1-4d134c279c23"),
        ("KEY_END", "474c90e3-7eab-4aa0-8030-34c43cbc707a"),
        ("KEY_PAGE_DOWN", "177a20d5-f30a-4a52-923e-4fadb7db8392"),
    ] {
        records.push(json!({"inputID":input,"inputType":"AnalogInput","isHyperShift":true,
            "outputType":"profileNavigationGroup","profileNavigationGroup":{"profileNavigationAssignment":"Specific","guid":guid}}));
    }
    records
}

fn source_context(
    previous: Option<&Value>,
    guids: impl IntoIterator<Item = String>,
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
        fresh_guids: guids.into_iter().collect(),
        is_analog_gen2: true,
        analog_binary_range: Some(65535),
        // Current679 DeviceInfo has no AnalogGenVersion. JS passes undefined.
        analog_generation: None,
        ..Default::default()
    };
    for (input, key) in [
        ("DKM_KBMK_01", 84),
        ("DKM_KBMK_02", 85),
        ("DKM_KBMK_03", 83),
        ("DKM_KBMK_04", 82),
        ("DKM_KB_SIDE2", 100),
    ] {
        context
            .dkm_keys
            .insert(input.into(), json!({"inputID":input,"key":key}));
    }
    context
}

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
        .context("Keyboard source device settings are absent")?;
    crate::keyboard_current_profile::decode_device_settings(device_settings)?;
    let mut context = source_context(previous, fresh_guids);
    let normalized = profiles
        .iter()
        .map(|profile| -> anyhow::Result<Value> {
            crate::keyboard_current_profile::decode_profile(profile)?;
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
    let version = crate::mapping_submission::next_source_version(
        previous.and_then(|value| value["version"].as_u64()),
    )?;
    let mut document = previous.cloned().unwrap_or_else(|| json!({}));
    for field in [
        "analogData",
        "reportIDs",
        "selectedProfile",
        "gameModeEnabled",
    ] {
        if let Some(value) = device_settings.get(field) {
            document[field] = value.clone();
        } else {
            document
                .as_object_mut()
                .context("Previous keyboard document is not an object")?
                .remove(field);
        }
    }
    crate::keyboard_current_profile::decode_device_settings(&document)?;
    document["version"] = json!(version);
    document["profiles"] = json!(normalized);
    document["activeProfile"] = json!(active_profile);
    document["vendorId"] = json!(5426);
    document["productId"] = json!(679);
    document["category"] = json!("KEYBOARD");
    document["bluetoothLE"] = json!(false);
    document["_source_layout_id"] = json!(layout);
    compile_with_context(&document, &mut context, None)
}

/// Rebuild from the persisted source records on actual game observations.
pub fn compile(
    document: &Value,
    running_games: Option<usize>,
    fresh_guids: impl IntoIterator<Item = String>,
) -> anyhow::Result<Value> {
    let mut context = source_context(Some(document), fresh_guids);
    compile_with_context(document, &mut context, running_games)
}

fn compile_with_context(
    document: &Value,
    context: &mut EngineContext,
    running_games: Option<usize>,
) -> anyhow::Result<Value> {
    ensure!(
        document["productId"] == 679,
        "Current keyboard compiler product differs"
    );
    crate::keyboard_current_profile::decode_device_settings(document)?;
    let layout = u32::try_from(
        document["_source_layout_id"]
            .as_u64()
            .context("Keyboard layout is missing")?,
    )?;
    let factory = MappingEngine::new(679)?;
    let mut draft = context.clone();
    if draft.default_turbos.is_none() {
        factory.generate_default_turbos(&mut draft)?;
    }
    let mut defaults = factory.generate(&source_defaults(), &mut draft)?.app_engine;
    for hypershift in [false, true] {
        for flag in [0, 1] {
            defaults["mappings"].as_array_mut().unwrap().push(json!({
            "input":{"type":"razerKey","key":1,"hypershift":hypershift,"flag":flag},
            "output":{"type":"hypershift","flag":flag}}));
        }
    }
    defaults["hash"] = json!(source_hash(&defaults)?);
    let mut result = document.clone();
    let active = result["activeProfile"]
        .as_str()
        .context("Active keyboard profile is missing")?
        .to_owned();
    let profiles = result["profiles"]
        .as_array_mut()
        .context("Keyboard profiles are missing")?;
    ensure!(
        profiles
            .iter()
            .filter(|profile| profile["guid"] == active)
            .count()
            == 1,
        "Active keyboard profile is not unique"
    );
    for profile in profiles.iter_mut() {
        crate::keyboard_current_profile::decode_profile(profile)?;
        let replaced = profile["appEngine"]["mappings"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|mapping| mapping["output"]["type"] == "turbo")
            .filter_map(|mapping| mapping["output"]["guid"].as_str().map(str::to_owned))
            .collect::<std::collections::BTreeSet<_>>();
        draft.synapse_turbos.as_mut().unwrap().retain(|turbo| {
            !turbo["guid"]
                .as_str()
                .is_some_and(|guid| replaced.contains(guid))
        });
        let records = profile["mappings"]
            .as_array()
            .context("Keyboard mappings are missing")?;
        let uses_backslash = layout == 12 || [1, 2, 5, 8, 9, 11, 19, 21].contains(&layout);
        let records = records
            .iter()
            .filter(|record| record["outputType"] != "modTapGroup")
            .filter(|record| {
                record["inputID"]
                    != if uses_backslash {
                        "KEY_NON_US_POUND"
                    } else {
                        "KEY_BACKSLASH"
                    }
            })
            .cloned()
            .collect::<Vec<_>>();
        let (analog, ordinary): (Vec<_>, Vec<_>) = records
            .into_iter()
            .partition(|record| record["inputType"] == "AnalogInput");
        let mut engine = factory.generate(&ordinary, &mut draft)?.app_engine;
        if !analog.is_empty() {
            let analog = factory.generate_analog(&analog, &mut draft)?.app_engine;
            for group in analog["mappings"]
                .as_array()
                .context("Analog mapping list is missing")?
            {
                engine["mappings"].as_array_mut().unwrap().extend(
                    group
                        .as_array()
                        .context("Analog mapping group is missing")?
                        .iter()
                        .cloned(),
                );
            }
            if analog["actuations"]
                .as_array()
                .is_some_and(|items| !items.is_empty())
            {
                engine["actuations"] = analog["actuations"].clone();
            }
            if analog["rapidTriggers"]
                .as_object()
                .is_some_and(|items| !items.is_empty())
            {
                engine["rapidTriggers"] = analog["rapidTriggers"].clone();
            }
        }
        engine["hash"] = json!(source_hash(&engine)?);
        profile["appEngine"] = engine;
    }
    let active_index = profiles
        .iter()
        .position(|profile| profile["guid"] == active)
        .unwrap();
    let mut profile = profiles.remove(active_index);
    let settings = &profile["gamingMode"];
    let state = settings["state"]
        .as_u64()
        .context("Gaming mode state is missing")?;
    ensure!(state <= 2, "Gaming mode state is invalid");
    let gaming = GamingModeSettings {
        state: state as u8,
        is_windows_key_disabled: settings["isWindowsKeyDisabled"] == true,
        is_alt_tab_disabled: settings["isAltTabDisabled"] == true,
        is_alt_f4_disabled: settings["isAltF4Disabled"] == true,
        is_alt_space_disabled: settings["isAltSpaceDisabled"] == true,
        is_alt_enter_disabled: settings["isAltEnterDisabled"] == true,
        ..Default::default()
    };
    let plan = KeyboardGamingMappingPlan::prepare(
        679,
        &profile["appEngine"],
        &defaults,
        gaming,
        GamingMappingContext {
            running_games,
            game_mode_enabled: document.get("gameModeEnabled").and_then(Value::as_bool),
            ..Default::default()
        },
        || Ok(()),
    )?;
    profile["appEngine"] = plan.active_engine().clone();
    apply_snap_tap(&factory, &mut profile, plan.default_engine(), &draft)?;
    profiles.push(profile);
    result["defaultMappings"] = json!({"appEngine":plan.default_engine()});
    result["defaultTurbos"] = json!(draft.default_turbos);
    result["synapseTurbos"] = json!(draft.synapse_turbos);
    *context = draft;
    Ok(result)
}

fn apply_snap_tap(
    factory: &MappingEngine,
    profile: &mut Value,
    defaults: &Value,
    context: &EngineContext,
) -> anyhow::Result<()> {
    if profile["snapTap"]["isEnabled"] != true {
        return Ok(());
    }
    let pairs = profile["snapTap"]["keyList"]
        .as_array()
        .context("Snap Tap pairs are missing")?
        .clone();
    let multi = profile["functionKeyPrimary"] == "multi";
    let mappings = profile["appEngine"]["mappings"].as_array_mut().unwrap();
    for pair in pairs {
        let id = pair["id"]
            .as_u64()
            .context("Snap Tap pair identity is missing")?;
        for field in ["key1", "key2"] {
            let name = pair[field]
                .as_str()
                .context("Snap Tap key identity is missing")?;
            let dkm = context.dkm_keys.get(name);
            let key = dkm
                .cloned()
                .map(Ok)
                .unwrap_or_else(|| factory.source_key_metadata(name))?;
            let flag = key["flag"].as_u64().unwrap_or(0);
            let matches = |mapping: &Value| {
                let input = &mapping["input"];
                (input["scancode"].as_u64().is_some_and(|value| value != 0)
                    && input["hypershift"] != true
                    && input["scancode"] == key["scancode"]
                    && matches!(input["flag"].as_u64(),Some(value) if value == flag || value == flag+1))
                    || (key.get("key").is_some() && input["key"] == key["key"])
            };
            let found = mappings.iter().any(matches);
            let mut copied_default = false;
            if multi {
                for mapping in defaults["mappings"]
                    .as_array()
                    .context("Default mappings are missing")?
                {
                    let input = &mapping["input"];
                    if input["scancode"] == key["scancode"]
                        && matches!(input["flag"].as_u64(),Some(value) if value == flag || value == flag+1)
                    {
                        copied_default = true;
                        if !mappings.iter().any(|current| {
                            ["type", "scancode", "flag", "hypershift"]
                                .iter()
                                .all(|field| current["input"][field] == input[field])
                        }) {
                            mappings.push(mapping.clone());
                        }
                    }
                }
            }
            if found || multi {
                for mapping in mappings.iter_mut().filter(|mapping| matches(mapping)) {
                    mapping["input"]["snaptapId"] = json!(id);
                }
            }
            if !found && (!multi || !copied_default) {
                let key_field = if dkm.is_some() { "key" } else { "scancode" };
                for flag in [flag, flag + 1] {
                    let mut input = json!({"type":if dkm.is_some(){"razerKey"}else{"analogKey"},"flag":flag,"snaptapId":id});
                    input[key_field] = key[key_field].clone();
                    let mut output = json!({"type":"keyboard","flag":flag});
                    output[key_field] = key[key_field].clone();
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
    fn guids() -> impl Iterator<Item = String> {
        (0..256).map(|value| format!("00000000-0000-4000-8000-{value:012x}"))
    }
    fn profile() -> Value {
        let mut value =
            serde_json::to_value(crate::keyboard_current_profile::initial_profile()).unwrap();
        value["guid"] = json!("profile");
        value
    }

    fn prepare(
        profiles: &[Value],
        active: &str,
        layout: u32,
        previous: Option<&Value>,
        guids: impl IntoIterator<Item = String>,
    ) -> anyhow::Result<Value> {
        let settings =
            serde_json::to_value(crate::keyboard_current_profile::initial_device_settings())
                .unwrap();
        prepare_document_with_device_settings(
            profiles,
            active,
            layout,
            previous,
            Some(&settings),
            guids,
        )
    }
    #[test]
    fn source_defaults_keep_dkm_analog_and_scroll_inputs() {
        let document = prepare(&[profile()], "profile", 1, None, guids()).unwrap();
        assert_eq!(source_defaults().len(), 28);
        let mappings = document["defaultMappings"]["appEngine"]["mappings"]
            .as_array()
            .unwrap();
        assert!(
            mappings
                .iter()
                .any(|mapping| mapping["input"]["type"] == "analogKey"
                    && mapping["output"]["type"] == "snapTap")
        );
        assert!(mappings.iter().any(
            |mapping| mapping["input"]["type"] == "razerKey" && mapping["input"]["key"] == 100
        ));
        assert!(
            mappings
                .iter()
                .any(|mapping| mapping["input"]["type"] == "mouse")
        );
        assert_eq!(document["reportIDs"]["11"], "analogKeyboardV2ReportID");
        assert_eq!(document["analogData"]["defaultMakeActuation"], 19656);
        assert_eq!(
            document["defaultMappings"]["appEngine"]["hash"],
            source_hash(&document["defaultMappings"]["appEngine"]).unwrap()
        );
    }
    #[test]
    fn snap_tap_publishes_analog_tags_then_removes_them_from_rebuilt_engine() {
        let mut source = profile();
        source["snapTap"]["isEnabled"] = json!(true);
        let first = prepare(&[source.clone()], "profile", 1, None, guids()).unwrap();
        let mappings = first["profiles"][0]["appEngine"]["mappings"]
            .as_array()
            .unwrap();
        assert_eq!(mappings.len(), 4);
        assert!(
            mappings
                .iter()
                .all(|mapping| mapping["input"]["type"] == "analogKey"
                    && mapping["input"]["snaptapId"] == 1)
        );
        assert!(
            mappings
                .iter()
                .any(|mapping| mapping["input"]["scancode"] == 30)
        );
        assert!(
            mappings
                .iter()
                .any(|mapping| mapping["input"]["scancode"] == 32)
        );
        source["snapTap"]["isEnabled"] = json!(false);
        let second = prepare(&[source], "profile", 1, Some(&first), guids()).unwrap();
        assert_eq!(second["profiles"][0]["appEngine"]["mappings"], json!([]));
        assert_eq!(second["defaultTurbos"], first["defaultTurbos"]);
    }
    #[test]
    fn numeric_key_points_survive_analog_compilation_and_layout_filter() {
        let mut source = profile();
        source["mappings"] = json!([{"inputID":"KEY_A","inputType":"AnalogInput","mapping":[
            {"outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_B"},"actuationPoint":{"0":19656,"1":256}},
            {"outputType":"defaultGroup","actuationPoint":{"0":65520,"1":256}}]},
            {"inputID":"KEY_BACKSLASH","inputType":"KeyInput","outputType":"disableGroup"},
            {"inputID":"KEY_NON_US_POUND","inputType":"KeyInput","outputType":"disableGroup"}]);
        let document = prepare(&[source], "profile", 3, None, guids()).unwrap();
        let engine = &document["profiles"][0]["appEngine"];
        assert_eq!(
            engine["actuations"][0],
            json!({"keyId":31,"make":19656,"break":256})
        );
        assert!(
            engine["mappings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|mapping| mapping["input"]["type"] == "analogKey"
                    && mapping["output"]["scancode"] == 48)
        );
        assert_eq!(
            document["profiles"][0]["mappings"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
    }
    #[test]
    fn game_mode_and_snap_tags_are_regenerated_from_saved_records() {
        let mut source = profile();
        source["gamingMode"]["state"] = json!(2);
        source["gamingMode"]["isWindowsKeyDisabled"] = json!(true);
        let first = prepare(&[source], "profile", 1, None, guids()).unwrap();
        assert!(first["profiles"][0]["appEngine"].get("gamemode").is_none());
        let running = compile(&first, Some(1), guids()).unwrap();
        assert_eq!(running["profiles"][0]["appEngine"]["gamemode"], true);
        let stopped = compile(&running, Some(0), guids()).unwrap();
        assert!(
            stopped["profiles"][0]["appEngine"]
                .get("gamemode")
                .is_none()
        );
        assert_eq!(stopped["profiles"][0]["gamingMode"]["state"], 2);
    }

    #[test]
    fn compiler_requires_complete_profiles_and_explicit_device_settings() {
        assert!(prepare_document(&[profile()], "profile", 1, None, guids()).is_err());
        for field in [
            "gamingMode",
            "snapTap",
            "mappings",
            "actuationPoint",
            "brightness",
        ] {
            let mut incomplete = profile();
            incomplete.as_object_mut().unwrap().remove(field);
            assert!(
                prepare(&[incomplete], "profile", 1, None, guids()).is_err(),
                "{field}"
            );
        }
        let settings =
            serde_json::to_value(crate::keyboard_current_profile::initial_device_settings())
                .unwrap();
        let first = prepare(&[profile()], "profile", 1, None, guids()).unwrap();
        let mut changed = settings;
        changed["analogData"]["defaultMakeActuation"] = json!(29484);
        changed["analogData"]["defaultBreakActuation"] = json!(256);
        let rebuilt = prepare_document_with_device_settings(
            &[profile()],
            "profile",
            1,
            Some(&first),
            Some(&changed),
            guids(),
        )
        .unwrap();
        assert_eq!(rebuilt["analogData"], changed["analogData"]);
        changed.as_object_mut().unwrap().remove("analogData");
        assert!(
            prepare_document_with_device_settings(
                &[profile()],
                "profile",
                1,
                Some(&first),
                Some(&changed),
                guids(),
            )
            .is_err()
        );
    }
}
