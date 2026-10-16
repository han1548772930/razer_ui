use super::*;
use crate::keyboard_control_test_transport::{
    Mock, Reply, assert_report_integrity, reply, sequence,
};
use std::cell::Cell;

fn cap() -> KeyboardSettingsCapability {
    let mut cap = capability(708).unwrap().clone();
    cap.transport.max_retry_in = 1;
    cap.transport.max_retry_out = 1;
    cap.transport.sleep_between_out_ms = 0;
    cap.transport.sleep_between_out_in_ms = 0;
    cap.transport.sleep_between_in_ms = 0;
    cap
}

fn configuration() -> KnobConfiguration {
    std::array::from_fn(|index| KnobModeConfig {
        disabled: u8::from(index % 2 == 1),
        stage: index as u8,
        mode_id: (7 - index) as u8,
        red: (index * 17) as u8,
        green: (index * 13) as u8,
        blue: (index * 11) as u8,
    })
}

fn current() -> CurrentKnobMode {
    CurrentKnobMode {
        disabled: 0,
        stage: 5,
        mode_id: 2,
        red: 10,
        green: 20,
        blue: 30,
    }
}

fn configuration_payload(configuration: &KnobConfiguration) -> Vec<u8> {
    let mut payload = vec![1, 39];
    for record in configuration {
        record.encode(&mut payload);
    }
    payload
}

fn current_payload(mode: CurrentKnobMode) -> Vec<u8> {
    vec![
        1,
        39,
        mode.disabled,
        mode.stage,
        mode.mode_id,
        mode.red,
        mode.green,
        mode.blue,
    ]
}

fn ui(uid: &str, is_custom: bool, enabled: bool, mode_id: u8) -> KnobUiMode {
    KnobUiMode {
        uid: uid.into(),
        is_custom,
        enabled,
        mode_id,
        rgb: [mode_id, 2, 3],
    }
}

#[test]
fn compiler_excludes_custom_modes_and_counts_disabled_builtin_stages() {
    let modes = [
        ui("custom", true, true, 50),
        ui("first", false, false, 7),
        ui("second", false, true, 2),
    ];
    let compiled = compile_configuration(&modes).unwrap();
    assert_eq!(
        compiled[0],
        KnobModeConfig {
            disabled: 1,
            stage: 0,
            mode_id: 7,
            red: 7,
            green: 2,
            blue: 3
        }
    );
    assert_eq!(
        compiled[1],
        KnobModeConfig {
            disabled: 0,
            stage: 1,
            mode_id: 2,
            red: 2,
            green: 2,
            blue: 3
        }
    );
    assert_eq!(&compiled[2..], &[ZERO_MODE; 6]);
    let current = compile_current(&modes, "second").unwrap();
    assert_eq!(
        (current.stage, current.mode_id, current.disabled),
        (1, 2, 0)
    );
}

#[test]
fn all_disabled_uses_the_exact_eight_current_product_defaults() {
    for modes in [
        vec![],
        vec![ui("disabled", false, false, 7)],
        vec![ui("custom", true, true, 50)],
    ] {
        assert_eq!(
            compile_configuration(&modes).unwrap(),
            ALL_DISABLED_FALLBACK
        );
    }
    let bytes = configuration_payload(&ALL_DISABLED_FALLBACK);
    assert_eq!(
        &bytes[2..],
        &[
            0, 0, 0, 0, 0, 255, 0, 1, 1, 0, 0, 255, 0, 2, 2, 0, 0, 255, 0, 3, 3, 0, 0, 255, 1, 4,
            4, 64, 224, 208, 1, 5, 5, 255, 0, 255, 1, 6, 6, 255, 255, 255, 1, 7, 7, 255, 165, 0,
        ]
    );
}

#[test]
fn selected_custom_mode_uses_first_enabled_builtin_or_mode_zero_with_original_color() {
    let modes = [
        ui("zero", false, false, 0),
        ui("enabled", false, true, 5),
        ui("custom", true, true, 90),
    ];
    assert_eq!(
        compile_current(&modes, "custom").unwrap(),
        CurrentKnobMode {
            disabled: 0,
            stage: 1,
            mode_id: 5,
            red: 5,
            green: 2,
            blue: 3
        }
    );
    let modes = [
        ui("seven", false, false, 7),
        ui("zero", false, false, 0),
        ui("custom", true, true, 90),
    ];
    assert_eq!(
        compile_current(&modes, "custom").unwrap(),
        CurrentKnobMode {
            disabled: 1,
            stage: 1,
            mode_id: 0,
            red: 0,
            green: 2,
            blue: 3
        }
    );
}

