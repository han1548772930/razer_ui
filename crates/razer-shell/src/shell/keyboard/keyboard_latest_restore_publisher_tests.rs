use super::{Publisher, Transport};
use razer_device::keyboard_latest_restore::{Coordinator, Generation};
use razer_ipc::{
    ServiceRequest,
    mapping_control::{self, Control, Receipt, Registry, Suppression},
};
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

const TIMEOUT: Duration = Duration::from_secs(3);
type Trace = Arc<Mutex<Vec<ServiceRequest>>>;
struct Mock {
    handler: Box<dyn FnMut(ServiceRequest) -> anyhow::Result<Value> + Send>,
    trace: Trace,
    dropped: mpsc::Sender<()>,
    on_drop: Option<Box<dyn FnOnce() + Send>>,
}
impl Transport for Mock {
    fn request(&mut self, request: ServiceRequest) -> anyhow::Result<Value> {
        self.trace.lock().unwrap().push(request.clone());
        (self.handler)(request)
    }
}
impl Drop for Mock {
    fn drop(&mut self) {
        if let Some(on_drop) = self.on_drop.take() {
            on_drop();
        }
        let _ = self.dropped.send(());
    }
}
fn mock(
    handler: impl FnMut(ServiceRequest) -> anyhow::Result<Value> + Send + 'static,
) -> (Mock, Trace, mpsc::Receiver<()>) {
    let trace = Arc::new(Mutex::new(Vec::new()));
    let (dropped, cleanup) = mpsc::channel();
    (
        Mock {
            handler: Box::new(handler),
            trace: trace.clone(),
            dropped,
            on_drop: None,
        },
        trace,
        cleanup,
    )
}
fn acknowledge(request: ServiceRequest) -> anyhow::Result<Value> {
    Ok(match request {
        ServiceRequest::MappingDeviceStart { owner_guid, .. } => {
            json!({"owner_guid":owner_guid,"registered":true,"driver_ready":true})
        }
        ServiceRequest::MappingDevicePublish { owner_guid, .. } => {
            json!({"owner_guid":owner_guid,"published":true})
        }
        ServiceRequest::MappingDeviceEvents { owner_guid } => {
            json!({"owner_guid":owner_guid,"events":[]})
        }
        ServiceRequest::MappingExecution {
            owner_guid,
            revision,
            target,
        } => serde_json::to_value(Receipt::new(owner_guid, revision, target))?,
        ServiceRequest::Shutdown => json!({"stopped":true}),
        request => panic!("unexpected request: {request:?}"),
    })
}
fn generation() -> Generation {
    let mut queue = Coordinator::default();
    let now = Instant::now();
    queue.request(1, (), now, Duration::ZERO);
    queue.take_ready(now).unwrap().generation
}
fn observe_broadcast(owner: &mapping_control::Owner) -> Control {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let mut commands = owner.drain();
        if !commands.is_empty() {
            assert_eq!(commands.len(), 1);
            return commands.pop().unwrap();
        }
        assert!(Instant::now() < deadline, "no mapping execution broadcast");
        thread::yield_now();
    }
}

#[test]
fn recorder_mode_comes_from_the_shared_input_owner_without_a_device_worker_query() {
    let (mock, trace, _) = mock(acknowledge);
    let mut publisher =
        Publisher::start_with("container", || Ok(mock), Registry::default()).unwrap();
    let before = trace.lock().unwrap().len();
    assert_eq!(
        publisher
            .macro_mode_with(&generation(), || Ok(
                razer_platform::macro_recorder::Mode::Otfm
            ))
            .unwrap(),
        "kOTFM"
    );
    assert_eq!(trace.lock().unwrap().len(), before);
    let error = publisher
        .macro_mode_with(&generation(), || anyhow::bail!("recorder owner closed"))
        .unwrap_err();
    assert!(error.to_string().contains("recorder owner closed"));
    assert_eq!(trace.lock().unwrap().len(), before);
    publisher.finish().unwrap();
}

