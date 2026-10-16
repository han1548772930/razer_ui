use super::*;
use crate::{
    mapping_native_analog::{
        AnalogResolution, GamepadCurve, GamepadPoint, JoystickCurve, JoystickPoint,
    },
    mapping_native_output::{JoystickAxisGroup, JoystickButton, joystick_pov_code},
};
use serde_json::json;

fn joystick_button(button: u32, flag: u8) -> NativeDigitalOutput {
    NativeDigitalOutput::JoystickButton { button, flag }
}

fn gamepad_buttons(mask: u16, flag: u8) -> NativeDigitalOutput {
    NativeDigitalOutput::GamepadButtons { mask, flag }
}

fn joystick(group: JoystickGroup, value: [i8; 3]) -> NativeDigitalOutput {
    NativeDigitalOutput::JoystickAxes { group, value }
}

fn gamepad(axis: GamepadAxis, value: i16) -> NativeDigitalOutput {
    NativeDigitalOutput::GamepadAxis { axis, value }
}

fn apply(
    state: &mut NativeDigitalMappingState,
    id: &str,
    output: NativeDigitalOutput,
) -> NativeMappingDelta {
    state.apply_digital(id, output).unwrap()
}

fn joystick_curve(
    resolution: AnalogResolution,
    bank: JoystickBank,
    zero: [i32; 3],
    end: [i32; 3],
) -> NativeAnalogOutput {
    let last = if resolution == AnalogResolution::V1 {
        255
    } else {
        65535
    };
    NativeAnalogOutput::Joystick(
        JoystickCurve::compile(
            resolution,
            bank,
            &[
                JoystickPoint {
                    actuation: 0,
                    axes: zero,
                },
                JoystickPoint {
                    actuation: last,
                    axes: end,
                },
            ],
        )
        .unwrap(),
    )
}

fn gamepad_curve(
    resolution: AnalogResolution,
    axis: GamepadAxis,
    zero: i32,
    end: i32,
) -> NativeAnalogOutput {
    let last = if resolution == AnalogResolution::V1 {
        255
    } else {
        65535
    };
    NativeAnalogOutput::Gamepad(
        GamepadCurve::compile(
            resolution,
            axis,
            &[
                GamepadPoint {
                    actuation: 0,
                    value: zero,
                },
                GamepadPoint {
                    actuation: last,
                    value: end,
                },
            ],
        )
        .unwrap(),
    )
}

#[test]
fn json_joystick_parser_preserves_int32_narrowing_and_clamps_xyz() {
    assert_eq!(
        NativeDigitalOutput::from_native_json(&json!({
            "type":"joystick","id":"button","button":-1,"flag":257
        }))
        .unwrap(),
        joystick_button(u32::MAX, 1)
    );
    assert_eq!(
        NativeDigitalOutput::from_native_json(&json!({
            "type":"joystick","id":"button","button":0,"flag":-256
        }))
        .unwrap(),
        joystick_button(0, 0)
    );
    for (id, group) in [
        ("axes", JoystickGroup::Axes),
        ("raxes", JoystickGroup::RotationAxes),
        ("pov", JoystickGroup::Pov),
    ] {
        assert_eq!(
            NativeDigitalOutput::from_native_json(&json!({
                "type":"joystick","id":id,"xyz":[-99999,127,99999]
            }))
            .unwrap(),
            joystick(group, [-128, 127, 127])
        );
    }
}

#[test]
fn json_gamepad_parser_keeps_word_bit_patterns_and_accepts_unhandled_button_flags() {
    assert_eq!(
        NativeDigitalOutput::from_native_json(&json!({
            "type":"xboxGamepad","id":"button","value":-1,"flag":258
        }))
        .unwrap(),
        gamepad_buttons(0xffff, 2)
    );
    assert_eq!(
        NativeDigitalOutput::from_native_json(&json!({
            "type":"xboxGamepad","id":"leftThumbX","value":32768
        }))
        .unwrap(),
        gamepad(GamepadAxis::LeftThumbX, i16::MIN)
    );
    assert_eq!(
        NativeDigitalOutput::from_native_json(&json!({
            "type":"xboxGamepad","id":"rightTrigger","value":65536+200
        }))
        .unwrap(),
        gamepad(GamepadAxis::RightTrigger, 200)
    );
    assert_eq!(
        NativeDigitalOutput::from_native_json(&json!({
            "type":"xboxGamepad","id":"leftTrigger","value":-65536
        }))
        .unwrap(),
        gamepad(GamepadAxis::LeftTrigger, 0)
    );
}

