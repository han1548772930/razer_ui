//! Pure queue, lifecycle and error tests; no DLL, kernel driver or device opens.
use super::*;
fn sample(temperature: Option<f64>) -> SystemSample {
    SystemSample {
        cpu_temperature: temperature,
        cpu_clock_mhz: Some(4321.25),
        gpu_temperature: Some(64.5),
        gpu_clock_mhz: None,
    }
}
#[test]
fn system_monitor_preserves_actual_samples_and_poll_boundaries() {
    let mut queue = Queue::default();
    for value in [Some(35.25), None, Some(42.75)] {
        queue.push(sample(value), Vec::new());
    }
    let first = queue.drain("owner", 2);
    first.validate_for("owner").unwrap();
    assert_eq!(
        first
            .samples
            .iter()
            .map(|sample| sample.cpu_temperature)
            .collect::<Vec<_>>(),
        [Some(35.25), None]
    );
    assert_eq!(
        queue.drain("owner", 2).samples[0].cpu_temperature,
        Some(42.75)
    );
}
#[test]
fn system_monitor_queue_is_bounded_and_reports_discarded_samples() {
    let mut queue = Queue::default();
    for index in 0..MAX_PENDING_SAMPLES + 3 {
        queue.push(sample(Some(index as f64)), Vec::new());
    }
    let result = queue.drain("owner", usize::MAX);
    assert_eq!(result.samples.len(), MAX_PENDING_SAMPLES);
    assert_eq!(result.samples[0].cpu_temperature, Some(3.0));
    assert!(result.diagnostics[0].starts_with("3 old"));
}
#[test]
fn system_monitor_read_failure_never_uses_an_old_or_default_value() {
    let mut diagnostics = Vec::new();
    assert_eq!(
        observed(Ok(0.0), "temperature", &mut diagnostics),
        Some(0.0)
    );
    assert_eq!(
        observed(
            Err(anyhow::anyhow!("sensor disconnected")),
            "temperature",
            &mut diagnostics
        ),
        None
    );
    assert_eq!(observed(Ok(f64::NAN), "clock", &mut diagnostics), None);
    assert_eq!(diagnostics.len(), 2);
}
struct MockSource {
    control: Arc<Control>,
    closed: Arc<Mutex<bool>>,
    fail_close: bool,
}
impl Source for MockSource {
    fn sample(&mut self) -> (SystemSample, Vec<String>) {
        self.control.stop();
        (
            sample(Some(51.75)),
            vec!["actual simulated response".into()],
        )
    }
    fn close(&mut self) -> anyhow::Result<()> {
        *self.closed.lock().unwrap() = true;
        ensure!(!self.fail_close, "simulated close failed");
        Ok(())
    }
}
#[test]
fn system_monitor_cancel_wakes_wait_and_joins_cleanup() {
    let control = Arc::new(Control::default());
    let source_control = control.clone();
    let queue = Arc::new(Mutex::new(Queue::default()));
    let closed = Arc::new(Mutex::new(false));
    let source_closed = closed.clone();
    run(
        move || MockSource {
            control: source_control,
            closed: source_closed,
            fail_close: false,
        },
        queue.clone(),
        control,
        Timer::new(
            Instant::now() - Duration::from_secs(3600),
            Duration::from_secs(3600),
        ),
        Publication::new(Instant::now(), Duration::from_secs(3600)),
    )
    .unwrap();
    assert!(*closed.lock().unwrap());
    assert!(queue.lock().unwrap().samples.is_empty());
}
#[test]
fn system_monitor_cleanup_failure_is_not_a_success() {
    let control = Arc::new(Control::default());
    let source_control = control.clone();
    let closed = Arc::new(Mutex::new(false));
    let source_closed = closed.clone();
    assert!(
        run(
            move || MockSource {
                control: source_control,
                closed: source_closed,
                fail_close: true
            },
            Arc::new(Mutex::new(Queue::default())),
            control,
            Timer::new(
                Instant::now() - Duration::from_secs(3600),
                Duration::from_secs(3600),
            ),
            Publication::new(Instant::now(), Duration::from_secs(3600))
        )
        .is_err()
    );
    assert!(*closed.lock().unwrap());
}
#[test]
fn system_monitor_old_owner_cannot_poll_or_stop_a_successor() {
    let mut state = State {
        active: Some(Session {
            owner_guid: "new-owner".into(),
            queue: Arc::new(Mutex::new(Queue::default())),
            control: Arc::new(Control::default()),
            thread: None,
            terminal_error: None,
        }),
    };
    assert!(state.poll("old-owner").is_err());
    assert!(state.stop("old-owner").is_err());
    assert!(state.poll("new-owner").is_ok());
}

#[test]
fn system_monitor_failed_cleanup_cannot_turn_into_success_on_a_second_stop() {
    let mut session = Session {
        owner_guid: "owner".into(),
        queue: Arc::new(Mutex::new(Queue::default())),
        control: Arc::new(Control::default()),
        thread: Some(std::thread::spawn(|| anyhow::bail!("cleanup failed"))),
        terminal_error: None,
    };
    assert!(session.shutdown().is_err());
    assert!(session.shutdown().is_err());
}

