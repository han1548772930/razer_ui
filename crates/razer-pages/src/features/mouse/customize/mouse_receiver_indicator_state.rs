//! Source reducer/preview semantics, independent from a transport result.
use anyhow::{Context as _, ensure};
use razer_device::mouse_indicator_led::{Mode, Quality};
use serde_json::Value;

pub(super) const MODES: [(Mode, &str); 5] = [
    (Mode::Off, "ALWAYS_OFF"),
    (Mode::BatteryLevel, "BATTERY_LEVEL"),
    (Mode::ConnectionQuality, "CONNECTION_QUALITY"),
    (Mode::PollingRate, "POLLING_RATE"),
    (Mode::DpiStage, "DPI_STAGE"),
];
pub(super) const GUIDE_TABS: [&str; 4] = [
    "CONNECTION_STATUS",
    "BATTERY_LEVEL",
    "POLLING_RATE",
    "DPI_STAGE",
];

/// Ja/Ya use the first strictly numeric index match and a nullish fallback.
/// An unknown or string ledStatus has no matching enum case; it is not a
/// malformed three-element Configuration to replace with reducer defaults.
pub(super) fn mode(configuration: &Value, index: usize) -> Option<Mode> {
    let value = configuration.as_array()?.iter()
        .find(|entry| entry.get("index").and_then(Value::as_f64) == Some(index as f64))
        .and_then(|entry| entry.get("ledStatus"))
        .filter(|value| !value.is_null());
    let Some(value) = value else { return Some(Mode::ConnectionQuality); };
    let number = value.as_f64()?;
    MODES.iter().find(|(mode, _)| f64::from(u8::from(*mode)) == number).map(|(mode, _)| *mode)
}

/// Dropdown7734 indexes dataSet[value] using a JS property key, but marks a
/// row selected only for a strict numeric match. "2" therefore has a label
/// without a checked row; other in-range unknown keys keep an empty trigger.
pub(super) fn selector_label(configuration: &Value, index: usize) -> Option<&'static str> {
    let value = configuration.as_array()?.iter()
        .find(|entry| entry.get("index").and_then(Value::as_f64) == Some(index as f64))
        .and_then(|entry| entry.get("ledStatus"))
        .filter(|value| !value.is_null());
    let Some(value) = value else { return Some("CONNECTION_QUALITY"); };
    if value.as_f64() == Some(-1.)
        || razer_device::mouse_indicator_led::source_configuration_number(Some(value)) > 4.
    {
        return None;
    }
    fn property_index(value: &Value) -> Option<usize> {
        match value {
            Value::Number(value) => (0..5).find(|index| value.as_f64() == Some(*index as f64)),
            Value::String(value) => ["0", "1", "2", "3", "4"].iter().position(|key| *key == value),
            Value::Array(values) if values.len() == 1 => property_index(&values[0]),
            _ => None,
        }
    }
    Some(property_index(value).map_or("", |index| MODES[index].1))
}

/// The source click maps existing entries only. It does not add a missing LED,
/// deduplicate an index, sort the UI array, or discard unknown profile fields.
pub(super) fn with_mode(configuration: &Value, index: usize, mode: Mode) -> anyhow::Result<Value> {
    let mut entries = configuration.as_array().context("Source receiver configuration is not an array")?.clone();
    ensure!(entries.iter().all(|entry| !entry.is_null()), "Source receiver click cannot access a null entry");
    for entry in &mut entries {
        if entry.get("index").and_then(Value::as_f64) == Some(index as f64) {
            entry["ledStatus"] = serde_json::json!(u8::from(mode));
        }
    }
    Ok(Value::Array(entries))
}

pub(super) fn battery_color(percent: f64) -> u32 {
    let percent = percent.clamp(0., 100.);
    let (red, green) = if percent <= 50. {
        (255, (percent / 50. * 255.).round() as u32)
    } else {
        ((255. - (percent - 50.) / 50. * 255.).round() as u32, 255)
    };
    red << 16 | green << 8
}
pub(super) fn quality_color(quality: Quality) -> u32 {
    match quality {
        Quality::Red => 0xff0000,
        Quality::Orange => 0xff7f00,
        Quality::Yellow => 0xffff00,
        Quality::Green => 0x00ff00,
    }
}
pub(super) fn polling_color(hz: u32) -> Option<u32> {
    Some(match hz {
        125 => 0x8b00ff,
        500 => 0x0000ff,
        1000 => 0x00ff00,
        2000 => 0xffff00,
        4000 => 0xff7f00,
        8000 => 0xff0000,
        _ => return None,
    })
}
pub(super) fn dpi_color(stage: usize) -> Option<u32> {
    Some(match stage {
        1 => 0xff0000,
        2 => 0x00ff00,
        3 => 0x007ef5,
        4 => 0x00ffff,
        5 => 0xffff00,
        _ => return None,
    })
}

