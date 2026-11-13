use super::{
    AudioProductWorkspace, Operation, Report, finish_initial_restores, finish_pending_firmware,
    mount, reading,
};
use crate::features::EarbudsAutoPauseRequested;
use gpui_kit::{AppContext, Subscription, TestAppContext, px, size, test::TestWindowExt};
use std::{cell::RefCell, rc::Rc, sync::atomic::Ordering};

fn capture(
    owner: &gpui_kit::Entity<AudioProductWorkspace>,
    cx: &mut TestAppContext,
) -> (Rc<RefCell<Vec<EarbudsAutoPauseRequested>>>, Subscription) {
    let requests = Rc::new(RefCell::new(Vec::new()));
    let received = requests.clone();
    let subscription = cx.update(|cx| {
        cx.subscribe(owner, move |_, request: &EarbudsAutoPauseRequested, _| {
            if request.is_device_action() {
                received.borrow_mut().push(request.clone());
            }
        })
    });
    (requests, subscription)
}

fn code(pid: u32, value: &str) -> Report {
    serde_json::from_value(serde_json::json!({"product_id":pid,"operation":"ActivationCode",
        "connected":null,"left_battery":null,"right_battery":null,"observation":"Unobserved","write":"Skipped",
        "activation_code":{"declared_length":value.len(),"value":value}})).unwrap()
}

fn missing_profile(
    view: &mut AudioProductWorkspace,
    device: &mut razer_model::model::Device,
    window: &mut gpui_kit::Window,
    cx: &mut gpui_kit::Context<AudioProductWorkspace>,
) {
    device.profiles.clear();
    device.active_profile = "missing-profile".into();
    view.set_earbuds_context(device, true, cx);
    view.set_earbuds_power_context(device, true, 1, cx);
    view.set_earbuds_sound_context(device, true, 1, cx);
    view.observe_earbuds_shared_connection(&reading(device.product_id, false), cx);
    view.set_page("TAB_SOUND", window, cx);
    assert!(!view.earbuds.active);
}

#[gpui_kit::test]
fn earbuds_sound_global_actions_ignore_missing_profiles_and_serialize_through_completion(
    cx: &mut TestAppContext,
) {
    for pid in [2657, 2660, 2664, 2668, 2689] {
        let (handle, owner, mut device) = mount(pid, cx);
        cx.simulate_window_resize(handle, size(px(1600.), px(1400.)));
        let (requests, _subscription) = capture(&owner, cx);
        let before = cx
            .update_window(handle, |_, window, cx| {
                owner.update(cx, |view, cx| {
                    missing_profile(view, &mut device, window, cx)
                });
                window.render_frame(cx);
                let before = owner.read(cx).snapshot();
                window.click("earbuds-input-open", cx);
                assert!(owner.read(cx).earbuds_sound.input_popup);
                window.click("earbuds-input-confirm", cx);
                before
            })
            .unwrap();
        let first = requests.borrow()[0].clone();
        assert_eq!(first.operation(), Operation::SwitchInputSource);
        assert!(!first.needs_persistence() && !first.initializes_profile());
        cx.update_window(handle, |_, window, cx| {
            window.click("earbuds-activation-show", cx);
            assert!(owner.read(cx).earbuds_activation.loading);
        })
        .unwrap();
        assert_eq!(
            requests.borrow().len(),
            1,
            "later read waits for worker completion"
        );
        cx.update(|cx| {
            owner.update(cx, |view, cx| {
                view.finish_earbuds_request(&first, Err("switch failed".into()), cx)
            })
        });
        let second = requests.borrow()[1].clone();
        assert_eq!(second.operation(), Operation::ActivationCode);
        assert!(!second.needs_persistence());
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.find("earbuds-input-error").visible());
            cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string("UNCHANGED".into()));
            window.click("earbuds-activation-copy", cx);
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some("UNCHANGED".into())
            );
            owner.update(cx, |view, cx| {
                view.finish_earbuds_request(&second, Ok(code(pid, "LOCAL-123")), cx)
            });
            window.render_frame(cx);
            window.click("earbuds-activation-copy", cx);
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some("LOCAL-123".into())
            );
            assert_eq!(owner.read(cx).snapshot(), before);
            assert!(device.profiles.is_empty());
            assert_eq!(device.active_profile, "missing-profile");
        })
        .unwrap();
        assert_eq!(requests.borrow().len(), 2);
    }
}