#[test]
fn partial_or_mismatched_registration_still_shuts_down_and_releases_transport() {
    for receipt in [
        json!({}),
        json!({"owner_guid":"stale","registered":true,"driver_ready":true}),
        json!({"registered":false,"driver_ready":true}),
        json!({"registered":true,"driver_ready":false}),
    ] {
        let (mock, trace, dropped) = mock(move |request| match request {
            ServiceRequest::MappingDeviceStart { .. } => Ok(receipt.clone()),
            request => acknowledge(request),
        });
        let registry = Registry::default();
        assert!(Publisher::start_with("container", || Ok(mock), registry.clone()).is_err());
        dropped.recv_timeout(TIMEOUT).unwrap();
        let trace = trace.lock().unwrap();
        assert_eq!(trace.len(), 2);
        assert!(matches!(
            trace[0],
            ServiceRequest::MappingDeviceStart {
                product_id: 756,
                ..
            }
        ));
        assert!(matches!(trace[1], ServiceRequest::Shutdown));
        let mut scope = registry.suspend(Suppression::Mapping).unwrap();
        scope.release().unwrap();
    }
}

#[test]
fn owner_started_during_capture_confirms_inherited_control_before_publication() {
    let registry = Registry::default();
    let mut scope = registry.suspend(Suppression::MappingAndShortcuts).unwrap();
    let (mock, trace, dropped) = mock(acknowledge);
    let mut publisher = Publisher::start_with("container", || Ok(mock), registry).unwrap();
    publisher
        .publish(json!({"actual":"saved document"}), &generation())
        .unwrap();
    {
        let trace = trace.lock().unwrap();
        assert!(matches!(
            trace[0],
            ServiceRequest::MappingDeviceStart { .. }
        ));
        assert!(
            matches!(trace[1], ServiceRequest::MappingExecution {target, ..}
            if !target.mapping_enabled() && target.global_shortcuts_enabled() == Some(false))
        );
        assert!(matches!(
            trace[2],
            ServiceRequest::MappingDevicePublish { .. }
        ));
    }
    publisher.finish().unwrap();
    dropped.recv_timeout(TIMEOUT).unwrap();
    scope.release().unwrap();
}

