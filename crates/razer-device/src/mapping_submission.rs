//! Current 190 mouse and 678/679/688 analog-keyboard submission preparation.
//! This is source-derived profile/engine input normalization. A prepared plan
//! does not acknowledge persistence, engine publication, or a hardware write.
//! See docs/re/mapping-submission-current.md for the still-missing executors.
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_SOURCE_VERSION: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MappingSubmissionIntent {
    pub operation_id: String,
    pub generation: u64,
    pub product_id: u32,
    /// Rust ownership guard. The original source selects the live active profile.
    pub expected_profile_guid: String,
    /// Distinguishes an absent source version from a genuine version zero.
    pub expected_version: Option<u64>,
    pub mappings: Vec<Value>,
    pub view_index: Option<u32>,
    pub is_select_all: bool,
    pub is_reset_keybinds: bool,
}

/// Actual producer outcomes are separate observations. In particular, the
/// original task-maker completed event precedes its queued OBM device writes.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum MappingSubmissionStage {
    ProfilePersisted {
        version: u64,
    },
    EnginePublished,
    ObmWriteAcknowledged {
        slot: u8,
        input_id: String,
        is_hyper_shift: bool,
    },
    Failed {
        stage: String,
        message: String,
    },
    Canceled,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MappingSubmissionObservation {
    pub operation_id: String,
    pub generation: u64,
    pub product_id: u32,
    pub profile_guid: String,
    pub stage: MappingSubmissionStage,
}

/// Values must come from the actual current product/runtime/preset sources.
/// Unobserved actuation configuration cannot be replaced with a guessed value.
#[derive(Clone, Debug, Default)]
pub struct AnalogContext {
    pub layout_id: u32,
    pub actuation_unit: Option<f64>,
    pub actuation_min: Option<f64>,
    pub default_actuations: BTreeMap<String, [f64; 2]>,
    pub is_preset: Option<bool>,
    pub preset_reset_mappings: Option<Vec<Value>>,
}

#[derive(Clone, Debug)]
pub struct MappingSubmissionPlan {
    pub intent: MappingSubmissionIntent,
    pub old_mappings: Vec<Value>,
    /// The source list after layout, actuation, and OFTM-macro normalization.
    pub new_mappings: Vec<Value>,
    /// Independent input to the source app-engine generator. ModTap flattens only
    /// this list; source profile mappings retain their AnalogInput wrappers.
    pub engine_input: Vec<Value>,
    pub profile: Value,
    pub next_version: u64,
    pub removed_mappings: Vec<Value>,
    /// Some means the source removed DKS-conflicting snap-trigger entries and
    /// must dispatch the changed list, even when the remaining list is empty.
    pub snap_trigger_update: Option<Vec<Value>>,
    /// Source isSelectAll bypasses the queued OBM sync. Other prerequisite
    /// decisions (OBM-ready, slots, engine support) belong to observed runtime.
    pub source_allows_obm_queue: bool,
}

pub fn next_source_version(previous: Option<u64>) -> anyhow::Result<u64> {
    match previous {
        None => Ok(1),
        Some(MAX_SOURCE_VERSION) => Ok(0),
        Some(version) => {
            ensure!(
                version < MAX_SOURCE_VERSION,
                "Source profile version exceeds 53 bits"
            );
            Ok(version + 1)
        }
    }
}

fn js_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.),
        Value::String(value) => !value.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

fn same_mapping_input(left: &Value, right: &Value) -> bool {
    left.get("inputID") == right.get("inputID")
        && js_truthy(&left["isHyperShift"]) == js_truthy(&right["isHyperShift"])
}

pub fn removed_mappings(old: &[Value], new: &[Value]) -> Vec<Value> {
    old.iter()
        .filter(|mapping| !new.iter().any(|new| same_mapping_input(mapping, new)))
        .cloned()
        .collect()
}

fn macro_mapping(mapping: &Value) -> bool {
    mapping["outputType"] == "macroGroup"
        || (mapping["inputType"] == "AnalogInput"
            && mapping["mapping"][0]["outputType"] == "macroGroup")
}

fn dks_mapping(mapping: &Value) -> bool {
    mapping["outputType"] == "dynamicKeyStrokeGroup"
        || (mapping["inputType"] == "AnalogInput"
            && mapping["mapping"][0]["outputType"] == "dynamicKeyStrokeGroup")
}

