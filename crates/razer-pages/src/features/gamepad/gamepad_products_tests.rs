//! Receipts for the audited `QP` dual-range control on gamepad trigger panels.
//!
//! Current PID2629 `main.000315e2.js` (`JP`/`QP`) and `main.9f807a49.css` label
//! the analog branch with `<SIDE>_TRIGGER_RANGE` and the digital branch with
//! `<SIDE>_ACTUATION_POINT`, titles the panel `<SIDE>_TRIGGER_MODE`, and clamps the
//! two handles so they can never cross:
//! `Math.min(value, max - 1)` / `Math.max(value, min + 1)`.
//!
//! Pure state and style assertions do not create a window or access a device.
use super::{RangeHandle, power_saving_label, range_handle_value, range_tip_label, trigger_keys};
use gpui_kit::{Styled, WhiteSpace};

#[::core::prelude::v1::test]
fn trigger_range_tip_preserves_intrinsic_single_line_width_inside_zero_width_anchor() {
    for value in [0, 1, 9, 10, 50, 99, 100] {
        let mut label = range_tip_label(value);
        assert_eq!(label.style().flex_shrink, Some(0.));
        assert_eq!(label.style().text.white_space, Some(WhiteSpace::Nowrap));
    }
}

#[::core::prelude::v1::test]
fn power_saving_labels_follow_the_mounted_controller_minute_options() {
    // Current Am/wm print the number; their adjacent description supplies minutes.
    for (value, label) in [(5, "5"), (15, "15"), (30, "30"), (45, "45"), (60, "60")] {
        assert_eq!(power_saving_label(value), label);
    }
}

#[::core::prelude::v1::test]
fn power_saving_unit_keys_exist_in_every_locale() {
    for key in [
        "MIN",
        "SEC",
        "POWER_SAVING_HEADER",
        "POWER_SAVING_TOOLTIP",
        "CONTROLLER_POWER_SAVING_DESC",
        "SWITCH_OFF_LIGHTING_HEADER",
        "SWITCH_OFF_LIGHTING_TOOLTIP",
        "DISPLAY_TURNED_OFF",
        "IDLE_FOR_MIN",
        "BRIGHTNESS_TOOLTIP",
    ] {
        assert!(
            razer_i18n::has(key),
            "{key} missing from the bundled locales"
        );
    }
    // The bundled locales keep the `{{value}}` placeholder the formatter replaces.
    for key in ["MIN", "SEC"] {
        assert!(razer_i18n::t(key).contains("{{value}}"), "{key}");
    }
}

#[::core::prelude::v1::test]
fn trigger_panels_use_the_source_keys() {
    // `we.LST`/`we.gzX`, `we.Vwf`/`we.ukk`, `we.C7E`/`we.Fh7`.
    assert_eq!(
        trigger_keys("LEFT"),
        (
            "LEFT_TRIGGER_MODE",
            "LEFT_TRIGGER_RANGE",
            "LEFT_ACTUATION_POINT"
        )
    );
    assert_eq!(
        trigger_keys("RIGHT"),
        (
            "RIGHT_TRIGGER_MODE",
            "RIGHT_TRIGGER_RANGE",
            "RIGHT_ACTUATION_POINT"
        )
    );
    // Neither branch may fall back to the invented keys the page used before.
    for prefix in ["LEFT", "RIGHT"] {
        for key in {
            let (mode, range, point) = trigger_keys(prefix);
            [mode, range, point]
        } {
            assert_ne!(key, "MINIMUM");
            assert_ne!(key, "MAXIMUM");
            assert_ne!(key, "ACTUATION_POINT");
        }
    }
}

#[::core::prelude::v1::test]
fn trigger_keys_exist_in_every_locale() {
    for prefix in ["LEFT", "RIGHT"] {
        let (mode, range, point) = trigger_keys(prefix);
        for key in [mode, range, point] {
            assert!(
                razer_i18n::has(key),
                "{key} missing from the bundled locales"
            );
        }
    }
    // The analog/digital radio labels come from the shared keys.
    for key in [
        "ANALOG",
        "DIGITAL",
        "RAPID_TRIGGER",
        "RESET",
        "ACTUATION_DESC",
    ] {
        assert!(
            razer_i18n::has(key),
            "{key} missing from the bundled locales"
        );
    }
}

#[::core::prelude::v1::test]
fn start_handle_cannot_reach_the_end_handle() {
    // Dragging the start past the end stops one step short.
    assert_eq!(range_handle_value(RangeHandle::Start, 60, 30, 60), 59);
    assert_eq!(range_handle_value(RangeHandle::Start, 99, 30, 60), 59);
    assert_eq!(range_handle_value(RangeHandle::Start, 45, 30, 60), 45);
    // And it stays inside 0..=99.
    assert_eq!(range_handle_value(RangeHandle::Start, -5, 30, 60), 0);
    assert_eq!(range_handle_value(RangeHandle::Start, 100, 30, 100), 99);
}

#[::core::prelude::v1::test]
fn end_handle_cannot_reach_the_start_handle() {
    assert_eq!(range_handle_value(RangeHandle::End, 20, 20, 60), 21);
    assert_eq!(range_handle_value(RangeHandle::End, 0, 20, 60), 21);
    assert_eq!(range_handle_value(RangeHandle::End, 80, 20, 60), 80);
    // And it stays inside 1..=100.
    assert_eq!(range_handle_value(RangeHandle::End, 0, 0, 60), 1);
    assert_eq!(range_handle_value(RangeHandle::End, 140, 20, 60), 100);
}

#[::core::prelude::v1::test]
fn both_handles_stay_ordered_for_every_neighbouring_pair() {
    for start in 0..100 {
        let end = start + 1;
        assert!(range_handle_value(RangeHandle::Start, start + 1, start, end) < end);
        assert!(range_handle_value(RangeHandle::End, start, start, end) > start);
    }
}
