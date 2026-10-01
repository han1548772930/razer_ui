use super::*;

fn shortcut(input: &str, modifiers: &[&str], hypershift: bool, output: Value) -> Shortcut {
    serde_json::from_value(json!({
        "id":input, "input":input, "modifiers":modifiers,
        "hypershift":hypershift, "output":output,
    }))
    .unwrap()
}

#[test]
fn source_text_vector_preserves_extended_input_release_and_unicode_hash() {
    let shortcut = shortcut(
        "KEY_UP_ARROW",
        &[
            "KEY_LEFT_CTRL",
            "KEY_LEFT_ALT",
            "KEY_LEFT_SHIFT",
            "KEY_LEFT_GUI",
        ],
        true,
        json!({"kind":"text", "text":"<tag>你好😀\n"}),
    );
    // Inputs and output transcribed from 42358/vt/u; digest independently
    // calculated with Node crypto over 19019's canonical UTF-8 representation.
    let expected = json!({
        "mappings":[
            {"input":{"type":"keyboard","scancode":72,"hypershift":true,"flag":2,"modifiers":3840},
             "output":{"type":"clipboard","id":"text","text":"<tag>你好😀\n"}},
            {"input":{"type":"keyboard","scancode":72,"hypershift":true,"flag":3,"modifiers":3840},
             "output":{"type":"disabled"}},
            {"input":{"type":"keyboard","scancode":72,"hypershift":true,"flag":2,"modifiers":3840,"unitId":0},
             "output":{"type":"clipboard","id":"text","text":"<tag>你好😀\n"}},
            {"input":{"type":"keyboard","scancode":72,"hypershift":true,"flag":3,"modifiers":3840,"unitId":0},
             "output":{"type":"disabled"}},
            {"input":{"type":"keyboard","scancode":72,"hypershift":true,"flag":2,"modifiers":3840,"unitId":43981},
             "output":{"type":"clipboard","id":"text","text":"<tag>你好😀\n"}},
            {"input":{"type":"keyboard","scancode":72,"hypershift":true,"flag":3,"modifiers":3840,"unitId":43981},
             "output":{"type":"disabled"}},
        ],
        "hash":"e028f086f335c961aa7ae8becdc94f46",
    });
    assert_eq!(encode_shortcuts(&[shortcut]).unwrap(), expected);
}

#[test]
fn native_hash_uses_stable_keys_top_level_exclusions_and_array_order() {
    assert_eq!(
        encode_shortcuts(&[]).unwrap(),
        json!({"mappings":[],"hash":"de2ccb2014607a84df08306567cd96f0"})
    );
    assert_eq!(
        engine_hash(&json!({"hash":"old", "gamemode":true, "mappings":[]})).unwrap(),
        "de2ccb2014607a84df08306567cd96f0"
    );
    assert_eq!(
        stable_json(&json!({"😀":1,"\u{e000}":2,"a":3})).unwrap(),
        "{\"a\":3,\"😀\":1,\"\u{e000}\":2}"
    );
    assert_ne!(
        engine_hash(&json!({"mappings":[1,2]})).unwrap(),
        engine_hash(&json!({"mappings":[2,1]})).unwrap()
    );
}

#[test]
fn generic_modifier_masks_and_legacy_physical_masks_remain_distinct() {
    let output = json!({"kind":"text","text":"x"});
    for (modifiers, mask) in [
        (vec!["CTRL", "ALT", "SHIFT", "GUI"], 15),
        (
            vec![
                "KEY_LEFT_CTRL",
                "KEY_LEFT_ALT",
                "KEY_LEFT_SHIFT",
                "KEY_LEFT_GUI",
            ],
            3840,
        ),
        (
            vec![
                "KEY_RIGHT_CTRL",
                "KEY_RIGHT_ALT",
                "KEY_RIGHT_SHIFT",
                "KEY_RIGHT_GUI",
            ],
            240,
        ),
    ] {
        let encoded =
            encode_shortcuts(&[shortcut("KEY_K", &modifiers, false, output.clone())]).unwrap();
        assert_eq!(encoded["mappings"][0]["input"]["modifiers"], mask);
    }
}

