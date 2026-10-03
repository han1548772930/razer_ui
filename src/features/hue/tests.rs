use super::{HueWorkspace, Integration, onboarding};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext, px, size};
use serde_json::json;

// Compile-only during this task. AGENTS.md forbids executing UI/tests.
#[test]
fn manual_octets_follow_the_source_parse_int_contract() {
    for (input, expected) in [
        ("", ""),
        ("abc", ""),
        ("01", "1"),
        ("999", "255"),
        ("1a", "1"),
        ("-1", "-1"),
        ("0xF", "15"),
        ("255", "255"),
    ] {
        assert_eq!(onboarding::normalize_octet(input), expected);
    }
}

#[gpui_kit::test]
fn live_scan_is_unavailable_and_saved_observations_cannot_pair_a_bridge(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut view = None;
    let handle = cx.open_window(size(px(1280.), px(850.)), |window, cx| {
        let workspace = cx.new(|cx| HueWorkspace::new(window, cx));
        workspace.update(cx, |workspace, cx| {
            workspace.restore(Some(&json!({
            "isPaired":true,"bridgeEnabled":true,"devices":[{"name":"invented"}],"ip":"1.2.3.4",
            "isChromaEnabled":true,"brightness":{"value":60,"isEnabled":true}
        })),window,cx)
        });
        view = Some(workspace.clone());
        Root::new(workspace, window, cx)
    });
    let view = view.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("hue-scan").disabled(), Some(true));
        window.click("hue-scan", cx);
        let workspace = view.read(cx);
        assert!(!workspace.bridge.is_paired);
        assert!(workspace.bridge.devices.is_empty());
        assert!(!workspace.advanced);
        assert!(workspace.integration == Integration::Init);
        assert!(workspace.last_command.is_none());
        let saved = workspace.snapshot();
        assert_eq!(saved["brightness"]["value"], 60);
        assert!(saved.get("devices").is_none());
        assert!(saved.get("isPaired").is_none());
        assert!(saved.get("isChromaEnabled").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn preview_waits_for_responses_and_retains_light_controls_by_region(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut view = None;
    let handle = cx.open_window(size(px(1400.), px(1100.)), |window, cx| {
        let workspace = cx.new(|cx| HueWorkspace::new(window, cx));
        workspace.update(cx, |workspace, cx| {
            workspace.preview = true;
            workspace.sample("init", window, cx);
        });
        view = Some(workspace.clone());
        Root::new(workspace, window, cx)
    });
    let view = view.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("hue-scan", cx);
        assert!(view.read(cx).integration == Integration::Scanning);
        assert!(!view.read(cx).bridge.is_paired);
        window.click("hue-cancel-scan", cx);
        assert!(view.read(cx).integration == Integration::ScanCancel);
        view.update(cx, |view, cx| view.sample("found", window, cx));
        window.render_frame(cx);
        window.click("hue-pair", cx);
        assert!(view.read(cx).integration == Integration::WaitUserClickPair);
        assert!(!view.read(cx).bridge.is_paired);
        view.update(cx, |view, cx| {
            view.sample("per_light", window, cx);
            let first = view.light_brightness[&17].entity_id();
            view.bridge.devices.reverse();
            view.bridge.devices.push(view.bridge.devices[0].clone());
            view.rebuild_lights(window, cx);
            assert_eq!(view.bridge.counts(), (1, 2, 1));
            assert_eq!(view.light_brightness[&17].entity_id(), first);
            assert_eq!(view.port_brightness(17), 60.);
            assert_eq!(view.port_brightness(42), 50.);
        });
        window.render_frame(cx);
        window.click("hue-bridge-enable", cx);
        assert!(!view.read(cx).bridge.bridge_enabled);
        window.click("hue-bridge-enable", cx);
        window.press("escape", cx);
        assert!(!view.read(cx).bridge.bridge_enabled);
        assert!(view.read(cx).alert.is_none());
    })
    .unwrap();
}
