//! PID70 current MW main.4d2dda8011e526ce4d9b (profile/DKM/defaults)
//! and 6120.7fff31f083f844383e26 (84210 initialization, 91105 je).
//! Source documents and native engine publication remain separate operations.
use crate::{
    mapping_engine::{EngineContext, MappingEngine},
    mapping_submission::next_source_version,
};
use anyhow::{Context as _, Result, ensure};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::LazyLock;

#[path = "mouse_legacy_mapping_data_schema.rs"]
mod source_data;
use source_data::*;

static SOURCE: LazyLock<LegacyMouseSource> = LazyLock::new(|| make_mouse_legacy_mapping_table());

pub(crate) fn source_profile_defaults() -> Result<Value> {
    Ok(serde_json::to_value(&SOURCE.profile)?)
}

pub(crate) fn source_dkm_keys() -> Result<std::collections::BTreeMap<String, Value>> {
    SOURCE
        .dkm_keys
        .iter()
        .map(|key| Ok((key.input_id.clone(), serde_json::to_value(key)?)))
        .collect()
}

#[derive(Deserialize)]
struct ProfileIdentity {
    guid: String,
    name: String,
}

#[derive(Deserialize)]
struct DocumentVersion {
    #[serde(default, deserialize_with = "present_version")]
    version: Option<u64>,
}

fn present_version<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<u64>, D::Error> {
    u64::deserialize(deserializer).map(Some)
}

fn default_engine(engine: &MappingEngine, context: &mut EngineContext) -> Result<Value> {
    if context.default_turbos.as_ref().is_none_or(Vec::is_empty) {
        engine.generate_default_turbos(context)?;
    }
    let mappings = SOURCE
        .default_mappings
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(engine.generate(&mappings, context)?.app_engine)
}

fn compile_profile(
    source: &Value,
    engine: &MappingEngine,
    context: &mut EngineContext,
) -> Result<Value> {
    let mut profile = normalize_profile(source)?;
    let mut mappings = profile["mappings"].as_array().unwrap().clone();
    // je -> St appends native OTFS scroll interception only when this
    // profile contains an on-the-fly binding. These are engine inputs;
    // the user mapping list itself remains unchanged.
    if mappings
        .iter()
        .any(|mapping| mapping["sensitivityGroup"]["sensitivityAssignment"] == "DPI_OnTheFly")
    {
        mappings.extend(["ScrollUp", "ScrollDown"].map(|input| {
            json!({
                "outputType":"sensitivityGroup","inputType":"MouseInput","inputID":input,
                "sensitivityGroup":{"sensitivityAssignment":"OTFS_Scroll"}
            })
        }));
    }
    profile["appEngine"] = engine.generate(&mappings, context)?.app_engine;
    Ok(profile)
}

fn assemble_document(
    profiles: Vec<Value>,
    active_guid: Option<&str>,
    previous: Option<&Value>,
    default_engine: Value,
    context: &EngineContext,
) -> Result<Value> {
    if let Some(active_guid) = active_guid {
        ensure!(
            profiles
                .iter()
                .filter(|profile| profile.get("guid").and_then(Value::as_str) == Some(active_guid))
                .count()
                == 1,
            "Active mouse profile is not unique"
        );
    }
    let mut document = previous.cloned().unwrap_or_else(|| json!({}));
    ensure!(document.is_object(), "Invalid prior mouse document");
    let version: DocumentVersion = serde_json::from_value(document.clone())
        .context("Current mouse mapping document version is invalid")?;
    document["version"] = json!(next_source_version(version.version)?);
    document["productId"] = json!(70);
    document["vendorId"] = json!(5426);
    document["category"] = json!("MOUSE");
    document["reportIDs"] = json!({"4":"razerKeyReportID","5":"hardwareEventReportID"});
    document["profiles"] = json!(profiles);
    if let Some(active_guid) = active_guid {
        document["activeProfile"] = json!(active_guid);
    }
    document["defaultMappings"] = json!({"appEngine":default_engine});
    document["defaultTurbos"] = json!(
        context
            .default_turbos
            .as_ref()
            .context("Missing mouse default turbos")?
    );
    document
        .as_object_mut()
        .unwrap()
        .remove("synapseDefaultTurbos");
    document["synapseTurbos"] = json!(
        context
            .synapse_turbos
            .as_ref()
            .context("Missing mouse profile turbos")?
    );
    Ok(document)
}

