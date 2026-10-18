use super::*;
use serde_json::json;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

struct Mock {
    handler: Box<dyn FnMut(ServiceRequest) -> anyhow::Result<Value> + Send>,
    stopped: bool,
    dropped: Option<mpsc::Sender<thread::ThreadId>>,
}
impl Mock {
    fn new(handler: impl FnMut(ServiceRequest) -> anyhow::Result<Value> + Send + 'static) -> Self {
        Self {
            handler: Box::new(handler),
            stopped: false,
            dropped: None,
        }
    }
}
impl Transport for Mock {
    fn request(&mut self, request: ServiceRequest) -> anyhow::Result<Value> {
        self.stopped |= matches!(request, ServiceRequest::Shutdown);
        (self.handler)(request)
    }
    fn is_stopped(&self) -> bool {
        self.stopped
    }
}
impl Drop for Mock {
    fn drop(&mut self) {
        if let Some(dropped) = &self.dropped {
            let _ = dropped.send(thread::current().id());
        }
    }
}

#[test]
fn recorder_mode_queries_the_actual_retained_transport_on_every_observation() {
    let registry = Registry::default();
    assert!(registry.mode().is_err());
    let mut active = None;
    let mut owner = registry
        .register(Mock::new(move |request| match request {
            ServiceRequest::StartMacroRecording => {
                active = Some("kSoftware");
                Ok(json!({"accepted":true}))
            }
            ServiceRequest::StartOtfmRecording => {
                active = Some("kOTFM");
                Ok(json!({"accepted":true}))
            }
            ServiceRequest::StopMacroRecording => {
                active = None;
                Ok(json!({"accepted":true}))
            }
            ServiceRequest::MacroRecordingMode => match active {
                Some(mode) => Ok(json!({"mode":mode})),
                None => anyhow::bail!("macro recorder has not started"),
            },
            other => panic!("unexpected request: {other:?}"),
        }))
        .unwrap();
    assert!(registry.mode().is_err());
    owner.request(ServiceRequest::StartMacroRecording).unwrap();
    assert_eq!(registry.mode().unwrap(), Mode::Software);
    owner.request(ServiceRequest::StopMacroRecording).unwrap();
    assert!(registry.mode().is_err());
    owner.request(ServiceRequest::StartOtfmRecording).unwrap();
    assert_eq!(registry.mode().unwrap(), Mode::Otfm);
    drop(owner);
    assert!(registry.mode().is_err());
}

#[test]
fn recorder_mode_does_not_default_missing_unknown_or_failed_observations() {
    for value in [json!({}), json!({"mode":null}), json!({"mode":"future"})] {
        let registry = Registry::default();
        let _owner = registry
            .register(Mock::new(move |_| Ok(value.clone())))
            .unwrap();
        assert!(registry.mode().is_err());
    }
    let registry = Registry::default();
    let _owner = registry
        .register(Mock::new(|_| {
            anyhow::bail!("recorder input channel failed")
        }))
        .unwrap();
    assert!(
        registry
            .mode()
            .unwrap_err()
            .to_string()
            .contains("input channel failed")
    );
}

#[test]
fn recorder_shutdown_retires_queries_even_when_cleanup_reports_failure() {
    let registry = Registry::default();
    let mut owner = registry
        .register(Mock::new(|request| match request {
            ServiceRequest::MacroRecordingMode => Ok(json!({"mode":"kSoftware"})),
            ServiceRequest::Shutdown => anyhow::bail!("unhook failed"),
            other => panic!("unexpected request: {other:?}"),
        }))
        .unwrap();
    assert_eq!(registry.mode().unwrap(), Mode::Software);
    assert!(owner.request(ServiceRequest::Shutdown).is_err());
    assert!(registry.mode().unwrap_err().to_string().contains("closing"));
}

#[test]
fn recorder_query_borrow_does_not_retain_child_or_clear_a_successor() {
    let registry = Registry::default();
    let (dropped, cleanup) = mpsc::channel();
    let mut mock = Mock::new(|_| Ok(json!({"mode":"kSoftware"})));
    mock.dropped = Some(dropped);
    let owner = registry.register(mock).unwrap();
    let borrowed = registry.owner.lock().unwrap().upgrade().unwrap();
    drop(owner);
    assert_eq!(
        cleanup.recv_timeout(Duration::from_secs(3)).unwrap(),
        thread::current().id()
    );
    assert!(borrowed.lock().unwrap().transport.is_none());
    assert!(borrowed.lock().unwrap().closing);
    let successor = registry
        .register(Mock::new(|_| Ok(json!({"mode":"kOTFM"}))))
        .unwrap();
    drop(borrowed);
    assert_eq!(registry.mode().unwrap(), Mode::Otfm);
    drop(successor);
    assert!(registry.mode().is_err());
}

#[test]
fn recorder_mode_query_serializes_with_owner_shutdown_without_registry_lock() {
    let registry = Registry::default();
    let (entered, observed) = mpsc::channel();
    let (resume, proceed) = mpsc::channel();
    let (dropped, cleanup) = mpsc::channel();
    let mut mock = Mock::new(move |request| match request {
        ServiceRequest::MacroRecordingMode => {
            entered.send(()).unwrap();
            proceed.recv_timeout(Duration::from_secs(3)).unwrap();
            Ok(json!({"mode":"kOTFM"}))
        }
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        other => panic!("unexpected request: {other:?}"),
    });
    mock.dropped = Some(dropped);
    let mut owner = registry.register(mock).unwrap();
    let reader = registry.clone();
    let query = thread::spawn(move || reader.mode());
    observed.recv_timeout(Duration::from_secs(3)).unwrap();
    // A blocked pipe query must not hold the registration/suppression boundary.
    assert!(registry.owner.try_lock().is_ok());
    let closer = thread::spawn(move || {
        owner.request(ServiceRequest::Shutdown).unwrap();
        drop(owner);
        thread::current().id()
    });
    resume.send(()).unwrap();
    assert_eq!(query.join().unwrap().unwrap(), Mode::Otfm);
    let closer_id = closer.join().unwrap();
    assert_eq!(
        cleanup.recv_timeout(Duration::from_secs(3)).unwrap(),
        closer_id
    );
    assert!(registry.mode().is_err());
}

#[test]
fn recorder_does_not_admit_two_mutable_owners() {
    let registry = Registry::default();
    let owner = registry
        .register(Mock::new(|_| Ok(json!({"mode":"kSoftware"}))))
        .unwrap();
    assert!(registry.register(Mock::new(|_| unreachable!())).is_err());
    assert_eq!(registry.mode().unwrap(), Mode::Software);
    drop(owner);
}
