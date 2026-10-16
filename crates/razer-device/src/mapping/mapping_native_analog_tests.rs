use super::*;
use serde_json::json;

fn joystick(
    resolution: AnalogResolution,
    bank: JoystickBank,
    axes: [i32; 3],
) -> NativeAnalogOutput {
    let end = match resolution {
        AnalogResolution::V1 => 255,
        AnalogResolution::V2 => 65535,
    };
    NativeAnalogOutput::Joystick(
        JoystickCurve::compile(
            resolution,
            bank,
            &[
                JoystickPoint {
                    actuation: 0,
                    axes: [0; 3],
                },
                JoystickPoint {
                    actuation: end,
                    axes,
                },
            ],
        )
        .unwrap(),
    )
}

fn gamepad(resolution: AnalogResolution, axis: GamepadAxis, value: i32) -> NativeAnalogOutput {
    let end = match resolution {
        AnalogResolution::V1 => 255,
        AnalogResolution::V2 => 65535,
    };
    NativeAnalogOutput::Gamepad(
        GamepadCurve::compile(
            resolution,
            axis,
            &[
                GamepadPoint {
                    actuation: 0,
                    value: 0,
                },
                GamepadPoint {
                    actuation: end,
                    value,
                },
            ],
        )
        .unwrap(),
    )
}

#[test]
fn native_joystick_rounds_the_delta_before_subtracting_from_the_right_end() {
    let curve = JoystickCurve::compile(
        AnalogResolution::V1,
        JoystickBank::Axes,
        &[JoystickPoint {
            actuation: 3,
            axes: [2, -2, 127],
        }],
    )
    .unwrap();
    // Fixed points distinguish the native instruction order from rounding the
    // interpolated output, which would incorrectly produce zero at index one.
    assert_eq!(
        &curve.table()[..4],
        &[[0, 0, 0], [1, -1, 43], [2, -2, 85], [2, -2, 127]]
    );
}

#[test]
fn xbox_truncates_after_float_addition_and_keeps_a_different_fractional_curve() {
    let curve = GamepadCurve::compile(
        AnalogResolution::V1,
        GamepadAxis::LeftThumbX,
        &[GamepadPoint {
            actuation: 3,
            value: 2,
        }],
    )
    .unwrap();
    assert_eq!(&curve.table()[..4], &[0, 0, 1, 2]);
    let negative = GamepadCurve::compile(
        AnalogResolution::V1,
        GamepadAxis::LeftThumbY,
        &[GamepadPoint {
            actuation: 3,
            value: -2,
        }],
    )
    .unwrap();
    assert_eq!(&negative.table()[..4], &[0, 0, -1, -2]);
}

#[test]
fn source_endpoint_ranges_compile_identical_v1_and_v2_tables() {
    let NativeAnalogOutput::Joystick(v1) =
        joystick(AnalogResolution::V1, JoystickBank::Axes, [127, -128, 63])
    else {
        panic!()
    };
    let NativeAnalogOutput::Joystick(v2) =
        joystick(AnalogResolution::V2, JoystickBank::Axes, [127, -128, 63])
    else {
        panic!()
    };
    assert_eq!(v1.table(), v2.table());
    assert_eq!(v1.evaluate(255), [127, -128, 63]);
    assert_eq!(v2.evaluate(65535), [127, -128, 63]);
    let NativeAnalogOutput::Gamepad(g1) =
        gamepad(AnalogResolution::V1, GamepadAxis::RightThumbX, 32767)
    else {
        panic!()
    };
    let NativeAnalogOutput::Gamepad(g2) =
        gamepad(AnalogResolution::V2, GamepadAxis::RightThumbX, 32767)
    else {
        panic!()
    };
    assert_eq!(g1.table(), g2.table());
    assert_eq!(g2.evaluate(65535), 32767);
}

