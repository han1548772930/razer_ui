use super::*;
use razer_device::{
    mapping_native_action::parse_output,
    mapping_native_analog::{GamepadAxis, GamepadCurve, GamepadPoint, NativeAnalogOutput},
    mapping_native_events::NativeModifierSnapshot,
    mapping_native_lookup::{CompiledMapping, ResolvedOutput, Selectors},
    mapping_native_mode::DeviceInfo,
};
use serde_json::{Value, json};

#[derive(Debug)]
enum Recorded {
    Begin(bool),
    Analog(Resolved<Output>, u16),
    Action(Resolved<Output>),
    Finish,
    Interrupted(Resolved<Output>),
    Pause,
}
#[derive(Default)]
struct SimExecutor {
    gamepad_observation: super::super::gamepad_observation::Observation,
    recorded: Vec<Recorded>,
    completion_error: bool,
    interruption_error: bool,
    action_error: bool,
    execution_contexts: Vec<(u64, bool)>,
    closed: bool,
}
impl Executor for SimExecutor {
    fn next_wait(&self) -> Duration {
        Duration::from_millis(4)
    }
    fn gamepad_observation(
        &mut self,
        control: super::super::gamepad_observation::Control,
        sink: super::super::gamepad_observation::Sink,
    ) -> Result<()> {
        self.gamepad_observation.control(control, sink)
    }
    fn pause_mapping_replay(&mut self) -> Result<()> {
        self.recorded.push(Recorded::Pause);
        Ok(())
    }
    fn interrupt_macro(&mut self, resolved: &Resolved<Output>) -> Result<()> {
        self.recorded.push(Recorded::Interrupted(resolved.clone()));
        ensure!(!self.interruption_error, "simulated interruption failure");
        Ok(())
    }
    fn begin_hardware_frame(&mut self, cleanup: bool) -> Result<()> {
        self.recorded.push(Recorded::Begin(cleanup));
        Ok(())
    }
    fn finish_hardware_frame(&mut self) -> Result<()> {
        self.recorded.push(Recorded::Finish);
        ensure!(
            !std::mem::take(&mut self.completion_error),
            "simulated completion failure"
        );
        Ok(())
    }
    fn analog(&mut self, resolved: Resolved<Output>, actuation: u16) -> Result<()> {
        self.recorded.push(Recorded::Analog(resolved, actuation));
        Ok(())
    }
    fn execute(
        &mut self,
        resolved: Resolved<Output>,
        tick: u64,
        cleanup: bool,
        _: Option<String>,
    ) -> Result<()> {
        self.execution_contexts.push((tick, cleanup));
        self.recorded.push(Recorded::Action(resolved));
        ensure!(
            !std::mem::take(&mut self.action_error),
            "simulated action failure"
        );
        Ok(())
    }
    fn prepare_redirect(&mut self) -> Result<()> {
        Ok(())
    }
    fn redirect_activated(&mut self) -> Result<()> {
        Ok(())
    }
    fn poll(&mut self) -> Result<()> {
        Ok(())
    }
    fn close(&mut self) -> Result<()> {
        self.closed = true;
        Ok(())
    }
}

