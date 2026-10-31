//! Typed device import is projected once into the source file/UI boundary.
use super::*;
use anyhow::{Context as _, ensure};
use razer_device::controller_initialization::{ProfileObservation, ProfileValue, Read};
use razer_device::controller_mapping::{
    LocalKeyboardMetadata, MappingProjection, project_mappings,
};

pub struct ImportedControllerProfile {
    pub settings: Value,
    pub mappings: Option<MappingProjection>,
}

pub fn project_controller_socd(
    settings: &mut Value,
    mode: razer_device::controller_initialization::SocdMode,
) -> anyhow::Result<()> {
    ensure!(
        settings.get("profile").is_some_and(Value::is_object)
            && settings.get("controller").is_some_and(Value::is_object),
        "Controller SOCD profile boundary omits its source sections"
    );
    let projection = settings.pointer("/controller/dpad");
    ensure!(
        projection.is_none_or(Value::is_object),
        "Controller SOCD projection has an invalid D-pad object"
    );
    ensure!(
        settings
            .pointer("/controller/dpad/socdSettings")
            .is_none_or(Value::is_object),
        "Controller SOCD projection has an invalid setting object"
    );
    let dpad = settings
        .get_mut("controller")
        .and_then(Value::as_object_mut)
        .context("Controller SOCD projection has no control object")?
        .entry("dpad")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .context("Controller SOCD projection has an invalid D-pad object")?;
    dpad.entry("socdSettings")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .context("Controller SOCD setting is not an object")?
        .insert("value".into(), serde_json::to_value(mode)?);
    settings["profile"]["controllerDPadSOCD"] = serde_json::to_value(mode)?;
    Ok(())
}

#[derive(serde::Deserialize)]
#[serde(tag = "outputType")]
enum SavedMappingMetadata {
    #[serde(rename = "keyboardGroup")]
    Keyboard {
        #[serde(flatten)]
        metadata: LocalKeyboardMetadata,
    },
    #[serde(other)]
    Other,
}

fn local_keyboard_metadata(previous: Option<&Value>) -> anyhow::Result<Vec<LocalKeyboardMetadata>> {
    let Some(mappings) = previous.and_then(|saved| saved.pointer("/profile/mappings")) else {
        return Ok(Vec::new());
    };
    Ok(
        serde_json::from_value::<Vec<SavedMappingMetadata>>(mappings.clone())?
            .into_iter()
            .filter_map(|mapping| match mapping {
                SavedMappingMetadata::Keyboard { metadata } => Some(metadata),
                SavedMappingMetadata::Other => None,
            })
            .collect(),
    )
}

