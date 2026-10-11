//! Current shape onMouseUp / module3342.z guards and complete-list merge.
//! Receipt: keyboard-controller-drop-current-source.json. No device success is inferred.
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::OnceLock;

#[derive(Deserialize)]
struct Policy {
    product_id: u32,
    excluded_inputs: Vec<String>,
    unsupported_mapping_inputs: Vec<String>,
    hyper_excluded_buttons: Vec<String>,
    controller_choices: Vec<String>,
}
fn policy(pid: u32) -> Option<&'static Policy> {
    static DATA: OnceLock<Vec<Policy>> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("keyboard_controller_drop_data.json"))
            .expect("current controller drop policies")
    })
    .iter()
    .find(|p| p.product_id == pid)
}

fn occupied(list: &[Value], assignment: &str, shift: Option<bool>) -> bool {
    list.iter().any(|entry| {
        (match shift {
            Some(shift) => entry["isHyperShift"] == shift,
            None => entry.get("isHyperShift").is_none(),
        }) && entry["mapping"].as_array().is_some_and(|slots| {
            slots.iter().take(2).any(|slot| {
                slot["outputType"].as_str().is_some_and(|group| {
                    slot[group]["controllerAssignment"].as_str() == Some(assignment)
                })
            })
        })
    })
}

fn point_zero(points: &Value) -> &Value {
    if points.is_array() {
        &points[0]
    } else {
        &points["0"]
    }
}

/// Customize caller has no system FKP, arrow/function-key, valid-counter or
/// actuation-tab props. Keep the mounted MapKeyboard predicate on its button
/// state; the source button-state producer remains responsible for assignment.
pub(super) fn not_remapped(pid: u32, button: &Value, shift: bool) -> Option<bool> {
    let policy = policy(pid)?;
    let input = button["inputID"].as_str()?;
    Some(
        (button["isSidePanelList"] == false && button["buttonKey"] != "DKM_KEYPAD_JOYSTICK")
            || policy
                .unsupported_mapping_inputs
                .iter()
                .any(|id| id == input)
            || ["DKM_SB_65", "DKM_SB_67", "DKM_SB_66"].contains(&input)
            || (shift && button["hasHWFunc"] == true)
            || (shift
                && button["disableHypershiftMapping"] == true
                && button["assignment"] != "RAZER_HYPERSHIFT")
            || button["assignmentValue"] == "KEY_WINDOWS",
    )
}