#[test]
fn joystick_native_tail_stops_at_254_while_xbox_tail_includes_255() {
    let joy = JoystickCurve::compile(
        AnalogResolution::V1,
        JoystickBank::RotationAxes,
        &[JoystickPoint {
            actuation: 64,
            axes: [127, -128, 20],
        }],
    )
    .unwrap();
    assert_eq!(joy.evaluate(254), [127, -128, 20]);
    assert_eq!(joy.evaluate(255), [0; 3]);
    let xbox = GamepadCurve::compile(
        AnalogResolution::V1,
        GamepadAxis::LeftTrigger,
        &[GamepadPoint {
            actuation: 64,
            value: 255,
        }],
    )
    .unwrap();
    assert_eq!(xbox.evaluate(254), 255);
    assert_eq!(xbox.evaluate(255), 255);
}

#[test]
fn lone_zero_point_preserves_the_two_native_initialization_paths() {
    let joy = JoystickCurve::compile(
        AnalogResolution::V1,
        JoystickBank::Axes,
        &[JoystickPoint {
            actuation: 0,
            axes: [7, -8, 9],
        }],
    )
    .unwrap();
    assert_eq!(joy.evaluate(0), [7, -8, 9]);
    assert_eq!(joy.evaluate(254), [7, -8, 9]);
    assert_eq!(joy.evaluate(255), [0; 3]);
    let xbox = GamepadCurve::compile(
        AnalogResolution::V1,
        GamepadAxis::RightTrigger,
        &[GamepadPoint {
            actuation: 0,
            value: 99,
        }],
    )
    .unwrap();
    assert_eq!(xbox.evaluate(0), 0);
    assert_eq!(xbox.evaluate(1), 99);
    assert_eq!(xbox.evaluate(255), 99);
}

#[test]
fn duplicate_v2_buckets_keep_first_native_dictionary_entry() {
    let output = NativeAnalogOutput::from_native_json(&json!({
        "type":"analogJoystickV2", "id":"axes",
        "actuations":{"768":[90,90,90], "1000":[3,3,3], "65535":[127,127,127]}
    }))
    .unwrap();
    let NativeAnalogOutput::Joystick(curve) = output else {
        panic!()
    };
    // Chromium's string-sorted Dict visits "1000" before "768". Both become
    // numeric bucket 3. Native emplace keeps the first inserted value.
    assert_eq!(curve.evaluate(768), [3; 3]);
    assert_eq!(curve.evaluate(1000), [3; 3]);
    assert_eq!(curve.evaluate(65535), [127; 3]);
    let typed = JoystickCurve::compile(
        AnalogResolution::V2,
        JoystickBank::Axes,
        &[
            JoystickPoint {
                actuation: 1000,
                axes: [5; 3],
            },
            JoystickPoint {
                actuation: 768,
                axes: [50; 3],
            },
        ],
    )
    .unwrap();
    assert_eq!(typed.evaluate(768), [5; 3]);
}

#[test]
fn event_resolution_uses_the_correct_byte_without_cross_generation_scaling() {
    let NativeAnalogOutput::Joystick(v1) =
        joystick(AnalogResolution::V1, JoystickBank::Axes, [127; 3])
    else {
        panic!()
    };
    let NativeAnalogOutput::Joystick(v2) =
        joystick(AnalogResolution::V2, JoystickBank::Axes, [127; 3])
    else {
        panic!()
    };
    assert_eq!(v1.evaluate(0x7f00), v1.evaluate(0));
    assert_eq!(v2.evaluate(0x7f00), v1.evaluate(127));
    assert_eq!(v2.evaluate(0x7fff), v2.evaluate(0x7f00));
    assert_eq!(v1.evaluate(0x00ff), [127; 3]);
    assert_eq!(v2.evaluate(0x00ff), [0; 3]);
}

#[test]
fn native_lookup_data_preserves_signed_axis_values_and_word_endianness() {
    let NativeAnalogOutput::Joystick(joy) = joystick(
        AnalogResolution::V1,
        JoystickBank::RotationAxes,
        [-128, 127, -1],
    ) else {
        panic!()
    };
    assert_eq!(&joy.lookup_bytes()[765..], &[0x80, 0x7f, 0xff]);
    let NativeAnalogOutput::Gamepad(xbox) =
        gamepad(AnalogResolution::V2, GamepadAxis::RightThumbY, -32768)
    else {
        panic!()
    };
    assert_eq!(&xbox.lookup_bytes()[510..], &[0x00, 0x80]);
}

