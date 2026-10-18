//! Current740/746 profile records, firmware-filtered engine compilation and
//! source global/per-key analog task preparation. Publication and HID ACKs
//! belong to their retained transport owners.
use crate::{
    mapping_engine::{EngineContext, MappingEngine},
    mapping_engine_hash::source_hash,
};
use anyhow::{Context as _, ensure};
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[path = "keyboard_magnetic_controls.rs"]
pub(crate) mod controls;
#[path = "keyboard_magnetic_obm.rs"]
mod obm;
#[path = "keyboard_magnetic_source.rs"]
mod source;
#[path = "keyboard_magnetic_sync.rs"]
mod sync;
pub use crate::onboard_assignment::ButtonAssignment;
pub use controls::{EnginePreparation, InterruptSettings, InterruptionKey};
pub use obm::{MacroPlayback, PreparedAssignment};
pub use sync::{global_actuation, global_rapid_trigger, per_key_changes};

pub fn prepare_assignments(
    product: u32,
    layout: u32,
    profile: &Value,
    old: &Value,
) -> anyhow::Result<Vec<PreparedAssignment>> {
    obm::prepare(product, layout, profile, old)
}
pub fn normalize_mappings(
    product: u32,
    layout: u32,
    profile: &Value,
    old: &Value,
    rows: &[Value],
) -> anyhow::Result<Vec<Value>> {
    obm::normalize(product, layout, profile, old, rows)
}

pub const STORAGE_FIELD: &str = "_keyboard_magnetic_mapping_document";

pub fn supports(product: u32) -> bool {
    matches!(product, 740 | 746)
}

pub(crate) fn guest_profile_owner() -> &'static str {
    source::GUEST_PROFILE_OWNER
}

pub(crate) fn is_onboard_preset(guid: &str) -> bool {
    source::is_onboard_preset(guid)
}

pub(crate) fn macro_scancode(hid: u16) -> Option<u32> {
    source::macro_scancode(hid)
}

pub(crate) fn bottom_deadzone(value: &str) -> u8 {
    source::bottom_deadzone(value)
}

pub(super) fn removed_mappings(current: &Value, old: &Value) -> anyhow::Result<Vec<Value>> {
    let current = current["mappings"]
        .as_array()
        .context("Current magnetic mappings are absent")?;
    let old = old["mappings"]
        .as_array()
        .context("Previous magnetic mappings are absent")?;
    Ok(old
        .iter()
        .filter(|old| {
            !current.iter().any(|new| {
                old["inputID"] == new["inputID"]
                    && (old["isHyperShift"] == true) == (new["isHyperShift"] == true)
                    && if old["inputType"] == "AnalogInput" && new["inputType"] == "AnalogInput" {
                        old["mapping"] == new["mapping"]
                    } else {
                        old["outputType"] == new["outputType"]
                            && old["outputType"]
                                .as_str()
                                .is_some_and(|kind| old[kind] == new[kind])
                    }
            })
        })
        .cloned()
        .collect())
}

pub fn default_profile(product: u32) -> anyhow::Result<Value> {
    ensure!(supports(product), "Magnetic profile product differs");
    Ok(source::default_profile())
}

fn context(previous: Option<&Value>, guids: impl IntoIterator<Item = String>) -> EngineContext {
    EngineContext {
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
        ..Default::default()
    }
}