/// Append the import processor's completed profiles to the retained engine
/// projection. Old source profiles and their compiled projection have distinct
/// representations; their identities and selection must still agree.
/// Existing macros/phasedMacros belong to the prior native publication
/// projection and remain intact. The import's separate macro-library candidate
/// and this local document do not acknowledge a native publication.
pub(crate) fn append_imported_profiles(
    existing_profiles: &[Value],
    imported_profiles: &[Value],
    selected_guid: Option<&str>,
    previous: Option<&Value>,
    context: &mut EngineContext,
) -> Result<Value> {
    if let Some(selected_guid) = selected_guid {
        ensure!(
            existing_profiles
                .iter()
                .filter(|profile| profile.get("guid").and_then(Value::as_str) == Some(selected_guid))
                .count() == 1,
            "Selected mouse source profile is not unique"
        );
    }
    let engine = MappingEngine::new(70)?;
    let defaults = match previous {
        Some(document) => document
            .get("defaultMappings")
            .and_then(|mappings| mappings.get("appEngine"))
            .filter(|engine| engine.is_object())
            .cloned()
            .context("Current mouse default mapping projection is absent")?,
        None => default_engine(&engine, context)?,
    };
    let (mut profiles, active_guid) = if let Some(previous) = previous {
        let profiles = previous
            .get("profiles")
            .and_then(Value::as_array)
            .context("Current mouse mapping profile collection is absent")?;
        ensure!(
            profiles.len() == existing_profiles.len(),
            "Current mouse mapping collection is stale"
        );
        for (compiled, source) in profiles.iter().zip(existing_profiles) {
            let compiled: ProfileIdentity = serde_json::from_value(compiled.clone())
                .context("Current mouse mapping profile identity is invalid")?;
            let source: ProfileIdentity = serde_json::from_value(source.clone())
                .context("Current mouse source profile identity is invalid")?;
            ensure!(
                compiled.guid == source.guid && compiled.name == source.name,
                "Current mouse mapping profile identity is stale"
            );
        }
        let active = previous
            .get("activeProfile")
            .map(|value| {
                value
                    .as_str()
                    .context("Current mouse mapping selection is invalid")
            })
            .transpose()?;
        ensure!(
            existing_profiles.is_empty() || active.is_some(),
            "Current mouse mapping selection is absent"
        );
        if let Some(selected_guid) = selected_guid {
            ensure!(
                active == Some(selected_guid),
                "Current mouse mapping selection is stale"
            );
        }
        if let Some(active) = active {
            ensure!(
                profiles
                    .iter()
                    .filter(|profile| profile.get("guid").and_then(Value::as_str) == Some(active))
                    .count()
                    == 1,
                "Current mouse mapping selection is not unique"
            );
        }
        (profiles.clone(), active)
    } else {
        let profiles = existing_profiles
            .iter()
            .map(|source| compile_profile(source, &engine, context))
            .collect::<Result<Vec<_>>>()?;
        (profiles, selected_guid)
    };
    profiles.extend_from_slice(imported_profiles);
    assemble_document(profiles, active_guid, previous, defaults, context)
}

