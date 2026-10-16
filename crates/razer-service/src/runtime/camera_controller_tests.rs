use super::*;
use razer_device::{
    camera::{
        CameraSetting, CameraWriteAck, StandardCameraProperty, StandardCameraSetting,
        StandardCameraWrite, VideoProcAmpProperty,
    },
    camera_submission::{CameraCommand, CameraCommandOutcome},
};
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};
fn scope() -> CameraScope {
    CameraScope {
        product_id: 3594,
        container_id: "{01234567-89ab-cdef-0123-456789abcdef}".into(),
    }
}
fn plan() -> CameraSubmissionPlan {
    CameraSubmissionPlan {
        commands: vec![CameraCommand::StandardWrite(StandardCameraSetting {
            property: StandardCameraProperty::VideoProcAmp(VideoProcAmpProperty::Brightness),
            setting: CameraSetting {
                value: 17,
                is_auto: false,
            },
        })],
    }
}
struct Fake {
    events: Arc<Mutex<Vec<&'static str>>>,
    entered: Option<mpsc::Sender<()>>,
    release: Option<mpsc::Receiver<()>>,
}
impl Drop for Fake {
    fn drop(&mut self) {
        self.events.lock().unwrap().push("transport dropped");
    }
}
impl CameraCommandTransport for Fake {
    fn execute(
        &mut self,
        command: &CameraCommand,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<CameraCommandOutcome> {
        self.events.lock().unwrap().push("transport called");
        if let Some(entered) = self.entered.take() {
            entered.send(()).unwrap();
        }
        if let Some(release) = self.release.take() {
            release.recv_timeout(Duration::from_secs(2)).unwrap();
        }
        validate()?;
        let CameraCommand::StandardWrite(s) = command else {
            panic!("unexpected fake request")
        };
        Ok(CameraCommandOutcome::Write(StandardCameraWrite {
            property: s.property,
            requested: s.setting,
            acknowledgement: CameraWriteAck::from_hresult(0)?,
        }))
    }
}
fn finish(controller: &mut Controller) -> CameraSubmissionProgress {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let CameraOperationStatus::Finished(p) = controller.poll("op").unwrap() {
            return p;
        }
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
}
#[test]
fn camera_controller_fake_transport_releases_before_completion_and_retirement() {
    let events = Arc::new(Mutex::new(vec![]));
    let e = events.clone();
    let mut c = Controller::default();
    c.start_with("op".into(), scope(), plan(), move || {
        Ok(Fake {
            events: e,
            entered: None,
            release: None,
        })
    })
    .unwrap();
    let p = finish(&mut c);
    assert!(p.completed);
    assert_eq!(p.outcomes.len(), 1);
    assert_eq!(
        *events.lock().unwrap(),
        vec!["transport called", "transport dropped"]
    );
    assert!(c.jobs.is_empty());
}
#[test]
fn camera_controller_cancel_keeps_operation_owned_until_worker_cleanup() {
    let events = Arc::new(Mutex::new(vec![]));
    let e = events.clone();
    let (entered, wait) = mpsc::channel();
    let (release, recv) = mpsc::channel();
    let mut c = Controller::default();
    c.start_with("op".into(), scope(), plan(), move || {
        Ok(Fake {
            events: e,
            entered: Some(entered),
            release: Some(recv),
        })
    })
    .unwrap();
    wait.recv_timeout(Duration::from_secs(2)).unwrap();
    c.cancel("op").unwrap();
    assert!(matches!(
        c.poll("op").unwrap(),
        CameraOperationStatus::Running
    ));
    assert!(
        c.start_with("new".into(), scope(), plan(), || -> Result<Fake> {
            panic!("must not open second transport")
        })
        .is_err()
    );
    release.send(()).unwrap();
    let p = finish(&mut c);
    assert!(!p.completed);
    assert!(p.unconfirmed.is_some());
    assert!(p.error.as_deref().unwrap().contains("canceled"));
    assert_eq!(events.lock().unwrap().last(), Some(&"transport dropped"));
}
#[test]
fn camera_controller_factory_failure_returns_error_without_device_ack() {
    let mut c = Controller::default();
    c.start_with("op".into(), scope(), plan(), || -> Result<Fake> {
        anyhow::bail!("simulated discovery failure")
    })
    .unwrap();
    let p = finish(&mut c);
    assert!(!p.completed && p.outcomes.is_empty() && p.unconfirmed.is_none());
    assert!(p.error.unwrap().contains("discovery"));
}
#[test]
fn camera_named_lock_identity_uses_container_and_actual_task_name() {
    let mut uppercase = scope();
    uppercase.container_id.make_ascii_uppercase();
    assert_eq!(
        task_lock_name(&scope(), "autoFocus"),
        task_lock_name(&uppercase, "autoFocus")
    );
    assert_ne!(
        task_lock_name(&scope(), "autoFocus"),
        task_lock_name(&scope(), "shutterSpeed")
    );
    let mut other = scope();
    other.container_id = "{11234567-89ab-cdef-0123-456789abcdef}".into();
    assert_ne!(
        task_lock_name(&scope(), "autoFocus"),
        task_lock_name(&other, "autoFocus")
    );
}
#[test]
fn camera_named_jobs_keep_distinct_threads_and_cancellation_owners() {
    let events = Arc::new(Mutex::new(vec![]));
    let mut controller = Controller::default();
    let mut releases = vec![];
    for (id, name) in [("focus", "autoFocus"), ("exposure", "autoExposure")] {
        let (entered, wait) = mpsc::channel();
        let (release, recv) = mpsc::channel();
        let events = events.clone();
        controller
            .start_with_lock(
                id.into(),
                scope(),
                plan(),
                Some(name.into()),
                |_, _, _| Ok(()),
                move || {
                    Ok(Fake {
                        events,
                        entered: Some(entered),
                        release: Some(recv),
                    })
                },
            )
            .unwrap();
        wait.recv_timeout(Duration::from_secs(2)).unwrap();
        releases.push(release);
    }
    controller.cancel("focus").unwrap();
    assert!(!controller.jobs["exposure"].canceled.load(Ordering::Acquire));
    for release in releases {
        release.send(()).unwrap();
    }
    let mut results = HashMap::new();
    let deadline = Instant::now() + Duration::from_secs(3);
    while results.len() != 2 {
        for id in ["focus", "exposure"] {
            if !results.contains_key(id) {
                if let CameraOperationStatus::Finished(progress) = controller.poll(id).unwrap() {
                    results.insert(id, progress);
                }
            }
        }
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    assert!(!results["focus"].completed);
    assert!(results["exposure"].completed);
    assert!(controller.jobs.is_empty());
}
#[test]
fn camera_named_lock_wait_can_abort_before_creating_com_transport() {
    let (entered, wait) = mpsc::channel();
    let mut controller = Controller::default();
    controller
        .start_with_lock(
            "op".into(),
            scope(),
            plan(),
            Some("autoFocus".into()),
            move |_, _, canceled| -> Result<()> {
                entered.send(()).unwrap();
                let deadline = Instant::now() + Duration::from_secs(2);
                while !canceled.load(Ordering::Acquire) {
                    assert!(Instant::now() < deadline);
                    thread::yield_now();
                }
                anyhow::bail!("simulated Web Lock aborted")
            },
            || -> Result<Fake> { panic!("an aborted lock must not create a COM transport") },
        )
        .unwrap();
    wait.recv_timeout(Duration::from_secs(2)).unwrap();
    controller.cancel("op").unwrap();
    let progress = finish(&mut controller);
    assert!(!progress.completed && progress.outcomes.is_empty());
    assert!(progress.error.as_deref().unwrap().contains("aborted"));
}

#[test]
fn camera_controller_shutdown_cancels_all_lock_waiters_before_joining() {
    let signals = Arc::new(Mutex::new(Vec::<Arc<AtomicBool>>::new()));
    let (report, results) = mpsc::channel();
    let mut controller = Controller::default();
    for name in ["autoFocus", "imagePreset", "shutterSpeed"] {
        let signals = signals.clone();
        let report = report.clone();
        let (entered, wait) = mpsc::channel();
        controller
            .start_with_lock(
                name.into(),
                scope(),
                plan(),
                Some(name.into()),
                move |_, _, canceled| -> Result<()> {
                    entered.send(()).unwrap();
                    while !canceled.load(Ordering::Acquire) {
                        thread::yield_now();
                    }
                    // A blocked owner can require another retained job to abort before
                    // releasing a lock. Bound the regression fixture instead of hanging.
                    let deadline = Instant::now() + Duration::from_secs(2);
                    let all_canceled = loop {
                        if signals
                            .lock()
                            .unwrap()
                            .iter()
                            .all(|signal| signal.load(Ordering::Acquire))
                        {
                            break true;
                        }
                        if Instant::now() >= deadline {
                            break false;
                        }
                        thread::yield_now();
                    };
                    report.send(all_canceled).unwrap();
                    anyhow::bail!("simulated shutdown while waiting for native lock")
                },
                || -> Result<Fake> { panic!("canceled waiter cannot open COM") },
            )
            .unwrap();
        wait.recv_timeout(Duration::from_secs(2)).unwrap();
    }
    signals
        .lock()
        .unwrap()
        .extend(controller.jobs.values().map(|job| job.canceled.clone()));
    drop(controller);
    drop(report);
    let results: Vec<_> = results.into_iter().collect();
    assert_eq!(results, vec![true; 3]);
}

#[test]
fn camera_controller_dispatches_supplemental_exports_through_simulated_ks_and_cleans_up() {
    use razer_device::camera_extensions::{
        CameraExtensionRequest, CameraExtensionTransport, CameraKsCallOutcome,
        execute_camera_extension,
    };
    struct MemoryKs {
        writes: Arc<Mutex<Vec<[u8; 8]>>>,
    }
    impl CameraExtensionTransport for MemoryKs {
        fn begin_node(&mut self, node: u32) -> Result<()> {
            assert_eq!(node, 3);
            Ok(())
        }
        fn ks_property(
            &mut self,
            header: &[u8; 32],
            data: &mut [u8; 8],
        ) -> Result<CameraKsCallOutcome> {
            if header[20] == 2 {
                self.writes.lock().unwrap().push(*data);
                Ok(CameraKsCallOutcome {
                    hresult: 0,
                    bytes_returned: 0,
                })
            } else {
                assert_eq!(header[20], 1);
                *data = [0; 8];
                Ok(CameraKsCallOutcome {
                    hresult: 0,
                    bytes_returned: 8,
                })
            }
        }
        fn end_node(&mut self) {}
        fn wait_ms(&mut self, duration: u32) -> Result<()> {
            assert_eq!(duration, 10);
            Ok(())
        }
    }
    struct ExtensionFake {
        ks: MemoryKs,
        dropped: Arc<Mutex<bool>>,
    }
    impl Drop for ExtensionFake {
        fn drop(&mut self) {
            *self.dropped.lock().unwrap() = true;
        }
    }
    impl CameraCommandTransport for ExtensionFake {
        fn execute(
            &mut self,
            command: &CameraCommand,
            validate: &impl Fn() -> Result<()>,
        ) -> Result<CameraCommandOutcome> {
            let CameraCommand::Extension(request) = command else {
                panic!("unexpected camera command")
            };
            Ok(CameraCommandOutcome::Extension(execute_camera_extension(
                &mut self.ks,
                3594,
                *request,
                validate,
            )))
        }
    }
    for (request, expected) in [
        (
            CameraExtensionRequest::SetHdrMode { mode: 258 },
            [0xa0, 0x15, 1, 2, 0, 0, 0, 0],
        ),
        (
            CameraExtensionRequest::RebootDevice,
            [0xc0, 0x0e, 7, 2, 0, 0, 0, 0],
        ),
        (
            CameraExtensionRequest::RestoreDefaultAndReboot,
            [0xc0, 0x0e, 7, 1, 0, 0, 0, 0],
        ),
    ] {
        let writes = Arc::new(Mutex::new(vec![]));
        let dropped = Arc::new(Mutex::new(false));
        let w = writes.clone();
        let d = dropped.clone();
        let mut controller = Controller::default();
        controller
            .start_with(
                "op".into(),
                scope(),
                CameraSubmissionPlan {
                    commands: vec![CameraCommand::Extension(request)],
                },
                move || {
                    Ok(ExtensionFake {
                        ks: MemoryKs { writes: w },
                        dropped: d,
                    })
                },
            )
            .unwrap();
        let progress = finish(&mut controller);
        assert!(progress.completed && progress.error.is_none());
        assert_eq!(progress.outcomes.len(), 1);
        assert_eq!(*writes.lock().unwrap(), vec![expected]);
        assert!(*dropped.lock().unwrap());
        assert!(controller.jobs.is_empty());
    }
}
#[test]
fn camera_controller_validation_rejects_untrusted_scope_and_empty_plan_before_open() {
    let mut c = Controller::default();
    let mut s = scope();
    s.container_id = "PREVIEW-1".into();
    assert!(
        c.start_with("op".into(), s, plan(), || -> Result<Fake> {
            panic!("must not open")
        })
        .is_err()
    );
    assert!(
        c.start_with(
            "op".into(),
            scope(),
            CameraSubmissionPlan::default(),
            || -> Result<Fake> { panic!("must not open") }
        )
        .is_err()
    );
    assert!(c.jobs.is_empty());
}
