use super::{DeviceWorkspace, profile};
use crate::features::settings::Effect;
use crate::nav::Tab;
use gpui_kit::component::{Root, Theme};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, ScrollDelta, SharedString, TestAppContext, point, px, size};
use razer_catalog::AUDITED_MOUSE_MAT_IDS;
use razer_model::demo::mouse_mat_preview;

#[test]
fn new_mat_profiles_use_product_defaults_and_preserve_edited_profiles() {
    for pid in AUDITED_MOUSE_MAT_IDS {
        let mut device = mouse_mat_preview(pid).unwrap();
        let saved = device.clone();
        let original_id = device.active_profile.clone();
        let original = device.profiles[0].settings.as_mut().unwrap();
        original.lighting.brightness = 23;
        original.lighting.display_off = true;
        original.lighting.effect = Effect::Static;

        profile::create_local_profile(&mut device, &saved, false);
        let fresh = device
            .active_profile_obj()
            .unwrap()
            .settings
            .as_ref()
            .unwrap();
        assert_eq!(
            fresh.lighting.brightness,
            if matches!(pid, 3076 | 3077 | 3080) {
                66
            } else {
                100
            }
        );
        assert_eq!(fresh.lighting.idle_minutes, 1);
        assert_eq!(fresh.lighting.effect, Effect::Spectrum);
        assert!(!fresh.lighting.display_off);
        assert_ne!(device.active_profile, original_id);

        let stored = serde_json::to_vec(&device).unwrap();
        let restored: razer_model::model::Device = serde_json::from_slice(&stored).unwrap();
        assert_eq!(restored.product_id, pid);
        let edited = restored.profiles[0].settings.as_ref().unwrap();
        assert_eq!(edited.lighting.brightness, 23);
        assert!(edited.lighting.display_off);
        assert_eq!(edited.lighting.effect, Effect::Static);
        assert_eq!(Tab::for_product(pid), &[Tab::Lighting]);
        assert!(!restored.has_battery);
        assert!(restored.dkm_keys.is_empty());
        assert!(restored.features.keyboard.is_none());
        assert!(restored.features.power.is_none());
        assert!(restored.firmware_info.current_fw_version.is_empty());
        assert!(restored.ui_window_name.is_empty());
    }
    assert!(mouse_mat_preview(3075).is_none());
}

#[gpui_kit::test]
fn mat_display_off_and_wave_controls_follow_the_product(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
        Theme::update(cx, |theme| theme.font_size = px(16.));
    });
    for (pid, direction) in [(3072, 11), (3076, 11), (3077, 11), (3080, 1)] {
        let mut workspace = None;
        let handle = cx.open_window(size(px(1280.), px(1000.)), |window, cx| {
            let mut device = mouse_mat_preview(pid).unwrap();
            device.profiles[0]
                .settings
                .as_mut()
                .unwrap()
                .lighting
                .effect = Effect::Wave;
            let view = cx.new(|cx| DeviceWorkspace::new(device, true, window, cx));
            workspace = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = workspace.unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("lighting-idle-enabled").is_none());
            window.click(format!("effect-direction-{direction}"), cx);
            window.click("lighting-display-off", cx);
            window.click("brightness-enabled", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("lighting-display-off").disabled(), Some(true));
            window.click("lighting-display-off", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let lighting = &view.read(cx).settings().lighting;
            assert!(lighting.display_off);
            assert!(!lighting.enabled);
            assert_eq!(lighting.params().direction, direction);
        });
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("device-help", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("profile-select").is_none());
            assert!(window.try_find("help-device-support").is_some());
            window.click("device-tab-lighting", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            assert_eq!(
                view.read(cx).settings().lighting.params().direction,
                direction
            )
        });
    }
}

#[gpui_kit::test]
fn mat_reactive_requires_a_connected_mouse_and_keeps_its_parameters(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let mut workspace = None;
    let handle = cx.open_window(size(px(1280.), px(1000.)), |window, cx| {
        let mut device = mouse_mat_preview(3073).unwrap();
        let lighting = &mut device.profiles[0].settings.as_mut().unwrap().lighting;
        lighting.effect = Effect::Reactive;
        lighting.params_mut().duration = 3;
        let view = cx.new(|cx| DeviceWorkspace::new(device, true, window, cx));
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = workspace.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("lighting-reactive-warning").is_some());
        assert!(window.try_find("lighting-idle-enabled").is_none());
        window.within("lighting-effect").click("input", cx);
        window.press("down", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("lighting-reactive-warning").is_none());
        window.within("lighting-effect").click("input", cx);
        window.press("up", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("lighting-reactive-warning").is_some());
    })
    .unwrap();
    cx.update(|cx| {
        let lighting = &view.read(cx).settings().lighting;
        assert_eq!(lighting.effect, Effect::Reactive);
        assert_eq!(lighting.params().duration, 3);
    });
}

#[gpui_kit::test]
fn mat_product_art_precedes_controls_and_scrolls_with_the_page(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    for (index, pid) in AUDITED_MOUSE_MAT_IDS.into_iter().enumerate() {
        let scale = if index % 2 == 0 { 1. } else { 1.25 };
        cx.update(|cx| Theme::update(cx, |theme| theme.font_size = px(16. * scale)));
        let mut workspace = None;
        let handle = cx.open_window(size(px(1280. * scale), px(1000. * scale)), |window, cx| {
            let view = cx
                .new(|cx| DeviceWorkspace::new(mouse_mat_preview(pid).unwrap(), true, window, cx));
            workspace = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = workspace.unwrap();
        let body_id = cx.update(|cx| format!("device-body-{}", view.read(cx).identity()));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let banner = window.find("lighting-product-banner").bounds();
            let columns = window.find("page-columns").bounds();
            assert_eq!(banner.size, size(px(1220. * scale), px(250. * scale)));
            assert!((columns.top() - banner.bottom() - px(20. * scale)).abs() <= px(1.));
            assert!(window.try_find("lighting-product-image").is_some());
            window.click("device-help", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("lighting-product-banner").is_none());
            window.click("device-tab-lighting", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.simulate_window_resize(handle.into(), size(px(700. * scale), px(1000. * scale)));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let before = window.find("lighting-product-banner").bounds();
            assert_eq!(before.size, size(px(1024. * scale), px(250. * scale)));
            window.scroll(
                SharedString::from(body_id.clone()),
                ScrollDelta::Pixels(point(px(-200. * scale), px(0.))),
                cx,
            );
            let after = window.find("lighting-product-banner").bounds();
            assert!(after.left() < before.left());
            assert_eq!(after.size, before.size);
        })
        .unwrap();
    }
}