#[test]
fn retained_owner_acknowledges_pause_and_restore_in_its_own_transport() {
    let registry = Registry::default();
    let mut observer = registry.register(uuid::Uuid::new_v4().to_string()).unwrap();
    let (mock, trace, _) = mock(acknowledge);
    let mut publisher = Publisher::start_with("container", || Ok(mock), registry.clone()).unwrap();
    let requested = registry.clone();
    let pause = thread::spawn(move || requested.suspend(Suppression::MappingAndShortcuts));
    let control = observe_broadcast(&observer);
    assert!(!control.target().mapping_enabled());
    publisher.apply_control().unwrap();
    control.acknowledge(Ok(()));
    let mut scope = pause.join().unwrap().unwrap();
    publisher
        .publish(json!({"admitted":true}), &generation())
        .unwrap();
    let restore = thread::spawn(move || scope.release());
    let control = observe_broadcast(&observer);
    assert!(control.target().mapping_enabled());
    assert_eq!(control.target().global_shortcuts_enabled(), Some(true));
    publisher.apply_control().unwrap();
    control.acknowledge(Ok(()));
    restore.join().unwrap().unwrap();
    let controls: Vec<_> = trace
        .lock()
        .unwrap()
        .iter()
        .filter_map(|request| {
            if let ServiceRequest::MappingExecution { target, .. } = request {
                Some(*target)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(controls.len(), 2);
    assert!(!controls[0].mapping_enabled());
    assert!(controls[1].mapping_enabled());
    publisher.finish().unwrap();
    observer.finish(Ok(())).unwrap();
}

#[test]
fn closing_owner_satisfies_pending_scope_only_after_shutdown_ack_and_transport_drop() {
    let registry = Registry::default();
    let mut observer = registry.register(uuid::Uuid::new_v4().to_string()).unwrap();
    let (entered, shutdown_entered) = mpsc::channel();
    let (acknowledge_shutdown, proceed_shutdown) = mpsc::channel();
    let (released, drop_entered) = mpsc::channel();
    let (finish_drop, proceed_drop) = mpsc::channel();
    let (mut mock, _, dropped) = mock(move |request| {
        if matches!(request, ServiceRequest::Shutdown) {
            entered.send(()).unwrap();
            proceed_shutdown.recv_timeout(TIMEOUT).unwrap();
        }
        acknowledge(request)
    });
    mock.on_drop = Some(Box::new(move || {
        released.send(()).unwrap();
        proceed_drop.recv_timeout(TIMEOUT).unwrap();
    }));
    let publisher = Publisher::start_with("container", || Ok(mock), registry.clone()).unwrap();
    let finish = thread::spawn(move || publisher.finish());
    shutdown_entered.recv_timeout(TIMEOUT).unwrap();
    let (completed, completion) = mpsc::channel();
    let pending = thread::spawn(move || {
        let result = registry.suspend(Suppression::Mapping);
        completed.send(result).unwrap();
    });
    let control = observe_broadcast(&observer);
    control.acknowledge(Ok(()));
    assert!(matches!(
        completion.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    acknowledge_shutdown.send(()).unwrap();
    drop_entered.recv_timeout(TIMEOUT).unwrap();
    assert!(matches!(
        completion.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    finish_drop.send(()).unwrap();
    dropped.recv_timeout(TIMEOUT).unwrap();
    finish.join().unwrap().unwrap();
    let mut scope = completion.recv_timeout(TIMEOUT).unwrap().unwrap();
    pending.join().unwrap();
    observer.finish(Ok(())).unwrap();
    scope.release().unwrap();
}

#[test]
fn failed_cleanup_is_reported_to_pending_scope_after_transport_release() {
    for panic in [false, true] {
        let registry = Registry::default();
        let mut observer = registry.register(uuid::Uuid::new_v4().to_string()).unwrap();
        let (mock, _, dropped) = mock(move |request| {
            if matches!(request, ServiceRequest::Shutdown) {
                assert!(!panic, "simulated cleanup panic");
                return Ok(json!({"stopped":false}));
            }
            acknowledge(request)
        });
        let publisher = Publisher::start_with("container", || Ok(mock), registry.clone()).unwrap();
        let pause = thread::spawn(move || registry.suspend(Suppression::Mapping));
        let control = observe_broadcast(&observer);
        assert!(publisher.finish().is_err());
        dropped.recv_timeout(TIMEOUT).unwrap();
        control.acknowledge(Ok(()));
        observer.finish(Ok(())).unwrap();
        assert!(pause.join().unwrap().is_err());
    }
}

#[test]
fn invalid_initial_control_receipt_fails_start_and_retains_cleanup() {
    let registry = Registry::default();
    let mut scope = registry.suspend(Suppression::Mapping).unwrap();
    let (mock, trace, dropped) = mock(|request| {
        if let ServiceRequest::MappingExecution {
            owner_guid,
            revision,
            target,
        } = request
        {
            return Ok(serde_json::to_value(Receipt::new(
                owner_guid,
                revision + 1,
                target,
            ))?);
        }
        acknowledge(request)
    });
    assert!(Publisher::start_with("container", || Ok(mock), registry).is_err());
    dropped.recv_timeout(TIMEOUT).unwrap();
    let trace = trace.lock().unwrap();
    assert_eq!(trace.len(), 3);
    assert!(matches!(trace[1], ServiceRequest::MappingExecution { .. }));
    assert!(matches!(trace[2], ServiceRequest::Shutdown));
    scope.release().unwrap();
}
