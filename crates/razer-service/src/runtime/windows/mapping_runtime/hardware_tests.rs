use super::*;
use std::{collections::VecDeque, sync::Mutex};

#[derive(Default)]
struct State {
    paths: Vec<Collection>,
    reads: BTreeMap<String, VecDeque<std::result::Result<Read, i32>>>,
    open_errors: BTreeMap<String, VecDeque<i32>>,
    close_errors: BTreeMap<String, i32>,
    log: Vec<String>,
    tick: u64,
    close_gate: Option<(mpsc::SyncSender<()>, mpsc::Receiver<()>)>,
    read_observed: Option<mpsc::SyncSender<()>>,
}

struct SimTransport(Arc<Mutex<State>>);
struct SimReader {
    state: Arc<Mutex<State>>,
    path: String,
    closed: bool,
}

impl Transport for SimTransport {
    type Reader = SimReader;
    fn collections(&mut self) -> Result<Vec<Collection>> {
        Ok(self.0.lock().unwrap().paths.clone())
    }
    fn open(&mut self, collection: &Collection) -> Result<SimReader> {
        let mut state = self.0.lock().unwrap();
        state.log.push(format!("open:{}", collection.path));
        if let Some(code) = state
            .open_errors
            .get_mut(&collection.path)
            .and_then(VecDeque::pop_front)
        {
            return Err(io::Error::from_raw_os_error(code).into());
        }
        Ok(SimReader {
            state: self.0.clone(),
            path: collection.path.clone(),
            closed: false,
        })
    }
    fn capture(&mut self, bytes: Vec<u8>) -> Result<driver::Packet> {
        let mut state = self.0.lock().unwrap();
        state.tick += 1;
        Ok(driver::Packet {
            bytes,
            time_tick: state.tick,
            modifiers: razer_device::mapping_native_events::NativeModifierSnapshot {
                left_alt: true,
                left_control: false,
                left_shift: false,
                left_windows: false,
                right_alt: false,
                right_control: true,
                right_shift: false,
                right_windows: false,
            },
        })
    }
}

impl Reader for SimReader {
    fn poll(&mut self) -> io::Result<Option<Read>> {
        assert!(!self.closed);
        let mut state = self.state.lock().unwrap();
        let result = state
            .reads
            .get_mut(&self.path)
            .and_then(VecDeque::pop_front)
            .map(|result| result.map_err(io::Error::from_raw_os_error))
            .transpose();
        if !matches!(result, Ok(None))
            && let Some(observed) = state.read_observed.take()
        {
            observed.send(()).unwrap();
        }
        result
    }
    fn close(&mut self) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        let gate = {
            let mut state = self.state.lock().unwrap();
            state.log.push(format!("cancel:{}", self.path));
            state.close_gate.take()
        };
        if let Some((started, complete)) = gate {
            started.send(()).unwrap();
            complete.recv().unwrap();
        }
        let mut state = self.state.lock().unwrap();
        state.log.push(format!("drain:{}", self.path));
        state.log.push(format!("close:{}", self.path));
        self.closed = true;
        if let Some(code) = state.close_errors.get(&self.path) {
            return Err(io::Error::from_raw_os_error(*code)).context("Simulated input close");
        }
        Ok(())
    }
}

impl Drop for SimReader {
    fn drop(&mut self) {
        assert!(
            self.closed,
            "input handle dropped before its close/drain contract"
        );
    }
}

fn collection(path: &str) -> Collection {
    Collection {
        path: path.into(),
        instance: format!("instance-{path}"),
    }
}

fn setup(paths: &[&str]) -> (Arc<Mutex<State>>, Session<SimTransport>) {
    let state = Arc::new(Mutex::new(State {
        paths: paths.iter().map(|path| collection(path)).collect(),
        ..Default::default()
    }));
    let session = Session::open(SimTransport(state.clone())).unwrap();
    (state, session)
}

fn bytes(state: &Arc<Mutex<State>>, path: &str, bytes: &[u8]) {
    state
        .lock()
        .unwrap()
        .reads
        .entry(path.into())
        .or_default()
        .push_back(Ok(Read {
            bytes: bytes.to_vec(),
            rearm_error: None,
        }));
}

fn failure(state: &Arc<Mutex<State>>, path: &str, code: i32) {
    state
        .lock()
        .unwrap()
        .reads
        .entry(path.into())
        .or_default()
        .push_back(Err(code));
}

