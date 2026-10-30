use super::*;

#[test]
fn cancellation_finishes_production_without_inventing_a_failure_or_hardware_ack() {
    let signal = AtomicBool::new(true);
    let state = Mutex::new(SoftwareStatus::starting(4));
    complete_status(&state, check_cancellation(&signal));
    let state = state.into_inner().unwrap();
    assert_eq!(state.phase, SoftwarePhase::Stopped);
    assert_eq!(state.generation, 4);
    assert_eq!(state.acknowledged_writes, 0);
    assert!(state.errors.is_empty());
    assert!(!state.restore_acknowledged);
}

#[test]
fn stop_does_not_hide_an_independent_identity_or_native_failure() {
    let state = Mutex::new(SoftwareStatus::starting(5));
    complete_status(
        &state,
        Err(anyhow::anyhow!("Retained device identity changed")),
    );
    complete_status(&state, Err(LightingCanceled.into()));
    let state = state.into_inner().unwrap();
    assert_eq!(state.phase, SoftwarePhase::Failed);
    assert_eq!(state.errors, ["Retained device identity changed"]);
    assert_eq!(state.acknowledged_writes, 0);
}

#[test]
fn cancellation_after_frame_out_preserves_a_simultaneous_release_failure() {
    use razer_device::backend::{ExchangeLease, ExchangeOptions, FeatureTransport};
    use razer_device::controller_lighting::software::EngineFrame;
    use std::cell::Cell;

    struct Io {
        canceled: AtomicBool,
        reports: Cell<usize>,
        releases: Cell<usize>,
        fail_release: bool,
    }
    struct Lease<'a>(&'a Io);
    impl ExchangeLease for Lease<'_> {
        fn transport(&self) -> &dyn FeatureTransport {
            self.0
        }
        fn retry_release(&self) -> anyhow::Result<()> {
            panic!("A canceled frame must not enter another HID cycle")
        }
        fn finish(self: Box<Self>) -> anyhow::Result<()> {
            self.0.releases.set(self.0.releases.get() + 1);
            ensure!(!self.0.fail_release, "Simulated frame release failure");
            Ok(())
        }
    }
    impl FeatureTransport for Io {
        fn begin_exchange(
            &self,
            _: ExchangeOptions,
        ) -> anyhow::Result<Box<dyn ExchangeLease + '_>> {
            Ok(Box::new(Lease(self)))
        }
        fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
            panic!("Frame OUT must retain its actual byte count")
        }
        fn send_feature_at_collection_max_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
            self.reports.set(self.reports.get() + 1);
            self.canceled.store(true, Ordering::Release);
            Ok(report.len())
        }
        fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
            panic!("The source frame has no IN acknowledgement")
        }
        fn metadata(&self) -> Value {
            json!({"simulated": true})
        }
    }

    let frame = EngineFrame {
        device_handle: 7,
        region_id: 0,
        num_row: 1,
        num_col: 32,
        frame: (0..96).collect(),
    };
    for fail_release in [false, true] {
        let io = Io {
            canceled: AtomicBool::new(false),
            reports: Cell::new(0),
            releases: Cell::new(0),
            fail_release,
        };
        let status = Mutex::new(SoftwareStatus::starting(9));
        let submission =
            SoftwareHid::new(&io).send_frame(&frame, 7, &mut |_| check_cancellation(&io.canceled));
        let error = submission.as_ref().unwrap_err();
        assert!(error.is::<LightingCanceled>());
        assert_eq!(
            razer_device::backend::native_io_outcome(error),
            Some(razer_device::backend::NativeIoOutcome::Returned { byte_count: 91 })
        );
        assert_eq!(
            razer_device::backend::exchange_finalization_failed(error),
            fail_release
        );
        complete_status(&status, record_frame_submission(&status, submission));
        let status = status.into_inner().unwrap();
        assert_eq!(io.reports.get(), 1);
        assert_eq!(io.releases.get(), 1);
        assert_eq!(status.reports_submitted, 1);
        assert_eq!(status.frames_submitted, 0);
        assert_eq!(status.acknowledged_writes, 0);
        assert!(!status.restore_acknowledged);
        if fail_release {
            assert_eq!(status.phase, SoftwarePhase::Failed);
            assert_eq!(status.errors.len(), 1);
            assert!(status.errors[0].contains("Simulated frame release failure"));
        } else {
            assert_eq!(status.phase, SoftwarePhase::Stopped);
            assert!(status.errors.is_empty());
        }
    }
}

