use super::*;
use serde_json::json;

#[test]
fn source_scope_controls_the_mapping_gate_and_each_required_restore_switch() {
    for (mapping_enabled, global_shortcuts_enabled, expected) in [
        (false, None, vec!["disableMapping"]),
        (true, None, vec!["enableMapping"]),
        (
            false,
            Some(false),
            vec!["disableMapping", "disableGlobalShortcut"],
        ),
        (
            true,
            Some(true),
            vec!["enableMapping", "enableGlobalShortcut"],
        ),
        (
            false,
            Some(true),
            vec!["disableMapping", "enableGlobalShortcut"],
        ),
    ] {
        let target: Target = serde_json::from_value(json!({
            "mapping_enabled":mapping_enabled,
            "global_shortcuts_enabled":global_shortcuts_enabled,
        }))
        .unwrap();
        assert_eq!(
            operations(target).iter().map(symbol).collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn rejected_mapping_restore_still_submits_the_independent_shortcut_restore() {
    let target: Target =
        serde_json::from_value(json!({"mapping_enabled":true,"global_shortcuts_enabled":true}))
            .unwrap();
    let mut calls = Vec::new();
    let error = execute(target, |action| {
        let name = symbol(action);
        calls.push(name.to_owned());
        anyhow::bail!("simulated {name} rejection")
    })
    .unwrap_err();
    assert_eq!(calls, ["enableMapping", "enableGlobalShortcut"]);
    assert!(error.to_string().contains("enableMapping"));
    assert!(error.to_string().contains("enableGlobalShortcut"));
}

#[test]
fn repeated_source_gate_observations_remain_false_while_confirming_the_target() {
    for (mapping, shortcuts, expected) in [
        (
            false,
            None,
            vec![(Action::DisableMapping, "already disabled")],
        ),
        (
            true,
            Some(true),
            vec![
                (Action::EnableMapping, "already enabled"),
                (Action::EnableGlobalShortcuts, "hotkey was not disabled"),
            ],
        ),
    ] {
        let target: Target = serde_json::from_value(
            json!({"mapping_enabled":mapping,"global_shortcuts_enabled":shortcuts}),
        )
        .unwrap();
        let observed = execute(target, |action| {
            let reason = expected
                .iter()
                .find(|(expected, _)| expected == action)
                .unwrap()
                .1;
            Ok(Observation::new(action.clone(), false, reason.into()))
        })
        .unwrap();
        assert_eq!(observed.len(), expected.len());
        for (observation, (action, reason)) in observed.iter().zip(expected) {
            assert!(!observation.result());
            assert_eq!(observation.operation(), &action);
            assert_eq!(observation.reason(), reason);
        }
    }
}

#[test]
fn unrelated_false_results_never_confirm_a_mapping_target() {
    for action in [
        Action::EnableMapping,
        Action::DisableMapping,
        Action::EnableGlobalShortcuts,
        Action::DisableGlobalShortcuts,
    ] {
        for reason in [
            "",
            "access denied",
            "already disabled",
            "already enabled",
            "hotkey was not disabled",
        ] {
            let observation = Observation::new(action.clone(), false, reason.into());
            let expected = matches!(
                (&action, reason),
                (Action::EnableMapping, "already enabled")
                    | (Action::DisableMapping, "already disabled")
                    | (Action::EnableGlobalShortcuts, "hotkey was not disabled")
            );
            assert_eq!(confirmed(&observation), expected);
        }
    }
}
