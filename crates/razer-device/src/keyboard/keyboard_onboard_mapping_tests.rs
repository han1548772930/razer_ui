use super::*;
use crate::onboard_macro::{MacroInventory, MacroMetadata, MacroStorageInfo, MacroSummary};

fn catalog() -> MacroCatalog {
    MacroCatalog {
        inventory: MacroInventory {
            storage: MacroStorageInfo {
                max_macro_id: 1024,
                max_memory: 10000,
                available_memory: 10000,
                recyclable_memory: 0,
            },
            count: 0,
            ids: vec![],
            pages: vec![],
        },
        macros: vec![],
    }
}

fn raw(function_id: u8, mode: u8, data: [u8; 5]) -> ButtonAssignment {
    ButtonAssignment {
        key_id: 31,
        mode,
        function_id,
        data_size: 5,
        data,
    }
}

fn output(function_id: u8, mode: u8, data: [u8; 5]) -> Output {
    decode(&raw(function_id, mode, data), &catalog())
        .unwrap()
        .output
}

#[test]
fn assignment_retains_actual_wire_and_source_identity_without_default_substitution() {
    let wire = raw(0, 1, [9, 8, 7, 6, 5]);
    let assignment = decode(&wire, &catalog()).unwrap();
    assert_eq!(assignment.raw, wire);
    assert_eq!(assignment.input_id, "KEY_A");
    assert_eq!(assignment.mode, Mode::Hypershift);
    assert_eq!(assignment.output, Output::Disabled);
    assert!(matches!(
        default_output("KEY_A", Mode::Normal),
        Some(Output::Keyboard(_))
    ));
    assert!(default_output("missing", Mode::Normal).is_none());
    assert!(decode(&ButtonAssignment { mode: 2, ..wire }, &catalog()).is_err());
    assert!(
        decode(
            &ButtonAssignment {
                key_id: 255,
                ..wire
            },
            &catalog()
        )
        .is_err()
    );
    assert!(
        decode(
            &ButtonAssignment {
                data_size: 6,
                ..wire
            },
            &catalog()
        )
        .is_err()
    );
}

#[test]
fn keyboard_normal_modifiers_and_hypershift_group_resolution_follow_source() {
    let Output::Keyboard(group) = output(2, 0, [0x81, 4, 0, 0, 0]) else {
        panic!("keyboard")
    };
    assert_eq!(
        group.key,
        HidKey {
            hid: 4,
            flag: 0,
            page_id: 7
        }
    );
    assert_eq!(
        group.modifier_names().collect::<Vec<_>>(),
        ["Left_Ctrl", "Right_GUI"]
    );
    assert_eq!(keyboard_input(4, Mode::Normal), Some("KEY_A"));
    let Output::Keyboard(f9) = output(2, 1, [0, 66, 0, 0, 0]) else {
        panic!("keyboard")
    };
    assert_eq!(f9.key.hid, 66);
    assert!(matches!(
        default_output("KEY_F9", Mode::Hypershift),
        Some(Output::Razer(RazerAction::MacroRecordKey))
    ));
    assert!(matches!(
        output(13, 1, [0, 66, 0, 100, 0]),
        Output::Unknown {
            reason: UnknownReason::KeyboardGroup {
                hid: 66,
                mode: Mode::Hypershift
            },
            ..
        }
    ));
    let Output::Keyboard(extended) = output(2, 0, [1, 231, 0, 0, 0]) else {
        panic!("keyboard")
    };
    assert!(extended.extended_modifier);
    assert_eq!(extended.extended_key, Some(ExtendedKey::Extended00));
    assert_eq!(extended.modifiers, 1);
}

#[test]
fn nkro_modifier_only_does_not_invent_right_gui_or_combination_switch_cases() {
    for (bit, hid) in [
        (1, 224),
        (2, 225),
        (4, 226),
        (8, 227),
        (16, 228),
        (32, 229),
        (64, 230),
    ] {
        let Output::Keyboard(group) = output(2, 0, [bit, 0, 0, 0, 0]) else {
            panic!("modifier")
        };
        assert_eq!(group.key.hid, hid);
        assert_eq!(group.modifiers, 0);
    }
    for bit in [3, 128, 255] {
        assert!(matches!(
            output(2, 0, [bit, 0, 0, 0, 0]),
            Output::Unknown {
                reason: UnknownReason::KeyboardHid { hid: 0, .. },
                ..
            }
        ));
    }
}

#[test]
fn media_power_razer_and_shortcut_codes_use_actual_wire_tables() {
    assert_eq!(
        output(10, 0, [2, 159, 0, 0, 0]),
        Output::Media(MediaAction::CycleApps)
    );
    assert_eq!(
        output(10, 0, [0, 205, 0, 0, 0]),
        Output::Media(MediaAction::Play)
    );
    assert_eq!(
        output(9, 0, [130, 0, 0, 0, 0]),
        Output::Power(PowerAction::SystemSleep)
    );
    assert_eq!(
        output(17, 0, [117, 0, 0, 0, 0]),
        Output::Razer(RazerAction::SnapTapToggle)
    );
    assert_eq!(
        output(21, 0, [100, 0, 0, 0, 0]),
        Output::WindowsShortcut(ShortcutAction::XboxGameBar)
    );
    for (function, family) in [
        (10, ActionFamily::Media),
        (9, ActionFamily::Power),
        (17, ActionFamily::Razer),
        (21, ActionFamily::WindowsShortcut),
    ] {
        assert!(
            matches!(output(function,0,[255,255,0,0,0]),Output::Unknown {
            reason:UnknownReason::Action {family:actual,..},..
        } if actual==family)
        );
    }
}

