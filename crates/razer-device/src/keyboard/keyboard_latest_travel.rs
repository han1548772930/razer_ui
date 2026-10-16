//! Current6737 modules4368/18065/88758. Limits are restored from the exact
//! connected serial's local switch cache; they are not hardware read results.
use super::*;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
struct Cap {
    key_id: u8,
    maximum: f64,
}
#[derive(Clone, Debug, Default)]
pub struct Caps {
    active: bool,
    keys: Vec<(String, Cap)>,
}
impl Caps {
    pub fn for_serial(document: &Value, serial: &str) -> anyhow::Result<Self> {
        ensure!(
            !serial.is_empty(),
            "Magnetic travel requires the actual connected serial"
        );
        let mut result = Self {
            active: true,
            ..Default::default()
        };
        let layout = document["_source_layout_id"].as_u64();
        let entries = document["magneticKeySwitchTypes"]
            .as_array()
            .and_then(|rows| rows.iter().find(|row| row["serialNumber"] == serial))
            .and_then(|row| row["switchTypes"].as_array());
        for entry in entries.into_iter().flatten() {
            let Some(id) = integer(&entry["keyId"]).filter(|id| (1..255).contains(id)) else {
                continue;
            };
            let Some(travel) =
                integer(&entry["keyTravel"]).filter(|travel| (1..40).contains(travel))
            else {
                continue;
            };
            let input = if id == 59 {
                Some("KEY_FN".to_owned())
            } else if id == 42 && layout == Some(12) {
                Some("KEY_BACKSLASH".to_owned())
            } else {
                crate::mapping_engine::source_keys(756)
                    .into_iter()
                    .flatten()
                    .find(|key| u64::from(key.analog_key_id) == id)
                    .and_then(|key| key.input_id)
                    .map(|input| {
                        if input == "KEY_LEFT_GUI" {
                            "KEY_WINDOWS".to_owned()
                        } else {
                            input.to_owned()
                        }
                    })
            };
            if let Some(input) = input {
                let cap = Cap {
                    key_id: id as u8,
                    maximum: travel as f64 * 1638.,
                };
                if let Some((_, old)) = result.keys.iter_mut().find(|(key, _)| key == &input) {
                    *old = cap;
                } else {
                    result.keys.push((input, cap));
                }
            }
        }
        Ok(result)
    }
    pub(super) fn rows(&self, rows: &[Value], guid: &str) -> Vec<Value> {
        let mut result = rows.to_vec();
        if !self.active {
            return result;
        }
        for row in &mut result {
            if row["inputType"] != "AnalogInput" {
                continue;
            }
            let cap = self
                .keys
                .iter()
                .find(|(input, _)| row["inputID"] == *input)
                .map(|(_, cap)| cap);
            let Some(stages) = row["mapping"].as_array_mut() else {
                continue;
            };
            for stage in stages.iter_mut() {
                if let Some(cap) = cap {
                    if let Some(actuation) = stage.get_mut("actuationPoint") {
                        clamp_actuation(actuation, cap.maximum);
                    }
                    for (top, bottom) in [
                        (
                            "actuationDownPressTopValue",
                            "actuationDownPressBottomValue",
                        ),
                        ("actuationUpPressTopValue", "actuationUpPressBottomValue"),
                    ] {
                        let Some(group) = stage.get_mut("dynamicKeyStrokeGroup") else {
                            continue;
                        };
                        if let (Some(t), Some(b)) = (group[top].as_f64(), group[bottom].as_f64()) {
                            if t > cap.maximum || b > cap.maximum {
                                let new_bottom = b.min(cap.maximum);
                                group[top] = json!(t.min((new_bottom - 1638.).max(1638.)));
                                group[bottom] = json!(new_bottom);
                            }
                        }
                    }
                }
                if let Some(group) = stage.get_mut("controllerGroup") {
                    clamp_curve(
                        group,
                        cap.map(|cap| cap.maximum).unwrap_or(65520.),
                        cap.map(|cap| cap.maximum).unwrap_or(65520.),
                    );
                }
            }
            if let Some(cap) = cap {
                if stages.len() > 1
                    && ordinary(&stages[0])
                    && ordinary(&stages[1])
                    && point(&stages[1]["actuationPoint"], 0) == Some(cap.maximum)
                {
                    let maximum = cap.maximum - 16380.;
                    if maximum >= 1638. {
                        clamp_actuation(&mut stages[0]["actuationPoint"], maximum);
                    }
                }
            }
        }
        for (input, cap) in &self.keys {
            if input == "KEY_FN"
                || result
                    .iter()
                    .any(|row| row["inputID"] == *input && !truthy(&row["isHyperShift"]))
            {
                continue;
            }
            let value = per_key(
                defaults(guid, "preset_actuation", "default_actuation"),
                input,
            );
            let mut actuation = json!({"0":value["thresholdL"],"1":value["thresholdH"]});
            let before = actuation.clone();
            clamp_actuation(&mut actuation, cap.maximum);
            if before != actuation {
                result.push(json!({"inputID":input,"inputType":"AnalogInput","isHyperShift":false,"mapping":[{"outputType":"keyboardGroup","keyboardGroup":{},"actuationPoint":actuation},{"outputType":"defaultGroup","actuationPoint":actuation}]}));
            }
        }
        result
    }
    pub(super) fn engine(&self, mappings: &mut [Value]) -> anyhow::Result<()> {
        let factory = MappingEngine::new(756)?;
        let mut caps = BTreeMap::new();
        for (input, cap) in &self.keys {
            let input = if input == "KEY_WINDOWS" {
                "KEY_LEFT_GUI"
            } else {
                input
            };
            if let Ok(key) = factory.source_key_metadata(input) {
                if let Some(code) = key["scancode"].as_u64() {
                    caps.insert(code, cap.maximum);
                }
            }
        }
        for mapping in mappings {
            if mapping["input"]["type"] != "analogKey" {
                continue;
            }
            let Some(maximum) = mapping["input"]["scancode"]
                .as_u64()
                .and_then(|code| caps.get(&code))
            else {
                continue;
            };
            let output = if mapping["output"]["type"] == "doubleMapping" {
                &mut mapping["output"]["analog"]
            } else {
                &mut mapping["output"]
            };
            if output["type"] != "analogJoystickV2" {
                continue;
            }
            let Some(actuations) = output["actuations"].as_object_mut() else {
                continue;
            };
            let positive = actuations
                .keys()
                .filter(|key| {
                    key.parse::<f64>()
                        .is_ok_and(|value| value.is_finite() && value > 0.)
                })
                .cloned()
                .collect::<Vec<_>>();
            if positive.len() == 1 && positive[0].parse::<f64>().ok() != Some(*maximum) {
                if let Some(value) = actuations.remove(&positive[0]) {
                    actuations.insert((*maximum as u32).to_string(), value);
                }
            }
        }
        Ok(())
    }
    pub fn analog_pair(&self, pair: &mut AnalogPair) {
        for point in &mut pair.actuation {
            let Some((_, cap)) = self.keys.iter().find(|(_, cap)| cap.key_id == point.key_id)
            else {
                continue;
            };
            let mut values = json!([point.threshold_low, point.threshold_high]);
            clamp_actuation(&mut values, cap.maximum);
            point.threshold_low = values[0].as_f64().unwrap() as u16;
            point.threshold_high = values[1].as_f64().unwrap() as u16;
        }
    }
}
fn integer(value: &Value) -> Option<u64> {
    value
        .as_f64()
        .filter(|value| {
            value.is_finite() && *value >= 0. && value.fract() == 0. && *value <= u64::MAX as f64
        })
        .map(|value| value as u64)
}
fn point(value: &Value, index: usize) -> Option<f64> {
    value
        .get(index)
        .or_else(|| value.get(index.to_string()))
        .and_then(Value::as_f64)
}
fn clamp_actuation(value: &mut Value, maximum: f64) {
    let (Some(make), Some(release)) = (point(value, 0), point(value, 1)) else {
        return;
    };
    if make <= maximum && release <= maximum {
        return;
    }
    let make = make.min(maximum);
    let release = release.min((make - 1638.).max(256.));
    if value.is_array() {
        value[0] = json!(make);
        value[1] = json!(release);
    } else {
        value["0"] = json!(make);
        value["1"] = json!(release);
    }
}
fn ordinary(stage: &Value) -> bool {
    truthy(&stage["outputType"])
        && stage["outputType"] != "defaultGroup"
        && stage["outputType"] != "dynamicKeyStrokeGroup"
        && !truthy(&stage["controllerGroup"]["controllerMode"]["isAnalog"])
        && !(truthy(&stage["joystickGroup"]["isJoystickMovement"])
            || matches!(
                stage["joystickGroup"]["joystickMode"].as_str(),
                Some("axes" | "raxes")
            ))
}
fn clamp_curve(group: &mut Value, maximum: f64, previous: f64) {
    if !truthy(&group["controllerMode"]["isAnalog"]) {
        return;
    }
    let minimum = 0.1;
    let maximum = maximum / 1638. / 10.;
    let Some(points) = group["analogSensitivityList"]["analogSensitivityAssignment"].as_array()
    else {
        return;
    };
    if points.is_empty()
        || maximum <= minimum
        || points
            .iter()
            .any(|point| point["x"].as_f64().is_none_or(|x| x < minimum) || !point["y"].is_number())
    {
        return;
    }
    let range = &group["analogSensitivityList"]["travelRange"];
    let valid = range["minimum"]
        .as_f64()
        .zip(range["maximum"].as_f64())
        .filter(|(min, max)| max > min);
    let (old_min, old_max) = valid.unwrap_or((
        minimum,
        points
            .iter()
            .filter_map(|point| point["x"].as_f64())
            .fold(previous / 1638. / 10., f64::max),
    ));
    if old_max <= old_min
        || points.iter().any(|point| {
            point["x"]
                .as_f64()
                .is_some_and(|x| x < old_min || x > old_max)
        })
    {
        return;
    }
    let mut result = points.clone();
    for point in &mut result {
        let x = point["x"].as_f64().unwrap();
        let u = point["u"]
            .as_f64()
            .filter(|u| {
                valid.is_some()
                    && (0.0..=1.0).contains(u)
                    && (x - (old_min + u * (old_max - old_min))).abs() <= 1e-12
            })
            .unwrap_or((x - old_min) / (old_max - old_min));
        point["x"] = json!(if u == 1. {
            maximum
        } else {
            minimum + u * (maximum - minimum)
        });
        point["u"] = json!(u);
    }
    group["analogSensitivityList"]["travelRange"] = json!({"minimum":minimum,"maximum":maximum});
    group["analogSensitivityList"]["analogSensitivityAssignment"] = json!(result);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serial_caps_bound_both_stages_before_hardware_primary_is_selected() {
        let key = MappingEngine::new(756)
            .unwrap()
            .source_key_metadata("KEY_A")
            .unwrap()["analogKeyID"]
            .as_u64()
            .unwrap();
        let document = json!({"_source_layout_id":0,"magneticKeySwitchTypes":[
            {"serialNumber":"other","switchTypes":[{"keyId":key,"keyTravel":10}]},
            {"serialNumber":"actual","switchTypes":[{"keyId":key as f64,"keyTravel":20.0}]}]});
        let caps = Caps::for_serial(&document, "actual").unwrap();
        let row = json!({"inputID":"KEY_A","inputType":"AnalogInput","isHyperShift":false,"mapping":[
            {"outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_B"},"actuationPoint":[60000,59000]},
            {"outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_C"},"actuationPoint":[65520,63882]}]});
        let bounded = caps.rows(&[row.clone()], "custom");
        assert_eq!(
            bounded[0]["mapping"][0]["actuationPoint"],
            json!([16380.0, 14742.0])
        );
        assert_eq!(
            bounded[0]["mapping"][1]["actuationPoint"],
            json!([32760.0, 31122.0])
        );
        let pair = analog_pair_with_caps(1, "custom", &[], &[row.clone()], &caps).unwrap();
        let primary = pair
            .actuation
            .iter()
            .find(|point| point.key_id == key as u8)
            .unwrap();
        assert_eq!(
            (primary.threshold_low, primary.threshold_high),
            (16380, 14742)
        );
        assert_eq!(row["mapping"][0]["actuationPoint"], json!([60000, 59000]));
        let other = Caps::for_serial(&document, "other").unwrap();
        assert_ne!(
            other.rows(&[row], "custom")[0]["mapping"][1]["actuationPoint"],
            bounded[0]["mapping"][1]["actuationPoint"]
        );
    }

    #[test]
    fn curve_reprojection_preserves_relative_points_across_travel_changes() {
        let mut group = json!({"controllerMode":{"isAnalog":true},"analogSensitivityList":{"analogSensitivityAssignment":[{"x":0.1,"y":0},{"x":2.05,"y":128},{"x":4.0,"y":255}]}});
        clamp_curve(&mut group, 32760., 65520.);
        let rows = group["analogSensitivityList"]["analogSensitivityAssignment"]
            .as_array()
            .unwrap();
        assert_eq!(rows[0]["x"], 0.1);
        assert!((rows[1]["x"].as_f64().unwrap() - 1.05).abs() < 1e-12);
        assert_eq!(rows[2]["x"], 2.0);
        assert_eq!(rows[2]["u"], 1.0);
        clamp_curve(&mut group, 65520., 32760.);
        let rows = group["analogSensitivityList"]["analogSensitivityAssignment"]
            .as_array()
            .unwrap();
        assert!((rows[1]["x"].as_f64().unwrap() - 2.05).abs() < 1e-12);
        assert_eq!(rows[2]["x"], 4.0);
    }
}
