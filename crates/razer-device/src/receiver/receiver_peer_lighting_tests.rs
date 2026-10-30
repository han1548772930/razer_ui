use super::*;
use crate::backend::{
    FeatureBatchOperation, FeatureBatchReceipt, NativeIoOutcome, execute_feature_batch,
    native_io_failure, native_io_outcome,
};
use crate::feature_exchange::tests::{ExchangeProbe, LeaseEvent};
use std::cell::Cell;
use std::collections::VecDeque;
use std::sync::Mutex;
struct Fake {
    sent: Mutex<Vec<Vec<u8>>>,
    payload: Vec<u8>,
    statuses: Mutex<VecDeque<u8>>,
    native_results: Mutex<VecDeque<anyhow::Result<usize>>>,
    batches: Mutex<Vec<FeatureBatch>>,
    waits: Mutex<Vec<u8>>,
    reads: Cell<usize>,
}
impl FeatureTransport for Fake {
    fn send_feature(&self, bytes: &[u8]) -> anyhow::Result<()> {
        self.sent.lock().unwrap().push(bytes.to_vec());
        Ok(())
    }
    fn send_feature_at_collection_max_counted(&self, bytes: &[u8]) -> anyhow::Result<usize> {
        self.send_feature(bytes)?;
        match self.native_results.lock().unwrap().pop_front() {
            Some(result) => result,
            None => Ok(bytes.len()),
        }
    }
    fn send_feature_batch(&self, batch: &FeatureBatch) -> anyhow::Result<FeatureBatchReceipt> {
        self.batches.lock().unwrap().push(batch.clone());
        execute_feature_batch(
            batch,
            |bytes| self.send_feature_at_collection_max_counted(bytes),
            |milliseconds| {
                self.waits.lock().unwrap().push(milliseconds);
                Ok(())
            },
        )
    }
    fn get_feature(&self, bytes: &mut [u8]) -> anyhow::Result<usize> {
        self.reads.set(self.reads.get() + 1);
        let sent = self.sent.lock().unwrap().last().unwrap().clone();
        bytes.fill(0);
        bytes[1] = self.statuses.lock().unwrap().pop_front().unwrap_or(2);
        bytes[2] = sent[2];
        bytes[6] = self.payload.len() as u8;
        bytes[7..9].copy_from_slice(&sent[7..9]);
        bytes[9..9 + self.payload.len()].copy_from_slice(&self.payload);
        Ok(91)
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::json!({"simulated":true})
    }
}
fn fake(payload: &[u8]) -> Fake {
    Fake {
        sent: Mutex::new(Vec::new()),
        payload: payload.to_vec(),
        statuses: Mutex::new(VecDeque::new()),
        native_results: Mutex::new(VecDeque::new()),
        batches: Mutex::new(Vec::new()),
        waits: Mutex::new(Vec::new()),
        reads: Cell::new(0),
    }
}

fn immediate_support(pid: u16, edition: u8) -> PeerSupport {
    let mut support = super::super::peer_support(pid, edition).unwrap();
    support.max_retry_in = 1;
    support.max_retry_out = 1;
    support.sleep_time_between_in_ms = 0;
    support.sleep_time_between_out_ms = 0;
    support.sleep_time_between_out_in_ms = 0;
    support
}

#[test]
fn failed_custom_selection_retains_partial_mode_restoration_until_real_acknowledgements() {
    let mut peer = PeerHid::new(immediate_support(214, 0)).unwrap();
    let io = fake(&[]);
    io.statuses.lock().unwrap().extend([2, 3, 3]);
    assert!(!peer.restoration_required());
    assert!(peer.select_custom(&io, &|| Ok(())).is_err());
    assert!(peer.restoration_required());
    assert!(!peer.custom_selected());
    assert!(peer.restore(&io, &|| Ok(())).is_err());
    assert!(peer.restoration_required());
    assert_eq!(peer.restore(&io, &|| Ok(())).unwrap(), 2);
    assert!(!peer.restoration_required());
    assert!(!peer.custom_selected());
    let sent = io.sent.lock().unwrap();
    assert_eq!(&sent[0][6..11], &[2, 0, 4, 3, 0]);
    assert_eq!(&sent[1][6..15], &[6, 15, 2, 0, 0, 8, 0, 0, 0]);
    assert_eq!(&sent[3][6..11], &[2, 0, 4, 0, 0]);
    assert_eq!(&sent[4][6..15], &[6, 15, 2, 1, 0, 3, 0, 0, 0]);
}

