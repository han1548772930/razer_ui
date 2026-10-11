//! Pure projection of the current non-attachment mouse button-state producer.
//! This observes local mappings; it performs no transport, persistence or device operation.
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
}

fn policy(product_id: u32) -> Result<&'static Policy, DeriveError> {
    static POLICIES: OnceLock<Vec<Policy>> = OnceLock::new();
    POLICIES
        .get_or_init(|| {
            serde_json::from_str(include_str!("mouse_button_state_current_data.json"))
                .expect("validated current mouse button-state policies")
        })
        .iter()
        .find(|policy| policy.product_id == product_id)
        .ok_or(DeriveError::UnverifiedProduct(product_id))
}

/// Returns current `isEnabled`, `isMapped` and assignment fields for the current view.
/// The source caller filters by layer with JS boolean coercion, while the primary
/// helpers intentionally count only mappings whose `isHyperShift` is exactly false.
/// `all_buttons` is the source loader's concatenation of all original view rows.
/// Attachment/New additionally needs other side-panel mappings, so is an explicit gap.
/// `isRequireSynapse` remains outside this projection: it needs the real macro/BLE
/// observations and its source producer; callers must not treat it as newly derived.
pub fn derive(
    product_id: u32,
    buttons: &[Value],
    _all_buttons: &[Value],
    mappings: &[Value],
    hyper: bool,
    view_index: i32,
) -> Result<Vec<Value>, DeriveError> {
    let policy = policy(product_id)?;
    if !matches!(policy.category.as_str(), "MOUSE" | "MOUSEPLUSMAT") {
        return Err(DeriveError::UnsupportedCategory(policy.category.clone()));
    }
    if policy.has_attachment {
        return Err(DeriveError::AttachmentObservationRequired);
    }
    let mut rows = buttons.to_vec();
    for row in &mut rows {
        let object = row.as_object_mut().ok_or(DeriveError::InvalidButton)?;
        object.insert("isMapped".into(), json!(false));
        object.insert("isEnabled".into(), json!(true));
        if let Some(value) = object.get("defaultValue").cloned() {
            object.insert("assignmentValue".into(), value);
        } else {
            object.remove("assignmentValue");
        }
        for key in [
            "assignmentGroup",
            "secondaryAssignment",
            "secondaryAssignmentValue",
            "secondaryAssignmentGroup",
        ] {
            object.remove(key);
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
        if let Some(parts) = mapping.get("mapping").filter(|parts| truthy(parts)) {
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
                assign(&mut rows[index], part, part_index != 0, hyper, policy)?;
            }
        } else {
            assign(&mut rows[index], mapping, false, hyper, policy)?;
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
) -> Result<(), DeriveError> {
    let Some(group) = mapping["outputType"].as_str() else {
        return Ok(());
    };
    if group == "hyperShiftGroup" && hyper {
        row["isEnabled"] = json!(false);
    }
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
        "disableGroup" => ("DISABLE", Some(json!("DISABLE"))),
        "hyperShiftGroup" => ("RAZER_HYPERSHIFT", Some(json!("Razer Hypershift"))),
        "mouseGroup" | "sensitivityGroup" => {
            let key = if group == "mouseGroup" {
                "mouseAssignment"
            } else {
                "sensitivityAssignment"
            };
            let payload = payload(mapping, group)?;
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
        | "multimediaGroup"
        | "win8ShortcutsGroup"
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
    #[test]
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
    #[test]
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
    #[test]
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
    #[test]
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

    #[test]
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
