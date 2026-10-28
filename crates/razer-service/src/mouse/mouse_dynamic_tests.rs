use super::*;
use anyhow::ensure;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
};

fn clock(now: Duration) -> ClockSample {
    ClockSample {
        timer: now,
        wall_millis: now.as_millis() as i128,
    }
}

enum Update {
    MonitoringStarted,
    Speed(f64),
}

// This deterministic capture harness services the retained publication timer
// at each scripted poll. The asynchronous owner is exercised separately below.
fn monitor_with(
    backend: &mut impl Monitor,
    product_id: u32,
    active: &AtomicBool,
    admit: impl Fn(&InputEvent) -> anyhow::Result<bool>,
    publish: impl FnMut(Update),
    now: impl FnMut() -> ClockSample,
    wait: impl FnMut(),
) -> Completion {
    struct Timed<'a, B, T> {
        backend: &'a mut B,
        tick: T,
    }
    impl<B: Monitor, T: FnMut()> Monitor for Timed<'_, B, T> {
        fn stop(
            &mut self,
            observe: &mut dyn FnMut(Vec<InputEvent>) -> anyhow::Result<()>,
        ) -> anyhow::Result<()> {
            self.backend.stop(observe)
        }
        fn shutdown(&mut self) -> anyhow::Result<()> {
            self.backend.shutdown()
        }
        fn terminate(&mut self) -> anyhow::Result<()> {
            self.backend.terminate()
        }
        fn start(&mut self) -> anyhow::Result<()> {
            self.backend.start()
        }
        fn repeat_start(&mut self) -> anyhow::Result<StartAcknowledgement> {
            self.backend.repeat_start()
        }
        fn events(&mut self) -> anyhow::Result<Vec<InputEvent>> {
            (self.tick)();
            self.backend.events()
        }
    }
    let history = RefCell::new(SpeedPublication::new(product_id).ok());
    let publish = RefCell::new(publish);
    let now = RefCell::new(now);
    capture_with(
        &mut Timed {
            backend,
            tick: || {
                if let Some(history) = history.borrow_mut().as_mut() {
                    if let Some(speed) = history.publish_due((now.borrow_mut())()) {
                        (publish.borrow_mut())(Update::Speed(speed));
                    }
                }
            },
        },
        product_id,
        active,
        &monitor_requests().1,
        admit,
        |json, admitted| {
            // This harness represents the single listener removed by STOP.
            // Retained PID226 listeners use SpeedHistoryInput below.
            if !active.load(Ordering::Acquire) {
                return Ok(());
            }
            if let Some(speed) = history
                .borrow_mut()
                .as_mut()
                .expect("Capture validates its product before observing input")
                .observe(json, admitted, &mut *now.borrow_mut())?
            {
                (publish.borrow_mut())(Update::Speed(speed));
            }
            Ok(())
        },
        || {
            if let Some(history) = history.borrow_mut().as_mut() {
                history.stop_acknowledged();
            }
            Ok(())
        },
        || (publish.borrow_mut())(Update::MonitoringStarted),
        wait,
    )
}

struct Backend {
    calls: RefCell<Vec<&'static str>>,
    batches: VecDeque<anyhow::Result<Vec<InputEvent>>>,
    start_failure: bool,
    stop_failure: bool,
    termination_failures: usize,
    cancel_on_poll: Option<Arc<AtomicBool>>,
    repeat_results: VecDeque<StartResult>,
    final_events: Vec<InputEvent>,
}
impl Backend {
    fn new() -> Self {
        Self {
            calls: RefCell::new(vec![]),
            batches: VecDeque::new(),
            start_failure: false,
            stop_failure: false,
            termination_failures: 0,
            cancel_on_poll: None,
            repeat_results: VecDeque::new(),
            final_events: Vec::new(),
        }
    }
}
impl Monitor for Backend {
    fn start(&mut self) -> anyhow::Result<()> {
        self.calls.borrow_mut().push("start");
        ensure!(!self.start_failure, "lost START acknowledgement");
        Ok(())
    }
    fn events(&mut self) -> anyhow::Result<Vec<InputEvent>> {
        self.calls.borrow_mut().push("poll");
        if let Some(active) = &self.cancel_on_poll {
            active.store(false, Ordering::Release);
        }
        self.batches.pop_front().unwrap_or_else(|| Ok(vec![]))
    }
    fn repeat_start(&mut self) -> anyhow::Result<StartAcknowledgement> {
        self.calls.borrow_mut().push("repeat start");
        self.repeat_results
            .pop_front()
            .expect("Scripted repeat START requires an explicit response")
            .map_err(anyhow::Error::msg)
    }
    fn stop(
        &mut self,
        observe: &mut dyn FnMut(Vec<InputEvent>) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        self.calls.borrow_mut().push("stop");
        ensure!(!self.stop_failure, "lost STOP acknowledgement");
        if !self.final_events.is_empty() {
            observe(std::mem::take(&mut self.final_events))?;
        }
        Ok(())
    }
    fn shutdown(&mut self) -> anyhow::Result<()> {
        self.calls.borrow_mut().push("shutdown");
        Ok(())
    }
    fn terminate(&mut self) -> anyhow::Result<()> {
        self.calls.borrow_mut().push("terminate");
        if self.termination_failures > 0 {
            self.termination_failures -= 1;
            anyhow::bail!("process still owned");
        }
        Ok(())
    }
}

#[test]
fn mouse_dynamic_filtered_callback_still_publishes_source_zero() {
    for product_id in [190, 192, 196, 222, 226, 229, 239] {
        let mut backend = Backend::new();
        backend.batches.push_back(Ok(vec![batch("other", 999)]));
        let active = AtomicBool::new(true);
        let speeds = RefCell::new(vec![]);
        let completion = monitor_with(
            &mut backend,
            product_id,
            &active,
            |_| Ok(false),
            |update| {
                if let Update::Speed(speed) = update {
                    speeds.borrow_mut().push(speed);
                }
            },
            || clock(Duration::ZERO),
            || active.store(false, Ordering::Release),
        );
        assert!(completion.result.is_ok());
        assert!(completion.cleanup_failure.is_none());
        assert_eq!(*speeds.borrow(), vec![0.], "{product_id}");
    }
}

