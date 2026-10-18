use super::*;

struct Fake {
    events: Arc<Mutex<Vec<String>>>,
    gate: Option<mpsc::Receiver<()>>,
    started: Option<mpsc::Sender<()>>,
    fail_command: bool,
    fail_shutdown: bool,
    panic_command: bool,
    hid_failure: Option<String>,
    stop_command: bool,
    stop_observe: bool,
    system_samples: Vec<razer_device::system_monitor::SystemSample>,
}

impl Drop for Fake {
    fn drop(&mut self) {
        self.events.lock().unwrap().push("drop".into());
    }
}

impl Transport for Fake {
    fn state(&self) -> TransportState {
        match &self.hid_failure {
            Some(diagnostic) => TransportState::HidStopped {
                diagnostic: diagnostic.clone(),
            },
            None => TransportState::Active,
        }
    }

    fn submit(&mut self, command: CoolerCommand) -> anyhow::Result<()> {
        self.events
            .lock()
            .unwrap()
            .push(format!("command:{:?}", command.header()));
        if let Some(started) = self.started.take() {
            started.send(()).unwrap();
        }
        if let Some(gate) = self.gate.take() {
            gate.recv().unwrap();
        }
        assert!(!self.panic_command, "simulated transport panic");
        if self.stop_command {
            self.hid_failure = Some("simulated fatal setter failure".into());
            anyhow::bail!("simulated fatal setter failure");
        }
        anyhow::ensure!(!self.fail_command, "simulated setter failure");
        Ok(())
    }
    fn observe(&mut self) -> anyhow::Result<Telemetry> {
        self.events.lock().unwrap().push("observe".into());
        if let Some(started) = self.started.take() {
            started.send(()).unwrap();
        }
        if let Some(gate) = self.gate.take() {
            gate.recv().unwrap();
        }
        if self.stop_observe {
            self.hid_failure = Some("simulated fatal telemetry failure".into());
            anyhow::bail!("simulated fatal telemetry failure");
        }
        anyhow::bail!("simulated telemetry failure")
    }
    fn shutdown(&mut self) -> anyhow::Result<()> {
        self.events.lock().unwrap().push("shutdown".into());
        anyhow::ensure!(!self.fail_shutdown, "simulated cleanup failure");
        Ok(())
    }
    fn observe_system(
        &mut self,
    ) -> anyhow::Result<Vec<razer_device::system_monitor::SystemSample>> {
        self.events.lock().unwrap().push("system".into());
        Ok(std::mem::take(&mut self.system_samples))
    }
}

fn fake(events: &Arc<Mutex<Vec<String>>>) -> Fake {
    Fake {
        events: events.clone(),
        gate: None,
        started: None,
        fail_command: false,
        fail_shutdown: false,
        panic_command: false,
        hid_failure: None,
        stop_command: false,
        stop_observe: false,
        system_samples: Vec::new(),
    }
}

fn fan() -> CoolerCommand {
    CoolerCommand::FanParameter { mode: 2, speed: 50 }
}
fn pump() -> CoolerCommand {
    CoolerCommand::PumpParameter { mode: 1, speed: 20 }
}

fn finish(
    replies: &async_channel::Receiver<Reply>,
) -> (Vec<(u64, Result<(), String>)>, Result<(), String>) {
    let mut completed = Vec::new();
    loop {
        match replies.recv_blocking().expect("retained worker reply") {
            Reply::Finished { generation, result } => completed.push((generation, result)),
            Reply::Restored(_) => panic!("command-only fixture must not restore profiles"),
            Reply::Observed(_) => panic!("command-only fixture must not request telemetry"),
            Reply::SystemObserved(_) | Reply::SystemWriteFinished(_) => {
                panic!("command-only fixture must not request system monitoring")
            }
            Reply::Stopped(result) => return (completed, result),
        }
    }
}

#[test]
fn profile_restore_shares_fifo_without_completing_an_interactive_generation() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let transport = fake(&events);
    let (owner, replies) = Session::start_with(move |_| Ok(transport)).unwrap();
    owner.restore(vec![pump(), fan()]).unwrap();
    owner.submit(7, fan()).unwrap();
    owner.finish();
    assert!(matches!(
        replies.recv_blocking().unwrap(),
        Reply::Restored(Ok(()))
    ));
    assert!(matches!(
        replies.recv_blocking().unwrap(),
        Reply::Finished {
            generation: 7,
            result: Ok(())
        }
    ));
    assert!(matches!(
        replies.recv_blocking().unwrap(),
        Reply::Stopped(Ok(()))
    ));
    owner.wait().unwrap();
    assert_eq!(
        *events.lock().unwrap(),
        [
            "command:[20, 1]",
            "command:[34, 1]",
            "command:[34, 1]",
            "shutdown",
            "drop"
        ]
    );
}

