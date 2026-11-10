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
        cx.update_window(handle, |_, window, cx| {
            owner.update(cx, |view, cx| {
                missing_profile(view, &mut device, window, cx)
            });
            window.render_frame(cx);
            let before = owner.read(cx).snapshot();
            assert_eq!(window.find("earbuds-input-open").disabled(), Some(false));
            window.click("earbuds-input-open", cx);
            window.click("earbuds-input-confirm", cx);
            let first = requests.borrow()[0].clone();
            assert_eq!(first.operation(), Operation::SwitchInputSource);
            assert!(!first.needs_persistence() && !first.initializes_profile());
            window.render_frame(cx);
            window.click("earbuds-activation-show", cx);
            assert_eq!(
                requests.borrow().len(),
                1,
                "later read waits for the real worker completion"
            );
            owner.update(cx, |view, cx| {
                view.finish_earbuds_request(&first, Err("switch failed".into()), cx)
            });
            let second = requests.borrow()[1].clone();
            assert_eq!(second.operation(), Operation::ActivationCode);
            assert!(!second.needs_persistence());
            window.render_frame(cx);
            assert!(window.find("earbuds-input-error").visible());
            assert_eq!(
                window.find("earbuds-activation-copy").disabled(),
                Some(true)
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
            let first = requests.borrow()[0].clone();
            window.click("earbuds-activation-close", cx);
            window.click("earbuds-activation-show", cx);
            assert!(!first.guard().load(Ordering::Acquire));
            assert_eq!(requests.borrow().len(), 1);
            owner.update(cx, |view, cx| {
                view.finish_earbuds_request(&first, Ok(code(pid, "OLD")), cx)
            });
            assert!(owner.read(cx).earbuds_activation.code.is_none());
            let second = requests.borrow()[1].clone();
            owner.update(cx, |view, cx| {
                view.finish_earbuds_request(&second, Err("code read failed".into()), cx)
            });
            window.render_frame(cx);
            assert!(window.find("earbuds-activation-error").visible());
            assert_eq!(
                window.find("earbuds-activation-copy").disabled(),
                Some(true)
            );
            window.click("earbuds-activation-close", cx);
            window.click("earbuds-activation-show", cx);
            let third = requests.borrow()[2].clone();
            owner.update(cx, |view, cx| {
                view.set_earbuds_power_context(&device, true, 2, cx);
                view.set_earbuds_sound_context(&device, true, 2, cx);
                view.observe_earbuds_shared_connection(&reading(pid, false), cx);
            });
            window.render_frame(cx);
            window.click("earbuds-input-open", cx);
            window.click("earbuds-input-confirm", cx);
            assert!(!third.guard().load(Ordering::Acquire));
            assert_eq!(requests.borrow().len(), 3);
            owner.update(cx, |view, cx| {
                view.finish_earbuds_request(&third, Ok(code(pid, "STALE")), cx)
            });
            let next = requests.borrow()[3].clone();
            assert_eq!(next.operation(), Operation::SwitchInputSource);
            assert_ne!(next.device_scope(), third.device_scope());
            assert!(owner.read(cx).earbuds_activation.code.is_none());
            assert!(owner.read(cx).earbuds_sound.input_result.is_none());
        })
        .unwrap();
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
