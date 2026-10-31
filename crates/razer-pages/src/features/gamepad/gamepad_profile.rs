//! Controller feature records are added by the current middleware after the
//! initial UI profile is created. Validate them at the persistence boundary
//! without treating saved command logs as device acknowledgements.
use super::{
    GamepadProductSpec, chroma_runtime::ChromaRuntime, lighting_runtime::LightingRuntime,
    trigger_runtime::TriggerRuntime,
};
use anyhow::{Context as _, ensure};
use razer_device::controller_mapping::SourceMapping;
use razer_device::controller_triggers::{AnalogTrigger, DigitalTrigger, ThumbstickValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnalogActuation {
    controller_id: u8,
    start_range: u8,
    end_range: u8,
    is_active: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnalogActuations {
    analog_actuations: Vec<AnalogActuation>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DigitalActuation {
    controller_id: u8,
    actuation_point: u8,
    is_rapid_trigger: bool,
    is_active: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DigitalActuations {
    digital_actuations: Vec<DigitalActuation>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StickValue {
    controller_id: u8,
    value: u16,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Deadzone {
    deadzones: Vec<StickValue>,
    #[serde(rename = "preventDoubleDeadzone")]
    _prevent_double_deadzone: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Sensitivity {
    sensitivity_values: Vec<StickValue>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum Circularity {
    Standard,
    Circular,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SwitchOffLighting {
    is_display_on: bool,
    is_idle_enabled: bool,
    idle_minutes: u16,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Brightness {
    is_enabled: bool,
    value: u8,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct HapticIntensity {
    is_enabled: bool,
    value: u8,
    enable_audio_to_haptics: Option<bool>,
}

/// Module 69427's middleware profile is independent of the UI reducer's
/// initial profile. These local values never establish a device observation.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MiddlewareProfile {
    guid: String,
    name: String,
    obm_slot_id: Vec<u8>,
    switch_off_lighting: Option<SwitchOffLighting>,
    brightness: Option<Brightness>,
    controller_lighting_effects: Option<super::lighting_state::ControllerLightingSettings>,
    polling_rate: u32,
    polling_rate_wireless: Option<u32>,
    mappings: Vec<SourceMapping>,
    #[serde(rename = "controllerDPadSOCD")]
    controller_dpad_socd: Option<razer_device::controller_initialization::SocdMode>,
    controller_digital_actuation: DigitalActuations,
    controller_analog_actuation: AnalogActuations,
    controller_analog_circularity: Circularity,
    controller_deadzone: Deadzone,
    #[serde(rename = "controllerDPadIgnoreDiagonals")]
    controller_dpad_ignore_diagonals: bool,
    controller_sensitivity: Sensitivity,
    haptic_intensity: HapticIntensity,
}

impl MiddlewareProfile {
    fn new(product_id: u32, slot: u8) -> anyhow::Result<Self> {
        ensure!(
            matches!(product_id, 2636 | 2676),
            "No verified controller middleware import profile for {product_id}"
        );
        Ok(Self {
            guid: String::new(),
            name: String::new(),
            obm_slot_id: vec![slot],
            switch_off_lighting: (product_id == 2636).then_some(SwitchOffLighting {
                is_display_on: false,
                is_idle_enabled: false,
                idle_minutes: 1,
            }),
            brightness: (product_id == 2636).then_some(Brightness {
                is_enabled: true,
                value: 100,
            }),
            controller_lighting_effects: (product_id == 2636)
                .then(super::lighting_state::ControllerLightingSettings::default),
            polling_rate: 1000,
            polling_rate_wireless: (product_id == 2636).then_some(250),
            mappings: Vec::new(),
            controller_dpad_socd: (product_id == 2676)
                .then_some(razer_device::controller_initialization::SocdMode::Neutral),
            controller_digital_actuation: DigitalActuations {
                digital_actuations: [1, 2]
                    .map(|controller_id| DigitalActuation {
                        controller_id,
                        actuation_point: 0,
                        is_rapid_trigger: false,
                        is_active: false,
                    })
                    .into(),
            },
            controller_analog_actuation: AnalogActuations {
                analog_actuations: [1, 2]
                    .map(|controller_id| AnalogActuation {
                        controller_id,
                        start_range: 0,
                        end_range: 100,
                        is_active: true,
                    })
                    .into(),
            },
            controller_analog_circularity: Circularity::Standard,
            controller_deadzone: Deadzone {
                deadzones: [1, 2]
                    .map(|controller_id| StickValue {
                        controller_id,
                        value: 7,
                    })
                    .into(),
                _prevent_double_deadzone: true,
            },
            controller_dpad_ignore_diagonals: false,
            controller_sensitivity: Sensitivity {
                sensitivity_values: [1, 2]
                    .map(|controller_id| StickValue {
                        controller_id,
                        value: 50,
                    })
                    .into(),
            },
            haptic_intensity: HapticIntensity {
                is_enabled: true,
                value: 4,
                enable_audio_to_haptics: (product_id == 2636).then_some(true),
            },
        })
    }
}

// This operates only on a freshly serialized typed template. Optional source
// fields supply its complete key schema, then disappear when not applicable.
fn omit_unset_template_fields(template: &mut Value) {
    if let Some(fields) = template.as_object_mut() {
        fields.retain(|_, value| !value.is_null());
        for value in fields.values_mut() {
            omit_unset_template_fields(value);
        }
    }
}

fn preserve_file_extensions(target: &mut Value, previous: &Value, schemas: &[&Value]) {
    let (Some(target), Some(previous)) = (target.as_object_mut(), previous.as_object()) else {
        return;
    };
    for (key, value) in previous {
        if matches!(
            key.as_str(),
            "actionsFromUI" | "device_write_acknowledged" | "audioHapticIntensity"
        ) {
            continue;
        }
        let known = schemas
            .iter()
            .filter_map(|schema| schema.get(key))
            .collect::<Vec<_>>();
        if known.is_empty() {
            target.insert(key.clone(), value.clone());
        } else if let Some(target) = target.get_mut(key) {
            // Lists contain source settings with domain identities. Their old
            // rows are never copied as extensions of a reconstructed profile.
            preserve_file_extensions(target, value, &known);
        }
    }
}

pub(super) fn new_import_settings(
    spec: &GamepadProductSpec,
    slot: u8,
    previous: Option<&Value>,
) -> anyhow::Result<Value> {
    let schema = serde_json::to_value(MiddlewareProfile::new(spec.product_id, slot)?)?;
    let mut profile = schema.clone();
    omit_unset_template_fields(&mut profile);
    let mut controller = spec.controller.clone();
    if let Some(previous) = previous {
        let saved = previous
            .get("profile")
            .filter(|value| value.is_object())
            .context("Saved controller workspace omits its profile")?;
        let controls = previous
            .get("controller")
            .filter(|value| value.is_object())
            .context("Saved controller workspace omits its controls")?;
        preserve_file_extensions(&mut profile, saved, &[&schema, &spec.profile]);
        preserve_file_extensions(&mut controller, controls, &[&spec.controller]);
        if let Some(guid) = saved.get("guid") {
            profile["guid"] = serde_json::to_value(
                guid.as_str()
                    .context("Saved controller GUID is not a string")?,
            )?;
        }
        // Module 11365.z copies the whole local-only object, including an
        // absent property, after rebuilding each slot in module 31759.eL.
        let fields = profile.as_object_mut().context("Invalid import profile")?;
        if let Some(policy) = saved.get("switchOffLighting") {
            ensure!(policy.is_object(), "Saved lighting policy is not an object");
            fields.insert("switchOffLighting".into(), policy.clone());
        } else {
            fields.remove("switchOffLighting");
        }
    }
    Ok(serde_json::json!({"profile": profile, "controller": controller}))
}

pub(super) fn observe_imported_haptic(profile: &mut Value, scaling: u8) -> anyhow::Result<()> {
    let level = [0, 17, 33, 50, 67, 83, 100]
        .iter()
        .position(|raw| *raw == scaling)
        .context("Unknown controller haptic scaling")? as u8;
    let initial: HapticIntensity = serde_json::from_value(
        profile
            .get("hapticIntensity")
            .context("Controller middleware profile omits haptic intensity")?
            .clone(),
    )?;
    // Module 47442 replaces the profile record after a successful GET. Zero
    // retains the rebuilt profile's level; audio policy belongs to the root.
    let mut observed = serde_json::to_value(HapticIntensity {
        is_enabled: level > 0,
        value: if level > 0 { level } else { initial.value },
        enable_audio_to_haptics: None,
    })?;
    omit_unset_template_fields(&mut observed);
    profile["hapticIntensity"] = observed;
    Ok(())
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
    #[serde(rename = "controllerDPadIgnoreDiagonals")]
    controller_dpad_ignore_diagonals: Option<bool>,
    polling_rate_wireless: Option<u32>,
    haptic_intensity: Option<HapticIntensity>,
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
        if let Some(haptic) = &self.haptic_intensity {
            ensure!(
                (1..=6).contains(&haptic.value),
                "Saved controller haptic intensity exceeds the source limits"
            );
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
        (
            "controllerDPadIgnoreDiagonals",
            features.controller_dpad_ignore_diagonals.is_some(),
        ),
        (
            "pollingRateWireless",
            features.polling_rate_wireless.is_some(),
        ),
        ("hapticIntensity", features.haptic_intensity.is_some()),
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
    use super::{new_import_settings, restore_features, restore_local_profile};
    use serde_json::json;

    #[test]
    fn middleware_import_profiles_use_the_complete_current_source_defaults() {
        // Current 2636/2676 module 69427 DEFAULTPROFILE, before 11365 getters.
        for product in [2636, 2676] {
            let spec = source_product(product).unwrap();
            let imported = new_import_settings(spec, 3, None).unwrap();
            let profile = &imported["profile"];
            assert_eq!(profile["guid"], "");
            assert_eq!(profile["name"], "");
            assert_eq!(profile["obmSlotId"], json!([3]));
            assert_eq!(profile["pollingRate"], 1000);
            assert_eq!(profile["mappings"], json!([]));
            assert_eq!(
                profile["controllerDigitalActuation"],
                json!({
                    "digitalActuations": [
                        {"controllerId":1,"actuationPoint":0,"isRapidTrigger":false,"isActive":false},
                        {"controllerId":2,"actuationPoint":0,"isRapidTrigger":false,"isActive":false}
                    ]
                })
            );
            assert_eq!(
                profile["controllerAnalogActuation"],
                json!({
                    "analogActuations": [
                        {"controllerId":1,"startRange":0,"endRange":100,"isActive":true},
                        {"controllerId":2,"startRange":0,"endRange":100,"isActive":true}
                    ]
                })
            );
            assert_eq!(profile["controllerAnalogCircularity"], "standard");
            assert_eq!(
                profile["controllerDeadzone"],
                json!({
                    "preventDoubleDeadzone":true,
                    "deadzones":[{"controllerId":1,"value":7},{"controllerId":2,"value":7}]
                })
            );
            assert_eq!(profile["controllerDPadIgnoreDiagonals"], false);
            assert_eq!(
                profile["controllerSensitivity"],
                json!({
                    "sensitivityValues":[{"controllerId":1,"value":50},{"controllerId":2,"value":50}]
                })
            );
            if product == 2636 {
                assert_eq!(profile["brightness"], json!({"isEnabled":true,"value":100}));
                assert_eq!(
                    profile["switchOffLighting"],
                    json!({
                        "isDisplayOn":false,"isIdleEnabled":false,"idleMinutes":1
                    })
                );
                assert_eq!(profile["pollingRateWireless"], 250);
                assert!(profile.get("controllerDPadSOCD").is_none());
                assert_eq!(
                    profile["hapticIntensity"],
                    json!({
                        "isEnabled":true,"value":4,"enableAudioToHaptics":true
                    })
                );
                assert_eq!(
                    profile["controllerLightingEffects"],
                    json!({
                        "effectId":3,"mode":"preset","index":0,"rate":2,
                        "colorList":[
                            {"value":"#00ff00","index":0},{"value":"#44d7b6","index":1},
                            {"value":"#0019ff","index":2},{"value":"#7f00ff","index":3},
                            {"value":"#ff0000","index":4},{"value":"#fa6400","index":5},
                            {"value":"#f7b500","index":6},{"value":"#ffffff","index":7}
                        ]
                    })
                );
            } else {
                assert_eq!(profile["controllerDPadSOCD"], "neutral");
                assert_eq!(
                    profile["hapticIntensity"],
                    json!({"isEnabled":true,"value":4})
                );
                for field in [
                    "brightness",
                    "switchOffLighting",
                    "controllerLightingEffects",
                    "pollingRateWireless",
                ] {
                    assert!(profile.get(field).is_none(), "{field}");
                }
            }
            assert!(profile.get("quickEffects").is_none());
            assert!(profile.get("gamingMode").is_none());
            assert_eq!(imported["controller"], spec.controller);
            assert_eq!(imported["controller"]["leftTrigger"]["actuationPoint"], 10);
            assert_eq!(imported["controller"]["leftTrigger"]["startRange"], 50);
            assert_eq!(imported["controller"]["leftStick"]["sensitivityValue"], 10);
        }
        for product in [2629, 2647, 2650, 2684, 4115, 4133, 4144] {
            assert!(new_import_settings(source_product(product).unwrap(), 3, None).is_err());
        }
    }

    #[test]
    fn middleware_import_retains_only_source_local_metadata_and_file_extensions() {
        for product in [2636, 2676] {
            let spec = source_product(product).unwrap();
            let mut previous = json!({"profile":spec.profile,"controller":spec.controller});
            previous["profile"]["guid"] = json!("retained-guid");
            previous["profile"]["name"] = json!("old-name");
            previous["profile"]["obmSlotId"] = json!([1, 3]);
            previous["profile"]["brightness"] = json!({"isEnabled":false,"value":73});
            previous["profile"]["pollingRate"] = json!(125);
            previous["profile"]["pollingRateWireless"] = json!(500);
            previous["profile"]["controllerSensitivity"] = json!({
                "sensitivityValues":[{"controllerId":1,"value":33},{"controllerId":2,"value":72}]
            });
            previous["profile"]["controllerDPadIgnoreDiagonals"] = json!(true);
            previous["profile"]["controllerDPadSOCD"] = json!("last");
            previous["profile"]["hapticIntensity"] = json!({
                "isEnabled":false,"value":6,"enableAudioToHaptics":false
            });
            previous["profile"]["controllerLightingEffects"] = json!(null);
            previous["profile"]["switchOffLighting"] = json!({
                "isDisplayOn":true,"isIdleEnabled":true,"idleMinutes":11,"extra":{"preserved":true}
            });
            previous["profile"]["futureExtension"] = json!({"preserved":true});
            previous["profile"]["futureNull"] = json!(null);
            previous["profile"]["actionsFromUI"] = json!([{"status":"SUCCESS"}]);
            previous["profile"]["device_write_acknowledged"] = json!(true);
            previous["controller"]["leftTrigger"]["actuationPoint"] = json!(41);
            previous["controller"]["leftTrigger"]["futureExtension"] = json!("retained");
            let imported = new_import_settings(spec, 3, Some(&previous)).unwrap();
            let profile = &imported["profile"];
            assert_eq!(profile["guid"], "retained-guid");
            assert_eq!(profile["name"], "");
            assert_eq!(profile["obmSlotId"], json!([3]));
            assert_eq!(profile["pollingRate"], 1000);
            assert_eq!(
                profile["controllerSensitivity"]["sensitivityValues"][0]["value"],
                50
            );
            assert_eq!(profile["controllerDPadIgnoreDiagonals"], false);
            assert_eq!(profile["hapticIntensity"]["value"], 4);
            assert_eq!(profile["hapticIntensity"]["isEnabled"], true);
            assert_eq!(
                profile["switchOffLighting"],
                previous["profile"]["switchOffLighting"]
            );
            assert_eq!(
                profile["futureExtension"],
                previous["profile"]["futureExtension"]
            );
            assert_eq!(profile.get("futureNull"), Some(&serde_json::Value::Null));
            assert_eq!(imported["controller"]["leftTrigger"]["actuationPoint"], 10);
            assert_eq!(
                imported["controller"]["leftTrigger"]["futureExtension"],
                "retained"
            );
            for field in [
                "actionsFromUI",
                "device_write_acknowledged",
                "quickEffects",
                "gamingMode",
            ] {
                assert!(profile.get(field).is_none(), "{product}: {field}");
            }
            previous["profile"]
                .as_object_mut()
                .unwrap()
                .remove("switchOffLighting");
            assert!(
                new_import_settings(spec, 3, Some(&previous)).unwrap()["profile"]
                    .get("switchOffLighting")
                    .is_none()
            );
        }
    }

    #[test]
    fn middleware_import_feature_records_survive_local_page_restoration() {
        for product in [2636, 2676] {
            let spec = source_product(product).unwrap();
            let mut saved = new_import_settings(spec, 3, None).unwrap();
            super::observe_imported_haptic(&mut saved["profile"], 50).unwrap();
            saved["profile"]["controllerDPadIgnoreDiagonals"] = json!(true);
            saved["profile"]["pollingRateWireless"] = json!(500);
            let mut restored = json!({"profile":spec.profile,"controller":spec.controller});
            let mut trigger = TriggerRuntime::default();
            let mut lighting = LightingRuntime::new(product);
            let mut chroma = ChromaRuntime::default();
            restore_local_profile(
                &mut restored,
                Some(&saved),
                spec,
                &mut trigger,
                &mut lighting,
                &mut chroma,
            )
            .unwrap();
            assert_eq!(
                restored["profile"]["hapticIntensity"],
                json!({"isEnabled":true,"value":3})
            );
            assert_eq!(restored["profile"]["controllerDPadIgnoreDiagonals"], true);
            assert_eq!(restored["profile"]["pollingRateWireless"], 500);
            assert!(
                restored["profile"]
                    .get("device_write_acknowledged")
                    .is_none()
            );
        }
    }

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
