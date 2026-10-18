//! Controller feature records are added by the current middleware after the
//! initial UI profile is created. Validate them at the persistence boundary
//! without treating saved command logs as device acknowledgements.
use super::GamepadProductSpec;
use anyhow::ensure;
use razer_device::controller_triggers::{AnalogTrigger, DigitalTrigger, ThumbstickValue};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnalogActuation {
    controller_id: u8,
    start_range: u8,
    end_range: u8,
    is_active: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnalogActuations {
    analog_actuations: Vec<AnalogActuation>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DigitalActuation {
    controller_id: u8,
    actuation_point: u8,
    is_rapid_trigger: bool,
    is_active: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DigitalActuations {
    digital_actuations: Vec<DigitalActuation>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StickValue {
    controller_id: u8,
    value: u16,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Deadzone {
    deadzones: Vec<StickValue>,
    #[serde(rename = "preventDoubleDeadzone")]
    _prevent_double_deadzone: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Sensitivity {
    sensitivity_values: Vec<StickValue>,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Circularity {
    Standard,
    Circular,
}

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Features {
    obm_slot_id: Option<Vec<u8>>,
    controller_analog_actuation: Option<AnalogActuations>,
    controller_digital_actuation: Option<DigitalActuations>,
    controller_deadzone: Option<Deadzone>,
    controller_sensitivity: Option<Sensitivity>,
    controller_analog_circularity: Option<Circularity>,
    controller_lighting_effects: Option<super::lighting_state::ControllerLightingSettings>,
    quick_effects: Option<super::quick_lighting_state::QuickLightingSettings>,
}

fn validate_ids(ids: impl Iterator<Item = u8>) -> anyhow::Result<()> {
    let mut present = [false; 2];
    for id in ids {
        ensure!(matches!(id, 1 | 2), "Invalid saved controller side");
        let used = &mut present[usize::from(id - 1)];
        ensure!(!*used, "Duplicate saved controller side");
        *used = true;
    }
    Ok(())
}

impl Features {
    fn validate(&self, spec: &GamepadProductSpec) -> anyhow::Result<()> {
        let supported = razer_device::controller_triggers::capability(spec.product_id).is_some();
        ensure!(
            spec.product_id == 2636 || self.controller_lighting_effects.is_none(),
            "Saved controller preset effects do not belong to this product"
        );
        ensure!(
            spec.product_id == 4115 || self.quick_effects.is_none(),
            "Saved quick effects do not belong to this product"
        );
        ensure!(
            supported
                || self.controller_analog_actuation.is_none()
                    && self.controller_digital_actuation.is_none(),
            "Saved trigger settings do not belong to this product"
        );
        ensure!(
            supported
                || self.controller_deadzone.is_none()
                    && self.controller_sensitivity.is_none()
                    && self.controller_analog_circularity.is_none(),
            "Saved thumbstick settings do not belong to this product"
        );
        if let Some(settings) = &self.controller_analog_actuation {
            validate_ids(settings.analog_actuations.iter().map(|s| s.controller_id))?;
            for s in &settings.analog_actuations {
                AnalogTrigger::new(s.controller_id, s.start_range, s.end_range, s.is_active)?;
            }
        }
        if let Some(settings) = &self.controller_digital_actuation {
            validate_ids(settings.digital_actuations.iter().map(|s| s.controller_id))?;
            for s in &settings.digital_actuations {
                DigitalTrigger::new(
                    s.controller_id,
                    s.actuation_point,
                    s.is_rapid_trigger,
                    s.is_active,
                )?;
            }
        }
        let values = self
            .controller_deadzone
            .as_ref()
            .map(|settings| settings.deadzones.as_slice())
            .into_iter()
            .chain(
                self.controller_sensitivity
                    .as_ref()
                    .map(|settings| settings.sensitivity_values.as_slice()),
            );
        for values in values {
            validate_ids(values.iter().map(|s| s.controller_id))?;
            for s in values {
                ThumbstickValue::new(s.controller_id, s.value)?;
            }
        }
        Ok(())
    }
}

pub(super) fn restore_features(
    profile: &mut Value,
    saved: &Value,
    spec: &GamepadProductSpec,
) -> anyhow::Result<()> {
    let features: Features = serde_json::from_value(saved.clone())?;
    features.validate(spec)?;
    let target = profile
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("Active controller profile is missing"))?;
    for (key, present) in [
        ("obmSlotId", features.obm_slot_id.is_some()),
        (
            "controllerAnalogActuation",
            features.controller_analog_actuation.is_some(),
        ),
        (
            "controllerDigitalActuation",
            features.controller_digital_actuation.is_some(),
        ),
        ("controllerDeadzone", features.controller_deadzone.is_some()),
        (
            "controllerSensitivity",
            features.controller_sensitivity.is_some(),
        ),
        (
            "controllerAnalogCircularity",
            features.controller_analog_circularity.is_some(),
        ),
        (
            "controllerLightingEffects",
            features.controller_lighting_effects.is_some(),
        ),
        ("quickEffects", features.quick_effects.is_some()),
    ] {
        if present {
            // Keep file extension fields and list order at this boundary.
            // Only the known validated records enter the active profile.
            target.insert(key.into(), saved[key].clone());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::{merge_known, source_product};
    use super::restore_features;
    use serde_json::json;

    #[test]
    fn saved_feature_records_survive_the_initial_ui_profile_schema() {
        for pid in [2629, 2636, 2647, 2650, 2676, 2684, 4133, 4144] {
            let spec = source_product(pid).unwrap();
            let saved = json!({
                "profile": {
                    "obmSlotId": [3, 1],
                    "controllerDeadzone": {
                        "deadzones": [{"controllerId": 2, "value": 9}, {"controllerId": 1, "value": 5}],
                        "preventDoubleDeadzone": false,
                        "extension": {"retained": true}
                    },
                    "controllerSensitivity": {
                        "sensitivityValues": [{"controllerId": 2, "value": 80}, {"controllerId": 1, "value": 30}]
                    },
                    "controllerAnalogCircularity": "circular"
                }
            });
            let mut restored = json!({"profile": spec.profile});
            merge_known(&mut restored, &saved);
            restore_features(&mut restored["profile"], &saved["profile"], spec).unwrap();
            for (key, value) in saved["profile"].as_object().unwrap() {
                assert_eq!(&restored["profile"][key], value, "{pid}: {key}");
            }
            assert!(
                restored["profile"]
                    .get("device_write_acknowledged")
                    .is_none()
            );
        }
    }

    #[test]
    fn inactive_trigger_records_are_restorable_without_scheduling_a_write() {
        for pid in [2629, 2636, 2647, 2650, 2676, 2684, 4133, 4144] {
            let spec = source_product(pid).unwrap();
            let saved = json!({
                "controllerAnalogActuation": {"analogActuations": [
                    {"controllerId": 2, "startRange": 0, "endRange": 100, "isActive": false},
                    {"controllerId": 1, "startRange": 20, "endRange": 80, "isActive": false}
                ]},
                "controllerDigitalActuation": {"digitalActuations": [
                    {"controllerId": 1, "actuationPoint": 0, "isRapidTrigger": false, "isActive": true},
                    {"controllerId": 2, "actuationPoint": 40, "isRapidTrigger": true, "isActive": true}
                ]},
                "actionsFromUI": [{"status": "SUCCESS"}],
                "device_write_acknowledged": true
            });
            let mut profile = spec.profile.clone();
            restore_features(&mut profile, &saved, spec).unwrap();
            assert_eq!(
                profile["controllerAnalogActuation"],
                saved["controllerAnalogActuation"]
            );
            assert_eq!(
                profile["controllerDigitalActuation"],
                saved["controllerDigitalActuation"]
            );
            assert!(profile.get("actionsFromUI").is_none());
            assert!(profile.get("device_write_acknowledged").is_none());
        }
    }

    #[test]
    fn malformed_or_wrong_product_features_are_rejected_before_any_record_changes() {
        let spec = source_product(2629).unwrap();
        for invalid in [
            json!({"obmSlotId": [256]}),
            json!({"controllerDeadzone": {"deadzones": [{"controllerId": 3, "value": 5}], "preventDoubleDeadzone": false}}),
            json!({"controllerDeadzone": {"deadzones": [{"controllerId": 1, "value": 5}, {"controllerId": 1, "value": 7}], "preventDoubleDeadzone": false}}),
            json!({"controllerDeadzone": {"deadzones": [], "preventDoubleDeadzone": "false"}}),
            json!({"controllerSensitivity": {"sensitivityValues": [{"controllerId": 1, "value": 101}]}}),
            json!({"controllerAnalogCircularity": "unknown"}),
            json!({"controllerAnalogActuation": {"analogActuations": [{"controllerId": 1, "startRange": 60, "endRange": 40, "isActive": false}]}}),
        ] {
            let mut profile = spec.profile.clone();
            let before = profile.clone();
            assert!(
                restore_features(&mut profile, &invalid, spec).is_err(),
                "{invalid}"
            );
            assert_eq!(profile, before);
        }
        let spec = source_product(4115).unwrap();
        let mut profile = spec.profile.clone();
        assert!(
            restore_features(
                &mut profile,
                &json!({"controllerAnalogCircularity": "circular"}),
                spec
            )
            .is_err()
        );
    }
}