#[test]
fn int32_joystick_values_clamp_but_xbox_values_narrow_before_trigger_clamp() {
    let curve = JoystickCurve::compile(
        AnalogResolution::V1,
        JoystickBank::Axes,
        &[JoystickPoint {
            actuation: 255,
            axes: [i32::MIN, i32::MAX, -127],
        }],
    )
    .unwrap();
    assert_eq!(curve.evaluate(255), [-128, 127, -127]);
    for (input, trigger, thumb) in [
        (32768, 0, -32768),
        (65535, 0, -1),
        (65536, 0, 0),
        (300, 255, 300),
    ] {
        let t = GamepadCurve::compile(
            AnalogResolution::V1,
            GamepadAxis::LeftTrigger,
            &[GamepadPoint {
                actuation: 255,
                value: input,
            }],
        )
        .unwrap();
        let x = GamepadCurve::compile(
            AnalogResolution::V1,
            GamepadAxis::LeftThumbX,
            &[GamepadPoint {
                actuation: 255,
                value: input,
            }],
        )
        .unwrap();
        assert_eq!(t.evaluate(255), trigger);
        assert_eq!(x.evaluate(255), thumb);
    }
}

#[test]
fn native_actuation_key_parser_accepts_only_the_recovered_uint32_syntax() {
    for (text, value) in [("0", 0), ("+1", 1), ("00127", 127), ("65535", 65535)] {
        assert_eq!(native_actuation_key(text).unwrap(), value);
    }
    for text in [
        "",
        "+",
        "-0",
        "-1",
        " 1",
        "1 ",
        "1x",
        "1.0",
        "1e2",
        "65536",
        "4294967296",
        "９",
    ] {
        assert!(
            native_actuation_key(text).is_err(),
            "Unexpected native key {text:?}"
        );
    }
    assert!(
        JoystickCurve::compile(
            AnalogResolution::V1,
            JoystickBank::Axes,
            &[JoystickPoint {
                actuation: 256,
                axes: [0; 3]
            },]
        )
        .is_err()
    );
    assert!(GamepadCurve::compile(AnalogResolution::V1, GamepadAxis::LeftTrigger, &[]).is_err());
}

#[test]
fn native_json_rejects_missing_ids_wrong_types_truncation_and_fractional_values() {
    let valid = json!({"type":"analogJoystick", "id":"axes", "actuations":{"0":[0,0,0], "255":[127,-128,0]}});
    assert!(NativeAnalogOutput::from_native_json(&valid).is_ok());
    for replacement in [
        json!([1, 2]),
        json!([1, 2, 3, 4]),
        json!([1, 2, null]),
        json!([1, 2, 3.0]),
        json!([1, 2, 2147483648u64]),
    ] {
        let mut broken = valid.clone();
        broken["actuations"]["255"] = replacement;
        assert!(NativeAnalogOutput::from_native_json(&broken).is_err());
    }
    for broken in [
        Value::Null,
        json!({}),
        json!({"type":"joystick", "id":"axes", "actuations":{"255":[1,2,3]}}),
        json!({"type":"analogJoystick", "id":"AXES", "actuations":{"255":[1,2,3]}}),
        json!({"type":"analogJoystick", "id":"axes", "actuations":{}}),
        json!({"type":"analogXboxGamepad", "id":"leftTrigger", "actuations":{"255":12.5}}),
    ] {
        assert!(NativeAnalogOutput::from_native_json(&broken).is_err());
    }
}

#[test]
fn all_four_native_types_resolve_both_joystick_banks_and_six_gamepad_axes() {
    for kind in ["analogJoystick", "analogJoystickV2"] {
        for id in ["axes", "raxes"] {
            assert!(
                NativeAnalogOutput::from_native_json(
                    &json!({"type":kind, "id":id, "actuations":{"0":[0,0,0], "255":[1,2,3]}})
                )
                .is_ok()
            );
        }
    }
    for kind in ["analogXboxGamepad", "analogXboxGamepadV2"] {
        for id in [
            "leftTrigger",
            "rightTrigger",
            "leftThumbX",
            "leftThumbY",
            "rightThumbX",
            "rightThumbY",
        ] {
            assert!(
                NativeAnalogOutput::from_native_json(
                    &json!({"type":kind, "id":id, "actuations":{"0":0, "255":99}})
                )
                .is_ok()
            );
        }
    }
}