#[test]
fn retains_all_collections_in_discovery_order_and_exact_transferred_bytes() {
    let (state, mut session) = setup(&["z", "a"]);
    bytes(&state, "z", &[7, 0, 0xff]);
    bytes(&state, "a", &[0, 91]);
    let packets = session
        .poll(Instant::now())
        .into_iter()
        .collect::<std::result::Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(packets.len(), 2);
    assert_eq!(packets[0].endpoint.path, "z");
    assert_eq!(packets[0].input.bytes, [7, 0, 0xff]);
    assert_eq!(packets[1].input.bytes, [0, 91]);
    assert_eq!(packets[0].input.time_tick, 1);
    assert_eq!(packets[1].input.time_tick, 2);
    assert!(packets[0].input.modifiers.left_alt);
    assert!(packets[0].input.modifiers.right_control);
    assert_eq!(packets[0].endpoint.instance, "instance-z");
    session.refresh().unwrap();
    state.lock().unwrap().paths.push(collection("b"));
    session.refresh().unwrap();
    assert_eq!(state.lock().unwrap().log, ["open:z", "open:a", "open:b"]);
    session.shutdown().unwrap();
    assert!(session.generations().is_empty());
}

#[test]
fn per_collection_initial_failures_are_real_errors_without_device_wide_failure() {
    let state = Arc::new(Mutex::new(State {
        paths: vec![collection("a"), collection("b")],
        open_errors: BTreeMap::from([("a".into(), VecDeque::from([5]))]),
        ..Default::default()
    }));
    let mut session = Session::open(SimTransport(state.clone())).unwrap();
    bytes(&state, "b", &[4, 23]);
    let events = session.poll(Instant::now());
    assert_eq!(events.len(), 2);
    assert!(events[0].is_err());
    assert_eq!(events[1].as_ref().unwrap().input.bytes, [4, 23]);
    assert!(!session.generations().contains_key("a"));
    assert!(session.endpoints[0].retry.is_none());
}

#[test]
fn initial_open_failure_does_not_invent_read_error_retry_or_ready_state() {
    let state = Arc::new(Mutex::new(State {
        paths: vec![collection("a")],
        open_errors: BTreeMap::from([("a".into(), VecDeque::from([1167]))]),
        ..Default::default()
    }));
    let mut session = Session::open(SimTransport(state.clone())).unwrap();
    assert!(session.generations().is_empty());
    assert!(session.poll(Instant::now())[0].is_err());
    assert!(
        session
            .poll(Instant::now() + Duration::from_secs(100))
            .is_empty()
    );
    assert_eq!(state.lock().unwrap().log, ["open:a"]);
}

#[test]
fn only_device_not_connected_retries_after_one_second_and_advances_generation() {
    let (state, mut session) = setup(&["a", "b"]);
    let now = Instant::now();
    failure(&state, "a", 1167);
    failure(&state, "b", 5);
    let events = session.poll(now);
    assert_eq!(events.len(), 1);
    assert!(
        events[0]
            .as_ref()
            .unwrap_err()
            .contains("Mapping hardware read")
    );
    assert!(
        session
            .endpoints
            .iter()
            .all(|endpoint| endpoint.reader.is_none())
    );
    assert!(session.poll(now + Duration::from_millis(999)).is_empty());
    assert_eq!(
        state
            .lock()
            .unwrap()
            .log
            .iter()
            .filter(|item| item.starts_with("open:"))
            .count(),
        2
    );
    bytes(&state, "a", &[8, 17]);
    let events = session.poll(now + Duration::from_secs(1));
    let packet = events[0].as_ref().unwrap();
    assert_eq!(packet.endpoint.generation, 2);
    assert_eq!(packet.input.bytes, [8, 17]);
    assert!(session.endpoints[1].reader.is_none());
    assert!(session.endpoints[1].retry.is_none());
}

#[test]
fn reconnect_is_bounded_to_native_fifty_nine_attempts() {
    let (state, mut session) = setup(&["a"]);
    let now = Instant::now();
    failure(&state, "a", 1167);
    session.poll(now);
    state
        .lock()
        .unwrap()
        .open_errors
        .insert("a".into(), VecDeque::from(vec![5; 59]));
    for second in 1..60 {
        assert!(session.poll(now + Duration::from_secs(second)).is_empty());
    }
    let terminal = session.poll(now + Duration::from_secs(60));
    assert!(terminal[0].as_ref().unwrap_err().contains("59 attempts"));
    assert!(terminal[0].as_ref().unwrap_err().contains("Open:"));
    assert!(session.poll(now + Duration::from_secs(61)).is_empty());
    assert_eq!(
        state
            .lock()
            .unwrap()
            .log
            .iter()
            .filter(|item| item.as_str() == "open:a")
            .count(),
        60
    );
    assert!(session.endpoints[0].retry.is_none());
}

