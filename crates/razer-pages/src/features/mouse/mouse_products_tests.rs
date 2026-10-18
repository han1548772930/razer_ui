//! Receipts for the mouse page's localized names.
//!
//! Panel, input and effect labels retain source translation identities;
//! language changes must preserve the selected command or effect.
use super::{effect_label, input_label, localized_name};

#[test]
fn packaged_catalog_mouse_profile_defaults_have_one_product_owner() {
    for (product_id, factory) in super::read_mouse_products_catalog() {
        let spec = factory();
        assert!(spec.profile_defaults().is_object(), "PID{product_id}");
        if product_id == 182 {
            assert!(spec.profile.is_none());
            assert!(spec.device_profile.is_none());
            assert_eq!(
                spec.profile_defaults()["dpi"],
                serde_json::json!({"x":800,"y":800})
            );
            assert_eq!(spec.profile_defaults()["pollingRate"], 1000);
            assert_eq!(spec.device_defaults().unwrap()["productId"], 182);
        } else {
            assert!(spec.profile.is_some(), "PID{product_id}");
        }
    }
}

#[test]
fn customize_viewport_mounted_sidebar_routes_share_one_scroll_owner() {
    use super::super::product_surface::BodyLayout;

    for product_id in [70, 190, 120, 229, 239] {
        assert_eq!(
            super::source_body_layout(product_id, "TAB_CUSTOMIZE", 0, 16),
            BodyLayout::SplitViewport,
            "PID{product_id}"
        );
    }
    // The two dedicated renderers use their first source group directly.
    for product_id in [70, 190] {
        assert_eq!(
            super::source_body_layout(product_id, "TAB_CUSTOMIZE", 0, 0),
            BodyLayout::SplitViewport,
            "PID{product_id}"
        );
    }
}

#[test]
fn customize_viewport_missing_surface_preserves_document_scroll() {
    use super::super::product_surface::BodyLayout;

    for (product_id, group_index, button_count) in [
        (u32::MAX, 0, 16),
        (229, usize::MAX, 16),
        (229, 0, 0),
        (239, 0, 1),
    ] {
        assert_eq!(
            super::source_body_layout(product_id, "TAB_CUSTOMIZE", group_index, button_count),
            BodyLayout::Document,
            "PID{product_id} group {group_index} buttons {button_count}"
        );
    }
}

#[test]
fn customize_viewport_other_pages_preserve_document_scroll_for_every_product() {
    use super::super::product_surface::BodyLayout;

    for (product_id, _) in super::read_mouse_products_catalog() {
        for page in [
            "TAB_PERFORMANCE",
            "TAB_POWER",
            "TAB_LIGHTING",
            "TAB_CALIBRATION",
            "TAB_SCROLLING",
            "ADVANCED",
            "HELP",
        ] {
            assert_eq!(
                super::source_body_layout(product_id, page, 0, 16),
                BodyLayout::Document,
                "PID{product_id} {page}"
            );
        }
    }
}

#[test]
#[should_panic(expected = "Packaged mouse profile defaults are absent")]
fn packaged_catalog_missing_mouse_defaults_cannot_borrow_another_product() {
    let factory = super::read_mouse_products_catalog()
        .into_iter()
        .find(|(product_id, _)| *product_id == 70)
        .unwrap()
        .1;
    let mut spec = factory();
    spec.profile = None;
    spec.profile_defaults();
}

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

#[::core::prelude::v1::test]
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

#[::core::prelude::v1::test]
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

#[::core::prelude::v1::test]
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

#[::core::prelude::v1::test]
fn localized_names_have_no_leftover_chinese_in_the_page_sources() {
    // The page must not spell its own labels any more: every page label now goes
    // through `t(...)`. This guards against re-introducing a hardcoded string.
    let source = include_str!("mod.rs");
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
