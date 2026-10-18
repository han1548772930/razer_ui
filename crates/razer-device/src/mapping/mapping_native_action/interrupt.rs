use super::Output;
use crate::mapping_native_lookup::{InputKind, Resolved, ResolvedOutput};

/// Matches the retained macro supplied by 0x8dd0e/0x8e394 to StopExecution.
/// The caller separately matches the connected device, as 0x5d890 does.
pub fn matches_interrupted_macro(left: &Resolved<Output>, right: &Resolved<Output>) -> bool {
    // 0xf9f62 / 0xf9dd6 / 0xf8dde retain every selector and both mode flags.
    if left.mapping_enabled != right.mapping_enabled
        || left.global_shortcut != right.global_shortcut
        || left.selectors != right.selectors
    {
        return false;
    }
    // 0x101d90 compares the GUID, execution flag and repeat count, not just GUID.
    if !matches!(left.output, ResolvedOutput::Mapped(Output::Macro { .. }))
        || left.output != right.output
    {
        return false;
    }
    // Native input equality excludes the base interrupt list. Each concrete
    // input compares only its own fields, including optional-value presence.
    match (&left.input.kind, &right.input.kind) {
        // 0x10c6a0 / 0x1000f0.
        (InputKind::AnalogKey { .. }, InputKind::AnalogKey { .. })
        | (InputKind::External { .. }, InputKind::External { .. }) => {
            left.input.kind == right.input.kind && left.input.modifiers == right.input.modifiers
        }
        // 0x10ccc0.
        (InputKind::Keyboard { .. }, InputKind::Keyboard { .. }) => {
            left.input.kind == right.input.kind
                && left.input.modifiers == right.input.modifiers
                && left.input.delay_ms == right.input.delay_ms
                && left.input.release_after_ms == right.input.release_after_ms
                && left.input.unit_id == right.input.unit_id
        }
        // 0x10d990.
        (InputKind::Mouse { .. }, InputKind::Mouse { .. }) => {
            left.input.kind == right.input.kind
                && left.input.modifiers == right.input.modifiers
                && left.input.unit_id == right.input.unit_id
        }
        // 0x103ae0.
        (InputKind::RazerKey { .. }, InputKind::RazerKey { .. }) => {
            left.input.kind == right.input.kind
                && left.input.modifiers == right.input.modifiers
                && left.input.report_id == right.input.report_id
        }
        // 0xfc230 / 0x1040e0.
        (InputKind::AnalogStick { .. }, InputKind::AnalogStick { .. })
        | (InputKind::StreamController { .. }, InputKind::StreamController { .. }) => {
            left.input.kind == right.input.kind
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapping_native_lookup::{Input, Selectors};

    fn mapping() -> Resolved<Output> {
        Resolved {
            input: Input::new(InputKind::AnalogKey {
                scancode: 30,
                flag: 0,
            }),
            output: ResolvedOutput::Mapped(Output::Macro {
                guid: "macro-a".into(),
                flag: 0,
                repeat: 2,
            }),
            selectors: Selectors::default(),
            mapping_enabled: true,
            global_shortcut: false,
        }
    }

    #[test]
    fn analog_macro_matching_ignores_interrupt_registration_metadata() {
        let left = mapping();
        let mut right = left.clone();
        right.input.interruptible = true;
        right.input.interrupts.push(Input::new(InputKind::Keyboard {
            scancode: 31,
            flag: 0,
        }));
        assert!(matches_interrupted_macro(&left, &right));
        assert!(matches_interrupted_macro(&right, &left));
        right.input.modifiers = 1;
        assert!(!matches_interrupted_macro(&left, &right));
    }

    #[test]
    fn analog_macro_matching_retains_output_and_input_identity() {
        let left = mapping();
        for output in [
            Output::Macro {
                guid: "macro-b".into(),
                flag: 0,
                repeat: 2,
            },
            Output::Macro {
                guid: "macro-a".into(),
                flag: 1,
                repeat: 2,
            },
            Output::Macro {
                guid: "macro-a".into(),
                flag: 0,
                repeat: 3,
            },
            Output::Disabled,
        ] {
            let mut right = left.clone();
            right.output = ResolvedOutput::Mapped(output);
            assert!(!matches_interrupted_macro(&left, &right));
        }
        for kind in [
            InputKind::AnalogKey {
                scancode: 31,
                flag: 0,
            },
            InputKind::AnalogKey {
                scancode: 30,
                flag: 1,
            },
            InputKind::Keyboard {
                scancode: 30,
                flag: 0,
            },
        ] {
            let mut right = left.clone();
            right.input.kind = kind;
            assert!(!matches_interrupted_macro(&left, &right));
        }
    }

    #[test]
    fn analog_macro_matching_retains_all_selectors_and_mode_flags() {
        let left = mapping();
        for selectors in [
            Selectors {
                hypershift: true,
                ..Selectors::default()
            },
            Selectors {
                gaming: true,
                ..Selectors::default()
            },
            Selectors {
                otfs: true,
                ..Selectors::default()
            },
            Selectors {
                dynamic_key: Some(0),
                ..Selectors::default()
            },
            Selectors {
                snap_tap: Some(0),
                ..Selectors::default()
            },
            Selectors {
                snap_tap_prioritize: true,
                ..Selectors::default()
            },
        ] {
            let mut right = left.clone();
            right.selectors = selectors;
            assert!(!matches_interrupted_macro(&left, &right));
        }
        let mut right = left.clone();
        right.mapping_enabled = false;
        assert!(!matches_interrupted_macro(&left, &right));
        right = left.clone();
        right.global_shortcut = true;
        assert!(!matches_interrupted_macro(&left, &right));
    }

    #[test]
    fn razer_macro_matching_retains_report_presence_value_and_modifiers() {
        let mut left = mapping();
        left.input.kind = InputKind::RazerKey { key: 7, flag: 0 };
        let mut right = left.clone();
        right.input.report_id = Some(0);
        assert!(!matches_interrupted_macro(&left, &right));
        left.input.report_id = Some(0);
        right.input.interruptible = true;
        assert!(matches_interrupted_macro(&left, &right));
        right.input.report_id = Some(1);
        assert!(!matches_interrupted_macro(&left, &right));
        right = left.clone();
        right.input.modifiers = 1;
        assert!(!matches_interrupted_macro(&left, &right));
        right = left.clone();
        right.input.kind = InputKind::RazerKey { key: 7, flag: 1 };
        assert!(!matches_interrupted_macro(&left, &right));
    }

    #[test]
    fn keyboard_macro_matching_retains_optional_timing_and_unit_fields() {
        let mut left = mapping();
        left.input.kind = InputKind::Keyboard {
            scancode: 30,
            flag: 0,
        };
        let mut right = left.clone();
        right.input.interruptible = true;
        right.input.interrupts.push(Input::new(InputKind::Keyboard {
            scancode: 31,
            flag: 0,
        }));
        assert!(matches_interrupted_macro(&left, &right));
        for field in 0..4 {
            let mut right = left.clone();
            match field {
                0 => right.input.delay_ms = Some(0),
                1 => right.input.release_after_ms = Some(0),
                2 => right.input.unit_id = Some(0),
                _ => right.input.modifiers = 1,
            }
            assert!(!matches_interrupted_macro(&left, &right));
        }
        left.input.delay_ms = Some(10);
        right = left.clone();
        right.input.delay_ms = Some(20);
        assert!(!matches_interrupted_macro(&left, &right));
    }

    #[test]
    fn mouse_macro_matching_retains_data_unit_and_modifiers() {
        let mut left = mapping();
        left.input.kind = InputKind::Mouse { id: 1, data: 0 };
        let mut right = left.clone();
        right.input.interruptible = true;
        assert!(matches_interrupted_macro(&left, &right));
        right.input.unit_id = Some(0);
        assert!(!matches_interrupted_macro(&left, &right));
        right = left.clone();
        right.input.kind = InputKind::Mouse { id: 1, data: 1 };
        assert!(!matches_interrupted_macro(&left, &right));
        right = left.clone();
        right.input.modifiers = 1;
        assert!(!matches_interrupted_macro(&left, &right));
    }

    #[test]
    fn external_macro_matching_keeps_modifiers_separate_from_lookup_modifiers() {
        let mut left = mapping();
        left.input.kind = InputKind::External { key: 7, flag: 0 };
        let mut right = left.clone();
        right.input.interruptible = true;
        assert!(matches_interrupted_macro(&left, &right));
        right.input.modifiers = 1;
        assert_eq!(
            left.input.lookup_modifiers(),
            right.input.lookup_modifiers()
        );
        assert!(!matches_interrupted_macro(&left, &right));
    }

    #[test]
    fn stream_and_stick_macros_compare_concrete_input_edges() {
        for kind in [
            InputKind::StreamController { key: 7, flag: 0 },
            InputKind::AnalogStick { axis: 1, flag: 0 },
        ] {
            let mut left = mapping();
            left.input.kind = kind;
            let mut right = left.clone();
            right.input.interruptible = true;
            assert!(matches_interrupted_macro(&left, &right));
            right.input = right.input.released();
            assert!(!matches_interrupted_macro(&left, &right));
        }
    }
}
