//! Receipts for the mouse page's localized names.
//!
//! The mouse page used to hardcode Chinese labels. Every replacement is a key the
//! source itself uses: the panel titles and control labels come from the mouse
//! bundles (`.ref/devices/100/static/js/main.1734869e.js` and its page chunks), the
//! input names from the source's assignment labels (`LEFT_CLICK`, `STEP_BACK`, …)
//! and the effect names from the source's effect table
//! (`{we.DU0: Static_Effect, we.xkP: Breathing_Effect, …}`). Each key's bundled
//! zh-CN value is exactly the string that used to be hardcoded, which is what
//! makes the mapping traceable rather than guessed.
//!
//! The tests are compile-checked only: running them is forbidden in this workspace.
use super::{effect_label, input_label, localized_name};

const PAGE_KEYS: [&str; 22] = [
    "SENSITIVITY_HEADER",
    "SENSITIVITY_STAGES",
    "POLLING_RATE_HEADER",
    "POWER_SAVING_HEADER",
    "POWER_SAVING_DESC",
    "LOW_POWER_MODE_HEADER",
    "LOW_POWER_MODE_DESC",
    "LOW_BATTERY_EFFECTS_HEADER",
    "LOW_BATTERY_EFFECTS_DESC",
    "SMARTTRACKING",
    "ENABLEASYMMETRICCUTOFF",
    "LIFTOFFDISTANCE",
    "LANDINGDISTANCE",
    "TRACKINGDISTANCE",
    "MOUSE_MAT_CALIBRATION_HEADER",
    "DYNAMIC_SENSITIVITY",
    "ROTATION",
    "SCROLL_WHEEL_STAGES",
    "SCROLL_WHEEL_STAGES_DESCRIPTION",
    "SCROLL_TENSION",
    "SCROLL_STEPS",
    "TAB_CUSTOMIZE",
];

#[test]
fn every_replaced_mouse_label_exists_in_every_locale() {
    let mut keys: Vec<&str> = PAGE_KEYS.to_vec();
    keys.extend([
        "CLASSIC",
        "NATURAL",
        "JUMP",
        "CUSTOM",
        "SW_STANDARD",
        "SW_DISTINCT",
        "SW_ULTRA_FINE",
        "SW_ADAPTIVE",
        "SW_SMOOTH_SCROLL",
        "SW_CUSTOM",
        "SMART_REEL",
        "SCROLL_ACCELERATION",
        "SCROLL_UP",
        "SCROLL_DOWN",
        "PAIR",
        "DEFAULT",
    ]);
    for key in keys {
        assert!(
            razer_i18n::has(key),
            "{key} missing from the bundled locales"
        );
    }
}

#[test]
fn inputs_map_to_the_source_assignment_keys() {
    assert_eq!(input_label("LeftClick"), Some("LEFT_CLICK"));
    assert_eq!(input_label("RightClick"), Some("RIGHT_CLICK"));
    assert_eq!(input_label("ScrollButton"), Some("SCROLL_CLICK"));
    assert_eq!(input_label("ScrollUp"), Some("SCROLL_UP"));
    assert_eq!(input_label("ScrollDown"), Some("SCROLL_DOWN"));
    assert_eq!(input_label("Button4"), Some("STEP_BACK"));
    assert_eq!(input_label("Button5"), Some("STEP_FORWARD"));
    // Unknown inputs keep their internal name instead of an invented label.
    assert_eq!(input_label("SomeNewInput"), None);
    assert_eq!(
        localized_name(input_label("SomeNewInput"), "SomeNewInput"),
        "SomeNewInput"
    );
    // And the keys behind them are real.
    for key in [
        "LEFT_CLICK",
        "RIGHT_CLICK",
        "SCROLL_CLICK",
        "STEP_BACK",
        "STEP_FORWARD",
    ] {
        assert!(razer_i18n::has(key), "{key}");
    }
}

#[test]
fn effects_map_to_the_source_effect_keys() {
    for (effect, key) in [
        ("Static_Effect", "STATIC"),
        ("Breathing_Effect", "BREATHING"),
        ("Spectrum_Effect", "SPECTRUM_CYCLING"),
        ("Wave_Effect", "WAVE"),
        ("Reactive_Effect", "REACTIVE"),
        ("Ripple_Effect", "RIPPLE"),
        ("Starlight_Effect", "STARLIGHT"),
        ("Fire_Effect", "FIRE"),
        ("Wheel_Effect", "WHEEL"),
        ("Audio_Meter_Effect", "AUDIO_METER"),
        ("Tidal_Effect", "TIDAL"),
        ("Battery_Level_Effect", "BATTERY_LEVEL"),
        ("Lamborghini", "LAMBORGHINI"),
        ("Ambient_Effect", "AMBIENT"),
    ] {
        assert_eq!(effect_label(effect), Some(key), "{effect}");
        assert!(razer_i18n::has(key), "{key}");
    }
    assert_eq!(effect_label("Unknown_Effect"), None);
    assert_eq!(
        localized_name(effect_label("Unknown_Effect"), "Unknown_Effect"),
        "Unknown_Effect"
    );
}

#[test]
fn localized_names_have_no_leftover_chinese_in_the_page_sources() {
    // The page must not spell its own labels any more: every page label now goes
    // through `t(...)`. This guards against re-introducing a hardcoded string.
    let source = include_str!("mouse_products.rs");
    for label in [
        "灵敏度",
        "回报率",
        "智能追踪",
        "非对称中止",
        "抬升距离",
        "着陆距离",
        "追踪距离",
        "表面校准",
        "动态灵敏度",
        "滚轮阶段",
        "滚动张力",
        "滚动刻度",
        "智能滚动",
        "滚动加速",
    ] {
        let quoted = format!("\"{label}\"");
        assert!(
            !source.contains(&quoted),
            "{label} is still a hardcoded page label"
        );
    }
}