impl MappingSubmissionPlan {
    /// Called only after a genuine current profile/version observation. This
    /// prepares data without mutating the caller's document on a rejected branch.
    pub fn prepare(
        intent: MappingSubmissionIntent,
        document: &Value,
        analog: Option<&AnalogContext>,
    ) -> anyhow::Result<Self> {
        ensure!(
            matches!(intent.product_id, 190 | 678 | 679 | 688),
            "Current mapping submission product branch has not been recovered"
        );
        ensure!(
            !intent.operation_id.is_empty() && !intent.operation_id.contains('\0'),
            "Invalid mapping operation identity"
        );
        ensure!(
            document["activeProfile"].as_str() == Some(&intent.expected_profile_guid),
            "Mapping active profile identity changed"
        );
        let observed_version = match document.get("version") {
            None => None,
            Some(value) => Some(value.as_u64().context("Invalid source profile version")?),
        };
        ensure!(
            observed_version == intent.expected_version,
            "Mapping profile version changed"
        );
        let source_profile = document["profiles"]
            .as_array()
            .context("Missing source profile list")?
            .iter()
            .find(|profile| profile["guid"] == intent.expected_profile_guid)
            .context("No active profile found")?;
        let old_mappings = source_profile["mappings"]
            .as_array()
            .context("Missing source mapping list")?
            .clone();
        ensure!(
            intent.mappings.iter().all(Value::is_object),
            "Invalid source mapping record"
        );
        let mut profile = source_profile.clone();
        let mut new_mappings = intent.mappings.clone();
        let mut snap_trigger_update = None;
        let engine_input = if intent.product_id == 190 {
            ensure!(
                intent.view_index.is_none() || intent.view_index == Some(0),
                "This mouse has no recovered side-panel submission branch"
            );
            new_mappings.clone()
        } else {
            let context = analog.context("Missing actual analog submission context")?;
            normalize_analog(&mut new_mappings, &old_mappings, context)?;
            let mut engine = modtap_engine_input(&mut new_mappings, js_truthy(&profile["modTap"]))?;
            if js_truthy(&profile["interruptKey"]["isEnabled"]) {
                let interrupts = if profile["interruptKey"]["interruptionKey"] == "any" {
                    vec![json!("KEY_ANY")]
                } else {
                    profile["interruptKey"]["interrupts"]
                        .as_array()
                        .context("Missing actual interrupt keys")?
                        .clone()
                };
                for mapping in &mut engine {
                    if mapping["inputType"] == "AnalogInput"
                        && (macro_mapping(mapping) || dks_mapping(mapping))
                    {
                        mapping["interrupts"] = json!(interrupts);
                    }
                }
            }
            if let Some(keys) = profile["singleKeySnapTap"]["keyList"].as_array() {
                let dks: BTreeSet<_> = engine
                    .iter()
                    .filter(|mapping| {
                        mapping["inputType"] == "AnalogInput"
                            && mapping["mapping"][0]["outputType"] == "dynamicKeyStrokeGroup"
                    })
                    .filter_map(|mapping| mapping["inputID"].as_str())
                    .collect();
                let mut remaining: Vec<_> = keys
                    .iter()
                    .filter(|pair| {
                        !pair["keyMapBreak"]
                            .as_str()
                            .is_some_and(|key| dks.contains(key))
                            && !pair["keyMapMake"]
                                .as_str()
                                .is_some_and(|key| dks.contains(key))
                    })
                    .cloned()
                    .collect();
                if remaining.len() != keys.len() {
                    for (index, pair) in remaining.iter_mut().enumerate() {
                        pair["id"] = json!(index + 1);
                    }
                    profile["singleKeySnapTap"]["keyList"] = json!(remaining);
                    snap_trigger_update = Some(remaining);
                }
            }
            for mapping in &mut new_mappings {
                if macro_mapping(mapping) || dks_mapping(mapping) {
                    mapping.as_object_mut().unwrap().remove("interrupts");
                }
            }
            if intent.is_reset_keybinds {
                if context
                    .is_preset
                    .context("Missing actual preset identity")?
                {
                    new_mappings = context
                        .preset_reset_mappings
                        .as_ref()
                        .context("Missing actual reset preset mappings")?
                        .clone();
                }
            }
            engine
        };
        profile["mappings"] = json!(new_mappings);
        Ok(Self {
            removed_mappings: removed_mappings(&old_mappings, &new_mappings),
            next_version: next_source_version(observed_version)?,
            source_allows_obm_queue: intent.product_id == 190 || !intent.is_select_all,
            intent,
            old_mappings,
            new_mappings,
            engine_input,
            profile,
            snap_trigger_update,
        })
    }
}

