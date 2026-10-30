use super::*;
use crate::backend::{
    FeatureBatchOperation, FeatureBatchReceipt, NativeIoOutcome, execute_feature_batch,
    native_io_outcome,
};
use crate::feature_exchange::tests::{ExchangeProbe, LeaseEvent};
use std::cell::{Cell, RefCell};

#[derive(Default)]
struct Transport {
    reports: RefCell<Vec<Vec<u8>>>,
    acknowledgements: RefCell<std::collections::VecDeque<Vec<u8>>>,
    fail_send: Cell<bool>,
    reads: Cell<usize>,
    batches: RefCell<Vec<FeatureBatch>>,
    returned_bytes: Cell<Option<usize>>,
}
impl FeatureTransport for Transport {
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        self.reports.borrow_mut().push(report.to_vec());
        ensure!(!self.fail_send.get(), "simulated uncertain Feature write");
        Ok(())
    }
    fn send_feature_at_collection_max_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        self.send_feature(report)?;
        Ok(match self.returned_bytes.get() {
            Some(count) => count,
            None => report.len(),
        })
    }
    fn send_feature_batch(&self, batch: &FeatureBatch) -> anyhow::Result<FeatureBatchReceipt> {
        self.batches.borrow_mut().push(batch.clone());
        execute_feature_batch(
            batch,
            |report| self.send_feature_at_collection_max_counted(report),
            |_| panic!("Dock frames contain one physical LED row"),
        )
    }
    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        self.reads.set(self.reads.get() + 1);
        let reply = self
            .acknowledgements
            .borrow_mut()
            .pop_front()
            .expect("unexpected Feature getter");
        report[..reply.len()].copy_from_slice(&reply);
        Ok(reply.len())
    }
    fn metadata(&self) -> Value {
        Value::Null
    }
}

fn capability(product: u16) -> ReceiverCapability {
    let mut capability = crate::receiver_capabilities::capability(product)
        .unwrap()
        .clone();
    capability.sleep_between_out_ms = 0;
    capability.sleep_between_out_in_ms = 0;
    capability.sleep_between_in_ms = 0;
    capability
}

fn frame() -> EngineFrame {
    EngineFrame {
        device_handle: 19,
        region_id: 0,
        num_row: 1,
        num_col: 8,
        frame: (1..=24).collect(),
    }
}