#[test]
fn source_mode_timeout_exception_completes_without_fabricating_feature_acknowledgements() {
    let support = immediate_support(227, 0);
    assert!(support.ignore_set_device_mode_timeout_error);
    let mut peer = PeerHid::new(support).unwrap();
    let io = fake(&[]);
    io.statuses.lock().unwrap().extend([4, 2]);
    assert_eq!(peer.select_custom(&io, &|| Ok(())).unwrap(), 1);
    assert!(peer.restoration_required());
    assert!(peer.custom_selected());
    assert_eq!(io.sent.lock().unwrap().len(), 2);
    let mut strict = PeerHid::new(immediate_support(214, 0)).unwrap();
    let io = fake(&[]);
    io.statuses.lock().unwrap().push_back(4);
    assert!(strict.select_custom(&io, &|| Ok(())).is_err());
    assert_eq!(strict.acknowledged, 0);
    assert!(strict.restoration_required());
}

#[test]
fn krakoff_mode_and_effect_use_both_namespaces_but_frame_only_dock() {
    let mut peer = PeerHid::new(super::super::peer_support(221, 0).unwrap()).unwrap();
    let io = fake(&[]);
    assert_eq!(peer.select_custom(&io, &|| Ok(())).unwrap(), 4);
    let rows = io.sent.lock().unwrap().clone();
    assert_eq!(rows.iter().map(|r| r[0]).collect::<Vec<_>>(), [3, 3, 3, 3]);
    assert_eq!(
        rows.iter().map(|r| r[2]).collect::<Vec<_>>(),
        [224, 0, 225, 1]
    );
    assert_eq!(&rows[2][9..15], &[0, 0, 8, 0, 0, 0]);
    let frame = EngineFrame {
        device_handle: 7,
        region_id: 0,
        num_row: 1,
        num_col: 17,
        frame: vec![1; 51],
    };
    assert_eq!(
        peer.send_frame(&io, &frame, 7, 1, 17, &|| Ok(())).unwrap(),
        1
    );
    let sent = io.sent.lock().unwrap();
    let last = sent.last().unwrap();
    assert_eq!(last[2], 226);
    assert_eq!(&last[6..9], &[56, 15, 3]);
    assert_eq!(&last[9..14], &[0, 0, 0, 0, 16]);
}

#[test]
fn peer_frame_release_failure_keeps_a_submission_count_without_a_feature_reply() {
    let mut peer = PeerHid::new(super::super::peer_support(221, 0).unwrap()).unwrap();
    let io = fake(&[]);
    let probe = crate::feature_exchange::tests::ExchangeProbe::failing_finish(&io);
    let frame = EngineFrame {
        device_handle: 7,
        region_id: 0,
        num_row: 1,
        num_col: 17,
        frame: vec![1; 51],
    };
    let error = peer
        .send_frame(&probe, &frame, 7, 1, 17, &|| Ok(()))
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
    assert_eq!(probe.events().last(), Some(&LeaseEvent::Finish));
    assert_eq!(io.sent.lock().unwrap().len(), 1);
    assert!(io.statuses.lock().unwrap().is_empty());
    assert_eq!(io.reads.get(), 0);
}

fn three_row_frame() -> EngineFrame {
    EngineFrame {
        device_handle: 7,
        region_id: 0,
        num_row: 3,
        num_col: 2,
        frame: (0..18).collect(),
    }
}

#[test]
fn peer_factories_flush_one_batch_with_source_delays_only_between_rows() {
    for (product, report_id, transaction_prefix) in
        [(214, 0, 0), (215, 0, 0), (227, 3, 0), (221, 3, 224)]
    {
        let mut peer = PeerHid::new(immediate_support(product, 0)).unwrap();
        let io = fake(&[]);
        let probe = ExchangeProbe::new(&io);
        let frame = three_row_frame();
        assert_eq!(
            peer.send_frame(&probe, &frame, 7, 3, 2, &|| Ok(()))
                .unwrap(),
            3
        );
        assert_eq!(
            probe.events(),
            [
                LeaseEvent::Begin(crate::backend::ExchangeTiming::Batch),
                LeaseEvent::Batch,
                LeaseEvent::Finish,
            ]
        );
        let batches = io.batches.lock().unwrap();
        assert_eq!(batches.len(), 1);
        assert!(matches!(
            batches[0].operations(),
            [
                FeatureBatchOperation::Report { .. },
                FeatureBatchOperation::Sleep { milliseconds: 1 },
                FeatureBatchOperation::Report { .. },
                FeatureBatchOperation::Sleep { milliseconds: 1 },
                FeatureBatchOperation::Report { .. },
            ]
        ));
        assert_eq!(*io.waits.lock().unwrap(), [1, 1]);
        let reports = io.sent.lock().unwrap();
        assert_eq!(reports.len(), 3);
        for (row, report) in reports.iter().enumerate() {
            assert_eq!(report.len(), 91);
            assert_eq!(report[0], report_id);
            assert_eq!(report[2], transaction_prefix + row as u8);
            assert_eq!(&report[6..14], &[11, 15, 3, 0, 0, row as u8, 0, 1]);
            assert_eq!(&report[14..20], &frame.frame[row * 6..(row + 1) * 6]);
            assert_eq!(report[89], report[3..89].iter().fold(0, |sum, b| sum ^ b));
        }
        assert_eq!(io.reads.get(), 0);
    }
}

