use super::{ButtonData, GameMode, Peer, PresentationContext, Tone, policy};
use crate::features::keyboard_products::KeyboardProductWorkspace;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext, component::Root, px, size};
use razer_device::keyboard_function_swap::FunctionKeyPrimary;
use serde_json::json;
use std::time::Duration;

fn button(value: serde_json::Value) -> ButtonData {
    serde_json::from_value(value).unwrap()
}
fn context(pid: u32) -> PresentationContext {
    PresentationContext {
        pid,
        policy: policy(pid).unwrap(),
        shift: false,
        primary: None,
        gaming: Some(GameMode {
            state: 0,
            is_game_on: Some(false),
        }),
        peer: Peer::Unobserved,
    }
}

#[test]
fn key_tip_distinguishes_fn_system_mode_and_live_gaming_restrictions() {
    let button = button(
        json!({"buttonKey":"KEY_WINDOWS","inputID":"KEY_WINDOWS","counter":"Windows",
        "assignment":"KEYBOARD","assignmentValue":"Windows","MappingGroup":"KEY_SYSTEM_GROUP",
        "disableHypershiftMapping":true,"isMapped":false}),
    );
    let mut state = context(717);
    state.primary = Some(FunctionKeyPrimary::Multimedia);
    let card = state.card(&button, "Windows".into(), String::new());
    assert_eq!(card.notice.unwrap().text, razer_i18n::t("SECOND_KEY_TIP"));
    state.gaming = Some(GameMode {
        state: 2,
        is_game_on: Some(true),
    });
    let card = state.card(&button, "Windows".into(), String::new());
    assert_eq!(card.border, Tone::Unavailable);
    assert_eq!(
        card.notice.unwrap().text,
        razer_i18n::t("WINKEY_DESC_GAMING_MODE_ON")
    );
    state.shift = true;
    state.primary = Some(FunctionKeyPrimary::Function);
    assert_eq!(
        state
            .card(&button, "Windows".into(), String::new())
            .notice
            .unwrap()
            .text,
        razer_i18n::t("MENUKEY_DESC_GAMING_MODE_ON")
    );
}

#[test]
fn key_tip_does_not_convert_unobserved_peers_into_disconnection() {
    let button = button(
        json!({"buttonKey":"KEY_A","inputID":"KEY_A","counter":"A","isMapped":true,
        "assignment":"INTERDEVICE","assignmentValue":"Next Profile","isRequireSynapse":true,
        "assignmentGroup":{"interPID":182,"name":"Mouse","interDeviceType":"profileNavigationGroup"}}),
    );
    let mut state = context(515);
    let pending = state.card(&button, "Next Profile".into(), String::new());
    assert_eq!(pending.border, Tone::Required);
    assert_eq!(
        pending.notice.unwrap().text,
        razer_i18n::t("MAPPING_REQUIRES_RAZER_SYNAPSE")
    );
    state.peer = Peer::Absent;
    let absent = state.card(&button, "Next Profile".into(), String::new());
    assert_eq!(absent.border, Tone::Unavailable);
    assert_eq!(
        absent.notice.unwrap().text,
        razer_i18n::t("DEVICE_NOT_CONNECTED")
    );
    state.peer = Peer::Present;
    assert_eq!(
        state
            .card(&button, "Next Profile".into(), String::new())
            .peer_name
            .as_deref(),
        Some("Mouse")
    );
}