#[test]
fn fresh_read_or_rearm_failure_resets_retry_budget_before_next_bounded_open_failures() {
    for rearm_failure in [false, true] {
        let (state, mut session) = setup(&["a"]);
        let now = Instant::now();
        failure(&state, "a", 1167);
        assert!(session.poll(now).is_empty());
        state
            .lock()
            .unwrap()
            .open_errors
            .insert("a".into(), VecDeque::from(vec![5; 58]));
        for second in 1..59 {
            assert!(session.poll(now + Duration::from_secs(second)).is_empty());
        }
        assert_eq!(session.endpoints[0].retry.as_ref().unwrap().attempts, 58);
        if rearm_failure {
            state
                .lock()
                .unwrap()
                .reads
                .entry("a".into())
                .or_default()
                .push_back(Ok(Read {
                    bytes: vec![9, 7],
                    rearm_error: Some(io::Error::from_raw_os_error(1167)),
                }));
        } else {
            failure(&state, "a", 1167);
        }
        let observations = session.poll(now + Duration::from_secs(59));
        assert_eq!(observations.len(), usize::from(rearm_failure));
        if rearm_failure {
            assert_eq!(observations[0].as_ref().unwrap().input.bytes, [9, 7]);
        }
        let endpoint = &session.endpoints[0];
        assert_eq!(endpoint.generation, 2);
        assert!(endpoint.reader.is_none());
        assert_eq!(endpoint.retry.as_ref().unwrap().attempts, 0);
        assert_eq!(
            endpoint.retry.as_ref().unwrap().due,
            now + Duration::from_secs(60)
        );
        state
            .lock()
            .unwrap()
            .open_errors
            .insert("a".into(), VecDeque::from(vec![5; 59]));
        for second in 60..119 {
            assert!(session.poll(now + Duration::from_secs(second)).is_empty());
        }
        let terminal = session.poll(now + Duration::from_secs(119));
        assert_eq!(terminal.len(), 1);
        assert!(terminal[0].as_ref().unwrap_err().contains("59 attempts"));
        assert!(session.endpoints[0].retry.is_none());
        assert!(session.poll(now + Duration::from_secs(120)).is_empty());
        assert_eq!(
            state
                .lock()
                .unwrap()
                .log
                .iter()
                .filter(|entry| entry.as_str() == "open:a")
                .count(),
            119
        );
    }
}

#[test]
fn reconnect_rejects_same_path_with_replacement_instance() {
    let (state, mut session) = setup(&["a"]);
    let now = Instant::now();
    failure(&state, "a", 1167);
    session.poll(now);
    state.lock().unwrap().paths[0].instance = "replacement".into();
    let events = session.poll(now + Duration::from_secs(1));
    assert!(events[0].as_ref().unwrap_err().contains("replacement"));
    assert!(session.generations().is_empty());
    assert!(session.endpoints[0].retired);
    session.refresh().unwrap();
    assert_eq!(
        state
            .lock()
            .unwrap()
            .log
            .iter()
            .filter(|item| item.as_str() == "open:a")
            .count(),
        1
    );
}

#[test]
fn refresh_retires_changed_instance_and_removes_its_generation_from_fence() {
    let (state, mut session) = setup(&["a"]);
    bytes(&state, "a", &[1]);
    let packet = session.poll(Instant::now()).pop().unwrap().unwrap();
    assert_eq!(
        session.generations()[&packet.endpoint.path].1,
        packet.endpoint.generation
    );
    state.lock().unwrap().paths[0].instance = "replacement".into();
    session.refresh().unwrap();
    assert!(session.generations().is_empty());
    assert_eq!(
        state.lock().unwrap().log,
        ["open:a", "cancel:a", "drain:a", "close:a"]
    );
}

#[test]
fn empty_read_does_not_emit_a_packet_and_oversized_completion_is_rejected() {
    let (state, mut session) = setup(&["a"]);
    bytes(&state, "a", &[]);
    assert!(session.poll(Instant::now()).is_empty());
    bytes(&state, "a", &[7; 101]);
    assert!(session.poll(Instant::now())[0].is_err());
    assert!(session.endpoints[0].reader.is_none());
    assert!(session.endpoints[0].retry.is_none());
}