#[test]
fn mouse_dynamic_cancellation_during_poll_rejects_final_batch_before_stop() {
    let active = Arc::new(AtomicBool::new(true));
    let mut backend = Backend::new();
    backend.cancel_on_poll = Some(active.clone());
    backend.batches.push_back(Ok(vec![batch("current", 190)]));
    let speeds = RefCell::new(vec![]);
    let completion = monitor_with(
        &mut backend,
        190,
        &active,
        |_| Ok(true),
        |update| {
            if let Update::Speed(speed) = update {
                speeds.borrow_mut().push(speed);
            }
        },
        || clock(Duration::ZERO),
        || {},
    );
    assert!(completion.result.is_ok());
    assert!(completion.cleanup_failure.is_none());
    assert!(speeds.borrow().is_empty());
    assert_eq!(
        *backend.calls.borrow(),
        vec!["start", "poll", "stop", "shutdown", "terminate"]
    );
}
fn batch(container: &str, pid: u32) -> InputEvent {
    InputEvent::from_native(&format!(r#"{{"containerId":"{container}","productId":{pid},"vendorId":5426}}"#),1,
        r#"[{"timeTick":"10000","data":"{\"type\":\"mouseMove\",\"x\":40,\"y\":40}"},{"timeTick":"20000","data":"{\"type\":\"mouseMove\",\"x\":3,\"y\":4}"}]"#.into()).unwrap()
}

#[test]
fn mouse_dynamic_event_filter_uses_product_and_current_pairing_without_event_container_gate() {
    for (product_id, dongle_id) in [
        (190, 191),
        (192, 193),
        (196, 197),
        (222, 223),
        (226, 227),
        (229, 230),
        (239, 240),
    ] {
        for event_product_id in [product_id, dongle_id] {
            let event = InputEvent::from_native(
                &format!(r#"{{"containerId":"another-instance","productId":{event_product_id},"vendorId":999}}"#),
                1, "[]".into(),
            ).unwrap();
            assert!(
                admits_with(product_id, &event, |_| panic!(
                    "direct IDs never read pairing"
                ))
                .unwrap()
            );
        }
        let receiver = batch("receiver-event-container", 165);
        for matched in [false, true] {
            assert_eq!(
                admits_with(product_id, &receiver, |pid| {
                    assert_eq!(pid, 165);
                    Ok(matched)
                })
                .unwrap(),
                matched
            );
        }
        assert!(
            admits_with(product_id, &receiver, |_| anyhow::bail!(
                "pairing read failed"
            ))
            .is_err()
        );
    }
    assert!(admits_with(226, &batch("ble", 228), |_| panic!("source BLE is direct")).unwrap());
    assert!(!admits_with(190, &batch("ble", 228), |_| Ok(false)).unwrap());
}

#[test]
fn mouse_dynamic_filtered_trailing_callback_preserves_each_products_pending_value() {
    for product_id in [190, 192, 196, 222, 226, 229, 239] {
        let mut updates = SpeedPublication::new(product_id).unwrap();
        assert_eq!(
            updates
                .observe("[]", true, || clock(Duration::ZERO))
                .unwrap(),
            Some(0.)
        );
        assert_eq!(updates.observe(r#"[{"timeTick":"10000","data":"{\"type\":\"mouseMove\",\"x\":3,\"y\":4}"},{"timeTick":"20000","data":"{\"type\":\"mouseMove\",\"x\":6,\"y\":8}"}]"#, true, || clock(Duration::ZERO)).unwrap(), None);
        assert_eq!(
            updates
                .observe("[]", false, || clock(Duration::ZERO))
                .unwrap(),
            None
        );
        assert!(
            updates
                .publish_due(clock(Duration::from_millis(16)))
                .is_none()
        );
        assert_eq!(
            updates.publish_due(clock(Duration::from_millis(17))),
            Some(if product_id == 226 { 0. } else { 10. })
        );
        assert!(
            updates
                .publish_due(clock(Duration::from_millis(34)))
                .is_none()
        );
    }
}

#[test]
fn mouse_dynamic_monitor_timer_follows_the_next_callback_after_a_trailing_publication() {
    for product_id in [190, 192, 196, 222, 226, 229, 239] {
        let mut backend = Backend::new();
        for events in [
            vec![batch("current", product_id)],
            vec![batch("current", product_id)],
            vec![],
            vec![batch("current", product_id)],
        ] {
            backend.batches.push_back(Ok(events));
        }
        let active = AtomicBool::new(true);
        let tick = Cell::new(Duration::ZERO);
        let waits = Cell::new(0);
        let publications = RefCell::new(vec![]);
        let completion = monitor_with(
            &mut backend,
            product_id,
            &active,
            |_| Ok(false),
            |update| {
                if let Update::Speed(speed) = update {
                    publications.borrow_mut().push((tick.get(), speed));
                }
            },
            || clock(tick.get()),
            || {
                let timeline = [16, 17, 18, 34, 35];
                let index = waits.get();
                waits.set(index + 1);
                if let Some(next) = timeline.get(index) {
                    tick.set(Duration::from_millis(*next));
                } else {
                    active.store(false, Ordering::Release);
                }
            },
        );
        assert!(completion.result.is_ok());
        assert!(completion.cleanup_failure.is_none());
        assert_eq!(
            *publications.borrow(),
            vec![
                (Duration::ZERO, 0.),
                (Duration::from_millis(17), 0.),
                (Duration::from_millis(35), 0.),
            ],
            "{product_id}"
        );
    }
}

#[test]
fn mouse_dynamic_monitor_timer_publishes_the_callback_after_a_clock_rollback() {
    for product_id in [190, 192, 196, 222, 226, 229, 239] {
        let mut backend = Backend::new();
        for _ in 0..2 {
            backend
                .batches
                .push_back(Ok(vec![batch("current", product_id)]));
        }
        let active = AtomicBool::new(true);
        let tick = Cell::new(Duration::from_millis(100));
        let waits = Cell::new(0);
        let publications = RefCell::new(vec![]);
        let completion = monitor_with(
            &mut backend,
            product_id,
            &active,
            |_| Ok(false),
            |update| {
                if let Update::Speed(speed) = update {
                    publications.borrow_mut().push((tick.get(), speed));
                }
            },
            || clock(tick.get()),
            || {
                waits.set(waits.get() + 1);
                if waits.get() == 1 {
                    tick.set(Duration::from_millis(90));
                } else {
                    active.store(false, Ordering::Release);
                }
            },
        );
        assert!(completion.result.is_ok());
        assert!(completion.cleanup_failure.is_none());
        assert_eq!(
            *publications.borrow(),
            vec![
                (Duration::from_millis(100), 0.),
                (Duration::from_millis(90), 0.),
            ],
            "{product_id}"
        );
    }
}

#[test]
fn mouse_dynamic_timer_uses_wall_time_for_rollback_and_the_retained_timer_for_delivery() {
    for product_id in [190, 192, 196, 222, 226, 229, 239] {
        let mut updates = SpeedPublication::new(product_id).unwrap();
        for (timer_millis, wall_millis, expected) in [(0, 100, Some(0.)), (1, 101, None)] {
            assert_eq!(
                updates
                    .observe("[]", false, || ClockSample {
                        timer: Duration::from_millis(timer_millis),
                        wall_millis,
                    },)
                    .unwrap(),
                expected
            );
        }
        assert_eq!(
            updates.publish_due(ClockSample {
                timer: Duration::from_millis(17),
                wall_millis: 90,
            }),
            Some(0.),
            "{product_id}"
        );
        assert!(
            updates
                .publish_due(ClockSample {
                    timer: Duration::from_millis(34),
                    wall_millis: 107,
                })
                .is_none()
        );
    }
}

#[test]
fn mouse_dynamic_timer_reschedules_until_the_wall_clock_reaches_the_source_wait() {
    for product_id in [190, 192, 196, 222, 226, 229, 239] {
        let mut updates = SpeedPublication::new(product_id).unwrap();
        assert_eq!(
            updates
                .observe("[]", false, || clock(Duration::ZERO))
                .unwrap(),
            Some(0.)
        );
        assert_eq!(
            updates
                .observe("[]", false, || clock(Duration::from_millis(1)))
                .unwrap(),
            None
        );
        assert!(
            updates
                .publish_due(ClockSample {
                    timer: Duration::from_nanos(16_666_667),
                    wall_millis: 16,
                })
                .is_none()
        );
        assert!(
            updates
                .publish_due(clock(Duration::from_millis(17)))
                .is_none()
        );
        assert_eq!(
            updates.publish_due(clock(Duration::from_millis(18))),
            Some(0.),
            "{product_id}"
        );
    }
}

#[test]
fn mouse_dynamic_max_wait_callback_rearms_the_timer_without_a_duplicate_trailing_publish() {
    for product_id in [190, 192, 196, 222, 226, 229, 239] {
        let mut updates = SpeedPublication::new(product_id).unwrap();
        for (millis, expected) in [(0, Some(0.)), (16, None), (18, Some(0.))] {
            assert_eq!(
                updates
                    .observe("[]", false, || clock(Duration::from_millis(millis)))
                    .unwrap(),
                expected,
                "{product_id}/{millis}"
            );
        }
        assert!(
            updates
                .publish_due(clock(Duration::from_millis(35)))
                .is_none()
        );
        assert_eq!(
            updates
                .observe("[]", false, || clock(Duration::from_millis(36)))
                .unwrap(),
            Some(0.)
        );
        assert!(
            updates
                .publish_due(clock(Duration::from_millis(53)))
                .is_none()
        );
    }
}

#[test]
fn mouse_dynamic_pairing_failure_stops_capture_and_releases_transport() {
    let mut backend = Backend::new();
    backend.batches.push_back(Ok(vec![batch("receiver", 165)]));
    let completion = monitor_with(
        &mut backend,
        190,
        &AtomicBool::new(true),
        |_| anyhow::bail!("pairing read failed"),
        |update| assert!(matches!(update, Update::MonitoringStarted)),
        || clock(Duration::ZERO),
        || {},
    );
    assert!(
        completion
            .result
            .unwrap_err()
            .contains("pairing read failed")
    );
    assert!(completion.cleanup_failure.is_none());
    assert_eq!(
        *backend.calls.borrow(),
        vec!["start", "poll", "stop", "shutdown", "terminate"]
    );
}

#[test]
fn mouse_dynamic_invalid_callback_does_not_request_a_throttle_clock_or_schedule_a_timer() {
    for product_id in [190, 192, 196, 222, 226, 229, 239] {
        let mut updates = SpeedPublication::new(product_id).unwrap();
        assert!(
            updates
                .observe("broken", false, || panic!(
                    "the source parses the callback before calling throttle"
                ))
                .is_err()
        );
        assert!(updates.publish_due(clock(Duration::from_secs(1))).is_none());
        assert_eq!(
            updates
                .observe("[]", false, || clock(Duration::from_secs(1)))
                .unwrap(),
            Some(0.)
        );
        assert!(
            updates
                .publish_due(clock(Duration::from_millis(1017)))
                .is_none()
        );
    }
}

#[test]
fn mouse_dynamic_monitor_publishes_each_products_native_string_tick_result() {
    for product_id in [190, 192, 196, 222, 226, 229, 239] {
        for (events, relative, previous_delta) in [
            (
                r#"[{"timeTick":"9","data":"{\"type\":\"mouseMove\",\"x\":3,\"y\":4}"},{"timeTick":"10","data":"{\"type\":\"mouseMove\",\"x\":6,\"y\":8}"}]"#,
                0.,
                50000.,
            ),
            (
                r#"[{"timeTick":"10000","data":"{\"type\":\"mouseMove\",\"x\":3,\"y\":4}"},{"timeTick":"20000","data":"{\"type\":\"mouseMove\",\"x\":3,\"y\":4}"}]"#,
                5.,
                0.,
            ),
            (
                r#"[{"timeTick":"10000","data":"{\"type\":\"mouseMove\",\"x\":3,\"y\":4}"},{"timeTick":"10000","data":"{\"type\":\"mouseMove\",\"x\":6,\"y\":8}"}]"#,
                0.,
                f64::INFINITY,
            ),
        ] {
            let mut backend = Backend::new();
            let event = InputEvent::from_native(
                &format!(r#"{{"containerId":"current","productId":{product_id},"vendorId":5426}}"#),
                1,
                events.into(),
            )
            .unwrap();
            backend.batches.push_back(Ok(vec![event]));
            let active = AtomicBool::new(true);
            let updates = RefCell::new(vec![]);
            let result = monitor_with(
                &mut backend,
                product_id,
                &active,
                |event| Ok(event.container_id() == "current"),
                |update| match update {
                    Update::MonitoringStarted => updates.borrow_mut().push(-1.),
                    Update::Speed(value) => updates.borrow_mut().push(value),
                },
                || clock(Duration::ZERO),
                || active.store(false, Ordering::Release),
            );
            assert!(matches!(result.result, Ok(Outcome::MonitoringStopped)));
            assert!(result.cleanup_failure.is_none());
            assert_eq!(
                *updates.borrow(),
                vec![
                    -1.,
                    if product_id == 226 {
                        previous_delta
                    } else {
                        relative
                    }
                ],
                "{product_id}"
            );
            assert_eq!(
                *backend.calls.borrow(),
                vec!["start", "poll", "stop", "shutdown", "terminate"]
            );
        }
    }
}

#[test]
fn mouse_dynamic_monitor_preserves_leading_and_latest_trailing_callback_order() {
    for product_id in [190, 192, 196, 222, 226, 229, 239] {
        let mut backend = Backend::new();
        let events = [
            r#"[{"timeTick":"10000","data":"{\"type\":\"mouseMove\",\"x\":3,\"y\":4}"},{"timeTick":"20000","data":"{\"type\":\"mouseMove\",\"x\":3,\"y\":4}"}]"#,
            r#"[{"timeTick":"30000","data":"{\"type\":\"mouseMove\",\"x\":3,\"y\":4}"},{"timeTick":"40000","data":"{\"type\":\"mouseMove\",\"x\":6,\"y\":8}"}]"#,
            r#"[{"timeTick":"50000","data":"{\"type\":\"mouseMove\",\"x\":9,\"y\":12}"}]"#,
        ].into_iter().map(|events| InputEvent::from_native(
            &format!(r#"{{"containerId":"current","productId":{product_id},"vendorId":5426}}"#), 1, events.into(),
        ).unwrap()).collect();
        backend.batches.push_back(Ok(events));
        let active = AtomicBool::new(true);
        let tick = Cell::new(Duration::ZERO);
        let waits = Cell::new(0);
        let observed = RefCell::new(vec![]);
        let result = monitor_with(
            &mut backend,
            product_id,
            &active,
            |_| Ok(true),
            |update| {
                if let Update::Speed(speed) = update {
                    observed.borrow_mut().push((tick.get(), speed));
                }
            },
            || clock(tick.get()),
            || {
                waits.set(waits.get() + 1);
                match waits.get() {
                    1 => tick.set(Duration::from_millis(16)),
                    2 => tick.set(Duration::from_millis(17)),
                    _ => active.store(false, Ordering::Release),
                }
            },
        );
        assert!(matches!(result.result, Ok(Outcome::MonitoringStopped)));
        assert!(result.cleanup_failure.is_none());
        assert_eq!(
            *observed.borrow(),
            vec![
                (Duration::ZERO, if product_id == 226 { 0. } else { 5. }),
                (
                    Duration::from_millis(17),
                    if product_id == 226 { 5. } else { 15. }
                ),
            ],
            "{product_id}"
        );
        assert_eq!(
            &backend.calls.borrow()[4..],
            &["stop", "shutdown", "terminate"]
        );
    }
}

#[test]
fn mouse_dynamic_monitor_rejects_invalid_callback_and_unsupported_product_with_cleanup() {
    for product_id in [190, 192, 196, 222, 226, 229, 239, 228] {
        let mut backend = Backend::new();
        backend.batches.push_back(Ok(vec![
            InputEvent::from_native(
                r#"{"containerId":"current","productId":226,"vendorId":5426}"#,
                1,
                r#"[{"timeTick":"invalid","data":"{\"type\":\"mouseMove\",\"x\":3,\"y\":4}"}]"#
                    .into(),
            )
            .unwrap(),
        ]));
        let speeds = Cell::new(0);
        let result = monitor_with(
            &mut backend,
            product_id,
            &AtomicBool::new(true),
            |_| Ok(true),
            |update| {
                if matches!(update, Update::Speed(_)) {
                    speeds.set(speeds.get() + 1);
                }
            },
            || clock(Duration::ZERO),
            || {},
        );
        assert!(result.result.is_err());
        assert!(result.cleanup_failure.is_none());
        assert_eq!(speeds.get(), 0);
        let calls = backend.calls.borrow();
        if product_id == 228 {
            assert_eq!(*calls, vec!["stop", "shutdown", "terminate"]);
        } else {
            assert_eq!(
                *calls,
                vec!["start", "poll", "stop", "shutdown", "terminate"]
            );
        }
    }
}
#[test]
fn mouse_dynamic_monitor_cancellation_still_stops_shuts_down_and_reaps_before_release() {
    let mut backend = Backend::new();
    backend.batches.push_back(Ok(vec![batch("current", 190)]));
    let active = AtomicBool::new(true);
    let observed = RefCell::new(vec![]);
    let completion = monitor_with(
        &mut backend,
        190,
        &active,
        |event| Ok(event.container_id() == "current"),
        |update| match update {
            Update::MonitoringStarted => observed.borrow_mut().push(-1.),
            Update::Speed(value) => observed.borrow_mut().push(value),
        },
        || clock(Duration::ZERO),
        || active.store(false, Ordering::Release),
    );
    assert!(matches!(completion.result, Ok(Outcome::MonitoringStopped)));
    assert!(completion.cleanup_failure.is_none());
    assert_eq!(*observed.borrow(), vec![-1., 5.]);
    assert_eq!(
        *backend.calls.borrow(),
        vec!["start", "poll", "stop", "shutdown", "terminate"]
    );
}
#[test]
fn mouse_dynamic_partial_start_failure_retains_stop_and_shutdown_obligations() {
    let mut backend = Backend::new();
    backend.start_failure = true;
    let result = monitor_with(
        &mut backend,
        190,
        &AtomicBool::new(true),
        |_| Ok(true),
        |_| panic!("unacknowledged start cannot publish"),
        || clock(Duration::ZERO),
        || panic!("no monitoring loop after failed start"),
    );
    assert!(result.result.is_err());
    assert_eq!(
        *backend.calls.borrow(),
        vec!["start", "stop", "shutdown", "terminate"]
    );
}
#[test]
fn mouse_dynamic_filtered_callback_publishes_zero_and_keeps_native_failure_cleanup() {
    let mut backend = Backend::new();
    backend.batches.push_back(Ok(vec![batch("other", 190)]));
    backend
        .batches
        .push_back(Err(anyhow::anyhow!("callback queue overflow")));
    let speeds = Cell::new(0);
    let result = monitor_with(
        &mut backend,
        190,
        &AtomicBool::new(true),
        |_| Ok(false),
        |update| {
            if matches!(update, Update::Speed(_)) {
                speeds.set(speeds.get() + 1);
            }
        },
        || clock(Duration::ZERO),
        || {},
    );
    assert!(
        result
            .result
            .unwrap_err()
            .contains("callback queue overflow")
    );
    assert_eq!(speeds.get(), 1);
    assert_eq!(
        &backend.calls.borrow()[3..],
        &["stop", "shutdown", "terminate"]
    );
}
#[test]
fn mouse_dynamic_monitor_retains_lease_until_termination_retry_and_keeps_failures_visible() {
    let mut backend = Backend::new();
    backend.stop_failure = true;
    backend.termination_failures = 1;
    let active = AtomicBool::new(true);
    let waits = Cell::new(0);
    let result = monitor_with(
        &mut backend,
        190,
        &active,
        |_| Ok(true),
        |_| {},
        || clock(Duration::ZERO),
        || {
            active.store(false, Ordering::Release);
            waits.set(waits.get() + 1);
        },
    );
    let failure = result.cleanup_failure.unwrap();
    assert!(failure.contains("lost STOP acknowledgement"));
    assert!(failure.contains("process still owned"));
    assert_eq!(
        &backend.calls.borrow()[2..],
        &["stop", "shutdown", "terminate", "terminate"]
    );
    assert_eq!(waits.get(), 2);
}
#[test]
fn mouse_dynamic_monitor_canceled_before_admission_never_starts_or_polls() {
    let mut backend = Backend::new();
    let result = monitor_with(
        &mut backend,
        190,
        &AtomicBool::new(false),
        |_| Ok(true),
        |_| panic!("canceled monitor cannot publish"),
        || clock(Duration::ZERO),
        || {},
    );
    assert!(result.result.is_err());
    assert_eq!(
        *backend.calls.borrow(),
        vec!["stop", "shutdown", "terminate"]
    );
}

const RESTART_MOTION: &str = r#"[{"timeTick":"10000","data":"{\"type\":\"mouseMove\",\"x\":3,\"y\":4}"},{"timeTick":"20000","data":"{\"type\":\"mouseMove\",\"x\":6,\"y\":8}"}]"#;

struct HistoryHarness {
    input: SpeedHistoryInput,
    time: Arc<std::sync::atomic::AtomicU64>,
    wake: Arc<std::sync::Mutex<Option<std::task::Waker>>>,
    publications: std::sync::mpsc::Receiver<(u64, f64)>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl HistoryHarness {
    fn new(product_id: u32) -> Self {
        let (input, history) = SpeedHistory::new(product_id).unwrap();
        let time = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let wake = Arc::new(std::sync::Mutex::new(None::<std::task::Waker>));
        let (output, publications) = std::sync::mpsc::channel();
        let timer_time = time.clone();
        let clock_time = time.clone();
        let output_time = time.clone();
        let timer_wake = wake.clone();
        let worker = std::thread::spawn(move || {
            let sampled = Arc::new(std::sync::atomic::AtomicU64::new(0));
            let sampled_clock = sampled.clone();
            tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap()
                .block_on(history.run_with(
                    |speed| {
                        output
                            .send((output_time.load(Ordering::Acquire), speed))
                            .unwrap();
                    },
                    move |delay| {
                        let due = Duration::from_millis(sampled.load(Ordering::Acquire)) + delay;
                        let time = timer_time.clone();
                        let wake = timer_wake.clone();
                        std::future::poll_fn(move |cx| {
                            let mut wake = wake.lock().unwrap();
                            if Duration::from_millis(time.load(Ordering::Acquire)) >= due {
                                std::task::Poll::Ready(())
                            } else {
                                *wake = Some(cx.waker().clone());
                                std::task::Poll::Pending
                            }
                        })
                    },
                    move || {
                        let time = clock_time.load(Ordering::Acquire);
                        sampled_clock.store(time, Ordering::Release);
                        clock(Duration::from_millis(time))
                    },
                ));
        });
        Self {
            input,
            time,
            wake,
            publications,
            worker: Some(worker),
        }
    }

    fn advance(&self, millis: u64) {
        self.time.store(millis, Ordering::Release);
        if let Some(wake) = self.wake.lock().unwrap().take() {
            wake.wake();
        }
    }

    fn publication(&self) -> (u64, f64) {
        self.publications
            .recv_timeout(Duration::from_secs(5))
            .expect("Scripted publication must complete without wall-clock sleeps")
    }
}
impl Drop for HistoryHarness {
    fn drop(&mut self) {
        self.input.close();
        let result = self.worker.take().unwrap().join();
        if !std::thread::panicking() {
            result.unwrap();
        }
    }
}

#[test]
fn mouse_dynamic_history_publishes_while_native_stop_waits_and_survives_restart() {
    struct BlockedStop {
        backend: Backend,
        entered: std::sync::mpsc::Sender<()>,
        resume: std::sync::mpsc::Receiver<()>,
    }
    impl Monitor for BlockedStop {
        fn start(&mut self) -> anyhow::Result<()> {
            self.backend.start()
        }
        fn events(&mut self) -> anyhow::Result<Vec<InputEvent>> {
            self.backend.events()
        }
        fn repeat_start(&mut self) -> anyhow::Result<StartAcknowledgement> {
            self.backend.repeat_start()
        }
        fn stop(
            &mut self,
            observe: &mut dyn FnMut(Vec<InputEvent>) -> anyhow::Result<()>,
        ) -> anyhow::Result<()> {
            self.entered.send(())?;
            self.resume.recv()?;
            self.backend.stop(observe)
        }
        fn shutdown(&mut self) -> anyhow::Result<()> {
            self.backend.shutdown()
        }
        fn terminate(&mut self) -> anyhow::Result<()> {
            self.backend.terminate()
        }
    }
    for product_id in [190, 192, 196, 222, 226, 229, 239] {
        let history = HistoryHarness::new(product_id);
        let input = history.input.clone();
        let callback_time = history.time.clone();
        let (entered, stopped) = std::sync::mpsc::channel();
        let (resume, blocked) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let mut backend = Backend::new();
            backend.batches.push_back(Ok(vec![
                InputEvent::from_native(
                    &format!(
                        r#"{{"containerId":"current","productId":{product_id},"vendorId":5426}}"#
                    ),
                    1,
                    "[]".into(),
                )
                .unwrap(),
                InputEvent::from_native(
                    &format!(
                        r#"{{"containerId":"current","productId":{product_id},"vendorId":5426}}"#
                    ),
                    1,
                    RESTART_MOTION.into(),
                )
                .unwrap(),
            ]));
            let mut backend = BlockedStop {
                backend,
                entered,
                resume: blocked,
            };
            let active = AtomicBool::new(true);
            let completion = capture_with(
                &mut backend,
                product_id,
                &active,
                &monitor_requests().1,
                |_| Ok(true),
                |json, admitted| {
                    if json != "[]" {
                        callback_time.store(16, Ordering::Release);
                    }
                    input.observe(json, admitted)
                },
                || input.stop_acknowledged(),
                || {},
                || active.store(false, Ordering::Release),
            );
            (completion, backend.backend.calls.into_inner())
        });
        stopped.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(history.publication(), (0, 0.));
        history.advance(17);
        let expected = if product_id == 226 { 5. } else { 10. };
        assert_eq!(history.publication(), (17, expected), "{product_id}");
        resume.send(()).unwrap();
        let (completion, calls) = worker.join().unwrap();
        assert!(completion.result.is_ok());
        assert!(completion.cleanup_failure.is_none());
        assert_eq!(calls, ["start", "poll", "stop", "shutdown", "terminate"]);

        history.advance(18);
        history.input.observe(RESTART_MOTION, true).unwrap();
        assert!(
            history.publications.try_recv().is_err(),
            "Restart cannot recreate a leading edge"
        );
        history.advance(35);
        assert_eq!(history.publication(), (35, expected), "{product_id}");
    }
}

#[test]
fn mouse_dynamic_history_resets_only_after_actual_stop_ack_and_keeps_pending_timer() {
    for product_id in [190, 192, 196, 222, 226, 229, 239] {
        for stop_failure in [false, true] {
            let history = HistoryHarness::new(product_id);
            history.input.observe("[]", true).unwrap();
            assert_eq!(history.publication(), (0, 0.));
            history.input.observe(RESTART_MOTION, true).unwrap();
            let mut backend = Backend::new();
            backend.start_failure = true;
            backend.stop_failure = stop_failure;
            let completion = capture_with(
                &mut backend,
                product_id,
                &AtomicBool::new(true),
                &monitor_requests().1,
                |_| Ok(true),
                |json, admitted| history.input.observe(json, admitted),
                || history.input.stop_acknowledged(),
                || panic!("Unacknowledged START cannot report monitoring"),
                || {},
            );
            assert!(completion.result.is_err());
            assert_eq!(completion.cleanup_failure.is_some(), stop_failure);
            assert_eq!(
                *backend.calls.borrow(),
                ["start", "stop", "shutdown", "terminate"]
            );
            history.advance(17);
            assert_eq!(
                history.publication(),
                (
                    17,
                    if !stop_failure {
                        0.
                    } else if product_id == 226 {
                        5.
                    } else {
                        10.
                    }
                )
            );
        }
    }
}

#[test]
fn mouse_dynamic_history_cancel_before_start_does_not_invent_a_stop_reset() {
    let history = HistoryHarness::new(190);
    history.input.observe("[]", true).unwrap();
    assert_eq!(history.publication(), (0, 0.));
    history.input.observe(RESTART_MOTION, true).unwrap();
    let completion = capture_with(
        &mut Backend::new(),
        190,
        &AtomicBool::new(false),
        &monitor_requests().1,
        |_| Ok(true),
        |_, _| panic!("Canceled capture cannot observe input"),
        || panic!("An unattempted START has no native STOP acknowledgement"),
        || panic!("Canceled capture cannot start"),
        || {},
    );
    assert!(completion.result.is_err());
    assert!(completion.cleanup_failure.is_none());
    history.advance(17);
    assert_eq!(history.publication(), (17, 10.));
}

#[test]
fn mouse_dynamic_history_callback_failure_returns_to_capture_and_retains_cleanup() {
    let history = HistoryHarness::new(190);
    let mut backend = Backend::new();
    backend.batches.push_back(Ok(vec![
        InputEvent::from_native(
            r#"{"containerId":"current","productId":190,"vendorId":5426}"#,
            1,
            r#"[{"data":"invalid"}]"#.into(),
        )
        .unwrap(),
    ]));
    let completion = capture_with(
        &mut backend,
        190,
        &AtomicBool::new(true),
        &monitor_requests().1,
        |_| Ok(true),
        |json, admitted| history.input.observe(json, admitted),
        || history.input.stop_acknowledged(),
        || {},
        || {},
    );
    assert!(completion.result.is_err());
    assert!(completion.cleanup_failure.is_none());
    assert_eq!(
        *backend.calls.borrow(),
        ["start", "poll", "stop", "shutdown", "terminate"]
    );
    assert!(history.publications.try_recv().is_err());
    history.input.observe(RESTART_MOTION, true).unwrap();
    assert_eq!(history.publication(), (0, 10.));
}

#[test]
fn mouse_dynamic_history_disposal_rejects_late_callbacks_and_releases_pending_timer() {
    let history = HistoryHarness::new(190);
    history.input.observe("[]", true).unwrap();
    assert_eq!(history.publication(), (0, 0.));
    history.input.observe(RESTART_MOTION, true).unwrap();
    history.input.close();
    history.advance(17);
    assert!(history.input.observe(RESTART_MOTION, true).is_err());
    assert!(history.input.stop_acknowledged().is_ok());
    // Drop joins the owner even though a trailing timer was still pending.
    let output = history.publications.try_recv();
    assert!(output.is_err());

    let (input, owner) = SpeedHistory::new(190).unwrap();
    drop(owner);
    assert!(input.observe("[]", true).is_err());
    assert!(input.stop_acknowledged().is_ok());
}

#[test]
fn mouse_dynamic_history_tracks_pid226_observers_without_releasing_the_connection_owner() {
    let (input, owner) = SpeedHistory::new(226).unwrap();
    assert_eq!(input.observer_count(), 1);
    input.register_observer().unwrap();
    assert_eq!(input.observer_count(), 2);
    input.unregister_observer();
    assert_eq!(input.observer_count(), 1);
    input.unregister_observer();
    input.unregister_observer();
    assert_eq!(input.observer_count(), 0);
    // A stopped source can retain its history with no listeners. The next
    // mount registers again without inventing a callback for the idle gap.
    input.observe("[]", true).unwrap();
    input.register_observer().unwrap();
    assert_eq!(input.observer_count(), 1);
    drop(owner);
}

fn received_start(reply: StartReply) -> StartResult {
    assert!(
        !reply.completed.is_empty() || reply.completed.is_closed(),
        "Worker must settle every accepted START"
    );
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(reply.receive())
}

#[test]
fn mouse_dynamic_repeated_start_uses_retained_worker_and_keeps_negative_ack() {
    let (control, requests) = monitor_requests();
    let reply = control.request_start().unwrap();
    let mut backend = Backend::new();
    backend
        .repeat_results
        .push_back(Ok(StartAcknowledgement::rejected(
            "current".into(),
            "already started",
        )));
    backend.batches.push_back(Ok(vec![batch("current", 226)]));
    let active = AtomicBool::new(true);
    let callbacks = RefCell::new(Vec::new());
    let completion = capture_with(
        &mut backend,
        226,
        &active,
        &requests,
        |_| Ok(true),
        |_, admitted| {
            assert!(admitted);
            callbacks.borrow_mut().push("input");
            Ok(())
        },
        || {
            callbacks.borrow_mut().push("reset");
            Ok(())
        },
        || callbacks.borrow_mut().push("started"),
        || active.store(false, Ordering::Release),
    );
    let acknowledgement = received_start(reply).unwrap();
    assert!(!acknowledgement.result());
    assert_eq!(acknowledgement.reason(), "already started");
    assert!(completion.result.is_ok());
    assert!(completion.cleanup_failure.is_none());
    assert_eq!(
        *backend.calls.borrow(),
        [
            "start",
            "repeat start",
            "poll",
            "stop",
            "shutdown",
            "terminate"
        ]
    );
    assert_eq!(*callbacks.borrow(), ["started", "input", "reset"]);
    assert!(control.request_start().is_err());
}

#[test]
fn mouse_dynamic_accepted_repeat_precedes_stop_when_canceled_during_poll() {
    let (control, requests) = monitor_requests();
    let reply = RefCell::new(None);
    let active = AtomicBool::new(true);
    let mut backend = Backend::new();
    backend
        .repeat_results
        .push_back(Ok(StartAcknowledgement::rejected(
            "current".into(),
            "already started",
        )));
    let completion = capture_with(
        &mut backend,
        226,
        &active,
        &requests,
        |_| Ok(true),
        |_, _| Ok(()),
        || Ok(()),
        || {},
        || {
            *reply.borrow_mut() = Some(control.request_start().unwrap());
            active.store(false, Ordering::Release);
        },
    );
    assert!(
        !received_start(reply.into_inner().unwrap())
            .unwrap()
            .result()
    );
    assert!(completion.result.is_ok());
    assert!(completion.cleanup_failure.is_none());
    assert_eq!(
        *backend.calls.borrow(),
        [
            "start",
            "poll",
            "repeat start",
            "stop",
            "shutdown",
            "terminate"
        ]
    );
}

#[test]
fn mouse_dynamic_partial_or_canceled_initial_start_settles_queued_repeats() {
    for canceled in [false, true] {
        let (control, requests) = monitor_requests();
        let replies = [
            control.request_start().unwrap(),
            control.request_start().unwrap(),
        ];
        let mut backend = Backend::new();
        backend.start_failure = true;
        let completion = capture_with(
            &mut backend,
            226,
            &AtomicBool::new(!canceled),
            &requests,
            |_| panic!("No input before START acknowledgement"),
            |_, _| unreachable!(),
            || Ok(()),
            || panic!("Initial START was never acknowledged"),
            || {},
        );
        assert!(completion.result.is_err());
        for reply in replies {
            assert!(received_start(reply).is_err());
        }
        let expected = if canceled {
            vec!["stop", "shutdown", "terminate"]
        } else {
            vec!["start", "stop", "shutdown", "terminate"]
        };
        assert_eq!(*backend.calls.borrow(), expected);
        assert!(control.request_start().is_err());
    }
}

#[test]
fn mouse_dynamic_repeat_failure_does_not_stop_existing_capture_or_abandon_later_requests() {
    let (control, requests) = monitor_requests();
    let failed = control.request_start().unwrap();
    let later = control.request_start().unwrap();
    let mut backend = Backend::new();
    backend
        .repeat_results
        .push_back(Err("lost repeat START acknowledgement".into()));
    backend
        .repeat_results
        .push_back(Ok(StartAcknowledgement::rejected(
            "current".into(),
            "already started",
        )));
    let active = AtomicBool::new(true);
    let completion = capture_with(
        &mut backend,
        226,
        &active,
        &requests,
        |_| Ok(true),
        |_, _| Ok(()),
        || Ok(()),
        || {},
        || active.store(false, Ordering::Release),
    );
    assert!(
        received_start(failed)
            .unwrap_err()
            .contains("lost repeat START")
    );
    assert!(!received_start(later).unwrap().result());
    assert!(completion.result.is_ok());
    assert!(completion.cleanup_failure.is_none());
    assert_eq!(
        *backend.calls.borrow(),
        [
            "start",
            "repeat start",
            "repeat start",
            "poll",
            "stop",
            "shutdown",
            "terminate"
        ]
    );
}

#[test]
fn mouse_dynamic_start_queue_is_bounded_and_owner_drop_settles_every_reply() {
    let (control, requests) = monitor_requests();
    let replies = (0..MAX_PENDING_STARTS)
        .map(|_| control.request_start().unwrap())
        .collect::<Vec<_>>();
    let error = control.request_start().err().unwrap();
    assert!(error.to_string().contains("queue is full"));
    drop(requests);
    assert!(control.request_start().is_err());
    for reply in replies {
        assert!(
            received_start(reply)
                .unwrap_err()
                .contains("ended before START")
        );
    }
    let (successor, requests) = monitor_requests();
    let reply = successor.request_start().unwrap();
    drop(requests);
    assert!(received_start(reply).is_err());
}

#[test]
fn mouse_dynamic_remaining_pid226_observer_consumes_poll_and_final_stop_before_reset() {
    let history = HistoryHarness::new(226);
    history.input.register_observer().unwrap();
    history.input.unregister_observer();
    assert_eq!(history.input.observer_count(), 1);
    let active = Arc::new(AtomicBool::new(true));
    let mut backend = Backend::new();
    backend.cancel_on_poll = Some(active.clone());
    backend.batches.push_back(Ok(vec![
        InputEvent::from_native(
            r#"{"containerId":"current","productId":226,"vendorId":5426}"#,
            1,
            RESTART_MOTION.into(),
        )
        .unwrap(),
    ]));
    backend.final_events.push(
        InputEvent::from_native(
            r#"{"containerId":"current","productId":226,"vendorId":5426}"#,
            1,
            r#"[{"timeTick":"30000","data":"{\"type\":\"mouseMove\",\"x\":9,\"y\":12}"},{"timeTick":"40000","data":"{\"type\":\"mouseMove\",\"x\":12,\"y\":16}"}]"#.into(),
        )
        .unwrap(),
    );
    let observations = Cell::new(0);
    let completion = capture_with(
        &mut backend,
        226,
        &active,
        &monitor_requests().1,
        |_| Ok(true),
        |json, admitted| {
            let count = observations.get();
            if count == 1 {
                history.advance(18);
            }
            history.input.observe(json, admitted)?;
            assert_eq!(history.publication(), (if count == 0 { 0 } else { 18 }, 5.));
            observations.set(count + 1);
            Ok(())
        },
        || {
            assert_eq!(observations.get(), 2);
            history.input.stop_acknowledged()
        },
        || {},
        || {},
    );
    assert!(completion.result.is_ok());
    assert!(completion.cleanup_failure.is_none());
    assert_eq!(observations.get(), 2);
    assert_eq!(
        *backend.calls.borrow(),
        ["start", "poll", "stop", "shutdown", "terminate"]
    );
    assert_eq!(history.input.observer_count(), 1);
}

#[test]
fn mouse_dynamic_final_callback_failure_keeps_cleanup_failure_visible() {
    let mut backend = Backend::new();
    backend.final_events.push(batch("current", 226));
    let active = AtomicBool::new(true);
    let completion = capture_with(
        &mut backend,
        226,
        &active,
        &monitor_requests().1,
        |_| Ok(true),
        |_, _| anyhow::bail!("remaining listener rejected final callback"),
        || panic!("A failed final handoff cannot masquerade as reset completion"),
        || {},
        || active.store(false, Ordering::Release),
    );
    assert!(
        completion
            .cleanup_failure
            .unwrap()
            .contains("remaining listener rejected final callback")
    );
    assert_eq!(
        *backend.calls.borrow(),
        ["start", "poll", "stop", "shutdown", "terminate"]
    );
}
