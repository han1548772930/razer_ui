//! Jobs exercise only scripted Rust transports and memory channels.
use super::*;
use razer_device::thx_v4::{
    SimulatedThxV4Transport, SpatialStatus, ThxDeviceIdentity, ThxOperation, ThxReply,
    ThxTransportKind, ThxV4Transport,
};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
impl ThxJobTransport for SimulatedThxV4Transport {}
fn scope() -> ThxSubmissionScope {
    ThxSubmissionScope {
        product_id: 1398,
        container_id: "mock-container".into(),
        serial_number: "mock".into(),
        profile_id: "local-profile".into(),
        generation: 11,
    }
}
struct Fake {
    events: Arc<Mutex<Vec<&'static str>>>,
    entered: Option<mpsc::Sender<()>>,
    release: Option<mpsc::Receiver<()>>,
}
impl Drop for Fake {
    fn drop(&mut self) {
        self.events.lock().unwrap().push("drop");
    }
}
impl ThxJobTransport for Fake {}
impl ThxV4Transport for Fake {
    fn kind(&self) -> ThxTransportKind {
        ThxTransportKind::Simulated
    }
    fn exchange(&mut self, _: &ThxDeviceIdentity, operation: &ThxOperation) -> Result<ThxReply> {
        match operation {
            ThxOperation::AddDevice => {
                self.events.lock().unwrap().push("add");
                Ok(ThxReply::LocalBinding(true))
            }
            ThxOperation::CheckStatus => Ok(ThxReply::LocalReady(true)),
            ThxOperation::RemoveDevice => {
                self.events.lock().unwrap().push("remove");
                Ok(ThxReply::LocalBinding(true))
            }
            ThxOperation::GetProperty(id) => {
                assert_eq!(id, "spatial_enabled");
                if let Some(entered) = self.entered.take() {
                    entered.send(()).unwrap();
                }
                if let Some(release) = self.release.take() {
                    release.recv_timeout(Duration::from_secs(2)).unwrap();
                }
                Ok(ThxReply::Property(Some(serde_json::json!(true))))
            }
            ThxOperation::SetProperty(request) => {
                let event = match request.id.as_str() {
                    "upmix_enabled" => "upmix",
                    "eq_enabled" => "eq",
                    "spatial_advanced_early_reflections_enabled" => "early",
                    "spatial_advanced_late_reverb_enabled" => "late",
                    _ => panic!("Unscripted fake property"),
                };
                self.events.lock().unwrap().push(event);
                if let Some(entered) = self.entered.take() {
                    entered.send(()).unwrap();
                }
                if let Some(release) = self.release.take() {
                    release.recv_timeout(Duration::from_secs(2)).unwrap();
                }
                Ok(ThxReply::RpcAcknowledged)
            }
            _ => panic!("Unscripted fake write"),
        }
    }
}