#[test]
fn source_mouse_inputs_omit_undefined_data_and_emit_both_edges() {
    // 42358/rt and Dt use button bitfields, not Win32 mouse virtual keys.
    for (input, down, up) in [
        ("RightClick", 4, 8),
        ("ScrollButton", 16, 32),
        ("Button4", 64, 128),
        ("Button5", 256, 512),
    ] {
        let encoded = encode_shortcuts(&[shortcut(
            input,
            &[],
            false,
            json!({"kind":"multimedia","action":"VolumeUp"}),
        )])
        .unwrap();
        let expected = json!([
            {"input":{"type":"mouse","id":down,"hypershift":false,"modifiers":0},
             "output":{"type":"keyboard","scancode":48,"flag":2}},
            {"input":{"type":"mouse","id":up,"hypershift":false,"modifiers":0},
             "output":{"type":"keyboard","scancode":48,"flag":3}},
        ]);
        let mappings = encoded["mappings"].as_array().unwrap();
        assert_eq!(&mappings[..2], expected.as_array().unwrap().as_slice());
        assert_eq!(mappings.len(), 6);
        assert_eq!(mappings[2]["input"]["unitId"], 0);
        assert_eq!(mappings[4]["input"]["unitId"], 43981);
    }
}

#[test]
fn source_special_key_aliases_include_both_native_report_variants() {
    for (input, id) in [
        ("KEY_BACKSPACE", 251),
        ("KEY_B", 252),
        ("KEY_SPACEBAR", 253),
    ] {
        let engine = encode_shortcuts(&[shortcut(
            input,
            &["KEY_LEFT_CTRL"],
            true,
            json!({"kind":"text","text":"x"}),
        )])
        .unwrap();
        let mappings = engine["mappings"].as_array().unwrap();
        assert_eq!(mappings.len(), 12);
        assert_eq!(
            mappings[2]["input"],
            json!({"type":"razerKey","key":id,"hypershift":true,"flag":0,"modifiers":512})
        );
        assert_eq!(mappings[3]["input"]["flag"], 1);
        assert_eq!(mappings[6]["input"]["reportId"], 4);
        assert_eq!(mappings[10]["input"]["reportId"], 8);
        assert!(mappings[6]["input"].get("unitId").is_none());
        assert_eq!(mappings[0]["output"], mappings[10]["output"]);
    }
}

#[test]
fn source_extended_and_international_scancodes_are_preserved() {
    for (input, scan, make, release) in [
        ("KEY_A", 30, 0, 1),
        ("KEY_NUMPAD_ENTER", 28, 2, 3),
        ("KEY_NUMPAD_SLASH", 53, 2, 3),
        ("KEY_PAUSE", 29, 4, 5),
        ("KEY_YEN", 125, 0, 1),
        ("KEY_RO", 115, 0, 1),
        ("KEY_HENKAN", 121, 0, 1),
    ] {
        let encoded = encode_shortcuts(&[shortcut(
            input,
            &[],
            false,
            json!({"kind":"text","text":"x"}),
        )])
        .unwrap();
        assert_eq!(encoded["mappings"][0]["input"]["scancode"], scan);
        assert_eq!(encoded["mappings"][0]["input"]["flag"], make);
        assert_eq!(encoded["mappings"][1]["input"]["flag"], release);
    }
}

#[test]
fn multimedia_uses_audio_repeat_and_source_playback_timing() {
    assert_eq!(
        encode_media("MuteMic").unwrap(),
        (
            json!({"type":"audio","id":"mic","mute":2,"repeat":1}),
            json!({"type":"disabled"})
        )
    );
    assert_eq!(
        encode_media("Play").unwrap(),
        (
            json!({"type":"multi","outputs":[
                {"type":"keyboard","scancode":34,"flag":2},
                {"type":"delay","ms":10},
                {"type":"keyboard","scancode":34,"flag":3},
            ]}),
            json!({"type":"disabled"})
        )
    );
    for (action, _) in super::super::workspace::MEDIA {
        assert!(encode_media(action).is_ok(), "{action}");
    }
}

