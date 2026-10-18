//! Shared mapping vocabulary and input/website normalization.

pub mod interdevice;

/// A compiled save may update the live mapping only while its source profiles,
/// device settings, selection and route still belong to the captured revision.
pub fn matches_saved_revision(
    current: &crate::model::Device,
    saved: &crate::model::Device,
) -> bool {
    crate::profile_transfer::ProfileContext::from_device(saved).matches(current)
        && current.profiles == saved.profiles
        && match (
            mapping_source_settings(current),
            mapping_source_settings(saved),
        ) {
            (Some(current), Some(saved)) => current == saved,
            _ => false,
        }
}

fn mapping_source_settings(
    device: &crate::model::Device,
) -> Option<std::collections::BTreeMap<&str, &serde_json::Value>> {
    let settings = match device.source_device_settings.as_ref() {
        None => return Some(Default::default()),
        Some(serde_json::Value::Object(settings)) => settings,
        Some(_) => return None,
    };
    let assigned_version = device.product_id == razer_device::mouse_philips::PRODUCT_ID;
    if assigned_version
        && settings.get("version").is_some_and(|version| {
            !version.as_u64().is_some_and(|version| {
                version <= razer_device::mapping_submission::MAX_SOURCE_VERSION
            })
        })
    {
        return None;
    }
    Some(
        settings
            .iter()
            .filter(|(field, _)| {
                !razer_device::mapping_document::MappingProduct::owns_storage_field(field)
                    && !(assigned_version && field.as_str() == "version")
            })
            .map(|(field, value)| (field.as_str(), value))
            .collect(),
    )
}

pub const MEDIA: &[(&str, &str)] = &[
    ("VolumeDown", "VOLUME_DOWN"),
    ("VolumeUp", "VOLUME_UP"),
    ("MuteVolume", "MUTE_VOLUME"),
    ("MicVolumeUp", "MIC_VOLUME_UP"),
    ("MicVolumeDown", "MIC_VOLUME_DOWN"),
    ("MuteMic", "MUTE_MIC"),
    ("MuteAll", "MUTE_ALL"),
    ("Play", "PLAY_PAUSE"),
    ("PrevTrack", "PREVIOUS_TRACK"),
    ("NextTrack", "NEXT_TRACK"),
];

pub const WINDOWS: &[(&str, &str)] = &[
    ("Calculator", "LAUNCH_CALCULATOR"),
    ("MSPaint", "LAUNCH_MSPAINT"),
    ("Notepad", "LAUNCH_NOTEPAD"),
    ("Snipping_Tool", "LAUNCH_SNIPPING_TOOL"),
    ("LaunchTaskManager", "LAUNCH_TASK_MANAGER"),
    ("User_Directory", "OPEN_USER_DIRECTORY"),
    ("PowerUserMenu", "OPEN_SYSTEM_UTILITY"),
    ("ShowDesktop", "SHOW_DESKTOP"),
    ("CycleApps", "CYCLE_APPS"),
    ("SwitchApps", "SWITCH_APPS"),
    ("CloseApp", "CLOSE_APP"),
    ("Cut", "CUT"),
    ("Copy", "COPY"),
    ("Paste", "PASTE"),
    ("Mail", "MAIL"),
    ("ThisPC", "THIS_PC"),
    ("Refresh", "REFRESH"),
    ("DisplayBrightnessUp", "DISPLAY_BRIGHTNESS_UP"),
    ("DisplayBrightnessDown", "DISPLAY_BRIGHTNESS_DOWN"),
    ("File_Explorer", "OPEN_FILE_EXPLORER"),
    ("LockComputer", "LOCK_COMPUTER"),
    ("WindowsZoomIn", "WINDOWS_ZOOM_IN"),
    ("WindowsZoomOut", "WINDOWS_ZOOM_OUT"),
    ("OfficeZoomIn", "OFFICE_ZOOM_IN"),
    ("OfficeZoomOut", "OFFICE_ZOOM_OUT"),
];