/// Ya counts visible rows before testing the one-based active row. Only an
/// absent active row uses red; an existing row with no visible stages has no
/// color-table entry. Current 190/229/239 full source owners are retained by
/// `audit-family-function --family mouse --function main --query mouse-receiver-indicator`.
pub(super) fn active_dpi_color(stages: &[Value], active: usize) -> Option<u32> {
    let mut visible = 0;
    for (ix, stage) in stages.iter().enumerate() {
        if json_truthy(&stage["visible"]) {
            visible += 1;
        }
        if ix.checked_add(1) == Some(active) {
            return dpi_color(visible);
        }
    }
    Some(0xff0000)
}

fn json_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.),
        Value::String(value) => !value.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

pub(super) const GUIDE_KEYS: [&[&str]; 4] = [
    &["1", "2", "3", "4"],
    &["0", "25", "50", "75", "100"],
    &["125", "500", "1000", "2000", "4000", "8000"],
    &["1", "2", "3", "4", "5"],
];

/// xa retains its string key while activeTab changes. It does not reinterpret
/// the old point's position in the new tab's table.
pub(super) fn preview_colors(tab: usize, key: &str) -> [Option<u32>; 3] {
    let mut colors = [None; 3];
    let Ok(value) = key.parse::<u32>() else {
        return colors;
    };
    let (led, color) = match tab {
        0 => (
            0,
            u8::try_from(value)
                .ok()
                .and_then(|value| Quality::try_from(value).ok())
                .map(quality_color),
        ),
        1 => (1, Some(battery_color(value as f64))),
        2 => (2, polling_color(value)),
        _ => (0, dpi_color(value as usize)),
    };
    colors[led] = color;
    colors
}

