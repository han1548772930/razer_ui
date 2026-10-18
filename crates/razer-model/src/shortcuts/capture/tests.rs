use super::*;
use serde_json::json;

fn event(kind: &str, scan_code: u32, flag: u32) -> Value {
    json!({"deviceInfo":{"productId":756},"eventType":3,"input":{"type":kind,"scancode":scan_code,"flag":flag}})
}

#[test]
fn native_modifiers_keep_sides_hypershift_and_repeat_boundaries() {
    let mut capture = CaptureState::default();
    capture.set_mode(true);
    assert!(
        capture
            .redirected(event("keyboard", 29, 0))
            .unwrap()
            .is_none()
    );
    assert!(
        capture
            .redirected(event("keyboard", 56, 2))
            .unwrap()
            .is_none()
    );
    let input = capture
        .redirected(event("keyboard", 30, 0))
        .unwrap()
        .unwrap();
    assert_eq!(input.input(), "KEY_A");
    assert_eq!(input.modifiers(), ["KEY_LEFT_CTRL", "KEY_RIGHT_ALT"]);
    assert!(input.hypershift());
    assert!(
        capture
            .redirected(event("keyboard", 30, 0))
            .unwrap()
            .is_none()
    );
    assert!(
        capture
            .redirected(event("keyboard", 30, 1))
            .unwrap()
            .is_none()
    );
    capture.redirected(event("keyboard", 29, 1)).unwrap();
    let next = capture
        .redirected(event("analogKey", 48, 0))
        .unwrap()
        .unwrap();
    assert_eq!(next.input(), "KEY_B");
    assert_eq!(next.modifiers(), ["KEY_RIGHT_ALT"]);
}

#[test]
fn source_defaults_and_shift_ctrl_order_are_shared_by_mouse_and_keyboard() {
    let mut capture = CaptureState::default();
    let plain = capture.browser_key_down("KEY_A", &[]).unwrap();
    assert_eq!(plain.modifiers(), ["KEY_LEFT_CTRL", "KEY_LEFT_SHIFT"]);
    capture.browser_key_up("KEY_A");
    assert!(capture.browser_key_down("KEY_LEFT_SHIFT", &[]).is_none());
    assert!(capture.browser_key_down("KEY_LEFT_CTRL", &[]).is_none());
    let mouse = capture.mouse("Button4", &[]).unwrap();
    assert_eq!(mouse.modifiers(), ["KEY_LEFT_CTRL", "KEY_LEFT_SHIFT"]);
    assert!(capture.mouse("LeftClick", &[]).is_none());
    capture.browser_key_up("KEY_RIGHT_CTRL");
    let letter = capture.browser_key_down("KEY_K", &[]).unwrap();
    assert_eq!(letter.modifiers(), ["KEY_LEFT_SHIFT"]);
}

#[test]
fn razer_keys_update_mode_and_unrelated_redirect_types_do_not_record() {
    let mut capture = CaptureState::default();
    for key in [1, 10, 11] {
        for (flag, enabled) in [(0, true), (1, false)] {
            assert!(
                capture
                    .redirected(json!({"deviceInfo":{"productId":756},"eventType":3,
                "input":{"type":"razerKey","key":key,"flag":flag}}))
                    .unwrap()
                    .is_none()
            );
            assert_eq!(
                capture.mouse("RightClick", &[]).unwrap().hypershift(),
                enabled
            );
        }
    }
    assert!(
        capture
            .redirected(json!({"deviceInfo":{"productId":756},"eventType":3,
        "input":{"type":"mouse","id":1}}))
            .unwrap()
            .is_none()
    );
}

#[test]
fn invalid_payload_fields_stay_errors_while_source_exclusions_stay_ignored() {
    let mut capture = CaptureState::default();
    for input in EXCLUDED.iter().copied().chain(["KEY_LEFT_GUI"]) {
        assert!(capture.browser_key_down(input, &[]).is_none());
    }
    for input in [
        json!({"type":"keyboard","flag":0}),
        json!({"type":"analogKey","scancode":30}),
        json!({"type":"keyboard","scancode":"30","flag":0}),
        json!({"type":"razerKey","key":1}),
    ] {
        assert!(
            capture
                .redirected(json!({"deviceInfo":{"productId":756},"eventType":3,"input":input}))
                .is_err()
        );
    }
}

#[test]
fn native_and_browser_callbacks_share_the_same_pressed_key_owner() {
    let mut capture = CaptureState::default();
    assert!(
        capture
            .redirected(event("keyboard", 30, 0))
            .unwrap()
            .is_some()
    );
    assert!(
        capture
            .browser_key_down("KEY_A", &["KEY_LEFT_CTRL".into()])
            .is_none()
    );
    capture.browser_key_up("KEY_A");
    assert!(
        capture
            .redirected(event("keyboard", 30, 0))
            .unwrap()
            .is_some()
    );
}

#[test]
fn dkm_redirects_are_limited_to_the_current_keyboard_product_branch() {
    let raw = |product| {
        json!({"deviceInfo":{"productId":product},"eventType":3,
        "input":{"type":"razerKey","key":252,"flag":0}})
    };
    for product in [706, 707, 708] {
        let captured = CaptureState::default()
            .redirected(raw(product))
            .unwrap()
            .unwrap();
        assert_eq!(captured.input(), "KEY_B");
    }
    assert!(
        CaptureState::default()
            .redirected(raw(756))
            .unwrap()
            .is_none()
    );
}
