//! Typed PID70 measurement IPC. Progress is streamed while the source engine
//! waits for physical clicks/notifications; cancellation drains owned cleanup.
use super::*;
use razer_device::{
    device_reads::DeviceReadTarget,
    mouse_philips::measurement::{Kind, Progress},
};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    sync::{Mutex, TryLockError},
    time::{Duration, Instant},
};

pub struct Completion {
    pub progress: Option<Progress>,
    pub error: Option<String>,
}
/// Own the entire input capture, native worker and cross-process restoration.
/// This entry point performs blocking I/O and belongs on a background executor.
pub fn run(
    observed: &ObservedDevice,
    id: String,
    kind: Kind,
    valid: &AtomicBool,
    publish: impl FnMut(Progress) -> anyhow::Result<()>,
) -> anyhow::Result<Completion> {
    with_capture_scope(
        &razer_ipc::KEY_CAPTURE_LIFECYCLE,
        &razer_ipc::mapping_control::Registry::global(),
        valid,
        || {
            ensure!(
                valid.load(Ordering::Acquire),
                "Philips measurement canceled before worker creation"
            );
            let mut client = ServiceClient::spawn()?;
            let completion = request(&mut client, observed, id, kind, valid, publish);
            // request waits for source callback release through Shutdown. Worker
            // destruction precedes restoring the independently retained actors.
            drop(client);
            Ok(completion)
        },
    )
}
fn with_capture_scope(
    capture: &Mutex<()>,
    registry: &razer_ipc::mapping_control::Registry,
    valid: &AtomicBool,
    work: impl FnOnce() -> anyhow::Result<Completion>,
) -> anyhow::Result<Completion> {
    let _capture = loop {
        ensure!(
            valid.load(Ordering::Acquire),
            "Philips measurement canceled before capture ownership"
        );
        match capture.try_lock() {
            Ok(lease) => break lease,
            Err(TryLockError::WouldBlock) => std::thread::sleep(Duration::from_millis(10)),
            Err(TryLockError::Poisoned(_)) => anyhow::bail!("Capture lifecycle lock poisoned"),
        }
    };
    let mut suppression = registry
        .suspend_while(razer_ipc::mapping_control::Suppression::Mapping, || {
            valid.load(Ordering::Acquire)
        })
        .map_err(anyhow::Error::msg)?;
    let outcome = work();
    let restore = suppression.release();
    match (outcome, restore) {
        (Ok(mut completion), Err(error)) => {
            completion.error = Some(match completion.error {
                Some(previous) => format!("{previous}; mapping restore: {error}"),
                None => format!("Mapping restore: {error}"),
            });
            Ok(completion)
        }
        (Err(error), Err(cleanup)) => {
            Err(error.context(format!("Mapping restore also failed: {cleanup}")))
        }
        (outcome, Ok(())) => outcome,
    }
}
pub fn available(observed: &ObservedDevice) -> bool {
    #[cfg(windows)]
    {
        super::mouse_philips::available(observed) && observed.hid_node().is_none()
    }
    #[cfg(not(windows))]
    {
        let _ = observed;
        false
    }
}
fn request(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
    id: String,
    kind: Kind,
    valid: &AtomicBool,
    mut publish: impl FnMut(Progress) -> anyhow::Result<()>,
) -> Completion {
    let mut target = None;
    let mut latest = None;
    let result = (|| {
        ensure!(
            valid.load(Ordering::Acquire),
            "Philips measurement canceled before resolution"
        );
        ensure!(
            available(observed),
            "Philips native measurement has no live Windows route"
        );
        let selected = resolve_target(client, observed)?;
        target = Some(selected.clone());
        run_with(
            &selected,
            &id,
            kind,
            valid,
            |request| client.request(request),
            &mut publish,
            &mut latest,
        )
    })();
    let shutdown = client.request(ServiceRequest::Shutdown);
    complete(result, shutdown, target.as_ref(), &id, kind, latest)
}
fn resolve_target(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
) -> anyhow::Result<DeviceReadTarget> {
    let route = super::resolve(
        client,
        observed,
        razer_device::mouse_legacy_polling::read_capability(),
    )?;
    #[cfg(windows)]
    match route {
        DirectRoute::Container(target) => Ok(target),
        _ => anyhow::bail!("Philips measurement requires the current native USB event adapter"),
    }
    #[cfg(not(windows))]
    {
        let _ = route;
        anyhow::bail!("Philips event adapter unavailable on this platform")
    }
}
fn validate(
    progress: &Progress,
    target: &DeviceReadTarget,
    id: &str,
    kind: Kind,
) -> anyhow::Result<()> {
    progress.validate(id, target, kind)
}
fn run_with(
    target: &DeviceReadTarget,
    id: &str,
    kind: Kind,
    valid: &AtomicBool,
    mut request: impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
    publish: &mut impl FnMut(Progress) -> anyhow::Result<()>,
    latest: &mut Option<Progress>,
) -> anyhow::Result<Progress> {
    ensure!(
        valid.load(Ordering::Acquire),
        "Philips scope canceled before start"
    );
    let mut progress: Progress =
        serde_json::from_value(request(ServiceRequest::MousePhilipsMeasurementStart {
            operation_id: id.into(),
            target: target.clone(),
            kind,
        })?)?;
    let began = Instant::now();
    let mut canceled_at = None;
    let mut publish_error = None;
    loop {
        validate(&progress, target, id, kind)?;
        *latest = Some(progress.clone());
        if publish_error.is_none() {
            if let Err(error) = publish(progress.clone()) {
                publish_error = Some(error);
            }
        }
        if progress.terminal {
            if let Some(error) = publish_error {
                return Err(error).context("Philips progress consumer closed");
            }
            return Ok(progress);
        }
        // Source maximum waits: initial35s, Start60s, collection60s and End60s.
        // The outer bound includes native registration/I/O and is application
        // policy; it is not a replacement for any source phase deadline.
        let cancel = !valid.load(Ordering::Acquire)
            || publish_error.is_some()
            || began.elapsed() >= Duration::from_secs(250);
        if cancel && canceled_at.is_none() {
            canceled_at = Some(Instant::now());
            progress =
                serde_json::from_value(request(ServiceRequest::MousePhilipsMeasurementCancel {
                    operation_id: id.into(),
                })?)?;
        } else {
            std::thread::sleep(Duration::from_millis(10));
            progress =
                serde_json::from_value(request(ServiceRequest::MousePhilipsMeasurementPoll {
                    operation_id: id.into(),
                })?)?;
        }
        ensure!(
            !canceled_at.is_some_and(|began| began.elapsed() >= Duration::from_secs(120)),
            "Philips cancellation cleanup deadline exceeded"
        );
    }
}
fn complete(
    result: anyhow::Result<Progress>,
    shutdown: anyhow::Result<Value>,
    target: Option<&DeviceReadTarget>,
    id: &str,
    kind: Kind,
    latest: Option<Progress>,
) -> Completion {
    let mut errors = Vec::new();
    let mut progress = match result {
        Ok(progress) => Some(progress),
        Err(error) => {
            errors.push(format!("Philips measurement transport: {error:#}"));
            latest
        }
    };
    let cleanup = (|| -> anyhow::Result<()> {
        #[derive(serde::Deserialize)]
        struct Shutdown {
            stopped: bool,
            mouse_philips_measurement_cleanup: Option<Progress>,
        }
        let shutdown: Shutdown = serde_json::from_value(shutdown?)?;
        ensure!(shutdown.stopped, "Philips worker did not stop");
        if let Some(cleanup) = shutdown.mouse_philips_measurement_cleanup {
            validate(
                &cleanup,
                target.context("Philips cleanup has no target")?,
                id,
                kind,
            )?;
            ensure!(
                cleanup.terminal && cleanup.cleanup_confirmed,
                "Philips callback release was not confirmed"
            );
            if let Some(previous) = &progress {
                if previous.terminal {
                    ensure!(
                        previous.commands == cleanup.commands
                            && previous.measured == cleanup.measured,
                        "Philips shutdown changed the completed measurement"
                    );
                } else {
                    ensure!(
                        cleanup.commands.starts_with(&previous.commands),
                        "Philips shutdown discarded prior acknowledgements"
                    );
                }
            }
            progress = Some(cleanup);
        } else {
            ensure!(
                target.is_none(),
                "Philips worker omitted its measurement cleanup report"
            );
        }
        Ok(())
    })();
    if let Err(error) = cleanup {
        errors.push(format!("Philips measurement cleanup: {error:#}"));
    }
    Completion {
        progress,
        error: (!errors.is_empty()).then(|| errors.join("; ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use razer_device::mouse_philips::measurement::Phase;
    fn target() -> DeviceReadTarget {
        DeviceReadTarget {
            product_id: 70,
            physical_product_id: 70,
            peer_product_id: None,
            device_container_id: "simulated-container".into(),
            path: "simulated-hid".into(),
        }
    }
    fn progress() -> Progress {
        razer_device::mouse_philips::measurement::Machine::new(Kind::Manual).progress(
            "owner",
            &target(),
            vec![],
        )
    }
    fn next_control(
        owner: &razer_ipc::mapping_control::Owner,
    ) -> razer_ipc::mapping_control::Control {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let mut controls = owner.drain();
            if !controls.is_empty() {
                assert_eq!(controls.len(), 1);
                return controls.remove(0);
            }
            assert!(
                Instant::now() < deadline,
                "Mapping control was not delivered"
            );
            std::thread::yield_now();
        }
    }
    #[test]
    fn philips_measurement_capture_waits_for_pause_and_retains_owner_until_restore_ack() {
        use razer_ipc::mapping_control::Registry;
        use std::sync::{Arc, mpsc};
        for work_fails in [false, true] {
            let registry = Registry::default();
            let mut mapping = registry.register(uuid::Uuid::new_v4().to_string()).unwrap();
            let capture = Arc::new(Mutex::new(()));
            let background_capture = capture.clone();
            let background_registry = registry.clone();
            let (native_released, observed) = mpsc::channel();
            let worker = std::thread::spawn(move || {
                with_capture_scope(
                    &background_capture,
                    &background_registry,
                    &AtomicBool::new(true),
                    || {
                        native_released.send(()).unwrap();
                        if work_fails {
                            anyhow::bail!("simulated native registration failed after cleanup");
                        }
                        Ok(Completion {
                            progress: Some(progress()),
                            error: None,
                        })
                    },
                )
            });
            let pause = next_control(&mapping);
            assert!(!pause.target().mapping_enabled());
            assert_eq!(pause.target().global_shortcuts_enabled(), None);
            assert!(observed.try_recv().is_err());
            assert!(capture.try_lock().is_err());
            pause.acknowledge(Ok(()));
            observed.recv_timeout(Duration::from_secs(3)).unwrap();
            let restore = next_control(&mapping);
            assert!(restore.target().mapping_enabled());
            assert!(capture.try_lock().is_err());
            assert!(!worker.is_finished());
            restore.acknowledge(Ok(()));
            let outcome = worker.join().unwrap();
            assert_eq!(outcome.is_err(), work_fails);
            assert!(capture.try_lock().is_ok());
            mapping.finish(Ok(())).unwrap();
        }
    }
    #[test]
    fn philips_measurement_restore_failure_retains_progress_and_canceled_scope_never_starts() {
        use razer_ipc::mapping_control::Registry;
        let registry = Registry::default();
        let mut mapping = registry.register(uuid::Uuid::new_v4().to_string()).unwrap();
        let background_registry = registry.clone();
        let worker = std::thread::spawn(move || {
            with_capture_scope(
                &Mutex::new(()),
                &background_registry,
                &AtomicBool::new(true),
                || {
                    Ok(Completion {
                        progress: Some(progress()),
                        error: Some("native diagnostic".into()),
                    })
                },
            )
        });
        next_control(&mapping).acknowledge(Ok(()));
        next_control(&mapping).acknowledge(Err("simulated restore rejected".into()));
        let outcome = worker.join().unwrap().unwrap();
        assert!(outcome.progress.is_some());
        assert!(
            outcome
                .error
                .as_ref()
                .unwrap()
                .contains("native diagnostic")
        );
        assert!(
            outcome
                .error
                .unwrap()
                .contains("simulated restore rejected")
        );
        mapping.finish(Ok(())).unwrap();
        assert!(
            with_capture_scope(
                &Mutex::new(()),
                &Registry::default(),
                &AtomicBool::new(false),
                || panic!("Canceled capture must not create a native worker")
            )
            .is_err()
        );
    }
    #[test]
    fn philips_measurement_cancellation_uses_its_owner_and_streams_terminal() {
        let valid = AtomicBool::new(true);
        let mut calls = Vec::new();
        let mut updates = 0;
        let mut latest = None;
        let result = run_with(
            &target(),
            "owner",
            Kind::Manual,
            &valid,
            |request| {
                let mut progress = progress();
                match request {
                    ServiceRequest::MousePhilipsMeasurementStart { .. } => {
                        calls.push("start");
                        valid.store(false, Ordering::Release);
                    }
                    ServiceRequest::MousePhilipsMeasurementCancel { operation_id } => {
                        assert_eq!(operation_id, "owner");
                        calls.push("cancel");
                        progress.phase = Phase::Finished;
                        progress.terminal = true;
                        progress.canceled = true;
                        progress.cleanup_confirmed = true;
                    }
                    _ => panic!("Unexpected Philips endpoint"),
                }
                Ok(serde_json::to_value(progress)?)
            },
            &mut |_| {
                updates += 1;
                Ok(())
            },
            &mut latest,
        )
        .unwrap();
        assert_eq!(calls, ["start", "cancel"]);
        assert_eq!(updates, 2);
        assert!(result.canceled && result.cleanup_confirmed);
    }
    #[test]
    fn philips_measurement_owner_or_cleanup_mismatch_is_not_success() {
        let mut wrong = progress();
        wrong.operation_id = "replacement".into();
        assert!(validate(&wrong, &target(), "owner", Kind::Manual).is_err());
        let completion = complete(
            Ok(progress()),
            Ok(serde_json::json!({"stopped":true})),
            Some(&target()),
            "owner",
            Kind::Manual,
            None,
        );
        assert!(completion.error.unwrap().contains("omitted"));
    }
    #[test]
    fn philips_measurement_transport_error_keeps_last_ack_and_checks_shutdown_prefix() {
        use razer_device::mouse_philips::measurement::{CommandReport, DeviceCommand};
        let mut prior = progress();
        prior.commands.push(CommandReport {
            command: DeviceCommand::Configure,
            acknowledged: Some(vec![0, 5]),
            command_may_have_reached_device: true,
            error: None,
        });
        let failed = complete(
            Err(anyhow::anyhow!("poll pipe failed")),
            Err(anyhow::anyhow!("shutdown pipe failed")),
            Some(&target()),
            "owner",
            Kind::Manual,
            Some(prior.clone()),
        );
        assert_eq!(failed.progress.unwrap().commands, prior.commands);
        assert!(failed.error.unwrap().contains("shutdown pipe failed"));
        let mut released = prior.clone();
        released.phase = Phase::Finished;
        released.terminal = true;
        released.cleanup_confirmed = true;
        released.canceled = true;
        released.commands.push(CommandReport {
            command: DeviceCommand::SensorState(false),
            acknowledged: Some(vec![0, 5]),
            command_may_have_reached_device: true,
            error: None,
        });
        let outcome = complete(
            Err(anyhow::anyhow!("poll pipe failed")),
            Ok(serde_json::json!({"stopped":true,"mouse_philips_measurement_cleanup":released})),
            Some(&target()),
            "owner",
            Kind::Manual,
            Some(prior.clone()),
        );
        assert!(outcome.progress.unwrap().cleanup_confirmed);
        released.commands.remove(0);
        let outcome = complete(
            Err(anyhow::anyhow!("poll pipe failed")),
            Ok(serde_json::json!({"stopped":true,"mouse_philips_measurement_cleanup":released})),
            Some(&target()),
            "owner",
            Kind::Manual,
            Some(prior.clone()),
        );
        assert!(
            outcome
                .error
                .unwrap()
                .contains("discarded prior acknowledgements")
        );
        assert_eq!(outcome.progress.unwrap().commands, prior.commands);
    }
}
