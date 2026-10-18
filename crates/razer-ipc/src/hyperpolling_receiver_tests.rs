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
    let expected=br#"{"operation":"hyperpolling_receiver","target":{"WindowsContainer":{"path":"physical-path","container":"physical-container"}},"command":{"SetIndicator":{"status":254}}}"#;
    let request = ServiceRequest::HyperpollingReceiver {
        target: target(),
        command: Command::SetIndicator { status: 254 },
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
    ] {assert!(serde_json::from_slice::<ServiceRequest>(invalid).is_err());}
}

#[test]
fn hyperpolling_receiver_read_and_mutation_have_distinct_task_ownership() {
    let read = ServiceRequest::HyperpollingReceiver {
        target: target(),
        command: Command::Indicator,
    };
    assert!(read.synchronous_device_task().unwrap().is_none());
    let write = ServiceRequest::HyperpollingReceiver {
        target: target(),
        command: Command::SetIndicator { status: 3 },
    };
    assert_eq!(
        write.synchronous_device_task().unwrap(),
        Some(device_tasks::Scope::container(179, "physical-container").unwrap())
    );
    let expected=br#"{"operation":"hyperpolling_receiver","target":{"WindowsContainer":{"path":"physical-path","container":"physical-container"}},"command":"Serial"}"#;
    assert!(matches!(
        serde_json::from_slice::<ServiceRequest>(expected).unwrap(),
        ServiceRequest::HyperpollingReceiver {
            command: Command::Serial,
            ..
        }
    ));
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