#[test]
fn launches_match_direct_exe_and_hidden_website_branches() {
    let program = ShortcutOutput::Program {
        target: r"C:\Program Files\Example\Game.exe".into(),
    };
    assert_eq!(
        encode_output(&program).unwrap(),
        (
            json!({"type":"launch", "path":r"C:\Program Files\Example\Game.exe", "pathToCheck":r"C:\Program Files\Example\Game.exe", "startHidden":false}),
            json!({"type":"disabled"}),
        )
    );
    let website = ShortcutOutput::Website {
        target: "https://example.com/?a=1&b=2".into(),
    };
    assert_eq!(
        encode_output(&website).unwrap(),
        (
            json!({"type":"launch", "path":"cmd /c start \"\" \"https://example.com/?a=1&b=2\"", "startHidden":true}),
            json!({"type":"disabled"}),
        )
    );
    assert!(
        encode_output(&ShortcutOutput::Program {
            target: r"C:\Apps\script.bat".into()
        })
        .is_err()
    );
    for target in [
        "https://example.com/\"&calc&",
        "https://example.com/%PATH%",
        "https://example.com/!PATH!",
        "https://example.com/\n",
    ] {
        assert!(
            encode_output(&ShortcutOutput::Website {
                target: target.into()
            })
            .is_err()
        );
    }
}

#[test]
fn windows_preserves_release_actions_and_rejects_unregistered_turbo_references() {
    assert_eq!(
        encode_windows("SwitchApps").unwrap(),
        (
            json!({"type":"multi","outputs":[{"type":"keyboard","scancode":56,"flag":0},{"type":"delay","ms":10},{"type":"keyboard","scancode":15,"flag":0}]}),
            json!({"type":"multi","outputs":[{"type":"keyboard","scancode":15,"flag":1},{"type":"delay","ms":10},{"type":"keyboard","scancode":56,"flag":1}]}),
        )
    );
    assert_eq!(
        encode_windows("Calculator").unwrap().0,
        json!({"type":"launch","path":"calc"})
    );
    assert_eq!(
        encode_windows("LaunchTaskManager").unwrap().0,
        json!({"type":"launch","path":"cmd /c start taskmgr","startHidden":true})
    );
    assert_eq!(
        encode_windows("DisplayBrightnessDown").unwrap().1,
        json!({"type":"display","id":"driverBrightnessStop"})
    );
    for action in [
        "PowerUserMenu",
        "ShowDesktop",
        "CycleApps",
        "CloseApp",
        "Cut",
        "Copy",
        "Paste",
        "WindowsZoomIn",
        "WindowsZoomOut",
        "OfficeZoomIn",
        "OfficeZoomOut",
        "LockComputer",
    ] {
        assert!(encode_windows(action).is_err(), "{action}");
    }
}

#[test]
fn request_is_atomic_and_rejects_scan_alias_collisions() {
    let text = json!({"kind":"text","text":"x"});
    let first = shortcut("KEY_BACKSLASH", &[], false, text.clone());
    let same_scan = shortcut("KEY_NON_US_POUND", &[], false, text);
    assert!(encode_shortcuts(&[first.clone(), same_scan]).is_err());
    assert!(encode_shortcuts(&[first.clone(), first.clone()]).is_err());
    let unsupported = shortcut(
        "KEY_A",
        &[],
        false,
        json!({"kind":"windows","action":"Copy"}),
    );
    assert!(encode_shortcuts(&[first, unsupported]).is_err());
    let invalid = shortcut("KEY_FUTURE", &[], false, json!({"kind":"text","text":"x"}));
    assert!(encode_shortcuts(&[invalid]).is_err());
}
