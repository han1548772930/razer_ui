use crate::features::DeviceWorkspace;
use crate::nav::Tab;
use gpui_kit::component::{Root, Theme};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, ClipboardItem, TestAppContext, px, size};
use razer_model::model::DeviceCategory;
use std::time::Duration;

#[::core::prelude::v1::test]
fn help_remains_a_device_destination() {
    assert_eq!(Tab::from_arg("help"), Some(Tab::Help));
    assert!(
        [182, 653, 777]
            .iter()
            .all(|pid| !Tab::for_product(*pid).contains(&Tab::Help))
    );
}

#[gpui_kit::test]
fn help_reflows_at_source_breakpoint_and_preserves_the_device(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    for (pid, scale) in [(182, 1.), (653, 1.25), (777, 1.)] {
        cx.update(|cx| Theme::update(cx, |theme| theme.font_size = px(16. * scale)));
        let mut workspace = None;
        let handle = cx.open_window(size(px(1280. * scale), px(1100. * scale)), |window, cx| {
            let mut device = crate::test_devices::keyboard();
            device.product_id = pid;
            device.category = match pid {
                182 => DeviceCategory::Mouse,
                777 => DeviceCategory::Headset,
                _ => DeviceCategory::Keyboard,
            };
            device.serial_number = format!("source-serial-{pid}");
            device.firmware_info.current_fw_version = "1.02.03".into();
            let view = cx.new(|cx| DeviceWorkspace::new(device, true, window, cx));
            workspace = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = workspace.unwrap();
        let (identity, profile, settings) = cx.update(|cx| {
            let state = view.read(cx);
            (
                state.identity(),
                state.device().active_profile.clone(),
                state.settings().clone(),
            )
        });
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("profile-select").is_some());
            window.click("device-help", cx);
        })
        .unwrap();
        cx.run_until_parked();
        for (width, stacked) in [(1280., false), (1279., true), (700., true), (1280., false)] {
            cx.simulate_window_resize(handle.into(), size(px(width * scale), px(1100. * scale)));
            cx.update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                assert!(window.try_find("profile-select").is_none());
                assert_eq!(window.find("device-help").selected(), Some(true));
                assert!(window.try_find("help-device-support").is_none());
                assert!(window.try_find("help-master-guide").is_none());
                assert!(window.try_find("help-register-product").is_none());
                let serial = window.find("help-serial").bounds();
                let reset = window.find("help-factory-reset").bounds();
                assert_eq!(
                    window.find("help-copy-serial").bounds().size.height,
                    px(27. * scale)
                );
                assert!(reset.size.width >= px(100. * scale));
                if stacked {
                    assert!((serial.left() - reset.left()).abs() <= px(1.));
                    assert!(serial.top() > reset.bottom());
                } else {
                    assert!((serial.left() - reset.left() - px(620. * scale)).abs() <= px(1.));
                    assert!(serial.top() < reset.top());
                }
            })
            .unwrap();
            cx.update(|cx| {
                let state = view.read(cx);
                assert_eq!(state.page, Tab::Help);
                assert_eq!(state.identity(), identity);
                assert_eq!(state.device().active_profile, profile);
                assert_eq!(state.settings(), &settings);
            });
        }
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("help-factory-reset", cx);
            let tab = if pid == 777 {
                "device-tab-sound"
            } else {
                "device-tab-customize"
            };
            window.click(tab, cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("profile-select").is_some());
            assert!(window.try_find("help-copy-serial").is_none());
        })
        .unwrap();
        cx.update(|cx| {
            assert_eq!(view.read(cx).page, Tab::for_product(pid)[0]);
            assert_eq!(view.read(cx).settings(), &settings);
        });
    }
}

#[gpui_kit::test]
fn copy_serial_uses_real_value_and_reenables_after_two_seconds(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let mut workspace = None;
    let handle = cx.open_window(size(px(1280.), px(1000.)), |window, cx| {
        let mut device = crate::test_devices::keyboard();
        device.serial_number = "CN-653-REAL-SERIAL".into();
        let view = cx.new(|cx| DeviceWorkspace::new(device, true, window, cx));
        view.update(cx, |view, cx| view.set_page(Tab::Help, window, cx));
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = workspace.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("help-copy-serial", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(
        cx.read_from_clipboard()
            .and_then(|item| item.text())
            .as_deref(),
        Some("CN-653-REAL-SERIAL")
    );
    cx.update(|cx| assert!(view.read(cx).help.copied_serial));
    cx.write_to_clipboard(ClipboardItem::new_string("unchanged".into()));
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("help-copy-serial", cx)
    })
    .unwrap();
    assert_eq!(
        cx.read_from_clipboard()
            .and_then(|item| item.text())
            .as_deref(),
        Some("unchanged")
    );
    cx.executor().advance_clock(Duration::from_millis(1999));
    cx.run_until_parked();
    cx.update(|cx| assert!(view.read(cx).help.copied_serial));
    cx.executor().advance_clock(Duration::from_millis(1));
    cx.run_until_parked();
    cx.update(|cx| assert!(!view.read(cx).help.copied_serial));
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("help-copy-serial", cx)
    })
    .unwrap();
    assert_eq!(
        cx.read_from_clipboard()
            .and_then(|item| item.text())
            .as_deref(),
        Some("CN-653-REAL-SERIAL")
    );
}
