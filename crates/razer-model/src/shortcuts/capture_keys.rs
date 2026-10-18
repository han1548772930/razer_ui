//! Dashboard module 46114 Rk fields consumed by the mounted recorder, in source order.
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Deserialize)]
pub(super) struct CaptureKey {
    pub(super) name: String,
    #[serde(rename = "inputID")]
    pub(super) input_id: Option<String>,
    pub(super) scancode: u32,
    #[serde(rename = "keyCode")]
    pub(super) key_code: String,
    pub(super) flag: u32,
    #[serde(rename = "outputFlag")]
    pub(super) output_flag: Option<u32>,
}

pub(super) fn keys() -> &'static [CaptureKey] {
    static KEYS: OnceLock<Vec<CaptureKey>> = OnceLock::new();
    KEYS.get_or_init(make_shortcut_capture_keys_table)
}

/// Module 26061 DKMKEYS belongs to product 708, BLE 707 and receiver 706.
/// The source tests truthiness before replacing the native scan code.
pub(super) fn dkm_scan_code(key: u32) -> Option<u32> {
    match key {
        251 => Some(14),
        252 => Some(48),
        253 => Some(57),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mounted_capture_table_keeps_source_order_and_compiler_differences() {
        let rows = keys();
        assert_eq!(rows.len(), 144);
        assert_eq!(rows[0].input_id.as_deref(), Some("KEY_A"));
        let by_id = |id| {
            rows.iter()
                .find(|row| row.input_id.as_deref() == Some(id))
                .unwrap()
        };
        assert_eq!(by_id("KEY_RIGHT_CTRL").output_flag, None);
        assert_eq!(by_id("KEY_RIGHT_ALT").output_flag, None);
        assert_eq!(by_id("KEY_NUMPAD_PERIOD").key_code, "110");
        assert_eq!(by_id("KEY_CANCEL").name, "Invalid/cancel");
        assert_eq!(
            rows.iter()
                .enumerate()
                .filter(|(_, row)| row.input_id.as_deref() == Some("KEY_NUMPAD_NUM_LOCK"))
                .map(|(ix, row)| (ix, row.scancode))
                .collect::<Vec<_>>(),
            [(60, 69), (105, 69)]
        );
        assert!(
            rows.iter()
                .any(|row| row.input_id.is_none() && row.name == "<")
        );
        assert!(!rows.iter().any(|row| {
            row.input_id
                .as_deref()
                .is_some_and(|id| id.starts_with("DPAD_"))
        }));
    }

    #[test]
    fn dkm_redirect_preserves_only_source_defined_keys() {
        assert_eq!(dkm_scan_code(251), Some(14));
        assert_eq!(dkm_scan_code(252), Some(48));
        assert_eq!(dkm_scan_code(253), Some(57));
        for key in [0, 1, 10, 11, 32, 65, 84, 96, 247, 250, 254] {
            assert_eq!(dkm_scan_code(key), None);
        }
    }
}

// Generated typed table: razer-model/shortcut_capture_keys.
#[rustfmt::skip]
fn make_shortcut_capture_keys_table() -> Vec<CaptureKey> {
    vec![
        CaptureKey{
            name: "A".into(),
            input_id: Some("KEY_A".into()),
            scancode: 30u32,
            key_code: "65".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "B".into(),
            input_id: Some("KEY_B".into()),
            scancode: 48u32,
            key_code: "66".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "C".into(),
            input_id: Some("KEY_C".into()),
            scancode: 46u32,
            key_code: "67".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "D".into(),
            input_id: Some("KEY_D".into()),
            scancode: 32u32,
            key_code: "68".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "E".into(),
            input_id: Some("KEY_E".into()),
            scancode: 18u32,
            key_code: "69".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "F".into(),
            input_id: Some("KEY_F".into()),
            scancode: 33u32,
            key_code: "70".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "G".into(),
            input_id: Some("KEY_G".into()),
            scancode: 34u32,
            key_code: "71".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "H".into(),
            input_id: Some("KEY_H".into()),
            scancode: 35u32,
            key_code: "72".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "I".into(),
            input_id: Some("KEY_I".into()),
            scancode: 23u32,
            key_code: "73".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "J".into(),
            input_id: Some("KEY_J".into()),
            scancode: 36u32,
            key_code: "74".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "K".into(),
            input_id: Some("KEY_K".into()),
            scancode: 37u32,
            key_code: "75".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "L".into(),
            input_id: Some("KEY_L".into()),
            scancode: 38u32,
            key_code: "76".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "M".into(),
            input_id: Some("KEY_M".into()),
            scancode: 50u32,
            key_code: "77".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "N".into(),
            input_id: Some("KEY_N".into()),
            scancode: 49u32,
            key_code: "78".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "O".into(),
            input_id: Some("KEY_O".into()),
            scancode: 24u32,
            key_code: "79".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "P".into(),
            input_id: Some("KEY_P".into()),
            scancode: 25u32,
            key_code: "80".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "Q".into(),
            input_id: Some("KEY_Q".into()),
            scancode: 16u32,
            key_code: "81".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "R".into(),
            input_id: Some("KEY_R".into()),
            scancode: 19u32,
            key_code: "82".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "S".into(),
            input_id: Some("KEY_S".into()),
            scancode: 31u32,
            key_code: "83".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "T".into(),
            input_id: Some("KEY_T".into()),
            scancode: 20u32,
            key_code: "84".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "U".into(),
            input_id: Some("KEY_U".into()),
            scancode: 22u32,
            key_code: "85".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "V".into(),
            input_id: Some("KEY_V".into()),
            scancode: 47u32,
            key_code: "86".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "W".into(),
            input_id: Some("KEY_W".into()),
            scancode: 17u32,
            key_code: "87".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "X".into(),
            input_id: Some("KEY_X".into()),
            scancode: 45u32,
            key_code: "88".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "Y".into(),
            input_id: Some("KEY_Y".into()),
            scancode: 21u32,
            key_code: "89".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "Z".into(),
            input_id: Some("KEY_Z".into()),
            scancode: 44u32,
            key_code: "90".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "0".into(),
            input_id: Some("KEY_0".into()),
            scancode: 11u32,
            key_code: "48".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "1".into(),
            input_id: Some("KEY_1".into()),
            scancode: 2u32,
            key_code: "49".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "2".into(),
            input_id: Some("KEY_2".into()),
            scancode: 3u32,
            key_code: "50".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "3".into(),
            input_id: Some("KEY_3".into()),
            scancode: 4u32,
            key_code: "51".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "4".into(),
            input_id: Some("KEY_4".into()),
            scancode: 5u32,
            key_code: "52".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "5".into(),
            input_id: Some("KEY_5".into()),
            scancode: 6u32,
            key_code: "53".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "6".into(),
            input_id: Some("KEY_6".into()),
            scancode: 7u32,
            key_code: "54".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "7".into(),
            input_id: Some("KEY_7".into()),
            scancode: 8u32,
            key_code: "55".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "8".into(),
            input_id: Some("KEY_8".into()),
            scancode: 9u32,
            key_code: "56".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "9".into(),
            input_id: Some("KEY_9".into()),
            scancode: 10u32,
            key_code: "57".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "F1".into(),
            input_id: Some("KEY_F1".into()),
            scancode: 59u32,
            key_code: "112".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "F2".into(),
            input_id: Some("KEY_F2".into()),
            scancode: 60u32,
            key_code: "113".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "F3".into(),
            input_id: Some("KEY_F3".into()),
            scancode: 61u32,
            key_code: "114".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "F4".into(),
            input_id: Some("KEY_F4".into()),
            scancode: 62u32,
            key_code: "115".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "F5".into(),
            input_id: Some("KEY_F5".into()),
            scancode: 63u32,
            key_code: "116".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "F6".into(),
            input_id: Some("KEY_F6".into()),
            scancode: 64u32,
            key_code: "117".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "F7".into(),
            input_id: Some("KEY_F7".into()),
            scancode: 65u32,
            key_code: "118".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "F8".into(),
            input_id: Some("KEY_F8".into()),
            scancode: 66u32,
            key_code: "119".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "F9".into(),
            input_id: Some("KEY_F9".into()),
            scancode: 67u32,
            key_code: "120".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "F10".into(),
            input_id: Some("KEY_F10".into()),
            scancode: 68u32,
            key_code: "121".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "F11".into(),
            input_id: Some("KEY_F11".into()),
            scancode: 87u32,
            key_code: "122".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "F12".into(),
            input_id: Some("KEY_F12".into()),
            scancode: 88u32,
            key_code: "123".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "F13".into(),
            input_id: Some("KEY_F13".into()),
            scancode: 100u32,
            key_code: "124".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "F14".into(),
            input_id: Some("KEY_F14".into()),
            scancode: 101u32,
            key_code: "125".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "F15".into(),
            input_id: Some("KEY_F15".into()),
            scancode: 102u32,
            key_code: "126".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "F16".into(),
            input_id: Some("KEY_F16".into()),
            scancode: 103u32,
            key_code: "127".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "F17".into(),
            input_id: Some("KEY_F17".into()),
            scancode: 104u32,
            key_code: "128".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "F18".into(),
            input_id: Some("KEY_F18".into()),
            scancode: 105u32,
            key_code: "129".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "F19".into(),
            input_id: Some("KEY_F19".into()),
            scancode: 106u32,
            key_code: "130".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "F20".into(),
            input_id: Some("KEY_F20".into()),
            scancode: 107u32,
            key_code: "131".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "F21".into(),
            input_id: Some("KEY_F21".into()),
            scancode: 108u32,
            key_code: "132".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "F22".into(),
            input_id: Some("KEY_F22".into()),
            scancode: 109u32,
            key_code: "133".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "F23".into(),
            input_id: Some("KEY_F23".into()),
            scancode: 110u32,
            key_code: "134".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "F24".into(),
            input_id: Some("KEY_F24".into()),
            scancode: 118u32,
            key_code: "135".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "Num Lock".into(),
            input_id: Some("KEY_NUMPAD_NUM_LOCK".into()),
            scancode: 69u32,
            key_code: "144".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "Num 0".into(),
            input_id: Some("KEY_NUMPAD_0".into()),
            scancode: 82u32,
            key_code: "96".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "Num 1".into(),
            input_id: Some("KEY_NUMPAD_1".into()),
            scancode: 79u32,
            key_code: "97".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "Num 2".into(),
            input_id: Some("KEY_NUMPAD_2".into()),
            scancode: 80u32,
            key_code: "98".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "Num 3".into(),
            input_id: Some("KEY_NUMPAD_3".into()),
            scancode: 81u32,
            key_code: "99".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "Num 4".into(),
            input_id: Some("KEY_NUMPAD_4".into()),
            scancode: 75u32,
            key_code: "100".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "Num 5".into(),
            input_id: Some("KEY_NUMPAD_5".into()),
            scancode: 76u32,
            key_code: "101".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "Num 6".into(),
            input_id: Some("KEY_NUMPAD_6".into()),
            scancode: 77u32,
            key_code: "102".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "Num 7".into(),
            input_id: Some("KEY_NUMPAD_7".into()),
            scancode: 71u32,
            key_code: "103".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "Num 8".into(),
            input_id: Some("KEY_NUMPAD_8".into()),
            scancode: 72u32,
            key_code: "104".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "Num 9".into(),
            input_id: Some("KEY_NUMPAD_9".into()),
            scancode: 73u32,
            key_code: "105".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "Num /".into(),
            input_id: Some("KEY_NUMPAD_SLASH".into()),
            scancode: 53u32,
            key_code: "111".into(),
            flag: 0u32,
            output_flag: Some(2u32)
        },
        CaptureKey{
            name: "Num *".into(),
            input_id: Some("KEY_NUMPAD_ASTERISK".into()),
            scancode: 55u32,
            key_code: "106".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "Num -".into(),
            input_id: Some("KEY_NUMPAD_DASH".into()),
            scancode: 74u32,
            key_code: "109".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "Num +".into(),
            input_id: Some("KEY_NUMPAD_PLUS".into()),
            scancode: 78u32,
            key_code: "107".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "Num .".into(),
            input_id: Some("KEY_NUMPAD_PERIOD".into()),
            scancode: 83u32,
            key_code: "110".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "Num Enter".into(),
            input_id: Some("KEY_NUMPAD_ENTER".into()),
            scancode: 28u32,
            key_code: "13".into(),
            flag: 1u32,
            output_flag: Some(2u32)
        },
        CaptureKey{
            name: "Up".into(),
            input_id: Some("KEY_UP_ARROW".into()),
            scancode: 72u32,
            key_code: "38".into(),
            flag: 0u32,
            output_flag: Some(2u32)
        },
        CaptureKey{
            name: "Down".into(),
            input_id: Some("KEY_DOWN_ARROW".into()),
            scancode: 80u32,
            key_code: "40".into(),
            flag: 0u32,
            output_flag: Some(2u32)
        },
        CaptureKey{
            name: "Left".into(),
            input_id: Some("KEY_LEFT_ARROW".into()),
            scancode: 75u32,
            key_code: "37".into(),
            flag: 0u32,
            output_flag: Some(2u32)
        },
        CaptureKey{
            name: "Right".into(),
            input_id: Some("KEY_RIGHT_ARROW".into()),
            scancode: 77u32,
            key_code: "39".into(),
            flag: 0u32,
            output_flag: Some(2u32)
        },
        CaptureKey{
            name: "Page Up".into(),
            input_id: Some("KEY_PAGE_UP".into()),
            scancode: 73u32,
            key_code: "33".into(),
            flag: 0u32,
            output_flag: Some(2u32)
        },
        CaptureKey{
            name: "Page Down".into(),
            input_id: Some("KEY_PAGE_DOWN".into()),
            scancode: 81u32,
            key_code: "34".into(),
            flag: 0u32,
            output_flag: Some(2u32)
        },
        CaptureKey{
            name: "Backspace".into(),
            input_id: Some("KEY_BACKSPACE".into()),
            scancode: 14u32,
            key_code: "8".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "Tab".into(),
            input_id: Some("KEY_TAB".into()),
            scancode: 15u32,
            key_code: "9".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "Space".into(),
            input_id: Some("KEY_SPACEBAR".into()),
            scancode: 57u32,
            key_code: "32".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "Enter".into(),
            input_id: Some("KEY_ENTER".into()),
            scancode: 28u32,
            key_code: "13".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "Esc".into(),
            input_id: Some("KEY_ESC".into()),
            scancode: 1u32,
            key_code: "27".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "Insert".into(),
            input_id: Some("KEY_INSERT".into()),
            scancode: 82u32,
            key_code: "45".into(),
            flag: 0u32,
            output_flag: Some(2u32)
        },
        CaptureKey{
            name: "Home".into(),
            input_id: Some("KEY_HOME".into()),
            scancode: 71u32,
            key_code: "36".into(),
            flag: 0u32,
            output_flag: Some(2u32)
        },
        CaptureKey{
            name: "Delete".into(),
            input_id: Some("KEY_DELETE".into()),
            scancode: 83u32,
            key_code: "46".into(),
            flag: 0u32,
            output_flag: Some(2u32)
        },
        CaptureKey{
            name: "End".into(),
            input_id: Some("KEY_END".into()),
            scancode: 79u32,
            key_code: "35".into(),
            flag: 0u32,
            output_flag: Some(2u32)
        },
        CaptureKey{
            name: "Print Screen".into(),
            input_id: Some("KEY_PRINT_SCREEN".into()),
            scancode: 55u32,
            key_code: "44".into(),
            flag: 0u32,
            output_flag: Some(2u32)
        },
        CaptureKey{
            name: "Pause".into(),
            input_id: Some("KEY_PAUSE".into()),
            scancode: 29u32,
            key_code: "19".into(),
            flag: 0u32,
            output_flag: Some(4u32)
        },
        CaptureKey{
            name: "Invalid/cancel".into(),
            input_id: Some("KEY_CANCEL".into()),
            scancode: 70u32,
            key_code: "3".into(),
            flag: 0u32,
            output_flag: Some(2u32)
        },
        CaptureKey{
            name: "Menu".into(),
            input_id: Some("KEY_APPLICATION".into()),
            scancode: 93u32,
            key_code: "93".into(),
            flag: 0u32,
            output_flag: Some(2u32)
        },
        CaptureKey{
            name: "Ctrl".into(),
            input_id: Some("KEY_LEFT_CTRL".into()),
            scancode: 29u32,
            key_code: "17".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "Right Ctrl".into(),
            input_id: Some("KEY_RIGHT_CTRL".into()),
            scancode: 29u32,
            key_code: "17".into(),
            flag: 2u32,
            output_flag: None
        },
        CaptureKey{
            name: "Shift".into(),
            input_id: Some("KEY_LEFT_SHIFT".into()),
            scancode: 42u32,
            key_code: "16".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "Right Shift".into(),
            input_id: Some("KEY_RIGHT_SHIFT".into()),
            scancode: 54u32,
            key_code: "16".into(),
            flag: 2u32,
            output_flag: Some(0u32)
        },
        CaptureKey{
            name: "Alt".into(),
            input_id: Some("KEY_LEFT_ALT".into()),
            scancode: 56u32,
            key_code: "18".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "Right Alt".into(),
            input_id: Some("KEY_RIGHT_ALT".into()),
            scancode: 56u32,
            key_code: "18".into(),
            flag: 2u32,
            output_flag: None
        },
        CaptureKey{
            name: "Windows".into(),
            input_id: Some("KEY_LEFT_GUI".into()),
            scancode: 91u32,
            key_code: "91".into(),
            flag: 0u32,
            output_flag: Some(2u32)
        },
        CaptureKey{
            name: "Caps Lock".into(),
            input_id: Some("KEY_CAPS_LOCK".into()),
            scancode: 58u32,
            key_code: "20".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "Scroll Lock".into(),
            input_id: Some("KEY_SCROLL_LOCK".into()),
            scancode: 70u32,
            key_code: "145".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "Num Lock".into(),
            input_id: Some("KEY_NUMPAD_NUM_LOCK".into()),
            scancode: 69u32,
            key_code: "144".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "`".into(),
            input_id: Some("KEY_TILDE".into()),
            scancode: 41u32,
            key_code: "192".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "~".into(),
            input_id: None,
            scancode: 41u32,
            key_code: "192".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "!".into(),
            input_id: None,
            scancode: 2u32,
            key_code: "49".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "@".into(),
            input_id: None,
            scancode: 3u32,
            key_code: "50".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "#".into(),
            input_id: Some("KEY_NON_US_POUND".into()),
            scancode: 43u32,
            key_code: "51".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "$".into(),
            input_id: None,
            scancode: 5u32,
            key_code: "52".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "%".into(),
            input_id: None,
            scancode: 6u32,
            key_code: "53".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "^".into(),
            input_id: None,
            scancode: 7u32,
            key_code: "54".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "&".into(),
            input_id: None,
            scancode: 8u32,
            key_code: "55".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "*".into(),
            input_id: None,
            scancode: 9u32,
            key_code: "56".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "(".into(),
            input_id: None,
            scancode: 10u32,
            key_code: "57".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: ")".into(),
            input_id: None,
            scancode: 11u32,
            key_code: "48".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "-".into(),
            input_id: Some("KEY_HYPEN".into()),
            scancode: 12u32,
            key_code: "189".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "=".into(),
            input_id: Some("KEY_EQUAL".into()),
            scancode: 13u32,
            key_code: "187".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "_".into(),
            input_id: None,
            scancode: 12u32,
            key_code: "189".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "+".into(),
            input_id: None,
            scancode: 13u32,
            key_code: "187".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "[".into(),
            input_id: Some("KEY_OPEN_SQUARE_BRACKET".into()),
            scancode: 26u32,
            key_code: "219".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "]".into(),
            input_id: Some("KEY_CLOSE_SQUARE_BRACKET".into()),
            scancode: 27u32,
            key_code: "221".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "\\".into(),
            input_id: Some("KEY_BACKSLASH".into()),
            scancode: 43u32,
            key_code: "220".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "{".into(),
            input_id: None,
            scancode: 26u32,
            key_code: "219".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "}".into(),
            input_id: None,
            scancode: 27u32,
            key_code: "221".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "|".into(),
            input_id: None,
            scancode: 43u32,
            key_code: "220".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: ";".into(),
            input_id: Some("KEY_SEMICOLON".into()),
            scancode: 39u32,
            key_code: "186".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "'".into(),
            input_id: Some("KEY_APOSTROPHE".into()),
            scancode: 40u32,
            key_code: "222".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: ":".into(),
            input_id: None,
            scancode: 39u32,
            key_code: "186".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "\"".into(),
            input_id: None,
            scancode: 40u32,
            key_code: "222".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: ",".into(),
            input_id: Some("KEY_COMMA".into()),
            scancode: 51u32,
            key_code: "188".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: ".".into(),
            input_id: Some("KEY_PERIOD".into()),
            scancode: 52u32,
            key_code: "190".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "/".into(),
            input_id: Some("KEY_SLASH".into()),
            scancode: 53u32,
            key_code: "191".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "<".into(),
            input_id: None,
            scancode: 51u32,
            key_code: "188".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: ">".into(),
            input_id: None,
            scancode: 52u32,
            key_code: "190".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "?".into(),
            input_id: None,
            scancode: 53u32,
            key_code: "191".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "KEY_YEN".into(),
            input_id: Some("KEY_YEN".into()),
            scancode: 125u32,
            key_code: "".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "KEY_BEFORE_LEFT_SHIFT".into(),
            input_id: Some("KEY_RO".into()),
            scancode: 115u32,
            key_code: "".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "KEY_BEFORE_SPACE".into(),
            input_id: Some("KEY_MUHENKAN".into()),
            scancode: 123u32,
            key_code: "".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "KEY_AFTER_SPACE".into(),
            input_id: Some("KEY_HENKAN".into()),
            scancode: 121u32,
            key_code: "".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "KEY_BEFORE_RIGHT_ALT".into(),
            input_id: Some("KEY_KATAKANA_HIRAGANA".into()),
            scancode: 112u32,
            key_code: "".into(),
            flag: 0u32,
            output_flag: None
        },
        CaptureKey{
            name: "\\".into(),
            input_id: Some("KEY_NON_US_BACKSLASH".into()),
            scancode: 86u32,
            key_code: "226".into(),
            flag: 0u32,
            output_flag: None
        }
    ]
}