fn normalize_analog(
    mappings: &mut Vec<Value>,
    old: &[Value],
    context: &AnalogContext,
) -> anyhow::Result<()> {
    // Current MW constants Yi0/nLe: ISO layouts omit KEY_BACKSLASH.
    if matches!(context.layout_id, 3 | 4 | 6 | 7 | 10 | 15 | 16 | 17 | 18) {
        mappings.retain(|mapping| mapping["inputID"] != "KEY_BACKSLASH");
    }
    for mapping in mappings.iter_mut() {
        if mapping["inputType"] == "AnalogInput" {
            let unit = context
                .actuation_unit
                .context("Missing actual actuation unit")?;
            let minimum = context
                .actuation_min
                .context("Missing actual actuation minimum")?;
            ensure!(
                unit.is_finite() && minimum.is_finite(),
                "Invalid actual actuation converter"
            );
            for output in mapping["mapping"]
                .as_array_mut()
                .context("Missing AnalogInput outputs")?
            {
                let points = output["actuationPoint"]
                    .as_array_mut()
                    .context("Missing actuation points")?;
                ensure!(points.len() >= 2, "Truncated actuation points");
                let make = points[0]
                    .as_f64()
                    .filter(|value| value.is_finite())
                    .context("Invalid actuation make value")?;
                points[1] = json!((make - unit).max(minimum));
            }
        }
    }
    // The source performs macro conversion after the break-point pass, retaining
    // actual existing/default pairs rather than normalizing the converted pair.
    for mapping in mappings.iter_mut() {
        if mapping["outputType"] != "macroGroup"
            || !js_truthy(&mapping["inputID"])
            || (js_truthy(&mapping["mapping"]) && mapping["inputType"] != "KeyInput")
        {
            continue;
        }
        let input = mapping["inputID"]
            .as_str()
            .context("Invalid macro input identity")?;
        let previous = old
            .iter()
            .find(|entry| entry["inputID"] == input && !js_truthy(&entry["isHyperShift"]));
        let points = if previous.is_some_and(|entry| entry["inputType"] == "AnalogInput") {
            previous
                .and_then(|entry| entry["mapping"][0].get("actuationPoint"))
                .cloned()
        } else {
            None
        };
        let points = match points {
            Some(points) if js_truthy(&points) => points,
            _ => json!(
                context
                    .default_actuations
                    .get(input)
                    .context("Missing source default macro actuation")?
            ),
        };
        *mapping = json!({"inputID":input,"inputType":"AnalogInput",
            "isHyperShift":if js_truthy(&mapping["isHyperShift"]){mapping["isHyperShift"].clone()}else{json!(false)},
            "mapping":[{"outputType":"macroGroup","macroGroup":mapping["macroGroup"],"actuationPoint":points},
                {"outputType":"defaultGroup","actuationPoint":points,"isDefault":false,"isHyperShift":false}]});
    }
    Ok(())
}

