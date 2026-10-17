//! Shared mapping vocabulary and input/website normalization.

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
    ("MSCopilot", "LAUNCH_WINDOWS_COPILOT"),
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
