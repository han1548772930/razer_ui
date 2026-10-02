use super::OnboardMemoryPanel;
use crate::features::DeviceWorkspace;
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext, px, size};

#[gpui_kit::test]
fn preview_slot_assignment_and_conflict_leave_workspace_untouched(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut entities = None;
    let handle = cx.open_window(size(px(1280.), px(900.)), |window, cx| {
        let mut device = crate::demo::demo_keyboard();
        device.serial_number = "PREVIEW-OBM".into();
        let workspace = cx.new(|cx| DeviceWorkspace::new(device, true, window, cx));
        let panel = cx.new(|cx| OnboardMemoryPanel::new(workspace.downgrade(), window, cx));
        entities = Some((workspace, panel.clone()));
        Root::new(panel, window, cx)
    });
    let (workspace, panel) = entities.unwrap();
    let before = cx.update(|cx| serde_json::to_value(&workspace.read(cx).device).unwrap());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("profile-obm", cx);
        let bounds = window.find("profile-obm-content").bounds();
        assert!((f32::from(bounds.size.width) - 270.).abs() < 1.);
        window.click("onboard-preview-ready", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.click(("onboard-slot-select", 3usize), cx);
        window.press("down", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let snapshot = panel.read(cx).readback.as_ref().unwrap();
        assert!(snapshot.profiles.iter().any(|profile| profile.slot_id == 3));
        assert_eq!(
            serde_json::to_value(&workspace.read(cx).device).unwrap(),
            before
        );
    });
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(
            window.try_find("profile-obm-content").is_some(),
            "selecting a slot must keep the OBM menu open"
        );
        window.click("onboard-preview-conflict", cx);
        window.press("escape", cx);
        assert!(
            window.try_find("onboard-use-keyboard").is_some(),
            "the source conflict requires a choice"
        );
        window.click("onboard-use-keyboard", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("onboard-use-keyboard").is_none());
    })
    .unwrap();
    cx.update(|cx| {
        assert!(!panel.read(cx).readback.as_ref().unwrap().mapping_conflict);
        assert_eq!(
            serde_json::to_value(&workspace.read(cx).device).unwrap(),
            before
        );
        assert!(!workspace.read(cx).dirty());
    });
}

#[gpui_kit::test]
fn live_device_cannot_enter_preview_or_accept_another_devices_readback(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut entities = None;
    let handle = cx.open_window(size(px(1280.), px(900.)), |window, cx| {
        let workspace =
            cx.new(|cx| DeviceWorkspace::new(crate::demo::demo_keyboard(), true, window, cx));
        let panel = cx.new(|cx| OnboardMemoryPanel::new(workspace.downgrade(), window, cx));
        entities = Some((workspace, panel.clone()));
        Root::new(panel, window, cx)
    });
    let (_workspace, panel) = entities.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("profile-obm", cx);
        assert!(window.try_find("onboard-preview-ready").is_none());
        panel.update(cx, |panel, cx| {
            panel.show_preview("ready", window, cx);
            assert!(panel.readback.is_none());
            assert!(
                panel
                    .receive(
                        "another-device",
                        serde_json::json!({"availableMemory":100}),
                        None,
                        window,
                        cx
                    )
                    .is_err()
            );
            assert!(panel.readback.is_none());
            assert!(!panel.can_assign(2, cx));
        });
    })
    .unwrap();
}
