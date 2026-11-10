use super::{AudioProductWorkspace, EarbudsPowerRequested};
use gpui_kit::{
    AnyWindowHandle, AppContext, Entity, Subscription, TestAppContext, component::Root, px, size,
    test::TestWindowExt,
};
use razer_device::{
    earbuds_auto_pause::{Operation, Report},
    earbuds_power::{MINUTE_CHOICES, Setting},
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc, sync::atomic::Ordering};

struct Fixture {
    window: AnyWindowHandle,
    owner: Entity<AudioProductWorkspace>,
    device: razer_model::model::Device,
    requests: Rc<RefCell<Vec<EarbudsPowerRequested>>>,
    _subscription: Subscription,
}
fn radio(pid: u32) -> Report {
    serde_json::from_value(json!({"product_id":pid,"operation":"Read","connected":true,
        "left_battery":78,"right_battery":82,"observation":"Unobserved","write":"Skipped"}))
    .unwrap()
}
fn result(request: &EarbudsPowerRequested, mode: Value, time: Value) -> Report {
    let mut report = radio(request.scope().product_id());
    report.operation = request.operation();
    report.connected = None;
    report.left_battery = None;
    report.right_battery = None;
    report.power = Some(
        serde_json::from_value(json!({
            "observation":{"mode":mode,"time":time},"mode_write":"Skipped","time_write":"Skipped"
        }))
        .unwrap(),
    );
    report
}
fn mount(pid: u32, cx: &mut TestAppContext) -> Fixture {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let mut device = razer_model::product_preview::registered_preview(pid).unwrap();
    device.device_container_id = "live-container".into();
    device.profiles.clear();
    device.active_profile.clear();
    let mut owner = None;
    let window = cx
        .open_window(size(px(1500.), px(900.)), |window, cx| {
            let view = cx.new(|cx| {
                let mut view = AudioProductWorkspace::new(pid, window, cx);
                view.set_page("TAB_POWER", window, cx);
                view
            });
            owner = Some(view.clone());
            Root::new(view, window, cx)
        })
        .into();
    let owner = owner.unwrap();
    let requests = Rc::new(RefCell::new(Vec::new()));
    let captured = requests.clone();
    let subscription = cx.update(|cx| {
        cx.subscribe(&owner, move |_, request: &EarbudsPowerRequested, _| {
            captured.borrow_mut().push(request.clone());
        })
    });
    cx.update_window(window, |_, _, cx| {
        owner.update(cx, |view, cx| {
            view.set_earbuds_power_context(&device, true, 1, cx);
            view.observe_earbuds_shared_connection(&radio(pid), cx);
        })
    })
    .unwrap();
    Fixture {
        window,
        owner,
        device,
        requests,
        _subscription: subscription,
    }
}

