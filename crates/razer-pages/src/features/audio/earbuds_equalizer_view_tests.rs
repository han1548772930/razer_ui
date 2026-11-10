use super::*;
use razer_device::earbuds_equalizer::{Mode, Preset, Setting};

fn profile(pid: u32, device: &mut razer_model::model::Device) {
    device.profiles.clear();
    device.active_profile.clear();
    razer_storage::earbuds_profiles::initialize_auto_pause_profile(
        device,
        "eq-card",
        "TEST-Default",
    )
    .unwrap();
    let eq = &mut device.profiles[0].source_settings.as_mut().unwrap()["profile"]["audioEqualizer"];
    eq["presets"]["standard"][3]["current"] = serde_json::to_value([1; 10]).unwrap();
    eq["presets"]["standard"][3]["default"] = serde_json::to_value([2; 10]).unwrap();
    assert_eq!(device.product_id, pid);
}

fn initialize(
    view: &mut AudioProductWorkspace,
    device: &razer_model::model::Device,
    window: &mut gpui_kit::Window,
    cx: &mut gpui_kit::Context<AudioProductWorkspace>,
) {
    view.earbuds.cancel();
    view.earbuds.pending = None;
    view.set_earbuds_context(device, true, cx);
    let request = view.earbuds.pending.clone().unwrap();
    view.finish_earbuds_request(&request, Ok(reading(device.product_id, false)), cx);
    finish_pending_firmware(view, cx);
    finish_initial_restores(view, device.product_id, cx);
    view.set_page("TAB_SOUND", window, cx);
}

fn complete_edit(
    view: &mut AudioProductWorkspace,
    cx: &mut gpui_kit::Context<AudioProductWorkspace>,
) -> Setting {
    let request = view.earbuds.pending.clone().unwrap();
    let Operation::Equalizer { setting } = request.operation() else {
        panic!("Expected EQ edit")
    };
    assert!(request.needs_persistence());
    assert!(view.mark_earbuds_persisted(&request));
    use razer_device::earbuds_equalizer::Submission;
    let report = equalizer_reply(view.spec.product_id, setting, Submission::Confirmed, Submission::Confirmed);
    view.finish_earbuds_request(&request, Ok(report), cx);
    setting
}

#[gpui_kit::test]
fn equalizer_partial_ack_and_cleanup_failure_are_visible_without_rolling_back_saved_draft(
    cx: &mut TestAppContext,
) {
    use razer_device::{earbuds_equalizer::Submission, earbuds_protocol40::WriteOutcome};
    for pid in [2657, 2660, 2664, 2668, 2689] {
        let (handle, owner, mut device) = mount(pid, cx);
        profile(pid, &mut device);
        cx.update_window(handle, |_, window, cx| {
            owner.update(cx, |view, cx| initialize(view, &device, window, cx));
            for (button, cleanup) in [("earbuds-eq-preset-music", false), ("earbuds-eq-preset-movie", true)] {
                window.render_frame(cx);
                window.click(button, cx);
                owner.update(cx, |view, cx| {
                    let request = view.earbuds.pending.clone().unwrap();
                    let Operation::Equalizer { setting } = request.operation() else { panic!("EQ edit") };
                    assert!(view.mark_earbuds_persisted(&request));
                    let saved = view.snapshot();
                    let bands = if cleanup { Submission::Confirmed } else {
                        Submission::Failed { message: "band reply missing".into(), acknowledged: false }
                    };
                    let mut report = equalizer_reply(pid, setting, Submission::Confirmed, bands);
                    if cleanup { report.write = WriteOutcome::Failed("EQ cleanup failed".into()); }
                    view.finish_earbuds_request(&request, Ok(report), cx);
                    assert_eq!(view.snapshot(), saved, "failed hardware submission does not change the saved preference");
                    let report = view.earbuds.eq_result.as_ref().unwrap().as_ref().unwrap();
                    let confirmation = report.equalizer.as_ref().unwrap();
                    assert!(confirmation.preset.acknowledged());
                    assert_eq!(confirmation.bands.acknowledged(), cleanup);
                    assert_eq!(report.observation, AutoPauseObservation::Unobserved);
                    assert_eq!(report.connected, None);
                    assert!(view.earbuds_equalizer_error().unwrap().contains(
                        if cleanup { "cleanup failed" } else { "preset was acknowledged" }
                    ));
                });
                window.render_frame(cx);
                assert!(window.find("earbuds-eq-submission-error").visible());
            }
            window.click("earbuds-eq-preset-custom", cx);
            owner.update(cx, |view, cx| { complete_edit(view, cx); });
            window.render_frame(cx);
            assert!(window.try_find("earbuds-eq-submission-error").is_none());
        }).unwrap();
    }
}