fn modtap_engine_input(mappings: &mut [Value], enabled: bool) -> anyhow::Result<Vec<Value>> {
    mappings
        .iter_mut()
        .map(|mapping| {
            if enabled
                && matches!(
                    mapping["inputID"].as_str(),
                    Some(
                        "KEY_RIGHT_SHIFT" | "KEY_RIGHT_ALT" | "KEY_APPLICATION" | "KEY_RIGHT_CTRL"
                    )
                )
            {
                let mut first = mapping["mapping"]
                    .as_array()
                    .and_then(|outputs| outputs.first())
                    .filter(|output| output.is_object())
                    .cloned()
                    .context("Missing source ModTap first output")?;
                first["inputID"] = mapping["inputID"].clone();
                first["inputType"] = json!("KeyInput");
                if let Some(hyper) = mapping.get("isHyperShift") {
                    first["isHyperShift"] = hyper.clone();
                } else {
                    first.as_object_mut().unwrap().remove("isHyperShift");
                }
                // Source `i=t.mapping[0]` aliases the original nested output:
                // flattening the engine list also retains these fields inside
                // the still-wrapped persisted AnalogInput record.
                mapping["mapping"][0] = first.clone();
                Ok(first)
            } else {
                Ok(mapping.clone())
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn intent(pid: u32, mappings: Vec<Value>) -> MappingSubmissionIntent {
        MappingSubmissionIntent {
            operation_id: "simulated-operation".into(),
            generation: 8,
            product_id: pid,
            expected_profile_guid: "selected-profile".into(),
            expected_version: Some(19),
            mappings,
            view_index: Some(0),
            is_select_all: false,
            is_reset_keybinds: false,
        }
    }

    fn document(mappings: Vec<Value>) -> Value {
        json!({"activeProfile":"selected-profile","version":19,
            "profiles":[{"guid":"other-profile","mappings":[]},
                {"guid":"selected-profile","mappings":mappings}]})
    }

    fn analog_output(key: &str, output: &str, make: f64) -> Value {
        json!({"inputID":key,"inputType":"AnalogInput","isHyperShift":false,
            "mapping":[{"outputType":output,"actuationPoint":[make,999.]}]})
    }

    fn context() -> AnalogContext {
        // Deliberate simulation values, not an asserted product observation.
        AnalogContext {
            layout_id: 1,
            actuation_unit: Some(10.),
            actuation_min: Some(5.),
            default_actuations: BTreeMap::from([("KEY_A".into(), [70., 60.])]),
            is_preset: Some(false),
            preset_reset_mappings: None,
        }
    }

    #[test]
    fn source_version_zero_is_distinct_from_missing_and_max_wraps_to_zero() {
        assert_eq!(next_source_version(None).unwrap(), 1);
        assert_eq!(next_source_version(Some(0)).unwrap(), 1);
        assert_eq!(next_source_version(Some(MAX_SOURCE_VERSION)).unwrap(), 0);
        assert!(next_source_version(Some(MAX_SOURCE_VERSION + 1)).is_err());
    }

    #[test]
    fn removal_preserves_distinct_hypershift_layers_and_js_truthiness() {
        let normal = json!({"inputID":"LeftClick"});
        let hyper = json!({"inputID":"LeftClick","isHyperShift":true});
        let old = [normal.clone(), hyper.clone()];
        assert_eq!(
            removed_mappings(&old, &[json!({"inputID":"LeftClick","isHyperShift":0})]),
            vec![hyper]
        );
        assert_eq!(
            removed_mappings(
                &old,
                &[json!({"inputID":"LeftClick","isHyperShift":"false"})]
            ),
            vec![normal]
        );
    }

    #[test]
    fn stale_profile_or_version_never_mutates_the_observed_document() {
        let doc = document(vec![json!({"inputID":"LeftClick"})]);
        let original = doc.clone();
        let mut request = intent(190, vec![]);
        request.expected_profile_guid = "other-profile".into();
        assert!(MappingSubmissionPlan::prepare(request, &doc, None).is_err());
        let mut request = intent(190, vec![]);
        request.expected_version = Some(18);
        assert!(MappingSubmissionPlan::prepare(request, &doc, None).is_err());
        assert_eq!(doc, original);
    }

    #[test]
    fn mouse_plan_is_profile_data_and_does_not_invent_engine_or_device_success() {
        let old = json!({"inputID":"LeftClick","outputType":"mouseGroup","mouseGroup":{"mouseAssignment":"LeftClick"}});
        let doc = document(vec![old.clone()]);
        let plan = MappingSubmissionPlan::prepare(intent(190, vec![]), &doc, None).unwrap();
        assert_eq!(plan.old_mappings, vec![old.clone()]);
        assert_eq!(plan.removed_mappings, vec![old]);
        assert_eq!(plan.next_version, 20);
        assert!(plan.engine_input.is_empty());
        assert!(plan.profile.get("appEngine").is_none());
        assert!(plan.profile.get("device_write_verified").is_none());
        assert_eq!(doc["version"], 19);
    }

    #[test]
    fn all_three_analog_products_normalize_actual_break_points() {
        for pid in [678, 679, 688] {
            let request = intent(
                pid,
                vec![
                    analog_output("KEY_A", "keyboardGroup", 40.),
                    analog_output("KEY_B", "keyboardGroup", 7.),
                ],
            );
            let plan = MappingSubmissionPlan::prepare(request, &document(vec![]), Some(&context()))
                .unwrap();
            assert_eq!(
                plan.new_mappings[0]["mapping"][0]["actuationPoint"],
                json!([40., 30.])
            );
            assert_eq!(
                plan.new_mappings[1]["mapping"][0]["actuationPoint"],
                json!([7., 5.])
            );
        }
    }

    #[test]
    fn iso_exclusion_matches_source_layouts_and_keeps_other_keys() {
        for layout in [3, 4, 6, 7, 10, 15, 16, 17, 18] {
            let mut ctx = context();
            ctx.layout_id = layout;
            let plan = MappingSubmissionPlan::prepare(
                intent(
                    679,
                    vec![
                        analog_output("KEY_BACKSLASH", "keyboardGroup", 40.),
                        analog_output("KEY_NON_US_POUND", "keyboardGroup", 40.),
                    ],
                ),
                &document(vec![]),
                Some(&ctx),
            )
            .unwrap();
            assert_eq!(plan.new_mappings.len(), 1);
            assert_eq!(plan.new_mappings[0]["inputID"], "KEY_NON_US_POUND");
        }
        let plan = MappingSubmissionPlan::prepare(
            intent(
                679,
                vec![analog_output("KEY_BACKSLASH", "keyboardGroup", 40.)],
            ),
            &document(vec![]),
            Some(&context()),
        )
        .unwrap();
        assert_eq!(plan.new_mappings.len(), 1);
    }

    #[test]
    fn oft_macro_conversion_uses_existing_normal_layer_after_normalization_pass() {
        let mut old = analog_output("KEY_A", "keyboardGroup", 42.);
        old["mapping"][0]["actuationPoint"] = json!([42., 39.]);
        let requested = json!({"inputID":"KEY_A","inputType":"KeyInput","isHyperShift":true,
            "outputType":"macroGroup","macroGroup":{"guid":"macro-1"}});
        let plan = MappingSubmissionPlan::prepare(
            intent(679, vec![requested]),
            &document(vec![old]),
            Some(&context()),
        )
        .unwrap();
        assert_eq!(plan.new_mappings[0]["inputType"], "AnalogInput");
        assert!(
            plan.new_mappings[0]["mapping"][0]
                .get("inputType")
                .is_none()
        );
        assert_eq!(
            plan.new_mappings[0]["mapping"][0]["actuationPoint"],
            json!([42., 39.])
        );
        assert_eq!(
            plan.new_mappings[0]["mapping"][1]["actuationPoint"],
            json!([42., 39.])
        );
        assert_eq!(plan.new_mappings[0]["isHyperShift"], true);
        assert_eq!(plan.new_mappings[0]["mapping"][1]["isHyperShift"], false);
    }

    #[test]
    fn missing_macro_actuation_observation_is_an_error() {
        let request = intent(
            678,
            vec![json!({"inputID":"KEY_Z","inputType":"KeyInput",
            "outputType":"macroGroup","macroGroup":{"guid":"macro-1"}})],
        );
        assert!(
            MappingSubmissionPlan::prepare(request, &document(vec![]), Some(&context())).is_err()
        );
    }

    #[test]
    fn modtap_flattens_engine_only_and_dks_changes_snap_trigger_pairs() {
        let mut doc = document(vec![]);
        doc["profiles"][1]["modTap"] = json!(true);
        doc["profiles"][1]["interruptKey"] = json!({"isEnabled":true,"interruptionKey":"any"});
        doc["profiles"][1]["singleKeySnapTap"] = json!({"isEnabled":false,"keyList":[
            {"id":7,"keyMapMake":"KEY_A","keyMapBreak":"KEY_B"},
            {"id":9,"keyMapMake":"KEY_C","keyMapBreak":"KEY_D"}]});
        let mappings = vec![
            analog_output("KEY_RIGHT_SHIFT", "keyboardGroup", 40.),
            analog_output("KEY_A", "dynamicKeyStrokeGroup", 40.),
        ];
        let plan =
            MappingSubmissionPlan::prepare(intent(688, mappings), &doc, Some(&context())).unwrap();
        assert_eq!(plan.engine_input[0]["inputType"], "KeyInput");
        assert_eq!(plan.new_mappings[0]["inputType"], "AnalogInput");
        assert_eq!(plan.new_mappings[0]["mapping"][0]["inputType"], "KeyInput");
        assert_eq!(
            plan.new_mappings[0]["mapping"][0]["inputID"],
            "KEY_RIGHT_SHIFT"
        );
        assert_eq!(plan.engine_input[1]["interrupts"], json!(["KEY_ANY"]));
        assert!(plan.new_mappings[1].get("interrupts").is_none());
        assert_eq!(
            plan.snap_trigger_update.unwrap(),
            vec![json!({"id":1,"keyMapMake":"KEY_C","keyMapBreak":"KEY_D"})]
        );
        // Source removes conflicting pairs regardless of isEnabled.
        assert_eq!(plan.profile["singleKeySnapTap"]["isEnabled"], false);
    }

    #[test]
    fn preset_reset_changes_profile_list_after_generator_input_and_select_all_skips_obm_queue() {
        let mut ctx = context();
        ctx.is_preset = Some(true);
        ctx.preset_reset_mappings = Some(vec![analog_output("KEY_C", "defaultGroup", 80.)]);
        let mut request = intent(679, vec![analog_output("KEY_A", "keyboardGroup", 40.)]);
        request.is_reset_keybinds = true;
        request.is_select_all = true;
        let plan = MappingSubmissionPlan::prepare(request, &document(vec![]), Some(&ctx)).unwrap();
        assert_eq!(plan.engine_input[0]["inputID"], "KEY_A");
        assert_eq!(plan.new_mappings[0]["inputID"], "KEY_C");
        assert!(!plan.source_allows_obm_queue);
    }
}