#[gpui_kit::test]
fn earbuds_sound_modal_reopen_and_reconnect_wait_for_old_cleanup_and_reject_old_values(
    cx: &mut TestAppContext,
) {
    for pid in [2657, 2660, 2664, 2668, 2689] {
        let (handle, owner, mut device) = mount(pid, cx);
        cx.simulate_window_resize(handle, size(px(1600.), px(1400.)));
        let (requests, _subscription) = capture(&owner, cx);
        cx.update_window(handle, |_, window, cx| {
            owner.update(cx, |view, cx| {
                missing_profile(view, &mut device, window, cx)
            });
            window.render_frame(cx);
            window.click("earbuds-activation-show", cx);
        })
        .unwrap();
        let first = requests.borrow()[0].clone();
        cx.update_window(handle, |_, window, cx| {
            window.click("earbuds-activation-close", cx);
            window.click("earbuds-activation-show", cx);
        })
        .unwrap();
        assert!(!first.guard().load(Ordering::Acquire));
        assert_eq!(requests.borrow().len(), 1);
        cx.update(|cx| {
            owner.update(cx, |view, cx| {
                view.finish_earbuds_request(&first, Ok(code(pid, "OLD")), cx);
                assert!(view.earbuds_activation.code.is_none());
            })
        });
        let second = requests.borrow()[1].clone();
        cx.update_window(handle, |_, window, cx| {
            owner.update(cx, |view, cx| {
                view.finish_earbuds_request(&second, Err("code read failed".into()), cx)
            });
            window.render_frame(cx);
            assert!(window.find("earbuds-activation-error").visible());
            cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string("UNCHANGED".into()));
            window.click("earbuds-activation-copy", cx);
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some("UNCHANGED".into())
            );
            window.click("earbuds-activation-close", cx);
            window.click("earbuds-activation-show", cx);
        })
        .unwrap();
        let third = requests.borrow()[2].clone();
        cx.update_window(handle, |_, window, cx| {
            owner.update(cx, |view, cx| {
                view.set_earbuds_power_context(&device, true, 2, cx);
                view.set_earbuds_sound_context(&device, true, 2, cx);
                view.observe_earbuds_shared_connection(&reading(pid, false), cx);
            });
            window.render_frame(cx);
            window.click("earbuds-input-open", cx);
            window.click("earbuds-input-confirm", cx);
        })
        .unwrap();
        assert!(!third.guard().load(Ordering::Acquire));
        assert_eq!(requests.borrow().len(), 3);
        cx.update(|cx| {
            owner.update(cx, |view, cx| {
                view.finish_earbuds_request(&third, Ok(code(pid, "STALE")), cx);
                assert!(view.earbuds_activation.code.is_none());
                assert!(view.earbuds_sound.input_result.is_none());
            })
        });
        let next = requests.borrow()[3].clone();
        assert_eq!(next.operation(), Operation::SwitchInputSource);
        assert_ne!(next.device_scope(), third.device_scope());
    }
}

