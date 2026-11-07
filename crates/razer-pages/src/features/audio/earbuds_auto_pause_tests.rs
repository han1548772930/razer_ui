use super::{AudioProductWorkspace, AutoPauseObservation, DndObservation, Operation, Report};
use gpui_kit::{
    AnyWindowHandle, AppContext, Entity, InputEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, TestAppContext, component::Root, point, px, size, test::TestWindowExt,
};
use serde_json::json;
#[path = "earbuds_anc_tests.rs"]
mod anc;
#[path = "earbuds_gesture_tests.rs"]
mod gesture;
#[path = "earbuds_profile_context_tests.rs"]
mod profile_context;
#[path = "earbuds_voice_prompt_tests.rs"]
mod voice_prompt;
#[path = "earbuds_equalizer_view_tests.rs"]
mod equalizer_view;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

fn mount(
    pid: u32,
    cx: &mut TestAppContext,
) -> (
    AnyWindowHandle,
    Entity<AudioProductWorkspace>,
    razer_model::model::Device,
) {
    mount_with_dnd(pid, None, cx)
}

fn mount_with_dnd(
    pid: u32,
    dnd: Option<bool>,
    cx: &mut TestAppContext,
) -> (
    AnyWindowHandle,
    Entity<AudioProductWorkspace>,
    razer_model::model::Device,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let mut device = razer_model::product_preview::registered_preview(pid).unwrap();
    device.active_profile = "active".into();
    device.profiles = vec![razer_model::model::Profile {
        id: "active".into(),
        guid: "source-active".into(),
        name: "Active".into(),
        source_settings: Some(
            json!({"profile":{"audioGestureControl":{"isEnabled":true},"audioTapSpeed":{"tapSpeed":"normal"}}}),
        ),
        settings: None,
        dpi_stages: None,
    }];
    device.device_container_id = "test-earbud-container".into();
    device.dashboard.connection_observation =
        Some(razer_model::model::DeviceConnectionObservation::UsbPresent);
    let profile = device.active_profile.clone();
    razer_storage::earbuds_profiles::merge_auto_pause(&mut device, &profile, false).unwrap();
    if let Some(enabled) = dnd {
        razer_storage::earbuds_profiles::merge_do_not_disturb(&mut device, &profile, enabled)
            .unwrap();
    }
    if pid == 2657 {
        razer_storage::earbuds_profiles::merge_anc(
            &mut device,
            &profile,
            razer_device::earbuds_protocol40::ANC_PROFILE_DEFAULT,
        )
        .unwrap();
    }
    let mut owner = None;
    let window = cx.open_window(size(px(1600.), px(1000.)), |window, cx| {
        let view = cx.new(|cx| {
            let mut view = AudioProductWorkspace::new(pid, window, cx);
            view.set_page("TAB_ENHANCEMENT", window, cx);
            view.set_earbuds_context(&device, true, cx);
            view
        });
        owner = Some(view.clone());
        Root::new(view, window, cx)
    });
    (window.into(), owner.unwrap(), device)
}

fn reading(pid: u32, _enabled: bool) -> Report {
    serde_json::from_value(json!({"product_id":pid,"operation":"Read","connected":true,
        "left_battery":80,"right_battery":90,"observation":"Unobserved","write":"Skipped"}))
    .unwrap()
}

fn finish_initial_restores(
    view: &mut AudioProductWorkspace,
    pid: u32,
    cx: &mut gpui_kit::Context<AudioProductWorkspace>,
) {
    for _ in 0..8 {
        let Some(request) = view
            .earbuds
            .pending
            .clone()
            .filter(|request| request.is_profile_restore())
        else {
            return;
        };
        assert!(!request.needs_persistence());
        let mut report = reading(pid, false);
        report.operation = request.operation();
        report.connected = None;
        match request.operation() {
            Operation::Equalizer { .. } => {}
            Operation::Anc { setting } => {
                report.anc = Some(razer_device::earbuds_anc::Report {
                    runtime: razer_device::earbuds_protocol40::AncRuntimeState(setting),
                    write: razer_device::earbuds_protocol40::AncWrite::Skipped,
                    confirmation: None,
                })
            }
            Operation::DoNotDisturb { enabled } => {
                report.do_not_disturb = Some(DndObservation::Observed(enabled))
            }
            Operation::RestoreGesture { enabled } => {
                report.gesture = Some(
                    razer_device::earbuds_protocol40::ToggleObservation::Observed(
                        enabled.unwrap_or(true),
                    ),
                )
            }
            Operation::RestoreTapSpeed { speed } => {
                report.tap_speed = Some(razer_device::earbuds_gesture::TapObservation::Observed(
                    speed.unwrap_or_default().ticks(),
                ))
            }
            Operation::RestoreVoicePrompt { enabled } => {
                report.voice_prompt = Some(
                    razer_device::earbuds_protocol40::ToggleObservation::Observed(
                        enabled.unwrap_or(true),
                    ),
                )
            }
            Operation::Restore { enabled } => {
                report.observation = AutoPauseObservation::Observed(enabled.unwrap_or(true))
            }
            _ => panic!("unexpected initial restore"),
        }
        view.finish_earbuds_request(&request, Ok(report), cx);
    }
    panic!("initial restore loop did not complete");
}

fn finish_pending_firmware(
    view: &mut AudioProductWorkspace,
    cx: &mut gpui_kit::Context<AudioProductWorkspace>,
) {
    let Some(request) = view
        .earbuds
        .pending
        .clone()
        .filter(|request| request.operation() == Operation::Firmware)
    else {
        return;
    };
    let report = serde_json::from_value(json!({"product_id":view.spec.product_id,"operation":"Firmware",
        "connected":null,"left_battery":null,"right_battery":null,"observation":"Unobserved","write":"Skipped",
        "firmware":{"Observed":{"version":{"major":1,"minor":5,"internal":0,"reserved":0},"source":"DockExtended","dock_failure":null}}})).unwrap();
    assert!(!request.needs_persistence());
    view.finish_earbuds_request(&request, Ok(report), cx);
}