#[test]
fn thx_lifecycle_owned_job_keeps_partial_ack_and_scope_until_cancel_teardown() {
    let events = Arc::new(Mutex::new(vec![]));
    let recorded = events.clone();
    let (entered, wait_entered) = mpsc::channel();
    let (release, wait_release) = mpsc::channel();
    let mut c = Controller::default();
    let plan = ThxSubmissionPlan::SetupRender {
        trigger: razer_device::thx_v4::ThxRenderSetupTrigger::InitialDeviceSetup,
    };
    c.start_with("op".into(), scope(), plan.clone(), move |_| {
        Ok(Fake {
            events: recorded,
            entered: Some(entered),
            release: Some(wait_release),
        })
    })
    .unwrap();
    wait_entered.recv_timeout(Duration::from_secs(2)).unwrap();
    c.cancel("op").unwrap();
    assert!(
        c.start_with(
            "successor".into(),
            scope(),
            plan.clone(),
            |_| -> Result<SimulatedThxV4Transport> {
                panic!("Canceled job must retain its transport slot until teardown")
            }
        )
        .is_err()
    );
    release.send(()).unwrap();
    let progress = finish(&mut c);
    assert_eq!(progress.plan, plan);
    assert_eq!(progress.scope, scope());
    assert!(progress.error.is_some() && progress.cleanup_error.is_none());
    assert!(progress.outcome.is_none());
    let setup = progress.render_setup.unwrap();
    assert_eq!(
        setup.status,
        razer_device::thx_v4::ThxRenderSetupStatus::Canceled
    );
    assert!(!setup.setup_succeeded);
    assert_eq!(setup.receipts.len(), 3);
    assert_eq!(setup.receipts[2].reply, Some(ThxReply::RpcAcknowledged));
    assert_eq!(
        *events.lock().unwrap(),
        vec!["add", "upmix", "remove", "drop"]
    );
    assert!(c.jobs.is_empty());
}
fn finish(c: &mut Controller) -> ThxSubmissionProgress {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let ThxOperationStatus::Finished(progress) = c.poll("op").unwrap() {
            return progress;
        }
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
}
#[test]
fn thx_job_removes_and_drops_transport_before_completion() {
    let events = Arc::new(Mutex::new(vec![]));
    let received = events.clone();
    let mut c = Controller::default();
    c.start_with(
        "op".into(),
        scope(),
        ThxSubmissionPlan::ReadSpatial,
        move |_| {
            Ok(Fake {
                events: received,
                entered: None,
                release: None,
            })
        },
    )
    .unwrap();
    let progress = finish(&mut c);
    assert_eq!(progress.outcome.unwrap().status, SpatialStatus::Observed);
    assert_eq!(*events.lock().unwrap(), vec!["add", "remove", "drop"]);
    assert!(c.jobs.is_empty());
}
#[test]
fn thx_job_cancel_keeps_ownership_until_teardown_and_filters_late_read() {
    let events = Arc::new(Mutex::new(vec![]));
    let e = events.clone();
    let mut c = Controller::default();
    let (entered, wait) = mpsc::channel();
    let (release, recv) = mpsc::channel();
    c.start_with(
        "op".into(),
        scope(),
        ThxSubmissionPlan::ReadSpatial,
        move |_| {
            Ok(Fake {
                events: e,
                entered: Some(entered),
                release: Some(recv),
            })
        },
    )
    .unwrap();
    wait.recv_timeout(Duration::from_secs(2)).unwrap();
    c.cancel("op").unwrap();
    assert!(matches!(c.poll("op").unwrap(), ThxOperationStatus::Running));
    assert!(
        c.start_with(
            "new".into(),
            scope(),
            ThxSubmissionPlan::ReadSpatial,
            |_| -> Result<Fake> { panic!("No second transport") }
        )
        .is_err()
    );
    release.send(()).unwrap();
    let p = finish(&mut c);
    assert_eq!(p.outcome.unwrap().status, SpatialStatus::Canceled);
    assert_eq!(events.lock().unwrap().last(), Some(&"drop"));
}
#[test]
fn thx_job_factory_error_is_explicit_with_no_ack_or_observation() {
    let mut c = Controller::default();
    c.start_with(
        "op".into(),
        scope(),
        ThxSubmissionPlan::ReadSpatial,
        |_| -> Result<Fake> { anyhow::bail!("synthetic open failure") },
    )
    .unwrap();
    let p = finish(&mut c);
    assert!(p.outcome.is_none() && p.firmware.is_empty());
    assert!(p.error.unwrap().contains("open failure"));
}
#[test]
fn thx_job_invalid_scope_and_limits_fail_before_factory() {
    let mut c = Controller::default();
    for product in [1427, 0, u32::MAX] {
        let mut s = scope();
        s.product_id = product;
        assert!(
            c.start_with(
                "op".into(),
                s,
                ThxSubmissionPlan::ReadSpatial,
                |_| -> Result<Fake> { panic!() }
            )
            .is_err()
        );
    }
    let mut s = scope();
    s.container_id = "hid-collection:synthetic".into();
    assert!(
        c.start_with(
            "op".into(),
            s,
            ThxSubmissionPlan::ReadSpatial,
            |_| -> Result<Fake> { panic!() }
        )
        .is_err()
    );
    let plan = ThxSubmissionPlan::SetSpatial {
        enabled: true,
        presets: vec![serde_json::Value::Null; 257],
        timer_tick: 0,
    };
    assert!(
        c.start_with("op".into(), scope(), plan, |_| -> Result<Fake> { panic!() })
            .is_err()
    );
}
#[test]
fn thx_job_drop_cancels_and_joins_every_retained_worker() {
    let events = Arc::new(Mutex::new(vec![]));
    let e = events.clone();
    let mut c = Controller::default();
    let (entered, wait) = mpsc::channel();
    c.start_with(
        "op".into(),
        scope(),
        ThxSubmissionPlan::ReadSpatial,
        move |cancel| {
            entered.send(()).unwrap();
            while cancel.check().is_ok() {
                thread::yield_now();
            }
            Ok(Fake {
                events: e,
                entered: None,
                release: None,
            })
        },
    )
    .unwrap();
    wait.recv_timeout(Duration::from_secs(2)).unwrap();
    drop(c);
    assert_eq!(events.lock().unwrap().last(), Some(&"drop"));
}

#[test]
fn thx_job_shared_product_endpoint_rejects_another_container_until_teardown() {
    let mut c = Controller::default();
    let (entered, wait) = mpsc::channel();
    c.start_with(
        "op".into(),
        scope(),
        ThxSubmissionPlan::ReadSpatial,
        move |cancel| -> Result<Fake> {
            entered.send(()).unwrap();
            while cancel.check().is_ok() {
                thread::yield_now();
            }
            anyhow::bail!("synthetic canceled open")
        },
    )
    .unwrap();
    wait.recv_timeout(Duration::from_secs(2)).unwrap();
    let mut other = scope();
    other.container_id = "another-container".into();
    let error = c
        .start_with(
            "other".into(),
            other,
            ThxSubmissionPlan::ReadSpatial,
            |_| -> Result<Fake> { panic!("No ambiguous endpoint transport") },
        )
        .unwrap_err();
    assert!(error.to_string().contains("product/endpoint"));
    c.cancel("op").unwrap();
    assert!(finish(&mut c).error.is_some());
    assert!(c.jobs.is_empty());
}

#[test]
fn thx_job_worker_panic_returns_terminal_unknown_result_and_releases_scope() {
    let mut c = Controller::default();
    let plan = ThxSubmissionPlan::SetSpatial {
        enabled: true,
        presets: vec![],
        timer_tick: 42,
    };
    c.start_with("op".into(), scope(), plan.clone(), |_| -> Result<Fake> {
        panic!("Synthetic Rust worker unwind")
    })
    .unwrap();
    let progress = finish(&mut c);
    assert_eq!(progress.scope, scope());
    assert_eq!(progress.plan, plan);
    assert!(progress.error.unwrap().contains("outcome is unknown"));
    assert!(progress.outcome.is_none() && progress.initial_observation.is_none());
    assert!(progress.firmware.is_empty());
    assert!(c.jobs.is_empty());
    assert!(c.cancel("op").is_err());
    c.start_with(
        "op".into(),
        scope(),
        ThxSubmissionPlan::ReadSpatial,
        |_| -> Result<Fake> { anyhow::bail!("synthetic next job") },
    )
    .unwrap();
    assert!(finish(&mut c).error.unwrap().contains("next job"));
}