#[test]
fn system_monitor_publication_matches_source_two_decimal_rounding() {
    assert_eq!(
        observed(Ok(4321.256), "clock", &mut Vec::new()),
        Some(4321.26)
    );
}

#[test]
fn system_monitor_registration_waits_for_a_due_tick_and_an_actual_frame() {
    // Current SystemCommon RegisterFFIEvent 0x6d360 records its registration
    // tick; publisher 0x67850 requires 1000 ms and a received hardware frame.
    let registered = Instant::now();
    let control = Control::default();
    let queue = Mutex::new(Queue::default());
    let mut publication = Publication::new(registered, Duration::from_millis(1000));
    assert!(
        !control
            .publish(
                &mut publication,
                registered + Duration::from_millis(1000),
                &queue,
            )
            .unwrap()
    );
    let actual = sample(Some(51.75));
    control.observe(actual.clone(), Vec::new()).unwrap();
    assert!(
        !control
            .publish(
                &mut publication,
                registered + Duration::from_millis(1001),
                &queue,
            )
            .unwrap()
    );
    assert!(queue.lock().unwrap().samples.is_empty());
    assert!(
        control
            .publish(
                &mut publication,
                registered + Duration::from_millis(2000),
                &queue,
            )
            .unwrap()
    );
    assert_eq!(queue.lock().unwrap().samples.front(), Some(&actual));
}

#[test]
fn system_monitor_sampling_and_subscription_keep_independent_phases() {
    let initialized = Instant::now();
    let registered = initialized + Duration::from_millis(250);
    let control = Control::default();
    let queue = Mutex::new(Queue::default());
    let mut publication = Publication::new(registered, Duration::from_millis(1000));
    let mut sync = Timer::new(initialized, Duration::from_millis(1000));
    let actual = sample(Some(63.25));
    assert!(!sync.tick(initialized + Duration::from_millis(999)));
    assert!(
        !control
            .publish(
                &mut publication,
                initialized + Duration::from_millis(999),
                &queue,
            )
            .unwrap()
    );
    assert!(queue.lock().unwrap().drain("owner", 256).samples.is_empty());
    assert!(sync.tick(initialized + Duration::from_millis(1000)));
    control.observe(actual.clone(), Vec::new()).unwrap();
    assert!(
        !control
            .publish(
                &mut publication,
                initialized + Duration::from_millis(1000),
                &queue,
            )
            .unwrap()
    );
    assert!(
        control
            .publish(
                &mut publication,
                registered + Duration::from_millis(1000),
                &queue,
            )
            .unwrap()
    );
    assert_eq!(queue.lock().unwrap().drain("owner", 256).samples, [actual]);
    assert_eq!(sync.next, initialized + Duration::from_millis(2000));
    assert_eq!(
        publication.timer.next,
        registered + Duration::from_millis(2000)
    );
}

#[test]
fn system_monitor_late_ticks_coalesce_without_bursts_or_interval_drift() {
    let registered = Instant::now();
    let actual = sample(Some(40.5));
    let mut publication = Publication::new(registered, Duration::from_millis(1000));
    assert!(
        publication
            .tick(registered + Duration::from_millis(1000), Some(&actual))
            .is_some()
    );
    assert!(
        publication
            .tick(registered + Duration::from_millis(3500), Some(&actual))
            .is_some()
    );
    assert_eq!(
        publication.timer.next,
        registered + Duration::from_millis(4000)
    );
    assert!(
        publication
            .tick(registered + Duration::from_millis(3500), Some(&actual))
            .is_none()
    );
    assert!(
        publication
            .tick(registered + Duration::from_millis(4000), Some(&actual))
            .is_none()
    );
    assert!(
        publication
            .tick(registered + Duration::from_millis(5000), Some(&actual))
            .is_some()
    );
    let mut sync = Timer::new(registered, Duration::from_millis(1000));
    assert!(!sync.tick(registered + Duration::from_millis(999)));
    assert!(sync.tick(registered + Duration::from_millis(3200)));
    assert_eq!(sync.next, registered + Duration::from_millis(4000));
    assert!(!sync.tick(registered + Duration::from_millis(3200)));
}

#[test]
fn system_monitor_callback_uses_the_last_received_frame_while_a_read_is_pending() {
    let registered = Instant::now();
    let control = Control::default();
    let queue = Mutex::new(Queue::default());
    let mut publication = Publication::new(registered, Duration::from_millis(1000));
    let initial = sample(Some(40.5));
    control.observe(initial.clone(), Vec::new()).unwrap();
    control
        .publish(
            &mut publication,
            registered + Duration::from_millis(1000),
            &queue,
        )
        .unwrap();
    control
        .publish(
            &mut publication,
            registered + Duration::from_millis(2000),
            &queue,
        )
        .unwrap();
    let failed_read = sample(None);
    control.observe(failed_read.clone(), Vec::new()).unwrap();
    control
        .publish(
            &mut publication,
            registered + Duration::from_millis(3000),
            &queue,
        )
        .unwrap();
    assert_eq!(
        queue.lock().unwrap().drain("owner", 256).samples,
        [initial.clone(), initial, failed_read],
    );
}