fn finish_gesture_restores(
    view: &mut AudioProductWorkspace,
    pid: u32,
    cx: &mut gpui_kit::Context<AudioProductWorkspace>,
) {
    for gesture in [true, false] {
        let request = view.earbuds.pending.clone().unwrap();
        if gesture {
            assert!(matches!(
                request.operation(),
                Operation::RestoreGesture { .. }
            ));
        } else {
            assert!(matches!(
                request.operation(),
                Operation::RestoreTapSpeed { .. }
            ));
        }
        assert!(request.is_profile_restore());
        assert!(!request.needs_persistence());
        let mut report = reading(pid, false);
        report.operation = request.operation();
        report.connected = None;
        report.observation = AutoPauseObservation::Unobserved;
        if gesture {
            report.gesture =
                Some(razer_device::earbuds_protocol40::ToggleObservation::Observed(true));
        } else {
            report.tap_speed = Some(razer_device::earbuds_gesture::TapObservation::Observed(60));
        }
        view.finish_earbuds_request(&request, Ok(report), cx);
    }
}

fn finish_anc_restore(
    view: &mut AudioProductWorkspace,
    pid: u32,
    cx: &mut gpui_kit::Context<AudioProductWorkspace>,
) {
    if pid != 2657 {
        return;
    }
    let request = view.earbuds.pending.clone().unwrap();
    let Operation::Anc { setting } = request.operation() else {
        panic!("ANC restore must precede the other 2657 features")
    };
    assert!(request.is_profile_restore());
    assert!(!request.needs_persistence());
    let mut report = reading(pid, false);
    report.operation = request.operation();
    report.connected = None;
    report.anc = Some(razer_device::earbuds_anc::Report {
        runtime: razer_device::earbuds_protocol40::AncRuntimeState(setting),
        write: razer_device::earbuds_protocol40::AncWrite::Skipped,
        confirmation: None,
    });
    view.finish_earbuds_request(&request, Ok(report), cx);
}

fn dnd_written(pid: u32, enabled: bool) -> Report {
    serde_json::from_value(
        json!({"product_id":pid,"operation":{"DoNotDisturb":{"enabled":enabled}},
        "connected":null,"left_battery":null,"right_battery":null,"observation":"Unobserved",
        "write":"Confirmed","do_not_disturb":{"Observed":enabled}}),
    )
    .unwrap()
}

fn failed_dnd(pid: u32, enabled: bool, observed: bool) -> Report {
    serde_json::from_value(
        json!({"product_id":pid,"operation":{"DoNotDisturb":{"enabled":enabled}},
        "connected":null,"left_battery":null,"right_battery":null,"observation":"Unobserved",
        "write":{"Failed":"SET failed"},"do_not_disturb":{"Observed":observed}}),
    )
    .unwrap()
}

#[gpui_kit::test]
fn failed_dnd_set_preserves_the_fresh_get_and_the_optimistic_draft(cx: &mut TestAppContext) {
    let (window, owner, _) = mount(2660, cx);
    cx.update_window(window, |_, window, cx| {
        owner.update(cx, |view, cx| {
            let read = view.earbuds.pending.clone().unwrap();
            view.finish_earbuds_request(&read, Ok(reading(2660, false)), cx);
            finish_pending_firmware(view, cx);
            finish_initial_restores(view, view.spec.product_id, cx);
        });
        for (enabled, observed) in [(true, false), (false, true)] {
            window.render_frame(cx);
            window.click("earbuds-do-not-disturb", cx);
            owner.update(cx, |view, cx| {
                let request = view.earbuds.pending.clone().unwrap();
                assert_eq!(request.operation(), Operation::DoNotDisturb { enabled });
                view.mark_earbuds_persisted(&request);
                view.finish_earbuds_request(&request, Ok(failed_dnd(2660, enabled, observed)), cx);
                assert_eq!(view.earbuds.dnd_device, DndObservation::Observed(observed));
                assert_eq!(view.earbuds.dnd_draft, enabled);
                assert_eq!(view.earbuds.dnd_persisted, Some(enabled));
                assert_eq!(view.earbuds.dnd_error.as_deref(), Some("SET failed"));
            });
        }
    })
    .unwrap();
}

fn auto_restored(pid: u32, enabled: bool) -> Report {
    serde_json::from_value(json!({"product_id":pid,"operation":{"Restore":{"enabled":enabled}},
        "connected":null,"left_battery":null,"right_battery":null,"observation":{"Observed":enabled},"write":"Skipped"})).unwrap()
}