pub fn prepare_document(
    product: u32,
    profiles: &[Value],
    active_profile: &str,
    layout: u32,
    previous: Option<&Value>,
    fresh_guids: impl IntoIterator<Item = String>,
) -> anyhow::Result<Value> {
    ensure!(supports(product), "Magnetic mapping product differs");
    let normalized = profiles
        .iter()
        .map(|profile| -> anyhow::Result<Value> {
            let mut value = source::default_profile();
            value.as_object_mut().unwrap().extend(
                profile
                    .as_object()
                    .context("Magnetic profile is not an object")?
                    .clone(),
            );
            ensure!(
                value["guid"].as_str().is_some_and(|guid| !guid.is_empty()),
                "Magnetic profile GUID is absent"
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
    let mut document=previous.cloned().unwrap_or_else(||json!({"version":1,"vendorId":5426,"productId":product,"category":"KEYBOARD",
        "analogData":{"defaultMakeActuation":19656,"defaultBreakActuation":1638},"turnOffAnalogToKey":true,"gameModeEnabled":false,
        "reportIDs":{"11":"analogKeyboardV2ReportID","4":"razerKeyReportID"},"selectedProfile":{}}));
    document["version"] = json!(crate::mapping_submission::next_source_version(
        previous.and_then(|document| document["version"].as_u64())
    )?);
    document["profiles"] = json!(normalized);
    document["activeProfile"] = json!(active_profile);
    document["productId"] = json!(product);
    document["_source_layout_id"] = json!(layout);
    let mut context = context(previous, fresh_guids);
    compile_with_context(&document, None, &mut context)
}

pub fn compile(
    document: &Value,
    running_games: Option<usize>,
    fresh_guids: impl IntoIterator<Item = String>,
) -> anyhow::Result<Value> {
    let mut context = context(Some(document), fresh_guids);
    compile_with_context(document, running_games, &mut context)
}

fn compile_with_context(
    document: &Value,
    running_games: Option<usize>,
    context: &mut EngineContext,
) -> anyhow::Result<Value> {
    let product = u32::try_from(
        document["productId"]
            .as_u64()
            .context("Magnetic document product is absent")?,
    )?;
    ensure!(supports(product), "Magnetic compiler product differs");
    let layout = u32::try_from(
        document["_source_layout_id"]
            .as_u64()
            .context("Magnetic keyboard layout is absent")?,
    )?;
    let active = document["activeProfile"]
        .as_str()
        .context("Magnetic active profile is absent")?;
    let mut draft = context.clone();
    let factory = MappingEngine::new(product)?;
    if draft.default_turbos.is_none() {
        factory.generate_default_turbos(&mut draft)?;
    }
    let defaults = source::default_mappings(product, layout)?
        .iter()
        .map(|row| row.record())
        .collect::<Vec<_>>();
    let mut defaults = factory.generate(&defaults, &mut draft)?.app_engine;
    for hypershift in [false, true] {
        for flag in [0, 1] {
            defaults["mappings"].as_array_mut().unwrap().push(json!({
        "input":{"type":"razerKey","key":1,"hypershift":hypershift,"flag":flag},"output":{"type":"hypershift","flag":flag}}));
        }
    }
    defaults["hash"] = json!(source_hash(&defaults)?);
    let mut result = document.clone();
    let profiles = result["profiles"]
        .as_array_mut()
        .context("Magnetic profiles are absent")?;
    ensure!(
        profiles
            .iter()
            .filter(|profile| profile["guid"] == active)
            .count()
            == 1,
        "Magnetic active profile is not unique"
    );
    let mut identities = BTreeSet::new();
    for profile in profiles.iter_mut() {
        let guid = profile["guid"]
            .as_str()
            .context("Magnetic profile GUID is invalid")?;
        ensure!(
            identities.insert(guid.to_owned()),
            "Magnetic profile GUID is repeated"
        );
        let replaced = profile["appEngine"]["mappings"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|row| row["output"]["type"] == "turbo")
            .filter_map(|row| row["output"]["guid"].as_str().map(str::to_owned))
            .collect::<BTreeSet<_>>();
        draft.synapse_turbos.as_mut().unwrap().retain(|turbo| {
            !turbo["guid"]
                .as_str()
                .is_some_and(|guid| replaced.contains(guid))
        });
        let rows = profile["mappings"]
            .as_array()
            .context("Magnetic mapping records are absent")?;
        let preparation = controls::preparation(profile)?;
        let runtime = if guid == active && preparation == EnginePreparation::Gamepad {
            crate::keyboard_firmware_mapping::gamepad_rows(
                profile,
                rows,
                &source::full_mapping_list(product)?,
            )?
        } else {
            None
        };
        let generic = runtime.is_some();
        let rows = if let Some(runtime) = runtime {
            runtime
        } else {
            crate::keyboard_firmware_mapping::firmware_mappings(rows)
        };
        let rows = crate::keyboard_firmware_mapping::normalize_engine_rows(&rows)?;
        let rows = controls::engine_rows(profile, &rows, preparation)?;
        let uses_backslash = layout == 12 || [1, 2, 5, 8, 9, 11, 19, 20, 21].contains(&layout);
        let (analog, ordinary): (Vec<_>, Vec<_>) = rows
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
            .partition(|row| row["inputType"] == "AnalogInput");
        let mut engine = factory.generate(&ordinary, &mut draft)?.app_engine;
        if !analog.is_empty() {
            let generated = factory.generate_analog(&analog, &mut draft)?.app_engine;
            for group in generated["mappings"]
                .as_array()
                .context("Magnetic analog mappings are absent")?
            {
                for row in group
                    .as_array()
                    .context("Magnetic analog group is invalid")?
                {
                    if row["input"]["type"] == "analogKey"
                        && row["output"]["type"] == "keyboard"
                        && row["input"]["scancode"] == row["output"]["scancode"]
                        && row["input"]["hypershift"] != true
                    {
                        continue;
                    }
                    engine["mappings"].as_array_mut().unwrap().push(row.clone());
                }
            }
            for field in ["actuations", "rapidTriggers"] {
                if generic && field == "rapidTriggers" {
                    continue;
                }
                if generated[field]
                    .as_array()
                    .is_some_and(|items| !items.is_empty())
                    || generated[field]
                        .as_object()
                        .is_some_and(|items| !items.is_empty())
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
    let mut result =
        crate::keyboard_firmware_gaming::apply(&result, running_games, true, &mut draft)?;
    let index = result["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .position(|profile| profile["guid"] == active)
        .unwrap();
    let defaults = result["defaultMappings"]["appEngine"].clone();
    crate::keyboard_firmware_mapping::snap_tap(
        &factory,
        &mut result["profiles"][index],
        &defaults,
        &draft,
    )?;
    *context = draft;
    Ok(result)
}

#[cfg(test)]
#[path = "keyboard_magnetic_mapping_tests.rs"]
mod tests;