#[test]
fn peer_batch_failure_keeps_partial_native_progress_and_releases_without_sending_later_rows() {
    for returned_before_failure in [false, true] {
        let mut peer = PeerHid::new(immediate_support(221, 0)).unwrap();
        let io = fake(&[]);
        let outcome = if returned_before_failure {
            NativeIoOutcome::Returned { byte_count: 91 }
        } else {
            NativeIoOutcome::Rejected
        };
        io.native_results.lock().unwrap().extend([
            Ok(91),
            Err(native_io_failure(
                outcome,
                anyhow::anyhow!("simulated second row failure"),
            )),
        ]);
        let probe = ExchangeProbe::new(&io);
        let error = peer
            .send_frame(&probe, &three_row_frame(), 7, 3, 2, &|| Ok(()))
            .unwrap_err();
        let expected = if returned_before_failure { 2 } else { 1 };
        assert_eq!(
            error
                .downcast_ref::<crate::ReceiverFrameSubmissionFailure>()
                .unwrap()
                .reports_submitted,
            expected
        );
        let batch = FeatureBatchFailure::from_error(&error).unwrap();
        assert_eq!(batch.progress.reports_submitted(), expected as usize);
        assert_eq!(batch.progress.completed_operations, 2);
        assert_eq!(batch.raw_return, None);
        assert_eq!(native_io_outcome(&error), Some(NativeIoOutcome::Rejected));
        assert_eq!(io.batches.lock().unwrap().len(), 1);
        assert_eq!(io.sent.lock().unwrap().len(), 2);
        assert_eq!(*io.waits.lock().unwrap(), [1]);
        assert_eq!(io.reads.get(), 0);
        assert_eq!(probe.events().last(), Some(&LeaseEvent::Finish));
        let frame = EngineFrame {
            num_row: 1,
            frame: vec![0; 6],
            ..three_row_frame()
        };
        assert_eq!(
            peer.send_frame(&probe, &frame, 7, 1, 2, &|| Ok(()))
                .unwrap(),
            1
        );
        assert_eq!(io.sent.lock().unwrap().last().unwrap()[2], 227);
    }
}

#[test]
fn peer_batch_normal_zero_stops_before_later_rows_and_keeps_the_prior_submission() {
    let mut peer = PeerHid::new(immediate_support(221, 0)).unwrap();
    let io = fake(&[]);
    io.native_results.lock().unwrap().extend([Ok(91), Ok(0)]);
    let probe = ExchangeProbe::new(&io);
    let error = peer
        .send_frame(&probe, &three_row_frame(), 7, 3, 2, &|| Ok(()))
        .unwrap_err();
    assert_eq!(
        error
            .downcast_ref::<crate::ReceiverFrameSubmissionFailure>()
            .unwrap()
            .reports_submitted,
        1
    );
    let batch = FeatureBatchFailure::from_error(&error).unwrap();
    assert_eq!(batch.progress.reports_submitted(), 1);
    assert_eq!(batch.progress.report_returns[1].returned_bytes, 0);
    assert_eq!(batch.raw_return, Some(0));
    assert_eq!(
        native_io_outcome(&error),
        Some(NativeIoOutcome::Returned { byte_count: 0 })
    );
    assert_eq!(io.batches.lock().unwrap().len(), 1);
    assert_eq!(io.sent.lock().unwrap().len(), 2);
    assert_eq!(*io.waits.lock().unwrap(), [1]);
    assert_eq!(io.reads.get(), 0);
    assert_eq!(probe.events().last(), Some(&LeaseEvent::Finish));
}

#[test]
fn peer_frame_cancellation_does_not_interrupt_the_admitted_native_batch_or_publish_success() {
    let mut peer = PeerHid::new(immediate_support(221, 0)).unwrap();
    let io = fake(&[]);
    let probe = ExchangeProbe::new(&io);
    assert!(
        peer.send_frame(&probe, &three_row_frame(), 7, 3, 2, &|| {
            anyhow::bail!("simulated canceled admission")
        })
        .is_err()
    );
    assert!(probe.events().is_empty());
    assert!(io.batches.lock().unwrap().is_empty());
    let error = peer
        .send_frame(&probe, &three_row_frame(), 7, 3, 2, &|| {
            ensure!(
                io.sent.lock().unwrap().is_empty(),
                "simulated canceled owner"
            );
            Ok(())
        })
        .unwrap_err();
    assert_eq!(
        error
            .downcast_ref::<crate::ReceiverFrameSubmissionFailure>()
            .unwrap()
            .reports_submitted,
        3
    );
    let batch = FeatureBatchFailure::from_error(&error).unwrap();
    assert_eq!(batch.progress.reports_submitted(), 3);
    assert_eq!(batch.raw_return, Some(273));
    assert_eq!(
        native_io_outcome(&error),
        Some(NativeIoOutcome::Returned { byte_count: 273 })
    );
    assert_eq!(io.batches.lock().unwrap().len(), 1);
    assert_eq!(io.sent.lock().unwrap().len(), 3);
    assert_eq!(*io.waits.lock().unwrap(), [1, 1]);
    assert_eq!(io.reads.get(), 0);
    assert!(crate::backend::exchange_finalization_failed(&error));
    assert_eq!(
        probe.events(),
        [
            LeaseEvent::Begin(crate::backend::ExchangeTiming::Batch),
            LeaseEvent::Batch,
            LeaseEvent::ForcedDrop,
        ]
    );
}

