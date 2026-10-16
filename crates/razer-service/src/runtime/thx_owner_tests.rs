use super::*;
use razer_device::thx_v4::{
    ThxDeviceIdentity, ThxOperation, ThxRenderSetupTrigger, ThxReply, ThxTransportKind,
    ThxV4Transport,
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

fn scope(container: &str, profile: &str) -> ThxSubmissionScope {
    ThxSubmissionScope {
        product_id: 1398,
        container_id: container.into(),
        serial_number: "owner-serial".into(),
        profile_id: profile.into(),
        generation: 7,
    }
}

fn setup_plan() -> ThxSubmissionPlan {
    ThxSubmissionPlan::SetupRender {
        trigger: ThxRenderSetupTrigger::InitialDeviceSetup,
    }
}

#[derive(Clone)]
struct Events {
    values: Arc<Mutex<Vec<String>>>,
}
impl Events {
    fn new() -> Self {
        Self {
            values: Arc::new(Mutex::new(Vec::new())),
        }
    }
    fn push(&self, value: impl Into<String>) {
        self.values.lock().unwrap().push(value.into());
    }
    fn snapshot(&self) -> Vec<String> {
        self.values.lock().unwrap().clone()
    }
}

struct FakeTransport {
    events: Events,
}
impl ThxJobTransport for FakeTransport {}
impl ThxV4Transport for FakeTransport {
    fn kind(&self) -> ThxTransportKind {
        ThxTransportKind::Simulated
    }

    fn exchange(&mut self, _: &ThxDeviceIdentity, operation: &ThxOperation) -> Result<ThxReply> {
        match operation {
            ThxOperation::AddDevice => {
                self.events.push("add");
                Ok(ThxReply::LocalBinding(true))
            }
            ThxOperation::CheckStatus => {
                self.events.push("check");
                Ok(ThxReply::LocalReady(true))
            }
            ThxOperation::SetProperty(request) => {
                self.events
                    .push(format!("set:{}={}", request.id, request.value));
                Ok(ThxReply::RpcAcknowledged)
            }
            ThxOperation::GetProperty(id) => {
                self.events.push(format!("get:{id}"));
                Ok(ThxReply::Property(Some(serde_json::json!(true))))
            }
            ThxOperation::RemoveDevice => {
                self.events.push("remove");
                Ok(ThxReply::LocalBinding(true))
            }
            _ => panic!("unexpected firmware operation in retained-owner test"),
        }
    }
}

fn finish(controller: &mut Controller, operation_id: &str) -> ThxSubmissionProgress {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let ThxOperationStatus::Finished(progress) = controller.poll(operation_id).unwrap() {
            return progress;
        }
        assert!(Instant::now() < deadline, "operation did not finish");
        thread::yield_now();
    }
}

fn start_owner(controller: &mut Controller, events: &Events) {
    let retained_events = events.clone();
    controller
        .start_owner_with(
            "owner".into(),
            "setup".into(),
            scope("container-a", "profile-a"),
            setup_plan(),
            move |_| {
                Ok(FakeTransport {
                    events: retained_events,
                })
            },
        )
        .unwrap();
}

#[test]
fn retained_owner_reuses_transport_and_preserves_source_shutdown_order() {
    let events = Events::new();
    let mut controller = Controller::default();
    start_owner(&mut controller, &events);
    let setup = finish(&mut controller, "setup");
    assert!(setup.error.is_none());
    assert!(setup.render_setup.as_ref().unwrap().setup_succeeded);

    let changed_profile = scope("container-a", "profile-b");
    controller
        .submit_owner(
            "owner",
            "read".into(),
            changed_profile.clone(),
            ThxSubmissionPlan::ReadSpatial,
        )
        .unwrap();
    let read = finish(&mut controller, "read");
    assert_eq!(read.outcome.as_ref().unwrap().readback, Some(true));

    controller
        .submit_owner(
            "owner",
            "rebind".into(),
            changed_profile.clone(),
            ThxSubmissionPlan::SetupRender {
                trigger: ThxRenderSetupTrigger::DefaultPlaybackDeviceChanged,
            },
        )
        .unwrap();
    let rebind = finish(&mut controller, "rebind");
    assert!(rebind.render_setup.as_ref().unwrap().setup_succeeded);

    controller
        .submit_owner(
            "owner",
            "shutdown".into(),
            changed_profile,
            ThxSubmissionPlan::ShutdownRender,
        )
        .unwrap();
    let shutdown = finish(&mut controller, "shutdown");
    assert!(
        shutdown
            .render_shutdown
            .as_ref()
            .unwrap()
            .shutdown_completed
    );
    assert!(shutdown.error.is_none());
    assert!(controller.owners.is_empty());

    assert_eq!(
        events.snapshot(),
        vec![
            "add",
            "check",
            "set:upmix_enabled=true",
            "set:eq_enabled=true",
            "set:spatial_advanced_early_reflections_enabled=true",
            "set:spatial_advanced_late_reverb_enabled=true",
            "get:spatial_enabled",
            "remove",
            "add",
            "check",
            "set:upmix_enabled=true",
            "set:eq_enabled=true",
            "set:spatial_advanced_early_reflections_enabled=true",
            "set:spatial_advanced_late_reverb_enabled=true",
            "set:eq_enabled=false",
            "set:spatial_enabled=false",
            "remove",
        ]
    );
}

