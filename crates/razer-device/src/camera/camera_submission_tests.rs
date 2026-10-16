use super::*;
use crate::camera::{CameraLongObservation, CameraWriteAck};
use crate::camera_extensions::{CameraExtensionFailure, CameraExtensionValue};
use std::cell::Cell;

fn profile() -> Value {
    serde_json::json!({"imageMode":0,"image":[{"dataSet":{"brightness":121,"contrast":146,"saturation":136,"whiteBalance":{"value":3950,"isAuto":true}}}],
    "camera":{"antiFlicker":1,"mirrorVideo":true,"lowLightCompensation":true,
    "viewPresets":{"viewMode":3,"data":[{"mode":3,"zoom":1.235,"pan":-2,"tilt":3}]},
    "autoFocus":{"isEnabled":true,"manualFocus":99,"mode":2,"lightingType":0,"tracking":1},
    "autoExposure":{"isEnabled":false,"isoSensitivity":7,"shutterSpeed":-42,"metering":5,"compensation":-1.7},
    "dynamicNoiseReduction":{"state2D":true,"state3D":false}}})
}
fn plan(key: &str) -> CameraSubmissionPlan {
    compile_camera_edit(3594, key, &profile()).unwrap()
}
#[test]
fn camera_image_preset_preserves_source_order_and_auto_flag() {
    assert_eq!(
        plan("image-preset").commands,
        vec![
            write(P::VideoProcAmp(V::Brightness), 121, false),
            write(P::VideoProcAmp(V::Contrast), 146, false),
            write(P::VideoProcAmp(V::Saturation), 136, false),
            write(P::VideoProcAmp(V::WhiteBalance), 3950, true)
        ]
    );
}
#[test]
fn camera_mic_source_apply_is_explicit_and_carries_both_device_scoped_values() {
    let mut profile = profile();
    profile["deviceMic"] =
        serde_json::json!({"adaptiveNoiseReduction": true, "crystalClearVoice": false});
    let plan = compile_camera_edit(3595, "3595:apply-microphone", &profile).unwrap();
    assert_eq!(
        plan.commands,
        [CameraCommand::FamilyProcessing(Q::SetAudioFunction {
            adaptive_noise_reduction: true,
            crystal_clear_voice: false,
        })]
    );
    assert!(compile_camera_edit(3595, "3595:adaptiveNoiseReduction", &profile).is_err());
    assert!(compile_camera_edit(3595, "3595:crystalClearVoice", &profile).is_err());
    assert!(compile_camera_edit(3596, "apply-microphone", &profile).is_err());
    profile["deviceMic"]["crystalClearVoice"] = Value::Null;
    assert!(compile_camera_edit(3595, "apply-microphone", &profile).is_err());
}
#[test]
fn camera_image_preset_includes_present_sharpness_and_gain_in_current_runner_order() {
    let mut profile = profile();
    profile["image"][0]["dataSet"]["sharpness"] = 9.into();
    profile["image"][0]["dataSet"]["gain"] = 7.into();
    let plan = compile_camera_edit(3594, "image-preset", &profile).unwrap();
    assert_eq!(
        &plan.commands[4..],
        &[
            write(P::VideoProcAmp(V::Sharpness), 9, false),
            write(P::VideoProcAmp(V::Gain), 7, false),
        ]
    );
}
#[test]
fn camera_third_party_framing_disable_preserves_full_local_object_and_restores_view() {
    for pid in [3594, 3595, 3596] {
        let mut profile = profile();
        profile["camera"]["autoFraming"] = serde_json::json!({"isEnabled":false,"view":"Close","trackingSpeed":"Quick","trackingSubject":"People"});
        let before = profile.clone();
        let plan = compile_camera_edit(pid, "auto-framing", &profile).unwrap();
        assert_eq!(plan.commands.len(), 4);
        assert_eq!(
            plan.commands[0],
            if pid == 3594 {
                CameraCommand::Extension(E::SetAutoframing { mode: 0 })
            } else {
                CameraCommand::FamilyFocus(F::SetAutoframing { mode: 0 })
            }
        );
        assert_eq!(
            &plan.commands[1..],
            &compile_camera_edit(pid, "view-preset", &profile)
                .unwrap()
                .commands
        );
        assert_eq!(profile, before);
        profile["camera"]["autoFraming"]["isEnabled"] = true.into();
        let enabled = compile_camera_edit(pid, "auto-framing", &profile).unwrap();
        assert_eq!(enabled.commands.len(), if pid == 3596 { 1 } else { 2 });
        if pid == 3595 {
            assert_eq!(
                enabled.commands[1],
                CameraCommand::FamilyFocus(F::SetAutoframingSpeedAndSize { speed: 2, size: 2 })
            );
        }
    }
}
#[test]
fn camera_source_image_runner_continues_after_real_set_rejection_without_making_an_ack() {
    struct Rejection {
        calls: Vec<CameraCommand>,
    }
    impl CameraCommandTransport for Rejection {
        fn execute(
            &mut self,
            command: &CameraCommand,
            _: &impl Fn() -> Result<()>,
        ) -> Result<CameraCommandOutcome> {
            self.calls.push(command.clone());
            let CameraCommand::StandardWrite(setting) = command else {
                panic!("standard image setter expected")
            };
            if self.calls.len() == 2 {
                Ok(CameraCommandOutcome::RejectedWrite {
                    requested: *setting,
                    native_hresult: 0x8000_4005u32 as i32,
                    phase: crate::camera::CameraSetPhase::Set,
                })
            } else {
                Ok(valid_write(setting.property, setting.setting))
            }
        }
    }
    let mut transport = Rejection { calls: vec![] };
    let plan = plan("image-preset");
    let progress = execute_camera_submission(&mut transport, 3594, &plan, &|| Ok(()));
    assert_eq!(transport.calls, plan.commands);
    assert_eq!(progress.outcomes.len(), 4);
    assert!(matches!(
        progress.outcomes[1],
        CameraCommandOutcome::RejectedWrite { .. }
    ));
    assert!(!progress.completed && progress.error.is_some());
    assert!(progress.rejected_outcome.is_none() && progress.unconfirmed.is_none());
}
#[test]
fn camera_zoom_uses_ui_to_long_conversion() {
    assert_eq!(
        plan("3594:zoom").commands,
        vec![write(P::CameraControl(C::Zoom), 124, false)]
    );
}
#[test]
fn camera_view_selection_uses_actual_mode_and_signed_axes() {
    assert_eq!(
        plan("view-preset").commands,
        vec![
            write(P::CameraControl(C::Zoom), 124, false),
            write(P::CameraControl(C::Pan), -2, false),
            write(P::CameraControl(C::Tilt), 3, false)
        ]
    );
}
#[test]
fn camera_ev_source_offset_is_distinct_from_stops() {
    assert_eq!(
        plan("compensation").commands,
        vec![CameraCommand::Extension(E::SetEvCompensation { value: 13 })]
    );
}
#[test]
fn camera_iso_is_raw_selector_and_shutter_stays_signed() {
    assert_eq!(
        plan("iso").commands,
        vec![CameraCommand::Extension(E::SetIsoSensitivity { code: 7 })]
    );
    assert_eq!(
        plan("shutter").commands,
        vec![CameraCommand::Extension(E::SetShutterSpeed { value: -42 })]
    );
}