/// Convert the UI's retained sensitivity field names to the middleware's
/// normalized profile. Active is already one-based in the current UI adapter.
pub fn normalize_profile(source: &Value) -> Result<Value> {
    let mut profile = serde_json::to_value(&SOURCE.profile)?;
    let fields = source
        .as_object()
        .context("Mouse source profile is not an object")?;
    for (key, value) in fields {
        if !key.starts_with('_')
            && !matches!(
                key.as_str(),
                "DPI"
                    | "DPIStages"
                    | "PollingRate"
                    | "PowerSavingValue"
                    | "LowBatteryEffects"
                    | "appEngine"
            )
        {
            profile[key] = value.clone();
        }
    }
    if let Some(dpi) = source.get("DPI") {
        profile["dpi"] = json!({"x":dpi["X"],"y":dpi["Y"]});
    }
    if let Some(stages) = source.get("DPIStages") {
        let rows = stages["Stages"]["DPIStage"]
            .as_array()
            .context("Missing mouse sensitivity stages")?;
        let rows: Vec<_> = rows.iter().map(|row| json!({
            "x":row["X"], "y":row["Y"], "independent":row["Independent"], "visible":row["Active"]
        })).collect();
        profile["dpiStages"] = json!({
            "count":rows.iter().filter(|row| row["visible"] == true).count(),
            "stages":rows, "active":stages["Active"], "enable":stages["State"]
        });
    }
    if let Some(polling) = source.get("PollingRate") {
        profile["pollingRate"] = polling.clone();
    }
    ensure!(
        profile["guid"]
            .as_str()
            .is_some_and(|guid| !guid.is_empty()),
        "Missing profile GUID"
    );
    ensure!(profile["name"].is_string(), "Missing profile name");
    ensure!(profile["mappings"].is_array(), "Missing profile mappings");
    Ok(profile)
}

/// Prepare a complete document from local profile values. The caller supplies
/// UUIDs once and persists this result before publishing to the native service.
/// Existing turbo IDs survive profile switches, renames and process restarts.
pub fn prepare_document(
    profiles: &[Value],
    active_guid: &str,
    previous: Option<&Value>,
    fresh_guids: impl IntoIterator<Item = String>,
) -> Result<Value> {
    ensure!(!profiles.is_empty(), "No mouse profiles");
    let engine = MappingEngine::new(70)?;
    let mut context = EngineContext {
        default_turbos: previous
            .and_then(|doc| {
                doc["defaultTurbos"]
                    .as_array()
                    .or_else(|| doc["synapseDefaultTurbos"].as_array())
            })
            .cloned(),
        synapse_turbos: Some(
            previous
                .and_then(|doc| doc["synapseTurbos"].as_array())
                .cloned()
                .unwrap_or_default(),
        ),
        fresh_guids: fresh_guids.into_iter().collect(),
        ..Default::default()
    };
    for key in &SOURCE.dkm_keys {
        context
            .dkm_keys
            .insert(key.input_id.clone(), serde_json::to_value(key)?);
    }
    let defaults = default_engine(&engine, &mut context)?;
    let mut normalized = Vec::with_capacity(profiles.len());
    for source in profiles {
        normalized.push(compile_profile(source, &engine, &mut context)?);
    }
    assemble_document(normalized, Some(active_guid), previous, defaults, &context)
}

