//! PID70 current MW main.4d2dda8011e526ce4d9b (profile/DKM/defaults)
//! and 6120.7fff31f083f844383e26 (84210 initialization, 91105 je).
//! Source documents and native engine publication remain separate operations.
use crate::{
    mapping_engine::{EngineContext, MappingEngine},
    mapping_submission::next_source_version,
};
use anyhow::{Context as _, Result, ensure};
use serde_json::{Value, json};
use std::sync::LazyLock;

#[path = "../data/mouse_legacy_mapping_data_schema.rs"]
mod source_data;
use source_data::*;

static SOURCE: LazyLock<LegacyMouseSource> =
    LazyLock::new(|| crate::data::mouse_legacy_mapping_data::mouse_legacy_mapping_data!());

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
    if context.default_turbos.as_ref().is_none_or(Vec::is_empty) {
        engine.generate_default_turbos(&mut context)?;
    }
    let mappings = SOURCE
        .default_mappings
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<_>, _>>()?;
    let defaults = engine.generate(&mappings, &mut context)?;
    let mut normalized = Vec::with_capacity(profiles.len());
    for source in profiles {
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
        profile["appEngine"] = engine.generate(&mappings, &mut context)?.app_engine;
        normalized.push(profile);
    }
    ensure!(
        normalized
            .iter()
            .filter(|profile| profile["guid"] == active_guid)
            .count()
            == 1,
        "Active profile is not unique"
    );
    let mut document = previous.cloned().unwrap_or_else(|| json!({}));
    ensure!(document.is_object(), "Invalid prior mouse document");
    document["version"] = json!(next_source_version(
        previous.and_then(|doc| doc["version"].as_u64())
    )?);
    document["productId"] = json!(70);
    document["vendorId"] = json!(5426);
    document["category"] = json!("MOUSE");
    document["reportIDs"] = json!({"4":"razerKeyReportID","5":"hardwareEventReportID"});
    document["profiles"] = json!(normalized);
    document["activeProfile"] = json!(active_guid);
    document["defaultMappings"] = json!({"appEngine":defaults.app_engine});
    document["defaultTurbos"] = json!(context.default_turbos.unwrap_or_default());
    document
        .as_object_mut()
        .unwrap()
        .remove("synapseDefaultTurbos");
    document["synapseTurbos"] = json!(context.synapse_turbos.unwrap_or_default());
    Ok(document)
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
    let mut mappings: Vec<_> = source
        .iter()
        .filter(|mapping| mapping["outputType"] == "sensitivityGroup")
        .cloned()
        .collect();
    mappings.extend([("ScrollUp", "DPI_Value_Up"), ("ScrollDown", "DPI_Value_Down")].map(|(input, assignment)| json!({
        "inputType":"MouseInput", "inputID":input, "isHyperShift":false,
        "outputType":"sensitivityGroup",
        "sensitivityGroup":{"sensitivityAssignment":assignment,"value":50,"independent":false}
    })));
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
        .filter(|mapping| {
            mapping
                .sensitivity_group
                .as_ref()
                .is_none_or(|group| group.sensitivity_assignment != "OTFS_Scroll")
        })
        .map(serde_json::to_value)
        .collect::<Result<Vec<_>, _>>()?;
    let default_engine = engine.generate(&defaults, &mut context)?.app_engine;
    let mut result = document.clone();
    result["profiles"][active]["appEngine"] = engine.generate(&mappings, &mut context)?.app_engine;
    result["profiles"][active]["defaultMappings"] = default_engine.clone();
    result["defaultMappings"]["appEngine"] = default_engine;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{SOURCE, interdevice_otfs_document, prepare_document};
    use serde_json::json;

    #[test]
    fn typed_mouse_source_preserves_every_interchange_field_and_numeric_kind() {
        let value = serde_json::to_value(&*SOURCE).unwrap();
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
