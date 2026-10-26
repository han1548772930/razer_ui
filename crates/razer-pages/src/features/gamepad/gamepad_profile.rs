//! Controller feature records are added by the current middleware after the
//! initial UI profile is created. Validate them at the persistence boundary
//! without treating saved command logs as device acknowledgements.
use super::{
    GamepadProductSpec, chroma_runtime::ChromaRuntime, lighting_runtime::LightingRuntime,
    trigger_runtime::TriggerRuntime,
};
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
    #[serde(rename = "controllerDPadSOCD")]
    controller_dpad_socd: Option<razer_device::controller_initialization::SocdMode>,
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
        // Initial source profiles retain quick-effect metadata without mounting
        // Kitsune's runtime. This checks the persisted schema, not a device capability.
        ensure!(
            spec.profile.get("quickEffects").is_some() || self.quick_effects.is_none(),
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
        (
            "controllerDPadSOCD",
            features.controller_dpad_socd.is_some(),
        ),
    ] {
        if present {
            // Keep file extension fields and list order at this boundary.
            // Only the known validated records enter the active profile.
            target.insert(key.into(), saved[key].clone());
        }
    }
    Ok(())
}

pub(super) fn restore_local_profile(
    active: &mut Value,
    saved: Option<&Value>,
    spec: &GamepadProductSpec,
    trigger: &mut TriggerRuntime,
    lighting: &mut LightingRuntime,
    chroma: &mut ChromaRuntime,
) -> anyhow::Result<()> {
    let mut next = serde_json::json!({"profile": spec.profile, "controller": spec.controller});
    if let Some(saved) = saved {
        ensure!(
            saved.get("profile").is_some_and(Value::is_object)
                && saved.get("controller").is_some_and(Value::is_object),
            "Saved controller workspace is missing its profile or controls"
        );
        // Validate the file before schema merging can replace an active value.
        // Rejected local input must not clear the current connection's results.
        restore_features(&mut next["profile"], &saved["profile"], spec)?;
        super::merge_known(&mut next, saved);
    }
    trigger.invalidate();
    lighting.invalidate();
    chroma.invalidate();
    *active = next;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::chroma_runtime::{ChromaRuntime, ControllerChromaObservation};
    use super::super::lighting_runtime::LightingRuntime;
    use super::super::trigger_runtime::TriggerRuntime;
    use super::super::{merge_known, source_product};
    use super::{restore_features, restore_local_profile};
    use serde_json::json;

    #[test]
    fn full_profiles_retain_source_quick_effect_metadata_without_enabling_kitsune_runtime() {
        for pid in [2629, 2636, 2647, 2650, 2676, 2684] {
            let spec = source_product(pid).unwrap();
            assert_eq!(spec.profile["quickEffects"], json!({"selectedEffectId": 2}));
            let mut saved = json!({"profile": spec.profile, "controller": spec.controller});
            saved["profile"]["obmSlotId"] = json!([3, 1]);
            saved["profile"]["controllerDeadzone"] = json!({
                "deadzones": [{"controllerId": 2, "value": 9}, {"controllerId": 1, "value": 5}],
                "preventDoubleDeadzone": false,
                "extension": {"retained": true}
            });
            saved["profile"]["actionsFromUI"] = json!([{"status": "SUCCESS"}]);
            saved["profile"]["device_write_acknowledged"] = json!(true);
            let mut restored = json!({"profile": spec.profile, "controller": spec.controller});
            merge_known(&mut restored, &saved);
            restore_features(&mut restored["profile"], &saved["profile"], spec).unwrap();
            assert_eq!(
                restored["profile"]["quickEffects"],
                json!({"selectedEffectId": 2}),
                "{pid}"
            );
            assert_eq!(restored["profile"]["obmSlotId"], json!([3, 1]), "{pid}");
            assert_eq!(
                restored["profile"]["controllerDeadzone"], saved["profile"]["controllerDeadzone"],
                "{pid}"
            );
            assert!(restored["profile"].get("actionsFromUI").is_none(), "{pid}");
            assert!(
                restored["profile"]
                    .get("device_write_acknowledged")
                    .is_none(),
                "{pid}"
            );
            let runtime = LightingRuntime::new(pid);
            assert!(runtime.quick.is_none(), "{pid}");
            assert_eq!(runtime.settings.is_some(), pid == 2636, "{pid}");
        }
    }

    #[test]
    fn kitsune_full_profile_restoration_keeps_strict_quick_effect_validation() {
        let spec = source_product(4115).unwrap();
        assert_eq!(spec.profile["quickEffects"], json!({"selectedEffectId": 3}));
        for quick in [
            json!({"selectedEffectId": 3}),
            json!({
                "selectedEffectId": 1,
                "selectedEffectSetting": {"effectId": 1, "color1": "#112233"},
                "effectSettings": [
                    {"effectId": 1, "color1": "#112233"},
                    {"effectId": 4, "direction": 2}
                ],
                "extension": {"retained": true}
            }),
        ] {
            let mut saved = spec.profile.clone();
            saved["quickEffects"] = quick.clone();
            let mut restored = spec.profile.clone();
            restore_features(&mut restored, &saved, spec).unwrap();
            assert_eq!(restored["quickEffects"], quick);
        }
        assert!(LightingRuntime::new(4115).quick.is_some());
        for invalid in [
            json!({"selectedEffectId": 6}),
            json!({"selectedEffectId": "3"}),
            json!({"selectedEffectId": 1, "selectedEffectSetting": {"effectId": 3, "color1": "#112233"}}),
            json!({"selectedEffectId": 1, "selectedEffectSetting": {"color1": "#11223g"}}),
            json!({"selectedEffectId": 1, "selectedEffectSetting": {"color1": "no-color"}}),
            json!({"selectedEffectId": 4, "selectedEffectSetting": {"direction": 3}}),
            json!({"selectedEffectId": 5, "selectedEffectSetting": {"color1": "#112233"}}),
            json!({"selectedEffectId": 3, "effectSettings": [
                {"effectId": 1, "color1": "#112233"},
                {"effectId": 1, "color1": "#445566"}
            ]}),
            json!({"selectedEffectId": 3, "effectSettings": [{"effectId": 1, "color1": "invalid"}]}),
        ] {
            let mut saved = spec.profile.clone();
            saved["quickEffects"] = invalid.clone();
            let mut restored = spec.profile.clone();
            let before = restored.clone();
            assert!(
                restore_features(&mut restored, &saved, spec).is_err(),
                "{invalid}"
            );
            assert_eq!(restored, before, "{invalid}");
        }
    }

    #[test]
    fn full_profiles_without_source_quick_effects_do_not_acquire_them_on_restore() {
        for pid in [4133, 4144] {
            let spec = source_product(pid).unwrap();
            assert!(spec.profile.get("quickEffects").is_none(), "{pid}");
            let mut saved = json!({"profile": spec.profile, "controller": spec.controller});
            saved["profile"]["obmSlotId"] = json!([3]);
            let mut restored = json!({"profile": spec.profile, "controller": spec.controller});
            merge_known(&mut restored, &saved);
            restore_features(&mut restored["profile"], &saved["profile"], spec).unwrap();
            assert_eq!(restored["profile"]["obmSlotId"], json!([3]), "{pid}");
            assert!(restored["profile"].get("quickEffects").is_none(), "{pid}");
            saved["profile"]["quickEffects"] = json!({"selectedEffectId": 2});
            let before = restored.clone();
            assert!(
                restore_features(&mut restored["profile"], &saved["profile"], spec).is_err(),
                "{pid}"
            );
            assert_eq!(restored, before, "{pid}");
        }
    }

    #[test]
    fn page_restore_rejects_invalid_known_quick_fields_before_clearing_observations() {
        for pid in [2629, 2636, 2647, 2650, 2676, 2684, 4115, 4133, 4144] {
            let spec = source_product(pid).unwrap();
            for quick in [
                json!({"selectedEffectId": 6}),
                json!({"selectedEffectId": "3"}),
                json!({"selectedEffectId": 1, "selectedEffectSetting": {"color1": "invalid"}}),
            ] {
                let mut trigger = TriggerRuntime::default();
                let mut lighting = LightingRuntime::new(pid);
                let lighting_before = serde_json::to_value(&lighting.settings).unwrap();
                let quick_before = serde_json::to_value(&lighting.quick).unwrap();
                let mut chroma = ChromaRuntime::default();
                chroma.observation = ControllerChromaObservation::new()
                    .with_installation(true, false)
                    .with_enabled(true)
                    .with_dynamic_lighting(true);
                chroma.advanced = true;
                chroma.last_error = Some("Existing device read failed".into());
                let observed = chroma.observation.clone();
                let mut active = json!({"profile": spec.profile, "controller": spec.controller});
                active["profile"]["name"] = json!("Current profile");
                if let Some(brightness) = active["profile"].get_mut("brightness") {
                    brightness["value"] = json!(42);
                }
                let before = active.clone();
                let mut saved = active.clone();
                saved["profile"]["name"] = json!("Rejected profile");
                if let Some(brightness) = saved["profile"].get_mut("brightness") {
                    brightness["value"] = json!(77);
                }
                saved["profile"]["quickEffects"] = quick;
                saved["profile"]["actionsFromUI"] = json!([{"status": "SUCCESS"}]);
                saved["profile"]["device_write_acknowledged"] = json!(true);
                let result = restore_local_profile(
                    &mut active,
                    Some(&saved),
                    spec,
                    &mut trigger,
                    &mut lighting,
                    &mut chroma,
                );
                assert!(result.is_err(), "{pid}: {saved}");
                assert_eq!(active, before, "{pid}");
                assert_eq!(chroma.observation, observed, "{pid}");
                assert!(chroma.advanced, "{pid}");
                assert_eq!(
                    chroma.last_error.as_deref(),
                    Some("Existing device read failed"),
                    "{pid}"
                );
                assert_eq!(
                    serde_json::to_value(&lighting.settings).unwrap(),
                    lighting_before
                );
                assert_eq!(serde_json::to_value(&lighting.quick).unwrap(), quick_before);
                assert!(active["profile"].get("actionsFromUI").is_none(), "{pid}");
                assert!(
                    active["profile"].get("device_write_acknowledged").is_none(),
                    "{pid}"
                );
            }
        }
    }

    #[test]
    fn page_restore_accepts_complete_profiles_without_importing_device_results() {
        for pid in [2629, 2636, 2647, 2650, 2676, 2684, 4115, 4133, 4144] {
            let spec = source_product(pid).unwrap();
            let mut trigger = TriggerRuntime::default();
            let mut lighting = LightingRuntime::new(pid);
            let mut chroma = ChromaRuntime::default();
            chroma.observation = ControllerChromaObservation::new().with_enabled(true);
            chroma.last_error = Some("Prior profile error".into());
            let mut saved = json!({"profile": spec.profile, "controller": spec.controller});
            saved["profile"]["name"] = json!("Restored profile");
            saved["profile"]["actionsFromUI"] = json!([{"status": "SUCCESS"}]);
            saved["profile"]["device_write_acknowledged"] = json!(true);
            let mut restored = json!({"profile": spec.profile, "controller": spec.controller});
            restore_local_profile(
                &mut restored,
                Some(&saved),
                spec,
                &mut trigger,
                &mut lighting,
                &mut chroma,
            )
            .unwrap();
            assert_eq!(restored["profile"]["name"], "Restored profile", "{pid}");
            assert_eq!(
                restored["profile"].get("quickEffects"),
                spec.profile.get("quickEffects"),
                "{pid}"
            );
            assert!(restored["profile"].get("actionsFromUI").is_none(), "{pid}");
            assert!(
                restored["profile"]
                    .get("device_write_acknowledged")
                    .is_none(),
                "{pid}"
            );
            assert_eq!(chroma.observation.enabled(), None, "{pid}");
            assert!(chroma.last_error.is_none(), "{pid}");
        }
    }

    #[test]
    fn page_restore_rejects_malformed_workspaces_without_replacing_the_active_profile() {
        let spec = source_product(2636).unwrap();
        for invalid in [
            json!(null),
            json!({}),
            json!({"profile": [], "controller": {}}),
            json!({"profile": {}, "controller": null}),
        ] {
            let mut active = json!({"profile": spec.profile, "controller": spec.controller});
            active["profile"]["name"] = json!("Current profile");
            let before = active.clone();
            let mut trigger = TriggerRuntime::default();
            let mut lighting = LightingRuntime::new(2636);
            let mut chroma = ChromaRuntime::default();
            chroma.observation = ControllerChromaObservation::new().with_enabled(true);
            let observed = chroma.observation.clone();
            assert!(
                restore_local_profile(
                    &mut active,
                    Some(&invalid),
                    spec,
                    &mut trigger,
                    &mut lighting,
                    &mut chroma,
                )
                .is_err(),
                "{invalid}"
            );
            assert_eq!(active, before, "{invalid}");
            assert_eq!(chroma.observation, observed, "{invalid}");
        }
    }

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