#[test]
fn json_parsers_reject_noninteger_fields_unknown_ids_and_invalid_narrowed_ranges() {
    assert_eq!(
        NativeDigitalOutput::from_native_json(&json!([])),
        Err(NativeDigitalError::ExpectedObject)
    );
    assert_eq!(
        NativeDigitalOutput::from_native_json(
            &json!({"type":"joystick","id":"button","button":1,"flag":258})
        ),
        Err(NativeDigitalError::InvalidJoystickFlag(2))
    );
    assert_eq!(
        NativeDigitalOutput::from_native_json(&json!({"type":"joystick","id":"axes","xyz":[1,2]})),
        Err(NativeDigitalError::ExpectedXyz)
    );
    assert_eq!(
        NativeDigitalOutput::from_native_json(
            &json!({"type":"joystick","id":"axes","xyz":[1,2,3.0]})
        ),
        Err(NativeDigitalError::ExpectedXyz)
    );
    assert_eq!(
        NativeDigitalOutput::from_native_json(
            &json!({"type":"joystick","id":"button","button":2147483648i64,"flag":0})
        ),
        Err(NativeDigitalError::ExpectedInt32("button"))
    );
    assert_eq!(
        NativeDigitalOutput::from_native_json(
            &json!({"type":"xboxGamepad","id":"leftTrigger","value":-1})
        ),
        Err(NativeDigitalError::InvalidTriggerValue(-1))
    );
    assert_eq!(
        NativeDigitalOutput::from_native_json(
            &json!({"type":"xboxGamepad","id":"leftTrigger","value":256})
        ),
        Err(NativeDigitalError::InvalidTriggerValue(256))
    );
    assert_eq!(
        NativeDigitalOutput::from_native_json(
            &json!({"type":"xboxGamepad","id":"leftThumbX","value":1.0})
        ),
        Err(NativeDigitalError::ExpectedInt32("value"))
    );
    assert_eq!(
        NativeDigitalOutput::from_native_json(&json!({"type":"joystick","id":"unsupported"})),
        Err(NativeDigitalError::UnsupportedId("unsupported".into()))
    );
    assert_eq!(
        NativeDigitalOutput::from_native_json(&json!({"type":"unsupported","id":"button"})),
        Err(NativeDigitalError::UnsupportedType("unsupported".into()))
    );
}

#[test]
fn all_24_joystick_buttons_have_counts_and_first_press_last_release_output() {
    let mut state = NativeDigitalMappingState::default();
    for button in 1..=24 {
        let first = apply(&mut state, "first", joystick_button(button, 0));
        assert_eq!(
            first.requests,
            vec![DriverInput::JoystickButton {
                button: JoystickButton::new(button as u8).unwrap(),
                pressed: true
            }]
        );
        assert!(
            apply(&mut state, "second", joystick_button(button, 0))
                .requests
                .is_empty()
        );
        assert_eq!(state.joystick_button_count(button), Some(2));
        assert!(
            apply(&mut state, "first", joystick_button(button, 1))
                .requests
                .is_empty()
        );
        assert_eq!(state.joystick_button_count(button), Some(1));
        assert_eq!(
            apply(&mut state, "second", joystick_button(button, 1)).requests,
            vec![DriverInput::JoystickButton {
                button: JoystickButton::new(button as u8).unwrap(),
                pressed: false
            }]
        );
        assert_eq!(state.joystick_button_count(button), Some(0));
    }
    assert_eq!(state.output(), MappingOutputState::default());
}