pub const SHIFTED_SYMBOLS: &[(&str, &str)] = &[
    ("~", "KEY_TILDE"),
    ("!", "KEY_1"),
    ("@", "KEY_2"),
    ("#", "KEY_3"),
    ("$", "KEY_4"),
    ("%", "KEY_5"),
    ("^", "KEY_6"),
    ("&", "KEY_7"),
    ("*", "KEY_8"),
    ("(", "KEY_9"),
    (")", "KEY_0"),
    ("_", "KEY_HYPEN"),
    ("+", "KEY_EQUAL"),
    ("{", "KEY_OPEN_SQUARE_BRACKET"),
    ("}", "KEY_CLOSE_SQUARE_BRACKET"),
    ("|", "KEY_BACKSLASH"),
    (":", "KEY_SEMICOLON"),
    ("\"", "KEY_APOSTROPHE"),
    ("<", "KEY_COMMA"),
    (">", "KEY_PERIOD"),
    ("?", "KEY_SLASH"),
];

pub const KEYS: &[(&str, &str, &str)] = crate::settings::MAPPING_KEYS;

pub fn valid_key(key: &str) -> bool {
    KEYS.iter().any(|(_, id, _)| *id == key)
}

pub fn canonical_key(key: &str) -> Option<String> {
    if valid_key(key) {
        return Some(key.into());
    }
    let lower = key.to_ascii_lowercase();
    let mapped = match lower.as_str() {
        "enter" | "return" => "KEY_ENTER",
        "escape" | "esc" => "KEY_ESC",
        "space" | "spacebar" | " " => "KEY_SPACEBAR",
        "tab" => "KEY_TAB",
        "backspace" => "KEY_BACKSPACE",
        "delete" => "KEY_DELETE",
        "insert" => "KEY_INSERT",
        "home" => "KEY_HOME",
        "end" => "KEY_END",
        "pageup" | "page-up" | "page up" => "KEY_PAGE_UP",
        "pagedown" | "page-down" | "page down" => "KEY_PAGE_DOWN",
        "printscreen" | "print-screen" | "print screen" => "KEY_PRINT_SCREEN",
        "pause" => "KEY_PAUSE",
        "capslock" | "caps-lock" | "caps lock" => "KEY_CAPS_LOCK",
        "scrolllock" | "scroll-lock" | "scroll lock" => "KEY_SCROLL_LOCK",
        "numlock" | "num-lock" | "num lock" => "KEY_NUMPAD_NUM_LOCK",
        "menu" | "apps" => "KEY_APPLICATION",
        "ctrl" | "control" => "KEY_LEFT_CTRL",
        "alt" => "KEY_LEFT_ALT",
        "shift" => "KEY_LEFT_SHIFT",
        "win" | "windows" | "super" => "KEY_LEFT_GUI",
        "up" => "KEY_UP_ARROW",
        "down" => "KEY_DOWN_ARROW",
        "left" => "KEY_LEFT_ARROW",
        "right" => "KEY_RIGHT_ARROW",
        "-" => "KEY_HYPEN",
        "=" => "KEY_EQUAL",
        "[" => "KEY_OPEN_SQUARE_BRACKET",
        "]" => "KEY_CLOSE_SQUARE_BRACKET",
        "\\" => "KEY_BACKSLASH",
        ";" => "KEY_SEMICOLON",
        "'" => "KEY_APOSTROPHE",
        "`" => "KEY_TILDE",
        "," => "KEY_COMMA",
        "." => "KEY_PERIOD",
        "/" => "KEY_SLASH",
        _ => {
            if let Some((_, key)) = SHIFTED_SYMBOLS.iter().find(|(symbol, _)| *symbol == key) {
                return Some((*key).into());
            }
            if let Some((_, id, _)) = KEYS
                .iter()
                .find(|(_, _, label)| label.eq_ignore_ascii_case(key))
            {
                return Some((*id).into());
            }
            let id = format!("KEY_{}", key.to_ascii_uppercase());
            return valid_key(&id).then_some(id);
        }
    };
    Some(mapped.into())
}

pub fn key_label(key: &str) -> String {
    KEYS.iter()
        .find(|(_, id, _)| *id == key)
        .map(|(_, _, label)| (*label).to_string())
        .unwrap_or_else(|| key.into())
}