#[test]
fn telemetry_waits_for_product_activation_and_repeated_activation_does_not_reset_timer() {
    let mut schedule = PollSchedule {
        telemetry_interval: Some(TELEMETRY_INTERVAL),
        telemetry: None,
        system: None,
    };
    assert!(schedule.deadline().is_none());
    schedule.activate_telemetry();
    let first = schedule.deadline().unwrap();
    schedule.activate_telemetry();
    assert_eq!(schedule.deadline(), Some(first));
    schedule.stop();
    assert!(schedule.deadline().is_none());
}

#[test]
fn ordered_monitor_samples_publish_nulls_and_only_changed_temperatures_write_back() {
    use razer_device::system_monitor::SystemSample;
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut transport = fake(&events);
    let sample = SystemSample {
        cpu_temperature: Some(42.7),
        cpu_clock_mhz: Some(4200.),
        gpu_temperature: Some(63.5),
        gpu_clock_mhz: Some(1600.),
    };
    let mut clock_only = sample.clone();
    clock_only.cpu_clock_mhz = Some(4300.);
    let mut null_cpu = clock_only.clone();
    null_cpu.cpu_temperature = None;
    transport.system_samples = vec![sample.clone(), clock_only, null_cpu];
    let now = Instant::now();
    let mut schedule = PollSchedule {
        telemetry_interval: None,
        telemetry: None,
        system: Some(now),
    };
    let (output, replies) = async_channel::unbounded();
    let mut last = None;
    let (commands, _input) = mpsc::sync_channel(128);
    let admission = Mutex::new(Some(commands));
    assert!(
        poll_due(
            &mut transport,
            &mut schedule,
            now,
            &output,
            &mut last,
            &AtomicBool::new(true),
            &admission,
        )
        .is_none()
    );
    assert!(
        matches!(replies.try_recv().unwrap(),Reply::SystemObserved(Ok(value)) if value==sample)
    );
    assert!(matches!(
        replies.try_recv().unwrap(),
        Reply::SystemWriteFinished(Ok(()))
    ));
    assert!(
        matches!(replies.try_recv().unwrap(),Reply::SystemObserved(Ok(value)) if value.cpu_clock_mhz==Some(4300.))
    );
    assert!(
        matches!(replies.try_recv().unwrap(),Reply::SystemObserved(Ok(value)) if value.cpu_temperature.is_none())
    );
    assert!(
        matches!(replies.try_recv().unwrap(),Reply::SystemWriteFinished(Err(error)) if error.contains("unobserved"))
    );
    assert!(replies.try_recv().is_err());
    assert_eq!(*events.lock().unwrap(), ["system", "command:[192, 1]"]);
    assert_eq!(last, Some((None, Some(63.5))));
}

#[test]
fn finish_drains_both_channels_before_shutdown_and_release() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let transport = fake(&events);
    let (owner, replies) = Session::start_with(move |_| Ok(transport)).unwrap();
    owner.submit(1, fan()).unwrap();
    owner.submit(2, pump()).unwrap();
    owner.finish();
    assert!(owner.submit(3, fan()).is_err());
    let (completed, stopped) = finish(&replies);
    assert_eq!(completed, vec![(1, Ok(())), (2, Ok(()))]);
    assert_eq!(stopped, Ok(()));
    owner.wait().unwrap();
    assert_eq!(
        *events.lock().unwrap(),
        ["command:[34, 1]", "command:[20, 1]", "shutdown", "drop"]
    );
}

#[test]
fn equal_cleanup_batch_preserves_both_normal_passes_even_when_commands_fail() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut transport = fake(&events);
    transport.fail_command = true;
    let (owner, replies) = Session::start_with(move |_| Ok(transport)).unwrap();
    assert!(owner.submit_batch(1, Vec::new()).is_err());
    owner
        .submit_batch(2, vec![pump(), fan(), pump(), fan()])
        .unwrap();
    owner.finish();
    let (completed, stopped) = finish(&replies);
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].0, 2);
    assert_eq!(
        completed[0]
            .1
            .as_ref()
            .unwrap_err()
            .matches("setter failure")
            .count(),
        4
    );
    assert_eq!(stopped, Ok(()));
    owner.wait().unwrap();
    assert_eq!(
        *events.lock().unwrap(),
        [
            "command:[20, 1]",
            "command:[34, 1]",
            "command:[20, 1]",
            "command:[34, 1]",
            "shutdown",
            "drop"
        ]
    );
}