#[test]
fn camera_shutter_guard_preserves_source_mode_polarity_and_model_boundary() {
    let mut p = profile();
    for (value, expected) in [("normal", 1), ("bright", 0)] {
        p["camera"]["autoExposure"]["shutterLimitControl"] = value.into();
        assert_eq!(
            compile_camera_edit(3594, "AUTO_EXPOSURE:shutter-guard", &p)
                .unwrap()
                .commands,
            vec![CameraCommand::Extension(E::SetShutterLimitControl {
                mode: expected,
            })]
        );
        for product in [3595, 3596] {
            assert!(compile_camera_edit(product, "shutter-guard", &p).is_err());
        }
    }
    assert!(compile_camera_edit(3594, "shutter-guard", &profile()).is_err());
}

#[test]
fn camera_standard_manual_exposure_uses_actual_signed_profile_and_current_model_lane() {
    let mut p = profile();
    p["camera"]["autoExposure"]["exposureValue"] = (-7).into();
    assert_eq!(
        compile_camera_edit(3596, "3596:manual-exposure", &p)
            .unwrap()
            .commands,
        vec![write(P::CameraControl(C::Exposure), -7, false)]
    );
    for pid in [3594, 3595] {
        assert!(compile_camera_edit(pid, "manual-exposure", &p).is_err());
    }
    p["camera"]["autoExposure"]["isEnabled"] = true.into();
    assert!(compile_camera_edit(3596, "manual-exposure", &p).is_err());
    p["camera"]["autoExposure"]["isEnabled"] = false.into();
    p["camera"]["autoExposure"]["exposureValue"] = (-7.5).into();
    assert!(compile_camera_edit(3596, "manual-exposure", &p).is_err());
    p["camera"]["autoExposure"]
        .as_object_mut()
        .unwrap()
        .remove("exposureValue");
    assert!(compile_camera_edit(3596, "manual-exposure", &p).is_err());
}

#[test]
fn camera_v2_and_x_auto_exposure_use_independent_family_protocols_and_source_companions() {
    let mut p = profile();
    p["camera"]["autoExposure"]["exposureValue"] = (-7).into();
    for product_id in [3595, 3596] {
        p["camera"]["autoExposure"]["isEnabled"] = true.into();
        assert_eq!(
            compile_camera_edit(product_id, "auto-exposure", &p)
                .unwrap()
                .commands,
            vec![
                write(P::CameraControl(C::Exposure), 0, true),
                CameraCommand::FamilyExposure(X::SetAutoExposureMetering { mode: 5 }),
                CameraCommand::FamilyExposure(X::SetEvCompensation { value: 13 }),
            ]
        );
        p["camera"]["autoExposure"]["isEnabled"] = false.into();
        assert_eq!(
            compile_camera_edit(product_id, "auto-exposure", &p)
                .unwrap()
                .commands,
            vec![
                write(P::CameraControl(C::Exposure), 0, false),
                if product_id == 3595 {
                    CameraCommand::FamilyExposure(X::SetIsoSensitivity { code: 7 })
                } else {
                    write(P::CameraControl(C::Exposure), -7, false)
                },
                CameraCommand::FamilyExposure(X::SetShutterSpeed { value: -42 }),
            ]
        );
    }
    assert!(compile_camera_edit(3596, "iso", &p).is_err());
}

#[test]
fn camera_family_exposure_queue_retains_independent_tasks_and_x_shared_shutter_coalescing() {
    use crate::camera_queue::CameraIntentQueue;
    for product_id in [3595, 3596] {
        for automatic in [false, true] {
            let mut profile = profile();
            profile["camera"]["autoExposure"]["isEnabled"] = automatic.into();
            profile["camera"]["autoExposure"]["exposureValue"] = (-7).into();
            let mut queue = CameraIntentQueue::default();
            queue.set_scope(Some(CameraScope {
                product_id,
                container_id: "{12345678-1234-1234-1234-123456789abc}".into(),
            }));
            queue
                .edit(
                    format!("{product_id}:auto-exposure"),
                    compile_camera_edit(product_id, "auto-exposure", &profile).unwrap(),
                )
                .unwrap();
            let first = queue.next().unwrap();
            assert_eq!(
                first.plan.commands,
                [write(P::CameraControl(C::Exposure), 0, automatic)]
            );
            queue.finish(&first, Err("simulated standard Set failure".into()));
            let next = queue.next().unwrap();
            if automatic {
                assert!(matches!(
                    next.plan.commands.as_slice(),
                    [CameraCommand::FamilyExposure(
                        X::SetAutoExposureMetering { .. }
                    )]
                ));
                queue.finish(&next, Err("simulated metering failure".into()));
                let ev = queue.next().unwrap();
                assert!(matches!(
                    ev.plan.commands.as_slice(),
                    [CameraCommand::FamilyExposure(X::SetEvCompensation {
                        value: 13
                    })]
                ));
            } else if product_id == 3595 {
                assert!(matches!(
                    next.plan.commands.as_slice(),
                    [CameraCommand::FamilyExposure(X::SetIsoSensitivity { .. })]
                ));
                queue.finish(&next, Err("simulated ISO failure".into()));
                let shutter = queue.next().unwrap();
                assert!(matches!(
                    shutter.plan.commands.as_slice(),
                    [CameraCommand::FamilyExposure(X::SetShutterSpeed {
                        value: -42
                    })]
                ));
            } else {
                assert_eq!(next.key, "3596:shutter");
                // N0J starts immediately under shutterSpeed; Wcp waits behind
                // the current task of the same name (current 3596 MW30525).
                assert_eq!(
                    next.plan.commands,
                    [write(P::CameraControl(C::Exposure), -7, false)]
                );
                queue.finish(&next, Err("simulated manual exposure failure".into()));
                let shutter = queue.next().unwrap();
                assert_eq!(shutter.key, "3596:shutter");
                assert!(matches!(
                    shutter.plan.commands.as_slice(),
                    [CameraCommand::FamilyExposure(X::SetShutterSpeed {
                        value: -42
                    })]
                ));
                queue.finish(&shutter, Err("simulated shutter failure".into()));
                assert!(queue.next().is_none());
                queue
                    .edit(
                        "3596:manual-exposure".into(),
                        compile_camera_edit(product_id, "manual-exposure", &profile).unwrap(),
                    )
                    .unwrap();
                let manual = queue.next().unwrap();
                assert_eq!(manual.key, "3596:shutter");
                assert_eq!(
                    manual.plan.commands,
                    [write(P::CameraControl(C::Exposure), -7, false)]
                );
            }
        }
    }
}
#[test]
fn camera_auto_focus_preserves_three_step_chain() {
    assert_eq!(
        plan("autofocus").commands,
        vec![
            write(P::CameraControl(C::Focus), 0, true),
            CameraCommand::Extension(E::SetFdafMode {
                mode: 2,
                light_condition: 0
            }),
            CameraCommand::Extension(E::SetAutoFocusLowSensitivity { value: 1 })
        ]
    );
}
#[test]
fn camera_manual_focus_has_two_steps() {
    let mut p = profile();
    p["camera"]["autoFocus"]["isEnabled"] = false.into();
    assert_eq!(
        compile_camera_edit(3594, "focus", &p).unwrap().commands,
        vec![
            write(P::CameraControl(C::Focus), 99, false),
            CameraCommand::Extension(E::SetFdafMode {
                mode: 1,
                light_condition: 0
            })
        ]
    );
}
#[test]
fn camera_noise_channels_keep_source_order_even_when_native_ignores_channel() {
    assert_eq!(
        plan("state3D").commands,
        vec![
            CameraCommand::Extension(E::SetDynamicNoiseReduction {
                channel: 2,
                enabled: true
            }),
            CameraCommand::Extension(E::SetDynamicNoiseReduction {
                channel: 1,
                enabled: false
            })
        ]
    );
}
#[test]
fn camera_family_has_independent_focus_and_mirror_chains() {
    for pid in [3595, 3596] {
        assert!(compile_camera_edit(pid, "brightness", &profile()).is_ok());
        assert_eq!(
            compile_camera_edit(pid, "mirror", &profile())
                .unwrap()
                .commands,
            [CameraCommand::FamilyProcessing(
                crate::camera_processing_family::CameraFamilyProcessingRequest::SetMirrorVideo {
                    mode: 1
                }
            )]
        );
        for key in [
            "autofocus",
            "focus",
            "focus-tracking",
            "focus-mode",
            "focus-lighting",
        ] {
            assert_eq!(
                compile_camera_edit(pid, key, &profile()).unwrap().commands,
                vec![
                    write(P::CameraControl(C::Focus), 0, true),
                    CameraCommand::FamilyFocus(F::SetFdafMode {
                        mode: 2,
                        light_condition: 0
                    }),
                    CameraCommand::FamilyFocus(F::SetAutoFocusLowSensitivity { value: 1 })
                ]
            );
            let mut manual = profile();
            manual["camera"]["autoFocus"]["isEnabled"] = false.into();
            assert_eq!(
                compile_camera_edit(pid, key, &manual).unwrap().commands,
                vec![
                    write(P::CameraControl(C::Focus), 99, false),
                    CameraCommand::FamilyFocus(F::SetFdafMode {
                        mode: 1,
                        light_condition: 0
                    })
                ]
            );
        }
    }
}
#[test]
fn camera_missing_lifecycle_and_unresolved_callers_do_not_compile_to_success() {
    for key in ["resolution", "shortcut-key", "unknown"] {
        assert!(compile_camera_edit(3594, key, &profile()).is_err());
    }
}
#[test]
fn hdr_plan_keeps_brightness_only_when_enabled() {
    let mut profile = profile();
    profile["camera"]["hdr"] = serde_json::json!({"isEnabled":true,"mode":2});
    assert_eq!(
        compile_camera_edit(3594, "3594:hdr", &profile)
            .unwrap()
            .commands,
        vec![
            CameraCommand::Extension(E::SetHdr { enabled: true }),
            CameraCommand::Extension(E::SetHdrBrightness { brightness: 2 }),
        ]
    );
    profile["camera"]["hdr"]["isEnabled"] = false.into();
    assert_eq!(
        compile_camera_edit(3594, "3594:hdr-mode", &profile)
            .unwrap()
            .commands,
        vec![CameraCommand::Extension(E::SetHdr { enabled: false })]
    );
    profile["camera"]["hdr"]["mode"] = "invalid".into();
    profile["camera"]["hdr"]["isEnabled"] = true.into();
    assert!(compile_camera_edit(3594, "3594:hdr", &profile).is_err());
}
#[test]
fn camera_flash_save_and_restore_are_distinct_commands() {
    assert_eq!(
        plan("3594:save-camera").commands,
        vec![CameraCommand::Extension(E::SaveUvcExtCmdToFlash)]
    );
    assert_eq!(
        plan("3594:restore-camera").commands,
        vec![CameraCommand::Extension(E::RestoreDefaults)]
    );
    for pid in [3595, 3596] {
        assert_eq!(
            compile_camera_edit(pid, "save-camera", &profile())
                .unwrap()
                .commands,
            vec![CameraCommand::FamilyProcessing(Q::SaveUvcExtCmdToFlash)]
        );
    }
    assert!(compile_camera_edit(3596, "restore-camera", &profile()).is_err());
    assert!(
        CameraSubmissionPlan {
            commands: vec![
                CameraCommand::ShowProperties,
                CameraCommand::StandardRead(P::CameraControl(C::Zoom))
            ]
        }
        .validate(3594)
        .is_err()
    );
}

