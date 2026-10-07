//! Receipts for the accessory-system page dispatch, monitor tips and PIP widget.
//!
//! The Raptor monitor widget picks its HDR tip from the host OS version, the
//! display page mounts the `qAA`/`JAA` PIP picker over the `Screen` mock, and the
//! monitor/cooling pages have local renderers without generated descriptors, so
//! the dispatch must reach them before the descriptor guard.
use super::{
    COLOR_PAGE_COLUMNS, COLOR_PRESETS, INPUT_SOURCES, InputSourceStep, PIP_SOURCES, PIP_SQUARES,
    gaming_gamma_steps, gaming_gamma_tags, gaming_overdrive_tags, hdr_tooltip_key,
    input_source_step, source_product, supports_page, valid_percent_draft,
};

// Compile-checked only; application/test execution is prohibited in this audit.
#[gpui_kit::test]
fn monitor_observations_clear_on_disconnect_and_do_not_enter_profiles(
    cx: &mut gpui_kit::TestAppContext,
) {
    use gpui_kit::{AppContext, component::Root, px, size};
    use serde_json::json;
    cx.update(gpui_kit::init);
    let mut view = None;
    let handle = cx.open_window(size(px(1100.), px(800.)), |window, cx| {
        let body = cx.new(|cx| super::AccessorySystemProductWorkspace::new(3880, window, cx));
        view = Some(body.clone());
        Root::new(body, window, cx)
    });
    let view = view.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |body, cx| {
            let initial = body.snapshot();
            assert!(body.supported_refresh_rates.is_empty());
            assert_eq!(body.selected_refresh_rate, None);
            body.set_monitor_runtime(
                Some(&json!({
                    "colorProfiles": ["Standard.icm", "Cinema.icm"],
                    "selectedColorProfile": "C:/Color/Standard.icm",
                    "supportedRefreshRate": [165, 60, 60],
                    "selectedRefreshRate": 165,
                    "uiRestraint": {"refreshRate": "Restricted"}
                })),
                window,
                cx,
            );
            assert_eq!(body.supported_refresh_rates, vec![60, 165]);
            assert_eq!(body.selected_refresh_rate, Some(165));
            assert_eq!(body.selected_color_profile, "C:/Color/Standard.icm");
            assert!(body.restricted("refreshRate"));
            body.restore(
                Some(&json!({"uiRestraint": {"refreshRate": ""}})),
                window,
                cx,
            );
            assert!(body.restricted("refreshRate"));
            assert_eq!(body.restriction_reason("refreshRate"), Some("Restricted"));
            // Current nSA/HSA FPS widgets do not consume refreshRate restraint.
            assert!(body.allowed("/refeshRateCounter/isEnabled"));
            body.pending_refresh_rate = Some(60);
            body.pending_color_profile = Some("Cinema.icm".into());
            assert_eq!(body.selected_refresh_rate, Some(165));
            assert_eq!(body.snapshot(), initial);
            body.set_monitor_runtime(None, window, cx);
            assert!(body.color_profiles.is_empty());
            assert!(body.selected_color_profile.is_empty());
            assert!(body.supported_refresh_rates.is_empty());
            assert_eq!(body.selected_refresh_rate, None);
            assert_eq!(body.pending_refresh_rate, None);
            assert_eq!(body.pending_color_profile, None);
            assert!(!body.restricted("refreshRate"));
            assert_eq!(body.snapshot(), initial);
        });
    })
    .unwrap();
}

#[test]
fn color_presets_match_the_gm_enum() {
    // `Object.entries(GM)` order, with the labels `KAA["SCARLETT_"+name]` resolves to.
    assert_eq!(
        COLOR_PRESETS,
        [
            (5, "NORMAL"),
            (12, "LOW_BLUE_LIGHT"),
            (4, "WARM"),
            (8, "COOL"),
            (1, "SRGB"),
            (11, "SCARLETT_CUSTOM"),
        ]
    );
    // The custom branch keys off `GM.CUSTOM` (11), and no other preset may share it.
    let custom = COLOR_PRESETS
        .iter()
        .filter(|(id, _)| *id == 11)
        .collect::<Vec<_>>();
    assert_eq!(custom.len(), 1);
    assert_eq!(custom[0].1, "SCARLETT_CUSTOM");
}

#[test]
fn color_page_columns_follow_the_source_grid() {
    // `NSA`: left column THX Cinema + Color Profile, right column HDR + Color
    // Temperature, each inside a 600px `.widget-col`.
    assert_eq!(
        COLOR_PAGE_COLUMNS[0],
        [
            "THX_CINEMA_HEADER",
            "PERFORMANCE_MODE_SCREEN_COLOR_PROFILE_HEADER"
        ]
    );
    assert_eq!(
        COLOR_PAGE_COLUMNS[1],
        ["HDR_HEADER", "COLOR_TEMPERATURE_HEADER"]
    );
    // The color temperature widget is shared with 3858, so it must appear once and
    // every column title must be unique.
    let mut titles = COLOR_PAGE_COLUMNS.iter().flatten().collect::<Vec<_>>();
    titles.sort_unstable();
    let unique = titles.len();
    titles.dedup();
    assert_eq!(titles.len(), unique);
}

