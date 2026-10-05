//! Receipts for the audited `QP` dual-range control on gamepad trigger panels.
//!
//! The source (`JP`/`QP` in `.ref/devices/2629/static/js/main.000315e2.js`) labels
//! the analog branch with `<SIDE>_TRIGGER_RANGE` and the digital branch with
//! `<SIDE>_ACTUATION_POINT`, titles the panel `<SIDE>_TRIGGER_MODE`, and clamps the
//! two handles so they can never cross:
//! `Math.min(value, max - 1)` / `Math.max(value, min + 1)`.
//!
//! The tests are compile-checked only: running them is forbidden in this workspace.
use super::{RangeHandle, power_saving_label, range_handle_value, trigger_keys};

#[test]
fn power_saving_labels_follow_the_source_rule() {
    // Source `GR`: `item.value >= 60 ? ra.pHP : ra.yvH` with
    // `data.value = item.value >= 60 ? item.value / 60 : item.value`; the two aliases
    // resolve to `MIN` ("{{value}} min.") and `SEC` ("{{value}} sec."). The mapping is
    // asserted against the same templates so the test does not depend on the active
    // locale; the English rendering is pinned separately below.
    assert_eq!(power_saving_label(30), crate::i18n::t_value("SEC", 30));
    assert_eq!(power_saving_label(59), crate::i18n::t_value("SEC", 59));
    assert_eq!(power_saving_label(60), crate::i18n::t_value("MIN", 1));
    assert_eq!(power_saving_label(300), crate::i18n::t_value("MIN", 5));
    assert_eq!(power_saving_label(3600), crate::i18n::t_value("MIN", 60));
    if crate::i18n::locale().starts_with("en") {
        assert_eq!(power_saving_label(30), "30 sec.");
        assert_eq!(power_saving_label(300), "5 min.");
    }
    for value in [5, 15, 30, 45, 60, 300] {
        let label = power_saving_label(value);
        assert!(!label.contains("{{value}}"), "{label}");
        assert_ne!(label, "MIN");
        assert_ne!(label, "SEC");
        assert!(!label.contains("MINUTES"));
    }
}

#[test]
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
            crate::i18n::has(key),
            "{key} missing from the bundled locales"
        );
    }
    // The bundled locales keep the `{{value}}` placeholder the formatter replaces.
    for key in ["MIN", "SEC"] {
        assert!(crate::i18n::t(key).contains("{{value}}"), "{key}");
    }
}

#[test]
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

#[test]
fn trigger_keys_exist_in_every_locale() {
    for prefix in ["LEFT", "RIGHT"] {
        let (mode, range, point) = trigger_keys(prefix);
        for key in [mode, range, point] {
            assert!(
                crate::i18n::has(key),
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
            crate::i18n::has(key),
            "{key} missing from the bundled locales"
        );
    }
}

#[test]
fn start_handle_cannot_reach_the_end_handle() {
    // Dragging the start past the end stops one step short.
    assert_eq!(range_handle_value(RangeHandle::Start, 60, 30, 60), 59);
    assert_eq!(range_handle_value(RangeHandle::Start, 99, 30, 60), 59);
    assert_eq!(range_handle_value(RangeHandle::Start, 45, 30, 60), 45);
    // And it stays inside 0..=99.
    assert_eq!(range_handle_value(RangeHandle::Start, -5, 30, 60), 0);
    assert_eq!(range_handle_value(RangeHandle::Start, 100, 30, 100), 99);
}

#[test]
fn end_handle_cannot_reach_the_start_handle() {
    assert_eq!(range_handle_value(RangeHandle::End, 20, 20, 60), 21);
    assert_eq!(range_handle_value(RangeHandle::End, 0, 20, 60), 21);
    assert_eq!(range_handle_value(RangeHandle::End, 80, 20, 60), 80);
    // And it stays inside 1..=100.
    assert_eq!(range_handle_value(RangeHandle::End, 0, 0, 60), 1);
    assert_eq!(range_handle_value(RangeHandle::End, 140, 20, 60), 100);
}

#[test]
fn both_handles_stay_ordered_for_every_neighbouring_pair() {
    for start in 0..100 {
        let end = start + 1;
        assert!(range_handle_value(RangeHandle::Start, start + 1, start, end) < end);
        assert!(range_handle_value(RangeHandle::End, start, start, end) > start);
    }
}