#[test]
fn receiver_lighting_manifests_keep_product_led_order_and_one_physical_row() {
    for (product, expected) in [
        (164, [5, 6, 4, 7, 3, 0, 2, 1]),
        (241, [6, 7, 5, 0, 4, 1, 3, 2]),
    ] {
        let config = led_configuration(product, 0).unwrap();
        assert_eq!((config.matrix_max_row, config.matrix_max_col), (5, 3));
        assert_eq!((config.device_max_row, config.device_max_col), (1, 8));
        assert_eq!(
            config
                .led_matrix
                .iter()
                .map(|led| led.device_pos[1])
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(config.led_matrix[0].matrix_pos, [0, 1]);
        assert_eq!(config.led_matrix[7].matrix_pos, [4, 1]);
        assert!(led_configuration(product, 1).is_err());
    }
    assert!(led_configuration(4115, 0).is_err());
}

#[test]
fn receiver_lighting_native_frame_keeps_linker_registration_and_has_no_feature_getter() {
    let transport = Transport::default();
    let capability = capability(164);
    let mut lighting = LightingHid::new(&capability).unwrap();
    let probe = ExchangeProbe::new(&transport);
    lighting
        .send_frame(&probe, &frame(), 19, &|| Ok(()))
        .unwrap();
    assert_eq!(
        probe.events(),
        [
            LeaseEvent::Begin(crate::backend::ExchangeTiming::Batch),
            LeaseEvent::Batch,
            LeaseEvent::Finish,
        ]
    );
    let batches = transport.batches.borrow();
    assert_eq!(batches.len(), 1);
    assert!(matches!(
        batches[0].operations(),
        [FeatureBatchOperation::Report { .. }]
    ));
    let reports = transport.reports.borrow();
    assert_eq!(reports.len(), 1);
    let report = &reports[0];
    assert_eq!(report.len(), 91);
    assert_eq!(&report[..3], &[0, 0, 0xe1]);
    assert_eq!(&report[6..14], &[29, 15, 3, 0, 0, 0, 0, 7]);
    assert_eq!(&report[14..38], &(1..=24).collect::<Vec<u8>>());
    assert!(report[38..89].iter().all(|byte| *byte == 0));
    assert_eq!(
        report[89],
        report[3..89].iter().fold(0, |sum, byte| sum ^ byte)
    );
    assert_eq!(transport.reads.get(), 0);
}

#[test]
fn frame_release_failure_retains_actual_submission_without_fabricating_an_ack() {
    let transport = Transport::default();
    let probe = crate::feature_exchange::tests::ExchangeProbe::failing_finish(&transport);
    let capability = capability(164);
    let mut lighting = LightingHid::new(&capability).unwrap();
    let error = lighting
        .send_frame(&probe, &frame(), 19, &|| Ok(()))
        .unwrap_err();
    let progress = error
        .downcast_ref::<crate::ReceiverFrameSubmissionFailure>()
        .unwrap();
    assert_eq!(progress.reports_submitted, 1);
    let batch = FeatureBatchFailure::from_error(&error).unwrap();
    assert_eq!(batch.progress.reports_submitted(), 1);
    assert_eq!(batch.raw_return, Some(91));
    assert_eq!(
        native_io_outcome(&error),
        Some(NativeIoOutcome::Returned { byte_count: 91 })
    );
    assert!(crate::backend::exchange_finalization_failed(&error));
    assert_eq!(
        probe.events(),
        [
            LeaseEvent::Begin(crate::backend::ExchangeTiming::Batch),
            LeaseEvent::Batch,
            LeaseEvent::Finish,
        ]
    );
    assert_eq!(transport.reports.borrow().len(), 1);
    assert_eq!(transport.reads.get(), 0);
}

#[test]
fn receiver_frame_rejects_zero_short_and_overlong_normal_returns_without_inventing_progress() {
    for byte_count in [0, 90, 92] {
        let transport = Transport::default();
        transport.returned_bytes.set(Some(byte_count));
        let probe = ExchangeProbe::new(&transport);
        let error = LightingHid::new(&capability(164))
            .unwrap()
            .send_frame(&probe, &frame(), 19, &|| Ok(()))
            .unwrap_err();
        let failure = error
            .downcast_ref::<crate::ReceiverFrameSubmissionFailure>()
            .unwrap();
        assert_eq!(failure.reports_submitted, 0);
        let batch = FeatureBatchFailure::from_error(&error).unwrap();
        assert_eq!(batch.progress.reports_submitted(), 0);
        assert_eq!(batch.progress.report_returns[0].returned_bytes, byte_count);
        assert_eq!(batch.raw_return, Some(byte_count as i32));
        assert_eq!(
            native_io_outcome(&error),
            Some(NativeIoOutcome::Returned { byte_count })
        );
        assert_eq!(transport.batches.borrow().len(), 1);
        assert_eq!(transport.reports.borrow().len(), 1);
        assert_eq!(transport.reads.get(), 0);
        assert_eq!(probe.events().last(), Some(&LeaseEvent::Finish));
    }
}

#[test]
fn receiver_frame_cancellation_after_batch_return_retains_submission_and_forced_cleanup() {
    let transport = Transport::default();
    let probe = ExchangeProbe::new(&transport);
    let error = LightingHid::new(&capability(164))
        .unwrap()
        .send_frame(&probe, &frame(), 19, &|| {
            ensure!(
                transport.reports.borrow().is_empty(),
                "simulated canceled frame owner"
            );
            Ok(())
        })
        .unwrap_err();
    let failure = error
        .downcast_ref::<crate::ReceiverFrameSubmissionFailure>()
        .unwrap();
    assert_eq!(failure.reports_submitted, 1);
    let batch = FeatureBatchFailure::from_error(&error).unwrap();
    assert_eq!(batch.progress.reports_submitted(), 1);
    assert_eq!(batch.raw_return, Some(91));
    assert_eq!(
        native_io_outcome(&error),
        Some(NativeIoOutcome::Returned { byte_count: 91 })
    );
    assert!(crate::backend::exchange_finalization_failed(&error));
    assert_eq!(
        probe.events(),
        [
            LeaseEvent::Begin(crate::backend::ExchangeTiming::Batch),
            LeaseEvent::Batch,
            LeaseEvent::ForcedDrop,
        ]
    );
    assert_eq!(transport.reads.get(), 0);
}

#[test]
fn receiver_lighting_rejects_stale_native_handles_and_geometry_before_any_write() {
    let transport = Transport::default();
    let capability = capability(241);
    let mut lighting = LightingHid::new(&capability).unwrap();
    for case in 0..5 {
        let mut invalid = frame();
        match case {
            0 => invalid.device_handle = 20,
            1 => invalid.region_id = 1,
            2 => invalid.num_row = 2,
            3 => invalid.num_col = 7,
            4 => {
                invalid.frame.pop();
            }
            _ => unreachable!(),
        }
        assert!(
            lighting
                .send_frame(&transport, &invalid, 19, &|| Ok(()))
                .is_err()
        );
    }
    assert!(transport.reports.borrow().is_empty());
    assert!(
        lighting
            .send_frame(&transport, &frame(), 19, &|| anyhow::bail!(
                "retired device instance"
            ))
            .is_err()
    );
    assert!(transport.reports.borrow().is_empty());
    transport.fail_send.set(true);
    assert!(
        lighting
            .send_frame(&transport, &frame(), 19, &|| Ok(()))
            .is_err()
    );
    assert_eq!(transport.reports.borrow().len(), 1);
    assert_eq!(transport.reads.get(), 0);
}

#[test]
fn receiver_lighting_driver_and_custom_effect_need_distinct_real_feature_receipts() {
    let transport = Transport::default();
    for (transaction, command_class, command) in [(224, 0, 4), (225, 15, 2)] {
        let mut reply = vec![0; 91];
        reply[1] = 2;
        reply[2] = transaction;
        reply[7] = command_class;
        reply[8] = command;
        transport.acknowledgements.borrow_mut().push_back(reply);
    }
    let capability = capability(164);
    let mut lighting = LightingHid::new(&capability).unwrap();
    let next = Cell::new(224u8);
    let ack = lighting
        .select_custom(
            &transport,
            || {
                let value = next.get();
                next.set(value + 1);
                Ok(value)
            },
            &|| Ok(()),
        )
        .unwrap();
    assert_eq!(ack, 2);
    let reports = transport.reports.borrow();
    assert_eq!(&reports[0][6..11], &[2, 0, 4, 3, 0]);
    assert_eq!(
        reports.iter().map(|report| report[0]).collect::<Vec<_>>(),
        [0, 0]
    );
    assert_eq!(&reports[1][6..15], &[6, 15, 2, 0, 0, 8, 0, 0, 0]);
    assert_eq!(transport.reads.get(), 2);
}

#[test]
fn receiver_lighting_exit_restores_normal_mode_then_the_current_source_default() {
    for battery in [false, true] {
        let transport = Transport::default();
        for (transaction, class, command) in [(224, 0, 4), (225, 15, 2)] {
            let mut reply = vec![0; 91];
            reply[1] = 2;
            reply[2] = transaction;
            reply[7] = class;
            reply[8] = command;
            transport.acknowledgements.borrow_mut().push_back(reply);
        }
        let mut lighting = LightingHid::new(&capability(241)).unwrap();
        let transaction = Cell::new(224);
        let count = lighting
            .restore_on_exit(
                &transport,
                battery,
                || {
                    let next = transaction.get();
                    transaction.set(next + 1);
                    Ok(next)
                },
                &|| Ok(()),
            )
            .unwrap();
        assert_eq!(count, 2);
        let reports = transport.reports.borrow();
        assert_eq!(&reports[0][6..11], &[2, 0, 4, 0, 0]);
        assert_eq!(
            &reports[1][6..15],
            &[6, 15, 2, 1, 0, if battery { 13 } else { 3 }, 0, 0, 0]
        );
        assert_eq!(transport.reads.get(), 2);
    }
}

#[test]
fn receiver_lighting_frame_sequence_survives_replacing_the_short_lived_feature_handle() {
    let mut lighting = LightingHid::new(&capability(164)).unwrap();
    let first = Transport::default();
    lighting
        .send_frame(&first, &frame(), 19, &|| Ok(()))
        .unwrap();
    assert_eq!(first.reports.borrow()[0][2], 0xe1);
    drop(first);
    let next = Transport::default();
    lighting
        .send_frame(&next, &frame(), 19, &|| Ok(()))
        .unwrap();
    assert_eq!(next.reports.borrow()[0][2], 0xe2);
}

#[test]
fn receiver_lighting_failed_frame_does_not_retire_the_shared_sender_or_invent_a_getter() {
    let mut lighting = LightingHid::new(&capability(241)).unwrap();
    let transport = Transport::default();
    transport.fail_send.set(true);
    assert!(
        lighting
            .send_frame(&transport, &frame(), 19, &|| Ok(()))
            .is_err()
    );
    transport.fail_send.set(false);
    lighting
        .send_frame(&transport, &frame(), 19, &|| Ok(()))
        .unwrap();
    let reports = transport.reports.borrow();
    assert_eq!(reports.len(), 2);
    assert_eq!([reports[0][2], reports[1][2]], [0xe1, 0xe2]);
    assert_eq!([reports[0][0], reports[1][0]], [0, 0]);
    assert_eq!(transport.reads.get(), 0);
}

#[test]
fn receiver_battery_monitor_receipt_distinguishes_source_no_write_from_a_real_frame() {
    let configuration = Configuration {
        product_id: 164,
        edition_id: 0,
        effect: QuickEffect::parse(&serde_json::json!({"selectedEffectId":14})).unwrap(),
        selected_docking_mouse: None,
    };
    configuration.validate().unwrap();
    let mut status = Status::starting(
        "owner".into(),
        "path".into(),
        "container".into(),
        &configuration,
    );
    status.software.phase = crate::lighting_engine::SoftwarePhase::Running;
    status.application = ApplicationReceipt::SourceNoWrite {
        generation: 1,
        reason: battery_monitor::NoWriteReason::NoEligiblePeer,
    };
    assert!(status.ensure_applied(&configuration).is_err());
    status.monitoring.observations = 1;
    status.monitoring.monitored_peers = 1;
    status.ensure_applied(&configuration).unwrap();
    assert_eq!(status.software.frames_submitted, 0);
    status.software.generation = 2;
    assert!(status.ensure_applied(&configuration).is_err());
    status.application = ApplicationReceipt::SubmittedFrame { generation: 2 };
    assert!(status.ensure_applied(&configuration).is_err());
    status.software.frames_submitted = 1;
    status.ensure_applied(&configuration).unwrap();
}

#[test]
fn receiver_effect_validation_rejects_null_settings_and_product_unmounted_choices() {
    assert!(
        serde_json::from_value::<QuickEffect>(
            serde_json::json!({"selectedEffectId":1,"selectedEffectSetting":null})
        )
        .is_err()
    );
    let effect = QuickEffect::parse(&serde_json::json!({"selectedEffectId":8})).unwrap();
    assert!(
        Configuration {
            product_id: 164,
            edition_id: 0,
            effect: effect.clone(),
            selected_docking_mouse: None
        }
        .validate()
        .is_err()
    );
    Configuration {
        product_id: 241,
        edition_id: 0,
        effect,
        selected_docking_mouse: None,
    }
    .validate()
    .unwrap();
}

#[test]
fn receiver_restoration_receipt_distinguishes_no_mutation_from_real_acknowledgements() {
    let configuration = Configuration {
        product_id: 164,
        edition_id: 0,
        effect: QuickEffect::parse(&serde_json::json!({"selectedEffectId":3})).unwrap(),
        selected_docking_mouse: None,
    };
    let mut status = Status::starting(
        "owner".into(),
        "path".into(),
        "container".into(),
        &configuration,
    );
    assert!(status.restoration_complete());
    assert!(!status.software.restore_acknowledged);
    assert_eq!(status.software.acknowledged_writes, 0);
    status.restoration = RestorationDisposition::AwaitingAcknowledgements;
    assert!(!status.restoration_complete());
    status.software.restore_acknowledged = true;
    assert!(!status.restoration_complete());
    status.restoration = RestorationDisposition::Acknowledged { writes: 0 };
    assert!(!status.restoration_complete());
    status.restoration = RestorationDisposition::Acknowledged { writes: 2 };
    assert!(status.restoration_complete());
    status.software.restore_acknowledged = false;
    assert!(!status.restoration_complete());
}