#[test]
fn joystick_button_handler_ignores_outside_range_and_unknown_flags() {
    let mut state = NativeDigitalMappingState::default();
    for button in [0, 25, 100, u32::MAX] {
        assert!(
            apply(&mut state, "bad", joystick_button(button, 0))
                .requests
                .is_empty()
        );
        assert_eq!(state.joystick_button_count(button), None);
    }
    for flag in [2, 128, 255] {
        assert!(
            apply(&mut state, "bad", joystick_button(1, flag))
                .requests
                .is_empty()
        );
    }
    assert_eq!(state.output(), MappingOutputState::default());
}

#[test]
fn joystick_counter_overflow_and_unpaired_release_preserve_wrapping_byte_semantics() {
    let mut state = NativeDigitalMappingState::default();
    apply(&mut state, "release without press", joystick_button(1, 1));
    assert_eq!(state.joystick_button_count(1), Some(255));
    assert!(state.output().joystick.buttons[0]);
    let wrapped = apply(
        &mut state,
        "same identity is not deduplicated",
        joystick_button(1, 0),
    );
    assert_eq!(state.joystick_button_count(1), Some(0));
    assert!(!state.output().joystick.buttons[0]);
    assert_eq!(wrapped.requests.len(), 1);
    for _ in 0..255 {
        apply(&mut state, "repeat", joystick_button(2, 0));
    }
    assert_eq!(state.joystick_button_count(2), Some(255));
    apply(&mut state, "repeat", joystick_button(2, 0));
    assert_eq!(state.joystick_button_count(2), Some(0));
    assert!(!state.output().joystick.buttons[1]);
}

#[test]
fn all_14_gamepad_bits_use_independent_counts_and_ignore_two_reserved_bits() {
    let mut state = NativeDigitalMappingState::default();
    apply(&mut state, "first", gamepad_buttons(u16::MAX, 0));
    assert_eq!(state.output().gamepad.buttons, 0xf3ff);
    for bit in GAMEPAD_BUTTON_BITS {
        assert_eq!(state.gamepad_button_count(bit), Some(1));
    }
    assert_eq!(state.gamepad_button_count(0x400), None);
    assert_eq!(state.gamepad_button_count(0x800), None);
    assert_eq!(state.gamepad_button_count(3), None);
    assert!(
        apply(&mut state, "reserved", gamepad_buttons(0x0c00, 0))
            .requests
            .is_empty()
    );
    assert!(
        apply(&mut state, "second", gamepad_buttons(0xf3ff, 0))
            .requests
            .is_empty()
    );
    assert!(
        apply(&mut state, "first", gamepad_buttons(u16::MAX, 1))
            .requests
            .is_empty()
    );
    assert_eq!(state.output().gamepad.buttons, 0xf3ff);
    apply(&mut state, "second", gamepad_buttons(0xf3ff, 1));
    assert_eq!(state.output().gamepad.buttons, 0);
}

#[test]
fn overlapping_gamepad_masks_release_only_bits_whose_last_counter_is_released() {
    let mut state = NativeDigitalMappingState::default();
    apply(&mut state, "first", gamepad_buttons(0x1003, 0));
    apply(&mut state, "second", gamepad_buttons(0x4002, 0));
    apply(&mut state, "first", gamepad_buttons(0x1003, 1));
    assert_eq!(state.output().gamepad.buttons, 0x4002);
    assert_eq!(state.gamepad_button_count(2), Some(1));
    assert_eq!(state.gamepad_button_count(1), Some(0));
    apply(&mut state, "second", gamepad_buttons(0x4002, 1));
    assert_eq!(state.output().gamepad.buttons, 0);
}

#[test]
fn gamepad_counter_wrap_keeps_independent_mask_and_unpaired_release_does_not_set_it() {
    let mut state = NativeDigitalMappingState::default();
    apply(&mut state, "unpaired", gamepad_buttons(1, 1));
    assert_eq!(state.gamepad_button_count(1), Some(255));
    assert_eq!(state.output().gamepad.buttons, 0);
    apply(&mut state, "press", gamepad_buttons(1, 0));
    assert_eq!(state.gamepad_button_count(1), Some(0));
    assert_eq!(state.output().gamepad.buttons, 1);
    apply(&mut state, "release", gamepad_buttons(1, 1));
    assert_eq!(state.gamepad_button_count(1), Some(255));
    assert_eq!(state.output().gamepad.buttons, 1);
    for _ in 0..254 {
        apply(&mut state, "release", gamepad_buttons(1, 1));
    }
    assert_eq!(state.gamepad_button_count(1), Some(1));
    apply(&mut state, "last release", gamepad_buttons(1, 1));
    assert_eq!(state.output().gamepad.buttons, 0);
}

