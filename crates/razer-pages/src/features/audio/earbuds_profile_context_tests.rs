use super::*;
use razer_device::earbuds_gesture::TapSpeed;
use razer_storage::earbuds_profiles as profiles;

#[gpui_kit::test]
fn unknown_toggle_replies_reach_each_control_without_becoming_off(cx: &mut TestAppContext) {
    let (window, owner, _) = mount_with_dnd(2660, Some(false), cx);
    cx.update_window(window, |_, _, cx| {
        owner.update(cx, |view, cx| {
            let gate = view.earbuds.pending.clone().unwrap();
            view.finish_earbuds_request(&gate, Ok(reading(2660, false)), cx);
            finish_pending_firmware(view, cx);
            for _ in 0..5 {
                let request = view.earbuds.pending.clone().unwrap();
                assert!(request.is_profile_restore());
                let mut report = reading(2660, false);
                report.connected = None;
                report.operation = request.operation();
                report.write = razer_device::earbuds_protocol40::WriteOutcome::Failed(
                    "Unknown boolean byte 2".into(),
                );
                match request.operation() {
                    Operation::DoNotDisturb { .. } => {
                        report.do_not_disturb = Some(AutoPauseObservation::Unknown(2))
                    }
                    Operation::RestoreGesture { .. } => {
                        report.gesture = Some(AutoPauseObservation::Unknown(2))
                    }
                    Operation::RestoreVoicePrompt { .. } => {
                        report.voice_prompt = Some(AutoPauseObservation::Unknown(2))
                    }
                    Operation::Restore { .. } => {
                        report.observation = AutoPauseObservation::Unknown(2)
                    }
                    Operation::RestoreTapSpeed { .. } => {
                        report.tap_speed =
                            Some(razer_device::earbuds_gesture::TapObservation::Observed(60));
                        report.write = razer_device::earbuds_protocol40::WriteOutcome::Skipped;
                    }
                    _ => panic!("unexpected restore operation"),
                }
                view.finish_earbuds_request(&request, Ok(report), cx);
            }
            assert_eq!(view.earbuds.dnd_device, AutoPauseObservation::Unknown(2));
            assert_eq!(
                view.earbuds.gesture.device,
                AutoPauseObservation::Unknown(2)
            );
            assert_eq!(view.earbuds.voice_device, AutoPauseObservation::Unknown(2));
            assert_eq!(
                view.earbuds.controls.as_ref().unwrap().auto_pause_device,
                AutoPauseObservation::Unknown(2)
            );
            assert_eq!(view.earbuds.dnd_persisted, Some(false));
            assert_eq!(view.earbuds.gesture.persisted, Some(true));
            assert!(view.earbuds.pending.is_none());
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn same_guid_control_updates_cancel_old_writes_and_preserve_observations(cx: &mut TestAppContext) {
    for field in ["auto", "dnd", "voice", "gesture", "tap", "anc"] {
        let (window, owner, mut device) = mount(2657, cx);
        cx.update_window(window, |_, window, cx| {
            owner.update(cx, |view, cx| {
                let gate = view.earbuds.pending.clone().unwrap();
                view.finish_earbuds_request(&gate, Ok(reading(2657, false)), cx);
                finish_pending_firmware(view, cx);
                finish_initial_restores(view, 2657, cx);
            });
            window.render_frame(cx);
            window.click("earbuds-auto-pause", cx);
            owner.update(cx, |view, cx| {
                let old = view.earbuds.pending.clone().unwrap();
                assert_eq!(old.operation(), Operation::Write { enabled: true });
                let monitor = old.monitor_guard();
                let generation = view.earbuds.controls.as_ref().unwrap().generation();
                let profile = device.active_profile.clone();
                match field {
                    "auto" => profiles::merge_auto_pause(&mut device, &profile, true).unwrap(),
                    "dnd" => profiles::merge_do_not_disturb(&mut device, &profile, true).unwrap(),
                    "voice" => profiles::merge_voice_prompt(&mut device, &profile, false).unwrap(),
                    "gesture" => profiles::merge_gesture(&mut device, &profile, false).unwrap(),
                    "tap" => {
                        profiles::merge_tap_speed(&mut device, &profile, TapSpeed::Fast).unwrap()
                    }
                    "anc" => profiles::merge_anc(
                        &mut device,
                        &profile,
                        razer_device::earbuds_protocol40::ANC_UI_INITIAL,
                    )
                    .unwrap(),
                    _ => unreachable!(),
                }
                view.set_earbuds_context(&device, true, cx);
                assert!(!old.guard().load(Ordering::Acquire), "{field}");
                assert!(!view.earbuds_request_current(&old));
                assert!(Arc::ptr_eq(
                    &monitor,
                    view.earbuds.monitor.as_ref().unwrap()
                ));
                assert!(monitor.load(Ordering::Acquire));
                let controls = view.earbuds.controls.as_ref().unwrap();
                assert_eq!(controls.generation(), generation);
                assert_eq!(
                    controls.auto_pause_device,
                    AutoPauseObservation::Observed(false)
                );
                assert_eq!(controls.firmware_version.as_deref(), Some("1.5.0.0"));
                view.finish_earbuds_request(
                    &old,
                    Err("old request finished after replacement".into()),
                    cx,
                );
                assert!(view.earbuds.pending.as_ref().unwrap().is_profile_restore());
                finish_initial_restores(view, 2657, cx);
                let controls = view.earbuds.controls.as_ref().unwrap();
                assert_eq!(controls.auto_pause_persisted, Some(field == "auto"));
                match field {
                    "auto" => assert!(controls.draft.auto_pause),
                    "dnd" => assert!(view.earbuds.dnd_draft),
                    "voice" => assert!(!view.earbuds.voice_draft),
                    "gesture" => assert!(!view.earbuds.gesture.draft),
                    "tap" => assert_eq!(view.earbuds.gesture.tap_draft, TapSpeed::Fast),
                    "anc" => assert_eq!(
                        controls.anc_persisted,
                        Some(razer_device::earbuds_protocol40::ANC_UI_INITIAL)
                    ),
                    _ => unreachable!(),
                }
                assert!(view.earbuds.pending.is_none());
            });
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn same_guid_update_keeps_pending_firmware_and_uses_new_restore_values(cx: &mut TestAppContext) {
    let (window, owner, mut device) = mount(2660, cx);
    cx.update_window(window, |_, _, cx| {
        owner.update(cx, |view, cx| {
            let gate = view.earbuds.pending.clone().unwrap();
            view.finish_earbuds_request(&gate, Ok(reading(2660, false)), cx);
            let firmware = view.earbuds.pending.clone().unwrap();
            let profile = device.active_profile.clone();
            profiles::merge_voice_prompt(&mut device, &profile, false).unwrap();
            view.set_earbuds_context(&device, true, cx);
            assert!(view.earbuds_request_current(&firmware));
            assert_eq!(firmware.operation(), Operation::Firmware);
            finish_pending_firmware(view, cx);
            finish_initial_restores(view, 2660, cx);
            assert_eq!(view.earbuds.voice_persisted, Some(false));
            assert!(!view.earbuds.voice_draft);
            assert!(view.earbuds.pending.is_none());
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn own_save_and_profile_metadata_changes_do_not_restart_device_restore(cx: &mut TestAppContext) {
    let (window, owner, mut device) = mount(2660, cx);
    cx.update_window(window, |_, window, cx| {
        owner.update(cx, |view, cx| {
            let gate = view.earbuds.pending.clone().unwrap();
            view.finish_earbuds_request(&gate, Ok(reading(2660, false)), cx);
            finish_pending_firmware(view, cx);
            finish_initial_restores(view, 2660, cx);
        });
        window.render_frame(cx);
        window.click("earbuds-auto-pause", cx);
        owner.update(cx, |view, cx| {
            let request = view.earbuds.pending.clone().unwrap();
            let profile = device.active_profile.clone();
            profiles::merge_auto_pause(&mut device, &profile, true).unwrap();
            assert!(view.mark_earbuds_persisted(&request));
            device.profiles[0].name = "Renamed".into();
            device.profiles[0].source_settings.as_mut().unwrap()["unrelated"] = json!(42);
            view.set_earbuds_context(&device, true, cx);
            assert!(view.earbuds_request_current(&request));
            assert!(view.earbuds.restore_sequence.is_empty());
            view.finish_earbuds_request(&request, Err("SET rejected".into()), cx);
            view.set_earbuds_context(&device, true, cx);
            assert!(view.earbuds.pending.is_none());
            assert!(view.earbuds.controls.as_ref().unwrap().draft.auto_pause);
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn invalid_same_guid_voice_field_blocks_default_restore_until_repaired(cx: &mut TestAppContext) {
    let (window, owner, mut device) = mount(2660, cx);
    cx.update_window(window, |_, _, cx| {
        owner.update(cx, |view, cx| {
            let gate = view.earbuds.pending.clone().unwrap();
            view.finish_earbuds_request(&gate, Ok(reading(2660, false)), cx);
            finish_pending_firmware(view, cx);
            finish_initial_restores(view, 2660, cx);
            device.profiles[0].source_settings.as_mut().unwrap()["profile"]["audioVoicePromptControl"] = json!({"isEnabled":"invalid"});
            view.set_earbuds_context(&device, true, cx);
            assert!(view.earbuds.voice_profile_error.is_some());
            assert!(view.earbuds.restore_sequence.iter().all(|op| !matches!(op, Operation::RestoreVoicePrompt { .. })));
            finish_initial_restores(view, 2660, cx);
            device.profiles[0].source_settings.as_mut().unwrap()["profile"]["audioVoicePromptControl"] = json!({"isEnabled":false});
            view.set_earbuds_context(&device, true, cx);
            finish_initial_restores(view, 2660, cx);
            assert!(view.earbuds.voice_profile_error.is_none());
            assert_eq!(view.earbuds.voice_persisted, Some(false));
        });
    }).unwrap();
}

#[gpui_kit::test]
fn workspace_discard_restores_updated_same_guid_earbud_profile(cx: &mut TestAppContext) {
    use crate::features::source_workspace::SourceProductWorkspace;
    cx.update(gpui_kit::init);
    let mut device = razer_model::product_preview::registered_preview(2660).unwrap();
    profiles::initialize_auto_pause_profile(&mut device, "retained", "Test-Default").unwrap();
    profiles::merge_auto_pause(&mut device, "retained", false).unwrap();
    let mut owner = None;
    let window = cx.open_window(size(px(1600.), px(1000.)), |window, cx| {
        let view = cx.new(|cx| SourceProductWorkspace::new(device, window, cx));
        owner = Some(view.clone());
        Root::new(view, window, cx)
    });
    let owner = owner.unwrap();
    cx.update_window(window.into(), |_, window, cx| {
        owner.update(cx, |workspace, cx| {
            workspace.observe_connection(
                Some(razer_model::model::DeviceConnectionObservation::UsbPresent),
                cx,
            );
            workspace.set_active(true, window, cx);
            let page = workspace.audio_mixer_page().unwrap();
            page.update(cx, |view, cx| {
                let gate = view.earbuds.pending.clone().unwrap();
                view.finish_earbuds_request(&gate, Ok(reading(2660, false)), cx);
                finish_pending_firmware(view, cx);
                finish_initial_restores(view, 2660, cx);
            });
            let monitor = page.read(cx).earbuds.monitor.clone().unwrap();
            let mut saved = workspace.saved_snapshot();
            profiles::merge_auto_pause(&mut saved, "retained", true).unwrap();
            workspace.mark_saved(saved, cx);
            workspace.discard(window, cx);
            page.update(cx, |view, cx| {
                assert_eq!(
                    view.earbuds.controls.as_ref().unwrap().auto_pause_persisted,
                    Some(true)
                );
                assert!(Arc::ptr_eq(
                    &monitor,
                    view.earbuds.monitor.as_ref().unwrap()
                ));
                assert!(view.earbuds.pending.as_ref().unwrap().is_profile_restore());
                finish_initial_restores(view, 2660, cx);
                assert!(view.earbuds.controls.as_ref().unwrap().draft.auto_pause);
            });
        });
    })
    .unwrap();
}