#[test]
fn completed_bytes_survive_transient_rearm_failure_without_premature_error_notification() {
    let (state, mut session) = setup(&["a"]);
    state
        .lock()
        .unwrap()
        .reads
        .entry("a".into())
        .or_default()
        .push_back(Ok(Read {
            bytes: vec![9, 3],
            rearm_error: Some(io::Error::from_raw_os_error(1167)),
        }));
    let observations = session.poll(Instant::now());
    assert_eq!(observations[0].as_ref().unwrap().input.bytes, [9, 3]);
    assert_eq!(observations.len(), 1);
    assert_eq!(session.generations()["a"].1, 1);
    assert!(session.endpoints[0].reader.is_none());
    assert!(session.endpoints[0].retry.is_some());
}

#[test]
fn worker_fence_preserves_completed_read_when_only_rearm_failed() {
    for code in [5, 1167] {
        let (observed, received) = mpsc::sync_channel(1);
        let state = Arc::new(Mutex::new(State {
            paths: vec![collection("a")],
            reads: BTreeMap::from([(
                "a".into(),
                VecDeque::from([Ok(Read {
                    bytes: vec![9, 3],
                    rearm_error: Some(io::Error::from_raw_os_error(code)),
                })]),
            )]),
            read_observed: Some(observed),
            ..Default::default()
        }));
        let factory = state.clone();
        let mut owner = Owner::open_with(move || Ok(SimTransport(factory))).unwrap();
        received.recv_timeout(Duration::from_secs(5)).unwrap();
        // Refresh runs after the observed read has completed and been queued.
        owner.refresh_paths().unwrap();
        let events = owner.inputs().unwrap();
        assert_eq!(events.len(), if code == 1167 { 1 } else { 2 });
        if code != 1167 {
            assert!(
                events[0].is_err(),
                "Rearm error precedes completed bytes (123600)"
            );
        }
        let packet = events.last().unwrap().as_ref().unwrap();
        assert_eq!(packet.input.bytes, [9, 3]);
        assert_eq!(packet.endpoint.generation, 1);
        owner.shutdown().unwrap();
    }
}

#[test]
fn worker_fence_discards_completed_bytes_after_physical_instance_replacement() {
    let (observed, received) = mpsc::sync_channel(1);
    let state = Arc::new(Mutex::new(State {
        paths: vec![collection("a")],
        read_observed: Some(observed),
        ..Default::default()
    }));
    bytes(&state, "a", &[9, 3]);
    let factory = state.clone();
    let mut owner = Owner::open_with(move || Ok(SimTransport(factory))).unwrap();
    received.recv_timeout(Duration::from_secs(5)).unwrap();
    state.lock().unwrap().paths[0].instance = "replacement".into();
    owner.refresh_paths().unwrap();
    owner.refresh_paths().unwrap();
    let events = owner.inputs().unwrap();
    assert!(events.iter().all(|event| event.is_err()));
    assert!(
        events
            .iter()
            .any(|event| event.as_ref().unwrap_err().contains("physical instance"))
    );
    owner.shutdown().unwrap();
}

#[test]
fn endpoint_cleanup_failures_survive_read_error_and_repeated_shutdown() {
    let (state, mut session) = setup(&["a", "b"]);
    state.lock().unwrap().close_errors.insert("a".into(), 5);
    failure(&state, "a", 1167);
    let events = session.poll(Instant::now());
    assert_eq!(events.len(), 1);
    assert!(
        events[0]
            .as_ref()
            .unwrap_err()
            .contains("Simulated input close")
    );
    let error = session.shutdown().unwrap_err().to_string();
    assert!(error.contains("Simulated input close"));
    assert_eq!(session.shutdown().unwrap_err().to_string(), error);
    assert!(session.refresh().is_err());
    assert_eq!(
        state.lock().unwrap().log,
        [
            "open:a", "open:b", "cancel:a", "drain:a", "close:a", "cancel:b", "drain:b", "close:b",
        ]
    );
}