fn document(make: u16, release: u16) -> Value {
    json!({"activeProfile":"test","profiles":[{"guid":"test","appEngine":{"mappings":[]}}],
        "reportIDs":{"11":"analogKeyboardReportID","12":"controlPodReportID","13":"razerKeyReportID"},
        "analogData":{"defaultMakeActuation":make,"defaultBreakActuation":release}})
}
fn keyboard_output(scancode: u16, flag: u16) -> Output {
    parse_output(&json!({"type":"keyboard","scancode":scancode,"flag":flag})).unwrap()
}
fn rows(output: u16) -> Tables<Output> {
    let mappings = [30, 31]
        .into_iter()
        .flat_map(|scancode| {
            [0, 1].into_iter().map(move |flag| CompiledMapping {
                input: Input::new(InputKind::AnalogKey { scancode, flag }),
                selectors: Selectors::default(),
                output: keyboard_output(output, flag),
            })
        })
        .collect();
    Tables::new(vec![], mappings, vec![]).unwrap()
}
fn worker(tables: Tables<Output>, doc: Value) -> (Worker<SimExecutor>, mpsc::Receiver<Event>) {
    let caller = Uuid::from_u128(1);
    let mut modes = ModeState::default();
    modes
        .register_device(
            caller,
            DeviceInfo {
                guid: "hardware-test".into(),
                vendor_id: 0x1532,
                product_id: 679,
                edition_id: 0,
                manufacturer_name: String::new(),
                product_name: String::new(),
                serial_number: String::new(),
                container_id: "test-container".into(),
            },
        )
        .unwrap();
    let (events, receiver) = mpsc::sync_channel(512);
    (
        Worker {
            input_observed: true,
            input_observer: None,
            snaptap_observer: None,
            analog_observation: Default::default(),
            lookup: Lookup::new(tables),
            composites: CompositeState::default(),
            hardware: Hardware {
                config: Some(NativeHidDeviceConfig::from_document(&doc).unwrap()),
                ..Default::default()
            },
            snaptap: Default::default(),
            dynamic_keys: Default::default(),
            began: Instant::now(),
            route: driver::Route::disconnected_for_test(),
            executor: SimExecutor::default(),
            caller,
            modes: Arc::new(Mutex::new(modes)),
            mapping_enabled: true,
            shortcuts_enabled: true,
            events,
            overflow: Arc::new(AtomicBool::new(false)),
        },
        receiver,
    )
}
fn packet(bytes: &[u8]) -> hardware::Packet {
    hardware::Packet {
        endpoint: hardware::EndpointIdentity {
            path: "sim-hid".into(),
            instance: "sim-instance".into(),
            generation: 1,
        },
        input: driver::Packet {
            bytes: bytes.into(),
            modifiers: NativeModifierSnapshot::default(),
            time_tick: 123,
        },
    }
}
fn actions(worker: &Worker<SimExecutor>) -> Vec<ResolvedOutput<Output>> {
    worker
        .executor
        .recorded
        .iter()
        .filter_map(|record| match record {
            Recorded::Action(value) => Some(value.output.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn hardware_batches_all_analog_outputs_between_one_begin_and_finish() {
    let curve = GamepadCurve::compile(
        AnalogResolution::V1,
        GamepadAxis::LeftTrigger,
        &[
            GamepadPoint {
                actuation: 0,
                value: 0,
            },
            GamepadPoint {
                actuation: 255,
                value: 255,
            },
        ],
    )
    .unwrap();
    let mappings = [30, 31]
        .into_iter()
        .flat_map(|scancode| {
            [0, 1].into_iter().map({
                let curve = curve.clone();
                move |flag| CompiledMapping {
                    input: Input::new(InputKind::AnalogKey { scancode, flag }),
                    selectors: Selectors::default(),
                    output: Output::Analog(NativeAnalogOutput::Gamepad(curve.clone())),
                }
            })
        })
        .collect();
    let (mut worker, _events) = worker(
        Tables::new(vec![], mappings, vec![]).unwrap(),
        document(100, 80),
    );
    worker
        .hardware_input(packet(&[11, 31, 120, 32, 130]))
        .unwrap();
    assert_eq!(worker.executor.recorded.len(), 4);
    assert!(matches!(
        worker.executor.recorded[0],
        Recorded::Begin(false)
    ));
    let levels: Vec<_> = worker
        .executor
        .recorded
        .iter()
        .filter_map(|record| match record {
            Recorded::Analog(resolved, level) => {
                assert!(matches!(
                    resolved.output,
                    ResolvedOutput::Mapped(Output::Analog(_))
                ));
                Some(*level)
            }
            _ => None,
        })
        .collect();
    assert_eq!(levels, vec![130, 120]);
    assert!(matches!(worker.executor.recorded[3], Recorded::Finish));
}

#[test]
fn held_mapping_survives_publication_while_thresholds_are_live() {
    let (mut worker, _events) = worker(rows(40), document(100, 80));
    worker.hardware_input(packet(&[11, 31, 120])).unwrap();
    assert_eq!(
        actions(&worker),
        vec![ResolvedOutput::Mapped(keyboard_output(40, 0))]
    );
    worker.lookup.replace_tables(rows(50));
    worker.hardware.config =
        Some(NativeHidDeviceConfig::from_document(&document(110, 100)).unwrap());
    worker.hardware_input(packet(&[11, 31, 90])).unwrap();
    assert_eq!(
        actions(&worker)[1],
        ResolvedOutput::Mapped(keyboard_output(40, 1))
    );
    worker.hardware_input(packet(&[11, 31, 0])).unwrap();
    assert!(worker.hardware.inputs.contains_key(&31));
    worker.hardware_input(packet(&[11, 31, 120])).unwrap();
    assert_eq!(
        actions(&worker)[2],
        ResolvedOutput::Mapped(keyboard_output(40, 0))
    );
    worker.hardware_input(packet(&[11, 0, 0])).unwrap();
    assert!(!worker.hardware.inputs.contains_key(&31));
    worker.hardware_input(packet(&[11, 31, 120])).unwrap();
    assert_eq!(
        actions(&worker).last(),
        Some(&ResolvedOutput::Mapped(keyboard_output(50, 0)))
    );
}

#[test]
fn completion_failure_is_reported_without_replaying_a_physical_press() {
    let (mut worker, _events) = worker(rows(40), document(100, 80));
    worker.executor.completion_error = true;
    let error = worker.hardware_input(packet(&[11, 31, 120])).unwrap_err();
    assert!(format!("{error:#}").contains("simulated completion failure"));
    assert_eq!(worker.hardware.ingress.tracked_level(31), Some(120));
    worker.hardware_input(packet(&[11, 31, 120])).unwrap();
    assert_eq!(actions(&worker).len(), 1);
    worker.hardware_input(packet(&[11, 0, 0])).unwrap();
    assert_eq!(
        actions(&worker)[1],
        ResolvedOutput::Mapped(keyboard_output(40, 1))
    );
}

#[test]
fn disabled_mapping_still_releases_a_missing_hardware_key() {
    let (mut worker, _events) = worker(rows(40), document(100, 80));
    worker.hardware_input(packet(&[11, 31, 120])).unwrap();
    worker.mapping_enabled = false;
    worker.hardware_input(packet(&[11, 0, 0])).unwrap();
    assert_eq!(
        actions(&worker),
        vec![
            ResolvedOutput::Mapped(keyboard_output(40, 0)),
            ResolvedOutput::Mapped(keyboard_output(40, 1))
        ]
    );
    assert!(worker.hardware.inputs.is_empty());
}

#[test]
fn control_pod_notifies_once_before_repeating_the_resolved_action() {
    let mappings = vec![CompiledMapping {
        input: Input::new(InputKind::ControlPodDial { clockwise: true }),
        selectors: Selectors::default(),
        output: keyboard_output(42, 0),
    }];
    let (mut worker, events) = worker(
        Tables::new(vec![], mappings, vec![]).unwrap(),
        document(100, 80),
    );
    worker.hardware_input(packet(&[12, 0, 0, 3])).unwrap();
    assert_eq!(actions(&worker).len(), 3);
    let inputs = events
        .try_iter()
        .filter_map(|event| match event {
            Event::Input { input, .. } => Some(input.kind),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(inputs, vec![InputKind::ControlPodDial { clockwise: true }]);
}

#[test]
fn stale_endpoint_generation_does_not_release_current_input() {
    let (mut worker, _events) = worker(rows(40), document(100, 80));
    let mut newest = packet(&[11, 31, 120]);
    newest.endpoint.generation = 2;
    worker.hardware_input(newest).unwrap();
    worker.hardware_input(packet(&[11, 0, 0])).unwrap();
    assert_eq!(worker.hardware.ingress.tracked_level(31), Some(120));
    assert_eq!(actions(&worker).len(), 1);
}

#[test]
fn empty_interrupt_list_releases_once_and_suppresses_until_physical_absence() {
    let mappings = [0, 1]
        .into_iter()
        .map(|flag| {
            let mut input = Input::new(InputKind::AnalogKey { scancode: 30, flag });
            input.interruptible = true;
            CompiledMapping {
                input,
                selectors: Selectors::default(),
                output: keyboard_output(40, flag),
            }
        })
        .collect();
    let (mut worker, _events) = worker(
        Tables::new(vec![], mappings, vec![]).unwrap(),
        document(100, 80),
    );
    worker.hardware_input(packet(&[11, 31, 120])).unwrap();
    worker
        .interrupt_hardware(
            &Input::new(InputKind::Keyboard {
                scancode: 45,
                flag: 0,
            }),
            125,
            false,
        )
        .unwrap();
    assert_eq!(
        actions(&worker),
        vec![
            ResolvedOutput::Mapped(keyboard_output(40, 0)),
            ResolvedOutput::Mapped(keyboard_output(40, 1))
        ]
    );
    assert!(worker.hardware.interrupts.blocked_keys().contains(&31));
    worker.hardware_input(packet(&[11, 31, 0])).unwrap();
    worker.hardware_input(packet(&[11, 31, 120])).unwrap();
    assert_eq!(actions(&worker).len(), 2);
    assert!(worker.hardware.interrupts.blocked_keys().contains(&31));
    worker.hardware_input(packet(&[11, 0, 0])).unwrap();
    assert!(!worker.hardware.interrupts.blocked_keys().contains(&31));
    worker.hardware_input(packet(&[11, 31, 120])).unwrap();
    assert_eq!(
        actions(&worker).last(),
        Some(&ResolvedOutput::Mapped(keyboard_output(40, 0)))
    );
    assert_eq!(actions(&worker).len(), 3);
}

#[test]
fn interrupt_cancels_the_original_macro_mapping_after_sending_its_release() {
    let mappings = [0u16, 1]
        .into_iter()
        .map(|flag| {
            let mut input = Input::new(InputKind::AnalogKey { scancode: 30, flag });
            input.interruptible = true;
            CompiledMapping {
                input,
                selectors: Selectors::default(),
                output: Output::Macro {
                    guid: "original-macro".into(),
                    flag: flag.into(),
                    repeat: 2,
                },
            }
        })
        .collect();
    let (mut worker, _events) = worker(
        Tables::new(vec![], mappings, vec![]).unwrap(),
        document(100, 80),
    );
    worker.hardware_input(packet(&[11, 31, 120])).unwrap();
    worker.executor.recorded.clear();
    let other = Input::new(InputKind::Keyboard {
        scancode: 45,
        flag: 0,
    });
    worker.interrupt_hardware(&other, 125, false).unwrap();
    assert!(matches!(&worker.executor.recorded[..], [
        Recorded::Begin(false),
        Recorded::Action(release),
        Recorded::Interrupted(make),
        Recorded::Finish,
    ] if release.output == ResolvedOutput::Mapped(Output::Macro { guid:"original-macro".into(), flag:1, repeat:2 })
        && make.output == ResolvedOutput::Mapped(Output::Macro { guid:"original-macro".into(), flag:0, repeat:2 })
        && make.input.phase() == Phase::Press));
    // The interrupt registration outlives the lookup pair until physical absence.
    worker.executor.recorded.clear();
    worker.interrupt_hardware(&other, 126, false).unwrap();
    assert!(worker.executor.recorded.iter().any(|item| matches!(item,
    Recorded::Interrupted(make) if make.output == ResolvedOutput::Mapped(Output::Macro {
        guid:"original-macro".into(), flag:0, repeat:2
    }))));

    worker.hardware_input(packet(&[11, 0, 0])).unwrap();
    worker.executor.recorded.clear();
    worker.interrupt_hardware(&other, 127, false).unwrap();
    assert!(matches!(&worker.executor.recorded[..], [
        Recorded::Begin(false), Recorded::Interrupted(make), Recorded::Finish,
    ] if make.output == ResolvedOutput::Mapped(Output::Macro {
        guid:"original-macro".into(), flag:0, repeat:2
    })));
    assert!(!worker.hardware.interrupts.blocked_keys().contains(&31));

    worker
        .hardware
        .replace_config(NativeHidDeviceConfig::from_document(&document(100, 80)).unwrap());
    worker.executor.recorded.clear();
    worker.interrupt_hardware(&other, 128, false).unwrap();
    assert!(worker.executor.recorded.is_empty());
}

#[test]
fn failed_macro_interruption_does_not_discard_the_current_control_pod_action() {
    let mut mappings = [0u16, 1]
        .into_iter()
        .map(|flag| {
            let mut input = Input::new(InputKind::AnalogKey { scancode: 30, flag });
            input.interruptible = true;
            CompiledMapping {
                input,
                selectors: Selectors::default(),
                output: Output::Macro {
                    guid: "original-macro".into(),
                    flag: flag.into(),
                    repeat: 2,
                },
            }
        })
        .collect::<Vec<_>>();
    mappings.push(CompiledMapping {
        input: Input::new(InputKind::ControlPodDial { clockwise: true }),
        selectors: Selectors::default(),
        output: keyboard_output(45, 0),
    });
    let (mut worker, events) = worker(
        Tables::new(vec![], mappings, vec![]).unwrap(),
        document(100, 80),
    );
    worker.hardware_input(packet(&[11, 31, 120])).unwrap();
    worker.executor.recorded.clear();
    worker.executor.interruption_error = true;

    let error = worker.hardware_input(packet(&[12, 0, 0, 3])).unwrap_err();
    assert!(format!("{error:#}").contains("simulated interruption failure"));
    assert!(matches!(&worker.executor.recorded[..], [
        Recorded::Begin(false),
        Recorded::Action(release),
        Recorded::Interrupted(make),
        Recorded::Action(first),
        Recorded::Action(second),
        Recorded::Action(third),
        Recorded::Finish,
    ] if release.output == ResolvedOutput::Mapped(Output::Macro { guid:"original-macro".into(), flag:1, repeat:2 })
        && make.output == ResolvedOutput::Mapped(Output::Macro { guid:"original-macro".into(), flag:0, repeat:2 })
        && first.output == ResolvedOutput::Mapped(keyboard_output(45, 0))
        && second.output == first.output && third.output == first.output));
    let inputs = events
        .try_iter()
        .filter_map(|event| match event {
            Event::Input { input, .. } => Some(input.kind),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        inputs,
        vec![
            InputKind::AnalogKey {
                scancode: 30,
                flag: 0
            },
            InputKind::ControlPodDial { clockwise: true },
            InputKind::AnalogKey {
                scancode: 30,
                flag: 1
            },
        ]
    );
    assert!(worker.hardware.interrupts.blocked_keys().contains(&31));
}

#[test]
fn digital_interruption_retains_the_macro_registration_beyond_physical_release() {
    let mappings = [0u16, 1]
        .into_iter()
        .map(|flag| {
            let mut input = Input::new(InputKind::Keyboard { scancode: 30, flag });
            input.interruptible = true;
            CompiledMapping {
                input,
                selectors: Selectors::default(),
                output: Output::Macro {
                    guid: "digital-macro".into(),
                    flag: flag.into(),
                    repeat: 2,
                },
            }
        })
        .collect();
    let (mut worker, _) = worker(
        Tables::new(vec![], mappings, vec![]).unwrap(),
        document(100, 80),
    );
    let modes = worker.current_modes().unwrap();
    let key = Input::new(InputKind::Keyboard {
        scancode: 30,
        flag: 0,
    });
    worker
        .resolve_digital(key.clone(), modes, 123, 1, false)
        .unwrap();
    worker.executor.recorded.clear();
    let other = Input::new(InputKind::Keyboard {
        scancode: 45,
        flag: 0,
    });
    worker.notify_input(&other, 124, false).unwrap();
    assert!(
        matches!(&worker.executor.recorded[..], [Recorded::Action(release), Recorded::Interrupted(make)]
        if release.input.phase() == Phase::Release && make.input.phase() == Phase::Press)
    );
    assert_eq!(worker.lookup.held_count(), 1);
    worker.executor.recorded.clear();
    worker.notify_input(&other, 125, false).unwrap();
    assert!(matches!(
        &worker.executor.recorded[..],
        [Recorded::Action(_), Recorded::Interrupted(_)]
    ));
    worker.executor.recorded.clear();
    worker
        .resolve_digital(key.released(), modes, 126, 1, false)
        .unwrap();
    assert!(worker.executor.recorded.is_empty());
    worker.notify_input(&other, 127, false).unwrap();
    assert!(
        matches!(&worker.executor.recorded[..], [Recorded::Interrupted(make)]
        if make.output == ResolvedOutput::Mapped(Output::Macro {guid:"digital-macro".into(),flag:0,repeat:2}))
    );
}

#[test]
fn digital_snaptap_restores_the_retained_mapping_and_swallows_inactive_physical_release() {
    let mappings = [30u16, 31, 32]
        .into_iter()
        .flat_map(|scancode| {
            [0, 1].into_iter().map(move |flag| CompiledMapping {
                input: Input::new(InputKind::Keyboard { scancode, flag }),
                selectors: Selectors {
                    snap_tap: Some(7),
                    ..Default::default()
                },
                output: keyboard_output(scancode + 10, flag),
            })
        })
        .collect();
    let (mut worker, events) = worker(
        Tables::new(vec![], mappings, vec![]).unwrap(),
        document(100, 80),
    );
    let modes = worker.current_modes().unwrap();
    for (scancode, flag) in [(30, 0), (31, 0), (32, 0), (31, 1), (32, 1), (30, 1)] {
        worker
            .resolve_digital(
                Input::new(InputKind::Keyboard { scancode, flag }),
                modes,
                123,
                1,
                false,
            )
            .unwrap();
    }
    assert_eq!(
        actions(&worker),
        [
            (40, 0),
            (40, 1),
            (41, 0),
            (41, 1),
            (42, 0),
            (42, 1),
            (40, 0),
            (40, 1)
        ]
        .into_iter()
        .map(|(scancode, flag)| ResolvedOutput::Mapped(keyboard_output(scancode, flag)))
        .collect::<Vec<_>>()
    );
    assert_eq!(
        worker
            .executor
            .recorded
            .iter()
            .filter(|item| matches!(item, Recorded::Pause))
            .count(),
        3
    );
    assert_eq!(
        events
            .try_iter()
            .filter(|event| matches!(event, Event::Input { .. }))
            .count(),
        3
    );
    assert_eq!(worker.lookup.held_count(), 0);
}

fn digital_key(scancode: u16) -> Input {
    Input::new(InputKind::Keyboard { scancode, flag: 0 })
}

#[test]
fn input_observation_shares_otfm_gate_and_preserves_subscription_generation() {
    let (mut worker, events) = worker(rows(40), document(100, 80));
    let input = digital_key(30);
    worker.input_observed = false;
    worker.notify_input(&input, 1, false).unwrap();
    assert!(events.try_iter().next().is_none());
    let first = Uuid::from_u128(31);
    let second = Uuid::from_u128(32);
    worker.input_observer = Some(first);
    worker.notify_input(&input, 2, false).unwrap();
    worker.input_observer = Some(second);
    worker.notify_input(&input, 3, false).unwrap();
    worker.input_observed = true;
    worker.input_observer = None;
    worker.notify_input(&input, 4, false).unwrap();
    assert!(worker.mapping_enabled && worker.shortcuts_enabled);
    let observed = events
        .try_iter()
        .map(|event| match event {
            Event::Input {
                observer,
                tick,
                redirected: false,
                ..
            } => (observer, tick),
            _ => panic!("Unexpected input observation event"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        observed.iter().map(|item| item.0).collect::<Vec<_>>(),
        [Some(first), Some(second), None]
    );
    assert!(observed.windows(2).all(|pair| pair[0].1 <= pair[1].1));
}

#[test]
fn input_observation_replacement_preserves_physical_analog_deduplication() {
    let (mut worker, events) = worker(rows(40), document(100, 80));
    worker.input_observed = false;
    let first = Uuid::from_u128(41);
    let second = Uuid::from_u128(42);
    worker.input_observer = Some(first);
    worker.hardware_input(packet(&[11, 31, 120])).unwrap();
    let first_events = events
        .try_iter()
        .filter_map(|event| match event {
            Event::Input { observer, .. } => Some(observer),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(first_events, [Some(first)]);
    worker.input_observer = None;
    worker.input_observer = Some(second);
    worker.hardware_input(packet(&[11, 31, 130])).unwrap();
    assert!(
        !events
            .try_iter()
            .any(|event| matches!(event, Event::Input { .. }))
    );
    worker.hardware_input(packet(&[11])).unwrap();
    let released = events
        .try_iter()
        .filter_map(|event| match event {
            Event::Input {
                observer, input, ..
            } => Some((observer, input.kind)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        released,
        [(
            Some(second),
            InputKind::AnalogKey {
                scancode: 30,
                flag: 1
            }
        )]
    );
}

fn observed_digital_snaptap_worker() -> (Worker<SimExecutor>, mpsc::Receiver<Event>) {
    let mappings = [30u16, 31]
        .into_iter()
        .flat_map(|scancode| {
            [0, 1].into_iter().map(move |flag| CompiledMapping {
                input: Input::new(InputKind::Keyboard { scancode, flag }),
                selectors: Selectors {
                    snap_tap: Some(7),
                    ..Default::default()
                },
                output: keyboard_output(scancode + 10, flag),
            })
        })
        .collect();
    let (mut worker, events) = worker(
        Tables::new(vec![], mappings, vec![]).unwrap(),
        document(100, 80),
    );
    worker.snaptap_observer = Some(Uuid::from_u128(44));
    (worker, events)
}

fn snaptap_observations(events: &mpsc::Receiver<Event>) -> Vec<(Uuid, Input, bool)> {
    events
        .try_iter()
        .filter_map(|event| match event {
            Event::SnapTap {
                observer,
                observation,
            } => Some((
                observer,
                observation.input().clone(),
                observation.replayed(),
            )),
            _ => None,
        })
        .collect()
}

#[test]
fn snaptap_empty_current_callback_still_observes_the_inactive_physical_release() {
    let (mut worker, events) = observed_digital_snaptap_worker();
    press_digital(&mut worker, 30);
    press_digital(&mut worker, 31);
    let _ = snaptap_observations(&events);
    worker.executor.recorded.clear();
    worker
        .resolve_digital(
            digital_key(30).released(),
            worker.current_modes().unwrap(),
            124,
            1,
            false,
        )
        .unwrap();
    assert!(actions(&worker).is_empty());
    assert_eq!(
        snaptap_observations(&events),
        [(Uuid::from_u128(44), digital_key(30).released(), false)]
    );
}

#[test]
fn snaptap_replay_observes_the_retained_input_even_when_its_output_fails() {
    let (mut worker, events) = observed_digital_snaptap_worker();
    press_digital(&mut worker, 30);
    let _ = snaptap_observations(&events);
    worker.executor.action_error = true;
    let error = worker
        .resolve_digital(
            digital_key(31),
            worker.current_modes().unwrap(),
            124,
            1,
            false,
        )
        .unwrap_err();
    assert!(error.to_string().contains("simulated action failure"));
    assert_eq!(
        snaptap_observations(&events),
        [
            (Uuid::from_u128(44), digital_key(30).released(), true),
            (Uuid::from_u128(44), digital_key(31), false),
        ]
    );
    worker
        .resolve_digital(
            digital_key(31).released(),
            worker.current_modes().unwrap(),
            125,
            1,
            false,
        )
        .unwrap();
    assert_eq!(
        snaptap_observations(&events),
        [
            (Uuid::from_u128(44), digital_key(31).released(), false),
            (Uuid::from_u128(44), digital_key(30), true),
        ]
    );
}

#[test]
fn snaptap_unregistered_callback_emits_nothing_and_new_registration_uses_its_epoch() {
    let (mut worker, events) = observed_digital_snaptap_worker();
    worker.snaptap_observer = None;
    press_digital(&mut worker, 30);
    assert!(snaptap_observations(&events).is_empty());
    assert_eq!(
        actions(&worker),
        [ResolvedOutput::Mapped(keyboard_output(40, 0))]
    );
    worker.snaptap_observer = Some(Uuid::from_u128(45));
    press_digital(&mut worker, 31);
    assert_eq!(
        snaptap_observations(&events),
        [
            (Uuid::from_u128(45), digital_key(30).released(), true),
            (Uuid::from_u128(45), digital_key(31), false),
        ]
    );
}

fn analog_snaptap_worker(policy: u32) -> (Worker<SimExecutor>, mpsc::Receiver<Event>) {
    let mappings = [30u16, 31]
        .into_iter()
        .flat_map(|scancode| {
            [0, 1].into_iter().map(move |flag| CompiledMapping {
                input: Input::new(InputKind::AnalogKey { scancode, flag }),
                selectors: Selectors {
                    snap_tap: Some(7),
                    ..Default::default()
                },
                output: keyboard_output(scancode + 10, flag),
            })
        })
        .collect();
    let mut doc = document(100, 80);
    doc["profiles"][0]["appEngine"]["snaptaps"] = json!({"7":policy});
    let (mut worker, events) = worker(Tables::new(vec![], mappings, vec![]).unwrap(), doc);
    worker.snaptap_observer = Some(Uuid::from_u128(44));
    (worker, events)
}

fn analog_observations(
    events: &mpsc::Receiver<Event>,
    epoch: Uuid,
) -> Vec<(Input, u8, u16, Option<Output>)> {
    events
        .try_iter()
        .flat_map(|event| match event {
            Event::Analog { observer, delivery } => {
                assert_eq!(observer, epoch);
                delivery
                    .into_events()
                    .into_iter()
                    .map(|event| {
                        (
                            event.input().clone(),
                            event.key_id(),
                            event.level(),
                            event.output().cloned(),
                        )
                    })
                    .collect::<Vec<_>>()
            }
            _ => Vec::new(),
        })
        .collect()
}

#[test]
fn analog_observation_first_zero_retains_mapping_and_interrupt_registration() {
    use super::super::analog_observation::Control;
    use razer_ipc::analog_observation::Registration;
    let mut input = Input::new(InputKind::AnalogKey {
        scancode: 30,
        flag: 0,
    });
    input.interruptible = true;
    let mappings = [input.clone(), input.released()]
        .into_iter()
        .map(|input| CompiledMapping {
            output: keyboard_output(40, u16::from(input.phase() == Phase::Release)),
            input,
            selectors: Selectors::default(),
        })
        .collect();
    let (mut worker, events) = worker(
        Tables::new(vec![], mappings, vec![]).unwrap(),
        document(100, 80),
    );
    let epoch = Uuid::from_u128(51);
    worker
        .control_analog_observation(Control::Register {
            epoch,
            registration: Registration::All { skip: 0 },
        })
        .unwrap();

    worker.hardware_input(packet(&[11, 31, 0])).unwrap();
    assert_eq!(
        analog_observations(&events, epoch),
        [(input.clone(), 31, 0, None)]
    );
    assert!(actions(&worker).is_empty());
    assert_eq!(worker.hardware.ingress.tracked_level(31), Some(0));
    assert_eq!(
        worker
            .lookup
            .analog_held_by_stateless(&input.stateless_id())
            .unwrap()
            .make
            .output,
        ResolvedOutput::Mapped(keyboard_output(40, 0))
    );
    assert_eq!(
        worker
            .hardware
            .interrupts
            .notify(&Input::new(InputKind::ControlPodDial { clockwise: true })),
        [31]
    );

    worker.lookup.replace_tables(rows(45));
    worker.hardware_input(packet(&[11, 31, 120])).unwrap();
    assert_eq!(
        actions(&worker),
        [ResolvedOutput::Mapped(keyboard_output(40, 0))]
    );
    assert_eq!(
        analog_observations(&events, epoch),
        [
            (input.clone(), 31, 120, None),
            (input, 31, 120, Some(keyboard_output(40, 0))),
        ]
    );
}

#[test]
fn analog_observation_preserves_raw_then_actual_mapping_order_and_missing_release() {
    use super::super::analog_observation::Control;
    use razer_ipc::analog_observation::Registration;
    let (mut worker, events) = worker(rows(40), document(100, 80));
    let epoch = Uuid::from_u128(49);
    worker
        .control_analog_observation(Control::Register {
            epoch,
            registration: Registration::All { skip: 0 },
        })
        .unwrap();
    let input = Input::new(InputKind::AnalogKey {
        scancode: 30,
        flag: 0,
    });
    worker.hardware_input(packet(&[11, 31, 120])).unwrap();
    assert_eq!(
        analog_observations(&events, epoch),
        [
            (input.clone(), 31, 120, None),
            (input.clone(), 31, 120, None),
            (input.clone(), 31, 120, Some(keyboard_output(40, 0))),
        ]
    );
    worker.hardware_input(packet(&[11, 31, 120])).unwrap();
    assert!(analog_observations(&events, epoch).is_empty());
    worker.hardware_input(packet(&[11])).unwrap();
    assert_eq!(
        analog_observations(&events, epoch),
        [
            (input.released(), 31, 0, None),
            (input.released(), 31, 0, Some(keyboard_output(40, 1))),
        ]
    );
}

#[test]
fn analog_observation_missing_release_raw_carries_current_modifiers() {
    use super::super::analog_observation::Control;
    use razer_ipc::analog_observation::Registration;
    let (mut worker, events) = worker(rows(40), document(100, 80));
    let epoch = Uuid::from_u128(52);
    worker
        .control_analog_observation(Control::Register {
            epoch,
            registration: Registration::All { skip: 0 },
        })
        .unwrap();
    worker.hardware_input(packet(&[11, 31, 120])).unwrap();
    let _ = analog_observations(&events, epoch);
    let mut release = packet(&[11]);
    release.input.modifiers.left_shift = true;
    release.input.modifiers.right_control = true;
    worker.hardware_input(release).unwrap();
    let observations = analog_observations(&events, epoch);
    assert_eq!(observations.len(), 2);
    let mut raw = Input::new(InputKind::AnalogKey {
        scancode: 30,
        flag: 1,
    });
    raw.modifiers = 0x420;
    assert_eq!(observations[0], (raw, 31, 0, None));
    assert_eq!(observations[1].3, Some(keyboard_output(40, 1)));
}

#[test]
fn analog_observation_forced_first_and_output_survive_skip_and_cancel_rejects_old_epoch() {
    use super::super::analog_observation::Control;
    use razer_ipc::analog_observation::Registration;
    let (mut worker, events) = worker(rows(40), document(100, 80));
    let epoch = Uuid::from_u128(49);
    worker
        .control_analog_observation(Control::Register {
            epoch,
            registration: Registration::All { skip: u32::MAX },
        })
        .unwrap();
    let input = Input::new(InputKind::AnalogKey {
        scancode: 30,
        flag: 0,
    });
    worker.hardware_input(packet(&[11, 31, 120])).unwrap();
    assert_eq!(
        analog_observations(&events, epoch),
        [
            (input.clone(), 31, 120, None),
            (input.clone(), 31, 120, Some(keyboard_output(40, 0))),
        ]
    );
    assert!(
        worker
            .control_analog_observation(Control::Stop {
                epoch: Uuid::from_u128(50)
            })
            .is_err()
    );
    worker
        .control_analog_observation(Control::Stop { epoch })
        .unwrap();
    worker.hardware_input(packet(&[11])).unwrap();
    assert!(analog_observations(&events, epoch).is_empty());
    assert_eq!(
        actions(&worker),
        [
            ResolvedOutput::Mapped(keyboard_output(40, 0)),
            ResolvedOutput::Mapped(keyboard_output(40, 1))
        ]
    );
}

#[test]
fn analog_last_input_displaces_then_restores_retained_mapping_before_missing_cleanup() {
    let (mut worker, events) = analog_snaptap_worker(0);
    worker.hardware_input(packet(&[11, 31, 120])).unwrap();
    worker
        .hardware_input(packet(&[11, 31, 120, 32, 130]))
        .unwrap();
    assert!(!worker.hardware.snaptap.key_state(31).unwrap().active);
    assert!(worker.hardware.snaptap.key_state(32).unwrap().active);
    worker.hardware_input(packet(&[11, 31, 120])).unwrap();
    assert!(worker.hardware.snaptap.key_state(32).is_none());
    assert!(worker.hardware.snaptap.key_state(31).unwrap().active);
    worker.hardware_input(packet(&[11])).unwrap();
    assert_eq!(
        actions(&worker),
        [(40, 0), (40, 1), (41, 0), (41, 1), (40, 0), (40, 1)]
            .map(|(key, flag)| ResolvedOutput::Mapped(keyboard_output(key, flag)))
    );
    assert_eq!(
        events
            .try_iter()
            .filter(|event| matches!(event, Event::SnapTap { .. }))
            .count(),
        6
    );
    assert!(worker.hardware.snaptap.key_state(31).is_none());
    assert!(worker.composites.active_analog_keys().is_empty());
}

#[test]
fn analog_depth_competes_without_a_new_threshold_crossing() {
    let (mut worker, _events) = analog_snaptap_worker(2);
    worker.hardware_input(packet(&[11, 31, 150])).unwrap();
    worker
        .hardware_input(packet(&[11, 31, 150, 32, 120]))
        .unwrap();
    assert_eq!(
        actions(&worker),
        [ResolvedOutput::Mapped(keyboard_output(40, 0))]
    );
    assert!(!worker.hardware.snaptap.key_state(32).unwrap().active);
    worker.executor.recorded.clear();
    worker
        .hardware_input(packet(&[11, 31, 150, 32, 160]))
        .unwrap();
    assert_eq!(
        actions(&worker),
        [(40, 1), (41, 0)].map(|(key, flag)| ResolvedOutput::Mapped(keyboard_output(key, flag)))
    );
    assert!(worker.hardware.snaptap.key_state(32).unwrap().active);
    assert!(
        worker
            .hardware
            .snaptap
            .key_state(31)
            .unwrap()
            .released_by_arbitration
    );
    worker.executor.recorded.clear();
    worker
        .hardware_input(packet(&[11, 31, 150, 32, 140]))
        .unwrap();
    assert_eq!(
        actions(&worker),
        [(41, 1), (40, 0)].map(|(key, flag)| ResolvedOutput::Mapped(keyboard_output(key, flag)))
    );
}

#[test]
fn analog_current_failure_still_notifies_and_keeps_arbitration_for_real_release() {
    let (mut worker, events) = analog_snaptap_worker(0);
    worker.executor.action_error = true;
    assert!(
        worker
            .hardware_input(packet(&[11, 31, 120]))
            .unwrap_err()
            .to_string()
            .contains("simulated action failure")
    );
    assert!(worker.hardware.snaptap.key_state(31).unwrap().active);
    assert_eq!(
        events
            .try_iter()
            .filter(|event| matches!(event, Event::SnapTap { .. }))
            .count(),
        1
    );
    worker.hardware_input(packet(&[11])).unwrap();
    assert_eq!(
        actions(&worker),
        [(40, 0), (40, 1)].map(|(key, flag)| ResolvedOutput::Mapped(keyboard_output(key, flag)))
    );
    assert!(worker.hardware.snaptap.key_state(31).is_none());
}

#[test]
fn blocked_analog_missing_key_cleans_arbitration_without_another_release() {
    let (mut worker, _events) = analog_snaptap_worker(0);
    worker.hardware_input(packet(&[11, 31, 120])).unwrap();
    worker.hardware.interrupts.block(31);
    worker
        .release_analog_keys(vec![31], 0, 123, false, false)
        .unwrap();
    worker.executor.recorded.clear();
    worker.hardware_input(packet(&[11])).unwrap();
    assert!(actions(&worker).is_empty());
    assert!(worker.hardware.snaptap.key_state(31).is_none());
    assert!(worker.hardware.ingress.tracked_level(31).is_none());
    assert!(!worker.hardware.interrupts.blocked_keys().contains(&31));
}

#[test]
fn earlier_frame_callback_interrupts_later_key_before_its_candidate_level_is_published() {
    let mappings = [30u16, 31]
        .into_iter()
        .flat_map(|scancode| {
            [0, 1].into_iter().map(move |flag| {
                let mut input = Input::new(InputKind::AnalogKey { scancode, flag });
                if scancode == 30 {
                    input.interruptible = true;
                    input.interrupts.push(Input::new(InputKind::AnalogKey {
                        scancode: 31,
                        flag: 0,
                    }));
                }
                CompiledMapping {
                    input,
                    selectors: Selectors::default(),
                    output: keyboard_output(scancode + 10, flag),
                }
            })
        })
        .collect();
    let (mut worker, _) = worker(
        Tables::new(vec![], mappings, vec![]).unwrap(),
        document(100, 80),
    );
    worker.hardware_input(packet(&[11, 31, 120])).unwrap();
    worker.executor.recorded.clear();
    // Native hash-chain traversal handles key 32 before key 31 in this report.
    worker
        .hardware_input(packet(&[11, 31, 150, 32, 130]))
        .unwrap();
    assert_eq!(
        actions(&worker),
        [(40, 1), (41, 0)].map(|(key, flag)| ResolvedOutput::Mapped(keyboard_output(key, flag)))
    );
    assert!(worker.hardware.interrupts.blocked_keys().contains(&31));
    assert_eq!(worker.hardware.ingress.tracked_level(31), Some(120));
    worker.executor.recorded.clear();
    worker.hardware_input(packet(&[11, 32, 130])).unwrap();
    assert!(actions(&worker).is_empty());
    assert!(worker.hardware.ingress.tracked_level(31).is_none());
    assert!(!worker.hardware.interrupts.blocked_keys().contains(&31));
}

fn digital_mode_worker() -> Worker<SimExecutor> {
    let mappings = [30u16, 31, 32]
        .into_iter()
        .flat_map(|scancode| {
            [0, 1].into_iter().map(move |flag| CompiledMapping {
                input: Input::new(InputKind::Keyboard { scancode, flag }),
                selectors: Selectors {
                    snap_tap: (scancode != 32).then_some(7),
                    ..Default::default()
                },
                output: keyboard_output(scancode + 10, flag),
            })
        })
        .collect();
    worker(
        Tables::new(vec![], mappings, vec![]).unwrap(),
        document(100, 80),
    )
    .0
}

fn press_digital(worker: &mut Worker<SimExecutor>, scancode: u16) {
    let modes = worker.current_modes().unwrap();
    worker
        .resolve_digital(digital_key(scancode), modes, 123, 1, false)
        .unwrap();
}

#[test]
fn mapping_mode_release_skips_inactive_snaptap_and_keeps_active_record() {
    let mut worker = digital_mode_worker();
    for scancode in [30, 31, 32] {
        press_digital(&mut worker, scancode);
    }
    worker.executor.recorded.clear();
    let releases = worker.lookup.mapping_changed(false);
    worker.mapping_enabled = false;
    worker.release_mode_mappings(releases).unwrap();
    assert_eq!(
        actions(&worker),
        [41, 42].map(|code| ResolvedOutput::Mapped(keyboard_output(code, 1)))
    );
    assert_eq!(worker.lookup.held_count(), 0);
    assert!(
        !worker
            .executor
            .recorded
            .iter()
            .any(|item| matches!(item, Recorded::Pause))
    );
    assert!(
        !worker
            .snaptap
            .replayed(&digital_key(30).stateless_id(), Phase::Release)
    );
    // 6a5e0 leaves the active record intact while dispatching its direct break.
    assert!(
        worker
            .snaptap
            .replayed(&digital_key(31).stateless_id(), Phase::Release)
    );
}

#[test]
fn hypershift_release_skips_inactive_snaptap_and_keeps_holds_until_real_break() {
    let mut worker = digital_mode_worker();
    for scancode in [30, 31, 32] {
        press_digital(&mut worker, scancode);
    }
    worker.executor.recorded.clear();
    let releases = worker.lookup.hypershift_disabled();
    worker.release_mode_mappings(releases).unwrap();
    assert_eq!(
        actions(&worker),
        [41, 42].map(|code| ResolvedOutput::Mapped(keyboard_output(code, 1)))
    );
    assert_eq!(worker.lookup.held_count(), 3);
    assert!(
        !worker
            .snaptap
            .replayed(&digital_key(30).stateless_id(), Phase::Release)
    );
    worker.executor.recorded.clear();
    let modes = worker.current_modes().unwrap();
    for scancode in [30, 31, 32] {
        worker
            .resolve_digital(digital_key(scancode).released(), modes, 124, 1, false)
            .unwrap();
    }
    assert!(worker.executor.recorded.is_empty());
    assert_eq!(worker.lookup.held_count(), 0);
    // Suppressed physical release does not consume the retained Snap Tap record.
    assert!(
        worker
            .snaptap
            .replayed(&digital_key(31).stateless_id(), Phase::Release)
    );
}

#[test]
fn shutdown_directly_releases_inactive_and_hypershift_suppressed_snaptap_pairs() {
    for hypershift in [false, true] {
        let mut worker = digital_mode_worker();
        press_digital(&mut worker, 30);
        press_digital(&mut worker, 31);
        if hypershift {
            let releases = worker.lookup.hypershift_disabled();
            worker.release_mode_mappings(releases).unwrap();
        }
        worker.executor.recorded.clear();
        worker.stop().unwrap();
        assert_eq!(
            actions(&worker),
            [40, 41].map(|code| ResolvedOutput::Mapped(keyboard_output(code, 1)))
        );
        assert_eq!(worker.lookup.held_count(), 0);
        assert!(
            !worker
                .executor
                .recorded
                .iter()
                .any(|item| matches!(item, Recorded::Pause))
        );
    }
}

#[test]
fn interrupted_snaptap_real_break_does_not_restore_or_consume_arbitration_state() {
    let mappings = [30u16, 31]
        .into_iter()
        .flat_map(|scancode| {
            [0, 1].into_iter().map(move |flag| {
                let mut input = Input::new(InputKind::Keyboard { scancode, flag });
                if scancode == 31 {
                    input.interruptible = true;
                    input.interrupts.push(digital_key(45));
                }
                CompiledMapping {
                    input,
                    selectors: Selectors {
                        snap_tap: Some(7),
                        ..Default::default()
                    },
                    output: keyboard_output(scancode + 10, flag),
                }
            })
        })
        .collect();
    let (mut worker, events) = worker(
        Tables::new(vec![], mappings, vec![]).unwrap(),
        document(100, 80),
    );
    press_digital(&mut worker, 30);
    press_digital(&mut worker, 31);
    worker.executor.recorded.clear();
    worker.notify_input(&digital_key(45), 124, false).unwrap();
    assert_eq!(
        actions(&worker),
        [(41, 1), (40, 0)].map(|(code, flag)| ResolvedOutput::Mapped(keyboard_output(code, flag)))
    );
    assert_eq!(worker.lookup.held_count(), 2);
    worker.executor.recorded.clear();
    let _ = events.try_iter().count();
    let modes = worker.current_modes().unwrap();
    worker
        .resolve_digital(digital_key(31).released(), modes, 125, 1, false)
        .unwrap();
    assert!(worker.executor.recorded.is_empty());
    assert!(events.try_iter().next().is_none());
    assert_eq!(worker.lookup.held_count(), 1);
    assert!(
        worker
            .snaptap
            .replayed(&digital_key(31).stateless_id(), Phase::Release)
    );
    worker
        .resolve_digital(digital_key(30).released(), modes, 126, 1, false)
        .unwrap();
    assert_eq!(
        actions(&worker),
        [ResolvedOutput::Mapped(keyboard_output(40, 1))]
    );
    assert_eq!(worker.lookup.held_count(), 0);
}

fn dynamic_key_worker(interruptible: bool) -> (Worker<SimExecutor>, mpsc::Receiver<Event>) {
    let mappings = [30u16, 31, 32, 33, 34]
        .into_iter()
        .flat_map(|scancode| {
            [0, 1].into_iter().map(move |flag| {
                let mut input = Input::new(InputKind::Keyboard { scancode, flag });
                if interruptible && scancode == 30 && flag == 0 {
                    input.interruptible = true;
                    input.interrupts.push(digital_key(45));
                }
                CompiledMapping {
                    input,
                    selectors: Selectors {
                        dynamic_key: Some(7),
                        ..Default::default()
                    },
                    output: keyboard_output(scancode + 10, flag),
                }
            })
        })
        .collect();
    worker(
        Tables::new(vec![], mappings, vec![]).unwrap(),
        document(100, 80),
    )
}
fn release_digital(worker: &mut Worker<SimExecutor>, scancode: u16) -> Result<()> {
    let modes = worker.current_modes()?;
    worker.resolve_digital(digital_key(scancode).released(), modes, 124, 1, false)
}
#[test]
fn dynamic_key_device_dispatches_first_two_then_replays_second_retained_make_without_observations()
{
    let (mut worker, events) = dynamic_key_worker(false);
    worker.snaptap_observer = Some(Uuid::from_u128(99));
    for key in [30, 31, 32, 33] {
        press_digital(&mut worker, key);
    }
    assert_eq!(
        actions(&worker),
        [40, 41].map(|code| ResolvedOutput::Mapped(keyboard_output(code, 0)))
    );
    worker.executor.recorded.clear();
    worker.executor.execution_contexts.clear();
    release_digital(&mut worker, 30).unwrap();
    assert_eq!(
        actions(&worker),
        [(40, 1), (42, 0)].map(|(code, flag)| ResolvedOutput::Mapped(keyboard_output(code, flag)))
    );
    assert!(matches!(
        worker.executor.recorded.as_slice(),
        [Recorded::Action(_), Recorded::Pause, Recorded::Action(_)]
    ));
    assert_eq!(
        worker.executor.execution_contexts,
        [(124, false), (0, false)]
    );
    assert!(events.try_iter().next().is_none());
}
#[test]
fn dynamic_key_inactive_tail_release_replays_second_without_a_break() {
    let (mut worker, _) = dynamic_key_worker(false);
    for key in [30, 31, 32, 33] {
        press_digital(&mut worker, key);
    }
    worker.executor.recorded.clear();
    release_digital(&mut worker, 33).unwrap();
    assert_eq!(
        actions(&worker),
        [ResolvedOutput::Mapped(keyboard_output(41, 0))]
    );
    assert!(matches!(
        worker.executor.recorded.as_slice(),
        [Recorded::Pause, Recorded::Action(_)]
    ));
}
#[test]
fn dynamic_key_failed_current_output_still_retains_its_native_queue_entry() {
    let (mut worker, _) = dynamic_key_worker(false);
    worker.executor.action_error = true;
    let modes = worker.current_modes().unwrap();
    assert!(
        worker
            .resolve_digital(digital_key(30), modes, 123, 1, false)
            .is_err()
    );
    press_digital(&mut worker, 31);
    press_digital(&mut worker, 32);
    assert_eq!(actions(&worker).len(), 2);
    worker.executor.recorded.clear();
    release_digital(&mut worker, 30).unwrap();
    assert_eq!(
        actions(&worker),
        [(40, 1), (42, 0)].map(|(code, flag)| ResolvedOutput::Mapped(keyboard_output(code, flag)))
    );
}
#[test]
fn dynamic_key_interruption_bypasses_arbitration_and_keeps_retained_order() {
    let (mut worker, _) = dynamic_key_worker(true);
    for key in [30, 31, 32] {
        press_digital(&mut worker, key);
    }
    worker.executor.recorded.clear();
    worker.notify_input(&digital_key(45), 124, false).unwrap();
    assert_eq!(
        actions(&worker),
        [ResolvedOutput::Mapped(keyboard_output(40, 1))]
    );
    assert!(
        !worker
            .executor
            .recorded
            .iter()
            .any(|event| matches!(event, Recorded::Pause))
    );
    worker.executor.recorded.clear();
    release_digital(&mut worker, 30).unwrap();
    assert!(actions(&worker).is_empty());
    release_digital(&mut worker, 31).unwrap();
    assert_eq!(
        actions(&worker),
        [(41, 1), (42, 0)].map(|(code, flag)| ResolvedOutput::Mapped(keyboard_output(code, flag)))
    );
}
#[test]
fn dynamic_key_mode_release_retains_queue_until_device_stop() {
    let (mut worker, _) = dynamic_key_worker(false);
    for key in [30, 31, 32] {
        press_digital(&mut worker, key);
    }
    worker.executor.recorded.clear();
    let releases = worker.lookup.mapping_changed(false);
    worker.release_mode_mappings(releases).unwrap();
    assert_eq!(
        actions(&worker),
        [40, 41, 42].map(|code| ResolvedOutput::Mapped(keyboard_output(code, 1)))
    );
    assert!(
        !worker
            .executor
            .recorded
            .iter()
            .any(|event| matches!(event, Recorded::Pause))
    );
    worker.executor.recorded.clear();
    press_digital(&mut worker, 33);
    assert!(actions(&worker).is_empty());
    release_digital(&mut worker, 33).unwrap();
    assert_eq!(
        actions(&worker),
        [ResolvedOutput::Mapped(keyboard_output(41, 0))]
    );
    worker.stop().unwrap();
    assert!(worker.executor.closed);
    assert_eq!(worker.lookup.held_count(), 0);
}

#[test]
fn dynamic_key_profile_replacement_replays_the_retained_original_mapping() {
    let (mut worker, _) = dynamic_key_worker(false);
    for key in [30, 31, 32] {
        press_digital(&mut worker, key);
    }
    let replacement = [30u16, 31, 32]
        .into_iter()
        .flat_map(|scancode| {
            [0, 1].into_iter().map(move |flag| CompiledMapping {
                input: Input::new(InputKind::Keyboard { scancode, flag }),
                selectors: Selectors {
                    dynamic_key: Some(7),
                    ..Default::default()
                },
                output: keyboard_output(scancode + 20, flag),
            })
        })
        .collect();
    worker
        .lookup
        .replace_tables(Tables::new(vec![], replacement, vec![]).unwrap());
    worker.executor.recorded.clear();
    release_digital(&mut worker, 30).unwrap();
    assert_eq!(
        actions(&worker),
        [(40, 1), (42, 0)].map(|(code, flag)| ResolvedOutput::Mapped(keyboard_output(code, flag)))
    );
}