#[test]
fn gamepad_button_flags_outside_zero_one_are_accepted_by_parser_and_ignored_by_handler() {
    let mut state = NativeDigitalMappingState::default();
    for flag in [2, 128, 255] {
        assert!(
            apply(&mut state, "ignored", gamepad_buttons(0xf3ff, flag))
                .requests
                .is_empty()
        );
    }
    assert_eq!(state.gamepad_button_count(1), Some(0));
    assert_eq!(state.output(), MappingOutputState::default());
}

#[test]
fn digital_joystick_axes_take_opposing_extrema_in_each_component_and_replace_input_ids() {
    let mut state = NativeDigitalMappingState::default();
    apply(
        &mut state,
        "strong",
        joystick(JoystickGroup::Axes, [90, 20, -50]),
    );
    apply(
        &mut state,
        "weak",
        joystick(JoystickGroup::Axes, [40, 10, -10]),
    );
    assert_eq!(state.output().joystick.axes, [90, 20, -50]);
    apply(
        &mut state,
        "opposite",
        joystick(JoystickGroup::Axes, [-30, -10, 20]),
    );
    assert_eq!(state.output().joystick.axes, [60, 10, -30]);
    apply(
        &mut state,
        "strong",
        joystick(JoystickGroup::Axes, [10, 5, -5]),
    );
    assert_eq!(state.output().joystick.axes, [10, 0, 10]);
    assert_eq!(state.joystick_contribution_count(JoystickGroup::Axes), 3);
    apply(&mut state, "weak", joystick(JoystickGroup::Axes, [0; 3]));
    assert_eq!(state.output().joystick.axes, [-20, -5, 15]);
    assert_eq!(state.joystick_contribution_count(JoystickGroup::Axes), 2);
}

#[test]
fn joystick_signed_extremes_cancel_exactly_and_three_banks_remain_independent() {
    let mut state = NativeDigitalMappingState::default();
    for group in [
        JoystickGroup::Axes,
        JoystickGroup::RotationAxes,
        JoystickGroup::Pov,
    ] {
        apply(&mut state, "positive", joystick(group, [127, 127, 127]));
        apply(&mut state, "negative", joystick(group, [-128, -128, -128]));
    }
    assert_eq!(state.output(), MappingOutputState::default());
    apply(
        &mut state,
        "positive",
        joystick(JoystickGroup::RotationAxes, [0; 3]),
    );
    assert_eq!(state.output().joystick.rotation_axes, [-128; 3]);
    assert_eq!(state.output().joystick.axes, [0; 3]);
    assert_eq!(state.output().joystick.pov, [0; 3]);
}

#[test]
fn pov_preserves_native_z_for_group_changes_while_driver_direction_uses_xy_signs() {
    let mut state = NativeDigitalMappingState::default();
    apply(
        &mut state,
        "key",
        joystick(JoystickGroup::Pov, [127, -128, 4]),
    );
    assert_eq!(joystick_pov_code(state.output().joystick.pov), 1);
    let delta = apply(
        &mut state,
        "key",
        joystick(JoystickGroup::Pov, [127, -128, 5]),
    );
    assert_eq!(
        delta.requests,
        vec![DriverInput::JoystickPov([127, -128, 5])]
    );
    assert_eq!(
        &delta.encoded_reports()[0].bytes()[4..10],
        &[9, 0, 0, 0, 1, 0]
    );
}

