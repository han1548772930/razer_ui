use super::{
    AudioProductWorkspace, EarbudsVolumeRequested, Edit, Ordering, Setting, Snapshot, Update, Uuid,
};
use gpui_kit::{
    AnyWindowHandle, AppContext, Entity, InputEvent, MouseButton, TestAppContext,
    component::{Root, slider::SliderEvent},
    point, px, size,
    test::TestWindowExt,
};
use razer_device::simple_audio::{EndpointType, SimpleAudioEndpoint};
use razer_device::simple_audio_volume::AudioVolumeReadResult;
use std::cell::RefCell;
use std::rc::Rc;

fn reading(volume: u8, muted: bool, request_id: Option<Uuid>) -> Update {
    let endpoint = SimpleAudioEndpoint {
        id: "live-endpoint".into(),
        name: "Headphones".into(),
        container_id: "observed-container".into(),
        endpoint_type: EndpointType::Speaker,
    };
    let snapshot = Snapshot::from_read(
        endpoint,
        AudioVolumeReadResult {
            result: true,
            reason: String::new(),
            device_id: "live-endpoint".into(),
            muted,
            volume,
        },
    )
    .unwrap();
    let update = Update::new(Ok(snapshot));
    match request_id {
        Some(id) => update.with_request_id(id),
        None => update,
    }
}

fn mount(
    pid: u32,
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
    device.device_container_id = "observed-container".into();
    device.active_profile = "profile-a".into();
    let mut owner = None;
    let window = cx.open_window(size(px(1600.), px(1000.)), |window, cx| {
        let view = cx.new(|cx| {
            let mut view = AudioProductWorkspace::new(pid, window, cx);
            view.set_page("TAB_SOUND", window, cx);
            view.set_earbuds_volume_context(&device, true, 1, cx);
            view
        });
        owner = Some(view.clone());
        Root::new(view, window, cx)
    });
    (window.into(), owner.unwrap(), device)
}

#[gpui_kit::test]
fn all_variants_read_real_volume_and_toggle_without_saving_the_profile(cx: &mut TestAppContext) {
    for pid in [2657, 2660, 2664, 2668, 2689] {
        let (handle, owner, _) = mount(pid, cx);
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("earbuds-volume-switch").disabled(), Some(true));
            let before = owner.read(cx).snapshot();
            let id = owner.read(cx).earbuds_volume.scope.as_ref().unwrap().id();
            owner.update(cx, |view, cx| {
                view.finish_earbuds_volume(id, reading(83, false, None), window, cx);
            });
            window.render_frame(cx);
            assert_ne!(window.find("earbuds-volume-switch").disabled(), Some(true));
            window.click("earbuds-volume-switch", cx);
            owner.update(cx, |view, cx| {
                assert_eq!(view.snapshot(), before, "SET_VOLUME is Storage_NoSave");
                assert_eq!(
                    view.earbuds_volume.observed.as_ref().unwrap().setting(),
                    Setting::new(true, 83).unwrap()
                );
                assert_eq!(
                    view.earbuds_volume.draft,
                    Some(Setting::new(false, 83).unwrap())
                );
                let request_id = *view.earbuds_volume.pending.last().unwrap();
                view.finish_earbuds_volume(id, reading(83, true, Some(request_id)), window, cx);
                assert!(view.earbuds_volume.draft.is_none());
                assert!(view.earbuds_volume.pending.is_empty());
                assert_eq!(
                    view.earbuds_volume.visible(),
                    Some(Setting::new(false, 83).unwrap())
                );
                view.finish_earbuds_volume(
                    id,
                    Update::new(Err("endpoint disconnected".into())),
                    window,
                    cx,
                );
                assert!(view.earbuds_volume.observed.is_none());
                assert!(view.earbuds_volume.visible().is_none());
            });
            window.render_frame(cx);
            assert_eq!(window.find("earbuds-volume-switch").disabled(), Some(true));
            window.click("earbuds-volume-switch", cx);
            assert!(owner.read(cx).earbuds_volume.pending.is_empty());
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn real_pointer_release_submits_volume_without_toggling_mute_and_arrows_leave_value_alone(
    cx: &mut TestAppContext,
) {
    let (handle, owner, _) = mount(2660, cx);
    let edits = Rc::new(RefCell::new(Vec::new()));
    let captured = edits.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&owner, move |_, event: &EarbudsVolumeRequested, _| {
            if let EarbudsVolumeRequested::Edit { setting, .. } = event {
                captured.borrow_mut().push(*setting);
            }
        })
    });
    cx.update_window(handle, |_, window, cx| {
        let id = owner.read(cx).earbuds_volume.scope.as_ref().unwrap().id();
        owner.update(cx, |view, cx| {
            view.finish_earbuds_volume(id, reading(83, false, None), window, cx)
        });
        window.render_frame(cx);
        let state = owner.read(cx).earbuds_volume.slider.clone().unwrap();
        let bounds = state.read(cx).bounds();
        assert!(bounds.size.width > px(0.) && bounds.size.height > px(0.));
        let position = point(bounds.left(), bounds.center().y);
        window.simulate_mouse_move(position, cx);
        window.render_frame(cx);
        window.dispatch_event(
            gpui_kit::MouseDownEvent {
                button: MouseButton::Left,
                position,
                modifiers: Default::default(),
                click_count: 1,
                first_mouse: false,
            }
            .to_platform_input(),
            cx,
        );
        window.render_frame(cx);
        assert!(
            owner.read(cx).earbuds_volume.pending.is_empty(),
            "Dragging previews until the global release"
        );
        window.dispatch_event(
            gpui_kit::MouseUpEvent {
                button: MouseButton::Left,
                position: point(bounds.right() + px(40.), bounds.bottom() + px(30.)),
                modifiers: Default::default(),
                click_count: 1,
            }
            .to_platform_input(),
            cx,
        );
        window.render_frame(cx);
    })
    .unwrap();
    assert_eq!(*edits.borrow(), [Setting::new(true, 0).unwrap()]);
    cx.update_window(handle, |_, window, cx| {
        assert!(owner.read(cx).earbuds_volume.draft.unwrap().enabled());
        assert_eq!(owner.read(cx).earbuds_volume.pending.len(), 1);
        window.press("right", cx);
        window.press("home", cx);
        window.press("end", cx);
    })
    .unwrap();
    assert_eq!(*edits.borrow(), [Setting::new(true, 0).unwrap()]);
}