#[test]
fn family_processing_plans_keep_source_order_native_model_and_watermark_conversion() {
    let mut p = profile();
    p["camera"]["hdr"] = serde_json::json!({"isEnabled":true,"mode":1});
    p["camera"]["mjpegQuality"] =
        serde_json::json!({"autoQuality":false,"manualQuality":83,"mode":7});
    p["camera"]["watermark"] = serde_json::json!({"isEnabled":true,"position":"right-bottom"});
    for pid in [3595, 3596] {
        assert_eq!(
            compile_camera_edit(pid, "state2D", &p).unwrap().commands,
            vec![
                CameraCommand::FamilyProcessing(Q::SetDynamicNoiseReduction {
                    channel: 2,
                    enabled: true
                }),
                CameraCommand::FamilyProcessing(Q::SetDynamicNoiseReduction {
                    channel: 1,
                    enabled: false
                }),
            ]
        );
        assert_eq!(
            compile_camera_edit(pid, "hdr", &p).unwrap().commands,
            vec![
                CameraCommand::FamilyProcessing(Q::SetHdr { enabled: true }),
                CameraCommand::FamilyProcessing(Q::SetHdrBrightness { brightness: 1 }),
            ]
        );
        assert_eq!(
            compile_camera_edit(pid, "mjpeg-quality", &p)
                .unwrap()
                .commands,
            vec![
                CameraCommand::FamilyProcessing(Q::SetAdaptiveMjpegQuality {
                    enabled: false,
                    quality: 0
                }),
                CameraCommand::FamilyProcessing(Q::SetFixedMjpegQuality { quality: 83 }),
            ]
        );
        assert_eq!(
            compile_camera_edit(pid, "ldc", &p).unwrap().commands,
            [CameraCommand::FamilyProcessing(Q::SetLdc {
                enabled: false,
                mode: 0
            })]
        );
        for (label, position) in [
            ("right-bottom", 0),
            ("left-top", 1),
            ("right-top", 2),
            ("left-bottom", 3),
            ("center-top", 4),
            ("center-bottom", 5),
            ("unknown", 0),
        ] {
            p["camera"]["watermark"]["position"] = label.into();
            assert_eq!(
                compile_camera_edit(pid, "watermark-position", &p)
                    .unwrap()
                    .commands,
                [CameraCommand::FamilyProcessing(Q::SetWatermark {
                    enabled: true,
                    position
                })]
            );
        }
        assert!(
            !compile_camera_edit(pid, "mirror", &p)
                .unwrap()
                .commands
                .iter()
                .any(|c| matches!(c, CameraCommand::Extension(_)))
        );
        assert!(compile_camera_edit(pid, "restore-camera", &p).is_err());
    }
}

