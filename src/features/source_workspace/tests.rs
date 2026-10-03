//! Compiled by cargo check --all-targets; not executed under the source-audit policy.
use super::SourceProductWorkspace;
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext, px, size};
use serde_json::json;

#[gpui_kit::test]
fn receiver_never_mounts_a_profile_selector(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let handle = cx.open_window(size(px(1280.), px(900.)), |window, cx| {
        let view = cx.new(|cx| {
            SourceProductWorkspace::new(crate::demo::registered_preview(179).unwrap(), window, cx)
        });
        Root::new(view, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("source-profile-select").is_none());
        assert!(window.try_find("source-product-navigation").is_some());
    })
    .unwrap();
}

#[test]
fn legacy_layout_moves_once_from_active_profile_and_device_value_wins() {
    let mut device = crate::demo::registered_preview(784).unwrap();
    let mut active = device.profiles[0].clone();
    active.id = "active".into();
    active.source_settings = Some(json!({"_accessory":{"bendData":[1,2]},"brightness":75}));
    let mut inactive = active.clone();
    inactive.id = "inactive".into();
    inactive.source_settings = Some(json!({"_accessory":{"bendData":[3]},"brightness":10}));
    device.profiles = vec![inactive, active];
    device.active_profile = "active".into();
    SourceProductWorkspace::migrate_device_settings(&mut device);
    assert_eq!(
        device.source_device_settings.as_ref().unwrap()["_accessory"],
        json!({"bendData":[1,2]})
    );
    assert!(device.profiles.iter().all(|p| {
        p.source_settings
            .as_ref()
            .unwrap()
            .get("_accessory")
            .is_none()
    }));
    assert_eq!(
        device.profiles[0].source_settings.as_ref().unwrap()["brightness"],
        10
    );
    device.profiles[0].source_settings.as_mut().unwrap()["_accessory"] = json!({"bendData":[4]});
    device.active_profile = "inactive".into();
    SourceProductWorkspace::migrate_device_settings(&mut device);
    assert_eq!(
        device.source_device_settings.as_ref().unwrap()["_accessory"],
        json!({"bendData":[1,2]})
    );
}

#[gpui_kit::test]
fn lighting_changes_and_profile_switches_preserve_device_layout(cx: &mut TestAppContext) {
    cx.update(|cx| gpui_kit::init(cx));
    let handle = cx.open_window(size(px(1280.), px(900.)), |window, cx| {
        Root::new(
            cx.new(|cx| {
                SourceProductWorkspace::new(
                    crate::demo::registered_preview(784).unwrap(),
                    window,
                    cx,
                )
            }),
            window,
            cx,
        )
    });
    cx.update_window(handle.into(), |_, window, cx| {
        let view = cx.new(|cx| {
            SourceProductWorkspace::new(crate::demo::registered_preview(784).unwrap(), window, cx)
        });
        view.update(cx, |workspace, cx| {
            let layout =
                workspace.device.source_device_settings.as_ref().unwrap()["_accessory"].clone();
            let mut second = workspace.device.profiles[0].clone();
            second.id = "second".into();
            second.source_settings = None;
            workspace.device.profiles.push(second);
            workspace.capture(json!({"brightness":25}), cx);
            workspace.device.active_profile = "second".into();
            workspace.restore_active(window, cx);
            assert_eq!(
                workspace.device.source_device_settings.as_ref().unwrap()["_accessory"],
                layout
            );
            assert!(workspace.device.profiles.iter().all(|p| {
                p.source_settings
                    .as_ref()
                    .is_none_or(|s| s.get("_accessory").is_none())
            }));
        });
    })
    .unwrap();
}