#[gpui_kit::test]
fn earbuds_power_all_pages_emit_complete_global_settings_without_creating_profiles(
    cx: &mut TestAppContext,
) {
    for pid in [2657, 2660, 2664, 2668, 2689] {
        let f = mount(pid, cx);
        let read = f.requests.borrow()[0].clone();
        cx.update_window(f.window, |_, window, cx| {
            window.render_frame(cx);
            assert!(
                !window
                    .find_all("earbuds-power-switch-unobserved")
                    .is_empty()
            );
            f.owner.update(cx, |view, cx| {
                view.finish_earbuds_power(
                    &read,
                    Ok(result(
                        &read,
                        json!({"Observed":true}),
                        json!({"Observed":900}),
                    )),
                    cx,
                )
            });
            window.render_frame(cx);
            assert_eq!(window.find("earbuds-power-switch").checked(), Some(true));
            assert_eq!(
                window.find(("earbuds-power-minutes", 15_u64)).selected(),
                Some(true)
            );
            let before = f.owner.read(cx).snapshot();
            window.click(("earbuds-power-minutes", 5_u64), cx);
            window.click("earbuds-power-switch", cx);
            assert_eq!(f.owner.read(cx).snapshot(), before);
            assert_eq!(
                f.owner.read(cx).earbuds_power.visible(),
                Some(Setting::new(false, 300))
            );
        })
        .unwrap();
        let requests = f.requests.borrow();
        assert_eq!(requests.len(), 3);
        assert_eq!(requests[1].setting(), Some(Setting::new(true, 300)));
        assert_eq!(requests[2].setting(), Some(Setting::new(false, 300)));
        assert!(f.device.profiles.is_empty());
        cx.update_window(f.window, |_, window, cx| {
            f.owner.update(cx, |view, cx| {
                view.finish_earbuds_power(
                    &requests[1],
                    Ok(result(
                        &requests[1],
                        json!({"Observed":true}),
                        json!({"Observed":300}),
                    )),
                    cx,
                )
            });
            window.render_frame(cx);
            assert_eq!(
                window.find("earbuds-power-switch").checked(),
                Some(false),
                "older acknowledgement cannot replace the newer draft"
            );
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn earbuds_power_partial_read_preserves_each_observed_control_and_disables_writes(
    cx: &mut TestAppContext,
) {
    for pid in [2657, 2660, 2664, 2668, 2689] {
        let f = mount(pid, cx);
        let read = f.requests.borrow()[0].clone();
        cx.update_window(f.window, |_, window, cx| {
            f.owner.update(cx, |view, cx| {
                view.finish_earbuds_power(
                    &read,
                    Ok(result(
                        &read,
                        json!({"Observed":true}),
                        json!({"ReadFailed":"time read failed"}),
                    )),
                    cx,
                )
            });
            window.render_frame(cx);
            assert_eq!(window.find("earbuds-power-switch").checked(), Some(true));
            assert_eq!(window.find("earbuds-power-switch").disabled(), Some(true));
            for minutes in MINUTE_CHOICES {
                assert_eq!(
                    window
                        .find(("earbuds-power-minutes", u64::from(minutes)))
                        .selected(),
                    Some(false)
                );
            }
            window.click("earbuds-power-switch", cx);
        })
        .unwrap();
        assert_eq!(f.requests.borrow().len(), 1);
        let f = mount(pid, cx);
        let read = f.requests.borrow()[0].clone();
        cx.update_window(f.window, |_, window, cx| {
            f.owner.update(cx, |view, cx| {
                view.finish_earbuds_power(
                    &read,
                    Ok(result(&read, json!({"Unknown":9}), json!({"Observed":900}))),
                    cx,
                )
            });
            window.render_frame(cx);
            assert!(window.find_all("earbuds-power-switch").is_empty());
            assert!(
                !window
                    .find_all("earbuds-power-switch-unobserved")
                    .is_empty()
            );
            assert_eq!(
                window.find(("earbuds-power-minutes", 15_u64)).selected(),
                Some(true)
            );
            assert_eq!(
                window.find(("earbuds-power-minutes", 15_u64)).disabled(),
                Some(true)
            );
            window.click(("earbuds-power-minutes", 5_u64), cx);
        })
        .unwrap();
        assert_eq!(f.requests.borrow().len(), 1);
    }
}

#[gpui_kit::test]
fn earbuds_power_radio_reconnect_rejects_old_results_and_nonradio_reports(cx: &mut TestAppContext) {
    let f = mount(2657, cx);
    let old = f.requests.borrow()[0].clone();
    cx.update_window(f.window, |_, _, cx| {
        f.owner.update(cx, |view, cx| {
            let mut unrelated = radio(2657);
            unrelated.operation = Operation::DoNotDisturb { enabled: true };
            unrelated.connected = None;
            unrelated.left_battery = None;
            unrelated.right_battery = None;
            view.observe_earbuds_shared_connection(&unrelated, cx);
            assert!(view.earbuds_connection.ready());
            view.observe_earbuds_power_notification(
                &razer_device::earbuds_events::Notification::Connection {
                    status: 0,
                    status_bt: 0,
                },
                cx,
            );
            view.finish_earbuds_power(
                &old,
                Ok(result(
                    &old,
                    json!({"Observed":true}),
                    json!({"Observed":300}),
                )),
                cx,
            );
            assert!(view.earbuds_power.visible().is_none());
            view.observe_earbuds_shared_connection(&radio(2657), cx);
        })
    })
    .unwrap();
    let fresh = f.requests.borrow().last().unwrap().clone();
    assert_ne!(old.scope().id(), fresh.scope().id());
    cx.update_window(f.window, |_, window, cx| {
        f.owner.update(cx, |view, cx| {
            view.finish_earbuds_power(
                &fresh,
                Ok(result(
                    &fresh,
                    json!({"Observed":false}),
                    json!({"Observed":1800}),
                )),
                cx,
            );
            view.finish_earbuds_power(
                &old,
                Ok(result(
                    &old,
                    json!({"Observed":true}),
                    json!({"Observed":300}),
                )),
                cx,
            );
        });
        window.render_frame(cx);
        assert_eq!(window.find("earbuds-power-switch").checked(), Some(false));
        assert_eq!(
            window.find(("earbuds-power-minutes", 30_u64)).selected(),
            Some(true)
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn earbuds_power_explicit_profile_restore_is_unsaved_and_cancelled_on_profile_change(
    cx: &mut TestAppContext,
) {
    for pid in [2657, 2660, 2664, 2668, 2689] {
        let mut f = mount(pid, cx);
        let read = f.requests.borrow()[0].clone();
        f.device.profiles.push(razer_model::model::Profile {
            id: "with-power".into(),
            guid: "with-power".into(),
            name: "With power".into(),
            source_settings: Some(json!({"profile":{"powerSaving":{"isEnabled":true,"value":15}}})),
            settings: None,
            dpi_stages: None,
        });
        f.device.active_profile = "with-power".into();
        cx.update_window(f.window, |_, _, cx| {
            f.owner.update(cx, |view, cx| {
                view.set_earbuds_power_context(&f.device, true, 1, cx);
                view.finish_earbuds_power(
                    &read,
                    Ok(result(
                        &read,
                        json!({"Observed":false}),
                        json!({"Observed":300}),
                    )),
                    cx,
                );
            })
        })
        .unwrap();
        let restore = f.requests.borrow().last().unwrap().clone();
        assert_eq!(restore.setting(), Some(Setting::new(true, 900)));
        assert!(!restore.needs_persistence());
        f.device.active_profile = "missing-profile".into();
        cx.update_window(f.window, |_, _, cx| {
            f.owner.update(cx, |view, cx| {
                view.set_earbuds_power_context(&f.device, true, 1, cx);
                view.finish_earbuds_power(
                    &restore,
                    Ok(result(
                        &restore,
                        json!({"Observed":true}),
                        json!({"Observed":900}),
                    )),
                    cx,
                );
                assert!(view.earbuds_power.report.is_none());
                assert!(view.earbuds_power.persisted.is_none());
            })
        })
        .unwrap();
        assert!(!restore.guard().load(Ordering::Acquire));
        let fresh = f.requests.borrow().last().unwrap().clone();
        assert_eq!(fresh.operation(), Operation::ReadPowerSaving);
        cx.update_window(f.window, |_, _, cx| {
            f.owner.update(cx, |view, cx| {
                view.finish_earbuds_power(
                    &fresh,
                    Ok(result(
                        &fresh,
                        json!({"Observed":false}),
                        json!({"Observed":300}),
                    )),
                    cx,
                );
                assert_eq!(view.earbuds_power.visible(), Some(Setting::new(false, 300)));
            })
        })
        .unwrap();
    }
}