#[gpui_kit::test]
fn equalizer_loaded_curve_reads_current_profile_without_overwriting_newer_edit(
    cx: &mut TestAppContext,
) {
    let (window, owner, mut device) = mount(2660, cx);
    device.profiles.clear();
    device.active_profile.clear();
    razer_storage::earbuds_profiles::initialize_auto_pause_profile(
        &mut device,
        "eq-sync",
        "TEST-Default",
    )
    .unwrap();
    let key = cx.update(|cx| owner.read(cx).spec.equalizers[0].key.clone());
    cx.update_window(window, |_, _, cx| {
        owner.update(cx, |view, cx| {
            view.earbuds.cancel();
            view.earbuds.pending = None;
            view.set_earbuds_context(&device, true, cx);
            let read = view.earbuds.pending.clone().unwrap();
            view.finish_earbuds_request(&read, Ok(reading(2660, false)), cx);
            finish_pending_firmware(view, cx);
            finish_initial_restores(view, 2660, cx);
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let view = owner.read(cx);
        assert_eq!(
            view.earbuds_equalizer_setting().unwrap().bands,
            [2, 2, 3, 4, 1, -1, 2, 3, 3, 3]
        );
        assert!(
            !view
                .sliders
                .keys()
                .any(|key| key.starts_with("/equalizers/"))
        );
    });
    device.profiles[0].source_settings.as_mut().unwrap()["profile"]["audioEqualizer"]["presets"]
        ["standard"][0]["current"] = json!([5, 5, 5, 5, 5, 5, 5, 5, 5, 5]);
    cx.update_window(window, |_, window, cx| {
        owner.update(cx, |view, cx| {
            view.set_earbuds_context(&device, true, cx);
            view.edit(&format!("/equalizers/{key}/bands/0"), json!(-4), window, cx);
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let view = owner.read(cx);
        assert_eq!(view.earbuds_equalizer_setting().unwrap().bands[0], -4);
        assert_eq!(view.draft["equalizers"][&key]["bands"][0], -4);
    });
}

#[gpui_kit::test]
fn equalizer_chart_previews_until_global_release_and_zero_uses_the_device_queue(
    cx: &mut TestAppContext,
) {
    use razer_device::earbuds_equalizer::Preset;
    for pid in [2657, 2660, 2664, 2668, 2689] {
        let (window, owner, mut device) = mount(pid, cx);
        device.profiles.clear();
        device.active_profile.clear();
        razer_storage::earbuds_profiles::initialize_auto_pause_profile(
            &mut device,
            "eq-graph",
            "TEST-Default",
        )
        .unwrap();
        cx.update_window(window, |_, window, cx| {
            owner.update(cx, |view, cx| {
                view.earbuds.cancel();
                view.earbuds.pending = None;
                view.set_earbuds_context(&device, true, cx);
                let request = view.earbuds.pending.clone().unwrap();
                view.finish_earbuds_request(&request, Ok(reading(pid, false)), cx);
                finish_pending_firmware(view, cx);
                finish_initial_restores(view, pid, cx);
                view.set_page("TAB_SOUND", window, cx);
            });
            window.render_frame(cx);
            let panel = window.find("earbuds-eq-panel").bounds();
            assert_eq!(panel.size.width, window.rem_size() * (1220. / 16.));
            assert_eq!(panel.size.height, window.rem_size() * (544. / 16.));
            let bounds = window.find("earbuds-eq-chart").bounds();
            assert_eq!(bounds.size.width, window.rem_size() * (1002. / 16.));
            assert!(panel.contains(&window.find("earbuds-eq-zero").bounds().center()));
            let scale = f32::from(bounds.size.width) / 1002.;
            let from = bounds.origin + point(px(4. * scale), px(150. * scale));
            let outside = bounds.origin + point(px(200. * scale), px(400. * scale));
            let initial = owner.read(cx).earbuds_equalizer_setting().unwrap();
            window.click_at(
                "earbuds-eq-chart",
                point(px(30. * scale), px(150. * scale)),
                cx,
            );
            assert!(
                owner.read(cx).earbuds.pending.is_none(),
                "Column gaps do not start a drag"
            );
            assert_eq!(owner.read(cx).earbuds_equalizer_setting().unwrap(), initial);
            let unchanged_y = 8. + (6. - f32::from(initial.bands[0])) * 276. / 13.;
            window.click_at(
                "earbuds-eq-chart",
                point(px(4. * scale), px(unchanged_y * scale)),
                cx,
            );
            let unchanged = owner
                .read(cx)
                .earbuds
                .pending
                .clone()
                .expect("An unchanged-level click still submits custom");
            assert_eq!(
                unchanged.operation(),
                Operation::Equalizer {
                    setting: razer_device::earbuds_equalizer::Setting {
                        preset: Preset::Custom,
                        ..initial
                    }
                }
            );
            owner.update(cx, |view, cx| {
                view.finish_earbuds_request(&unchanged, Err("test transport failure".into()), cx)
            });
            let initial = owner.read(cx).earbuds_equalizer_setting().unwrap();
            window.simulate_mouse_move(from, cx);
            window.render_frame(cx);
            window.dispatch_event(
                MouseDownEvent {
                    button: MouseButton::Left,
                    position: from,
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
            window.render_frame(cx);
            window.dispatch_event(
                MouseMoveEvent {
                    pressed_button: Some(MouseButton::Left),
                    position: outside,
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
            window.render_frame(cx);
            assert!(owner.read(cx).earbuds.pending.is_none());
            assert_eq!(
                owner.read(cx).earbuds_equalizer_setting().unwrap(),
                initial,
                "Pointer preview must not persist before release"
            );
            let release = MouseUpEvent {
                button: MouseButton::Left,
                position: outside,
                ..Default::default()
            };
            window.dispatch_event(release.clone().to_platform_input(), cx);
            window.render_frame(cx);
            let request = owner
                .read(cx)
                .earbuds
                .pending
                .clone()
                .expect("Outside release submits EQ");
            let mut expected = initial;
            expected.preset = Preset::Custom;
            expected.bands[0] = -6;
            assert_eq!(
                request.operation(),
                Operation::Equalizer { setting: expected }
            );
            assert!(request.needs_persistence());
            window.dispatch_event(release.to_platform_input(), cx);
            assert_eq!(
                owner.read(cx).earbuds.pending.as_ref().unwrap().id(),
                request.id()
            );
            assert!(
                owner.read(cx).earbuds.edits.is_empty(),
                "One drag must submit once"
            );
            owner.update(cx, |view, cx| {
                view.finish_earbuds_request(&request, Err("test transport failure".into()), cx)
            });
            window.render_frame(cx);
            window.click("earbuds-eq-zero", cx);
            let zero = owner.read(cx).earbuds.pending.clone().unwrap();
            expected.bands = [0; 10];
            assert_eq!(zero.operation(), Operation::Equalizer { setting: expected });
            owner.update(cx, |view, cx| view.set_earbuds_context(&device, false, cx));
            window.render_frame(cx);
            let disconnected_draft = owner.read(cx).draft.clone();
            window.click_at(
                "earbuds-eq-chart",
                point(px(4. * scale), px(12. * scale)),
                cx,
            );
            assert_eq!(window.find("earbuds-eq-zero").disabled(), Some(true));
            window.click("earbuds-eq-zero", cx);
            assert_eq!(
                owner.read(cx).earbuds.pending.as_ref().unwrap().id(),
                zero.id()
            );
            assert!(!zero.guard().load(Ordering::Acquire));
            assert!(owner.read(cx).earbuds.edits.is_empty());
            assert_eq!(owner.read(cx).draft, disconnected_draft);
            owner.update(cx, |view, cx| {
                view.finish_earbuds_request(&zero, Err("cancelled".into()), cx)
            });
            assert!(owner.read(cx).earbuds.pending.is_none());
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn equalizer_chart_discards_drag_when_profile_page_or_connection_changes(cx: &mut TestAppContext) {
    let (window, owner, mut device) = mount(2660, cx);
    device.profiles.clear();
    device.active_profile.clear();
    razer_storage::earbuds_profiles::initialize_auto_pause_profile(
        &mut device,
        "eq-graph",
        "TEST-Default",
    )
    .unwrap();
    cx.update_window(window, |_, window, cx| {
        owner.update(cx, |view, cx| {
            view.earbuds.cancel();
            view.earbuds.pending = None;
            view.set_earbuds_context(&device, true, cx);
            let request = view.earbuds.pending.clone().unwrap();
            view.finish_earbuds_request(&request, Ok(reading(2660, false)), cx);
            finish_pending_firmware(view, cx);
            finish_initial_restores(view, 2660, cx);
            view.set_page("TAB_SOUND", window, cx);
        });
        for change in 0..3 {
            window.render_frame(cx);
            let bounds = window.find("earbuds-eq-chart").bounds();
            let from = bounds.origin + point(px(4.), px(150.));
            window.simulate_mouse_move(from, cx);
            window.render_frame(cx);
            window.dispatch_event(MouseDownEvent { button: MouseButton::Left, position: from, ..Default::default() }.to_platform_input(), cx);
            window.render_frame(cx);
            owner.update(cx, |view, cx| {
                match change {
                    0 => {
                        device.profiles[0].source_settings.as_mut().unwrap()["profile"]["audioEqualizer"]["presets"]["standard"][0]["current"] = json!([5,5,5,5,5,5,5,5,5,5]);
                        view.set_earbuds_context(&device, true, cx);
                        finish_initial_restores(view, 2660, cx);
                    }
                    1 => view.set_page("TAB_ENHANCEMENT", window, cx),
                    _ => view.set_earbuds_context(&device, false, cx),
                }
            });
            window.render_frame(cx);
            window.dispatch_event(MouseUpEvent { button: MouseButton::Left, position: from, ..Default::default() }.to_platform_input(), cx);
            assert!(owner.read(cx).earbuds.pending.is_none());
            assert_eq!(owner.read(cx).earbuds_equalizer_setting().unwrap().bands, [5; 10]);
            owner.update(cx, |view, cx| view.set_page("TAB_SOUND", window, cx));
        }
    }).unwrap();
}

#[gpui_kit::test]
fn equalizer_same_profile_refreshes_unselected_curves_and_own_save_does_not_restore(
    cx: &mut TestAppContext,
) {
    use razer_device::earbuds_equalizer::{Mode, Preset, Setting};
    let (window, owner, mut device) = mount(2660, cx);
    device.profiles.clear();
    device.active_profile.clear();
    razer_storage::earbuds_profiles::initialize_auto_pause_profile(
        &mut device,
        "eq-profile",
        "TEST-Default",
    )
    .unwrap();
    cx.update_window(window, |_, window, cx| {
        owner.update(cx, |view, cx| {
            view.earbuds.cancel();
            view.earbuds.pending = None;
            view.set_earbuds_context(&device, true, cx);
            let read = view.earbuds.pending.clone().unwrap();
            view.finish_earbuds_request(&read, Ok(reading(2660, false)), cx);
            finish_pending_firmware(view, cx);
            finish_initial_restores(view, 2660, cx);
            let key = view.spec.equalizers[0].key.clone();
            let field = &mut device.profiles[0].source_settings.as_mut().unwrap()["profile"]["audioEqualizer"];
            field["presets"]["standard"][1]["current"] = json!([1,1,1,1,1,1,1,1,1,1]);
            field["presets"]["standard"][1]["default"] = json!([2,2,2,2,2,2,2,2,2,2]);
            view.set_earbuds_context(&device, true, cx);
            assert_eq!(view.draft["equalizers"][&key]["presets"]["movie"], json!([1,1,1,1,1,1,1,1,1,1]));
            assert_eq!(view.draft["equalizers"][&key]["defaults"]["movie"], json!([2,2,2,2,2,2,2,2,2,2]));
            finish_initial_restores(view, 2660, cx);
            view.select_preset(&key, "movie", window, cx);
            let request = view.earbuds.pending.clone().unwrap();
            let setting = Setting { mode: Mode::Standard, preset: Preset::Movie, bands: [1;10] };
            assert_eq!(request.operation(), Operation::Equalizer { setting });
            razer_storage::earbuds_profiles::merge_equalizer(&mut device, "eq-profile", setting).unwrap();
            assert!(view.mark_earbuds_persisted(&request));
            let mut report = reading(2660, false);
            report.operation = request.operation();
            report.connected = None;
            view.finish_earbuds_request(&request, Ok(report), cx);
            view.set_earbuds_context(&device, true, cx);
            assert!(view.earbuds.pending.is_none(), "Own save must not repeat hardware restore");
            assert!(view.earbuds.restore_sequence.is_empty());
            let selected = view.draft["equalizers"][&key].clone();
            view.earbuds.controls.as_mut().unwrap().earbud_disconnected = true;
            view.select_preset(&key, "music", window, cx);
            view.edit(&format!("/equalizers/{key}/bands/0"), json!(-6), window, cx);
            view.reset_equalizer(&key, window, cx);
            assert!(view.earbuds.pending.is_none());
            assert_eq!(view.draft["equalizers"][&key], selected);
            view.earbuds.controls.as_mut().unwrap().earbud_disconnected = false;
            device.profiles[0].source_settings.as_mut().unwrap()["profile"].as_object_mut().unwrap().remove("audioEqualizer");
            view.set_earbuds_context(&device, true, cx);
            finish_initial_restores(view, 2660, cx);
            assert!(view.earbuds_equalizer_disabled());
            assert!(view.snapshot()["profile"].get("audioEqualizer").is_none());
            view.select_preset(&key, "music", window, cx);
            assert!(view.earbuds.pending.is_none());
        });
    }).unwrap();
}

#[gpui_kit::test]
fn equalizer_profile_restore_and_preset_click_reach_the_device_queue(cx: &mut TestAppContext) {
    use razer_device::earbuds_equalizer::Preset;
    for pid in [2657, 2660, 2664, 2668, 2689] {
        let (window, owner, mut device) = mount(pid, cx);
        device.profiles.clear();
        device.active_profile.clear();
        razer_storage::earbuds_profiles::initialize_auto_pause_profile(
            &mut device,
            "eq-profile",
            "TEST-Default",
        )
        .unwrap();
        cx.update_window(window, |_,window,cx| {
            owner.update(cx, |view,cx| {
                view.earbuds.cancel();
                view.earbuds.pending=None;
                view.set_earbuds_context(&device,true,cx);
                let read=view.earbuds.pending.clone().unwrap();
                view.finish_earbuds_request(&read,Ok(reading(pid,false)),cx);
                finish_pending_firmware(view,cx);
                if pid==2657 { finish_anc_restore(view,pid,cx); }
                let eq=view.earbuds.pending.clone().unwrap();
                assert!(matches!(eq.operation(),Operation::Equalizer {setting} if setting.preset==Preset::Game && setting.bands==[2,2,3,4,1,-1,2,3,3,3]));
                assert!(!eq.needs_persistence());
                finish_initial_restores(view,pid,cx);
                view.set_page("TAB_SOUND",window,cx);
            });
            window.render_frame(cx);
            let key=owner.read(cx).spec.equalizers[0].key.clone();
            window.click("earbuds-eq-preset-music",cx);
            owner.update(cx,|view,cx| {
                let request=view.earbuds.pending.clone().unwrap();
                assert!(matches!(request.operation(),Operation::Equalizer {setting} if setting.preset==Preset::Music && setting.bands==[2,2,-1,-1,-1,-1,1,3,3,3]));
                assert!(request.needs_persistence());
                assert!(view.mark_earbuds_persisted(&request));
                view.finish_earbuds_request(&request,Err("EQ SET failed".into()),cx);
                assert!(matches!(&view.earbuds.eq_result,Some(Err(error)) if error=="EQ SET failed"));
                assert_eq!(view.snapshot()["profile"]["audioEqualizer"]["presets"]["selectedPreset"]["standard"], "music");
                view.edit(&format!("/equalizers/{key}/bands/0"), json!(-6.0), window, cx);
                assert!(view.earbuds.pending.is_none());
                view.request_earbuds_equalizer(&key,cx);
                let request=view.earbuds.pending.clone().unwrap();
                assert!(matches!(request.operation(), Operation::Equalizer {setting} if setting.preset==Preset::Custom && setting.bands[0]==-6));
                view.finish_earbuds_request(&request,Err("not saved".into()),cx);
                assert_eq!(view.earbuds.eq_persisted.unwrap().preset,Preset::Music);
            });
        }).unwrap();
    }
}

#[gpui_kit::test]
fn input_source_buttons_open_and_confirm_once_without_saving(cx: &mut TestAppContext) {
    for pid in [2657, 2660, 2664, 2668, 2689] {
        let (window, owner, _) = mount(pid, cx);
        cx.update_window(window, |_, window, cx| {
            owner.update(cx, |view, cx| view.set_page("TAB_SOUND", window, cx));
            window.render_frame(cx);
            window.click("earbuds-input-open", cx);
            assert!(!owner.read(cx).earbuds.input_popup);
            owner.update(cx, |view, cx| {
                let read = view.earbuds.pending.clone().unwrap();
                view.finish_earbuds_request(&read, Ok(reading(pid, false)), cx);
                finish_pending_firmware(view, cx);
                finish_initial_restores(view, pid, cx);
                view.set_page("TAB_SOUND", window, cx);
            });
            window.render_frame(cx);
            assert!(window.try_find("earbuds-input-confirmation").is_none());
            window.click("earbuds-input-open", cx);
            window.render_frame(cx);
            assert!(window.try_find("earbuds-input-confirmation").is_some());
            assert_eq!(window.find("earbuds-input-open").focused(), Some(true));
            window.press("space", cx);
            assert!(owner.read(cx).earbuds.input_popup);
            assert!(owner.read(cx).earbuds.pending.is_none());
            assert_eq!(
                window
                    .find("earbuds-input-confirmation")
                    .bounds()
                    .size
                    .width,
                px(360.)
            );
            // The source is an inline confirmation: interacting inside it
            // neither dismisses it nor dispatches the device command.
            window.click("earbuds-input-confirmation", cx);
            assert!(owner.read(cx).earbuds.input_popup);
            assert!(owner.read(cx).earbuds.pending.is_none());
            window.right_click("earbuds-input-description", cx);
            assert!(owner.read(cx).earbuds.input_popup);
            window.click("earbuds-input-description", cx);
            assert!(!owner.read(cx).earbuds.input_popup);
            assert!(owner.read(cx).earbuds.pending.is_none());
            window.render_frame(cx);
            window.click("earbuds-input-open", cx);
            assert!(owner.read(cx).earbuds.pending.is_none());
            owner.update(cx, |view, cx| view.set_page("TAB_ENHANCEMENT", window, cx));
            assert!(!owner.read(cx).earbuds.input_popup);
            assert!(owner.read(cx).earbuds.pending.is_none());
            owner.update(cx, |view, cx| view.set_page("TAB_SOUND", window, cx));
            window.render_frame(cx);
            window.click("earbuds-input-open", cx);
            window.render_frame(cx);
            window.click("earbuds-input-confirm", cx);
            window.render_frame(cx);
            assert!(window.try_find("earbuds-input-confirmation").is_none());
            let view = owner.read(cx);
            let request = view.earbuds.pending.as_ref().unwrap();
            assert_eq!(request.operation(), Operation::SwitchInputSource);
            assert!(!request.needs_persistence());
            assert!(view.earbuds.edits.is_empty());
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn input_source_confirmation_queues_an_unsaved_command_and_preserves_failure(
    cx: &mut TestAppContext,
) {
    let (window, owner, _) = mount(2660, cx);
    cx.update_window(window, |_, window, cx| {
        owner.update(cx, |view, cx| {
            let read = view.earbuds.pending.clone().unwrap();
            view.finish_earbuds_request(&read, Ok(reading(2660, false)), cx);
            finish_pending_firmware(view, cx);
            finish_initial_restores(view, 2660, cx);
            view.set_page("TAB_SOUND", window, cx);
            view.confirm_earbuds_input(cx);
            assert!(view.earbuds.pending.is_none());
            view.earbuds.input_popup = true;
            view.confirm_earbuds_input(cx);
            assert!(!view.earbuds.input_popup);
            let request = view.earbuds.pending.clone().unwrap();
            assert_eq!(request.operation(), Operation::SwitchInputSource);
            assert!(!request.needs_persistence());
            view.finish_earbuds_request(&request, Err("transport failed".into()), cx);
            assert!(matches!(&view.earbuds.input_result, Some(Err(error)) if error == "transport failed"));
            view.earbuds.input_popup = true;
            view.confirm_earbuds_input(cx);
            let old = view.earbuds.pending.clone().unwrap();
            view.earbuds.cancel();
            view.finish_earbuds_request(&old, Err("late".into()), cx);
            assert!(view.earbuds.input_result.is_none());
        });
    }).unwrap();
}

fn voice_restored(pid: u32, stored: Option<bool>, observed: bool, failed: bool) -> Report {
    serde_json::from_value(json!({"product_id":pid,"operation":{"RestoreVoicePrompt":{"enabled":stored}},
        "connected":null,"left_battery":null,"right_battery":null,"observation":"Unobserved",
        "write":if failed {json!({"Failed":"SET failed"})}else{json!("Skipped")},"voice_prompt":{"Observed":observed}})).unwrap()
}

#[gpui_kit::test]
fn dnd_switch_keeps_its_own_draft_and_does_not_discard_a_queued_auto_pause_edit(
    cx: &mut TestAppContext,
) {
    let (window, owner, _) = mount(2660, cx);
    cx.update_window(window, |_, window, cx| {
        owner.update(cx, |view, cx| {
            let read = view.earbuds.pending.clone().unwrap();
            view.finish_earbuds_request(&read, Ok(reading(2660, false)), cx);
            finish_pending_firmware(view, cx);
            finish_initial_restores(view, view.spec.product_id, cx);
        });
        window.render_frame(cx);
        window.click("earbuds-do-not-disturb", cx);
        window.render_frame(cx);
        window.click("earbuds-auto-pause", cx);
        window.render_frame(cx);
        window.click("earbuds-do-not-disturb", cx);
        owner.update(cx, |view, cx| {
            let first = view.earbuds.pending.clone().unwrap();
            assert_eq!(first.operation(), Operation::DoNotDisturb { enabled: true });
            assert_eq!(view.earbuds.dnd_persisted, None);
            assert_eq!(view.earbuds.dnd_device, DndObservation::Unobserved);
            assert!(!view.earbuds.dnd_draft);
            assert_eq!(
                view.earbuds
                    .edits
                    .iter()
                    .map(|edit| edit.operation)
                    .collect::<Vec<_>>(),
                [
                    Operation::Write { enabled: true },
                    Operation::DoNotDisturb { enabled: false }
                ]
            );
            assert!(view.mark_earbuds_persisted(&first));
            view.finish_earbuds_request(&first, Ok(dnd_written(2660, true)), cx);
            assert!(!view.earbuds.dnd_draft);
            assert_eq!(view.earbuds.dnd_persisted, Some(true));
            assert_eq!(
                view.earbuds.pending.as_ref().unwrap().operation(),
                Operation::Write { enabled: true }
            );
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn dnd_restore_precedes_auto_pause_for_initial_and_each_actual_reconnect_pass(
    cx: &mut TestAppContext,
) {
    use razer_device::earbuds_events::Notification;
    for pid in [2657, 2689] {
        let (window, owner, _) = mount_with_dnd(pid, Some(true), cx);
        cx.update_window(window, |_, _, cx| {
            owner.update(cx, |view, cx| {
                let read = view.earbuds.pending.clone().unwrap();
                let monitor = read.monitor_guard();
                view.finish_earbuds_request(&read, Ok(reading(pid, false)), cx);
                finish_pending_firmware(view, cx);
                let mut configured_auto=false;
                for phase in 0..=1 {
                    let passes = if phase == 0 || pid == 2689 { 1 } else { 2 };
                    for pass in 0..passes {
                        finish_anc_restore(view,pid,cx);
                        let dnd = view.earbuds.pending.clone().unwrap();
                        assert_eq!(dnd.operation(), Operation::DoNotDisturb { enabled: true });
                        let newer_edit = phase == 0 && pid == 2657 && pass == 0;
                        if newer_edit { view.toggle_earbuds_auto_pause(cx); configured_auto=true; }
                        view.mark_earbuds_persisted(&dnd);
                        view.finish_earbuds_request(&dnd, Ok(dnd_written(pid, true)), cx);
                        if newer_edit {
                            let edited = view.earbuds.pending.clone().unwrap();
                            assert_eq!(edited.operation(), Operation::Write { enabled: true });
                            view.mark_earbuds_persisted(&edited);
                            let report = serde_json::from_value(json!({"product_id":pid,"operation":{"Write":{"enabled":true}},
                                "connected":null,"left_battery":null,"right_battery":null,"observation":{"Observed":true},"write":"Confirmed"})).unwrap();
                            view.finish_earbuds_request(&edited, Ok(report), cx);
                        }
                        finish_gesture_restores(view, pid, cx);
                        let voice=view.earbuds.pending.clone().unwrap();
                        assert_eq!(voice.operation(),Operation::RestoreVoicePrompt{enabled:None});
                        assert!(voice.is_profile_restore());assert!(!voice.needs_persistence());
                        view.finish_earbuds_request(&voice,Ok(voice_restored(pid,None,true,false)),cx);
                        let auto = view.earbuds.pending.clone().unwrap();
                        assert_eq!(
                            auto.operation(),
                            Operation::Restore {
                                enabled: Some(configured_auto)
                            }
                        );
                        assert!(!auto.needs_persistence());
                        assert!(!view.mark_earbuds_persisted(&auto));
                        view.finish_earbuds_request(&auto, Ok(auto_restored(pid,configured_auto)), cx);
                        finish_pending_firmware(view, cx);
                    }
                    if phase == 0 {
                        assert!(view.earbuds.pending.is_none());
                        view.observe_earbuds_notification(
                            &monitor,
                            Notification::Connection {
                                status: 0,
                                status_bt: 0,
                            },
                            cx,
                        );
                        view.observe_earbuds_notification(
                            &monitor,
                            Notification::Connection {
                                status: 3,
                                status_bt: 0,
                            },
                            cx,
                        );
                        let gate=view.earbuds.pending.clone().unwrap();assert_eq!(gate.operation(),Operation::Read);
                        view.finish_earbuds_request(&gate,Ok(reading(pid,false)),cx);
                        finish_pending_firmware(view,cx);
                    }
                }
                assert_eq!(
                    view.earbuds.pending.as_ref().unwrap().operation(),
                    Operation::Battery { levels: None }
                );
            });
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn mounted_switch_separates_local_save_from_optimistic_edit_and_reports_failure(
    cx: &mut TestAppContext,
) {
    let (window, owner, _) = mount(2660, cx);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("earbuds-auto-pause").disabled(), Some(true));
        owner.update(cx, |view, cx| {
            let request = view.earbuds.pending.clone().unwrap();
            view.finish_earbuds_request(&request, Ok(reading(2660, false)), cx);
            finish_pending_firmware(view, cx);
            finish_initial_restores(view, view.spec.product_id, cx);
        });
        window.render_frame(cx);
        assert_ne!(window.find("earbuds-auto-pause").disabled(), Some(true));
        window.click("earbuds-auto-pause", cx);
        owner.update(cx, |view, cx| {
            let request = view.earbuds.pending.clone().unwrap();
            assert_eq!(request.operation(), Operation::Write { enabled: true });
            let state = view.earbuds.controls.as_ref().unwrap();
            assert!(state.draft.auto_pause);
            assert_eq!(state.auto_pause_persisted, Some(false));
            assert_eq!(
                state.auto_pause_device,
                AutoPauseObservation::Observed(false)
            );
            assert!(view.mark_earbuds_persisted(&request));
            assert_eq!(
                view.earbuds.controls.as_ref().unwrap().auto_pause_persisted,
                Some(true)
            );
            view.finish_earbuds_request(&request, Err("device rejected SET".into()), cx);
            assert!(view.earbuds.controls.as_ref().unwrap().draft.auto_pause);
            assert_eq!(view.earbuds.error.as_deref(), Some("device rejected SET"));
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn all_products_reject_previous_connection_and_failed_reads_without_restore(
    cx: &mut TestAppContext,
) {
    for pid in razer_device::earbuds_protocol40::AUTO_PAUSE_PRODUCTS {
        let (window, owner, mut device) = mount(pid, cx);
        cx.update_window(window, |_, _, cx| {
            owner.update(cx, |view, cx| {
                let old = view.earbuds.pending.clone().unwrap();
                device.dashboard.connection_observation = None;
                view.set_earbuds_context(&device, true, cx);
                assert!(!old.guard().load(Ordering::Acquire));
                device.dashboard.connection_observation =
                    Some(razer_model::model::DeviceConnectionObservation::UsbPresent);
                view.set_earbuds_context(&device, true, cx);
                view.finish_earbuds_request(&old, Ok(reading(pid, true)), cx);
                finish_pending_firmware(view, cx);
                assert_eq!(
                    view.earbuds.controls.as_ref().unwrap().auto_pause_device,
                    AutoPauseObservation::Unobserved
                );
                let current = view.earbuds.pending.clone().unwrap();
                assert_ne!(old.id, current.id);
                view.finish_earbuds_request(&current, Err("connection read failed".into()), cx);
                assert!(view.earbuds.pending.is_none());
                assert!(!view.earbuds.initialized);
                assert!(matches!(
                    view.earbuds.controls.as_ref().unwrap().auto_pause_device,
                    AutoPauseObservation::Unobserved
                ));
                assert_eq!(
                    view.earbuds.error.as_deref(),
                    Some("connection read failed")
                );
                view.set_earbuds_context(&device, false, cx);
                view.set_earbuds_context(&device, true, cx);
                let next = view.earbuds.pending.clone().unwrap();
                let partial = reading(pid, false);
                view.finish_earbuds_request(&next, Ok(partial), cx);
                assert_eq!(view.earbuds.dnd_device, DndObservation::Unobserved);
                finish_pending_firmware(view, cx);
                finish_initial_restores(view, pid, cx);
                assert!(view.snapshot().pointer("/profile/notDisturb").is_none());
                assert_eq!(
                    view.earbuds.controls.as_ref().unwrap().auto_pause_device,
                    AutoPauseObservation::Observed(false)
                );
            });
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn notification_reconnect_preserves_one_versus_two_restore_branch_and_rejects_old_monitor(
    cx: &mut TestAppContext,
) {
    use razer_device::earbuds_events::Notification;
    for pid in [2657, 2689] {
        let (window, owner, _) = mount(pid, cx);
        cx.update_window(window, |_, _, cx| {
            owner.update(cx, |view, cx| {
                let initial = view.earbuds.pending.clone().unwrap();
                view.finish_earbuds_request(&initial, Ok(reading(pid, false)), cx);
                finish_pending_firmware(view, cx);
                finish_initial_restores(view, view.spec.product_id, cx);
                let monitor = initial.monitor_guard();
                view.observe_earbuds_notification(
                    &Arc::new(AtomicBool::new(true)),
                    Notification::Connection {
                        status: 0,
                        status_bt: 0,
                    },
                    cx,
                );
                assert!(!view.earbuds.was_off);
                view.observe_earbuds_notification(
                    &monitor,
                    Notification::Connection {
                        status: 42,
                        status_bt: 0,
                    },
                    cx,
                );
                assert!(!view.earbuds.was_off);
                assert!(view.earbuds.pending.is_none());
                assert!(matches!(
                    view.earbuds.dnd_device,
                    DndObservation::Unobserved
                ));
                assert!(!view.earbuds.initialized);
                view.observe_earbuds_notification(
                    &monitor,
                    Notification::Connection {
                        status: 0,
                        status_bt: 0,
                    },
                    cx,
                );
                assert!(view.earbuds.was_off);
                view.observe_earbuds_notification(
                    &monitor,
                    Notification::Connection {
                        status: 3,
                        status_bt: 0,
                    },
                    cx,
                );
                assert_eq!(
                    view.earbuds.reconnect_restores,
                    if pid == 2689 { 1 } else { 2 }
                );
                let gate = view.earbuds.pending.clone().unwrap();
                assert_eq!(gate.operation(), Operation::Read);
                view.finish_earbuds_request(&gate, Ok(reading(pid, false)), cx);
                finish_pending_firmware(view, cx);
                finish_anc_restore(view, pid, cx);
                assert!(matches!(
                    view.earbuds.pending.as_ref().unwrap().operation(),
                    Operation::RestoreGesture {
                        enabled: Some(true)
                    }
                ));
            });
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn accepted_workspace_save_updates_the_profile_used_by_reconnect(cx: &mut TestAppContext) {
    use crate::features::source_workspace::SourceProductWorkspace;
    cx.update(|cx| gpui_kit::init(cx));
    let mut owner = None;
    let window = cx.open_window(size(px(1600.), px(1000.)), |window, cx| {
        let view = cx.new(|cx| {
            SourceProductWorkspace::new(
                razer_model::product_preview::registered_preview(2657).unwrap(),
                window,
                cx,
            )
        });
        owner = Some(view.clone());
        Root::new(view, window, cx)
    });
    let owner = owner.unwrap();
    cx.update_window(window.into(), |_, window, cx| {
        owner.update(cx, |view, cx| {
            view.observe_connection(
                Some(razer_model::model::DeviceConnectionObservation::UsbPresent),
                cx,
            );
            view.set_active(true, window, cx);
            let page = view.audio_mixer_page().unwrap();
            let first = page.read(cx).earbuds.pending.clone().unwrap();
            assert!(first.initializes_profile());
            assert!(view.device().profiles.is_empty());
            page.update(cx, |page, cx| {
                page.finish_earbuds_request(&first, Err("first read failed".into()), cx)
            });
            assert!(view.device().profiles.is_empty());
            assert!(page.read(cx).earbuds.pending.is_none());
            view.set_active(false, window, cx);
            view.set_active(true, window, cx);
            let read = page.read(cx).earbuds.pending.clone().unwrap();
            page.update(cx, |page, cx| {
                page.finish_earbuds_request(&read, Ok(reading(2657, false)), cx);
                finish_pending_firmware(page, cx);
            });
            let restore = page.read(cx).earbuds.pending.clone().unwrap();
            assert!(restore.initializes_profile());
            assert!(matches!(restore.operation(), Operation::Anc { .. }));
            let id = restore.profile_id().to_owned();
            let mut saved = view.saved_snapshot();
            razer_storage::earbuds_profiles::initialize_auto_pause_profile(
                &mut saved,
                &id,
                "TEST-Default",
            )
            .unwrap();
            view.mark_saved(saved, cx);
            assert!(page.update(cx, |page, _| page.mark_earbuds_persisted(&restore)));
            assert_eq!(
                razer_storage::earbuds_profiles::read_auto_pause(view.device(), &id)
                    .unwrap()
                    .is_enabled,
                Some(true)
            );
            assert_eq!(
                razer_storage::earbuds_profiles::read_auto_pause(&view.saved_snapshot(), &id)
                    .unwrap()
                    .is_enabled,
                Some(true)
            );
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn profile_changes_reuse_the_physical_monitor_and_firmware_stage(cx: &mut TestAppContext) {
    for firmware_pending in [false, true] {
        let (window, owner, mut device) = mount(2660, cx);
        cx.update_window(window, |_, _, cx| {
            owner.update(cx, |view, cx| {
                let gate = view.earbuds.pending.clone().unwrap();
                view.finish_earbuds_request(&gate, Ok(reading(2660, false)), cx);
                let firmware = view.earbuds.pending.clone().unwrap();
                assert_eq!(firmware.operation(), Operation::Firmware);
                if !firmware_pending {
                    finish_pending_firmware(view, cx);
                    finish_initial_restores(view, 2660, cx);
                }
                let monitor = view.earbuds.monitor.clone().unwrap();
                let mut next = device.profiles[0].clone();
                next.id = "next".into();
                next.guid = "next-source".into();
                device.profiles.push(next);
                device.active_profile = "next".into();
                view.set_earbuds_context(&device, true, cx);
                assert!(Arc::ptr_eq(
                    &monitor,
                    view.earbuds.monitor.as_ref().unwrap()
                ));
                assert!(monitor.load(Ordering::Acquire));
                if firmware_pending {
                    assert!(view.earbuds_request_current(&firmware));
                    finish_pending_firmware(view, cx);
                }
                let next = view.earbuds.pending.as_ref().unwrap();
                assert_eq!(next.profile_id(), "next");
                assert!(next.is_profile_restore());
                assert!(matches!(next.operation(), Operation::RestoreGesture { .. }));
                finish_initial_restores(view, 2660, cx);
                assert!(view.earbuds.pending.is_none());
                assert_eq!(
                    view.earbuds
                        .controls
                        .as_ref()
                        .unwrap()
                        .firmware_version
                        .as_deref(),
                    Some("1.5.0.0")
                );
            })
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn explicit_first_profile_edit_retains_user_intent_before_initial_restore(cx: &mut TestAppContext) {
    let (window, owner, mut device) = mount(2660, cx);
    device.profiles.clear();
    device.active_profile.clear();
    cx.update_window(window, |_, window, cx| {
        owner.update(cx, |view, cx| {
            let old = view.earbuds.pending.clone().unwrap();
            view.set_earbuds_context(&device, true, cx);
            view.finish_earbuds_request(&old, Err("scope replaced".into()), cx);
            let gate = view.earbuds.pending.clone().unwrap();
            view.finish_earbuds_request(&gate, Ok(reading(2660, false)), cx);
            assert_eq!(view.earbuds.dnd_device, DndObservation::Unobserved);
        });
        window.render_frame(cx);
        window.click("earbuds-do-not-disturb", cx);
        owner.update(cx, |view, cx| {
            finish_pending_firmware(view, cx);
            let edit = view.earbuds.pending.clone().unwrap();
            assert_eq!(edit.operation(), Operation::DoNotDisturb { enabled: true });
            assert!(
                edit.initializes_profile()
                    && edit.needs_persistence()
                    && !edit.is_profile_restore()
            );
            razer_storage::earbuds_profiles::initialize_auto_pause_profile(
                &mut device,
                edit.profile_id(),
                "TEST-Default",
            )
            .unwrap();
            razer_storage::earbuds_profiles::merge_do_not_disturb(
                &mut device,
                edit.profile_id(),
                true,
            )
            .unwrap();
            assert_eq!(
                razer_storage::earbuds_profiles::read_do_not_disturb(&device, edit.profile_id())
                    .unwrap(),
                Some(true)
            );
            assert!(view.mark_earbuds_persisted(&edit));
            view.finish_earbuds_request(&edit, Ok(dnd_written(2660, true)), cx);
            assert_eq!(view.earbuds.dnd_persisted, Some(true));
        });
    })
    .unwrap();
}