#[test]
fn triggers_use_maximum_contributions_and_zero_removes_only_the_addressed_input() {
    let mut state = NativeDigitalMappingState::default();
    for axis in [GamepadAxis::LeftTrigger, GamepadAxis::RightTrigger] {
        apply(&mut state, "strong", gamepad(axis, 200));
        apply(&mut state, "weak", gamepad(axis, 100));
        apply(&mut state, "strong", gamepad(axis, 75));
        assert_eq!(state.gamepad_contribution_count(axis), 2);
    }
    assert_eq!(state.output().gamepad.left_trigger, 100);
    assert_eq!(state.output().gamepad.right_trigger, 100);
    apply(&mut state, "weak", gamepad(GamepadAxis::LeftTrigger, 0));
    assert_eq!(state.output().gamepad.left_trigger, 75);
    assert_eq!(state.output().gamepad.right_trigger, 100);
}

#[test]
fn invalid_typed_trigger_values_fail_without_mutating_any_state_or_contribution() {
    let mut state = NativeDigitalMappingState::default();
    apply(&mut state, "kept", gamepad(GamepadAxis::LeftTrigger, 200));
    for value in [-32768, -1, 256, 32767] {
        assert_eq!(
            state.apply_digital("kept", gamepad(GamepadAxis::LeftTrigger, value)),
            Err(NativeDigitalError::InvalidTriggerValue(value))
        );
        assert_eq!(state.output().gamepad.left_trigger, 200);
        assert_eq!(
            state.gamepad_contribution_count(GamepadAxis::LeftTrigger),
            1
        );
    }
}

#[test]
fn all_four_thumb_axes_use_opposing_extrema_and_release_restores_remaining_keys() {
    let mut state = NativeDigitalMappingState::default();
    for axis in [
        GamepadAxis::LeftThumbX,
        GamepadAxis::LeftThumbY,
        GamepadAxis::RightThumbX,
        GamepadAxis::RightThumbY,
    ] {
        apply(&mut state, "positive", gamepad(axis, 32767));
        apply(&mut state, "negative", gamepad(axis, -32768));
        apply(&mut state, "weak", gamepad(axis, 20000));
    }
    assert_eq!(state.output().gamepad.left_thumb, [0, 0]);
    assert_eq!(state.output().gamepad.right_thumb, [0, 0]);
    for axis in [
        GamepadAxis::LeftThumbX,
        GamepadAxis::LeftThumbY,
        GamepadAxis::RightThumbX,
        GamepadAxis::RightThumbY,
    ] {
        apply(&mut state, "positive", gamepad(axis, 0));
    }
    assert_eq!(state.output().gamepad.left_thumb, [-12768, -12768]);
    assert_eq!(state.output().gamepad.right_thumb, [-12768, -12768]);
}

#[test]
fn digital_and_v1_v2_analog_joystick_events_share_one_bank_and_identity() {
    for resolution in [AnalogResolution::V1, AnalogResolution::V2] {
        let raw = if resolution == AnalogResolution::V1 {
            255
        } else {
            65535
        };
        let mut state = NativeDigitalMappingState::default();
        let curve = joystick_curve(resolution, JoystickBank::Axes, [0; 3], [90, 20, -50]);
        apply(
            &mut state,
            "digital",
            joystick(JoystickGroup::Axes, [-30, 70, 10]),
        );
        state.apply_analog("analog", &curve, raw);
        assert_eq!(state.output().joystick.axes, [60, 70, -40]);
        state.apply_analog("digital", &curve, raw);
        assert_eq!(state.output().joystick.axes, [90, 20, -50]);
        assert_eq!(state.joystick_contribution_count(JoystickGroup::Axes), 2);
        apply(&mut state, "analog", joystick(JoystickGroup::Axes, [0; 3]));
        assert_eq!(state.joystick_contribution_count(JoystickGroup::Axes), 1);
        assert_eq!(state.output().joystick.axes, [90, 20, -50]);
    }
}