#[test]
fn edition_battery_and_dock_getters_require_real_payloads_and_source_prefixes() {
    let support = super::super::peer_support(227, 0).unwrap();
    let mut peer = PeerHid::new(support).unwrap();
    let edition = fake(&[0, 128, 1]);
    assert_eq!(
        peer.edition(&edition, &|| Ok(())).unwrap(),
        Edition {
            layout: 0,
            edition: 128,
            firmware: 1
        }
    );
    assert_eq!(edition.sent.lock().unwrap()[0][0], 3);
    let battery = fake(&[0, 128]);
    assert_eq!(peer.battery(&battery, &|| Ok(())).unwrap(), 50);
    assert_eq!(&battery.sent.lock().unwrap()[0][6..10], &[2, 7, 128, 0]);
    assert!(peer.battery(&fake(&[1, 255]), &|| Ok(())).is_err());
    assert!(peer.on_dock(&fake(&[1]), &|| Ok(())).unwrap());
    assert!(!peer.on_dock(&fake(&[2]), &|| Ok(())).unwrap());
}

#[test]
fn edition_release_failure_stops_before_any_source_retry_and_preserves_reply_diagnostics() {
    let mut peer = PeerHid::new(immediate_support(227, 0)).unwrap();
    let io = fake(&[0, 128, 1]);
    let first = std::cell::Cell::new(true);
    let finish_check = || {
        ensure!(
            !first.replace(false),
            "simulated first edition release failure"
        );
        Ok(())
    };
    let probe =
        crate::feature_exchange::tests::ExchangeProbe::with_finish_check(&io, &finish_check);
    let error = peer.edition(&probe, &|| Ok(())).unwrap_err();
    let progress = error
        .downcast_ref::<crate::ReceiverExchangeFailure>()
        .unwrap();
    assert_eq!(progress.command, [3, 0, 134]);
    assert_eq!(
        progress.progress.acknowledged_reply.as_deref(),
        Some([0, 128, 1].as_slice())
    );
    assert!(crate::backend::exchange_finalization_failed(&error));
    assert_eq!(io.sent.lock().unwrap().len(), 1);
}
#[test]
fn white_manifest_uses_ordinary_sender_and_cancel_rejects_before_device_write() {
    let mut peer = PeerHid::new(super::super::peer_support(227, 128).unwrap()).unwrap();
    let io = fake(&[0, 255]);
    assert_eq!(peer.battery(&io, &|| Ok(())).unwrap(), 100);
    assert_eq!(io.sent.lock().unwrap()[0][0], 0);
    let io = fake(&[]);
    assert!(
        peer.select_custom(&io, &|| anyhow::bail!("superseded"))
            .is_err()
    );
    assert!(io.sent.lock().unwrap().is_empty());
}

#[test]
fn static_exit_uses_the_hardware_effect_namespace_for_both_physical_routes() {
    // DeviceManifest_45066_0 / 45067_0 select static #a3e01f. The current
    // 1306 constant is Static=1; 6411.AL writes that ID without translation.
    for product in [45066, 45067] {
        let mut peer = PeerHid::new(immediate_support(product, 0)).unwrap();
        let io = fake(&[]);
        peer.select_custom(&io, &|| Ok(())).unwrap();
        assert_eq!(peer.restore(&io, &|| Ok(())).unwrap(), 2);
        let sent = io.sent.lock().unwrap();
        assert_eq!(sent.len(), 4);
        assert_eq!(&sent[2][6..11], &[2, 0, 4, 0, 0]);
        assert_eq!(&sent[3][6..18], &[9, 15, 2, 1, 0, 1, 0, 0, 1, 163, 224, 31]);
        assert_eq!(sent[3][2], 3);
        assert_eq!(
            sent[3][89],
            sent[3][3..89].iter().fold(0, |sum, byte| sum ^ byte)
        );
        assert!(!peer.restoration_required());
    }
}
