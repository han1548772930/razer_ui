use super::*;
use crate::keyboard_control_test_transport::{
    Mock, Reply, assert_report_integrity, reply, sequence,
};
use std::cell::Cell;

fn cap() -> KeyboardSettingsCapability {
    let mut cap = capability(717).unwrap().clone();
    cap.transport.max_retry_in = 1;
    cap.transport.max_retry_out = 1;
    cap.transport.sleep_between_out_ms = 0;
    cap.transport.sleep_between_out_in_ms = 0;
    cap.transport.sleep_between_in_ms = 0;
    cap
}

#[test]
fn audited_joro_set_and_get_use_multimedia_zero_and_function_one() {
    assert_eq!(AUDITED_WIRED_PRODUCTS, [717]);
    for primary in [FunctionKeyPrimary::Multimedia, FunctionKeyPrimary::Function] {
        let cap = cap();
        let device = Mock::new([reply(&[255]), reply(&[primary.swap_byte()])]);
        let result =
            apply_with_cap(&device, &cap, primary, true, &mut sequence(), &|| Ok(())).unwrap();
        assert!(result.attempted);
        assert_eq!(result.send_attempts, 1);
        assert_eq!(result.acknowledgment_data, Some(vec![255]));
        assert_eq!(result.observed.unwrap().primary(), Some(primary));
        assert!(result.verified());
        let sent = device.sent();
        assert_eq!(sent.len(), 2);
        assert_eq!(&sent[0][6..10], &[1, 2, 40, primary.swap_byte()]);
        assert_eq!(&sent[1][6..10], &[1, 2, 168, 0]);
        assert_eq!((sent[0][2], sent[1][2]), (0, 1));
        assert_report_integrity(&sent);
    }
}

#[test]
fn public_wired_getter_accepts_max_transaction_and_preserves_unknown_state() {
    let device = Mock::new([reply(&[200])]);
    let observed = read(&device, 717, 30, || Ok(())).unwrap();
    assert_eq!(observed.raw_swap, 200);
    assert_eq!(observed.primary(), None);
    assert_eq!(device.sent()[0][2], 30);
}

#[test]
fn pro_type_single_profile_receiver_ble_and_unregistered_products_do_not_send() {
    let device = Mock::new([]);
    for product_id in [708, 706, 707, 718, 730, 600, 756, 0] {
        assert!(read(&device, product_id, 0, || Ok(())).is_err());
        let error = apply(
            &device,
            product_id,
            FunctionKeyPrimary::Function,
            true,
            sequence(),
            || Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.phase, SwapFailurePhase::Prepare);
        assert!(!error.report.attempted);
        assert_eq!(error.report.send_attempts, 0);
    }
    assert!(device.sent().is_empty());
}

#[test]
fn setter_ack_payload_is_never_a_readback_or_a_verified_state() {
    for ack in [vec![], vec![1], vec![0, 99]] {
        let device = Mock::new([reply(&ack)]);
        let result = apply_with_cap(
            &device,
            &cap(),
            FunctionKeyPrimary::Function,
            false,
            &mut sequence(),
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(result.acknowledgment_data, Some(ack));
        assert!(result.observed.is_none());
        assert!(!result.verified());
        assert_eq!(device.sent().len(), 1);
    }
}

#[test]
fn failed_setter_retains_attempt_and_never_reads() {
    for status in [0, 3, 4, 5, 255] {
        let device = Mock::new([Reply {
            status,
            ..reply(&[])
        }]);
        let error = apply_with_cap(
            &device,
            &cap(),
            FunctionKeyPrimary::Function,
            true,
            &mut sequence(),
            &|| Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.phase, SwapFailurePhase::Write);
        assert!(error.report.attempted);
        assert_eq!(error.report.send_attempts, 1);
        assert!(error.report.acknowledgment_data.is_none());
        assert!(error.report.observed.is_none());
        assert!(!error.report.verified());
        assert_eq!(device.sent().len(), 1);
    }
}

#[test]
fn truncated_failed_mismatched_and_unknown_readback_keep_ack_without_verifying() {
    for getter in [
        reply(&[]),
        reply(&[0]),
        reply(&[2]),
        Reply {
            status: 5,
            ..reply(&[])
        },
    ] {
        let device = Mock::new([reply(&[1]), getter]);
        let error = apply_with_cap(
            &device,
            &cap(),
            FunctionKeyPrimary::Function,
            true,
            &mut sequence(),
            &|| Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.phase, SwapFailurePhase::Readback);
        assert_eq!(error.report.acknowledgment_data, Some(vec![1]));
        assert!(!error.report.verified());
        assert_eq!(device.sent().len(), 2);
    }
}

#[test]
fn exhausted_send_records_an_attempt_but_never_an_ack() {
    let device = Mock::new([]).fail_sends(1);
    let error = apply_with_cap(
        &device,
        &cap(),
        FunctionKeyPrimary::Function,
        false,
        &mut sequence(),
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.phase, SwapFailurePhase::Write);
    assert_eq!(error.report.send_attempts, 1);
    assert!(error.report.acknowledgment_data.is_none());
}

#[test]
fn retries_count_actual_sends_and_reuse_the_same_transaction() {
    let mut cap = cap();
    cap.transport.max_retry_out = 2;
    let device = Mock::new([reply(&[])]).fail_sends(1);
    let result = apply_with_cap(
        &device,
        &cap,
        FunctionKeyPrimary::Function,
        false,
        &mut || Ok(30),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(result.send_attempts, 2);
    let sent = device.sent();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0], sent[1]);
    assert_eq!(sent[0][2], 30);
}