#[test]
fn system_monitor_cancel_rejects_in_flight_frames_and_stops_future_publication() {
    let registered = Instant::now();
    let control = Arc::new(Control::default());
    let queue = Mutex::new(Queue::default());
    let mut publication = Publication::new(registered, Duration::from_millis(1000));
    control.observe(sample(Some(40.5)), Vec::new()).unwrap();
    let (started, ready) = std::sync::mpsc::channel();
    let (release, resumed) = std::sync::mpsc::channel();
    let closed = Arc::new(Mutex::new(false));
    let worker_closed = closed.clone();
    let worker_control = control.clone();
    struct BlockedSource {
        started: std::sync::mpsc::Sender<()>,
        release: std::sync::mpsc::Receiver<()>,
        closed: Arc<Mutex<bool>>,
    }
    impl Source for BlockedSource {
        fn sample(&mut self) -> (SystemSample, Vec<String>) {
            self.started.send(()).unwrap();
            self.release.recv().unwrap();
            (sample(Some(99.75)), Vec::new())
        }
        fn close(&mut self) -> anyhow::Result<()> {
            *self.closed.lock().unwrap() = true;
            Ok(())
        }
    }
    let worker = std::thread::spawn(move || {
        sample_source(
            || BlockedSource {
                started,
                release: resumed,
                closed: worker_closed,
            },
            &worker_control,
            Timer::new(
                Instant::now() - Duration::from_secs(3600),
                Duration::from_secs(3600),
            ),
        )
    });
    ready.recv().unwrap();
    control.stop();
    release.send(()).unwrap();
    worker.join().unwrap().unwrap();
    assert!(*closed.lock().unwrap());
    assert_eq!(
        control.stream.lock().unwrap().latest,
        Some(sample(Some(40.5)))
    );
    assert!(
        !control
            .publish(
                &mut publication,
                registered + Duration::from_millis(1000),
                &queue,
            )
            .unwrap()
    );
    assert!(queue.lock().unwrap().samples.is_empty());
}

#[test]
fn system_monitor_sampling_panic_still_closes_the_source_and_reports_cleanup_failure() {
    struct PanickingSource(Arc<Mutex<bool>>);
    impl Source for PanickingSource {
        fn sample(&mut self) -> (SystemSample, Vec<String>) {
            panic!("simulated sensor panic");
        }
        fn close(&mut self) -> anyhow::Result<()> {
            *self.0.lock().unwrap() = true;
            anyhow::bail!("simulated cleanup failure")
        }
    }
    let closed = Arc::new(Mutex::new(false));
    let result = sample_source(
        || PanickingSource(closed.clone()),
        &Control::default(),
        Timer::new(
            Instant::now() - Duration::from_secs(3600),
            Duration::from_secs(3600),
        ),
    )
    .unwrap_err();
    assert!(*closed.lock().unwrap());
    let diagnostic = format!("{result:#}");
    assert!(diagnostic.contains("sampling panicked"));
    assert!(diagnostic.contains("simulated cleanup failure"));
}

#[test]
fn system_monitor_cancel_before_the_first_request_never_initializes_the_source() {
    let control = Control::default();
    control.stop();
    let initialized = std::cell::Cell::new(false);
    let closed = Arc::new(Mutex::new(false));
    sample_source(
        || {
            initialized.set(true);
            MockSource {
                control: Arc::new(Control::default()),
                closed: closed.clone(),
                fail_close: false,
            }
        },
        &control,
        Timer::new(Instant::now(), Duration::from_secs(3600)),
    )
    .unwrap();
    assert!(!initialized.get());
    assert!(!*closed.lock().unwrap());
}

#[test]
fn system_monitor_cancel_during_source_initialization_closes_without_reading() {
    struct NeverSample(Arc<Mutex<Vec<&'static str>>>);
    impl Source for NeverSample {
        fn sample(&mut self) -> (SystemSample, Vec<String>) {
            self.0.lock().unwrap().push("read");
            panic!("cancelled initialization cannot start a hardware read");
        }
        fn close(&mut self) -> anyhow::Result<()> {
            self.0.lock().unwrap().push("close");
            Ok(())
        }
    }
    let control = Control::default();
    let events = Arc::new(Mutex::new(Vec::new()));
    sample_source(
        || {
            events.lock().unwrap().push("initialize");
            control.stop();
            NeverSample(events.clone())
        },
        &control,
        Timer::new(
            Instant::now() - Duration::from_secs(3600),
            Duration::from_secs(3600),
        ),
    )
    .unwrap();
    assert_eq!(*events.lock().unwrap(), ["initialize", "close"]);
}