#[test]
fn queue_overflow_joins_all_readers_and_returns_real_cleanup_failure() {
    let (closing, closing_received) = mpsc::sync_channel(1);
    let (complete, complete_received) = mpsc::sync_channel(1);
    let state = Arc::new(Mutex::new(State {
        paths: vec![collection("a"), collection("b")],
        close_errors: BTreeMap::from([("a".into(), 5)]),
        close_gate: Some((closing, complete_received)),
        ..Default::default()
    }));
    for _ in 0..=INPUT_CAPACITY {
        bytes(&state, "a", &[7, 3]);
    }
    let factory = state.clone();
    let mut owner = Owner::open_with(move || Ok(SimTransport(factory))).unwrap();
    closing_received
        .recv_timeout(Duration::from_secs(5))
        .unwrap();
    assert!(
        owner
            .inputs()
            .unwrap_err()
            .to_string()
            .contains("overflowed")
    );
    complete.send(()).unwrap();
    let error = owner.shutdown().unwrap_err().to_string();
    assert!(error.contains("overflowed"));
    assert!(error.contains("Simulated input close"));
    assert_eq!(owner.shutdown().unwrap_err().to_string(), error);
    assert!(owner.input.try_recv().is_err());
    assert_eq!(
        state.lock().unwrap().log,
        [
            "open:a", "open:b", "cancel:a", "drain:a", "close:a", "cancel:b", "drain:b", "close:b",
        ]
    );
}

#[test]
fn shutdown_cancels_reconnect_and_drains_every_retained_collection_once() {
    let (state, mut session) = setup(&["a", "b"]);
    failure(&state, "a", 1167);
    session.poll(Instant::now());
    session.shutdown().unwrap();
    session.shutdown().unwrap();
    assert!(
        session
            .endpoints
            .iter()
            .all(|endpoint| endpoint.reader.is_none() && endpoint.retry.is_none())
    );
    assert_eq!(
        state.lock().unwrap().log,
        [
            "open:a", "open:b", "cancel:a", "drain:a", "close:a", "cancel:b", "drain:b", "close:b",
        ]
    );
}

#[test]
fn worker_shutdown_waits_for_input_cleanup_before_joining_and_discards_queued_frames() {
    let (started, started_receive) = mpsc::sync_channel(1);
    let (complete, complete_receive) = mpsc::sync_channel(1);
    let state = Arc::new(Mutex::new(State {
        paths: vec![collection("a")],
        close_gate: Some((started, complete_receive)),
        ..Default::default()
    }));
    let factory = state.clone();
    let mut owner = Owner::open_with(move || Ok(SimTransport(factory))).unwrap();
    let (done, done_receive) = mpsc::sync_channel(1);
    let caller = thread::spawn(move || {
        let result = owner.shutdown();
        assert!(owner.input.try_recv().is_err());
        done.send(result).unwrap();
    });
    started_receive
        .recv_timeout(Duration::from_secs(5))
        .unwrap();
    assert!(matches!(
        done_receive.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    complete.send(()).unwrap();
    done_receive
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    caller.join().unwrap();
    assert_eq!(
        state.lock().unwrap().log,
        ["open:a", "cancel:a", "drain:a", "close:a"]
    );
}

#[test]
fn duplicate_collection_identity_fails_before_opening_any_handle() {
    let state = Arc::new(Mutex::new(State {
        paths: vec![collection("a"), collection("A")],
        ..Default::default()
    }));
    assert!(Session::open(SimTransport(state.clone())).is_err());
    assert!(state.lock().unwrap().log.is_empty());
}

#[test]
fn source_probe_uses_bounded_delays_and_three_quiet_observations_after_discovery() {
    let now = Instant::now();
    let mut probe = Probe::default();
    probe.observed(now, true, true);
    assert_eq!(probe.due, Some(now + Duration::from_millis(4)));
    probe.observed(now, true, false);
    assert_eq!(probe.due, Some(now + Duration::from_millis(8)));
    probe.observed(now, true, true);
    assert_eq!(probe.quiet, 0);
    assert_eq!(probe.due, Some(now + Duration::from_millis(10)));
    probe.observed(now, true, false);
    probe.observed(now, true, false);
    assert!(probe.due.is_some());
    probe.observed(now, true, false);
    assert!(probe.due.is_none());
}

#[test]
fn source_probe_stops_after_ten_empty_observations() {
    let now = Instant::now();
    let mut probe = Probe::default();
    for _ in 0..9 {
        probe.observed(now, false, false);
        assert!(probe.due.is_some());
    }
    probe.observed(now, false, false);
    assert!(probe.due.is_none());
}
