use super::*;
use crate::keyboard_control_test_transport::{Mock, assert_report_integrity, reply, sequence};

#[test]
fn release_failure_retains_the_indicator_ack_as_failure_evidence() {
    let device = Mock::new([reply(&[0xd3])]);
    let probe = crate::feature_exchange::tests::ExchangeProbe::failing_finish(&device);
    let error = execute(&probe, &cap(), &Change::Set(2), sequence(), &|| Ok(())).unwrap_err();
    let report = failed_report(&error).unwrap();
    assert_eq!(report.requested_status, Some(2));
    assert_eq!(report.setter_acknowledgement, Some(vec![0xd3]));
    assert!(report.queried_status.is_none());
    assert!(format!("{error:#}").contains("simulated explicit release failed"));
    assert_eq!(device.sent().len(), 1);
}

#[test]
fn initialization_cancellation_after_setter_ack_preserves_getter_and_setter_evidence() {
    let device = Mock::new([reply(&[3]), reply(&[0xd3])]);
    let finishes = std::cell::Cell::new(0);
    let on_finish = || finishes.set(finishes.get() + 1);
    let probe = crate::feature_exchange::tests::ExchangeProbe::with_on_finish(&device, &on_finish);
    let error = execute(&probe, &cap(), &Change::Initialize, sequence(), &|| {
        ensure!(
            finishes.get() < 2,
            "canceled after indicator setter acknowledgement"
        );
        Ok(())
    })
    .unwrap_err();
    let report = failed_report(&error).unwrap();
    assert_eq!(report.queried_status, Some(3));
    assert_eq!(report.requested_status, Some(3));
    assert_eq!(report.setter_acknowledgement, Some(vec![0xd3]));
    assert_eq!(device.sent().len(), 2);
}
fn cap() -> DeviceReadCapability {
    let mut cap = capability(691, 692).unwrap();
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    cap.sleep_between_in_ms = 0;
    cap.max_retry_out = 1;
    cap
}
#[test]
fn initialization_reads_then_enqueues_the_truthy_status_setter() {
    let device = Mock::new([reply(&[3]), reply(&[])]);
    let report = execute(&device, &cap(), &Change::Initialize, sequence(), &|| Ok(())).unwrap();
    assert_eq!(report.queried_status, Some(3));
    assert_eq!(report.requested_status, Some(3));
    assert_eq!(report.setter_acknowledgement, Some(vec![]));
    let sent = device.sent();
    assert_eq!(&sent[0][6..10], &[1, 7, 144, 0]);
    assert_eq!(&sent[1][6..10], &[1, 7, 16, 3]);
    assert_report_integrity(&sent);
}
#[test]
fn zero_or_missing_getter_status_does_not_write_a_default() {
    for data in [&[0][..], &[][..]] {
        let device = Mock::new([reply(data)]);
        let report = execute(&device, &cap(), &Change::Initialize, sequence(), &|| Ok(())).unwrap();
        assert_eq!(report.queried_status, data.first().copied());
        assert!(report.requested_status.is_none());
        assert_eq!(device.sent().len(), 1);
    }
}
#[test]
fn ui_setter_has_no_invented_readback_and_cancellation_prevents_send() {
    let device = Mock::new([reply(&[])]);
    let report = execute(&device, &cap(), &Change::Set(2), sequence(), &|| Ok(())).unwrap();
    assert!(report.queried_status.is_none());
    assert_eq!(device.sent().len(), 1);
    let canceled = Mock::new([]);
    assert!(
        execute(
            &canceled,
            &cap(),
            &Change::Set(1),
            sequence(),
            &|| anyhow::bail!("canceled")
        )
        .is_err()
    );
    assert!(canceled.sent().is_empty());
    assert!(capability(691, 691).is_err());
    assert!(capability(679, 692).is_err());
}
