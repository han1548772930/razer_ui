//! Deterministic middleware sequencing without a process, GUI or device.
use super::*;
use std::collections::VecDeque;

struct Script {
    commands: VecDeque<(DeviceCommand, Result<Reading, &'static str>)>,
    calls: Vec<&'static str>,
    retries: usize,
    cancel_after_serial: Option<Arc<AtomicBool>>,
}
impl Script {
    fn new(
        commands: impl IntoIterator<Item = (DeviceCommand, Result<Reading, &'static str>)>,
    ) -> Self {
        Self {
            commands: commands.into_iter().collect(),
            calls: vec![],
            retries: 0,
            cancel_after_serial: None,
        }
    }
}
impl Transport for Script {
    fn command(&mut self, command: DeviceCommand) -> anyhow::Result<Reading> {
        self.calls.push(match command {
            DeviceCommand::Serial => "serial",
            DeviceCommand::Edition => "edition",
            DeviceCommand::Indicator => "indicator",
            DeviceCommand::SetIndicator { .. } => "set",
        });
        let (expected, result) = self
            .commands
            .pop_front()
            .expect("unexpected device command");
        assert_eq!(command, expected);
        if command == DeviceCommand::Serial
            && let Some(signal) = &self.cancel_after_serial
        {
            signal.store(true, Ordering::Release);
        }
        result.map_err(anyhow::Error::msg)
    }
    fn peers(&mut self) -> anyhow::Result<ReceiverQueryProjection> {
        self.calls.push("peers");
        razer_discovery::discovery::project_receiver_query(
            179,
            "{11111111-2222-3333-4444-555555555555}",
            &serde_json::json!({
                "query":"receiver_wireless_status_v2","vendor_id":5426,"product_id":179,"claim_interface":0,"report_bytes":91,
                "path":"observed-path","device_instance_id":"observed-instance","device_container_id":"{11111111-2222-3333-4444-555555555555}","device_count":0,"devices":[]
            }),
        )
    }
    fn edition_retry_delay(&mut self) {
        self.retries += 1;
    }
}
fn serial(bytes: &[u8]) -> Reading {
    Reading::Serial(uma::SerialNumber::from_bytes(bytes.to_vec()))
}
fn edition() -> Reading {
    Reading::Edition(uma::Edition::from_source_bytes([7, 254, 9]))
}

#[::core::prelude::v1::test]
fn hyperpolling_runtime_initialization_reads_identity_before_peers_and_truthy_indicator() {
    let mut script = Script::new([
        (DeviceCommand::Serial, Ok(serial(b"A\0B"))),
        (DeviceCommand::Edition, Ok(edition())),
        (
            DeviceCommand::Indicator,
            Ok(Reading::Indicator { status: Some(254) }),
        ),
    ]);
    let (updates, receiver) = mpsc::channel();
    let mut state = Initialization::default();
    initialize(&mut script, &AtomicBool::new(false), &updates, &mut state).unwrap();
    assert_eq!(script.calls, ["serial", "edition", "peers", "indicator"]);
    assert!(state.complete && state.serial_observed);
    let messages = receiver.try_iter().collect::<Vec<_>>();
    assert!(matches!(&messages[0],Update::Serial(serial) if serial=="AB"));
    assert!(matches!(&messages[1],Update::Edition(metadata) if metadata.edition==254));
    assert!(matches!(&messages[2], Update::Peers(_)));
    assert!(matches!(&messages[3], Update::InitialStatus(254)));
    assert!(script.commands.is_empty());
}

#[::core::prelude::v1::test]
fn hyperpolling_runtime_successful_empty_serial_is_distinct_from_failed_read() {
    for (serial_result, expected_complete) in
        [(Ok(serial(&[])), true), (Err("serial failed"), false)]
    {
        let mut commands = vec![
            (DeviceCommand::Serial, serial_result),
            (DeviceCommand::Edition, Ok(edition())),
        ];
        if expected_complete {
            commands.push((
                DeviceCommand::Indicator,
                Ok(Reading::Indicator { status: Some(0) }),
            ));
        }
        let mut script = Script::new(commands);
        let (updates, receiver) = mpsc::channel();
        let mut state = Initialization::default();
        initialize(&mut script, &AtomicBool::new(false), &updates, &mut state).unwrap();
        assert_eq!(state.complete, expected_complete);
        let messages = receiver.try_iter().collect::<Vec<_>>();
        assert_eq!(
            messages.iter().any(
                |message| matches!(message,Update::Serial(serial) if serial=="NOSERIALNUMBER")
            ),
            expected_complete
        );
        assert!(
            !messages
                .iter()
                .any(|message| matches!(message, Update::InitialStatus(_) | Update::Submitted(..)))
        );
        if !expected_complete {
            assert_eq!(script.calls, ["serial", "edition", "peers"]);
        }
    }
}

#[::core::prelude::v1::test]
fn hyperpolling_runtime_edition_retries_are_source_bounded_without_default_observation() {
    let mut script = Script::new([
        (DeviceCommand::Serial, Ok(serial(b"S"))),
        (DeviceCommand::Edition, Err("edition1")),
        (DeviceCommand::Edition, Err("edition2")),
        (DeviceCommand::Edition, Err("edition3")),
        (
            DeviceCommand::Indicator,
            Ok(Reading::Indicator { status: None }),
        ),
    ]);
    let (updates, receiver) = mpsc::channel();
    let mut state = Initialization::default();
    initialize(&mut script, &AtomicBool::new(false), &updates, &mut state).unwrap();
    assert_eq!(script.retries, 2);
    assert!(state.complete);
    let messages = receiver.try_iter().collect::<Vec<_>>();
    assert!(
        !messages
            .iter()
            .any(|message| matches!(message, Update::Edition(_) | Update::InitialStatus(_)))
    );
    assert!(
        messages.iter().any(
            |message| matches!(message,Update::Diagnostic(error) if error.contains("edition3"))
        )
    );
}

#[::core::prelude::v1::test]
fn hyperpolling_runtime_cancel_after_serial_stops_edition_and_later_io() {
    let canceled = Arc::new(AtomicBool::new(false));
    let mut script = Script::new([(DeviceCommand::Serial, Ok(serial(b"S")))]);
    script.cancel_after_serial = Some(canceled.clone());
    let (updates, _receiver) = mpsc::channel();
    let mut state = Initialization::default();
    assert!(initialize(&mut script, &canceled, &updates, &mut state).is_err());
    assert_eq!(script.calls, ["serial"]);
    assert!(!state.complete);
}

#[::core::prelude::v1::test]
fn hyperpolling_runtime_failed_serial_recovers_on_next_explicit_initialization() {
    let mut script = Script::new([
        (DeviceCommand::Serial, Err("not ready")),
        (DeviceCommand::Edition, Ok(edition())),
        (DeviceCommand::Serial, Ok(serial(b"READY"))),
        (DeviceCommand::Edition, Ok(edition())),
        (
            DeviceCommand::Indicator,
            Ok(Reading::Indicator { status: Some(2) }),
        ),
    ]);
    let (updates, receiver) = mpsc::channel();
    let mut state = Initialization::default();
    initialize(&mut script, &AtomicBool::new(false), &updates, &mut state).unwrap();
    assert!(!state.complete);
    initialize(&mut script, &AtomicBool::new(false), &updates, &mut state).unwrap();
    assert!(state.complete);
    assert_eq!(
        script.calls,
        [
            "serial",
            "edition",
            "peers",
            "serial",
            "edition",
            "peers",
            "indicator"
        ]
    );
    assert!(
        receiver
            .try_iter()
            .any(|message| matches!(message, Update::InitialStatus(2)))
    );
}

#[::core::prelude::v1::test]
fn hyperpolling_runtime_replacement_waits_for_predecessor_release() {
    let (release, gate) = mpsc::channel();
    let (started, begin) = mpsc::channel();
    let (finished, done) = mpsc::channel();
    let predecessor = thread::spawn(move || gate.recv().unwrap());
    let successor = thread::spawn(move || {
        started.send(()).unwrap();
        join_predecessors(vec![predecessor]).unwrap();
        finished.send(()).unwrap();
    });
    begin.recv().unwrap();
    assert!(matches!(done.try_recv(), Err(mpsc::TryRecvError::Empty)));
    release.send(()).unwrap();
    done.recv().unwrap();
    successor.join().unwrap();
}