#[test]
fn percent_draft_matches_the_source_input_pattern() {
    // `<input maxLength={3} pattern="(100)|(0*\d{1,2})">`.
    for ok in ["0", "5", "05", "42", "99", "100"] {
        assert!(valid_percent_draft(ok), "{ok} should be accepted");
    }
    for bad in ["", " ", "101", "1000", "1e2", "-1", "12a", "100 "] {
        assert!(!valid_percent_draft(bad), "{bad} should be rejected");
    }
}

#[test]
fn gaming_rows_use_the_source_steps_and_tags() {
    // `STA`'s maxStep drives both the slider range and the tag set.
    assert_eq!(gaming_gamma_steps(3858), 2);
    assert_eq!(gaming_gamma_steps(3880), 3);
    assert_eq!(gaming_gamma_tags(3858), ("1.4", "1.8", "2.2", None));
    assert_eq!(gaming_gamma_tags(3880), ("1.4", "1.8", "2.2", Some("2.4")));
    assert_eq!(gaming_overdrive_tags(), ("OFF", "WEAK", "STRONG"));
    // The slider ranges must cover exactly the tagged stops.
    for pid in [3858, 3880] {
        let (_, _, max, boost) = gaming_gamma_tags(pid);
        let steps = gaming_gamma_steps(pid);
        assert_eq!(steps, if boost.is_some() { 3 } else { 2 }, "{pid}");
        assert!(max.starts_with("2.2"), "{pid}");
    }
}

#[test]
fn input_source_change_follows_the_confirmation_flow() {
    // `SSA.confirmInputSourceChange`.
    assert_eq!(input_source_step(0, 0, true), InputSourceStep::Ignore);
    assert_eq!(input_source_step(0, 17, true), InputSourceStep::Prompt);
    assert_eq!(input_source_step(0, 17, false), InputSourceStep::Apply);
}

#[test]
fn input_source_buttons_follow_the_monitor_enum() {
    // The widget renders Auto, HDMI, DisplayPort and USB-C in that order.
    assert_eq!(
        INPUT_SOURCES.map(|(_, key)| key),
        ["SCARLETT_AUTO", "HDMI_1", "DP_1", "USB_C"]
    );
    let spec = source_product(3858).expect("current monitor spec");
    assert_eq!(spec.enums["Bi"]["AUTO"], INPUT_SOURCES[0].0);
    assert_eq!(spec.enums["Bi"]["HDMI_1"], INPUT_SOURCES[1].0);
    assert_eq!(spec.enums["Bi"]["DP_1"], INPUT_SOURCES[2].0);
    assert_eq!(spec.enums["Bi"]["USB_C"], INPUT_SOURCES[3].0);
}

#[test]
fn hdr_tip_follows_the_host_windows_version() {
    assert_eq!(hdr_tooltip_key(true), "HDR_TOOLTIP_WINDOWS_11");
    assert_eq!(hdr_tooltip_key(false), "HDR_TOOLTIP");
}

#[test]
fn pip_presets_match_the_source_geometry() {
    // `QAA`: three sizes in four corners, z 3/2/1 so smaller boxes paint above.
    assert_eq!(PIP_SQUARES.len(), 12);
    for square in PIP_SQUARES {
        let (width, height) = match square.size {
            1 => (90., 49.5),
            2 => (120., 66.),
            3 => (151., 83.),
            other => panic!("unexpected PIP size {other}"),
        };
        assert_eq!((square.width, square.height), (width, height));
        assert_eq!(square.z, (4 - square.size) as u8);
        assert!((0..4).contains(&square.position));
    }
    for position in 0..4 {
        for size in 1..4 {
            assert!(
                PIP_SQUARES
                    .iter()
                    .any(|square| square.size == size && square.position == position),
                "size {size} must be offered in position {position}"
            );
        }
    }
}

#[test]
fn pip_sources_match_the_monitor_input_enum() {
    // `$AA`: DP_1 15, HDMI_1 17, USB_C 19, in that order.
    let spec = source_product(3858).expect("current monitor spec");
    for (key, id) in PIP_SOURCES {
        assert_eq!(spec.enums["Bi"][key], id, "{key}");
    }
    assert_eq!(PIP_SOURCES.map(|(key, _)| key), ["DP_1", "HDMI_1", "USB_C"]);
}

#[test]
fn monitor_and_cooling_pages_have_no_generated_descriptors() {
    // If a future extraction adds descriptors these pages keep working, but the
    // guard-order rule in tools/audit-page-coverage.py relies on this split.
    for (pid, key) in [
        (3858, "TAB_DISPLAY"),
        (3858, "TAB_COLOR"),
        (3858, "TAB_GAMING"),
        (3880, "TAB_DISPLAY"),
        (3900, "TAB_PERFORMANCE"),
        (3893, "TAB_PERFORMANCE"),
        (3907, "TAB_PERFORMANCE"),
        (3921, "TAB_CUSTOMIZE"),
    ] {
        assert!(
            !supports_page(pid, key),
            "{pid} {key} should come from its renderer, not the descriptor table"
        );
    }
}