#[test]
fn digital_and_analog_gamepad_contributions_use_max_triggers_and_shared_thumb_extrema() {
    let mut state = NativeDigitalMappingState::default();
    let trigger = gamepad_curve(AnalogResolution::V1, GamepadAxis::LeftTrigger, 0, 200);
    let thumb = gamepad_curve(AnalogResolution::V2, GamepadAxis::RightThumbY, 0, 32767);
    apply(
        &mut state,
        "digital",
        gamepad(GamepadAxis::LeftTrigger, 120),
    );
    apply(
        &mut state,
        "digital",
        gamepad(GamepadAxis::RightThumbY, -32768),
    );
    state.apply_analog("analog", &trigger, 255);
    state.apply_analog("analog", &thumb, 65535);
    assert_eq!(state.output().gamepad.left_trigger, 200);
    assert_eq!(state.output().gamepad.right_thumb[1], 0);
    apply(&mut state, "digital", gamepad(GamepadAxis::RightThumbY, 0));
    assert_eq!(state.output().gamepad.right_thumb[1], 32767);
    state.apply_analog("analog", &trigger, 0);
    assert_eq!(state.output().gamepad.left_trigger, 120);
}

#[test]
fn analog_raw_zero_release_preserves_nonzero_curve_zero_point_until_explicit_removal() {
    let mut state = NativeDigitalMappingState::default();
    let curve = joystick_curve(
        AnalogResolution::V1,
        JoystickBank::RotationAxes,
        [30, 0, 0],
        [90, 0, 0],
    );
    state.apply_analog("curve", &curve, 255);
    state.apply_analog("curve", &curve, 0);
    assert_eq!(state.output().joystick.rotation_axes, [30, 0, 0]);
    assert_eq!(
        state.joystick_contribution_count(JoystickGroup::RotationAxes),
        1
    );
    state.remove_axis_input("curve");
    assert_eq!(state.output().joystick.rotation_axes, [0; 3]);
    assert_eq!(
        state.joystick_contribution_count(JoystickGroup::RotationAxes),
        0
    );
}

#[test]
fn axis_removal_spans_all_banks_but_keeps_other_inputs_and_button_counters() {
    let mut state = NativeDigitalMappingState::default();
    apply(&mut state, "remove", joystick_button(3, 0));
    apply(&mut state, "remove", gamepad_buttons(0x2000, 0));
    for group in [
        JoystickGroup::Axes,
        JoystickGroup::RotationAxes,
        JoystickGroup::Pov,
    ] {
        apply(&mut state, "remove", joystick(group, [70, 0, 0]));
        apply(&mut state, "keep", joystick(group, [40, 0, 0]));
    }
    for axis in [
        GamepadAxis::LeftTrigger,
        GamepadAxis::RightTrigger,
        GamepadAxis::LeftThumbX,
        GamepadAxis::LeftThumbY,
        GamepadAxis::RightThumbX,
        GamepadAxis::RightThumbY,
    ] {
        apply(&mut state, "remove", gamepad(axis, 100));
        apply(&mut state, "keep", gamepad(axis, 50));
    }
    state.remove_axis_input("remove");
    assert_eq!(state.output().joystick.axes, [40, 0, 0]);
    assert_eq!(state.output().joystick.rotation_axes, [40, 0, 0]);
    assert_eq!(state.output().joystick.pov, [40, 0, 0]);
    assert_eq!(state.output().gamepad.left_trigger, 50);
    assert_eq!(state.output().gamepad.right_thumb, [50, 50]);
    assert_eq!(state.joystick_button_count(3), Some(1));
    assert_eq!(state.gamepad_button_count(0x2000), Some(1));
    assert!(state.output().joystick.buttons[2]);
    assert_eq!(state.output().gamepad.buttons, 0x2000);
}

#[test]
fn native_joystick_pooled_reset_clears_counters_and_maps_while_preserving_gamepad() {
    let mut state = NativeDigitalMappingState::default();
    apply(&mut state, "key", joystick_button(1, 0));
    apply(
        &mut state,
        "key",
        joystick(JoystickGroup::Axes, [40, 50, 60]),
    );
    apply(&mut state, "key", gamepad_buttons(0x1000, 0));
    apply(&mut state, "key", gamepad(GamepadAxis::RightThumbX, 500));
    state.native_pooled_reset_joystick();
    assert_eq!(state.output().joystick, Default::default());
    assert_eq!(state.joystick_button_count(1), Some(0));
    assert_eq!(state.joystick_contribution_count(JoystickGroup::Axes), 0);
    assert_eq!(state.output().gamepad.buttons, 0x1000);
    assert_eq!(state.output().gamepad.right_thumb[0], 500);
}

