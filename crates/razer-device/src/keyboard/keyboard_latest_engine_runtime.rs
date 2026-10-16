//! Current6737 generic61450 gamepad-configuration engine passes and4753
//! Snap Tap decoration. These transform engine inputs, never the saved rows.
use super::*;

fn stages(row: &Value) -> &[Value] {
    row["mapping"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
}
fn contains(row: &Value, kind: &str) -> bool {
    stages(row).iter().any(|stage| stage["outputType"] == kind)
}
fn disabled(row: &Value, retain_mapping: bool) -> Value {
    let mut row = row.clone();
    row["inputType"] = json!("KeyInput");
    row["outputType"] = json!("disableGroup");
    row["disableGroup"] = json!({"disableGroup":{"disableAssignment":"Disable"}});
    if !retain_mapping {
        row.as_object_mut().unwrap().remove("mapping");
    }
    row
}
fn flatten(row: &Value) -> Value {
    let mut result = row.clone();
    result["inputType"] = json!("KeyInput");
    if let Some(primary) = stages(row).first().and_then(Value::as_object) {
        result.as_object_mut().unwrap().extend(primary.clone());
    }
    result.as_object_mut().unwrap().remove("mapping");
    result
}
fn enabled(rows: &[Value]) -> Vec<Value> {
    let mut result = rows
        .iter()
        .filter(|row| contains(row, "controllerGroup"))
        .cloned()
        .collect::<Vec<_>>();
    for row in rows.iter().filter(|row| !contains(row, "controllerGroup")) {
        let stages = stages(row);
        let is_analog = row["inputType"] == "AnalogInput";
        let special = ["joystickGroup", "controllerGroup", "dynamicKeyStrokeGroup"]
            .iter()
            .any(|kind| contains(row, kind));
        let keyboard_mouse = stages
            .iter()
            .filter(|stage| {
                matches!(
                    stage["outputType"].as_str(),
                    Some("keyboardGroup" | "mouseGroup")
                )
            })
            .count();
        if is_analog && stages.len() > 1 && special {
            result.extend([row.clone(), disabled(row, false)]);
            continue;
        }
        let multiple_keyboard = is_analog
            && !stages.is_empty()
            && stages
                .iter()
                .filter(|stage| stage["outputType"] == "keyboardGroup")
                .count()
                > 1;
        let joystick_dks = is_analog
            && !stages.is_empty()
            && ["joystickGroup", "dynamicKeyStrokeGroup"]
                .iter()
                .any(|kind| contains(row, kind));
        if is_analog
            && !stages.is_empty()
            && keyboard_mouse <= 1
            && !multiple_keyboard
            && !joystick_dks
        {
            result.push(flatten(row));
        } else if is_analog && !stages.is_empty() && keyboard_mouse > 1
            || multiple_keyboard
            || joystick_dks
        {
            result.push(if is_analog && !stages.is_empty() && keyboard_mouse > 1 {
                flatten(row)
            } else {
                row.clone()
            });
            result.push(disabled(row, false));
        } else {
            result.push(row.clone());
        }
    }
    result
}
fn deduplicate(rows: Vec<Value>) -> Vec<Value> {
    let mut result = Vec::new();
    for row in rows {
        if !result.contains(&row) {
            result.push(row);
        }
    }
    result
}
/// The source invokes this pass for the active profile only. Inactive engines
/// retain the ordinary firmware-filtered preparation until they are selected.
pub(super) fn gamepad_rows(profile: &Value) -> anyhow::Result<Option<Vec<Value>>> {
    let Some(is_enabled) = profile["isEnableKeyboardKeys"]
        .as_bool()
        .or_else(|| profile["isGamepadOverride"].as_bool().map(|_| true))
    else {
        return Ok(None);
    };
    let rows = active_mappings(profile)?;
    if is_enabled {
        let mut result = enabled(rows);
        if profile["isGamepadOverride"] == true {
            result.extend(
                rows.iter()
                    .filter(|row| {
                        contains(row, "controllerGroup")
                            && row["inputType"] == "AnalogInput"
                            && stages(row).len() > 1
                    })
                    .map(|row| disabled(row, true)),
            );
        }
        return Ok(Some(deduplicate(result)));
    }
    let mut result = rows
        .iter()
        .filter(|row| truthy(&row["isHyperShift"]) && contains(row, "dynamicKeyStrokeGroup"))
        .cloned()
        .collect::<Vec<_>>();
    let hypershift = result.iter().filter(|row| row["inputType"] == "AnalogInput" && stages(row).len() > 1).map(|row| json!({"inputID":row["inputID"],"inputType":"KeyInput","isHyperShift":true,"outputType":"disableGroup","disableGroup":{"disableGroup":{"disableAssignment":"Disable"}}})).collect::<Vec<_>>();
    result.extend(hypershift);
    for row in rows.iter().filter(|row| contains(row, "controllerGroup")) {
        let mut row = row.clone();
        if stages(&row)
            .iter()
            .any(|stage| stage["outputType"] != "defaultGroup")
        {
            let actuation = stages(&row)
                .iter()
                .skip(1)
                .find_map(|stage| stage.get("actuationPoint"))
                .cloned()
                .unwrap_or_else(|| json!({"0":19656,"1":18018}));
            let controller = stages(&row)
                .iter()
                .find(|stage| stage["outputType"] == "controllerGroup")
                .cloned()
                .unwrap();
            row["mapping"] = json!([controller,{"actuationPoint":actuation,"isDefault":false,"outputType":"defaultGroup","isHyperShift":false}]);
        }
        result.push(row);
    }
    for row in source()["full_mapping_list"]
        .as_array()
        .context("756 source FULLMAPPINGLIST is absent")?
        .iter()
        .filter(|row| row["inputType"] == "AnalogInput" && row["isHyperShift"] == false)
    {
        let mut row = row.clone();
        row["inputType"] = json!("KeyInput");
        row["outputType"] = json!("disableGroup");
        row["disableGroup"] = json!({"disableAssignment":"Disable"});
        row.as_object_mut().unwrap().remove("mapping");
        result.push(row);
    }
    result.extend(
        rows.iter()
            .filter(|row| {
                matches!(
                    row.get("outputType")
                        .filter(|value| !value.is_null())
                        .unwrap_or(&row["mapping"][0]["outputType"])
                        .as_str(),
                    Some("profileNavigationGroup" | "keymapGroup")
                )
            })
            .cloned(),
    );
    Ok(Some(result))
}

pub(super) fn snap_tap(
    factory: &MappingEngine,
    profile: &mut Value,
    defaults: &Value,
    context: &EngineContext,
) -> anyhow::Result<()> {
    if !truthy(&profile["snapTap"]["isEnabled"]) {
        return Ok(());
    }
    let pairs = profile["snapTap"]["keyList"]
        .as_array()
        .context("756 Snap Tap pairs are absent")?
        .clone();
    let multi = profile["functionKeyPrimary"] == "multi";
    let mappings = profile["appEngine"]["mappings"]
        .as_array_mut()
        .context("756 Snap Tap engine mappings are absent")?;
    for pair in pairs {
        for (field, value) in pair
            .as_object()
            .context("756 Snap Tap pair is not an object")?
        {
            if field == "id" {
                continue;
            }
            let Some(name) = value.as_str() else { break };
            let dkm = context.dkm_keys.get(name);
            let key = match dkm {
                Some(key) => key.clone(),
                None => match factory.source_key_metadata(name) {
                    Ok(key) => key,
                    Err(_) => break,
                },
            };
            let flag = key
                .get("outputFlag")
                .unwrap_or(&key["flag"])
                .as_u64()
                .unwrap_or(0);
            let matches = |mapping: &Value| {
                let input = &mapping["input"];
                (truthy(&input["scancode"])
                    && !truthy(&input["hypershift"])
                    && input["scancode"] == key["scancode"]
                    && input["flag"]
                        .as_u64()
                        .is_some_and(|value| value == flag || value == flag + 1))
                    || (truthy(&input["key"]) && input["key"] == key["key"])
            };
            let found = mappings.iter().any(matches);
            let mut copied = false;
            if multi {
                for default in defaults["mappings"]
                    .as_array()
                    .context("756 Snap Tap default mappings are absent")?
                {
                    let input = &default["input"];
                    if input["scancode"] == key["scancode"]
                        && input["flag"]
                            .as_u64()
                            .is_some_and(|value| value == flag || value == flag + 1)
                    {
                        copied = true;
                        if !mappings.iter().any(|mapping| {
                            ["type", "scancode", "flag", "hypershift"]
                                .iter()
                                .all(|field| mapping["input"][field] == input[field])
                        }) {
                            mappings.push(default.clone());
                        }
                    }
                }
            }
            if found || multi {
                for mapping in mappings.iter_mut().filter(|mapping| matches(mapping)) {
                    mapping["input"]["snaptapId"] = pair["id"].clone();
                }
            }
            if !found && (!multi || !copied) {
                let key_field = if dkm.is_some() { "key" } else { "scancode" };
                for flag in [flag, flag + 1] {
                    let mut input = json!({"type":if dkm.is_some(){"razerKey"}else{"analogKey"},"flag":flag,"snaptapId":pair["id"]});
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
