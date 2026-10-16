//! Current70 UI main factories 3254 (KLID map), 1867.PJ and 857 (key names).
//! Tables are source presentation facts; no OS observation is fabricated here.
use serde_json::Value;
use std::sync::OnceLock;

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
    static NAMES: OnceLock<Value> = OnceLock::new();
    let names = NAMES.get_or_init(|| {
        serde_json::from_str(include_str!("keyboard_layout_names.json"))
            .expect("current source key name tables")
    });
    let table = layout
        .and_then(|layout| names.get(layout.to_string()))
        .unwrap_or(&names["default"]);
    let name = table.get(input).or_else(|| names["default"].get(input));
    name.and_then(|name| name.as_str().or_else(|| name["DEFAULT"].as_str()))
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
}