#[gpui_kit::test]
fn equalizer_malformed_or_old_profile_completion_cannot_replace_current_submission(
    cx: &mut TestAppContext,
) {
    use razer_device::earbuds_equalizer::Submission;
    for pid in [2657, 2660, 2664, 2668, 2689] {
        let (handle, owner, mut device) = mount(pid, cx);
        profile(pid, &mut device);
        cx.update_window(handle, |_, window, cx| {
            owner.update(cx, |view, cx| initialize(view, &device, window, cx));
            window.render_frame(cx);
            window.click("earbuds-eq-preset-music", cx);
            owner.update(cx, |view, cx| {
                let request = view.earbuds.pending.clone().unwrap();
                let mut incomplete = reading(pid, false);
                incomplete.operation = request.operation();
                view.finish_earbuds_request(&request, Ok(incomplete), cx);
                assert!(view.earbuds_equalizer_error().unwrap().contains("omitted"));
            });
            window.render_frame(cx);
            window.click("earbuds-eq-preset-movie", cx);
            owner.update(cx, |view, cx| {
                let old = view.earbuds.pending.clone().unwrap();
                let Operation::Equalizer { setting } = old.operation() else { panic!("EQ edit") };
                let mut next = device.profiles[0].clone();
                next.id = "next-eq-profile".into();
                next.guid = "next-eq-profile".into();
                device.profiles.push(next);
                device.active_profile = "next-eq-profile".into();
                view.set_earbuds_context(&device, true, cx);
                assert!(!old.guard().load(Ordering::Acquire));
                view.finish_earbuds_request(&old, Ok(equalizer_reply(pid, setting,
                    Submission::Confirmed, Submission::Failed {
                        message: "old profile bands failed".into(), acknowledged: false,
                    })), cx);
                assert!(view.earbuds.eq_result.is_none());
                assert_eq!(view.earbuds.profile_id, "next-eq-profile");
                finish_initial_restores(view, pid, cx);
                assert!(view.earbuds_equalizer_error().is_none());
            });
            window.render_frame(cx);
            assert!(window.try_find("earbuds-eq-submission-error").is_none());
        }).unwrap();
    }
}