#[gpui_kit::test]
fn earbuds_sound_old_profile_read_cannot_replace_new_shared_radio_observation(
    cx: &mut TestAppContext,
) {
    let (handle, owner, device) = mount(2657, cx);
    cx.update_window(handle, |_, _, cx| {
        owner.update(cx, |view, cx| {
            let old = view.earbuds.pending.clone().unwrap();
            view.set_earbuds_context(&device, false, cx);
            view.set_earbuds_context(&device, true, cx);
            view.set_earbuds_power_context(&device, true, 2, cx);
            view.set_earbuds_sound_context(&device, true, 2, cx);
            view.observe_earbuds_shared_connection(&reading(2657, false), cx);
            let mut old_report = reading(2657, false);
            old_report.connected = Some(false);
            view.finish_earbuds_request(&old, Ok(old_report), cx);
            assert!(!old.guard().load(Ordering::Acquire));
            assert!(view.earbuds_connection.ready());
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn earbuds_sound_current_radio_reads_update_and_successful_nonradio_reports_preserve_it(
    cx: &mut TestAppContext,
) {
    use razer_device::earbuds_equalizer::{Mode, Preset, Setting, Submission};
    let (handle, owner, _) = mount(2660, cx);
    cx.update_window(handle,|_,_,cx| {
        owner.update(cx,|view,cx| {
            let initial=view.earbuds.pending.clone().unwrap();
            view.finish_earbuds_request(&initial,Ok(reading(2660,false)),cx);
            assert!(view.earbuds_connection.ready());
            finish_pending_firmware(view,cx);
            finish_initial_restores(view,2660,cx);
            view.earbuds.queued=Some(Operation::Battery { levels:Some([255,90]) });
            view.start_earbuds_request(cx);
            let battery=view.earbuds.pending.clone().unwrap();
            assert!(matches!(battery.operation(),Operation::Battery { .. }));
            let mut observation=reading(2660,false);
            observation.operation=battery.operation();
            observation.left_battery=Some(255);
            view.finish_earbuds_request(&battery,Ok(observation),cx);
            assert!(view.earbuds_connection.disconnected());
            let eq=Report::from_equalizer(2660,razer_device::earbuds_equalizer::Report {
                setting:Setting { mode:Mode::Standard,preset:Preset::Music,bands:[1;10] },
                preset:Submission::Confirmed,bands:Submission::Confirmed,
            }).unwrap();
            view.observe_earbuds_shared_connection(&eq,cx);
            assert!(view.earbuds_connection.disconnected());
            let mic:Report=serde_json::from_value(serde_json::json!({"product_id":2660,
                "operation":"ReadMicrophoneMute","connected":null,"left_battery":null,"right_battery":null,
                "observation":"Unobserved","write":"Skipped",
                "microphone_mute":{"observation":{"Observed":false},"write":"Skipped"}})).unwrap();
            view.observe_earbuds_shared_connection(&mic,cx);
            assert!(view.earbuds_connection.disconnected());
            view.observe_earbuds_shared_connection(&reading(2660,false),cx);
            view.observe_earbuds_shared_connection(&eq,cx);
            view.observe_earbuds_shared_connection(&mic,cx);
            assert!(view.earbuds_connection.ready());
        });
    }).unwrap();
}

#[gpui_kit::test]
fn earbuds_sound_input_survives_profile_and_tab_changes_without_synthetic_radio_state(
    cx: &mut TestAppContext,
) {
    use razer_device::earbuds_protocol40::WriteOutcome;
    for pid in [2657, 2660, 2664, 2668, 2689] {
        let (handle, owner, mut device) = mount(pid, cx);
        cx.simulate_window_resize(handle, size(px(1600.), px(1400.)));
        cx.update_window(handle, |_, window, cx| {
            owner.update(cx, |view, cx| {
                missing_profile(view, &mut device, window, cx)
            });
            window.render_frame(cx);
            window.click("earbuds-input-open", cx);
            window.click("earbuds-input-confirm", cx);
            let request = owner.read(cx).earbuds_sound.pending.clone().unwrap();
            owner.update(cx, |view, cx| {
                device.active_profile = "another-missing-profile".into();
                view.set_earbuds_context(&device, true, cx);
                view.set_earbuds_sound_context(&device, true, 1, cx);
                view.set_page("TAB_MIC", window, cx);
                assert!(view.earbuds_request_current(&request));
                let mut radio = reading(pid, false);
                radio.left_battery = Some(255);
                view.observe_earbuds_shared_connection(&radio, cx);
                assert!(view.earbuds_request_current(&request));
                let mut response = reading(pid, false);
                response.operation = Operation::SwitchInputSource;
                response.connected = None;
                response.left_battery = None;
                response.right_battery = None;
                response.write = WriteOutcome::Confirmed;
                view.finish_earbuds_request(&request, Ok(response), cx);
                assert_eq!(
                    view.earbuds_sound.input_result,
                    Some(Ok(WriteOutcome::Confirmed))
                );
                assert!(view.earbuds_connection.disconnected());
                view.set_page("TAB_SOUND", window, cx);
            });
            window.render_frame(cx);
            window.hover("earbuds-input-disabled", cx);
            assert!(owner.read(cx).earbuds_sound.input_disabled_cursor.is_some());
            owner.update(cx, |view, cx| {
                view.observe_earbuds_shared_connection(&reading(pid, false), cx);
                assert!(view.earbuds_sound.input_disabled_cursor.is_none());
                let mut radio = reading(pid, false);
                radio.left_battery = Some(255);
                view.observe_earbuds_shared_connection(&radio, cx);
                assert!(view.earbuds_sound.input_disabled_cursor.is_none());
            });
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn earbuds_sound_radio_changes_preserve_submitted_device_actions(cx: &mut TestAppContext) {
    use razer_device::{earbuds_events::Notification, earbuds_protocol40::WriteOutcome};
    for (pid, missing) in [2657, 2660, 2664, 2668, 2689]
        .into_iter()
        .flat_map(|pid| [(pid, false), (pid, true)])
    {
        let (handle, owner, mut device) = mount(pid, cx);
        cx.simulate_window_resize(handle, size(px(1600.), px(1400.)));
        cx.update_window(handle, |_, window, cx| {
            owner.update(cx, |view, cx| {
                if missing {
                    missing_profile(view, &mut device, window, cx);
                } else {
                    view.observe_earbuds_shared_connection(&reading(pid, false), cx);
                    view.set_page("TAB_SOUND", window, cx);
                }
            });
            window.render_frame(cx);
            window.click("earbuds-input-open", cx);
            window.click("earbuds-input-confirm", cx);
            owner.update(cx, |view, cx| {
                let request = view.earbuds_sound.pending.clone().unwrap();
                view.observe_earbuds_power_notification(
                    &Notification::Connection {
                        status: 0,
                        status_bt: 1,
                    },
                    cx,
                );
                assert!(view.earbuds_connection.disconnected());
                assert!(
                    view.earbuds_request_current(&request),
                    "wireless input switching does not replace the physical connection"
                );
                view.observe_earbuds_shared_connection_error("radio read failed".into(), cx);
                assert!(!view.earbuds_connection.ready());
                assert!(view.earbuds_request_current(&request));
                assert!(view.earbuds_sound.input_result.is_none());
                assert!(view.earbuds_activation.code.is_none());
                let mut response = reading(pid, false);
                response.operation = Operation::SwitchInputSource;
                response.connected = None;
                response.left_battery = None;
                response.right_battery = None;
                response.write = WriteOutcome::Confirmed;
                view.finish_earbuds_request(&request, Ok(response), cx);
                assert_eq!(
                    view.earbuds_sound.input_result,
                    Some(Ok(WriteOutcome::Confirmed))
                );
                assert!(!view.earbuds_connection.ready());
                view.observe_earbuds_shared_connection_error("second radio failure".into(), cx);
                assert_eq!(
                    view.earbuds_sound.input_result,
                    Some(Ok(WriteOutcome::Confirmed))
                );
                view.observe_earbuds_shared_connection(&reading(pid, false), cx);
            });
            window.render_frame(cx);
            window.click("earbuds-activation-show", cx);
            owner.update(cx, |view, cx| {
                let request = view.earbuds_sound.pending.clone().unwrap();
                for status in [0, 7] {
                    view.observe_earbuds_power_notification(
                        &Notification::Connection {
                            status,
                            status_bt: 1,
                        },
                        cx,
                    );
                    if !missing {
                        let monitor = view
                            .earbuds
                            .monitor
                            .clone()
                            .expect("active profile monitor");
                        view.observe_earbuds_notification(
                            &monitor,
                            Notification::Connection {
                                status,
                                status_bt: 1,
                            },
                            cx,
                        );
                    } else {
                        assert!(!view.earbuds.active);
                    }
                    assert!(view.earbuds_request_current(&request));
                    assert!(view.earbuds_activation.loading);
                }
                view.observe_earbuds_shared_connection_error("radio retry failed".into(), cx);
                assert!(view.earbuds_activation.loading);
                assert!(view.earbuds_activation.code.is_none());
                view.finish_earbuds_request(&request, Ok(code(pid, "DONGLE-CODE")), cx);
            });
            window.render_frame(cx);
            window.click("earbuds-activation-copy", cx);
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some("DONGLE-CODE".into())
            );
            owner.update(cx, |view, cx| {
                view.set_earbuds_power_context(&device, false, 2, cx);
                view.set_earbuds_sound_context(&device, false, 2, cx);
                assert!(view.earbuds_activation.code.is_none());
                assert!(view.earbuds_sound.input_result.is_none());
            });
            window.render_frame(cx);
            assert!(window.try_find("earbuds-activation-modal").is_none());
        })
        .unwrap();
    }
}