#[test]
fn family_processing_native_false_continues_source_compounds_but_exceptions_stop() {
    struct Ks {
        set_count: usize,
        throw_first: bool,
    }
    impl crate::camera_focus_family::CameraFamilyFocusTransport for Ks {
        fn begin_topology(&mut self) -> Result<u32> {
            Ok(1)
        }
        fn begin_node(&mut self, _: u32) -> Result<()> {
            Ok(())
        }
        fn ks_property(
            &mut self,
            header: &[u8; 32],
            data: &mut [u8; 8],
        ) -> Result<CameraKsCallOutcome> {
            if header[20] == 2 {
                self.set_count += 1;
                if self.set_count == 1 {
                    ensure!(!self.throw_first, "simulated IPC/adapter exception");
                    return Ok(CameraKsCallOutcome {
                        hresult: -1,
                        bytes_returned: 0,
                    });
                }
            } else {
                *data = [0; 8];
            }
            Ok(CameraKsCallOutcome {
                hresult: 0,
                bytes_returned: if header[20] == 1 { 8 } else { 0 },
            })
        }
        fn end_node(&mut self) {}
        fn end_topology(&mut self) {}
        fn wait_ms(&mut self, _: u32) -> Result<()> {
            Ok(())
        }
    }
    struct Adapter {
        ks: Ks,
        pid: u32,
    }
    impl CameraCommandTransport for Adapter {
        fn execute(
            &mut self,
            command: &CameraCommand,
            validate: &impl Fn() -> Result<()>,
        ) -> Result<CameraCommandOutcome> {
            let CameraCommand::FamilyProcessing(request) = command else {
                bail!("unexpected command")
            };
            Ok(CameraCommandOutcome::FamilyProcessing(
                crate::camera_processing_family::execute_camera_family_processing(
                    &mut self.ks,
                    self.pid,
                    *request,
                    validate,
                ),
            ))
        }
    }
    let mut p = profile();
    p["camera"]["hdr"] = serde_json::json!({"isEnabled":true,"mode":1});
    p["camera"]["mjpegQuality"] =
        serde_json::json!({"autoQuality":false,"manualQuality":80,"mode":5});
    for pid in [3595, 3596] {
        for action in ["hdr", "mjpeg-quality"] {
            let plan = compile_camera_edit(pid, action, &p).unwrap();
            for throw_first in [true, false] {
                let mut adapter = Adapter {
                    ks: Ks {
                        set_count: 0,
                        throw_first,
                    },
                    pid,
                };
                let progress = execute_camera_submission(&mut adapter, pid, &plan, &|| Ok(()));
                assert!(!progress.completed && progress.error.is_some());
                assert_eq!(adapter.ks.set_count, if throw_first { 1 } else { 2 });
                assert_eq!(progress.outcomes.len(), if throw_first { 1 } else { 2 });
            }
        }
    }
}
#[test]
fn camera_auto_exposure_toggle_submits_source_mode_companions_as_independent_tasks() {
    for automatic in [false, true] {
        let mut profile = profile();
        profile["camera"]["autoExposure"]["isEnabled"] = automatic.into();
        let plan = compile_camera_edit(3594, "3594:auto-exposure", &profile).unwrap();
        assert_eq!(plan.commands.len(), 3);
        assert!(
            matches!(&plan.commands[0],CameraCommand::StandardWrite(s) if s.setting.value==0 && s.setting.is_auto==automatic)
        );
        let mut queue = crate::camera_queue::CameraIntentQueue::default();
        queue.set_scope(Some(CameraScope {
            product_id: 3594,
            container_id: "{01234567-89ab-cdef-0123-456789abcdef}".into(),
        }));
        queue.edit("3594:auto-exposure".into(), plan).unwrap();
        let mut keys = Vec::new();
        while let Some(intent) = queue.next() {
            keys.push(intent.key.clone());
            queue.finish(&intent, Err("simulated setter rejection".into()));
        }
        assert_eq!(
            keys,
            if automatic {
                vec!["3594:auto-exposure", "3594:metering", "3594:compensation"]
            } else {
                vec!["3594:auto-exposure", "3594:iso", "3594:shutter"]
            }
        );
    }
}
#[test]
fn hdr_native_false_retains_failure_and_still_submits_brightness() {
    let plan = CameraSubmissionPlan {
        commands: vec![
            CameraCommand::Extension(E::SetHdr { enabled: true }),
            CameraCommand::Extension(E::SetHdrBrightness { brightness: 1 }),
        ],
    };
    let mut mock = Mock::default();
    let progress = execute_camera_submission(&mut mock, 3594, &plan, &|| Ok(()));
    assert_eq!(mock.calls, plan.commands);
    assert_eq!(progress.outcomes.len(), 2);
    assert!(!progress.completed);
    assert!(progress.error.is_some());
}
#[test]
fn camera_reset_exposure_submits_only_the_current_automatic_tasks() {
    let mut profile = profile();
    profile["camera"]["autoExposure"] = serde_json::json!({
        "isEnabled": true, "metering": 5, "compensation": 0,
        "isoSensitivity": 4, "shutterSpeed": 16666, "shutterLimitControl": "normal"
    });
    let plan = compile_camera_edit(3594, "3594:reset-exposure", &profile).unwrap();
    assert_eq!(
        plan.commands,
        vec![
            write(P::CameraControl(C::Exposure), 0, true),
            CameraCommand::Extension(E::SetAutoExposureMetering { mode: 5 }),
            CameraCommand::Extension(E::SetEvCompensation { value: 30 }),
        ]
    );
    // Persisted manual companions and guard are not additional device writes.
    assert_eq!(
        E::SetAutoExposureMetering { mode: 5 }.commands()[0].bytes,
        [0xa0, 8, 5, 0, 0, 0, 0, 0]
    );
    assert_eq!(
        E::SetEvCompensation { value: 30 }.commands()[0].bytes,
        [0xa0, 0x14, 1, 9, 0, 0, 0, 0]
    );
    assert!(compile_camera_edit(3595, "reset-exposure", &profile).is_ok());
    assert!(compile_camera_edit(3596, "reset-exposure", &profile).is_err());
    profile["camera"]["autoExposure"]["isEnabled"] = false.into();
    assert!(compile_camera_edit(3594, "reset-exposure", &profile).is_err());
}