#[test]
fn busy_response_does_not_resend_a_completed_command() {
    let mut cap = cap();
    cap.transport.max_retry_in = 2;
    let device = Mock::new([
        Reply {
            status: 1,
            ..reply(&[])
        },
        reply(&[1]),
    ]);
    assert_eq!(
        read_with_cap(&device, &cap, 0, &|| Ok(()))
            .unwrap()
            .primary(),
        Some(FunctionKeyPrimary::Function)
    );
    assert_eq!(device.sent().len(), 1);
}

#[test]
fn invalid_transactions_and_forged_transport_identity_stop_before_io() {
    let device = Mock::new([]);
    for sequence in [31, 32, 128, 255] {
        assert!(read_with_cap(&device, &cap(), sequence, &|| Ok(())).is_err());
        let error = apply_with_cap(
            &device,
            &cap(),
            FunctionKeyPrimary::Function,
            true,
            &mut || Ok(sequence),
            &|| Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.phase, SwapFailurePhase::Prepare);
        assert!(!error.report.attempted);
    }
    let cap = cap();
    let mut forgeries = Vec::new();
    let mut forged = cap.clone();
    forged.transport.direct_pids.push(718);
    forgeries.push(forged);
    let mut forged = cap.clone();
    forged.transport.claim_interface = 2;
    forgeries.push(forged);
    let mut forged = cap.clone();
    forged.transport.vendor_id = 0x046d;
    forgeries.push(forged);
    let mut forged = cap.clone();
    forged.transport.transaction_prefix = 128;
    forgeries.push(forged);
    let mut forged = cap.clone();
    forged.transport.report_bytes = 90;
    forgeries.push(forged);
    let mut forged = cap.clone();
    forged.source_class = "rzDevice25DualLinkKeyboard".into();
    forgeries.push(forged);
    let mut forged = cap.clone();
    forged.transport.max_retry_in = 11;
    forgeries.push(forged);
    let mut forged = cap;
    forged.transport.max_retry_out = 21;
    forgeries.push(forged);
    for forged in forgeries {
        let error = apply_with_cap(
            &device,
            &forged,
            FunctionKeyPrimary::Function,
            false,
            &mut sequence(),
            &|| Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.phase, SwapFailurePhase::Prepare);
        assert!(!error.report.attempted);
    }
    assert!(device.sent().is_empty());
}

#[test]
fn cancellation_before_send_and_after_ack_preserves_actual_progress() {
    let device = Mock::new([]);
    let error = apply_with_cap(
        &device,
        &cap(),
        FunctionKeyPrimary::Function,
        true,
        &mut sequence(),
        &|| anyhow::bail!("cancelled"),
    )
    .unwrap_err();
    assert_eq!(error.phase, SwapFailurePhase::Prepare);
    assert!(!error.report.attempted);
    assert!(device.sent().is_empty());
    let device = Mock::new([reply(&[])]);
    let validations = Cell::new(0);
    let error = apply_with_cap(
        &device,
        &cap(),
        FunctionKeyPrimary::Function,
        true,
        &mut sequence(),
        &|| {
            validations.set(validations.get() + 1);
            ensure!(validations.get() < 6, "cancel after ACK");
            Ok(())
        },
    )
    .unwrap_err();
    assert_eq!(error.phase, SwapFailurePhase::Write);
    assert_eq!(error.report.acknowledgment_data, Some(vec![]));
    assert!(!error.report.verified());
    assert_eq!(device.sent().len(), 1);
}

#[test]
fn malformed_and_uncorrelated_getter_responses_are_rejected() {
    for getter in [
        Reply {
            count: 90,
            ..reply(&[1])
        },
        Reply {
            declared_length: Some(81),
            ..reply(&[1])
        },
        Reply {
            wrong_transaction: true,
            ..reply(&[1])
        },
        Reply {
            wrong_command: true,
            ..reply(&[1])
        },
    ] {
        let device = Mock::new([getter]);
        assert!(read_with_cap(&device, &cap(), 0, &|| Ok(())).is_err());
        assert_eq!(device.sent().len(), 1);
    }
}

#[test]
fn returned_prefix_is_stripped_before_function_swap_decoding() {
    let device = Mock::new([Reply {
        report_id: 1,
        ..reply(&[1])
    }]);
    assert_eq!(
        read_with_cap(&device, &cap(), 0, &|| Ok(()))
            .unwrap()
            .raw_swap,
        1
    );
    assert_eq!(device.sent().len(), 1);
}
