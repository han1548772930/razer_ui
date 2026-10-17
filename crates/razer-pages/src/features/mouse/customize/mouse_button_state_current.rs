//! Pure projection of the current non-attachment mouse button-state producer.
//! This observes local mappings; it performs no transport, persistence or device operation.
use razer_model::mapping_requirements::{MappingObservations, MappingRequirement};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeriveError {
    UnverifiedProduct(u32),
    UnsupportedCategory(String),
    AttachmentObservationRequired,
    InvalidButton,
    InvalidMappingPayload(String),
    InvalidRequirementObservation(String),
    UnresolvedAssignmentProducer(String),
}

#[derive(Deserialize)]
struct Policy {
    product_id: u32,
    category: String,
    has_attachment: bool,
    use_cycle_scroll_mode_label: bool,
    known_outputs: Vec<String>,
    verified_producers: Vec<String>,
    lookups: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(default)]
    source_family: String,
}

fn policy(product_id: u32) -> Result<&'static Policy, DeriveError> {
    static POLICIES: OnceLock<Vec<Policy>> = OnceLock::new();
    POLICIES
        .get_or_init(|| read_mouse_button_state_current_catalog())
        .iter()
        .find(|policy| policy.product_id == product_id)
        .ok_or(DeriveError::UnverifiedProduct(product_id))
}

/// Returns current `isEnabled`, `isMapped` and assignment fields for the current view.
/// The source caller filters by layer with JS boolean coercion, while the primary
/// helpers intentionally count only mappings whose `isHyperShift` is exactly false.
/// `all_buttons` is the source loader's concatenation of all original view rows.
/// Attachment/New additionally needs other side-panel mappings, so is an explicit gap.
pub fn derive(
    product_id: u32,
    buttons: &[Value],
    _all_buttons: &[Value],
    mappings: &[Value],
    hyper: bool,
    view_index: i32,
    observations: &MappingObservations<'_>,
) -> Result<Vec<Value>, DeriveError> {
    let policy = policy(product_id)?;
    if !matches!(policy.category.as_str(), "MOUSE" | "MOUSEPLUSMAT") {
        return Err(DeriveError::UnsupportedCategory(policy.category.clone()));
    }
    if policy.has_attachment {
        return Err(DeriveError::AttachmentObservationRequired);
    }
    let mut rows = buttons.to_vec();
    let babel = policy.source_family == "babel_162";
    for row in &mut rows {
        let object = row.as_object_mut().ok_or(DeriveError::InvalidButton)?;
        object.insert("isMapped".into(), json!(false));
        object.insert("isEnabled".into(), json!(true));
        object.insert("isRequireSynapse".into(), json!(false));
        if let Some(value) = object.get("defaultValue").cloned() {
            object.insert("assignmentValue".into(), value);
        } else {
            object.remove("assignmentValue");
        }
        object.remove("assignmentGroup");
        for (key, primary) in [
            ("secondaryAssignment", "assignment"),
            ("secondaryAssignmentValue", "assignmentValue"),
            ("secondaryAssignmentGroup", "assignmentGroup"),
        ] {
            // Y.clearButtonValues preserves existing secondary slots by
            // resetting them to the just-cleared primary values.
            if babel && object.get(key).is_some_and(|value| !value.is_null()) {
                if let Some(value) = object.get(primary).cloned() {
                    object.insert(key.into(), value);
                } else {
                    object.remove(key);
                }
            } else {
                object.remove(key);
            }
        }
    }
    let left = rows.iter().position(|row| row["buttonKey"] == "LeftClick");
    if let Some(index) = left {
        rows[index]["isEnabled"] = json!(hyper);
        // Preserve the original source's button-key vs value comparison.
        if view_index >= 0 && rows[index]["assignmentValue"] != "LeftClick" {
            rows[index]["isEnabled"] = json!(true);
        }
    }
    for mapping in mappings
        .iter()
        .filter(|mapping| truthy(&mapping["isHyperShift"]) == hyper)
    {
        let Some(index) = rows
            .iter()
            .position(|row| row.get("inputID") == mapping.get("inputID"))
        else {
            continue;
        };
        if !babel && let Some(parts) = mapping.get("mapping").filter(|parts| truthy(parts)) {
            // Source skips the entire outer mapping when a non-exempt first output
            // has an empty payload. It does not flatten this data for primary counts.
            if let Some(first) = parts.get(0)
                && truthy(first)
                && !matches!(
                    first["outputType"].as_str(),
                    Some("disableGroup" | "hyperShiftGroup")
                )
                && empty(first.get(first["outputType"].as_str().unwrap_or("")))
            {
                continue;
            }
        }
        if let Some(parts) = mapping["mapping"].as_array() {
            for (part_index, part) in parts.iter().enumerate() {
                assign(
                    &mut rows[index],
                    part,
                    part_index != 0,
                    hyper,
                    policy,
                    observations,
                )?;
            }
        } else {
            assign(
                &mut rows[index],
                mapping,
                false,
                hyper,
                policy,
                observations,
            )?;
        }
        if !hyper
            && rows[index]["assignmentValue"] == "LEFT_CLICK"
            && let Some(left) = left
        {
            rows[left]["isEnabled"] = json!(true);
        }
    }
    if !hyper {
        let primary: Vec<_> = rows
            .iter_mut()
            .enumerate()
            .filter_map(|(index, row)| {
                (row["assignmentValue"] == "LEFT_CLICK").then(|| {
                    row["isEnabled"] = json!(true);
                    index
                })
            })
            .collect();
        let standard: Vec<_> = mappings
            .iter()
            .filter(|mapping| mapping["isHyperShift"] == false)
            .collect();
        let count = standard
            .iter()
            .filter(|mapping| mapping["mouseGroup"]["mouseAssignment"] == "Click")
            .count();
        let maps_left = standard
            .iter()
            .any(|mapping| mapping["inputID"] == "LeftClick");
        if count > 0 {
            if maps_left && count == 1 && primary.len() == 1 {
                rows[primary[0]]["isEnabled"] = json!(false);
            } else if !maps_left && let Some(index) = primary.first() {
                rows[*index]["isEnabled"] = json!(true);
            }
        } else if left.is_some()
            && primary.len() == 1
            && rows[primary[0]]["buttonKey"] == "LeftClick"
        {
            rows[primary[0]]["isEnabled"] = json!(false);
        }
    }
    for row in &mut rows {
        if hyper && row["counter"] == "Application" {
            row["counter"] = json!("MENU");
        }
    }
    Ok(rows)
}