/// Current MW 91105 It/Tt: Inter-Device OTFS temporarily publishes only
/// sensitivity assignments plus ordinary-layer scroll DPI controls. The saved
/// profile mappings and source version remain unchanged; release republishes
/// the latest ordinary document, including its OTFS_Scroll interception/hash.
pub fn interdevice_otfs_document(document: &Value) -> Result<Value> {
    ensure!(document["productId"] == 70, "OTFS target is not PID70");
    let profiles = document["profiles"]
        .as_array()
        .context("Missing profiles")?;
    let active = profiles
        .iter()
        .position(|profile| profile["guid"] == document["activeProfile"])
        .context("Missing active OTFS profile")?;
    let source = profiles[active]["mappings"]
        .as_array()
        .context("Missing active mappings")?;
    let mut context = EngineContext {
        default_turbos: document["defaultTurbos"].as_array().cloned(),
        synapse_turbos: document["synapseTurbos"].as_array().cloned(),
        ..Default::default()
    };
    for key in &SOURCE.dkm_keys {
        context
            .dkm_keys
            .insert(key.input_id.clone(), serde_json::to_value(key)?);
    }
    let engine = MappingEngine::new(70)?;
    let defaults = SOURCE
        .default_mappings
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<_>, _>>()?;
    let projection = engine.generate_mouse_otfs(source, &defaults, &mut context)?;
    let mut result = document.clone();
    result["profiles"][active]["appEngine"] = projection.profile;
    result["profiles"][active]["defaultMappings"] = projection.defaults.clone();
    result["defaultMappings"]["appEngine"] = projection.defaults;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{SOURCE, interdevice_otfs_document, prepare_document};
    use serde_json::json;

    #[test]
    fn typed_mouse_source_preserves_every_interchange_field_and_numeric_kind() {
        let mut value = serde_json::to_value(&*SOURCE).unwrap();
        value.sort_all_objects();
        let encoded = serde_json::to_vec(&value).unwrap();
        assert_eq!(
            format!("{:x}", md5::compute(encoded)),
            "2420815508ff2cb512b6c1371602b638"
        );
    }

    #[test]
    fn interdevice_otfs_preserves_saved_mappings_and_intercepts_scroll() {
        let mut profile = serde_json::to_value(&SOURCE.profile).unwrap();
        profile["guid"] = json!("otfs-test-profile");
        profile["name"] = json!("Local profile");
        profile["mappings"] = json!([
            {"inputID":"Button4","inputType":"MouseInput","outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_A","modifiers":[]}},
            {"inputID":"Button5","inputType":"MouseInput","outputType":"sensitivityGroup","sensitivityGroup":{"sensitivityAssignment":"DPI_OnTheFly"}}
        ]);
        let ordinary = prepare_document(
            &[profile],
            "otfs-test-profile",
            None,
            (0..128).map(|n| format!("00000000-0000-4000-8000-{n:012}")),
        )
        .unwrap();
        let temporary = interdevice_otfs_document(&ordinary).unwrap();
        assert_eq!(temporary["version"], ordinary["version"]);
        assert_eq!(
            temporary["profiles"][0]["mappings"],
            ordinary["profiles"][0]["mappings"]
        );
        let mappings = temporary["profiles"][0]["appEngine"]["mappings"]
            .as_array()
            .unwrap();
        assert!(
            mappings
                .iter()
                .all(|mapping| mapping["input"]["otfs"] != true)
        );
        let encoded = serde_json::to_string(mappings).unwrap();
        assert!(encoded.contains("DPI_Value_Up"));
        assert!(encoded.contains("DPI_Value_Down"));
        assert_ne!(
            temporary["profiles"][0]["appEngine"]["hash"],
            ordinary["profiles"][0]["appEngine"]["hash"]
        );
        assert!(
            ordinary["profiles"][0]["appEngine"]["mappings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|mapping| mapping["input"]["otfs"] == true)
        );
    }
}