#[test]
fn source_mode_zero_fallback_is_not_assumed_to_be_builtin() {
    let modes = [
        ui("zero-custom", true, false, 0),
        ui("disabled", false, false, 7),
        ui("selected", true, true, 90),
    ];
    let compiled = compile_current(&modes, "selected").unwrap();
    assert_eq!(
        (compiled.stage, compiled.disabled, compiled.mode_id),
        (255, 1, 0)
    );
}

#[test]
fn compiler_rejects_ambiguous_or_unsupported_ui_input() {
    for modes in [
        vec![ui("", false, true, 0)],
        vec![ui("same", false, true, 0), ui("same", true, true, 1)],
        vec![ui("first", false, true, 0), ui("second", false, true, 0)],
        vec![ui("bad", false, true, 8)],
        (0..9)
            .map(|id| ui(&id.to_string(), false, true, id))
            .collect(),
    ] {
        assert!(compile_configuration(&modes).is_err());
        assert!(compile_current(&modes, "bad").is_err());
    }
    assert!(compile_current(&[ui("normal", false, true, 0)], "missing").is_err());
    assert!(compile_current(&[ui("custom", true, true, 90)], "custom").is_err());
}

#[test]
fn complete_write_sends_configuration_then_current_then_independent_gets() {
    let config = configuration();
    let mode = current();
    let device = Mock::new([
        reply(&[]),
        reply(&[200]),
        reply(&configuration_payload(&config)),
        reply(&current_payload(mode)),
    ]);
    let result = apply_with_cap(
        &device,
        &cap(),
        1,
        39,
        Some(&config),
        Some(mode),
        true,
        &mut sequence(),
        &|| Ok(()),
    )
    .unwrap();
    assert!(result.verified());
    assert_eq!(result.configuration.send_attempts, 1);
    assert_eq!(result.current_mode.send_attempts, 1);
    assert_eq!(result.observed_configuration, Some(config));
    assert_eq!(result.observed_current_mode, Some(mode));
    assert_eq!(result.current_mode.acknowledgment_data, Some(vec![200]));
    let sent = device.sent();
    assert_eq!(sent.len(), 4);
    assert_eq!(&sent[0][6..9], &[50, 3, 18]);
    assert_eq!(&sent[0][9..59], configuration_payload(&config));
    assert_eq!(&sent[1][6..9], &[8, 3, 19]);
    assert_eq!(&sent[1][9..17], current_payload(mode));
    assert_eq!(&sent[2][6..11], &[50, 3, 146, 1, 39]);
    assert_eq!(&sent[3][6..11], &[8, 3, 147, 1, 39]);
    assert_eq!(
        sent.iter().map(|report| report[2]).collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    assert_report_integrity(&sent);
}

#[test]
fn public_read_queries_both_fields_for_the_one_current_obm_slot() {
    let config = configuration();
    let mode = current();
    let device = Mock::new([
        reply(&configuration_payload(&config)),
        reply(&current_payload(mode)),
    ]);
    let result = read(&device, 708, 1, 39, sequence(), || Ok(())).unwrap();
    assert_eq!((result.profile_id, result.knob_id), (1, 39));
    assert_eq!(result.configuration, config);
    assert_eq!(result.current_mode, mode);
    assert_eq!(device.sent().len(), 2);
}

#[test]
fn individual_reads_preserve_unknown_actual_record_values() {
    let mut config = configuration();
    config[4] = KnobModeConfig {
        disabled: 255,
        stage: 255,
        mode_id: 255,
        red: 255,
        green: 255,
        blue: 255,
    };
    let unknown = CurrentKnobMode::from(config[4]);
    let device = Mock::new([
        reply(&configuration_payload(&config)),
        reply(&current_payload(unknown)),
    ]);
    assert_eq!(
        read_configuration(&device, 708, 1, 39, 29, || Ok(())).unwrap(),
        config
    );
    assert_eq!(
        read_current(&device, 708, 1, 39, 30, || Ok(())).unwrap(),
        unknown
    );
    assert_eq!(
        device
            .sent()
            .iter()
            .map(|report| report[2])
            .collect::<Vec<_>>(),
        [29, 30]
    );
}

#[test]
fn configuration_only_writes_and_confirms_only_configuration() {
    let config = configuration();
    let device = Mock::new([reply(&[]), reply(&configuration_payload(&config))]);
    let result = apply_with_cap(
        &device,
        &cap(),
        1,
        39,
        Some(&config),
        None,
        true,
        &mut sequence(),
        &|| Ok(()),
    )
    .unwrap();
    assert!(result.configuration.attempted);
    assert!(!result.current_mode.attempted);
    assert!(result.verified());
    assert_eq!(result.observed_configuration, Some(config));
    assert!(result.observed_current_mode.is_none());
    assert_eq!(
        device
            .sent()
            .iter()
            .map(|report| report[8])
            .collect::<Vec<_>>(),
        [18, 146]
    );
}

#[test]
fn current_only_writes_and_confirms_only_current() {
    let mode = current();
    let device = Mock::new([reply(&[]), reply(&current_payload(mode))]);
    let result = apply_with_cap(
        &device,
        &cap(),
        1,
        39,
        None,
        Some(mode),
        true,
        &mut sequence(),
        &|| Ok(()),
    )
    .unwrap();
    assert!(!result.configuration.attempted);
    assert!(result.current_mode.attempted);
    assert!(result.verified());
    assert!(result.observed_configuration.is_none());
    assert_eq!(result.observed_current_mode, Some(mode));
    assert_eq!(
        device
            .sent()
            .iter()
            .map(|report| report[8])
            .collect::<Vec<_>>(),
        [19, 147]
    );
}

#[test]
fn ack_only_mode_never_claims_observation_even_when_echoes_match() {
    let config = configuration();
    let mode = current();
    let device = Mock::new([
        reply(&configuration_payload(&config)),
        reply(&current_payload(mode)),
    ]);
    let result = apply_with_cap(
        &device,
        &cap(),
        1,
        39,
        Some(&config),
        Some(mode),
        false,
        &mut sequence(),
        &|| Ok(()),
    )
    .unwrap();
    assert!(result.configuration.acknowledgment_data.is_some());
    assert!(result.current_mode.acknowledgment_data.is_some());
    assert!(result.observed_configuration.is_none());
    assert!(result.observed_current_mode.is_none());
    assert!(!result.verified());
    assert_eq!(device.sent().len(), 2);
}

#[test]
fn empty_change_or_wrong_product_slot_or_knob_fails_before_send() {
    let device = Mock::new([]);
    for (pid, profile_id, knob_id) in [
        (708, 0, 39),
        (708, 2, 39),
        (708, 1, 0),
        (708, 1, 38),
        (706, 1, 39),
        (707, 1, 39),
        (717, 1, 39),
        (730, 1, 39),
        (0, 1, 39),
    ] {
        assert!(read(&device, pid, profile_id, knob_id, sequence(), || Ok(())).is_err());
        let error = apply(
            &device,
            pid,
            profile_id,
            knob_id,
            Some(&configuration()),
            Some(current()),
            true,
            sequence(),
            || Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.phase, KnobFailurePhase::Prepare);
        assert!(!error.report.configuration.attempted);
        assert!(!error.report.current_mode.attempted);
    }
    let error = apply_with_cap(
        &device,
        &cap(),
        1,
        39,
        None,
        None,
        true,
        &mut sequence(),
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.phase, KnobFailurePhase::Prepare);
    assert!(device.sent().is_empty());
}

#[test]
fn rejected_configuration_stops_current_mode_and_all_readback() {
    for status in [0, 3, 4, 5, 255] {
        let device = Mock::new([Reply {
            status,
            ..reply(&[])
        }]);
        let error = apply_with_cap(
            &device,
            &cap(),
            1,
            39,
            Some(&configuration()),
            Some(current()),
            true,
            &mut sequence(),
            &|| Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.phase, KnobFailurePhase::ConfigurationWrite);
        assert!(error.report.configuration.attempted);
        assert!(error.report.configuration.acknowledgment_data.is_none());
        assert!(!error.report.current_mode.attempted);
        assert_eq!(device.sent().len(), 1);
    }
}

#[test]
fn rejected_current_mode_preserves_configuration_ack_without_rollback() {
    let device = Mock::new([
        reply(&[]),
        Reply {
            status: 5,
            ..reply(&[])
        },
    ]);
    let error = apply_with_cap(
        &device,
        &cap(),
        1,
        39,
        Some(&configuration()),
        Some(current()),
        true,
        &mut sequence(),
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.phase, KnobFailurePhase::CurrentModeWrite);
    assert_eq!(error.report.configuration.acknowledgment_data, Some(vec![]));
    assert!(error.report.current_mode.attempted);
    assert!(error.report.current_mode.acknowledgment_data.is_none());
    assert!(!error.report.verified());
    assert_eq!(device.sent().len(), 2);
}

#[test]
fn mismatched_configuration_readback_preserves_both_write_acks_and_observation() {
    let mut changed = configuration();
    changed[0].blue ^= 1;
    let device = Mock::new([
        reply(&[]),
        reply(&[]),
        reply(&configuration_payload(&changed)),
    ]);
    let error = apply_with_cap(
        &device,
        &cap(),
        1,
        39,
        Some(&configuration()),
        Some(current()),
        true,
        &mut sequence(),
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.phase, KnobFailurePhase::ConfigurationReadback);
    assert_eq!(error.report.observed_configuration, Some(changed));
    assert!(error.report.observed_current_mode.is_none());
    assert_eq!(error.report.configuration.acknowledgment_data, Some(vec![]));
    assert_eq!(error.report.current_mode.acknowledgment_data, Some(vec![]));
    assert!(!error.report.verified());
    assert_eq!(device.sent().len(), 3);
}

#[test]
fn mismatched_current_readback_preserves_configuration_confirmation() {
    let config = configuration();
    let mut changed = current();
    changed.stage ^= 1;
    let device = Mock::new([
        reply(&[]),
        reply(&[]),
        reply(&configuration_payload(&config)),
        reply(&current_payload(changed)),
    ]);
    let error = apply_with_cap(
        &device,
        &cap(),
        1,
        39,
        Some(&config),
        Some(current()),
        true,
        &mut sequence(),
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.phase, KnobFailurePhase::CurrentModeReadback);
    assert_eq!(error.report.observed_configuration, Some(config));
    assert_eq!(error.report.observed_current_mode, Some(changed));
    assert!(!error.report.verified());
    assert_eq!(device.sent().len(), 4);
}

#[test]
fn truncated_or_wrong_identity_configuration_reads_are_rejected() {
    let good = configuration_payload(&configuration());
    let mut wrong_slot = good.clone();
    wrong_slot[0] = 2;
    let mut wrong_knob = good.clone();
    wrong_knob[1] = 38;
    for data in [vec![], good[..49].to_vec(), wrong_slot, wrong_knob] {
        let device = Mock::new([reply(&data)]);
        assert!(read_configuration_with_cap(&device, &cap(), 1, 39, 0, &|| Ok(())).is_err());
        assert_eq!(device.sent().len(), 1);
    }
}

#[test]
fn truncated_or_wrong_identity_current_reads_are_rejected() {
    let good = current_payload(current());
    let mut wrong_slot = good.clone();
    wrong_slot[0] = 0;
    let mut wrong_knob = good.clone();
    wrong_knob[1] = 40;
    for data in [vec![], good[..7].to_vec(), wrong_slot, wrong_knob] {
        let device = Mock::new([reply(&data)]);
        assert!(read_current_with_cap(&device, &cap(), 1, 39, 0, &|| Ok(())).is_err());
        assert_eq!(device.sent().len(), 1);
    }
}

#[test]
fn failed_independent_configuration_readback_never_starts_current_readback() {
    let device = Mock::new([
        reply(&[]),
        reply(&[]),
        Reply {
            status: 5,
            ..reply(&[])
        },
    ]);
    let error = apply_with_cap(
        &device,
        &cap(),
        1,
        39,
        Some(&configuration()),
        Some(current()),
        true,
        &mut sequence(),
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.phase, KnobFailurePhase::ConfigurationReadback);
    assert!(error.report.configuration.acknowledgment_data.is_some());
    assert!(error.report.current_mode.acknowledgment_data.is_some());
    assert!(error.report.observed_configuration.is_none());
    assert!(!error.report.verified());
    assert_eq!(device.sent().len(), 3);
}

#[test]
fn failed_second_sequence_preserves_ack_without_claiming_a_second_write() {
    let device = Mock::new([reply(&[])]);
    let count = Cell::new(0);
    let error = apply_with_cap(
        &device,
        &cap(),
        1,
        39,
        Some(&configuration()),
        Some(current()),
        true,
        &mut || {
            count.set(count.get() + 1);
            ensure!(count.get() == 1, "sequence source unavailable");
            Ok(0)
        },
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.phase, KnobFailurePhase::CurrentModeWrite);
    assert!(error.report.configuration.acknowledgment_data.is_some());
    assert!(!error.report.current_mode.attempted);
    assert_eq!(device.sent().len(), 1);
}

#[test]
fn invalid_transactions_and_forged_capability_never_send() {
    let device = Mock::new([]);
    for sequence in [31, 32, 128, 255] {
        assert!(read_current_with_cap(&device, &cap(), 1, 39, sequence, &|| Ok(())).is_err());
        let error = apply_with_cap(
            &device,
            &cap(),
            1,
            39,
            None,
            Some(current()),
            false,
            &mut || Ok(sequence),
            &|| Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.phase, KnobFailurePhase::CurrentModeWrite);
        assert!(!error.report.current_mode.attempted);
    }
    let mut forged = cap();
    forged.transport.claim_interface = 2;
    let error = apply_with_cap(
        &device,
        &forged,
        1,
        39,
        Some(&configuration()),
        Some(current()),
        false,
        &mut sequence(),
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.phase, KnobFailurePhase::Prepare);
    assert!(!error.report.configuration.attempted);
    assert!(device.sent().is_empty());
}

#[test]
fn cancellation_before_write_and_between_writes_preserves_real_progress() {
    let device = Mock::new([]);
    let error = apply_with_cap(
        &device,
        &cap(),
        1,
        39,
        Some(&configuration()),
        Some(current()),
        true,
        &mut sequence(),
        &|| anyhow::bail!("cancelled"),
    )
    .unwrap_err();
    assert_eq!(error.phase, KnobFailurePhase::Prepare);
    assert!(!error.report.configuration.attempted);
    assert!(device.sent().is_empty());
    let device = Mock::new([reply(&[])]);
    let validations = Cell::new(0);
    let error = apply_with_cap(
        &device,
        &cap(),
        1,
        39,
        Some(&configuration()),
        Some(current()),
        true,
        &mut sequence(),
        &|| {
            validations.set(validations.get() + 1);
            ensure!(validations.get() < 6, "cancel after configuration ACK");
            Ok(())
        },
    )
    .unwrap_err();
    assert_eq!(error.phase, KnobFailurePhase::CurrentModeWrite);
    assert_eq!(error.report.configuration.acknowledgment_data, Some(vec![]));
    assert!(!error.report.current_mode.attempted);
    assert!(!error.report.verified());
    assert_eq!(device.sent().len(), 1);
}

#[test]
fn configuration_only_cancel_after_ack_identifies_configuration_phase() {
    let device = Mock::new([reply(&[])]);
    let validations = Cell::new(0);
    let error = apply_with_cap(
        &device,
        &cap(),
        1,
        39,
        Some(&configuration()),
        None,
        true,
        &mut sequence(),
        &|| {
            validations.set(validations.get() + 1);
            ensure!(validations.get() < 6, "cancel after configuration ACK");
            Ok(())
        },
    )
    .unwrap_err();
    assert_eq!(error.phase, KnobFailurePhase::ConfigurationWrite);
    assert_eq!(error.report.configuration.acknowledgment_data, Some(vec![]));
    assert!(!error.report.verified());
}

#[test]
fn retry_send_progress_and_busy_reads_do_not_fabricate_extra_writes() {
    let mut cap = cap();
    cap.transport.max_retry_out = 2;
    let device = Mock::new([reply(&[])]).fail_sends(1);
    let result = apply_with_cap(
        &device,
        &cap,
        1,
        39,
        None,
        Some(current()),
        false,
        &mut sequence(),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(result.current_mode.send_attempts, 2);
    assert_eq!(device.sent()[0], device.sent()[1]);
    cap.transport.max_retry_in = 2;
    let device = Mock::new([
        Reply {
            status: 1,
            ..reply(&[])
        },
        reply(&current_payload(current())),
    ]);
    assert_eq!(
        read_current_with_cap(&device, &cap, 1, 39, 0, &|| Ok(())).unwrap(),
        current()
    );
    assert_eq!(device.sent().len(), 1);
}

#[test]
fn malformed_report_and_correlation_are_rejected() {
    for getter in [
        Reply {
            count: 90,
            ..reply(&current_payload(current()))
        },
        Reply {
            declared_length: Some(81),
            ..reply(&current_payload(current()))
        },
        Reply {
            wrong_transaction: true,
            ..reply(&current_payload(current()))
        },
        Reply {
            wrong_command: true,
            ..reply(&current_payload(current()))
        },
    ] {
        let device = Mock::new([getter]);
        assert!(read_current_with_cap(&device, &cap(), 1, 39, 0, &|| Ok(())).is_err());
    }
}

#[test]
fn returned_prefix_is_stripped_before_knob_decoding() {
    let device = Mock::new([Reply { report_id: 1, ..reply(&current_payload(current())) }]);
    let observed = read_current_with_cap(&device, &cap(), 1, 39, 0, &|| Ok(())).unwrap();
    assert_eq!(observed, current());
    assert_eq!(device.sent().len(), 1);
}