fn assign(
    row: &mut Value,
    mapping: &Value,
    secondary: bool,
    hyper: bool,
    policy: &Policy,
    observations: &MappingObservations<'_>,
) -> Result<(), DeriveError> {
    let Some(group) = mapping["outputType"].as_str() else {
        return Ok(());
    };
    if group == "hyperShiftGroup" && hyper {
        row["isEnabled"] = json!(false);
    }
    if group == "defaultGroup" {
        return Ok(());
    }
    let input_id = row
        .get("inputID")
        .and_then(Value::as_str)
        .ok_or(DeriveError::InvalidButton)?;
    let requirement = MappingRequirement::from_source_mapping(mapping, input_id)
        .map_err(|error| DeriveError::InvalidRequirementObservation(format!("{error:#}")))?;
    let capabilities =
        razer_model::mapping_requirements::current_mouse_mapping_capabilities(policy.product_id)
            .ok_or(DeriveError::UnverifiedProduct(policy.product_id))?;
    row["isRequireSynapse"] =
        json!(capabilities.requires_synapse(&requirement, observations, secondary));
    if matches!(
        group,
        "mouseGroup"
            | "sensitivityGroup"
            | "macroGroup"
            | "textBlockGroup"
            | "launchGroup"
            | "aiLauncherGroup"
            | "disableGroup"
            | "hyperShiftGroup"
    ) && !policy
        .verified_producers
        .iter()
        .any(|output| output == group)
    {
        return Err(DeriveError::UnresolvedAssignmentProducer(group.into()));
    }
    let (label, value) = match group {
        "defaultGroup" => return Ok(()),
        "disableGroup" => (
            "DISABLE",
            Some(json!(if policy.source_family == "babel_162" {
                "Disable"
            } else {
                "DISABLE"
            })),
        ),
        "hyperShiftGroup" => ("RAZER_HYPERSHIFT", Some(json!("Razer Hypershift"))),
        "mouseGroup" | "sensitivityGroup" => {
            let key = if group == "mouseGroup" {
                "mouseAssignment"
            } else {
                "sensitivityAssignment"
            };
            let payload = payload(mapping, group)?;
            if policy.source_family == "babel_162"
                && !payload.get(key).and_then(Value::as_str).is_some_and(|id| {
                    policy
                        .lookups
                        .get(group)
                        .is_some_and(|table| table.contains_key(id))
                })
            {
                // Babel G0/Ai access .find(...).content before the fallback;
                // unsupported values cannot be displayed as valid labels.
                return Err(DeriveError::InvalidMappingPayload(group.into()));
            }
            let value = payload.get(key).cloned().map(|value| {
                if let Some(id) = value.as_str() {
                    if id == "ScrollModeSwitch" && group == "mouseGroup" {
                        return json!(if policy.use_cycle_scroll_mode_label {
                            "CYCLE_SCROLL_MODE"
                        } else {
                            "SCROLL_MODE_TOGGLE"
                        });
                    }
                    if let Some(content) = policy.lookups.get(group).and_then(|table| table.get(id))
                    {
                        return json!(content);
                    }
                }
                value
            });
            (
                if group == "mouseGroup" {
                    "MOUSE_FUNCTION"
                } else {
                    "SENSITIVITY"
                },
                value,
            )
        }
        "macroGroup" => {
            let payload = payload(mapping, group)?;
            (
                "MACRO",
                Some(
                    payload
                        .get("name")
                        .filter(|name| truthy(name))
                        .cloned()
                        .unwrap_or(json!(" ")),
                ),
            )
        }
        "textBlockGroup" => (
            "TEXT_FUNCTION",
            payload(mapping, group)?.get("text").cloned(),
        ),
        "launchGroup" => {
            let payload = payload(mapping, group)?;
            (
                "LAUNCH_PROGRAM",
                payload.get("path").or_else(|| payload.get("url")).cloned(),
            )
        }
        "aiLauncherGroup" => (
            "AI_LAUNCHER",
            payload(mapping, group)?.get("assignment").cloned(),
        ),
        "multimediaGroup" | "win8ShortcutsGroup" => {
            let payload = payload(mapping, group)?;
            if group == "multimediaGroup" && payload["isAdvancedMultiDialAudiol"] == true {
                return Err(DeriveError::UnresolvedAssignmentProducer(group.into()));
            }
            super::mapping_basic_current::observed_assignment(policy.product_id, group, payload)
                .ok_or_else(|| DeriveError::UnresolvedAssignmentProducer(group.into()))?
        }
        // These are real source groups whose assignment producers need additional
        // observations or have not yet been reimplemented. Unknown outputType itself
        // follows the source switch default and leaves the cleared row unchanged.
        "keyboardGroup"
        | "dynamicKeyStrokeGroup"
        | "joystickGroup"
        | "keymapGroup"
        | "controllerGroup"
        | "scrollingGroup"
        | "interDeviceGroup"
        | "profileNavigationGroup"
        | "lightPacGroup"
        | "backlightGroup"
        | "brightnessGlobalGroup"
        | "audioGroup"
        | "appSpecificGroup"
        | "controlKnobGroup" => {
            return Err(DeriveError::UnresolvedAssignmentProducer(group.into()));
        }
        _ if policy.known_outputs.iter().any(|output| output == group) => {
            return Err(DeriveError::UnresolvedAssignmentProducer(group.into()));
        }
        _ => return Ok(()),
    };
    let prefix = if secondary {
        "secondaryAssignment"
    } else {
        "assignment"
    };
    row["isMapped"] = json!(true);
    row[prefix] = json!(label);
    let value_key = format!("{prefix}Value");
    if let Some(value) = value {
        row[&value_key] = value;
    } else {
        row.as_object_mut().unwrap().remove(&value_key);
    }
    if !matches!(group, "disableGroup") {
        row[format!("{prefix}Group")] = mapping.get(group).cloned().unwrap_or(Value::Null);
    }
    Ok(())
}