#[test]
fn camera_reset_exposure_runs_remaining_source_tasks_after_a_metering_failure() {
    struct ResetTransport {
        calls: Vec<CameraCommand>,
    }
    impl CameraCommandTransport for ResetTransport {
        fn execute(
            &mut self,
            command: &CameraCommand,
            validate: &impl Fn() -> Result<()>,
        ) -> Result<CameraCommandOutcome> {
            self.calls.push(command.clone());
            Ok(match command {
                CameraCommand::StandardWrite(setting) => {
                    valid_write(setting.property, setting.setting)
                }
                CameraCommand::Extension(request) => {
                    let mut ks = KsReceiptMock::default();
                    if matches!(request, E::SetAutoExposureMetering { .. }) {
                        ks.phase = Some(CameraExtensionPhase::Get);
                    }
                    CameraCommandOutcome::Extension(execute_camera_extension(
                        &mut ks, 3594, *request, validate,
                    ))
                }
                _ => bail!("unexpected reset task"),
            })
        }
    }
    let mut profile = profile();
    profile["camera"]["autoExposure"]["isEnabled"] = true.into();
    let expected_profile = profile.clone();
    let reset = compile_camera_edit(3594, "3594:reset-exposure", &profile).unwrap();
    let mut queue = crate::camera_queue::CameraIntentQueue::default();
    queue.set_scope(Some(CameraScope {
        product_id: 3594,
        container_id: "{01234567-89ab-cdef-0123-456789abcdef}".into(),
    }));
    queue
        .edit("3594:reset-exposure".into(), reset.clone())
        .unwrap();
    let mut transport = ResetTransport { calls: vec![] };
    let mut receipts = Vec::new();
    while let Some(intent) = queue.next() {
        let progress = execute_camera_submission(&mut transport, 3594, &intent.plan, &|| Ok(()));
        assert!(queue.finish(&intent, Ok(progress.clone())));
        receipts.push((intent.key, progress));
    }
    assert_eq!(transport.calls, reset.commands);
    assert_eq!(
        receipts
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        ["3594:auto-exposure", "3594:metering", "3594:compensation"]
    );
    assert!(receipts[0].1.completed);
    assert!(!receipts[1].1.completed && receipts[1].1.error.is_some());
    assert!(receipts[2].1.completed);
    let CameraCommandOutcome::Extension(failed) = &receipts[1].1.outcomes[0] else {
        panic!("missing private failure receipt")
    };
    assert!(failed.exchanges[0].set_attempted && failed.exchanges[0].get_attempted);
    assert!(!failed.exchanges[0].acknowledged);
    assert_eq!(
        profile, expected_profile,
        "device outcomes cannot roll back optimistic profile state"
    );
}
#[test]
fn camera_invalid_fields_and_ambiguous_presets_are_rejected() {
    let mut p = profile();
    p["camera"]["autoExposure"]["compensation"] = 4.into();
    assert!(compile_camera_edit(3594, "compensation", &p).is_err());
    p["camera"]["viewPresets"]["data"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"mode":3,"zoom":1,"pan":0,"tilt":0}));
    assert!(compile_camera_edit(3594, "zoom", &p).is_err());
    p["imageMode"] = 999.into();
    assert!(compile_camera_edit(3594, "image-preset", &p).is_err());
}
#[test]
fn camera_scope_rejects_preview_collection_and_nil_identities() {
    for id in [
        "PREVIEW-1",
        "hid-collection:1532:0e0a:0",
        "{00000000-0000-0000-0000-000000000000}",
        "{11111111-1111-1111-1111-11111111111Z}",
    ] {
        assert!(
            CameraScope {
                product_id: 3594,
                container_id: id.into()
            }
            .validate()
            .is_err()
        );
    }
    assert!(
        CameraScope {
            product_id: 3594,
            container_id: "{01234567-89ab-cdef-0123-456789abcdef}".into()
        }
        .validate()
        .is_ok()
    );
}

#[derive(Default)]
struct Mock {
    calls: Vec<CameraCommand>,
    fail: Option<usize>,
}
impl CameraCommandTransport for Mock {
    fn execute(
        &mut self,
        command: &CameraCommand,
        _: &impl Fn() -> Result<()>,
    ) -> Result<CameraCommandOutcome> {
        self.calls.push(command.clone());
        if self.fail == Some(self.calls.len()) {
            bail!("simulated failure after submission");
        }
        Ok(match command {
            CameraCommand::ShowProperties => CameraCommandOutcome::PropertiesClosed,
            CameraCommand::StandardRead(p) => CameraCommandOutcome::Read(StandardCameraRead {
                property: *p,
                observation: CameraLongObservation::from_native(-3, 1, 0)?,
            }),
            CameraCommand::StandardWrite(s) => CameraCommandOutcome::Write(StandardCameraWrite {
                property: s.property,
                requested: s.setting,
                acknowledgement: CameraWriteAck::from_hresult(0)?,
            }),
            CameraCommand::Extension(req) => {
                CameraCommandOutcome::Extension(CameraExtensionProgress {
                    request: *req,
                    exchanges: vec![CameraExtensionExchange {
                        command: req.commands()[0],
                        set_attempted: true,
                        set_outcome: Some(CameraKsCallOutcome {
                            hresult: 0,
                            bytes_returned: 0,
                        }),
                        get_attempted: true,
                        get_outcome: None,
                        response: Some([0xff, 0, 0, 0, 0, 0, 0, 0]),
                        acknowledged: false,
                    }],
                    value: None,
                    completed: false,
                    failure: Some(CameraExtensionFailure {
                        phase: CameraExtensionPhase::Get,
                        command_index: 0,
                        message: "simulated KS failure".into(),
                    }),
                })
            }
            CameraCommand::FamilyFocus(_)
            | CameraCommand::FamilyExposure(_)
            | CameraCommand::FamilyProcessing(_) => {
                bail!("this fixture requires the independent family transport")
            }
        })
    }
}