#[test]
fn profiles_and_dpi_clutch_keep_big_endian_coordinates_and_absent_zero_fields() {
    assert_eq!(
        output(7, 0, [3, 6, 0, 0, 0]),
        Output::Profile(ProfileNavigation {
            action: ProfileAction::Specific,
            slot_id: Some(6)
        })
    );
    assert_eq!(
        output(7, 0, [3, 0, 0, 0, 0]),
        Output::Profile(ProfileNavigation {
            action: ProfileAction::Specific,
            slot_id: None
        })
    );
    assert_eq!(
        output(7, 0, [1, 6, 0, 0, 0]),
        Output::Profile(ProfileNavigation {
            action: ProfileAction::Up,
            slot_id: None
        })
    );
    assert_eq!(
        output(6, 0, [5, 0x12, 0x34, 0x56, 0x78]),
        Output::Dpi(DpiOutput {
            action: DpiAction::Clutch,
            x: Some(0x1234),
            y: Some(0x5678)
        })
    );
    assert_eq!(
        output(6, 0, [5, 0, 0, 0, 0]),
        Output::Dpi(DpiOutput {
            action: DpiAction::Clutch,
            x: None,
            y: None
        })
    );
    assert!(matches!(
        output(6, 0, [255, 0, 0, 0, 0]),
        Output::Unknown {
            reason: UnknownReason::Action {
                family: ActionFamily::Dpi,
                code: 255
            },
            ..
        }
    ));
}

#[test]
fn macros_resolve_only_actual_catalog_identity_and_preserve_repeat_zero() {
    assert_eq!(
        output(3, 0, [1, 2, 0, 0, 0]),
        Output::Macro(MacroOutput {
            macro_id: 258,
            guid: None,
            playback: MacroPlayback::NTimes(0)
        })
    );
    let mut macros = catalog();
    macros.macros.push(MacroSummary {
        metadata: MacroMetadata {
            macro_id: 258,
            guid: "observed-guid".into(),
            num_items: 0,
            name: "Actual".into(),
            extra_info: [0; 16],
            timestamp: String::new(),
            hash: String::new(),
            raw: vec![],
            blocks: vec![],
        },
        data_size: 0,
        allocated_size_with_name: 0,
    });
    for (function, playback) in [
        (3, MacroPlayback::Once),
        (4, MacroPlayback::Held),
        (5, MacroPlayback::Toggle),
    ] {
        assert_eq!(
            decode(&raw(function, 0, [1, 2, 1, 0, 0]), &macros)
                .unwrap()
                .output,
            Output::Macro(MacroOutput {
                macro_id: 258,
                guid: Some("observed-guid".into()),
                playback
            })
        );
    }
}

#[test]
fn mouse_and_turbo_keep_source_rounding_and_zero_period_unobserved_rate() {
    let Output::Mouse(mouse) = output(14, 1, [4, 1, 144, 0, 0]) else {
        panic!("mouse")
    };
    assert_eq!(mouse.action, MouseAction::Previous);
    assert_eq!(
        mouse.turbo,
        Some(Turbo {
            period_ms: 400,
            keys_per_second: Some(3)
        })
    );
    let Output::Keyboard(key) = output(13, 0, [0, 4, 0, 0, 0]) else {
        panic!("keyboard")
    };
    assert_eq!(
        key.turbo,
        Some(Turbo {
            period_ms: 0,
            keys_per_second: None
        })
    );
    assert_eq!(
        output(11, 0, [255, 0, 0, 0, 0]),
        Output::Mouse(MouseOutput {
            action: MouseAction::DoubleClick,
            turbo: None
        })
    );
    assert!(matches!(
        output(1, 0, [255, 0, 0, 0, 0]),
        Output::Unknown {
            reason: UnknownReason::MouseButton { id: 255, .. },
            ..
        }
    ));
}

#[test]
fn undefined_source_branches_remain_unknown_with_complete_raw_fields() {
    for (function, reason) in [
        (8, UnknownReason::SourceHasNoDecoder),
        (15, UnknownReason::SourceHasNoDecoder),
        (16, UnknownReason::ControllerCatalogAbsent),
        (18, UnknownReason::ScrollWheelModeDisabled),
        (255, UnknownReason::Function),
    ] {
        let wire = raw(function, 0, [0xde, 0xad, 0xbe, 0xef, 0xab]);
        let assignment = decode(&wire, &catalog()).unwrap();
        assert_eq!(assignment.raw, wire);
        assert!(matches!(assignment.output,Output::Unknown {reason:actual,..} if actual==reason));
    }
    assert_eq!(output(12, 0, [255, 0, 0, 0, 0]), Output::Hypershift);
}