fn payload<'a>(mapping: &'a Value, group: &str) -> Result<&'a Value, DeriveError> {
    mapping
        .get(group)
        .filter(|value| !value.is_null())
        .ok_or_else(|| DeriveError::InvalidMappingPayload(group.into()))
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.),
        Value::String(value) => !value.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

fn empty(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Array(value)) => value.is_empty(),
        Some(Value::Object(value)) => value.is_empty(),
        Some(Value::String(value)) => value.is_empty(),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn derive(
        product_id: u32,
        buttons: &[Value],
        all_buttons: &[Value],
        mappings: &[Value],
        hyper: bool,
        view_index: i32,
    ) -> Result<Vec<Value>, DeriveError> {
        super::derive(
            product_id,
            buttons,
            all_buttons,
            mappings,
            hyper,
            view_index,
            &MappingObservations::new(None, &[]),
        )
    }
    fn buttons() -> Vec<Value> {
        vec![
            json!({"inputID":"LeftClick","buttonKey":"LeftClick","defaultValue":"LEFT_CLICK","isEnabled":false}),
            json!({"inputID":"RightClick","buttonKey":"RightClick","defaultValue":"RIGHT_CLICK","isEnabled":true}),
        ]
    }
    fn mouse(input: &str, assignment: &str, hyper: bool) -> Value {
        json!({"inputID":input,"isHyperShift":hyper,"outputType":"mouseGroup","mouseGroup":{"mouseAssignment":assignment}})
    }
    fn state(mappings: &[Value], hyper: bool) -> Vec<Value> {
        derive(163, &buttons(), &buttons(), mappings, hyper, 0).unwrap()
    }
    #[::core::prelude::v1::test]
    fn layer_and_primary_protection_follow_mapping_state() {
        assert_eq!(state(&[], false)[0]["isEnabled"], false);
        assert_eq!(state(&[], true)[0]["isEnabled"], true);
        let mappings = [mouse("RightClick", "Click", false)];
        assert_eq!(state(&mappings, false)[0]["isEnabled"], true);
        let mappings = [
            mouse("LeftClick", "Menu", false),
            mouse("RightClick", "Click", false),
        ];
        assert_eq!(state(&mappings, false)[1]["isEnabled"], false);
    }
    #[::core::prelude::v1::test]
    fn pid70_canvas_projection_preserves_non_mouse_labels_and_hypershift_lock() {
        let mappings = [
            json!({"inputID":"RightClick","isHyperShift":false,"outputType":"sensitivityGroup",
                "sensitivityGroup":{"sensitivityAssignment":"DPI_Up"}}),
            json!({"inputID":"LeftClick","isHyperShift":true,"outputType":"hyperShiftGroup",
                "hyperShiftGroup":{}}),
        ];
        let standard = derive(70, &buttons(), &buttons(), &mappings, false, 0).unwrap();
        assert_eq!(standard[1]["assignmentValue"], "SENSITIVITY_STAGE_UP");
        assert_eq!(standard[1]["isMapped"], true);
        assert_eq!(standard[0]["isEnabled"], false);
        let hyper = derive(70, &buttons(), &buttons(), &mappings, true, 0).unwrap();
        assert_eq!(hyper[1]["assignmentValue"], "RIGHT_CLICK");
        assert_eq!(hyper[1]["isMapped"], false);
        assert_eq!(hyper[0]["isEnabled"], false);
    }
    #[::core::prelude::v1::test]
    fn primary_helpers_count_top_level_and_require_exact_false() {
        let nested_left = json!({"inputID":"LeftClick","isHyperShift":false,"mapping":[{"outputType":"mouseGroup","mouseGroup":{"mouseAssignment":"Menu"}}]});
        let nested_right = json!({"inputID":"RightClick","isHyperShift":false,"mapping":[{"outputType":"mouseGroup","mouseGroup":{"mouseAssignment":"Click"}}]});
        // There is one derived primary, on RightClick. Source wP sees no
        // top-level Click and keeps it enabled; flattening would disable it.
        assert_eq!(
            state(&[nested_left, nested_right], false)[1]["isEnabled"],
            true
        );
        let mut missing = mouse("LeftClick", "Click", false);
        missing.as_object_mut().unwrap().remove("isHyperShift");
        assert_eq!(state(&[missing], false)[0]["isEnabled"], false);
    }
    #[::core::prelude::v1::test]
    fn source_requirements_are_derived_from_current_capabilities_and_assignment_order() {
        let dpi = json!({"inputID":"RightClick","isHyperShift":false,"outputType":"sensitivityGroup",
            "sensitivityGroup":{"sensitivityAssignment":"DPI_CycleUp"}});
        let rows = derive(229, &buttons(), &buttons(), &[dpi], false, 0).unwrap();
        assert_eq!(rows[1]["isRequireSynapse"], false);
        assert_eq!(rows[1]["assignmentValue"], "CYCLE_UP_SENSITIVITY");
        let turbo = json!({"inputID":"RightClick","isHyperShift":false,"outputType":"mouseGroup",
            "mouseGroup":{"mouseAssignment":"DoubleClick","turboMode":{"isTurbo":true}}});
        let rows = derive(229, &buttons(), &buttons(), &[turbo], false, 0).unwrap();
        assert_eq!(rows[1]["isRequireSynapse"], true);
        let secondary = json!({"inputID":"RightClick","isHyperShift":false,"mapping":[
            {"outputType":"mouseGroup","mouseGroup":{"mouseAssignment":"Click"}},
            {"outputType":"disableGroup"}]});
        let rows = derive(
            229,
            &buttons(),
            &buttons(),
            &[secondary, mouse("RightClick", "Menu", false)],
            false,
            0,
        )
        .unwrap();
        assert_eq!(rows[1]["secondaryAssignment"], "DISABLE");
        assert_eq!(rows[1]["isRequireSynapse"], false);
        let mut originals = buttons();
        originals[1]["isRequireSynapse"] = json!(true);
        assert_eq!(
            derive(229, &originals, &originals, &[], false, 0).unwrap()[1]["isRequireSynapse"],
            false
        );
    }

    #[::core::prelude::v1::test]
    fn firmware_macro_projection_uses_observed_events_and_ble_without_changing_mapping() {
        use razer_model::mapping_requirements::MacroRequirement;
        let macros = MacroRequirement::from_source_catalog(&[
            json!({"guid":"local","appEngine":{"mouseMoveType":"relative"},"macroEvents":[{"Type":1}]}),
        ]).unwrap();
        let mapping = json!({"inputID":"RightClick","isHyperShift":false,"outputType":"macroGroup",
            "macroGroup":{"guid":"local","name":"Local macro","macroPlaybackOption":"Once"}});
        for (bluetooth, expected) in [(None, false), (Some(false), false), (Some(true), true)] {
            let observations = MappingObservations::new(bluetooth, &macros);
            let rows = super::derive(
                235,
                &buttons(),
                &buttons(),
                &[mapping.clone()],
                false,
                0,
                &observations,
            )
            .unwrap();
            assert_eq!(rows[1]["isRequireSynapse"], expected);
            assert_eq!(rows[1]["assignmentValue"], "Local macro");
            assert_eq!(rows[1]["assignmentGroup"], mapping["macroGroup"]);
        }
    }
    #[::core::prelude::v1::test]
    fn source_media_labels_and_synapse_requirement_share_the_mounted_action_owner() {
        for (assignment, label, requires) in [
            ("VolumeUp", "VOLUME_UP", false),
            ("MuteMic", "MUTE_MIC", true),
        ] {
            let mapping = json!({"inputID":"RightClick","isHyperShift":false,"outputType":"multimediaGroup",
                "multimediaGroup":{"multimediaAssignment":assignment}});
            let rows = derive(229, &buttons(), &buttons(), &[mapping.clone()], false, 0).unwrap();
            assert_eq!(rows[1]["assignment"], "MULTIMEDIA");
            assert_eq!(rows[1]["assignmentValue"], label);
            assert_eq!(rows[1]["isRequireSynapse"], requires);
            assert_eq!(rows[1]["assignmentGroup"], mapping["multimediaGroup"]);
        }
    }
    #[::core::prelude::v1::test]
    fn secondary_does_not_replace_primary_and_empty_first_skips_outer_mapping() {
        let nested = json!({"inputID":"RightClick","isHyperShift":false,"mapping":[
            {"outputType":"mouseGroup","mouseGroup":{"mouseAssignment":"Menu"}},
            {"outputType":"mouseGroup","mouseGroup":{"mouseAssignment":"Click"}}]});
        let rows = state(&[nested], false);
        assert_eq!(rows[1]["assignmentValue"], "RIGHT_CLICK");
        assert_eq!(rows[1]["secondaryAssignmentValue"], "LEFT_CLICK");
        assert_eq!(rows[0]["isEnabled"], false);
        let empty = json!({"inputID":"RightClick","isHyperShift":false,"mapping":[
            {"outputType":"mouseGroup"}, {"outputType":"mouseGroup","mouseGroup":{"mouseAssignment":"Click"}}]});
        assert_eq!(state(&[empty], false)[1]["isMapped"], false);
        let missing_output = json!({"inputID":"RightClick","isHyperShift":false});
        assert_eq!(state(&[missing_output], false)[1]["isMapped"], false);
    }
    #[::core::prelude::v1::test]
    fn hypershift_group_and_unresolved_producers_are_explicit() {
        let mapping = json!({"inputID":"LeftClick","isHyperShift":true,"outputType":"hyperShiftGroup","hyperShiftGroup":{}});
        assert_eq!(state(&[mapping], true)[0]["isEnabled"], false);
        let mapping = json!({"inputID":"RightClick","outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_A"}});
        assert_eq!(
            derive(163, &buttons(), &buttons(), &[mapping], false, 0),
            Err(DeriveError::UnresolvedAssignmentProducer(
                "keyboardGroup".into()
            ))
        );
    }

    #[::core::prelude::v1::test]
    fn source_producer_values_preserve_payload_fields_and_null_path() {
        let cases = [
            (
                "launchGroup",
                json!({"path":null,"url":"https://ignored.example"}),
                "LAUNCH_PROGRAM",
                Value::Null,
            ),
            (
                "launchGroup",
                json!({"url":"https://example"}),
                "LAUNCH_PROGRAM",
                json!("https://example"),
            ),
            (
                "aiLauncherGroup",
                json!({"assignment":"PromptComposer","name":"unused"}),
                "AI_LAUNCHER",
                json!("PromptComposer"),
            ),
            ("macroGroup", json!({"name":""}), "MACRO", json!(" ")),
            (
                "textBlockGroup",
                json!({"text":"line1\nline2"}),
                "TEXT_FUNCTION",
                json!("line1\nline2"),
            ),
            (
                "sensitivityGroup",
                json!({"sensitivityAssignment":"DPI_Clutch"}),
                "SENSITIVITY",
                json!("SENSITIVITY_CLUTCH"),
            ),
            ("disableGroup", json!({}), "DISABLE", json!("DISABLE")),
            (
                "hyperShiftGroup",
                json!({}),
                "RAZER_HYPERSHIFT",
                json!("Razer Hypershift"),
            ),
        ];
        for (group, payload, label, value) in cases {
            let mut mapping =
                json!({"inputID":"RightClick","isHyperShift":false,"outputType":group});
            mapping[group] = payload;
            let rows = state(&[mapping], false);
            assert_eq!(rows[1]["assignment"], label);
            assert_eq!(rows[1]["assignmentValue"], value);
        }
    }
}

// Generated catalog reader: razer-pages/mouse_button_state_current_data.
fn read_mouse_button_state_current_catalog() -> Vec<Policy> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Policy>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Policy> {
        let value = match r.tag()? {
            0 => Policy {
                product_id: r.child(decode_2)?,
                category: r.child(decode_3)?,
                has_attachment: r.child(decode_4)?,
                use_cycle_scroll_mode_label: r.child(decode_4)?,
                known_outputs: r.child(decode_5)?,
                verified_producers: r.child(decode_5)?,
                lookups: r.child(decode_6)?,
                source_family: r.child(decode_3)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<String>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_3)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    > {
        let value = match r.tag()? {
            0 => r.map(decode_3, decode_7)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
        let value = match r.tag()? {
            0 => r.map(decode_3, decode_3)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_MOUSE_BUTTON_STATE_CURRENT_DATA,
        0,
        "3136eb39d73bfa50da9d6d6503a3a6fed50c96654b1bcc0940750aec115e566c",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod mouse_button_state_current_data_catalog_tests {
    #[test]
    fn packaged_catalog_mouse_button_state_current_data_decodes() {
        let _ = super::read_mouse_button_state_current_catalog();
    }
}
// End generated catalog reader: razer-pages/mouse_button_state_current_data.
