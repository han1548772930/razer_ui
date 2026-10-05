//! Receipts for the audited switch-off-lighting widget on source keyboards.
//!
//! The source renders `SWITCH_OFF_LIGHTING_HEADER` / `SWITCH_OFF_LIGHTING_TOOLTIP`
//! as the widget title and tip, `DISPLAY_TURNED_OFF` and `IDLE_FOR_MIN` as the two
//! `.check-item` labels, and a 1–15 minute slider tagged `1`/`15` with no label.
//! Both keys previously used here (`SWITCH_OFF_LIGHTING_WHEN_DISPLAY_IS_OFF`,
//! `SWITCH_OFF_LIGHTING_WHEN_IDLE`) and the `MINUTES` label do not exist in the
//! current sources, so `t()` returned the key text itself.
use super::{SWITCH_OFF_IDLE_MINUTES, SWITCH_OFF_LIGHTING_ITEMS};

#[test]
fn switch_off_lighting_uses_the_source_keys() {
    assert_eq!(
        SWITCH_OFF_LIGHTING_ITEMS,
        [
            ("/switchOffLighting/isDisplayOn", "DISPLAY_TURNED_OFF"),
            ("/switchOffLighting/isIdleEnabled", "IDLE_FOR_MIN"),
        ]
    );
    // `IDLE_FOR_MIN` is the whole label ("When idle for (minutes)"); the slider
    // itself is unlabelled and only carries the `1`/`15` range tags.
    for (_, key) in SWITCH_OFF_LIGHTING_ITEMS {
        assert_ne!(key, "MINUTES");
        assert!(!key.starts_with("SWITCH_OFF_LIGHTING_WHEN"));
    }
}

#[test]
fn idle_minutes_slider_covers_the_source_range() {
    // `<slider min={1} max={15} step={1} minTag="1" maxTag="15" />`.
    assert_eq!(SWITCH_OFF_IDLE_MINUTES, (1, 15));
    let (min, max) = SWITCH_OFF_IDLE_MINUTES;
    assert_eq!(min.to_string(), "1");
    assert_eq!(max.to_string(), "15");
    assert!(min < max);
}

#[test]
fn switch_off_lighting_labels_exist_in_every_locale() {
    // The two labels and the title/tip come from the bundled locales; a missing
    // key would render as the key text itself.
    for key in [
        "SWITCH_OFF_LIGHTING_HEADER",
        "SWITCH_OFF_LIGHTING_TOOLTIP",
        "DISPLAY_TURNED_OFF",
        "IDLE_FOR_MIN",
    ] {
        assert!(
            crate::i18n::has(key),
            "{key} missing from the bundled locales"
        );
    }
}