#[test]
fn command_failure_does_not_discard_other_accepted_channel() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut transport = fake(&events);
    transport.fail_command = true;
    let (owner, replies) = Session::start_with(move |_| Ok(transport)).unwrap();
    owner.submit(10, fan()).unwrap();
    owner.submit(11, pump()).unwrap();
    owner.finish();
    let (completed, stopped) = finish(&replies);
    assert_eq!(completed.len(), 2);
    assert!(
        completed
            .iter()
            .all(|(_, result)| result.as_ref().unwrap_err().contains("setter failure"))
    );
    assert_eq!(stopped, Ok(()));
    owner.wait().unwrap();
    assert_eq!(&events.lock().unwrap()[2..], ["shutdown", "drop"]);
}

#[test]
fn connection_cancel_rejects_queued_work_and_still_releases_transport() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (release, gate) = mpsc::channel();
    let (started, observed) = mpsc::channel();
    let mut transport = fake(&events);
    transport.gate = Some(gate);
    transport.started = Some(started);
    let (owner, replies) = Session::start_with(move |_| Ok(transport)).unwrap();
    owner.submit(1, fan()).unwrap();
    observed
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    owner.submit(2, pump()).unwrap();
    owner.cancel();
    assert!(!owner.is_accepting());
    release.send(()).unwrap();
    let (completed, stopped) = finish(&replies);
    assert_eq!(completed[0], (1, Ok(())));
    assert!(completed[1].1.as_ref().unwrap_err().contains("canceled"));
    assert_eq!(stopped, Ok(()));
    owner.wait().unwrap();
    assert_eq!(
        *events.lock().unwrap(),
        ["command:[34, 1]", "shutdown", "drop"]
    );
}

#[test]
fn cleanup_failure_remains_visible_after_repeated_join() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut transport = fake(&events);
    transport.fail_shutdown = true;
    let (owner, replies) = Session::start_with(move |_| Ok(transport)).unwrap();
    owner.finish();
    let (_, stopped) = finish(&replies);
    assert!(stopped.as_ref().unwrap_err().contains("cleanup failure"));
    assert_eq!(owner.wait(), stopped);
    assert_eq!(owner.wait(), stopped);
    assert_eq!(*events.lock().unwrap(), ["shutdown", "drop"]);
}

#[test]
fn setter_panic_preserves_error_and_runs_independent_cleanup() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut transport = fake(&events);
    transport.panic_command = true;
    let (owner, replies) = Session::start_with(move |_| Ok(transport)).unwrap();
    owner.submit(1, fan()).unwrap();
    owner.finish();
    let (completed, stopped) = finish(&replies);
    assert!(completed[0].1.as_ref().unwrap_err().contains("panicked"));
    assert_eq!(stopped, Ok(()));
    owner.wait().unwrap();
    assert_eq!(&events.lock().unwrap()[1..], ["shutdown", "drop"]);
}

#[test]
fn startup_failure_finishes_already_accepted_request_and_closes_admission() {
    let (release, gate) = mpsc::channel();
    let (owner, replies) = Session::start_with::<Fake>(move |_| {
        gate.recv().unwrap();
        anyhow::bail!("simulated startup failure")
    })
    .unwrap();
    owner.submit(4, fan()).unwrap();
    release.send(()).unwrap();
    let (completed, stopped) = finish(&replies);
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].0, 4);
    assert!(
        completed[0]
            .1
            .as_ref()
            .unwrap_err()
            .contains("startup failure")
    );
    assert!(stopped.is_err());
    assert!(!owner.is_accepting());
    assert!(owner.submit(5, pump()).is_err());
    assert_eq!(owner.wait(), stopped);
}

