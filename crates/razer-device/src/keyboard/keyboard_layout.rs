//! Current70 UI main factories 3254 (KLID map), 1867.PJ and 857 (key names).
//! Tables are source presentation facts; no OS observation is fabricated here.
use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Deserialize)]
#[serde(untagged)]
enum SourceKeyName {
    Text(String),
    Contextual(ContextualKeyName),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextualKeyName {
    #[serde(rename = "DEFAULT", default)]
    default_upper: Option<String>,
    #[serde(rename = "default", default)]
    _default_lower: Option<String>,
    #[serde(rename = "KEYBOARD")]
    _keyboard: String,
    #[serde(rename = "MOUSE")]
    _mouse: String,
    #[serde(rename = "KEY_NON_US_POUND")]
    _non_us_pound: String,
}

impl SourceKeyName {
    fn display(&self) -> Option<&str> {
        match self {
            Self::Text(name) => Some(name),
            Self::Contextual(names) => names.default_upper.as_deref(),
        }
    }
}

pub fn system_layout(klid: i32) -> u8 {
    match klid {
        2057 => 6,
        1040 => 17,
        4108 => 15,
        1031 => 3,
        1036 => 4,
        1041 => 12,
        1054 => 11,
        1034 => 16,
        // Current Ai has no RUS property. Do not add a Windows LANGID map
        // absent from the actual source lookup table.
        _ => 1,
    }
}
pub fn key_name(layout: Option<u8>, input: &str) -> String {
    static NAMES: OnceLock<BTreeMap<String, BTreeMap<String, SourceKeyName>>> = OnceLock::new();
    let names = NAMES.get_or_init(|| crate::data::keyboard_layout_names::keyboard_layout_names!());
    let default = names.get("default").expect("fixed default keyboard names");
    let table = layout
        .and_then(|layout| names.get(&layout.to_string()))
        .unwrap_or(default);
    let name = table.get(input).or_else(|| default.get(input));
    name.and_then(SourceKeyName::display)
        .unwrap_or(input)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_system_map_does_not_infer_unlisted_language_ids() {
        assert_eq!(system_layout(1031), 3);
        assert_eq!(system_layout(0xe0010804u32 as i32), 1);
        assert_eq!(system_layout(1049), 1);
        assert_eq!(key_name(None, "unlisted_input"), "unlisted_input");
        assert_eq!(key_name(Some(250), "KEY_ESC"), key_name(None, "KEY_ESC"));
    }

    #[test]
    fn contextual_names_use_only_the_source_uppercase_default() {
        let lower_only: SourceKeyName = serde_json::from_value(serde_json::json!({
            "default": "Lowercase name",
            "KEYBOARD": "Keyboard name",
            "MOUSE": "Mouse name",
            "KEY_NON_US_POUND": "Pound name"
        }))
        .unwrap();
        assert_eq!(lower_only.display(), None);
        let upper: SourceKeyName = serde_json::from_value(serde_json::json!({
            "DEFAULT": "Source name",
            "default": "Lowercase name",
            "KEYBOARD": "Keyboard name",
            "MOUSE": "Mouse name",
            "KEY_NON_US_POUND": "Pound name"
        }))
        .unwrap();
        assert_eq!(upper.display(), Some("Source name"));
    }
}