pub fn normalized_website(target: &str) -> Option<String> {
    let value = target.trim();
    if value.is_empty() || value.chars().any(char::is_whitespace) {
        return None;
    }
    let lower = value.to_ascii_lowercase();
    let (prefix, address) = if lower.starts_with("https://") {
        ("https://", &value[8..])
    } else if lower.starts_with("http://") {
        ("http://", &value[7..])
    } else if value.contains("://") {
        return None;
    } else {
        ("https://", value)
    };
    let authority = address.split(['/', '?', '#']).next().unwrap_or_default();
    let mut parts = authority.split(':');
    let host = parts.next().unwrap_or_default();
    if let Some(port) = parts.next()
        && (parts.next().is_some() || !port.parse::<u16>().is_ok_and(|port| port > 0))
    {
        return None;
    }
    let domain = host.contains('.')
        && host.split('.').all(|part| {
            !part.is_empty()
                && !part.starts_with('-')
                && !part.ends_with('-')
                && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        });
    if host.is_empty()
        || authority.contains('@')
        || !(host.eq_ignore_ascii_case("localhost") || domain)
    {
        return None;
    }
    Some(format!("{prefix}{address}"))
}

#[cfg(test)]
mod saved_revision_tests {
    use super::*;

    fn device() -> crate::model::Device {
        let mut device = crate::product_preview::registered_preview(70).unwrap();
        device.fill_defaults();
        let mut second = device.profiles[0].clone();
        second.id = "second-profile".into();
        second.guid = "second-profile".into();
        device.profiles.push(second);
        device
    }

    #[test]
    fn saved_mapping_does_not_replace_a_new_selection_or_edit() {
        let saved = device();
        let mut current = saved.clone();
        assert!(matches_saved_revision(&current, &saved));
        current.active_profile = current.profiles[1].id.clone();
        assert!(!matches_saved_revision(&current, &saved));
        current = saved.clone();
        current.profiles[0].name = "Edited during file I/O".into();
        assert!(!matches_saved_revision(&current, &saved));
        current = saved.clone();
        current.profiles[1].source_settings = Some(serde_json::json!({"extension": 42}));
        assert!(!matches_saved_revision(&current, &saved));
    }

    #[test]
    fn compiled_projection_and_metadata_do_not_change_the_source_profile_revision() {
        let mut saved = device();
        saved.source_device_settings = None;
        let mut current = saved.clone();
        let field = razer_device::mapping_document::MappingProduct::for_product_id(70)
            .unwrap()
            .storage_field();
        current.source_device_settings = Some(serde_json::json!({(field): {"version": 42}}));
        current
            .product_name
            .values
            .insert("en".into(), "Observed product name".into());
        assert!(matches_saved_revision(&current, &saved));
        current.layout_id += 1;
        assert!(!matches_saved_revision(&current, &saved));
        current = saved.clone();
        current.serial_number.push_str("replacement");
        assert!(!matches_saved_revision(&current, &saved));
    }

    #[test]
    fn root_device_edits_invalidate_an_older_mapping_even_when_profiles_are_equal() {
        let mut saved = crate::product_preview::registered_preview(756).unwrap();
        saved.fill_defaults();
        saved.source_device_settings = Some(serde_json::json!({"dimLighting": 15}));
        let mut current = saved.clone();
        current.source_device_settings.as_mut().unwrap()["dimLighting"] = 30.into();
        assert_eq!(current.profiles, saved.profiles);
        assert!(!matches_saved_revision(&current, &saved));
        current = saved.clone();
        current.source_device_settings.as_mut().unwrap()["version"] = 1.into();
        assert!(!matches_saved_revision(&current, &saved));
        current = saved.clone();
        current.source_device_settings.as_mut().unwrap()["unrecognizedBusinessField"] = true.into();
        assert!(!matches_saved_revision(&current, &saved));
    }

    #[test]
    fn only_the_verified_writer_version_is_excluded_and_invalid_roots_are_rejected() {
        let mut saved = device();
        saved.source_device_settings = None;
        let mut current = saved.clone();
        current.source_device_settings = Some(serde_json::json!({"version": 1}));
        assert!(matches_saved_revision(&current, &saved));
        for invalid in [
            serde_json::Value::Null,
            serde_json::json!([]),
            serde_json::json!({"version": "1"}),
            serde_json::json!({"version": -1}),
            serde_json::json!({"version": 9_007_199_254_740_992u64}),
        ] {
            current.source_device_settings = Some(invalid);
            assert!(!matches_saved_revision(&current, &saved));
        }
    }
}