/// ka's SVG viewBox, shared by painting and circular pointer hit testing.
#[derive(Clone, Copy)]
pub(super) struct GuideScale {
    pub(super) tab: usize,
}
impl GuideScale {
    pub(super) fn width(self) -> f32 {
        if self.tab == 2 { 442. } else { 405. }
    }
    pub(super) fn x(self, ix: usize) -> f32 {
        let start = match self.tab {
            2 => 28.5,
            3 => 48.5,
            _ => 57.,
        };
        start + ix as f32 * if self.tab == 0 { 97. } else { 77. }
    }
    pub(super) fn key_at(self, x: f32, y: f32) -> &'static str {
        GUIDE_KEYS[self.tab]
            .iter()
            .enumerate()
            .find(|(ix, _)| (x - self.x(*ix)).powi(2) + (y - 5.).powi(2) <= 25.)
            .map_or("none", |(_, key)| *key)
    }
}
pub(super) fn colors(
    configuration: &Value,
    quality: Quality,
    battery: f64,
    polling: u32,
    dpi: Option<u32>,
) -> [Option<u32>; 3] {
    std::array::from_fn(|index| match mode(configuration, index) {
        Some(Mode::Off) | None => None,
        Some(Mode::BatteryLevel) => Some(battery_color(battery)),
        Some(Mode::ConnectionQuality) => Some(quality_color(quality)),
        Some(Mode::PollingRate) => polling_color(polling),
        Some(Mode::DpiStage) => dpi,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        GuideScale, active_dpi_color, battery_color, colors, dpi_color, polling_color,
        preview_colors, mode, selector_label, with_mode,
    };
    use razer_device::mouse_indicator_led::{Configuration, Mode, Quality};
    #[test]
    fn source_lookup_distinguishes_missing_null_unknown_and_string_modes() {
        use serde_json::json;
        assert_eq!(mode(&json!([]), 0), Some(Mode::ConnectionQuality));
        assert_eq!(mode(&json!([{"index":0,"ledStatus":null}]),0),Some(Mode::ConnectionQuality));
        assert_eq!(mode(&json!([{"index":0,"ledStatus":99},{"index":0,"ledStatus":2}]),0),None);
        assert_eq!(mode(&json!([{"index":"0","ledStatus":0}]),0),Some(Mode::ConnectionQuality));
        assert_eq!(mode(&json!([{"index":0,"ledStatus":"2"}]),0),None);
        assert_eq!(colors(&json!([{"index":0,"ledStatus":99}]),Quality::Green,0.,1000,None),[None,Some(0x00ff00),Some(0x00ff00)]);
    }
    #[test]
    fn source_dropdown_property_lookup_and_strict_row_selection_stay_separate() {
        use serde_json::json;
        for value in [json!("2"), json!([2]), json!([["2"]])] {
            let configuration = json!([{"index":0,"ledStatus":value}]);
            assert_eq!(selector_label(&configuration,0),Some("CONNECTION_QUALITY"));
            assert_eq!(mode(&configuration,0),None);
        }
        for value in [json!(-2), json!(1.5), json!("foo"), json!(false), json!({}),json!([])] {
            assert_eq!(selector_label(&json!([{"index":0,"ledStatus":value}]),0),Some(""));
        }
        for value in [json!(-1),json!(5),json!("5"),json!([5])] {
            assert_eq!(selector_label(&json!([{"index":0,"ledStatus":value}]),0),None);
        }
        assert_eq!(selector_label(&json!([{"index":0,"ledStatus":"-1"}]),0),Some(""));
    }
    #[test]
    fn source_click_updates_all_numeric_matches_without_adding_sorting_or_losing_metadata() {
        use serde_json::json;
        let configuration=json!([{"index":2,"ledStatus":4,"extra":7},{"index":0,"ledStatus":2},{"index":0,"ledStatus":1},{"index":"0","ledStatus":0}]);
        let updated=with_mode(&configuration,0,Mode::Off).unwrap();
        assert_eq!(updated,json!([{"index":2,"ledStatus":4,"extra":7},{"index":0,"ledStatus":0},{"index":0,"ledStatus":0},{"index":"0","ledStatus":0}]));
        assert_eq!(with_mode(&updated,0,Mode::Off).unwrap(),updated);
        assert_eq!(with_mode(&configuration,1,Mode::Off).unwrap(),configuration);
        assert!(with_mode(&json!([null]),0,Mode::Off).is_err());
    }
    #[test]
    fn source_led_colors_preserve_battery_endpoints_and_exact_source_rounding() {
        assert_eq!(battery_color(-5.), 0xff0000);
        assert_eq!(battery_color(25.), 0xff8000);
        assert_eq!(battery_color(50.), 0xffff00);
        assert_eq!(battery_color(75.), 0x80ff00);
        assert_eq!(battery_color(200.), 0x00ff00);
    }
    #[test]
    fn unknown_polling_or_dpi_color_stays_absent_and_off_never_invents_a_light() {
        assert_eq!(polling_color(333), None);
        assert_eq!(dpi_color(0), None);
        assert_eq!(
            colors(
                &serde_json::json!(Configuration::new([Mode::Off, Mode::PollingRate, Mode::DpiStage])),
                Quality::Green,
                0.,
                333,
                None
            ),
            [None; 3]
        );
    }
    #[test]
    fn default_colors_are_source_reducer_defaults_not_device_observations() {
        assert_eq!(
            colors(&serde_json::json!(Configuration::default()), Quality::Green, 0., 1000, None),
            [Some(0x00ff00), Some(0xff0000), Some(0x00ff00)]
        );
    }
    #[test]
    fn retained_hover_key_uses_the_new_tabs_table_without_indexing_it() {
        assert_eq!(preview_colors(2, "8000"), [None, None, Some(0xff0000)]);
        assert_eq!(preview_colors(0, "8000"), [None; 3]);
        assert_eq!(preview_colors(3, "8000"), [None; 3]);
        assert_eq!(preview_colors(1, "8000"), [None, Some(0x00ff00), None]);
        assert_eq!(preview_colors(1, "none"), [None; 3]);
        assert_eq!(preview_colors(0, "1"), [Some(0xff0000), None, None]);
        assert_eq!(preview_colors(2, "1"), [None; 3]);
    }
    #[test]
    fn dpi_missing_row_and_existing_invisible_row_have_distinct_source_results() {
        let stages = serde_json::json!([{"visible":false}, {"visible":true}]);
        let stages = stages.as_array().unwrap();
        assert_eq!(active_dpi_color(stages, 0), Some(0xff0000));
        assert_eq!(active_dpi_color(stages, 1), None);
        assert_eq!(active_dpi_color(stages, 2), Some(0xff0000));
        assert_eq!(active_dpi_color(stages, 3), Some(0xff0000));
    }
    #[test]
    fn only_source_svg_circles_change_hover_value() {
        let scale = GuideScale { tab: 2 };
        assert_eq!(scale.width(), 442.);
        assert_eq!(scale.key_at(413.5, 5.), "8000");
        assert_eq!(scale.key_at(413.5, 30.), "none");
        assert_eq!(scale.key_at(418., 9.5), "none");
        assert_eq!(scale.key_at(28.5, 0.), "125");
        assert_eq!(GuideScale { tab: 0 }.x(3), 348.);
        assert_eq!(GuideScale { tab: 3 }.x(4), 356.5);
    }
}