#[gpui_kit::test]
fn rejected_volume_retains_the_optimistic_envelope_until_a_real_prop_changes(
    cx: &mut TestAppContext,
) {
    let (handle, owner, _) = mount(2664, cx);
    let edits = Rc::new(RefCell::new(Vec::new()));
    let captured = edits.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&owner, move |_, event: &EarbudsVolumeRequested, _| {
            if let EarbudsVolumeRequested::Edit { setting, .. } = event {
                captured.borrow_mut().push(*setting);
            }
        })
    });
    cx.update_window(handle, |_, window, cx| {
        owner.update(cx, |view, cx| {
            let id = view.earbuds_volume.scope.as_ref().unwrap().id();
            view.finish_earbuds_volume(id, reading(37, false, None), window, cx);
            view.edit_earbuds_volume(Edit::Volume(83), cx);
            let request_id = *view.earbuds_volume.pending.last().unwrap();
            view.finish_earbuds_volume(
                id,
                reading(37, false, Some(request_id)).with_error(Some("volume rejected".into())),
                window,
                cx,
            );
            assert_eq!(
                view.earbuds_volume
                    .observed
                    .as_ref()
                    .unwrap()
                    .setting()
                    .volume(),
                37
            );
            assert_eq!(view.earbuds_volume.draft.unwrap().volume(), 83);
            assert_eq!(
                view.earbuds_volume.error.as_deref(),
                Some("volume rejected")
            );
        });
        window.render_frame(cx);
        window.click("earbuds-volume-switch", cx);
        assert_eq!(
            owner.read(cx).earbuds_volume.draft,
            Some(Setting::new(false, 83).unwrap())
        );
        owner.update(cx, |view, cx| {
            let id = view.earbuds_volume.scope.as_ref().unwrap().id();
            view.finish_earbuds_volume(id, Update::new(Err("readback failed".into())), window, cx);
            assert!(view.earbuds_volume.visible().is_none());
            view.edit_earbuds_volume(Edit::Enabled(true), cx);
        });
    })
    .unwrap();
    assert_eq!(
        *edits.borrow(),
        [
            Setting::new(true, 83).unwrap(),
            Setting::new(false, 83).unwrap()
        ]
    );
}

#[gpui_kit::test]
fn profile_connection_and_page_switch_cancel_old_callbacks_and_slider_subscriptions(
    cx: &mut TestAppContext,
) {
    let (handle, owner, mut device) = mount(2668, cx);
    let old_slider = cx.update(|cx| owner.read(cx).earbuds_volume.slider.clone().unwrap());
    cx.update_window(handle, |_, window, cx| {
        let first = owner.read(cx).earbuds_volume.scope.as_ref().unwrap().id();
        let guard = owner.read(cx).earbuds_volume.active.clone().unwrap();
        owner.update(cx, |view, cx| {
            view.finish_earbuds_volume(first, reading(91, false, None), window, cx);
            device.active_profile = "profile-b".into();
            view.set_earbuds_volume_context(&device, true, 1, cx);
            assert!(!guard.load(Ordering::Acquire));
            let second = view.earbuds_volume.scope.as_ref().unwrap().id();
            assert_ne!(first, second);
            view.finish_earbuds_volume(first, reading(99, true, None), window, cx);
            assert!(view.earbuds_volume.observed.is_none());
            view.finish_earbuds_volume(second, reading(28, false, None), window, cx);
        });
    })
    .unwrap();
    old_slider.update(cx, |_, cx| cx.emit(SliderEvent::Release(95_f32.into())));
    cx.update_window(handle, |_, window, cx| {
        owner.update(cx, |view, cx| {
            assert!(view.earbuds_volume.pending.is_empty());
            assert_eq!(view.earbuds_volume.visible().unwrap().volume(), 28);
            let second = view.earbuds_volume.scope.as_ref().unwrap().id();
            view.set_earbuds_volume_context(&device, true, 2, cx);
            view.finish_earbuds_volume(second, reading(88, false, None), window, cx);
            assert!(view.earbuds_volume.observed.is_none());
            let guard = view.earbuds_volume.active.clone().unwrap();
            view.set_page("TAB_ENHANCEMENT", window, cx);
            assert!(!guard.load(Ordering::Acquire));
            assert!(view.earbuds_volume.slider.is_none());
            assert!(view.earbuds_volume.scope.is_none());
        });
    })
    .unwrap();
}