#[gpui_kit::test]
fn presets_reset_and_required_badge_follow_profile_values(cx: &mut TestAppContext) {
    for pid in [2657, 2660, 2664, 2668, 2689] {
        let (handle, owner, mut device) = mount(pid, cx);
        profile(pid, &mut device);
        cx.update_window(handle, |_, window, cx| {
            owner.update(cx, |view, cx| initialize(view, &device, window, cx));
            window.render_frame(cx);
            assert!(window.try_find("earbuds-eq-reset").is_none());
            assert!(window.try_find("earbuds-eq-requires").is_none());
            let game = window.find("earbuds-eq-preset-game").bounds();
            let movie = window.find("earbuds-eq-preset-movie").bounds();
            let music = window.find("earbuds-eq-preset-music").bounds();
            assert_eq!(game.size.width, window.rem_size() * (252. / 16.));
            assert_eq!(movie.left() - game.right(), window.rem_size() * (10. / 16.));
            assert!(movie.left() < music.left(), "Use profile ordering");

            window.click("earbuds-eq-preset-custom", cx);
            window.render_frame(cx);
            owner.update(cx, |view, cx| {
                assert_eq!(
                    complete_edit(view, cx),
                    Setting {
                        mode: Mode::Standard,
                        preset: Preset::Custom,
                        bands: [1; 10]
                    }
                );
            });
            assert_ne!(window.find("earbuds-eq-reset").disabled(), Some(true));
            assert!(window.find("earbuds-eq-requires").visible());
            window.press("space", cx);
            owner.update(cx, |view, cx| {
                assert_eq!(complete_edit(view, cx).preset, Preset::Custom);
            });
            window.click("earbuds-eq-reset", cx);
            owner.update(cx, |view, cx| {
                assert_eq!(
                    complete_edit(view, cx),
                    Setting {
                        mode: Mode::Standard,
                        preset: Preset::Custom,
                        bands: [2; 10]
                    }
                );
            });
            window.render_frame(cx);
            assert_eq!(window.find("earbuds-eq-reset").disabled(), Some(true));
            assert!(window.try_find("earbuds-eq-requires").is_none());
            window.click("earbuds-eq-reset", cx);
            assert!(owner.read(cx).earbuds.pending.is_none());

            window.click("earbuds-eq-preset-movie", cx);
            owner.update(cx, |view, cx| {
                complete_edit(view, cx);
            });
            window.render_frame(cx);
            assert!(window.try_find("earbuds-eq-reset").is_none());
            assert!(window.try_find("earbuds-eq-requires").is_none());
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn help_and_disabled_cursor_tip_dismiss_on_page_and_connection_changes(cx: &mut TestAppContext) {
    for pid in [2657, 2660, 2664, 2668, 2689] {
        let (handle, owner, mut device) = mount(pid, cx);
        profile(pid, &mut device);
        cx.update_window(handle, |_, window, cx| {
            owner.update(cx, |view, cx| initialize(view, &device, window, cx));
            window.render_frame(cx);
            window.hover("earbuds-eq-help", cx);
            window.render_frame(cx);
            assert_eq!(
                window.find("earbuds-eq-help").label(),
                Some(
                    razer_i18n::t(if pid == 2664 {
                        "EQUALIZER_TOOLTIP_1"
                    } else {
                        "EQUALIZER_TOOLTIP_2"
                    })
                    .as_str()
                )
            );
            assert!(window.find("earbuds-eq-help-tip").visible());
            let help = window.find("earbuds-eq-help").bounds();
            let tip = window.find("earbuds-eq-help-tip").bounds();
            assert_eq!(tip.top() - help.top(), window.rem_size() * (20. / 16.));
            assert_eq!(help.right() - tip.right(), window.rem_size() * (5. / 16.));
            owner.update(cx, |view, cx| view.set_page("TAB_ENHANCEMENT", window, cx));
            window.render_frame(cx);
            assert!(window.try_find("earbuds-eq-help-tip").is_none());
            owner.update(cx, |view, cx| {
                view.set_page("TAB_SOUND", window, cx);
                view.earbuds.controls.as_mut().unwrap().earbud_disconnected = true;
            });
            window.render_frame(cx);
            assert!(!window.find("earbuds-eq-help-tip").visible());
            let before = owner.read(cx).earbuds_equalizer_setting().unwrap();
            window.click("earbuds-eq-preset-custom", cx);
            window.hover("earbuds-eq-disabled", cx);
            window.render_frame(cx);
            assert_eq!(owner.read(cx).earbuds_equalizer_setting().unwrap(), before);
            assert!(owner.read(cx).earbuds.pending.is_none());
            assert!(window.find("earbuds-eq-disabled-tip").visible());
            owner.update(cx, |view, cx| view.set_earbuds_context(&device, false, cx));
            window.render_frame(cx);
            assert!(window.try_find("earbuds-eq-disabled-tip").is_none());
        })
        .unwrap();
    }
}