/// Exercises real plan compilation, standard acknowledgement, private packet
/// executor and receipt validation with entirely simulated KS calls.
struct FocusFamilySubmissionMock {
    product_id: u32,
    calls: Vec<CameraCommand>,
    packets: Vec<[u8; 8]>,
    fail_tracking: bool,
    reject_receipt: bool,
    active: bool,
    pending: [u8; 8],
}
impl FocusFamilySubmissionMock {
    fn new(product_id: u32) -> Self {
        Self {
            product_id,
            calls: vec![],
            packets: vec![],
            fail_tracking: false,
            reject_receipt: false,
            active: false,
            pending: [0; 8],
        }
    }
}
impl crate::camera_focus_family::CameraFamilyFocusTransport for FocusFamilySubmissionMock {
    fn begin_topology(&mut self) -> Result<u32> {
        Ok(1)
    }
    fn begin_node(&mut self, node: u32) -> Result<()> {
        assert_eq!(node, 0);
        self.active = true;
        Ok(())
    }
    fn ks_property(
        &mut self,
        header: &[u8; 32],
        data: &mut [u8; 8],
    ) -> Result<CameraKsCallOutcome> {
        assert!(self.active);
        if header[20] == 2 {
            self.pending = *data;
            self.packets.push(*data);
            Ok(CameraKsCallOutcome {
                hresult: 0,
                bytes_returned: 0,
            })
        } else {
            *data = [0; 8];
            if self.fail_tracking && self.pending[0..2] == [255, 6] {
                bail!("simulated tracking reply loss");
            }
            Ok(CameraKsCallOutcome {
                hresult: 0,
                bytes_returned: 8,
            })
        }
    }
    fn end_node(&mut self) {
        self.active = false;
    }
    fn end_topology(&mut self) {
        assert!(!self.active);
    }
    fn wait_ms(&mut self, ms: u32) -> Result<()> {
        assert_eq!(ms, 10);
        Ok(())
    }
}
impl CameraCommandTransport for FocusFamilySubmissionMock {
    fn execute(
        &mut self,
        command: &CameraCommand,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<CameraCommandOutcome> {
        self.calls.push(command.clone());
        Ok(match command {
            CameraCommand::StandardWrite(setting) => valid_write(setting.property, setting.setting),
            CameraCommand::FamilyFocus(request) => {
                let mut p = crate::camera_focus_family::execute_camera_family_focus(
                    self,
                    self.product_id,
                    *request,
                    validate,
                );
                if self.reject_receipt {
                    p.command.as_mut().unwrap().bytes[0] = 0xa0;
                }
                CameraCommandOutcome::FamilyFocus(p)
            }
            _ => bail!("unexpected command in focus family fixture"),
        })
    }
}
#[test]
fn camera_family_submission_executes_full_auto_and_manual_packets_for_each_model() {
    for pid in [3595, 3596] {
        for automatic in [true, false] {
            let mut profile = profile();
            profile["camera"]["autoFocus"]["isEnabled"] = automatic.into();
            let plan = compile_camera_edit(pid, "autofocus", &profile).unwrap();
            let mut mock = FocusFamilySubmissionMock::new(pid);
            let p = execute_camera_submission(&mut mock, pid, &plan, &|| Ok(()));
            assert!(p.completed && p.error.is_none());
            assert_eq!(p.outcomes.len(), if automatic { 3 } else { 2 });
            let fdaf = match (pid, automatic) {
                (3595, true) => [0xc0, 10, 1, 2, 0, 0, 0, 0],
                (3595, false) => [0xc0, 10, 1, 1, 0, 0, 0, 0],
                (_, true) => [46, 0, 1, 0, 0, 0, 0, 0],
                (_, false) => [46, 0, 0, 0, 0, 0, 0, 0],
            };
            assert_eq!(mock.packets[0], fdaf);
            if automatic {
                assert_eq!(mock.packets[1], [255, 6, 1, 0, 0, 0, 0, 0]);
            }
            assert_eq!(mock.calls, plan.commands);
        }
    }
}
#[test]
fn camera_family_submission_partial_tracking_keeps_standard_and_fdaf_acknowledgements() {
    for pid in [3595, 3596] {
        let mut mock = FocusFamilySubmissionMock::new(pid);
        mock.fail_tracking = true;
        let plan = compile_camera_edit(pid, "autofocus", &profile()).unwrap();
        let p = execute_camera_submission(&mut mock, pid, &plan, &|| Ok(()));
        assert!(!p.completed && p.rejected_outcome.is_none() && p.unconfirmed.is_none());
        assert_eq!(p.outcomes.len(), 3);
        let CameraCommandOutcome::FamilyFocus(fdaf) = &p.outcomes[1] else {
            panic!("FDAF receipt absent")
        };
        let CameraCommandOutcome::FamilyFocus(tracking) = &p.outcomes[2] else {
            panic!("tracking receipt absent")
        };
        assert!(fdaf.completed && fdaf.attempts[0].acknowledged);
        assert!(
            !tracking.completed
                && tracking.attempts[0].set_attempted
                && tracking.attempts[0].get_attempted
        );
        assert!(p.error.unwrap().contains("reply loss"));
    }
}
#[test]
fn camera_family_submission_rejects_pro_or_other_product_and_forged_family_receipts() {
    let plan = compile_camera_edit(3595, "autofocus", &profile()).unwrap();
    let mut mock = FocusFamilySubmissionMock::new(3595);
    let p = execute_camera_submission(&mut mock, 3594, &plan, &|| Ok(()));
    assert!(!p.completed && mock.calls.is_empty());
    mock.reject_receipt = true;
    let p = execute_camera_submission(&mut mock, 3595, &plan, &|| Ok(()));
    assert!(!p.completed && p.outcomes.len() == 1);
    assert!(matches!(
        p.rejected_outcome,
        Some(CameraCommandOutcome::FamilyFocus(_))
    ));
    assert_eq!(p.unconfirmed, Some(plan.commands[1].clone()));
    assert_eq!(mock.calls.len(), 2);
}
#[test]
fn camera_family_submission_cancellation_after_fdaf_preserves_both_acknowledgements() {
    let mut mock = FocusFamilySubmissionMock::new(3596);
    let plan = compile_camera_edit(3596, "autofocus", &profile()).unwrap();
    let calls = Cell::new(0);
    let p = execute_camera_submission(&mut mock, 3596, &plan, &|| {
        calls.set(calls.get() + 1);
        // Submission checks 1/2, FDAF checks 3..8, next command check 9.
        if calls.get() == 9 {
            bail!("cancel before tracking");
        }
        Ok(())
    });
    assert!(!p.completed && p.outcomes.len() == 2 && p.unconfirmed.is_none());
    let CameraCommandOutcome::FamilyFocus(fdaf) = &p.outcomes[1] else {
        panic!("missing FDAF")
    };
    assert!(fdaf.completed && fdaf.attempts[0].acknowledged);
    assert_eq!(mock.calls.len(), 2);
}
#[test]
fn camera_submission_stops_on_failure_and_preserves_acknowledged_prefix() {
    let mut mock = Mock {
        fail: Some(3),
        ..Default::default()
    };
    let p = plan("image-preset");
    let receipt = execute_camera_submission(&mut mock, 3594, &p, &|| Ok(()));
    assert!(!receipt.completed);
    assert_eq!(receipt.outcomes.len(), 2);
    assert_eq!(receipt.unconfirmed, Some(p.commands[2].clone()));
    assert_eq!(mock.calls.len(), 3);
}
#[test]
fn camera_submission_cancellation_is_checked_before_every_command() {
    let checks = Cell::new(0);
    let mut mock = Mock::default();
    let receipt = execute_camera_submission(&mut mock, 3594, &plan("image-preset"), &|| {
        checks.set(checks.get() + 1);
        if checks.get() == 2 {
            bail!("cancelled");
        }
        Ok(())
    });
    assert_eq!(mock.calls.len(), 1);
    assert_eq!(receipt.outcomes.len(), 1);
    assert!(!receipt.completed);
    assert!(receipt.unconfirmed.is_none());
}
#[test]
fn camera_submission_focus_runner_keeps_private_failure_and_continues_ignored_false_results() {
    let mut mock = Mock::default();
    let receipt = execute_camera_submission(&mut mock, 3594, &plan("autofocus"), &|| Ok(()));
    assert_eq!(mock.calls.len(), 3);
    assert_eq!(receipt.outcomes.len(), 3);
    assert!(!receipt.completed);
    assert!(receipt.error.as_deref().unwrap().contains("KS"));
}
#[test]
fn camera_submission_rejects_model_private_commands_before_io() {
    let mut mock = Mock::default();
    assert!(!execute_camera_submission(&mut mock, 3595, &plan("mirror"), &|| Ok(())).completed);
    assert!(mock.calls.is_empty());
}

struct ReceiptMock {
    outcome: CameraCommandOutcome,
    calls: usize,
}
impl CameraCommandTransport for ReceiptMock {
    fn execute(
        &mut self,
        _: &CameraCommand,
        _: &impl Fn() -> Result<()>,
    ) -> Result<CameraCommandOutcome> {
        self.calls += 1;
        Ok(self.outcome.clone())
    }
}
fn submit_receipt(
    command: CameraCommand,
    outcome: CameraCommandOutcome,
) -> CameraSubmissionProgress {
    execute_camera_submission(
        &mut ReceiptMock { outcome, calls: 0 },
        3594,
        &CameraSubmissionPlan {
            commands: vec![command],
        },
        &|| Ok(()),
    )
}
fn assert_invalid_receipt(command: CameraCommand, outcome: CameraCommandOutcome) {
    let receipt = submit_receipt(command.clone(), outcome.clone());
    assert!(!receipt.completed);
    assert!(receipt.outcomes.is_empty());
    assert_eq!(receipt.rejected_outcome, Some(outcome));
    assert_eq!(receipt.unconfirmed, Some(command));
    assert!(
        receipt
            .error
            .as_deref()
            .unwrap()
            .contains("invalid camera transport receipt")
    );
}
fn valid_write(property: P, setting: CameraSetting) -> CameraCommandOutcome {
    CameraCommandOutcome::Write(StandardCameraWrite {
        property,
        requested: setting,
        acknowledgement: CameraWriteAck::from_hresult(1).unwrap(),
    })
}
#[test]
fn camera_submission_standard_read_requires_matching_property_and_successful_hresult() {
    let property = P::CameraControl(C::Exposure);
    let observation = CameraLongObservation::from_native(-11, 3, 1).unwrap();
    let valid = CameraCommandOutcome::Read(StandardCameraRead {
        property,
        observation,
    });
    let receipt = submit_receipt(CameraCommand::StandardRead(property), valid.clone());
    assert!(receipt.completed);
    assert_eq!(receipt.outcomes, vec![valid]);
    assert_invalid_receipt(
        CameraCommand::StandardRead(property),
        CameraCommandOutcome::Read(StandardCameraRead {
            property: P::CameraControl(C::Focus),
            observation,
        }),
    );
    assert_invalid_receipt(
        CameraCommand::StandardRead(property),
        CameraCommandOutcome::Read(StandardCameraRead {
            property,
            observation: CameraLongObservation {
                native_hresult: -1,
                ..observation
            },
        }),
    );
    assert_invalid_receipt(
        CameraCommand::StandardRead(property),
        valid_write(
            property,
            CameraSetting {
                value: -11,
                is_auto: false,
            },
        ),
    );
}
#[test]
fn camera_submission_standard_write_validates_property_value_mode_acceptance_and_hresult() {
    let property = P::VideoProcAmp(V::Brightness);
    let setting = CameraSetting {
        value: 17,
        is_auto: false,
    };
    let command = CameraCommand::StandardWrite(StandardCameraSetting { property, setting });
    let outcome = valid_write(property, setting);
    assert!(submit_receipt(command.clone(), outcome.clone()).completed);
    let CameraCommandOutcome::Write(valid) = outcome else {
        unreachable!()
    };
    for bad in [
        StandardCameraWrite {
            property: P::VideoProcAmp(V::Contrast),
            ..valid.clone()
        },
        StandardCameraWrite {
            requested: CameraSetting {
                value: 18,
                ..setting
            },
            ..valid.clone()
        },
        StandardCameraWrite {
            requested: CameraSetting {
                is_auto: true,
                ..setting
            },
            ..valid.clone()
        },
        StandardCameraWrite {
            acknowledgement: CameraWriteAck {
                accepted: false,
                native_hresult: 0,
            },
            ..valid.clone()
        },
        StandardCameraWrite {
            acknowledgement: CameraWriteAck {
                accepted: true,
                native_hresult: -1,
            },
            ..valid.clone()
        },
    ] {
        assert_invalid_receipt(command.clone(), CameraCommandOutcome::Write(bad));
    }
    assert_invalid_receipt(
        command,
        CameraCommandOutcome::Read(StandardCameraRead {
            property,
            observation: CameraLongObservation::from_native(17, 2, 0).unwrap(),
        }),
    );
}

#[test]
fn camera_submission_rejected_receipt_preserves_only_the_valid_prefix_and_stops_io() {
    struct SequentialReceiptMock {
        outcomes: Vec<CameraCommandOutcome>,
        calls: usize,
    }
    impl CameraCommandTransport for SequentialReceiptMock {
        fn execute(
            &mut self,
            _: &CameraCommand,
            _: &impl Fn() -> Result<()>,
        ) -> Result<CameraCommandOutcome> {
            let outcome = self.outcomes[self.calls].clone();
            self.calls += 1;
            Ok(outcome)
        }
    }
    let property = P::VideoProcAmp(V::Brightness);
    let setting = CameraSetting {
        value: 17,
        is_auto: false,
    };
    let read = CameraCommandOutcome::Read(StandardCameraRead {
        property,
        observation: CameraLongObservation::from_native(11, 2, 0).unwrap(),
    });
    let rejected = valid_write(
        property,
        CameraSetting {
            value: 19,
            ..setting
        },
    );
    let mut transport = SequentialReceiptMock {
        outcomes: vec![read.clone(), rejected.clone()],
        calls: 0,
    };
    let write_command = CameraCommand::StandardWrite(StandardCameraSetting { property, setting });
    let plan = CameraSubmissionPlan {
        commands: vec![
            CameraCommand::StandardRead(property),
            write_command.clone(),
            CameraCommand::StandardRead(property),
        ],
    };
    let receipt = execute_camera_submission(&mut transport, 3594, &plan, &|| Ok(()));
    assert_eq!(transport.calls, 2);
    assert!(!receipt.completed);
    assert_eq!(receipt.outcomes, vec![read]);
    assert_eq!(receipt.rejected_outcome, Some(rejected));
    assert_eq!(receipt.unconfirmed, Some(write_command));
}

#[test]
fn camera_submission_old_serialized_progress_has_no_rejected_receipt() {
    let progress: CameraSubmissionProgress = serde_json::from_value(serde_json::json!({
        "outcomes": [], "unconfirmed": null, "completed": false, "error": "cancelled"
    }))
    .unwrap();
    assert!(progress.rejected_outcome.is_none());
    assert_eq!(progress.error.as_deref(), Some("cancelled"));
}

#[derive(Default)]
struct KsReceiptMock {
    phase: Option<CameraExtensionPhase>,
    responses: Vec<[u8; 8]>,
    gets: usize,
}
impl CameraExtensionTransport for KsReceiptMock {
    fn begin_node(&mut self, _: u32) -> Result<()> {
        if self.phase == Some(CameraExtensionPhase::OpenControl) {
            bail!("simulated open failure");
        }
        Ok(())
    }
    fn ks_property(
        &mut self,
        header: &[u8; 32],
        data: &mut [u8; 8],
    ) -> Result<CameraKsCallOutcome> {
        if *header == camera_extension_header(true) {
            if self.phase == Some(CameraExtensionPhase::Set) {
                bail!("simulated SET failure");
            }
            return Ok(CameraKsCallOutcome {
                hresult: 1,
                bytes_returned: 0,
            });
        }
        self.gets += 1;
        if self.phase == Some(CameraExtensionPhase::Get) {
            bail!("simulated GET failure");
        }
        *data = self.responses.get(self.gets - 1).copied().unwrap_or([0; 8]);
        if self.phase == Some(CameraExtensionPhase::Decode) {
            data[0] = 2;
        }
        Ok(CameraKsCallOutcome {
            hresult: 0,
            bytes_returned: 8,
        })
    }
    fn end_node(&mut self) {}
    fn wait_ms(&mut self, _: u32) -> Result<()> {
        if self.phase == Some(CameraExtensionPhase::Wait) {
            bail!("simulated wait failure");
        }
        Ok(())
    }
}
fn private_receipt(request: E) -> CameraExtensionProgress {
    execute_camera_extension(&mut KsReceiptMock::default(), 3594, request, &|| Ok(()))
}
#[test]
fn camera_submission_private_completion_requires_every_acknowledged_exchange() {
    let request = E::SetAudioFunction {
        adaptive_noise_reduction: 1,
        beamforming: 0,
    };
    let valid = private_receipt(request);
    assert_eq!(valid.exchanges.len(), 3);
    assert!(
        submit_receipt(
            CameraCommand::Extension(request),
            CameraCommandOutcome::Extension(valid.clone())
        )
        .completed
    );
    let mut missing = valid.clone();
    missing.exchanges.pop();
    assert_invalid_receipt(
        CameraCommand::Extension(request),
        CameraCommandOutcome::Extension(missing),
    );
    let mut extra = valid.clone();
    extra.exchanges.push(valid.exchanges[0].clone());
    assert_invalid_receipt(
        CameraCommand::Extension(request),
        CameraCommandOutcome::Extension(extra),
    );
    let mut interrupted = valid;
    interrupted.exchanges[0].acknowledged = false;
    assert_invalid_receipt(
        CameraCommand::Extension(request),
        CameraCommandOutcome::Extension(interrupted),
    );
}
#[test]
fn camera_submission_private_receipt_rejects_wrong_request_command_metadata_and_phase_results() {
    let request = E::SetMirrorVideo { mode: 1 };
    let valid = private_receipt(request);
    for case in 0..19 {
        let mut bad = valid.clone();
        let e = &mut bad.exchanges[0];
        match case {
            0 => bad.request = E::SetMirrorVideo { mode: 0 },
            1 => e.command.bytes[2] = 0,
            2 => e.command.mutates_state = false,
            3 => e.command.native_rva += 1,
            4 => e.command.wait_before_ms = 10,
            5 => e.set_attempted = false,
            6 => e.set_outcome = None,
            7 => e.set_outcome.as_mut().unwrap().hresult = -1,
            8 => e.get_attempted = false,
            9 => e.get_outcome = None,
            10 => e.get_outcome.as_mut().unwrap().hresult = 1,
            11 => e.get_outcome.as_mut().unwrap().bytes_returned = 7,
            12 => e.response = None,
            13 => e.response.as_mut().unwrap()[0] = 1,
            14 => e.acknowledged = false,
            15 => bad.value = None,
            16 => bad.value = Some(CameraExtensionValue::Byte(1)),
            17 => {
                bad.failure = Some(CameraExtensionFailure {
                    phase: CameraExtensionPhase::Get,
                    command_index: 0,
                    message: "contradictory failure".into(),
                })
            }
            18 => bad.exchanges.clear(),
            _ => unreachable!(),
        }
        assert_invalid_receipt(
            CameraCommand::Extension(request),
            CameraCommandOutcome::Extension(bad),
        );
    }
}
#[test]
fn camera_submission_private_observations_must_match_retained_response_bytes() {
    let request = E::GetAutoframingSpeedAndSize;
    let valid = execute_camera_extension(
        &mut KsReceiptMock {
            responses: vec![[0, 0, 0, 7, 0, 0, 0, 0], [0, 0, 0, 9, 0, 0, 0, 0]],
            ..Default::default()
        },
        3594,
        request,
        &|| Ok(()),
    );
    assert_eq!(
        valid.value,
        Some(CameraExtensionValue::AutoframingSpeedAndSize { speed: 7, size: 9 })
    );
    assert!(
        submit_receipt(
            CameraCommand::Extension(request),
            CameraCommandOutcome::Extension(valid.clone())
        )
        .completed
    );
    let mut bad = valid;
    bad.value = Some(CameraExtensionValue::AutoframingSpeedAndSize { speed: 7, size: 0 });
    assert_invalid_receipt(
        CameraCommand::Extension(request),
        CameraCommandOutcome::Extension(bad),
    );
}
#[test]
fn camera_submission_retains_each_valid_private_failure_phase_without_marking_complete() {
    let request = E::SetMirrorVideo { mode: 1 };
    for phase in [
        CameraExtensionPhase::OpenControl,
        CameraExtensionPhase::Set,
        CameraExtensionPhase::Wait,
        CameraExtensionPhase::Get,
        CameraExtensionPhase::Decode,
    ] {
        let failed = execute_camera_extension(
            &mut KsReceiptMock {
                phase: Some(phase),
                ..Default::default()
            },
            3594,
            request,
            &|| Ok(()),
        );
        assert_eq!(failed.failure.as_ref().unwrap().phase, phase);
        let receipt = submit_receipt(
            CameraCommand::Extension(request),
            CameraCommandOutcome::Extension(failed.clone()),
        );
        assert!(!receipt.completed);
        assert!(receipt.unconfirmed.is_none());
        assert_eq!(
            receipt.outcomes,
            vec![CameraCommandOutcome::Extension(failed.clone())]
        );
        assert_eq!(receipt.error, Some(failed.failure.unwrap().message));
    }
}
#[test]
fn camera_submission_private_validation_cancellation_keeps_receipts_at_all_boundaries() {
    let request = E::SetMirrorVideo { mode: 1 };
    for stop in 1..=5 {
        let checks = Cell::new(0);
        let failed =
            execute_camera_extension(&mut KsReceiptMock::default(), 3594, request, &|| {
                checks.set(checks.get() + 1);
                if checks.get() == stop {
                    bail!("simulated cancellation");
                }
                Ok(())
            });
        assert_eq!(
            failed.failure.as_ref().unwrap().phase,
            CameraExtensionPhase::Validation
        );
        let receipt = submit_receipt(
            CameraCommand::Extension(request),
            CameraCommandOutcome::Extension(failed),
        );
        assert!(!receipt.completed);
        assert!(receipt.unconfirmed.is_none());
        assert_eq!(receipt.error.as_deref(), Some("simulated cancellation"));
    }
}

#[test]
fn camera_submission_compound_private_cancellation_preserves_valid_phase_and_prefix() {
    let request = E::SetAudioFunction {
        adaptive_noise_reduction: 1,
        beamforming: 0,
    };
    for stop in 1..=17 {
        let checks = Cell::new(0);
        let failed =
            execute_camera_extension(&mut KsReceiptMock::default(), 3594, request, &|| {
                checks.set(checks.get() + 1);
                if checks.get() == stop {
                    bail!("simulated cancellation");
                }
                Ok(())
            });
        assert_eq!(
            failed.failure.as_ref().unwrap().phase,
            CameraExtensionPhase::Validation
        );
        let receipt = submit_receipt(
            CameraCommand::Extension(request),
            CameraCommandOutcome::Extension(failed.clone()),
        );
        assert!(!receipt.completed);
        assert!(
            receipt.rejected_outcome.is_none(),
            "rejected cancellation boundary {stop}"
        );
        assert_eq!(
            receipt.outcomes,
            vec![CameraCommandOutcome::Extension(failed)]
        );
        assert_eq!(receipt.error.as_deref(), Some("simulated cancellation"));
    }
}
#[test]
fn camera_submission_private_failure_phase_must_match_attempts_and_cannot_publish_values() {
    let request = E::SetMirrorVideo { mode: 1 };
    let failed = execute_camera_extension(
        &mut KsReceiptMock {
            phase: Some(CameraExtensionPhase::Set),
            ..Default::default()
        },
        3594,
        request,
        &|| Ok(()),
    );
    for case in 0..6 {
        let mut bad = failed.clone();
        match case {
            0 => bad.failure = None,
            1 => bad.failure.as_mut().unwrap().phase = CameraExtensionPhase::Get,
            2 => bad.failure.as_mut().unwrap().command_index = 1,
            3 => bad.value = Some(CameraExtensionValue::Acknowledged),
            4 => bad.exchanges.clear(),
            5 => bad.failure.as_mut().unwrap().message.clear(),
            _ => unreachable!(),
        }
        assert_invalid_receipt(
            CameraCommand::Extension(request),
            CameraCommandOutcome::Extension(bad),
        );
    }
}
#[test]
fn camera_submission_cancellation_after_last_ack_retains_receipt_and_stops_completion() {
    let checks = Cell::new(0);
    let mut mock = Mock::default();
    let p = plan("brightness");
    let receipt = execute_camera_submission(&mut mock, 3594, &p, &|| {
        checks.set(checks.get() + 1);
        if checks.get() == 2 {
            bail!("cancelled after ACK");
        }
        Ok(())
    });
    assert!(!receipt.completed);
    assert_eq!(receipt.outcomes.len(), 1);
    assert!(receipt.unconfirmed.is_none());
    assert_eq!(mock.calls, p.commands);
    assert_eq!(receipt.error.as_deref(), Some("cancelled after ACK"));
}