pub fn import_controller_profile(
    product_id: u32,
    previous: Option<&Value>,
    observed: &ProfileObservation,
    connection: Option<&razer_device::controller_profile::ConnectionProbeObservation>,
) -> anyhow::Result<ImportedControllerProfile> {
    let spec = source_product(product_id).context("No controller source profile")?;
    let mut result = profile::new_import_settings(spec, observed.slot, previous)?;
    let mut mapping_projection = None;
    for read in &observed.fields {
        ensure!(
            product_id == 2636
                || !matches!(
                    read.field,
                    razer_device::controller_initialization::ProfileField::Brightness
                        | razer_device::controller_initialization::ProfileField::ControllerLightingEffects
                ),
            "Controller import field belongs to another product"
        );
        let Read::Observed { value } = &read.result else {
            continue;
        };
        ensure!(
            value.field() == read.field,
            "Controller import result field differs"
        );
        match value {
            ProfileValue::Polling { observation, .. } => {
                if let Some(hertz) = observation.hertz() {
                    if product_id == 2676 {
                        result["profile"]["pollingRate"] = json!(hertz);
                        result["profile"]["pollingRateWireless"] = json!(hertz);
                    } else if let Some(connection) = connection {
                        connection.validate_for(product_id)?;
                        result["profile"]["pollingRateWireless"] = json!(hertz);
                    }
                }
            }
            ProfileValue::Mappings { observation } => {
                let local = local_keyboard_metadata(previous)?;
                let mappings = project_mappings(product_id, observation, &local)?;
                // This is the source's partial local cache, not a claim that
                // failed or omitted rows were observed as hardware defaults.
                // Per-button errors remain on the connection observation.
                result["profile"]["mappings"] = serde_json::to_value(mappings.source_mappings()?)?;
                mapping_projection = Some(mappings);
            }
            ProfileValue::Brightness { raw } => {
                let value = (u16::from(*raw) * 100).div_ceil(255);
                result["profile"]["brightness"]["isEnabled"] = json!(value > 0);
                if value > 0 {
                    result["profile"]["brightness"]["value"] = json!(value);
                }
            }
            ProfileValue::Lighting { effect } => {
                let mut settings: lighting_state::ControllerLightingSettings =
                    serde_json::from_value(
                        result["profile"]
                            .get("controllerLightingEffects")
                            .context("Controller middleware profile omits lighting")?
                            .clone(),
                    )?;
                settings.observe(effect)?;
                result["profile"]["controllerLightingEffects"] = serde_json::to_value(settings)?;
            }
            ProfileValue::Analog { values } => {
                result["profile"]["controllerAnalogActuation"] = json!({"analogActuations":values});
                for entry in serde_json::to_value(values)?
                    .as_array()
                    .context("Invalid analog trigger projection")?
                {
                    let side = control_side(entry, "leftTrigger", "rightTrigger")?;
                    let target = &mut result["controller"][side];
                    let active = entry
                        .get("isActive")
                        .and_then(Value::as_bool)
                        .context("Analog trigger projection omits its mode")?;
                    target["mode"] = json!(if active {
                        spec.analog_mode
                    } else {
                        spec.digital_mode
                    });
                    target["startRange"] = entry["startRange"].clone();
                    target["endRange"] = entry["endRange"].clone();
                }
            }
            ProfileValue::Digital { values } => {
                result["profile"]["controllerDigitalActuation"] =
                    json!({"digitalActuations":values});
                for entry in serde_json::to_value(values)?
                    .as_array()
                    .context("Invalid digital trigger projection")?
                {
                    let side = control_side(entry, "leftTrigger", "rightTrigger")?;
                    let target = &mut result["controller"][side];
                    let active = entry
                        .get("isActive")
                        .and_then(Value::as_bool)
                        .context("Digital trigger projection omits its mode")?;
                    target["mode"] = json!(if active {
                        spec.digital_mode
                    } else {
                        spec.analog_mode
                    });
                    target["actuationPoint"] = entry["actuationPoint"].clone();
                    target["isRapidTrigger"] = entry["isRapidTrigger"].clone();
                }
            }
            ProfileValue::Circularity { option } => {
                result["profile"]["controllerAnalogCircularity"] =
                    json!(if *option == 0 { "standard" } else { "circular" });
                result["controller"]["circularity"]["mode"] = json!(if *option == 0 {
                    spec.standard_mode
                } else {
                    spec.circular_mode
                });
            }
            ProfileValue::Deadzone { values, prevention } => {
                result["profile"]["controllerDeadzone"] =
                    json!({"deadzones":values,"preventDoubleDeadzone":*prevention != 0});
                for entry in serde_json::to_value(values)?
                    .as_array()
                    .context("Invalid controller deadzone projection")?
                {
                    let side = control_side(entry, "leftStick", "rightStick")?;
                    result["controller"][side]["deadzoneValue"] = entry["value"].clone();
                    result["controller"][side]["preventDoubleDeadzone"] = json!(*prevention != 0);
                }
            }
            ProfileValue::Sensitivity { values } => {
                result["profile"]["controllerSensitivity"] = json!({"sensitivityValues":values});
                for entry in serde_json::to_value(values)?
                    .as_array()
                    .context("Invalid controller sensitivity projection")?
                {
                    let side = control_side(entry, "leftStick", "rightStick")?;
                    result["controller"][side]["sensitivityValue"] = entry["value"].clone();
                }
            }
            ProfileValue::Dpad { option } => {
                result["profile"]["controllerDPadIgnoreDiagonals"] = json!(*option == 1);
            }
            ProfileValue::Haptic { scaling } => {
                profile::observe_imported_haptic(&mut result["profile"], *scaling)?;
            }
        }
    }
    Ok(ImportedControllerProfile {
        settings: result,
        mappings: mapping_projection,
    })
}
fn control_side(
    value: &Value,
    left: &'static str,
    right: &'static str,
) -> anyhow::Result<&'static str> {
    match value.get("controllerId").and_then(Value::as_u64) {
        Some(1) => Ok(left),
        Some(2) => Ok(right),
        _ => anyhow::bail!("Unknown controller side in import projection"),
    }
}