#[test]
fn native_gamepad_pooled_reset_keeps_counters_and_maps_and_recomputes_only_addressed_fields() {
    let mut state = NativeDigitalMappingState::default();
    apply(&mut state, "button", gamepad_buttons(0x1000, 0));
    apply(&mut state, "button", gamepad_buttons(0x1000, 0));
    apply(
        &mut state,
        "trigger",
        gamepad(GamepadAxis::LeftTrigger, 200),
    );
    apply(
        &mut state,
        "thumb",
        gamepad(GamepadAxis::RightThumbX, -1000),
    );
    state.native_pooled_reset_gamepad();
    assert_eq!(state.output().gamepad, Default::default());
    assert_eq!(state.gamepad_button_count(0x1000), Some(2));
    assert_eq!(
        state.gamepad_contribution_count(GamepadAxis::LeftTrigger),
        1
    );
    apply(&mut state, "weak", gamepad(GamepadAxis::LeftTrigger, 100));
    assert_eq!(state.output().gamepad.left_trigger, 200);
    assert_eq!(state.output().gamepad.right_thumb[0], 0);
    assert_eq!(state.output().gamepad.buttons, 0);
    apply(&mut state, "button", gamepad_buttons(0x1000, 1));
    assert_eq!(state.gamepad_button_count(0x1000), Some(1));
    assert_eq!(state.output().gamepad.buttons, 0);
    apply(&mut state, "thumb", gamepad(GamepadAxis::RightThumbX, -500));
    assert_eq!(state.output().gamepad.right_thumb[0], -500);
}

#[test]
fn explicit_clear_clears_all_contributions_and_counters_and_plans_neutral_requests() {
    let mut state = NativeDigitalMappingState::default();
    apply(&mut state, "key", joystick_button(1, 0));
    apply(
        &mut state,
        "key",
        joystick(JoystickGroup::Axes, [127, 0, 0]),
    );
    apply(
        &mut state,
        "key",
        joystick(JoystickGroup::Pov, [0, -128, 0]),
    );
    apply(&mut state, "key", gamepad_buttons(1, 0));
    apply(&mut state, "key", gamepad(GamepadAxis::LeftTrigger, 200));
    let delta = state.clear();
    assert_eq!(delta.desired, MappingOutputState::default());
    assert_eq!(delta.requests.len(), 4);
    assert_eq!(state.gamepad_button_count(1), Some(0));
    assert_eq!(state.joystick_button_count(1), Some(0));
    assert_eq!(
        state.gamepad_contribution_count(GamepadAxis::LeftTrigger),
        0
    );
    assert_eq!(state.joystick_contribution_count(JoystickGroup::Axes), 0);
    assert_eq!(state.joystick_contribution_count(JoystickGroup::Pov), 0);
    assert!(state.clear().requests.is_empty());
}

#[test]
fn state_delta_reaches_existing_driver_encoder_without_submission_or_false_acknowledgement() {
    let mut state = NativeDigitalMappingState::default();
    let delta = apply(
        &mut state,
        "key",
        joystick(JoystickGroup::Axes, [-128, 0, 127]),
    );
    assert_eq!(delta.previous, MappingOutputState::default());
    assert_eq!(
        delta.requests,
        vec![DriverInput::JoystickAxes {
            group: JoystickAxisGroup::Axes,
            axes: [-128, 0, 127]
        }]
    );
    assert_eq!(
        &delta.encoded_reports()[0].bytes()[4..11],
        &[5, 0, 0, 0, 0, 128, 255]
    );
    let delta = apply(&mut state, "key", gamepad(GamepadAxis::LeftThumbX, -32768));
    assert_eq!(delta.requests.len(), 1);
    assert_eq!(
        &delta.encoded_reports()[0].bytes()[4..8],
        &12u32.to_le_bytes()
    );
    assert_eq!(
        &delta.encoded_reports()[0].bytes()[14..16],
        &(-32768i16).to_le_bytes()
    );
}