// Generated typed table: razer-device/mouse_legacy_mapping_data.
#[rustfmt::skip]
fn make_mouse_legacy_mapping_table() -> LegacyMouseSource {
    LegacyMouseSource{
        default_mappings: vec![
            LegacyMouseSourceDefaultmappingsitem{
                input_id: "LeftClick".into(),
                input_type: "MouseInput".into(),
                mouse_input: Some("LeftClick".into()),
                mouse_group: Some(
                    LegacyMouseSourceDefaultmappingsitemMousegroup{
                        mouse_assignment: "Click".into(),
                        turbo_mode: None
                    }
                ),
                output_type: "mouseGroup".into(),
                dkm_input: None,
                sensitivity_group: None,
                is_hyper_shift: None
            },
            LegacyMouseSourceDefaultmappingsitem{
                input_id: "RightClick".into(),
                input_type: "MouseInput".into(),
                mouse_input: Some("RightClick".into()),
                mouse_group: Some(
                    LegacyMouseSourceDefaultmappingsitemMousegroup{
                        mouse_assignment: "Menu".into(),
                        turbo_mode: None
                    }
                ),
                output_type: "mouseGroup".into(),
                dkm_input: None,
                sensitivity_group: None,
                is_hyper_shift: None
            },
            LegacyMouseSourceDefaultmappingsitem{
                input_id: "ScrollButton".into(),
                input_type: "MouseInput".into(),
                mouse_input: Some("ScrollButton".into()),
                mouse_group: Some(
                    LegacyMouseSourceDefaultmappingsitemMousegroup{
                        mouse_assignment: "ScrollButton".into(),
                        turbo_mode: None
                    }
                ),
                output_type: "mouseGroup".into(),
                dkm_input: None,
                sensitivity_group: None,
                is_hyper_shift: None
            },
            LegacyMouseSourceDefaultmappingsitem{
                input_id: "DKM_M_01".into(),
                input_type: "DKMInput".into(),
                mouse_input: None,
                mouse_group: None,
                output_type: "sensitivityGroup".into(),
                dkm_input: Some("DKM_M_01".into()),
                sensitivity_group: Some(
                    LegacyMouseSourceDefaultmappingsitemSensitivitygroup{
                        sensitivity_assignment: "DPI_Up".into()
                    }
                ),
                is_hyper_shift: None
            },
            LegacyMouseSourceDefaultmappingsitem{
                input_id: "DKM_M_02".into(),
                input_type: "DKMInput".into(),
                mouse_input: None,
                mouse_group: None,
                output_type: "sensitivityGroup".into(),
                dkm_input: Some("DKM_M_02".into()),
                sensitivity_group: Some(
                    LegacyMouseSourceDefaultmappingsitemSensitivitygroup{
                        sensitivity_assignment: "DPI_Down".into()
                    }
                ),
                is_hyper_shift: None
            },
            LegacyMouseSourceDefaultmappingsitem{
                input_id: "ScrollUp".into(),
                input_type: "MouseInput".into(),
                mouse_input: Some("ScrollUp".into()),
                mouse_group: Some(
                    LegacyMouseSourceDefaultmappingsitemMousegroup{
                        mouse_assignment: "ScrollUp".into(),
                        turbo_mode: None
                    }
                ),
                output_type: "mouseGroup".into(),
                dkm_input: None,
                sensitivity_group: None,
                is_hyper_shift: None
            },
            LegacyMouseSourceDefaultmappingsitem{
                input_id: "ScrollDown".into(),
                input_type: "MouseInput".into(),
                mouse_input: Some("ScrollDown".into()),
                mouse_group: Some(
                    LegacyMouseSourceDefaultmappingsitemMousegroup{
                        mouse_assignment: "ScrollDown".into(),
                        turbo_mode: None
                    }
                ),
                output_type: "mouseGroup".into(),
                dkm_input: None,
                sensitivity_group: None,
                is_hyper_shift: None
            },
            LegacyMouseSourceDefaultmappingsitem{
                input_id: "Button4".into(),
                input_type: "MouseInput".into(),
                mouse_input: Some("Button4".into()),
                mouse_group: Some(
                    LegacyMouseSourceDefaultmappingsitemMousegroup{
                        mouse_assignment: "Previous".into(),
                        turbo_mode: None
                    }
                ),
                output_type: "mouseGroup".into(),
                dkm_input: None,
                sensitivity_group: None,
                is_hyper_shift: None
            },
            LegacyMouseSourceDefaultmappingsitem{
                input_id: "Button5".into(),
                input_type: "MouseInput".into(),
                mouse_input: Some("Button5".into()),
                mouse_group: Some(
                    LegacyMouseSourceDefaultmappingsitemMousegroup{
                        mouse_assignment: "Next".into(),
                        turbo_mode: None
                    }
                ),
                output_type: "mouseGroup".into(),
                dkm_input: None,
                sensitivity_group: None,
                is_hyper_shift: None
            },
            LegacyMouseSourceDefaultmappingsitem{
                input_id: "ScrollLeft".into(),
                input_type: "MouseInput".into(),
                mouse_input: Some("ScrollLeft".into()),
                mouse_group: Some(
                    LegacyMouseSourceDefaultmappingsitemMousegroup{
                        mouse_assignment: "ScrollUp".into(),
                        turbo_mode: Some(
                            LegacyMouseSourceDefaultmappingsitemMousegroupTurbomode{is_turbo:true}
                        )
                    }
                ),
                output_type: "mouseGroup".into(),
                dkm_input: None,
                sensitivity_group: None,
                is_hyper_shift: None
            },
            LegacyMouseSourceDefaultmappingsitem{
                input_id: "ScrollRight".into(),
                input_type: "MouseInput".into(),
                mouse_input: Some("ScrollRight".into()),
                mouse_group: Some(
                    LegacyMouseSourceDefaultmappingsitemMousegroup{
                        mouse_assignment: "ScrollDown".into(),
                        turbo_mode: Some(
                            LegacyMouseSourceDefaultmappingsitemMousegroupTurbomode{is_turbo:true}
                        )
                    }
                ),
                output_type: "mouseGroup".into(),
                dkm_input: None,
                sensitivity_group: None,
                is_hyper_shift: None
            },
            LegacyMouseSourceDefaultmappingsitem{
                input_id: "LeftClick".into(),
                input_type: "MouseInput".into(),
                mouse_input: Some("LeftClick".into()),
                mouse_group: Some(
                    LegacyMouseSourceDefaultmappingsitemMousegroup{
                        mouse_assignment: "Click".into(),
                        turbo_mode: None
                    }
                ),
                output_type: "mouseGroup".into(),
                dkm_input: None,
                sensitivity_group: None,
                is_hyper_shift: Some(true)
            },
            LegacyMouseSourceDefaultmappingsitem{
                input_id: "RightClick".into(),
                input_type: "MouseInput".into(),
                mouse_input: Some("RightClick".into()),
                mouse_group: Some(
                    LegacyMouseSourceDefaultmappingsitemMousegroup{
                        mouse_assignment: "Menu".into(),
                        turbo_mode: None
                    }
                ),
                output_type: "mouseGroup".into(),
                dkm_input: None,
                sensitivity_group: None,
                is_hyper_shift: Some(true)
            },
            LegacyMouseSourceDefaultmappingsitem{
                input_id: "ScrollButton".into(),
                input_type: "MouseInput".into(),
                mouse_input: Some("ScrollButton".into()),
                mouse_group: Some(
                    LegacyMouseSourceDefaultmappingsitemMousegroup{
                        mouse_assignment: "ScrollButton".into(),
                        turbo_mode: None
                    }
                ),
                output_type: "mouseGroup".into(),
                dkm_input: None,
                sensitivity_group: None,
                is_hyper_shift: Some(true)
            },
            LegacyMouseSourceDefaultmappingsitem{
                input_id: "DKM_M_01".into(),
                input_type: "DKMInput".into(),
                mouse_input: None,
                mouse_group: None,
                output_type: "sensitivityGroup".into(),
                dkm_input: Some("DKM_M_01".into()),
                sensitivity_group: Some(
                    LegacyMouseSourceDefaultmappingsitemSensitivitygroup{
                        sensitivity_assignment: "DPI_Up".into()
                    }
                ),
                is_hyper_shift: Some(true)
            },
            LegacyMouseSourceDefaultmappingsitem{
                input_id: "DKM_M_02".into(),
                input_type: "DKMInput".into(),
                mouse_input: None,
                mouse_group: None,
                output_type: "sensitivityGroup".into(),
                dkm_input: Some("DKM_M_02".into()),
                sensitivity_group: Some(
                    LegacyMouseSourceDefaultmappingsitemSensitivitygroup{
                        sensitivity_assignment: "DPI_Down".into()
                    }
                ),
                is_hyper_shift: Some(true)
            },
            LegacyMouseSourceDefaultmappingsitem{
                input_id: "ScrollUp".into(),
                input_type: "MouseInput".into(),
                mouse_input: Some("ScrollUp".into()),
                mouse_group: Some(
                    LegacyMouseSourceDefaultmappingsitemMousegroup{
                        mouse_assignment: "ScrollUp".into(),
                        turbo_mode: None
                    }
                ),
                output_type: "mouseGroup".into(),
                dkm_input: None,
                sensitivity_group: None,
                is_hyper_shift: Some(true)
            },
            LegacyMouseSourceDefaultmappingsitem{
                input_id: "ScrollDown".into(),
                input_type: "MouseInput".into(),
                mouse_input: Some("ScrollDown".into()),
                mouse_group: Some(
                    LegacyMouseSourceDefaultmappingsitemMousegroup{
                        mouse_assignment: "ScrollDown".into(),
                        turbo_mode: None
                    }
                ),
                output_type: "mouseGroup".into(),
                dkm_input: None,
                sensitivity_group: None,
                is_hyper_shift: Some(true)
            },
            LegacyMouseSourceDefaultmappingsitem{
                input_id: "Button4".into(),
                input_type: "MouseInput".into(),
                mouse_input: Some("Button4".into()),
                mouse_group: Some(
                    LegacyMouseSourceDefaultmappingsitemMousegroup{
                        mouse_assignment: "Previous".into(),
                        turbo_mode: None
                    }
                ),
                output_type: "mouseGroup".into(),
                dkm_input: None,
                sensitivity_group: None,
                is_hyper_shift: Some(true)
            },
            LegacyMouseSourceDefaultmappingsitem{
                input_id: "Button5".into(),
                input_type: "MouseInput".into(),
                mouse_input: Some("Button5".into()),
                mouse_group: Some(
                    LegacyMouseSourceDefaultmappingsitemMousegroup{
                        mouse_assignment: "Next".into(),
                        turbo_mode: None
                    }
                ),
                output_type: "mouseGroup".into(),
                dkm_input: None,
                sensitivity_group: None,
                is_hyper_shift: Some(true)
            },
            LegacyMouseSourceDefaultmappingsitem{
                input_id: "ScrollLeft".into(),
                input_type: "MouseInput".into(),
                mouse_input: Some("ScrollLeft".into()),
                mouse_group: Some(
                    LegacyMouseSourceDefaultmappingsitemMousegroup{
                        mouse_assignment: "ScrollUp".into(),
                        turbo_mode: Some(
                            LegacyMouseSourceDefaultmappingsitemMousegroupTurbomode{is_turbo:true}
                        )
                    }
                ),
                output_type: "mouseGroup".into(),
                dkm_input: None,
                sensitivity_group: None,
                is_hyper_shift: Some(true)
            },
            LegacyMouseSourceDefaultmappingsitem{
                input_id: "ScrollRight".into(),
                input_type: "MouseInput".into(),
                mouse_input: Some("ScrollRight".into()),
                mouse_group: Some(
                    LegacyMouseSourceDefaultmappingsitemMousegroup{
                        mouse_assignment: "ScrollDown".into(),
                        turbo_mode: Some(
                            LegacyMouseSourceDefaultmappingsitemMousegroupTurbomode{is_turbo:true}
                        )
                    }
                ),
                output_type: "mouseGroup".into(),
                dkm_input: None,
                sensitivity_group: None,
                is_hyper_shift: Some(true)
            }
        ],
        dkm_keys: vec![
            LegacyMouseSourceDkmkeysitem{
                input_id: "DKM_M_01".into(),
                button_key: Some("SensitivityStageUp".into()),
                key: 32u64
            },
            LegacyMouseSourceDkmkeysitem{
                input_id: "DKM_M_02".into(),
                button_key: Some("SensitivityStageDown".into()),
                key: 33u64
            },
            LegacyMouseSourceDkmkeysitem{input_id:"DKM_M_03".into(),button_key:None,key:35u64},
            LegacyMouseSourceDkmkeysitem{input_id:"DKM_M_04".into(),button_key:None,key:34u64}
        ],
        profile: LegacyMouseSourceProfile{
            guid: "".into(),
            name: "".into(),
            dpi: LegacyMouseSourceProfileDpi{x:1800u64,y:1800u64},
            brightness: LegacyMouseSourceProfileBrightness{is_enabled:true,value:66u64},
            switch_off_lighting: LegacyMouseSourceProfileSwitchofflighting{
                is_display_on: false,
                is_idle_enabled: false,
                idle_minutes: 1u64
            },
            quick_effects: LegacyMouseSourceProfileQuickeffects{selected_effect_id:3u64},
            mappings: vec![],
            dpi_stages: LegacyMouseSourceProfileDpistages{
                stages: vec![
                    LegacyMouseSourceProfileDpistagesStagesitem{
                        x: 800u64,
                        y: 800u64,
                        independent: false,
                        visible: true
                    },
                    LegacyMouseSourceProfileDpistagesStagesitem{
                        x: 1800u64,
                        y: 1800u64,
                        independent: false,
                        visible: true
                    },
                    LegacyMouseSourceProfileDpistagesStagesitem{
                        x: 4500u64,
                        y: 4500u64,
                        independent: false,
                        visible: true
                    },
                    LegacyMouseSourceProfileDpistagesStagesitem{
                        x: 9000u64,
                        y: 9000u64,
                        independent: false,
                        visible: true
                    },
                    LegacyMouseSourceProfileDpistagesStagesitem{
                        x: 16000u64,
                        y: 16000u64,
                        independent: false,
                        visible: true
                    }
                ],
                active: 2u64,
                enable: true,
                count: 5u64
            },
            polling_rate: 1000u64
        }
    }
}