#[test]
fn operation_cancellation_does_not_poison_the_next_owner_operation() {
    let owner = Cancellation::default();
    let gate = Cancellation::owner_gate(&owner);
    let first = Cancellation::default();
    gate.begin_operation(&first).unwrap();
    first.cancel();
    assert!(gate.check().is_err());
    gate.end_operation().unwrap();

    let second = Cancellation::default();
    gate.begin_operation(&second).unwrap();
    assert!(gate.check().is_ok());
    owner.cancel();
    assert!(gate.check().is_err());
    gate.end_operation().unwrap();
}

#[test]
fn retained_owner_rejects_physical_mismatch_and_duplicate_endpoint_owner() {
    let events = Events::new();
    let mut controller = Controller::default();
    start_owner(&mut controller, &events);
    finish(&mut controller, "setup");

    let wrong_identity = ThxSubmissionScope {
        container_id: "container-other".into(),
        ..scope("container-a", "profile-b")
    };
    assert!(
        controller
            .submit_owner(
                "owner",
                "wrong".into(),
                wrong_identity,
                ThxSubmissionPlan::ReadSpatial,
            )
            .is_err()
    );

    let before_duplicate = events.snapshot();
    assert!(
        controller
            .start_owner_with(
                "owner-2".into(),
                "other".into(),
                scope("container-b", "profile-c"),
                setup_plan(),
                move |_| -> Result<FakeTransport> {
                    panic!("duplicate endpoint must be rejected before opening transport")
                },
            )
            .is_err()
    );
    assert_eq!(events.snapshot(), before_duplicate);

    controller.release_owner("owner").unwrap();
    assert!(controller.owners.is_empty());
}

#[test]
fn idle_release_runs_resource_remove_without_inventing_source_shutdown_writes() {
    let events = Events::new();
    let mut controller = Controller::default();
    start_owner(&mut controller, &events);
    finish(&mut controller, "setup");
    let result = controller.release_owner("owner").unwrap();
    assert_eq!(result["released"], true);
    assert_eq!(
        events.snapshot(),
        vec![
            "add",
            "check",
            "set:upmix_enabled=true",
            "set:eq_enabled=true",
            "set:spatial_advanced_early_reflections_enabled=true",
            "set:spatial_advanced_late_reverb_enabled=true",
            "remove",
        ]
    );
}

struct BlockingTransport {
    events: Events,
    entered: mpsc::Sender<()>,
    cancellation: Cancellation,
    dropped: Arc<AtomicBool>,
}
impl Drop for BlockingTransport {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Release);
    }
}
impl ThxJobTransport for BlockingTransport {}
impl ThxV4Transport for BlockingTransport {
    fn kind(&self) -> ThxTransportKind {
        ThxTransportKind::Simulated
    }
    fn exchange(&mut self, _: &ThxDeviceIdentity, operation: &ThxOperation) -> Result<ThxReply> {
        if matches!(operation, ThxOperation::AddDevice) {
            self.events.push("add");
            let _ = self.entered.send(());
            let deadline = Instant::now() + Duration::from_secs(3);
            while self.cancellation.check().is_ok() {
                assert!(
                    Instant::now() < deadline,
                    "owner release did not cancel the busy worker"
                );
                thread::yield_now();
            }
            return Ok(ThxReply::LocalBinding(true));
        }
        FakeTransport {
            events: self.events.clone(),
        }
        .exchange(&ThxDeviceIdentity::new("x", "y")?, operation)
    }
}

#[test]
fn release_owner_joins_a_busy_worker_before_returning() {
    let events = Events::new();
    let dropped = Arc::new(AtomicBool::new(false));
    let (entered, wait_entered) = mpsc::channel();
    let mut controller = Controller::default();
    let owner_dropped = dropped.clone();
    let owner_events = events.clone();
    controller
        .start_owner_with(
            "owner".into(),
            "setup".into(),
            scope("container-a", "profile-a"),
            setup_plan(),
            move |cancellation| {
                Ok(BlockingTransport {
                    events: owner_events,
                    entered,
                    cancellation,
                    dropped: owner_dropped,
                })
            },
        )
        .unwrap();
    wait_entered.recv_timeout(Duration::from_secs(2)).unwrap();
    let result = controller.release_owner("owner").unwrap();
    assert_eq!(result["released"], true);
    assert!(result["cleanup_error"].is_null());
    assert!(dropped.load(Ordering::Acquire));
    assert!(controller.owners.is_empty());
}

#[test]
fn panicking_owner_reports_unknown_result_and_release_join_failure() {
    let mut controller = Controller::default();
    controller
        .start_owner_with(
            "owner".into(),
            "setup".into(),
            scope("container-a", "profile-a"),
            setup_plan(),
            |_| -> Result<FakeTransport> { panic!("synthetic owner open panic") },
        )
        .unwrap();
    let progress = finish(&mut controller, "setup");
    assert!(progress.error.unwrap().contains("disconnected"));
    let released = controller.release_owner("owner").unwrap();
    assert!(
        released["cleanup_error"]
            .as_str()
            .unwrap()
            .contains("panicked")
    );
}

#[test]
fn one_off_shutdown_is_refused_without_a_retained_owner() {
    let events = Events::new();
    let mut controller = Controller::default();
    controller
        .start_with(
            "oneoff".into(),
            scope("container-a", "profile-a"),
            ThxSubmissionPlan::ShutdownRender,
            move |_| {
                Ok(FakeTransport {
                    events: events.clone(),
                })
            },
        )
        .unwrap();
    let progress = finish(&mut controller, "oneoff");
    assert!(
        progress
            .error
            .unwrap()
            .contains("existing retained render owner")
    );
    assert!(progress.render_shutdown.is_none());
    assert!(controller.owners.is_empty());
}