#[test]
fn failed_observation_preserves_commands_and_finish_stops_future_polling() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (release, gate) = mpsc::channel();
    let (started, observed) = mpsc::channel();
    let mut transport = fake(&events);
    transport.gate = Some(gate);
    transport.started = Some(started);
    let (owner, replies) =
        Session::start_with_interval(move |_| Ok(transport), Some(Duration::ZERO)).unwrap();
    observed.recv_timeout(Duration::from_secs(5)).unwrap();
    owner.submit(8, fan()).unwrap();
    owner.finish();
    release.send(()).unwrap();
    let Reply::Observed(sample) = replies.recv_blocking().unwrap() else {
        panic!("missing sample");
    };
    assert!(sample.unwrap_err().contains("telemetry failure"));
    let (completed, stopped) = finish(&replies);
    assert_eq!(completed, [(8, Ok(()))]);
    assert_eq!(stopped, Ok(()));
    owner.wait().unwrap();
    assert_eq!(
        *events.lock().unwrap(),
        ["observe", "command:[34, 1]", "shutdown", "drop"]
    );
}

fn rejected_submission(
    replies: &async_channel::Receiver<Reply>,
    generation: Option<u64>,
    diagnostic: &str,
) {
    let (actual_generation, result) = match replies.recv_blocking().unwrap() {
        Reply::Finished { generation, result } => (Some(generation), result),
        Reply::Restored(result) => (None, result),
        _ => panic!("expected a rejected thermal submission"),
    };
    assert_eq!(actual_generation, generation);
    assert!(result.unwrap_err().contains(diagnostic));
}

fn assert_clean_terminal_and_successor(
    owner: &Session,
    replies: &async_channel::Receiver<Reply>,
    events: &Arc<Mutex<Vec<String>>>,
) {
    assert!(matches!(
        replies.recv_blocking().unwrap(),
        Reply::Stopped(Ok(()))
    ));
    assert!(!owner.is_accepting());
    assert!(owner.submit(90, pump()).is_err());
    assert!(owner.activate_telemetry().is_err());
    owner.finish();
    owner.cancel();
    owner.wait().unwrap();
    owner.wait().unwrap();
    let predecessor = owner.clone();
    let successor_events = events.clone();
    let (successor, replies) = Session::start_with(move |_| {
        predecessor.wait().map_err(anyhow::Error::msg)?;
        successor_events.lock().unwrap().push("successor".into());
        Ok(fake(&successor_events))
    })
    .unwrap();
    successor.submit(91, fan()).unwrap();
    successor.finish();
    let (completed, cleanup) = finish(&replies);
    assert_eq!(completed, [(91, Ok(()))]);
    assert_eq!(cleanup, Ok(()));
    successor.wait().unwrap();
    let events = events.lock().unwrap();
    let index = events
        .iter()
        .position(|event| event == "successor")
        .unwrap();
    assert_eq!(&events[index - 2..index], ["shutdown", "drop"]);
    assert_eq!(
        &events[index + 1..],
        ["command:[34, 1]", "shutdown", "drop"]
    );
}

#[test]
fn stopped_startup_rejects_every_accepted_task_and_allows_a_clean_successor() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut transport = fake(&events);
    transport.hid_failure = Some("simulated stopped startup".into());
    let (release, gate) = mpsc::channel();
    let (owner, replies) = Session::start_with(move |_| {
        gate.recv().unwrap();
        Ok(transport)
    })
    .unwrap();
    owner.submit(1, fan()).unwrap();
    owner.restore(vec![pump()]).unwrap();
    owner.submit(2, pump()).unwrap();
    release.send(()).unwrap();
    rejected_submission(&replies, Some(1), "stopped startup");
    rejected_submission(&replies, None, "stopped startup");
    rejected_submission(&replies, Some(2), "stopped startup");
    assert_clean_terminal_and_successor(&owner, &replies, &events);
    assert_eq!(&events.lock().unwrap()[..2], ["shutdown", "drop"]);
}

#[test]
fn fatal_setter_stops_the_batch_and_rejects_the_remaining_fifo_before_cleanup() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (release, gate) = mpsc::channel();
    let (started, observed) = mpsc::channel();
    let mut transport = fake(&events);
    transport.stop_command = true;
    transport.gate = Some(gate);
    transport.started = Some(started);
    let (owner, replies) = Session::start_with(move |_| Ok(transport)).unwrap();
    owner.submit_batch(1, vec![fan(), pump()]).unwrap();
    observed.recv_timeout(Duration::from_secs(5)).unwrap();
    owner.submit(2, pump()).unwrap();
    owner.restore(vec![fan()]).unwrap();
    release.send(()).unwrap();
    rejected_submission(&replies, Some(1), "fatal setter failure");
    rejected_submission(&replies, Some(2), "fatal setter failure");
    rejected_submission(&replies, None, "fatal setter failure");
    assert_clean_terminal_and_successor(&owner, &replies, &events);
    assert_eq!(
        &events.lock().unwrap()[..3],
        ["command:[34, 1]", "shutdown", "drop"]
    );
}