#[test]
fn frame_error_records_actual_transfers_before_preserving_the_terminal_failure() {
    for reports in [0, 1, 2] {
        let state = Mutex::new(SoftwareStatus::starting(7));
        let error =
            anyhow::anyhow!("Simulated frame release failure").context(FrameSubmissionFailure {
                reports_submitted: reports,
                errors: vec!["Simulated preceding chunk failure".into()],
                interruption: None,
            });
        let result = record_frame_submission(&state, Err(error));
        assert!(result.is_err());
        assert_eq!(
            result
                .as_ref()
                .unwrap_err()
                .downcast_ref::<FrameSubmissionFailure>()
                .unwrap()
                .reports_submitted,
            reports
        );
        complete_status(&state, result);
        let state = state.into_inner().unwrap();
        assert_eq!(state.reports_submitted, u64::from(reports));
        assert_eq!(state.frames_submitted, u64::from(reports == 2));
        assert_eq!(state.phase, SoftwarePhase::Failed);
        assert_eq!(state.acknowledged_writes, 0);
        assert_eq!(state.errors[0], "Simulated preceding chunk failure");
        assert!(state.errors[1].contains("Simulated frame release failure"));
        assert!(!state.restore_acknowledged);
    }
}

#[test]
fn frame_interruption_records_submission_without_claiming_a_device_ack() {
    let state = Mutex::new(SoftwareStatus::starting(8));
    let result = record_frame_submission(
        &state,
        Ok(FrameSubmission {
            reports_submitted: 1,
            complete: false,
            errors: Vec::new(),
            interruption: Some(LightingCanceled.into()),
        }),
    );
    assert!(result.as_ref().unwrap_err().is::<LightingCanceled>());
    complete_status(&state, result);
    let state = state.into_inner().unwrap();
    assert_eq!(state.phase, SoftwarePhase::Stopped);
    assert_eq!(state.reports_submitted, 1);
    assert_eq!(state.frames_submitted, 0);
    assert_eq!(state.acknowledged_writes, 0);
    assert!(state.errors.is_empty());
}

fn stopping_job(restore_acknowledged: bool) -> Job {
    let canceled = Arc::new(AtomicBool::new(false));
    let status = Arc::new(Mutex::new(SoftwareStatus::starting(6)));
    let worker_signal = canceled.clone();
    let worker_status = status.clone();
    let (sender, _receiver) = mpsc::sync_channel(1);
    let worker = thread::spawn(move || {
        while !worker_signal.load(Ordering::Acquire) {
            thread::yield_now();
        }
        let mut status = worker_status.lock().unwrap();
        status.phase = SoftwarePhase::Stopped;
        status.restore_acknowledged = restore_acknowledged;
        if restore_acknowledged {
            status.acknowledged_writes = 3;
        } else {
            status
                .cleanup_errors
                .push("Simulated restore rejected".into());
        }
    });
    Job {
        node: HidNode {
            path: b"simulated-lighting-stop".to_vec(),
            vendor_id: 5426,
            product_id: 4115,
            usage_page: 1,
            usage: 0,
            interface_number: 1,
            release_number: 0,
            serial_number: None,
            manufacturer: None,
            product: None,
        },
        generation: 6,
        sender,
        canceled,
        status,
        thread: Some(worker),
    }
}

#[test]
fn stop_acknowledges_only_the_restore_completed_by_this_stop() {
    let mut job = stopping_job(true);
    let first = job.stop_result().unwrap();
    assert_eq!(first.acknowledged_writes, 1);
    assert!(first.software.unwrap().restore_acknowledged);
    let repeated = job.stop_result().unwrap();
    assert_eq!(repeated.acknowledged_writes, 0);
    assert_eq!(repeated.software.unwrap().acknowledged_writes, 3);
}

#[test]
fn failed_restore_keeps_cleanup_error_without_an_ack() {
    let result = stopping_job(false).stop_result().unwrap();
    assert_eq!(result.acknowledged_writes, 0);
    let software = result.software.unwrap();
    assert!(!software.restore_acknowledged);
    assert_eq!(software.cleanup_errors, ["Simulated restore rejected"]);
}