impl GamepadProductWorkspace {
    pub fn set_controller_initialization_pending(&mut self, cx: &mut Context<Self>) {
        if matches!(self.spec.product_id, 2636 | 2676) {
            self.controller_initialization_ready = false;
            self.controller_initialization_lighting_observed = false;
            self.controller_socd_observation = Read::Unobserved;
            self.trigger_runtime.invalidate();
            self.lighting_runtime.invalidate();
            self.chroma_runtime.invalidate();
        }
        cx.notify();
    }
    pub fn observe_controller_initialization_profile(
        &mut self,
        profile: Option<&ProfileObservation>,
    ) {
        self.controller_initialization_lighting_observed = profile.is_some_and(|profile| {
            [
                razer_device::controller_initialization::ProfileField::Brightness,
                razer_device::controller_initialization::ProfileField::ControllerLightingEffects,
            ]
            .iter()
            .all(|field| {
                profile.fields.iter().any(|read| {
                    read.field == *field && matches!(read.result, Read::Observed { .. })
                })
            })
        });
    }
    pub(super) fn release_controller_initialization(&mut self, cx: &mut Context<Self>) {
        self.controller_initialization_ready = true;
        self.request_trigger_read(cx);
        self.request_controller_sleep(cx);
        self.request_controller_lighting(cx);
        if self.chroma_runtime.has_observers() {
            self.refresh_controller_chroma(cx);
        }
        cx.notify();
    }
    pub(crate) fn resume_controller_runtime_without_import(&mut self, cx: &mut Context<Self>) {
        // No old import observation is promoted to this connection. Each
        // retained feature reader owns its normal current-generation GET.
        self.release_controller_initialization(cx);
    }
    pub(crate) fn observe_controller_socd(
        &mut self,
        observation: Option<&Read<razer_device::controller_initialization::SocdObservation>>,
        cx: &mut Context<Self>,
    ) {
        self.controller_socd_observation = observation.cloned().unwrap_or(Read::Unobserved);
        cx.notify();
    }
    pub fn controller_socd_observation(
        &self,
    ) -> &Read<razer_device::controller_initialization::SocdObservation> {
        &self.controller_socd_observation
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use razer_device::{
        controller_initialization::{ProfileField, ProfileRead},
        controller_triggers::{AnalogTrigger, DigitalTrigger, ThumbstickValue},
    };
    use std::prelude::v1::test;

    #[test]
    fn controller_initialization_projects_every_shared_feature_from_current_reads_for_both_products()
     {
        use razer_device::controller_profile::{
            AssignmentRead, ButtonAssignment, ButtonRead, ConnectionProbeObservation,
            MappingObservation, PollingMethod, PollingObservation,
        };
        let read = |value: ProfileValue| ProfileRead {
            field: value.field(),
            result: Read::Observed { value },
        };
        for product in [2636, 2676] {
            let spec = source_product(product).unwrap();
            let mut previous = json!({"profile":spec.profile,"controller":spec.controller});
            previous["profile"]["pollingRate"] = json!(125);
            previous["profile"]["pollingRateWireless"] = json!(125);
            previous["profile"]["hapticIntensity"] = json!({"isEnabled":true,"value":6});
            previous["controller"]["leftTrigger"]["actuationPoint"] = json!(44);
            previous["controller"]["leftStick"]["sensitivityValue"] = json!(66);
            let mut fields = vec![
                read(ProfileValue::Polling {
                    field: ProfileField::PollingRate,
                    observation: PollingObservation {
                        profile_slot: 3,
                        method: if product == 2636 {
                            PollingMethod::ProfilePeriod
                        } else {
                            PollingMethod::UsbHighSpeed
                        },
                        raw_code: 2,
                    },
                }),
                read(ProfileValue::Mappings {
                    observation: MappingObservation {
                        profile_slot: 3,
                        buttons: vec![ButtonRead {
                            button_id: 16,
                            result: AssignmentRead::Observed {
                                assignment: ButtonAssignment {
                                    profile_slot: 3,
                                    button_id: 16,
                                    mode: 0,
                                    function_id: 2,
                                    data_size: 2,
                                    data: [1, 4, 0, 0, 0],
                                },
                            },
                        }],
                        canceled: false,
                    },
                }),
                read(ProfileValue::Digital {
                    values: vec![
                        DigitalTrigger::new(1, 21, true, true).unwrap(),
                        DigitalTrigger::new(2, 25, false, false).unwrap(),
                    ],
                }),
                read(ProfileValue::Analog {
                    values: vec![
                        AnalogTrigger::new(1, 10, 80, false).unwrap(),
                        AnalogTrigger::new(2, 20, 90, true).unwrap(),
                    ],
                }),
                read(ProfileValue::Circularity { option: 2 }),
                read(ProfileValue::Deadzone {
                    values: vec![
                        ThumbstickValue::new(1, 12).unwrap(),
                        ThumbstickValue::new(2, 16).unwrap(),
                    ],
                    prevention: 0,
                }),
                read(ProfileValue::Dpad { option: 1 }),
                read(ProfileValue::Sensitivity {
                    values: vec![
                        ThumbstickValue::new(1, 34).unwrap(),
                        ThumbstickValue::new(2, 65).unwrap(),
                    ],
                }),
                read(ProfileValue::Haptic { scaling: 83 }),
            ];
            if product == 2636 {
                fields.insert(0, read(ProfileValue::Brightness { raw: 127 }));
                fields.insert(
                    1,
                    read(ProfileValue::Lighting {
                        effect: razer_device::controller_lighting::ObservedEffect {
                            profile_slot: 3,
                            region_id: 0,
                            effect_id: 0,
                            flags: 0,
                            rate: 0,
                            color_count: 0,
                            color_bytes: Vec::new(),
                        },
                    }),
                );
            }
            let observed = ProfileObservation { slot: 3, fields };
            let connection = ConnectionProbeObservation {
                raw_data: Vec::new(),
            };
            let imported =
                import_controller_profile(product, Some(&previous), &observed, Some(&connection))
                    .unwrap();
            let profile = &imported.settings["profile"];
            assert_eq!(
                profile["pollingRate"],
                if product == 2636 { 1000 } else { 4000 }
            );
            assert_eq!(
                profile["pollingRateWireless"],
                if product == 2636 { 500 } else { 4000 }
            );
            assert_eq!(
                profile["mappings"],
                json!([{
                    "inputID":"BUTTON_M1","inputType":"ControllerInput","controllerInput":"BUTTON_M1",
                    "isHyperShift":false,"outputType":"keyboardGroup",
                    "keyboardGroup":{"key":"KEY_A","modifiers":["Left_Ctrl"]}
                }])
            );
            assert_eq!(
                profile["controllerDigitalActuation"]["digitalActuations"],
                json!([
                    {"controllerId":1,"actuationPoint":21,"isRapidTrigger":true,"isActive":true},
                    {"controllerId":2,"actuationPoint":25,"isRapidTrigger":false,"isActive":false}
                ])
            );
            assert_eq!(
                profile["controllerAnalogActuation"]["analogActuations"],
                json!([
                    {"controllerId":1,"startRange":10,"endRange":80,"isActive":false},
                    {"controllerId":2,"startRange":20,"endRange":90,"isActive":true}
                ])
            );
            assert_eq!(profile["controllerAnalogCircularity"], "circular");
            assert_eq!(
                profile["controllerDeadzone"],
                json!({
                    "deadzones":[{"controllerId":1,"value":12},{"controllerId":2,"value":16}],
                    "preventDoubleDeadzone":false
                })
            );
            assert_eq!(profile["controllerDPadIgnoreDiagonals"], true);
            assert_eq!(
                profile["controllerSensitivity"]["sensitivityValues"],
                json!([
                    {"controllerId":1,"value":34},{"controllerId":2,"value":65}
                ])
            );
            assert_eq!(
                profile["hapticIntensity"],
                json!({"isEnabled":true,"value":5})
            );
            assert_eq!(
                imported.settings["controller"]["leftTrigger"]["actuationPoint"],
                21
            );
            assert_eq!(
                imported.settings["controller"]["rightTrigger"]["startRange"],
                20
            );
            assert_eq!(
                imported.settings["controller"]["leftStick"]["sensitivityValue"],
                34
            );
            assert_eq!(
                imported.settings["controller"]["rightStick"]["deadzoneValue"],
                16
            );
            if product == 2636 {
                assert_eq!(profile["brightness"], json!({"isEnabled":true,"value":50}));
                assert_eq!(profile["controllerLightingEffects"]["effectId"], 0);
            }

            // 11365 catches an ordinary getter error without copying old
            // feature values into 31759's newly constructed profile.
            for unobserved in [false, true] {
                let mut failed = observed.clone();
                for field in &mut failed.fields {
                    field.result = if unobserved {
                        Read::Unobserved
                    } else {
                        Read::Failed {
                            error: "actual field read failed".into(),
                        }
                    };
                }
                let failed_snapshot = failed.clone();
                let restored =
                    import_controller_profile(product, Some(&previous), &failed, Some(&connection))
                        .unwrap();
                let profile = &restored.settings["profile"];
                assert_eq!(profile["pollingRate"], 1000);
                assert_eq!(profile["mappings"], json!([]));
                assert_eq!(
                    profile["controllerDigitalActuation"]["digitalActuations"][0]["actuationPoint"],
                    0
                );
                assert_eq!(
                    profile["controllerAnalogActuation"]["analogActuations"][0]["startRange"],
                    0
                );
                assert_eq!(profile["controllerAnalogCircularity"], "standard");
                assert_eq!(profile["controllerDeadzone"]["deadzones"][0]["value"], 7);
                assert_eq!(profile["controllerDPadIgnoreDiagonals"], false);
                assert_eq!(
                    profile["controllerSensitivity"]["sensitivityValues"][0]["value"],
                    50
                );
                assert_eq!(profile["hapticIntensity"]["value"], 4);
                if product == 2636 {
                    assert_eq!(profile["pollingRateWireless"], 250);
                    assert_eq!(profile["brightness"], json!({"isEnabled":true,"value":100}));
                    assert_eq!(profile["controllerLightingEffects"]["effectId"], 3);
                } else {
                    assert!(profile.get("pollingRateWireless").is_none());
                }
                assert_eq!(restored.settings["controller"], spec.controller);
                assert!(restored.mappings.is_none());
                assert_eq!(failed, failed_snapshot);
            }
        }
    }

    #[test]
    fn controller_initialization_haptic_zero_uses_the_rebuilt_level_and_unknown_scaling_fails() {
        // 47442 maps these exact wire values; 69427 supplies the pre-read 4.
        for product in [2636, 2676] {
            let spec = source_product(product).unwrap();
            let mut previous = json!({"profile":spec.profile,"controller":spec.controller});
            previous["profile"]["hapticIntensity"] = json!({
                "isEnabled":true,"value":6,"enableAudioToHaptics":true
            });
            for (scaling, value) in [
                (0, 4),
                (17, 1),
                (33, 2),
                (50, 3),
                (67, 4),
                (83, 5),
                (100, 6),
            ] {
                let observed = ProfileObservation {
                    slot: 3,
                    fields: vec![ProfileRead {
                        field: ProfileField::HapticIntensity,
                        result: Read::Observed {
                            value: ProfileValue::Haptic { scaling },
                        },
                    }],
                };
                let imported =
                    import_controller_profile(product, Some(&previous), &observed, None).unwrap();
                assert_eq!(
                    imported.settings["profile"]["hapticIntensity"],
                    json!({
                        "isEnabled":scaling > 0,"value":value
                    })
                );
            }
            let unknown = ProfileObservation {
                slot: 3,
                fields: vec![ProfileRead {
                    field: ProfileField::HapticIntensity,
                    result: Read::Observed {
                        value: ProfileValue::Haptic { scaling: 7 },
                    },
                }],
            };
            assert!(import_controller_profile(product, Some(&previous), &unknown, None).is_err());
        }
    }
    #[test]
    fn controller_initialization_imports_real_lighting_without_a_local_feature_record() {
        use razer_device::controller_lighting::ObservedEffect;

        let spec = source_product(2636).unwrap();
        assert!(spec.profile.get("controllerLightingEffects").is_none());
        for effect in [
            ObservedEffect {
                profile_slot: 3,
                region_id: 0,
                effect_id: 0,
                flags: 0,
                rate: 0,
                color_count: 0,
                color_bytes: Vec::new(),
            },
            ObservedEffect {
                profile_slot: 3,
                region_id: 0,
                effect_id: 13,
                flags: 9,
                rate: 1,
                color_count: 2,
                color_bytes: vec![0x12, 0x34, 0x56, 0xab, 0xcd, 0xef],
            },
        ] {
            let observed = ProfileObservation {
                slot: 3,
                fields: vec![
                    ProfileRead {
                        field: ProfileField::Brightness,
                        result: Read::Observed {
                            value: ProfileValue::Brightness { raw: 127 },
                        },
                    },
                    ProfileRead {
                        field: ProfileField::ControllerLightingEffects,
                        result: Read::Observed {
                            value: ProfileValue::Lighting {
                                effect: effect.clone(),
                            },
                        },
                    },
                ],
            };
            let imported = import_controller_profile(2636, None, &observed, None)
                .unwrap()
                .settings;
            assert_eq!(imported["profile"]["brightness"]["value"], 50);
            assert_eq!(imported["profile"]["brightness"]["isEnabled"], true);
            let settings: lighting_state::ControllerLightingSettings =
                serde_json::from_value(imported["profile"]["controllerLightingEffects"].clone())
                    .unwrap();
            assert_eq!(
                settings.protocol_effect().unwrap(),
                if effect.effect_id == 0 {
                    razer_device::controller_lighting::ControllerEffect::Off
                } else {
                    razer_device::controller_lighting::ControllerEffect::Static {
                        index: 9,
                        custom_colors: vec![[0x12, 0x34, 0x56], [0xab, 0xcd, 0xef]],
                    }
                }
            );
            assert_eq!(imported["profile"]["obmSlotId"], json!([3]));
            assert!(imported.get("device_write_acknowledged").is_none());
        }
    }

    #[test]
    fn controller_initialization_failed_or_missing_lighting_keeps_unobserved_middleware_initial_values()
     {
        for result in [
            Read::Unobserved,
            Read::Failed {
                error: "actual lighting getter failed".into(),
            },
        ] {
            let observed = ProfileObservation {
                slot: 3,
                fields: vec![ProfileRead {
                    field: ProfileField::ControllerLightingEffects,
                    result,
                }],
            };
            let imported = import_controller_profile(2636, None, &observed, None)
                .unwrap()
                .settings;
            // 31759.eL rebuilds from 69427 before 11365 attempts the getter.
            // This is a local initial value; the failed observation is separate.
            assert_eq!(
                imported["profile"]["controllerLightingEffects"]["effectId"],
                3
            );
            assert_eq!(imported["profile"]["controllerLightingEffects"]["rate"], 2);
            assert!(!matches!(observed.fields[0].result, Read::Observed { .. }));
        }
    }

    #[test]
    fn controller_initialization_lighting_observation_replaces_cached_effect_and_retains_local_fields()
     {
        use razer_device::controller_lighting::ObservedEffect;

        let spec = source_product(2636).unwrap();
        let mut previous = json!({"profile":spec.profile,"controller":spec.controller});
        previous["profile"]["controllerLightingEffects"] =
            serde_json::to_value(lighting_state::ControllerLightingSettings::default()).unwrap();
        previous["profile"]["brightness"]["value"] = json!(73);
        previous["profile"]["switchOffLighting"] =
            json!({"isIdle":true,"idleTime":11,"whenDisplayOff":false});
        let observed = ProfileObservation {
            slot: 3,
            fields: vec![
                ProfileRead {
                    field: ProfileField::Brightness,
                    result: Read::Observed {
                        value: ProfileValue::Brightness { raw: 0 },
                    },
                },
                ProfileRead {
                    field: ProfileField::ControllerLightingEffects,
                    result: Read::Observed {
                        value: ProfileValue::Lighting {
                            effect: ObservedEffect {
                                profile_slot: 3,
                                region_id: 0,
                                effect_id: 0,
                                flags: 0,
                                rate: 0,
                                color_count: 0,
                                color_bytes: Vec::new(),
                            },
                        },
                    },
                },
            ],
        };
        let imported = import_controller_profile(2636, Some(&previous), &observed, None)
            .unwrap()
            .settings;
        assert_eq!(
            imported["profile"]["controllerLightingEffects"]["effectId"],
            0
        );
        assert_eq!(imported["profile"]["brightness"]["isEnabled"], false);
        // 11365.N retains the newly reconstructed brightness at zero.
        assert_eq!(imported["profile"]["brightness"]["value"], 100);
        assert_eq!(
            imported["profile"]["switchOffLighting"],
            previous["profile"]["switchOffLighting"]
        );

        let mut unknown = observed.clone();
        let Read::Observed {
            value: ProfileValue::Lighting { effect },
        } = &mut unknown.fields[1].result
        else {
            panic!("expected the scripted lighting observation")
        };
        effect.effect_id = 0xff;
        assert!(import_controller_profile(2636, Some(&previous), &unknown, None).is_err());
        previous["profile"]["controllerLightingEffects"] = serde_json::Value::Null;
        let imported = import_controller_profile(2636, Some(&previous), &observed, None)
            .unwrap()
            .settings;
        assert_eq!(
            imported["profile"]["controllerLightingEffects"]["effectId"],
            0
        );
    }

    #[test]
    fn controller_initialization_socd_source_and_ui_projection_survive_profile_restoration() {
        use razer_device::controller_initialization::SocdMode;
        let spec = source_product(2676).unwrap();
        let mut saved = json!({"profile":spec.profile,"controller":spec.controller});
        project_controller_socd(&mut saved, SocdMode::First).unwrap();
        let mut restored = json!({"profile":spec.profile,"controller":spec.controller});
        merge_known(&mut restored, &saved);
        profile::restore_features(&mut restored["profile"], &saved["profile"], spec).unwrap();
        assert_eq!(restored["profile"]["controllerDPadSOCD"], "first");
        assert_eq!(
            saved["controller"]["dpad"]["socdSettings"],
            json!({"value":"first"})
        );
        let mut invalid = json!({"profile":{},"controller":{"dpad":"invalid"}});
        assert!(project_controller_socd(&mut invalid, SocdMode::Neutral).is_err());
    }
    #[test]
    fn controller_initialization_mapping_cache_is_partial_while_connection_errors_remain_typed() {
        use razer_device::controller_profile::{
            AssignmentRead, ButtonAssignment, ButtonRead, MappingObservation,
        };
        for product in [2636, 2676] {
            let spec = source_product(product).unwrap();
            let mut previous = json!({"profile":spec.profile,"controller":spec.controller});
            previous["profile"]["futureExtension"] = json!({"retained":true});
            previous["profile"]["switchOffLighting"] = json!({"whenDisplayOff":true,"whenIdle":true,"idleTime":8,"futureExtension":{"retained":true}});
            previous["profile"]["mappings"] = json!([
                {"inputID":"BUTTON_M1","inputType":"ControllerInput","controllerInput":"BUTTON_M1","isHyperShift":false,"outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_A","keyGroup":4,"modifiers":["Left_Alt"],"localExtension":"source-does-not-copy"}},
                {"inputID":"BUTTON_M2","inputType":"ControllerInput","controllerInput":"BUTTON_M2","isHyperShift":false,"outputType":"disableGroup"}
            ]);
            let observed = ProfileObservation {
                slot: 3,
                fields: vec![ProfileRead {
                    field: ProfileField::Mappings,
                    result: Read::Observed {
                        value: ProfileValue::Mappings {
                            observation: MappingObservation {
                                profile_slot: 3,
                                buttons: vec![
                                    ButtonRead {
                                        button_id: 16,
                                        result: AssignmentRead::Observed {
                                            assignment: ButtonAssignment {
                                                profile_slot: 3,
                                                button_id: 16,
                                                mode: 0,
                                                function_id: 2,
                                                data_size: 2,
                                                data: [1, 4, 0, 0, 0],
                                            },
                                        },
                                    },
                                    ButtonRead {
                                        button_id: 17,
                                        result: AssignmentRead::Failed {
                                            error: "actual second-button transfer failed".into(),
                                        },
                                    },
                                    ButtonRead {
                                        button_id: 18,
                                        result: AssignmentRead::Observed {
                                            assignment: ButtonAssignment {
                                                profile_slot: 3,
                                                button_id: 18,
                                                mode: 0,
                                                function_id: 16,
                                                data_size: 1,
                                                data: [18, 0, 0, 0, 0],
                                            },
                                        },
                                    },
                                ],
                                canceled: false,
                            },
                        },
                    },
                }],
            };
            let imported =
                import_controller_profile(product, Some(&previous), &observed, None).unwrap();
            assert!(!imported.mappings.as_ref().unwrap().all_rows_observed());
            assert!(
                matches!(&imported.mappings.unwrap().rows[1],razer_device::controller_mapping::MappingRow::Failed{button_id:17,error} if error=="actual second-button transfer failed")
            );
            let profile = &imported.settings["profile"];
            assert_eq!(
                profile["mappings"],
                json!([{"inputID":"BUTTON_M1","inputType":"ControllerInput","controllerInput":"BUTTON_M1","isHyperShift":false,"outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_A","keyGroup":4,"modifiers":["Left_Ctrl"]}}])
            );
            assert_eq!(
                profile["switchOffLighting"],
                previous["profile"]["switchOffLighting"]
            );
            assert_eq!(
                profile["futureExtension"],
                previous["profile"]["futureExtension"]
            );
            let persisted: Value =
                serde_json::from_slice(&serde_json::to_vec(&imported.settings).unwrap()).unwrap();
            assert_eq!(persisted, imported.settings);
        }
    }
    #[test]
    fn controller_initialization_canceled_mapping_projection_does_not_publish_a_source_cache() {
        use razer_device::controller_profile::MappingObservation;
        let observed = ProfileObservation {
            slot: 3,
            fields: vec![ProfileRead {
                field: ProfileField::Mappings,
                result: Read::Observed {
                    value: ProfileValue::Mappings {
                        observation: MappingObservation {
                            profile_slot: 3,
                            buttons: vec![],
                            canceled: true,
                        },
                    },
                },
            }],
        };
        for product in [2636, 2676] {
            assert!(import_controller_profile(product, None, &observed, None).is_err());
        }
    }

    #[test]
    fn controller_initialization_copies_keyboard_metadata_only_for_the_exact_source_mapping() {
        use razer_device::controller_profile::{
            AssignmentRead, ButtonAssignment, ButtonRead, MappingObservation,
        };
        let observed = ProfileObservation {
            slot: 3,
            fields: vec![ProfileRead {
                field: ProfileField::Mappings,
                result: Read::Observed {
                    value: ProfileValue::Mappings {
                        observation: MappingObservation {
                            profile_slot: 3,
                            buttons: vec![ButtonRead {
                                button_id: 16,
                                result: AssignmentRead::Observed {
                                    assignment: ButtonAssignment {
                                        profile_slot: 3,
                                        button_id: 16,
                                        mode: 0,
                                        function_id: 2,
                                        data_size: 2,
                                        data: [1, 4, 0, 0, 0],
                                    },
                                },
                            }],
                            canceled: false,
                        },
                    },
                },
            }],
        };
        for product in [2636, 2676] {
            let spec = source_product(product).unwrap();
            for mismatch in [
                None,
                Some("inputID"),
                Some("isHyperShift"),
                Some("outputType"),
                Some("key"),
            ] {
                let mut mapping = json!({
                    "inputID":"BUTTON_M1","inputType":"ControllerInput","controllerInput":"BUTTON_M1",
                    "isHyperShift":false,"outputType":"keyboardGroup",
                    "keyboardGroup":{"key":"KEY_A","keyGroup":7,"modifiers":["Left_Alt"],"localExtension":"ignored"}
                });
                match mismatch {
                    Some("inputID") => mapping["inputID"] = json!("BUTTON_M2"),
                    Some("isHyperShift") => mapping["isHyperShift"] = json!(true),
                    Some("outputType") => mapping["outputType"] = json!("disableGroup"),
                    Some("key") => mapping["keyboardGroup"]["key"] = json!("KEY_B"),
                    None => {}
                    _ => unreachable!(),
                }
                let mut previous = json!({"profile":spec.profile,"controller":spec.controller});
                previous["profile"]["mappings"] = json!([mapping]);
                let imported =
                    import_controller_profile(product, Some(&previous), &observed, None).unwrap();
                let keyboard = &imported.settings["profile"]["mappings"][0]["keyboardGroup"];
                assert_eq!(keyboard["key"], "KEY_A");
                assert_eq!(keyboard["modifiers"], json!(["Left_Ctrl"]));
                assert_eq!(
                    keyboard.get("keyGroup").and_then(Value::as_u64),
                    mismatch.is_none().then_some(7)
                );
                assert!(keyboard.get("localExtension").is_none());
            }
        }
    }
    #[test]
    fn controller_initialization_terminal_observations_project_real_fields_and_preserve_local_metadata()
     {
        let spec = source_product(2676).unwrap();
        let previous = json!({"profile":spec.profile,"controller":spec.controller});
        let mut previous = previous;
        previous["profile"]["obmSlotId"] = json!([3, 1]);
        previous["profile"]["futureExtension"] = json!({"retained":true});
        let observed = ProfileObservation {
            slot: 3,
            fields: vec![
                ProfileRead {
                    field: ProfileField::ControllerDigitalActuation,
                    result: Read::Observed {
                        value: ProfileValue::Digital {
                            values: vec![
                                DigitalTrigger::new(1, 21, true, true).unwrap(),
                                DigitalTrigger::new(2, 0, false, false).unwrap(),
                            ],
                        },
                    },
                },
                ProfileRead {
                    field: ProfileField::ControllerAnalogActuation,
                    result: Read::Observed {
                        value: ProfileValue::Analog {
                            values: vec![
                                AnalogTrigger::new(1, 10, 80, false).unwrap(),
                                AnalogTrigger::new(2, 20, 90, true).unwrap(),
                            ],
                        },
                    },
                },
                ProfileRead {
                    field: ProfileField::ControllerDeadzone,
                    result: Read::Observed {
                        value: ProfileValue::Deadzone {
                            values: vec![
                                ThumbstickValue::new(1, 12).unwrap(),
                                ThumbstickValue::new(2, 16).unwrap(),
                            ],
                            prevention: 0,
                        },
                    },
                },
                ProfileRead {
                    field: ProfileField::ControllerAnalogCircularity,
                    result: Read::Observed {
                        value: ProfileValue::Circularity { option: 1 },
                    },
                },
            ],
        };
        let result = import_controller_profile(2676, Some(&previous), &observed, None)
            .unwrap()
            .settings;
        assert_eq!(result["profile"]["obmSlotId"], json!([3]));
        assert_eq!(result["profile"]["futureExtension"]["retained"], true);
        assert_eq!(
            result["controller"]["leftTrigger"]["mode"],
            spec.digital_mode
        );
        assert_eq!(result["controller"]["leftTrigger"]["actuationPoint"], 21);
        assert_eq!(result["controller"]["leftTrigger"]["isRapidTrigger"], true);
        assert_eq!(
            result["controller"]["rightTrigger"]["mode"],
            spec.analog_mode
        );
        assert_eq!(result["controller"]["rightTrigger"]["startRange"], 20);
        assert_eq!(result["controller"]["leftStick"]["deadzoneValue"], 12);
        assert_eq!(result["controller"]["rightStick"]["deadzoneValue"], 16);
        assert_eq!(
            result["controller"]["leftStick"]["preventDoubleDeadzone"],
            false
        );
        assert_eq!(
            result["controller"]["circularity"]["mode"],
            spec.circular_mode
        );
    }
    #[test]
    fn controller_initialization_partial_failure_retains_a_new_local_record_and_the_real_error() {
        let spec = source_product(2676).unwrap();
        let mut previous = json!({"profile":spec.profile,"controller":spec.controller});
        previous["profile"]["controllerSensitivity"] = json!({"sensitivityValues":[{"controllerId":1,"value":34},{"controllerId":2,"value":65}],"extension":"kept"});
        let observed = ProfileObservation {
            slot: 7,
            fields: vec![ProfileRead {
                field: ProfileField::ControllerSensitivity,
                result: Read::Failed {
                    error: "real getter failed".into(),
                },
            }],
        };
        let result = import_controller_profile(2676, Some(&previous), &observed, None)
            .unwrap()
            .settings;
        // The source constructs a new MW profile; old device values are not
        // copied by 11365.z. Local extension metadata is not a read result.
        assert_eq!(
            result["profile"]["controllerSensitivity"]["sensitivityValues"],
            json!([
                {"controllerId":1,"value":50},{"controllerId":2,"value":50}
            ])
        );
        assert_eq!(
            result["profile"]["controllerSensitivity"]["extension"],
            "kept"
        );
        assert_eq!(result["controller"]["leftStick"]["sensitivityValue"], 10);
        assert!(matches!(observed.fields[0].result, Read::Failed { .. }));
    }
    #[test]
    fn controller_initialization_polling_projection_requires_the_source_transport_consumer() {
        use razer_device::controller_profile::{PollingMethod, PollingObservation};
        for (product_id, method, code) in [
            (2636, PollingMethod::ProfilePeriod, 2),
            (2676, PollingMethod::UsbHighSpeed, 2),
        ] {
            let spec = source_product(product_id).unwrap();
            let previous = json!({"profile":spec.profile,"controller":spec.controller});
            let observed = ProfileObservation {
                slot: 3,
                fields: vec![ProfileRead {
                    field: ProfileField::PollingRateWireless,
                    result: Read::Observed {
                        value: ProfileValue::Polling {
                            field: ProfileField::PollingRateWireless,
                            observation: PollingObservation {
                                profile_slot: 3,
                                method,
                                raw_code: code,
                            },
                        },
                    },
                }],
            };
            let result = import_controller_profile(product_id, Some(&previous), &observed, None)
                .unwrap()
                .settings;
            if product_id == 2676 {
                assert_eq!(result["profile"]["pollingRate"], 4000);
                assert_eq!(result["profile"]["pollingRateWireless"], 4000);
            } else {
                assert_eq!(
                    result["profile"]["pollingRate"],
                    previous["profile"]["pollingRate"]
                );
                assert_eq!(result["profile"]["pollingRateWireless"], 250);
                let connection = razer_device::controller_profile::ConnectionProbeObservation {
                    raw_data: vec![],
                };
                let confirmed = import_controller_profile(
                    product_id,
                    Some(&previous),
                    &observed,
                    Some(&connection),
                )
                .unwrap()
                .settings;
                assert_eq!(
                    confirmed["profile"]["pollingRate"],
                    previous["profile"]["pollingRate"]
                );
                assert_eq!(confirmed["profile"]["pollingRateWireless"], 500);
            }
        }
    }
}