#[test]
fn fatal_telemetry_read_closes_admission_without_sending_the_pending_command() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (release, gate) = mpsc::channel();
    let (started, observed) = mpsc::channel();
    let mut transport = fake(&events);
    transport.stop_observe = true;
    transport.gate = Some(gate);
    transport.started = Some(started);
    let (owner, replies) =
        Session::start_with_interval(move |_| Ok(transport), Some(Duration::ZERO)).unwrap();
    observed.recv_timeout(Duration::from_secs(5)).unwrap();
    owner.submit(8, fan()).unwrap();
    release.send(()).unwrap();
    assert!(matches!(
        replies.recv_blocking().unwrap(),
        Reply::Observed(Err(error)) if error.contains("fatal telemetry failure")
    ));
    rejected_submission(&replies, Some(8), "fatal telemetry failure");
    assert_clean_terminal_and_successor(&owner, &replies, &events);
    assert_eq!(
        &events.lock().unwrap()[..3],
        ["observe", "shutdown", "drop"]
    );
}

#[test]
fn fatal_system_temperature_write_stops_later_samples_and_pending_commands() {
    use razer_device::system_monitor::SystemSample;
    let events = Arc::new(Mutex::new(Vec::new()));
    let (release, gate) = mpsc::channel();
    let (started, observed) = mpsc::channel();
    let mut transport = fake(&events);
    transport.stop_command = true;
    transport.gate = Some(gate);
    transport.started = Some(started);
    let sample = SystemSample {
        cpu_temperature: Some(40.),
        cpu_clock_mhz: Some(4200.),
        gpu_temperature: Some(60.),
        gpu_clock_mhz: Some(1600.),
    };
    let mut later = sample.clone();
    later.cpu_temperature = Some(41.);
    transport.system_samples = vec![sample.clone(), later];
    let (owner, replies) = Session::start_with_schedule(
        move |_| Ok(transport),
        PollSchedule {
            telemetry_interval: None,
            telemetry: None,
            system: Some(Instant::now()),
        },
    )
    .unwrap();
    observed.recv_timeout(Duration::from_secs(5)).unwrap();
    owner.submit(8, fan()).unwrap();
    release.send(()).unwrap();
    assert!(matches!(
        replies.recv_blocking().unwrap(),
        Reply::SystemObserved(Ok(actual)) if actual == sample
    ));
    assert!(matches!(
        replies.recv_blocking().unwrap(),
        Reply::SystemWriteFinished(Err(error)) if error.contains("fatal setter failure")
    ));
    rejected_submission(&replies, Some(8), "fatal setter failure");
    assert_clean_terminal_and_successor(&owner, &replies, &events);
    assert_eq!(
        &events.lock().unwrap()[..4],
        ["system", "command:[192, 1]", "shutdown", "drop"]
    );
}

#[test]
fn fatal_operation_failure_stays_separate_from_a_failed_cleanup_receipt() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (release, gate) = mpsc::channel();
    let mut transport = fake(&events);
    transport.stop_command = true;
    transport.fail_shutdown = true;
    let (owner, replies) = Session::start_with(move |_| {
        gate.recv().unwrap();
        Ok(transport)
    })
    .unwrap();
    owner.submit(1, fan()).unwrap();
    release.send(()).unwrap();
    rejected_submission(&replies, Some(1), "fatal setter failure");
    let Reply::Stopped(cleanup) = replies.recv_blocking().unwrap() else {
        panic!("expected the independent cleanup receipt");
    };
    assert!(cleanup.as_ref().unwrap_err().contains("cleanup failure"));
    assert!(!cleanup.as_ref().unwrap_err().contains("setter failure"));
    assert_eq!(owner.wait(), cleanup);
    assert_eq!(owner.wait(), cleanup);
    assert!(!owner.is_accepting());
    assert_eq!(
        *events.lock().unwrap(),
        ["command:[34, 1]", "shutdown", "drop"]
    );
}
