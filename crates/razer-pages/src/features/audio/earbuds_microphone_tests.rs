use super::{AudioProductWorkspace, EarbudsMicrophoneRequested};
use gpui_kit::{
    AnyWindowHandle, AppContext, Entity, Subscription, TestAppContext, component::Root, point, px,
    size, test::TestWindowExt,
};
use razer_device::{
    earbuds_events::Notification,
    earbuds_microphone::{Setting, Snapshot, Update},
    earbuds_protocol40::ToggleObservation,
    simple_audio::{EndpointType, SimpleAudioEndpoint},
    simple_audio_volume::AudioVolumeReadResult,
};
use std::{cell::RefCell, rc::Rc};
use uuid::Uuid;

struct Fixture {
    window: AnyWindowHandle,
    owner: Entity<AudioProductWorkspace>,
    device: razer_model::model::Device,
    requests: Rc<RefCell<Vec<EarbudsMicrophoneRequested>>>,
    _subscription: Subscription,
}
fn radio(pid: u32) -> razer_device::earbuds_auto_pause::Report {
    serde_json::from_value(
        serde_json::json!({"product_id":pid,"operation":"Read","connected":true,
        "left_battery":78,"right_battery":82,"observation":"Unobserved","write":"Skipped"}),
    )
    .unwrap()
}
fn reading(volume: u8, system_muted: bool, hardware: ToggleObservation) -> Update {
    let endpoint = SimpleAudioEndpoint {
        id: "microphone".into(),
        name: "Input".into(),
        container_id: "live-container".into(),
        endpoint_type: EndpointType::Microphone,
    };
    let read = AudioVolumeReadResult {
        result: true,
        reason: String::new(),
        device_id: endpoint.id.clone(),
        muted: system_muted,
        volume,
    };
    let system = razer_device::earbuds_system_volume::Snapshot::from_read(endpoint, read).unwrap();
    Update::new(Ok(Snapshot::new(Ok(system), hardware)))
}
fn mount(pid: u32, cx: &mut TestAppContext) -> Fixture {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let mut device = razer_model::product_preview::registered_preview(pid).unwrap();
    device.device_container_id = "live-container".into();
    device.profiles.clear();
    device.active_profile = "missing-profile".into();
    let mut owner = None;
    let window = cx
        .open_window(size(px(1500.), px(900.)), |window, cx| {
            let view = cx.new(|cx| {
                let mut view = AudioProductWorkspace::new(pid, window, cx);
                view.set_page("TAB_MIC", window, cx);
                view
            });
            owner = Some(view.clone());
            Root::new(view, window, cx)
        })
        .into();
    let owner = owner.unwrap();
    let requests = Rc::new(RefCell::new(Vec::new()));
    let capture = requests.clone();
    let subscription = cx.update(|cx| {
        cx.subscribe(&owner, move |_, request: &EarbudsMicrophoneRequested, _| {
            capture.borrow_mut().push(request.clone());
        })
    });
    cx.update_window(window, |_, _, cx| {
        owner.update(cx, |view, cx| {
            view.set_earbuds_power_context(&device, true, 1, cx);
            view.set_earbuds_microphone_context(&device, true, 1, cx);
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
fn scope(f: &Fixture) -> Uuid {
    f.requests
        .borrow()
        .iter()
        .rev()
        .find_map(|event| match event {
            EarbudsMicrophoneRequested::Start { scope, .. } => Some(scope.id()),
            _ => None,
        })
        .unwrap()
}

#[gpui_kit::test]
fn earbuds_microphone_all_pages_toggle_and_drag_hardware_enabled_volume_without_profile_save(
    cx: &mut TestAppContext,
) {
    for pid in [2657, 2660, 2664, 2668, 2689] {
        let f = mount(pid, cx);
        let id = scope(&f);
        cx.update_window(f.window, |_, window, cx| {
            f.owner.update(cx, |view, cx| {
                view.finish_earbuds_microphone(
                    id,
                    reading(83, true, ToggleObservation::Observed(false)),
                    window,
                    cx,
                )
            });
            window.render_frame(cx);
            assert_eq!(
                window.find("earbuds-microphone-switch").checked(),
                Some(true),
                "hardware mute controls the switch, not OS mute"
            );
            let before = f.owner.read(cx).snapshot();
            window.click("earbuds-microphone-switch", cx);
            assert_eq!(
                f.owner.read(cx).earbuds_microphone.visible(),
                Some(Setting::new(false, 83).unwrap())
            );
            window.press("space", cx);
            window.render_frame(cx);
            assert_eq!(
                window.find("earbuds-microphone-switch").checked(),
                Some(true)
            );
            let bounds = window.find("earbuds-microphone-range").bounds();
            let y = bounds.bottom() - px(28.);
            window.drag(
                point(
                    bounds.left() + px(8.) + (bounds.size.width - px(16.)) * 0.83,
                    y,
                ),
                point(
                    bounds.left() + px(8.) + (bounds.size.width - px(16.)) * 0.2,
                    y,
                ),
                cx,
            );
            assert_eq!(f.owner.read(cx).snapshot(), before);
        })
        .unwrap();
        let edits = f
            .requests
            .borrow()
            .iter()
            .filter_map(|event| match event {
                EarbudsMicrophoneRequested::Edit { setting, .. } => Some(*setting),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            edits,
            [
                Setting::new(false, 83).unwrap(),
                Setting::new(true, 83).unwrap(),
                Setting::new(true, 20).unwrap()
            ]
        );
    }
}

#[gpui_kit::test]
fn earbuds_microphone_partial_observations_never_become_default_mute_or_volume(
    cx: &mut TestAppContext,
) {
    for pid in [2657, 2660, 2664, 2668, 2689] {
        let f = mount(pid, cx);
        let id = scope(&f);
        cx.update_window(f.window, |_, window, cx| {
            f.owner.update(cx, |view, cx| {
                view.finish_earbuds_microphone(
                    id,
                    Update::new(Ok(Snapshot::new(
                        Err("system volume unavailable".into()),
                        ToggleObservation::Observed(false),
                    ))),
                    window,
                    cx,
                )
            });
            window.render_frame(cx);
            assert_eq!(
                window.find("earbuds-microphone-switch").checked(),
                Some(true)
            );
            assert_eq!(
                window.find("earbuds-microphone-switch").disabled(),
                Some(true)
            );
            assert!(
                !window
                    .find_all("earbuds-microphone-volume-unobserved")
                    .is_empty()
            );
            window.click("earbuds-microphone-switch", cx);
            f.owner.update(cx, |view, cx| {
                view.finish_earbuds_microphone(
                    id,
                    reading(57, false, ToggleObservation::Unknown(9)),
                    window,
                    cx,
                )
            });
            window.render_frame(cx);
            assert!(window.find_all("earbuds-microphone-switch").is_empty());
            assert!(
                !window
                    .find_all("earbuds-microphone-switch-unobserved")
                    .is_empty()
            );
            assert_eq!(
                f.owner.read(cx).earbuds_microphone.visible_volume(),
                Some(57)
            );
            assert!(f.owner.read(cx).earbuds_microphone.visible().is_none());
        })
        .unwrap();
        assert_eq!(f.requests.borrow().len(), 1);
    }
}

#[gpui_kit::test]
fn earbuds_microphone_source_mute_help_differs_only_for_2664_and_clears_on_leave(
    cx: &mut TestAppContext,
) {
    for pid in [2657, 2660, 2664, 2668, 2689] {
        let f = mount(pid, cx);
        let id = scope(&f);
        cx.update_window(f.window, |_, window, cx| {
            f.owner.update(cx, |view, cx| {
                view.finish_earbuds_microphone(
                    id,
                    reading(42, false, ToggleObservation::Observed(true)),
                    window,
                    cx,
                )
            });
            window.render_frame(cx);
            assert_eq!(
                !window
                    .find_all("earbuds-microphone-muted-description")
                    .is_empty(),
                pid != 2664
            );
            window.hover("earbuds-microphone-panel", cx);
            window.render_frame(cx);
            if pid == 2664 {
                let cursor = f.owner.read(cx).earbuds_microphone.cursor.unwrap();
                let bounds = window.find("earbuds-microphone-muted-tip").bounds();
                assert!((bounds.left() - cursor.x).abs() < px(0.25));
                assert!(
                    (bounds.top() - cursor.y - window.rem_size() * (25. / 16.)).abs() < px(0.25)
                );
                window.hover("earbuds-microphone-help", cx);
                window.render_frame(cx);
                assert!(window.find_all("earbuds-microphone-muted-tip").is_empty());
            } else {
                assert!(window.find_all("earbuds-microphone-muted-tip").is_empty());
            }
            f.owner
                .update(cx, |view, cx| view.set_page("TAB_SOUND", window, cx));
            assert!(!f.owner.read(cx).earbuds_microphone_current(id));
            assert!(f.owner.read(cx).earbuds_microphone.cursor.is_none());
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn earbuds_microphone_reconnect_and_hardware_notifications_keep_the_current_scope(
    cx: &mut TestAppContext,
) {
    let mut f = mount(2664, cx);
    let old = scope(&f);
    cx.update_window(f.window, |_, _, cx| {
        f.owner.update(cx, |view, cx| {
            view.observe_earbuds_microphone_notification(
                &Notification::MicrophoneMute { muted: 1 },
                cx,
            );
            view.set_earbuds_microphone_context(&f.device, false, 1, cx);
        })
    })
    .unwrap();
    assert!(
        matches!(f.requests.borrow().last(), Some(EarbudsMicrophoneRequested::HardwareMute { scope_id, muted: 1 }) if *scope_id == old)
    );
    f.device.active_profile = "next-profile".into();
    cx.update_window(f.window, |_, window, cx| {
        f.owner.update(cx, |view, cx| {
            view.set_earbuds_microphone_context(&f.device, true, 2, cx);
            view.finish_earbuds_microphone(
                old,
                reading(99, false, ToggleObservation::Observed(false)),
                window,
                cx,
            );
            assert!(view.earbuds_microphone.observed.is_none());
        })
    })
    .unwrap();
    assert_ne!(scope(&f), old);
}
