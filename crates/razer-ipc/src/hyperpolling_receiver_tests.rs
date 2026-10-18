//! Independent fixed wire frames, never a worker process or device operation.
use super::*;
use razer_device::hyperpolling_receiver::{Command, Observation, Reading, Target};

fn target() -> Target {
    Target::WindowsContainer {
        path: "physical-path".into(),
        container: "physical-container".into(),
    }
}

#[test]
fn hyperpolling_receiver_request_has_fixed_tag_and_typed_write_byte() {
    let expected=br#"{"operation":"hyperpolling_receiver","target":{"WindowsContainer":{"path":"physical-path","container":"physical-container"}},"command":{"SetIndicator":{"status":254}},"transaction":226}"#;
    let request = ServiceRequest::HyperpollingReceiver {
        target: target(),
        command: Command::SetIndicator { status: 254 },
        transaction: 226,
    };
    assert_eq!(serde_json::to_vec(&request).unwrap(), expected);
    let decoded: ServiceRequest = serde_json::from_slice(expected).unwrap();
    assert!(matches!(
        decoded,
        ServiceRequest::HyperpollingReceiver {
            command: Command::SetIndicator { status: 254 },
            ..
        }
    ));
    for invalid in [
        br#"{"operation":"hyperpolling_receiver","command":"Indicator"}"#.as_slice(),
        br#"{"operation":"hyperpolling_receiver","target":{"WindowsContainer":{"path":"physical-path"}},"command":"Indicator"}"#.as_slice(),
        br#"{"operation":"hyperpolling_receiver","target":{"WindowsContainer":{"path":"p","container":"c"}},"command":{"SetIndicator":{"status":256}}}"#.as_slice(),
        br#"{"operation":"hyperpolling_receiver","target":{"WindowsContainer":{"path":"p","container":"c"}},"command":"MadeUp"}"#.as_slice(),
        br#"{"operation":"hyperpolling_receiver","target":{"WindowsContainer":{"path":"p","container":"c"}},"command":"Serial"}"#.as_slice(),
        br#"{"operation":"hyperpolling_receiver","target":{"WindowsContainer":{"path":"p","container":"c"}},"command":"Serial","transaction":256}"#.as_slice(),
    ] {assert!(serde_json::from_slice::<ServiceRequest>(invalid).is_err());}
}

#[test]
fn hyperpolling_receiver_read_and_mutation_have_distinct_task_ownership() {
    let read = ServiceRequest::HyperpollingReceiver {
        target: target(),
        command: Command::Indicator,
        transaction: 225,
    };
    assert!(read.synchronous_device_task().unwrap().is_none());
    let write = ServiceRequest::HyperpollingReceiver {
        target: target(),
        command: Command::SetIndicator { status: 3 },
        transaction: 226,
    };
    assert_eq!(
        write.synchronous_device_task().unwrap(),
        Some(device_tasks::Scope::container(179, "physical-container").unwrap())
    );
    let cancel = ServiceRequest::HyperpollingReceiver {
        target: target(),
        command: Command::CancelScan,
        transaction: 227,
    };
    assert_eq!(
        cancel.synchronous_device_task().unwrap(),
        Some(device_tasks::Scope::container(179, "physical-container").unwrap())
    );
    let expected=br#"{"operation":"hyperpolling_receiver","target":{"WindowsContainer":{"path":"physical-path","container":"physical-container"}},"command":"Serial","transaction":224}"#;
    assert!(matches!(
        serde_json::from_slice::<ServiceRequest>(expected).unwrap(),
        ServiceRequest::HyperpollingReceiver {
            command: Command::Serial,
            transaction: 224,
            ..
        }
    ));
}

#[test]
fn hyperpolling_receiver_operation_and_connection_guards_both_close_admission() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let active = Arc::new(AtomicBool::new(true));
    let canceled = Arc::new(AtomicBool::new(false));
    let guard = RequestGuard::Scoped {
        active: active.clone(),
        canceled: canceled.clone(),
    };
    assert!(!guard.canceled());
    canceled.store(true, Ordering::Release);
    assert!(guard.canceled());
    canceled.store(false, Ordering::Release);
    active.store(false, Ordering::Release);
    assert!(guard.canceled());
}

#[test]
fn hyperpolling_receiver_response_cannot_cross_physical_target_or_command() {
    let bytes=br#"{"target":{"WindowsContainer":{"path":"physical-path","container":"physical-container"}},"reading":{"Indicator":{"status":null}}}"#;
    let observation: Observation = serde_json::from_slice(bytes).unwrap();
    observation
        .validate_for(&target(), Command::Indicator)
        .unwrap();
    assert!(matches!(
        observation.reading,
        Reading::Indicator { status: None }
    ));
    assert!(
        observation
            .validate_for(
                &Target::WindowsContainer {
                    path: "new-path".into(),
                    container: "physical-container".into()
                },
                Command::Indicator
            )
            .is_err()
    );
    assert!(
        observation
            .validate_for(&target(), Command::SetIndicator { status: 3 })
            .is_err()
    );
}