/// `drag` is the source {data, dragCustomize?, originalButton?} object.
/// Both source duplicate checks are retained: the second omits the layer argument.
pub(super) fn merge(
    pid: u32,
    list: &[Value],
    target: &Value,
    not_remapped: bool,
    shift: bool,
    drag: &Value,
    default_points: &Value,
) -> Option<Vec<Value>> {
    let policy = policy(pid)?;
    let input = target["inputID"].as_str()?;
    let input_type = target["inputType"].as_str()?;
    let slots = drag["data"].as_array()?;
    let first = slots.first()?;
    let original = drag["originalButton"].as_str();
    let moving = drag["dragCustomize"] == true;
    let excluded = |id: &str| policy.excluded_inputs.iter().any(|item| item == id);
    if not_remapped || excluded(input) || original.is_some_and(excluded) || original == Some(input)
    {
        return None;
    }
    let controller = first["controllerGroup"]["controllerAssignment"].as_str();
    if !moving
        && controller.is_some_and(|assignment| {
            policy
                .controller_choices
                .iter()
                .any(|choice| choice == assignment)
                && (occupied(list, assignment, Some(shift)) || occupied(list, assignment, None))
        })
    {
        return None;
    }
    if first["outputType"] == "hyperShiftGroup"
        && target["buttonKey"]
            .as_str()
            .is_some_and(|key| policy.hyper_excluded_buttons.iter().any(|item| item == key))
    {
        return None;
    }
    let replacement =
        json!({"mapping":slots,"isHyperShift":shift,"inputType":input_type,"inputID":input});
    let mut out = list.to_vec();
    if let Some(index) = out.iter().position(|entry| {
        entry["inputID"] == input
            && entry["inputType"] == input_type
            && entry["isHyperShift"] == shift
    }) {
        out[index] = replacement;
    } else {
        out.push(replacement);
    }
    if moving {
        let original = original?;
        if let Some(index) = out
            .iter()
            .position(|entry| entry["inputID"] == original && entry["isHyperShift"] == shift)
        {
            let points = out[index]["mapping"][0]["actuationPoint"].clone();
            if point_zero(&points) != point_zero(default_points) {
                out[index]["mapping"][0] = json!({"actuationPoint":points,"outputType":"keyboardGroup","keyboardGroup":{}});
            } else {
                out.remove(index);
            }
        }
        if first["outputType"] == "hyperShiftGroup" {
            if let Some(entry) = list
                .iter()
                .find(|entry| entry["inputID"] == original && entry["isHyperShift"] != shift)
            {
                let mut opposite = entry.clone();
                opposite["inputID"] = json!(input);
                if let Some(index) = out
                    .iter()
                    .position(|entry| entry["inputID"] == input && entry["isHyperShift"] != shift)
                {
                    opposite["mapping"][0] = json!({"actuationPoint":opposite["mapping"][0]["actuationPoint"],"outputType":"hyperShiftGroup"});
                    out[index] = opposite;
                } else {
                    out.push(opposite);
                }
                if let Some(index) = out.iter().position(|entry| {
                    entry["inputID"] == original && entry["isHyperShift"] != shift
                }) {
                    out.remove(index);
                }
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::{merge, not_remapped};
    use serde_json::{Value, json};
    fn target() -> Value {
        json!({"inputID":"KEY_A","inputType":"AnalogInput","buttonKey":"KEY_A"})
    }
    fn drag() -> Value {
        json!({"data":[{"outputType":"controllerGroup","controllerGroup":{"controllerAssignment":"A_BUTTON"},"actuationPoint":{"0":19656,"1":18018}}]})
    }
    #[test]
    fn controller_drop_not_remapped_preserves_source_exemptions() {
        let mut button = target();
        button["isSidePanelList"] = json!(false);
        assert_eq!(not_remapped(679, &button, false), Some(true));
        button["buttonKey"] = json!("DKM_KEYPAD_JOYSTICK");
        assert_eq!(not_remapped(679, &button, false), Some(false));
        button["disableHypershiftMapping"] = json!(true);
        assert_eq!(not_remapped(679, &button, true), Some(true));
        button["assignment"] = json!("RAZER_HYPERSHIFT");
        assert_eq!(not_remapped(679, &button, true), Some(false));
        button["assignmentValue"] = json!("Windows");
        assert_eq!(not_remapped(679, &button, true), Some(false));
        button["assignmentValue"] = json!("KEY_WINDOWS");
        assert_eq!(not_remapped(679, &button, true), Some(true));
    }
    #[test]
    fn controller_drop_checks_current_secondary_and_omitted_layer() {
        for layer in [Some(false), None] {
            let mut entry = json!({"inputID":"KEY_B","mapping":[{},drag()["data"][0]]});
            if let Some(layer) = layer {
                entry["isHyperShift"] = json!(layer);
            }
            assert!(
                merge(
                    679,
                    &[entry],
                    &target(),
                    false,
                    false,
                    &drag(),
                    &json!({"0":19656})
                )
                .is_none()
            );
        }
        let entry = json!({"inputID":"KEY_B","isHyperShift":true,"mapping":drag()["data"]});
        assert!(
            merge(
                679,
                &[entry],
                &target(),
                false,
                false,
                &drag(),
                &json!({"0":19656})
            )
            .is_some()
        );
        // The omitted argument is undefined in JS; explicit null is different.
        let entry = json!({"inputID":"KEY_B","isHyperShift":null,"mapping":drag()["data"]});
        assert!(
            merge(
                679,
                &[entry],
                &target(),
                false,
                false,
                &drag(),
                &json!({"0":19656})
            )
            .is_some()
        );
    }
    #[test]
    fn controller_drop_keeps_list_and_custom_actuation_when_moving() {
        let mut source_drag = drag();
        source_drag["dragCustomize"] = json!(true);
        source_drag["originalButton"] = json!("KEY_B");
        let entry = json!({"inputID":"KEY_B","inputType":"AnalogInput","isHyperShift":false,"mapping":[{"outputType":"controllerGroup","actuationPoint":{"0":23000,"1":21000}}]});
        let out = merge(
            678,
            &[entry],
            &target(),
            false,
            false,
            &source_drag,
            &json!({"0":19656}),
        )
        .unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(
            out[0]["mapping"][0],
            json!({"actuationPoint":{"0":23000,"1":21000},"outputType":"keyboardGroup","keyboardGroup":{}})
        );
        assert_eq!(out[1]["mapping"], source_drag["data"]);
    }
    #[test]
    fn controller_drop_rejects_forbidden_hypershift_and_self_move() {
        let mut button = target();
        button["buttonKey"] = json!("KEY_RIGHT_SHIFT");
        let hyper = json!({"data":[{"outputType":"hyperShiftGroup"}]});
        assert!(merge(688, &[], &button, false, false, &hyper, &Value::Null).is_none());
        assert!(merge(679, &[], &button, false, false, &hyper, &Value::Null).is_some());
        let mut moving = drag();
        moving["originalButton"] = json!("KEY_A");
        moving["dragCustomize"] = json!(true);
        assert!(merge(679, &[], &target(), false, false, &moving, &Value::Null).is_none());
    }
}