#[test]
fn joystick_contributions_choose_opposing_extrema_and_neutralize_full_opposition() {
    let mut state = AnalogMappingState::default();
    let positive = joystick(AnalogResolution::V1, JoystickBank::Axes, [127, 80, 0]);
    let lesser_positive = joystick(AnalogResolution::V1, JoystickBank::Axes, [70, 30, 0]);
    let negative = joystick(AnalogResolution::V1, JoystickBank::Axes, [-128, -20, 0]);
    assert_eq!(state.apply("positive", &positive, 255).axes, [127, 80, 0]);
    assert_eq!(
        state.apply("also-positive", &lesser_positive, 255).axes,
        [127, 80, 0]
    );
    assert_eq!(state.apply("negative", &negative, 255).axes, [0, 60, 0]);
    assert_eq!(state.remove_input("positive").axes, [-58, 10, 0]);
    assert_eq!(state.apply("negative", &negative, 0).axes, [70, 30, 0]);
}

#[test]
fn contribution_replacement_release_and_rotation_bank_do_not_latch() {
    let mut state = AnalogMappingState::default();
    let base = joystick(AnalogResolution::V2, JoystickBank::Axes, [127; 3]);
    let rotation = joystick(AnalogResolution::V2, JoystickBank::RotationAxes, [-128; 3]);
    state.apply("same-key", &base, 65535);
    let output = state.apply("same-key", &rotation, 65535);
    assert_eq!(output.axes, [127; 3]);
    assert_eq!(output.rotation_axes, [-128; 3]);
    let output = state.apply("same-key", &base, 0);
    assert_eq!(output.axes, [0; 3]);
    assert_eq!(output.rotation_axes, [-128; 3]);
    assert_eq!(state.remove_input("same-key"), AnalogOutputState::default());
    state.apply("again", &rotation, 65535);
    assert_eq!(state.clear(), AnalogOutputState::default());
    assert_eq!(state.clear(), AnalogOutputState::default());
}

#[test]
fn gamepad_triggers_take_the_maximum_and_thumb_axes_preserve_opposing_inputs() {
    let mut state = AnalogMappingState::default();
    let left_trigger = gamepad(AnalogResolution::V1, GamepadAxis::LeftTrigger, 200);
    let lower_trigger = gamepad(AnalogResolution::V1, GamepadAxis::LeftTrigger, 50);
    let right_trigger = gamepad(AnalogResolution::V1, GamepadAxis::RightTrigger, 90);
    state.apply("lt-max", &left_trigger, 255);
    state.apply("lt-small", &lower_trigger, 255);
    assert_eq!(state.apply("rt", &right_trigger, 255).left_trigger, 200);
    assert_eq!(state.output().right_trigger, 90);
    assert_eq!(state.remove_input("lt-max").left_trigger, 50);
    let right = gamepad(AnalogResolution::V2, GamepadAxis::LeftThumbX, 32767);
    let left = gamepad(AnalogResolution::V2, GamepadAxis::LeftThumbX, -32768);
    state.apply("right", &right, 65535);
    assert_eq!(state.apply("left", &left, 65535).left_thumb[0], 0);
    assert_eq!(state.remove_input("right").left_thumb[0], -32768);
    let up = gamepad(AnalogResolution::V2, GamepadAxis::LeftThumbY, 10000);
    let down = gamepad(AnalogResolution::V2, GamepadAxis::LeftThumbY, -8000);
    state.apply("up", &up, 65535);
    assert_eq!(state.apply("down", &down, 65535).left_thumb, [-32768, 2000]);
    assert_eq!(state.clear(), AnalogOutputState::default());
}