#[test]
fn key_tip_preserves_the_source_xena_and_analog_label_differences() {
    let dial = button(
        json!({"buttonKey":"DKM_D2","inputID":"DKM_D2","counter":"Dial",
        "assignment":"KEYBOARD","assignmentValue":"Dial","isMapped":false}),
    );
    assert!(
        context(716)
            .card(&dial, "Dial".into(), String::new())
            .notice
            .is_none()
    );
    assert!(
        context(515)
            .card(&dial, "Dial".into(), String::new())
            .notice
            .is_some()
    );
    let key = button(
        json!({"buttonKey":"KEY_A","inputID":"KEY_A","counter":"A","isMapped":true,
        "assignment":"KEYBOARD","assignmentValue":"A","assignmentGroup":{"key":"KEY_A"},
        "secondaryAssignment":"KEYBOARD","secondaryAssignmentValue":"B","secondaryAssignmentGroup":{"key":"KEY_B"}}),
    );
    let older = context(679).card(&key, "A".into(), "B".into());
    let current = context(756).card(&key, "A".into(), "B".into());
    assert_eq!(
        older.kind.as_deref(),
        Some(razer_i18n::t("KEYBOARD_FUNCTION").as_str())
    );
    assert_eq!(current.kind.as_deref(), Some("KEYBOARD"));
    assert_eq!(
        current.secondary.as_ref().map(|(_, value)| value.as_str()),
        Some("B")
    );
}

#[gpui_kit::test]
fn key_tip_fn_hover_tracks_keyboard_selection_and_owner_cancellation(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut page = None;
    let handle = cx.open_window(size(px(1300.), px(1800.)), |window, cx| {
        let view = cx.new(|cx| KeyboardProductWorkspace::new(717, false, window, cx));
        view.update(cx, |page, cx| {
            let mut saved = page.snapshot();
            saved["functionKeyPrimary"] = "multi".into();
            page.restore(Some(&saved), window, cx);
        });
        page = Some(view.clone());
        Root::new(view, window, cx)
    });
    let page = page.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("keyboard-function-primary-multi", cx);
        window.press("shift-tab", cx);
        assert_eq!(
            window.find("keyboard-function-primary-func").focused(),
            Some(true)
        );
        window.hover("keyboard-hit-KEY_F5", cx);
        assert_eq!(
            window.find("keyboard-key-tip-name").label(),
            Some(razer_i18n::t("MUTE_VOLUME").as_str())
        );
        assert_eq!(
            window.find("keyboard-key-tip-notice").label(),
            Some(razer_i18n::t("SECOND_KEY_TIP").as_str())
        );
        let key = window.find("keyboard-hit-KEY_F5").bounds();
        let tip = window.find("keyboard-mapping-key-tip").bounds();
        assert!(tip.size.width > px(0.) && tip.size.height >= px(75.));
        assert!(tip.bottom() <= key.top() || tip.top() >= key.bottom());
        window.press("space", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("keyboard-key-tip-name").label(), Some("F5"));
        assert!(window.try_find("keyboard-key-tip-notice").is_none());
        page.update(cx, |page, _| page.set_mapping_callback_generation(9));
        window.render_frame(cx);
        assert!(window.try_find("keyboard-mapping-key-tip").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn key_tip_uses_the_selected_mapping_layer_and_clears_on_page_leave(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut page = None;
    let handle = cx.open_window(size(px(1300.), px(1800.)), |window, cx| {
        let view = cx.new(|cx| KeyboardProductWorkspace::new(515, false, window, cx));
        view.update(cx, |page, cx| {
            let mut saved = page.snapshot();
            saved["mappings"] = json!([{"inputID":"KEY_A","inputType":"KeyInput","isHyperShift":true,
                "outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_B","modifiers":[]}}]);
            page.restore(Some(&saved), window, cx);
        });
        page = Some(view.clone()); Root::new(view, window, cx)
    });
    let page = page.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.hover("keyboard-hit-KEY_A", cx);
        assert_eq!(window.find("keyboard-key-tip-name").label(), Some("A"));
        window.click("keyboard-hypershift", cx);
        window.hover("keyboard-hit-KEY_A", cx);
        assert_eq!(window.find("keyboard-key-tip-name").label(), Some("B"));
        page.update(cx, |page, cx| page.set_page("TAB_LIGHTING", window, cx));
        window.render_frame(cx);
        assert!(window.try_find("keyboard-mapping-key-tip").is_none());
    })
    .unwrap();
}
