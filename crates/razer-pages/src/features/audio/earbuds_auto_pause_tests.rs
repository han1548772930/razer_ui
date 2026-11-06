use super::{AudioProductWorkspace, AutoPauseObservation, Operation, Report};
use gpui_kit::{
    AnyWindowHandle, AppContext, Entity, TestAppContext, component::Root, px, size,
    test::TestWindowExt,
};
use serde_json::json;
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
        source_settings: Some(json!({"profile":{}})),
        settings: None,
        dpi_stages: None,
    }];
    device.device_container_id = "test-earbud-container".into();
    device.dashboard.connection_observation =
        Some(razer_model::model::DeviceConnectionObservation::UsbPresent);
    let profile = device.active_profile.clone();
    razer_storage::earbuds_profiles::merge_auto_pause(&mut device, &profile, false).unwrap();
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

fn reading(pid: u32, enabled: bool) -> Report {
    serde_json::from_value(json!({"product_id":pid,"operation":"Read","connected":true,
        "left_battery":80,"right_battery":90,"observation":{"Observed":enabled},"write":"Skipped"}))
    .unwrap()
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
                    AutoPauseObservation::ReadFailed(_)
                ));
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
                assert!(matches!(
                    view.earbuds.pending.as_ref().unwrap().operation(),
                    Operation::Restore {
                        enabled: Some(false)
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
                page.finish_earbuds_request(&read, Ok(reading(2657, false)), cx)
            });
            let restore = page.read(cx).earbuds.pending.clone().unwrap();
            assert!(restore.initializes_profile());
            assert!(matches!(
                restore.operation(),
                Operation::Restore {
                    enabled: Some(true)
                }
            ));
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
