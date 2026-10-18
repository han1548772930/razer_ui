//! Retained input lookup and output tasks for one device registration.
use super::{Cancellation, driver, hardware};
use anyhow::{Context, Result, ensure};
use razer_device::{
    mapping_native_action::Output,
    mapping_native_composite::{Dispatch, State as CompositeState},
    mapping_native_events::{NativeStreamKind, decode_native_stream_packet},
    mapping_native_hid::NativeHidDeviceConfig,
    mapping_native_input::{KeyboardInput, MouseInput, NativeInput},
    mapping_native_lookup::{Input, InputKind, Lookup, Modes, Resolved, Tables},
    mapping_native_mode::ModeState,
    mapping_native_session::hooks::HookInput,
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use uuid::Uuid;

#[path = "device/analog_observation.rs"]
pub(super) mod analog_observation;
#[path = "device/digital_state.rs"]
mod digital_state;
#[path = "device/gamepad_observation.rs"]
pub(super) mod gamepad_observation;
#[path = "device/hardware_state.rs"]
mod hardware_state;

pub(super) trait Executor: 'static {
    fn next_wait(&self) -> Duration;
    fn gamepad_observation(
        &mut self,
        control: gamepad_observation::Control,
        sink: gamepad_observation::Sink,
    ) -> Result<()>;
    fn begin_hardware_frame(&mut self, cleanup: bool) -> Result<()>;
    fn finish_hardware_frame(&mut self) -> Result<()>;
    fn analog(&mut self, resolved: Resolved<Output>, actuation: u16) -> Result<()>;
    fn interrupt_macro(&mut self, resolved: &Resolved<Output>) -> Result<()>;
    fn pause_mapping_replay(&mut self) -> Result<()>;
    fn execute(
        &mut self,
        resolved: Resolved<Output>,
        tick: u64,
        cleanup: bool,
        contribution_id: Option<String>,
    ) -> Result<()>;
    fn prepare_redirect(&mut self) -> Result<()>;
    fn redirect_activated(&mut self) -> Result<()>;
    fn poll(&mut self) -> Result<()>;
    fn close(&mut self) -> Result<()>;
}
#[derive(Debug)]
pub(super) enum Event {
    Input {
        input: Input,
        tick: u64,
        redirected: bool,
        observer: Option<Uuid>,
    },
    SnapTap {
        observer: Uuid,
        observation: razer_ipc::mapping_observation::SnapTapEvent,
    },
    Analog {
        observer: Uuid,
        delivery: razer_ipc::analog_observation::Delivery,
    },
    Gamepad {
        observer: Uuid,
        delivery: razer_ipc::gamepad_observation::Delivery,
    },
    Failure(String),
}
enum Command {
    Input(driver::Packet),
    Hardware(hardware::Packet),
    Tables(
        Tables<Output>,
        NativeHidDeviceConfig,
        mpsc::SyncSender<Result<(), String>>,
    ),
    Mapping(bool, mpsc::SyncSender<Result<(), String>>),
    Shortcuts(bool, mpsc::SyncSender<Result<(), String>>),
    InputObservation(bool, mpsc::SyncSender<Result<(), String>>),
    InputSubscriber(Option<Uuid>, mpsc::SyncSender<Result<(), String>>),
    SnapTapObservation(Option<Uuid>, mpsc::SyncSender<Result<(), String>>),
    GamepadObservation(
        gamepad_observation::Control,
        mpsc::SyncSender<Result<(), String>>,
    ),
    AnalogObservation(
        analog_observation::Control,
        mpsc::SyncSender<Result<(), String>>,
    ),
    HypershiftReleased,
    HeldCount(mpsc::SyncSender<u8>),
    RedirectActivated(mpsc::SyncSender<Result<(), String>>),
    Stop(mpsc::SyncSender<Result<(), String>>),
}
pub(super) struct Owner {
    sender: mpsc::SyncSender<Command>,
    events: mpsc::Receiver<Event>,
    overflow: Arc<AtomicBool>,
    cancellation: Arc<Cancellation>,
    worker: Option<JoinHandle<()>>,
    cleanup_result: Option<Result<(), String>>,
}
pub(super) fn native_lookup(input: NativeInput) -> Option<Input> {
    let mut normalized = match input {
        NativeInput::Keyboard(key) => Input::new(InputKind::Keyboard {
            scancode: key.scancode,
            flag: key.flag,
        }),
        NativeInput::Mouse(mouse) => Input::new(InputKind::Mouse {
            id: mouse.id,
            data: mouse.data,
        }),
        NativeInput::Display(_) => return None,
    };
    normalized.modifiers = match input {
        NativeInput::Keyboard(key) => key.modifiers,
        NativeInput::Mouse(mouse) => mouse.modifiers,
        _ => 0,
    };
    // Ordinary physical input uses no unitId property (114e3d/114ebf false).
    Some(normalized)
}
fn hooks(tables: &Tables<Output>) -> Vec<HookInput> {
    tables
        .hook_inputs()
        .into_iter()
        .filter_map(|input| match input.kind {
            InputKind::Keyboard { scancode, flag } => Some(HookInput::Keyboard {
                input: KeyboardInput {
                    scancode,
                    flag,
                    unit_id: input.unit_id.unwrap_or(0),
                    modifiers: input.modifiers,
                    reserved: 0,
                },
                extra_information: 0,
            }),
            InputKind::Mouse { id, data } => Some(HookInput::Mouse {
                input: MouseInput {
                    id,
                    data,
                    unit_id: input.unit_id.unwrap_or(0),
                    modifiers: input.modifiers,
                    ..Default::default()
                },
                extra_information: 0,
            }),
            _ => None, // Hardware-report hooks belong to the separate HID endpoint.
        })
        .collect()
}
struct Worker<E> {
    lookup: Lookup<Output>,
    composites: CompositeState,
    hardware: hardware_state::Hardware,
    snaptap: razer_device::mapping_native_snaptap::DigitalState,
    dynamic_keys: razer_device::mapping_native_dynamic_key::State,
    began: Instant,
    route: driver::Route,
    executor: E,
    caller: Uuid,
    modes: Arc<Mutex<ModeState>>,
    mapping_enabled: bool,
    shortcuts_enabled: bool,
    input_observed: bool,
    input_observer: Option<Uuid>,
    snaptap_observer: Option<Uuid>,
    analog_observation: analog_observation::Observation,
    events: mpsc::SyncSender<Event>,
    overflow: Arc<AtomicBool>,
}
impl<E: Executor> Worker<E> {
    pub(super) fn input_observation_enabled(&self) -> bool {
        self.input_observed || self.input_observer.is_some()
    }
    pub(super) fn notify_snaptap(&self, input: &Input, replayed: bool) -> Result<()> {
        if let Some(observer) = self.snaptap_observer {
            self.emit(Event::SnapTap {
                observer,
                observation: razer_ipc::mapping_observation::SnapTapEvent::new(
                    input.clone(),
                    replayed,
                    razer_platform::event_clock::tick()?,
                ),
            });
        }
        Ok(())
    }
    fn current_modes(&self) -> Result<Modes> {
        let state = self
            .modes
            .lock()
            .map_err(|_| anyhow::anyhow!("Mapping modes poisoned"))?;
        Ok(Modes {
            mapping_enabled: self.mapping_enabled,
            global_shortcuts_enabled: self.shortcuts_enabled,
            hypershift: state.hypershift(),
            otfs: state.otfs(),
            gaming: state.gaming(self.caller)?,
        })
    }
    fn emit(&self, event: Event) {
        if self.events.try_send(event).is_err() {
            self.overflow.store(true, Ordering::Release);
        }
    }
    fn execute_resolved(
        &mut self,
        value: Resolved<Output>,
        tick: u64,
        cleanup: bool,
    ) -> Result<()> {
        self.execute_dispatches(
            vec![Dispatch {
                resolved: value,
                contribution_id: None,
            }],
            tick,
            cleanup,
        )
    }
    fn execute_dispatches(
        &mut self,
        dispatches: Vec<Dispatch>,
        tick: u64,
        cleanup: bool,
    ) -> Result<()> {
        let mut pending = std::collections::VecDeque::from(dispatches);
        let mut errors = Vec::new();
        while let Some(Dispatch {
            resolved,
            contribution_id,
        }) = pending.pop_front()
        {
            if matches!(
                resolved.output,
                razer_device::mapping_native_lookup::ResolvedOutput::Mapped(
                    Output::Cascade(_) | Output::Dpad(_) | Output::ModTap { .. }
                )
            ) {
                let lookup = &self.lookup;
                match self.composites.apply_at(
                    resolved,
                    |id| lookup.held_by_stateless(id),
                    self.began.elapsed(),
                ) {
                    Ok(dispatches) => {
                        for mut dispatch in dispatches.into_iter().rev() {
                            if dispatch.contribution_id.is_none() {
                                dispatch.contribution_id = contribution_id.clone();
                            }
                            pending.push_front(dispatch);
                        }
                    }
                    Err(error) => errors.push(format!("{error:#}")),
                }
                errors.extend(self.composites.take_diagnostics());
            } else if let Err(error) =
                self.executor
                    .execute(resolved, tick, cleanup, contribution_id)
            {
                errors.push(format!("{error:#}"));
            }
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
    fn poll(&mut self) -> Result<()> {
        self.poll_analog_observation()?;
        let dispatches = self.composites.poll(self.began.elapsed());
        let mut errors = Vec::new();
        if !dispatches.is_empty() {
            let tick = razer_platform::event_clock::tick()?;
            if let Err(error) = self.execute_dispatches(dispatches, tick, false) {
                errors.push(format!("Timed mapping: {error:#}"));
            }
        }
        if let Err(error) = self.executor.poll() {
            errors.push(format!("{error:#}"));
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
    fn released(&mut self, values: Vec<Resolved<Output>>) -> Result<()> {
        let mut errors = Vec::new();
        let tick = razer_platform::event_clock::tick()?;
        for value in values {
            if let Err(error) = self.execute_resolved(value, tick, true) {
                errors.push(format!("{error:#}"));
            }
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
    fn release_mode_mappings(&mut self, values: Vec<Resolved<Output>>) -> Result<()> {
        let mut errors = Vec::new();
        let tick = razer_platform::event_clock::tick()?;
        for value in values {
            // 6a5e0/6ae30 skip inactive Snap Tap outputs without arbitrating
            // group membership. Shutdown's 7961a instead releases every pair.
            if self.snaptap.remove_inactive(&value.input.stateless_id()) {
                continue;
            }
            if let Err(error) = self.execute_resolved(value, tick, true) {
                errors.push(format!("{error:#}"));
            }
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
    fn input(&mut self, packet: driver::Packet) -> Result<()> {
        let decoded = decode_native_stream_packet(&packet.bytes)?;
        let Some(input) = decoded
            .input
            .normalized(false, packet.modifiers)
            .and_then(native_lookup)
        else {
            return Ok(());
        };
        match decoded.kind {
            NativeStreamKind::Hook => {
                let modes = self.current_modes()?;
                self.resolve_digital(input, modes, packet.time_tick, 1, false)?;
            }
            NativeStreamKind::Notify => {
                self.notify_input(&input, packet.time_tick, false)?;
            }
            NativeStreamKind::Redirect => {
                if matches!(input.kind, InputKind::Mouse { id: 0, data: 0 }) {
                    return Ok(());
                }
                self.emit(Event::Input {
                    input,
                    tick: packet.time_tick,
                    redirected: true,
                    observer: None,
                });
            }
            NativeStreamKind::Unknown(_) => {}
        }
        Ok(())
    }
    fn stop(&mut self) -> Result<()> {
        self.control_gamepad_observation(gamepad_observation::Control::Clear)?;
        self.analog_observation = Default::default();
        let mut errors = Vec::new();
        if let Err(error) = self.executor.prepare_redirect() {
            errors.push(format!("Stop keyboard timers: {error:#}"));
        }
        if let Err(error) = self.release_hardware() {
            errors.push(format!("Release hardware mappings: {error:#}"));
        }
        let releases = self.lookup.drain_releases();
        if let Err(error) = self.released(releases) {
            errors.push(format!("Release input mappings: {error:#}"));
        }
        self.composites.clear();
        self.snaptap.clear();
        if let Err(error) = self.executor.close() {
            errors.push(format!("Stop device execution: {error:#}"));
        }
        self.dynamic_keys.clear();
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
}
impl Owner {
    pub fn open<E: Executor>(
        route: driver::Route,
        caller: Uuid,
        modes: Arc<Mutex<ModeState>>,
        create: impl FnOnce(driver::Route, Arc<Cancellation>) -> Result<E> + Send + 'static,
    ) -> Result<Self> {
        let (sender, commands) = mpsc::sync_channel(512);
        let (event_sender, events) = mpsc::sync_channel(512);
        let (ready, initialized) = mpsc::sync_channel(1);
        let cancellation = Arc::new(Cancellation::default());
        let worker_cancel = cancellation.clone();
        let overflow = Arc::new(AtomicBool::new(false));
        let worker_overflow = overflow.clone();
        let worker = thread::Builder::new()
            .name("razer-mapping-device".into())
            .spawn(move || {
                let result = crate::mapping::guarded(|| create(route.clone(), worker_cancel));
                let executor = match result {
                    Ok(value) => value,
                    Err(error) => {
                        let _ = ready.send(Err(format!("{error:#}")));
                        return;
                    }
                };
                let mut state = Worker {
                    lookup: Lookup::new(Tables::new(Vec::new(), Vec::new(), Vec::new()).unwrap()),
                    composites: CompositeState::default(),
                    hardware: hardware_state::Hardware::default(),
                    snaptap: Default::default(),
                    dynamic_keys: Default::default(),
                    began: Instant::now(),
                    route,
                    executor,
                    caller,
                    modes,
                    mapping_enabled: true,
                    shortcuts_enabled: true,
                    input_observed: false,
                    input_observer: None,
                    snaptap_observer: None,
                    analog_observation: Default::default(),
                    events: event_sender,
                    overflow: worker_overflow,
                };
                let _ = ready.send(Ok(()));
                loop {
                    let mut stopping = false;
                    match commands
                        .recv_timeout(state.observation_wait().min(state.executor.next_wait()))
                    {
                        Ok(Command::Input(packet)) => {
                            if let Err(error) = crate::mapping::guarded(|| state.input(packet)) {
                                state.emit(Event::Failure(format!("{error:#}")));
                            }
                        }
                        Ok(Command::Hardware(packet)) => {
                            if let Err(error) =
                                crate::mapping::guarded(|| state.hardware_input(packet))
                            {
                                state.emit(Event::Failure(format!("{error:#}")));
                            }
                        }
                        Ok(Command::Tables(tables, config, reply)) => {
                            let result = crate::mapping::guarded(|| {
                                state.route.replace_hooks(hooks(&tables))?;
                                state.lookup.replace_tables(tables);
                                state.hardware.replace_config(config);
                                Ok(())
                            });
                            let _ = reply.send(result.map_err(|e| format!("{e:#}")));
                        }
                        Ok(Command::Mapping(enabled, reply)) => {
                            let result = crate::mapping::guarded(|| {
                                let releases = state.lookup.mapping_changed(enabled);
                                state.mapping_enabled = enabled;
                                state.release_mode_mappings(releases)
                            });
                            let _ = reply.send(result.map_err(|e| format!("{e:#}")));
                        }
                        Ok(Command::Shortcuts(enabled, reply)) => {
                            state.shortcuts_enabled = enabled;
                            let _ = reply.send(Ok(()));
                        }
                        Ok(Command::InputObservation(enabled, reply)) => {
                            state.input_observed = enabled;
                            let _ = reply.send(Ok(()));
                        }
                        Ok(Command::InputSubscriber(observer, reply)) => {
                            state.input_observer = observer;
                            let _ = reply.send(Ok(()));
                        }
                        Ok(Command::SnapTapObservation(observer, reply)) => {
                            state.snaptap_observer = observer;
                            let _ = reply.send(Ok(()));
                        }
                        Ok(Command::GamepadObservation(control, reply)) => {
                            let result = crate::mapping::guarded(|| {
                                state.control_gamepad_observation(control)
                            });
                            let _ = reply.send(result.map_err(|error| format!("{error:#}")));
                        }
                        Ok(Command::AnalogObservation(control, reply)) => {
                            let result = crate::mapping::guarded(|| {
                                state.control_analog_observation(control)
                            });
                            let _ = reply.send(result.map_err(|error| format!("{error:#}")));
                        }
                        Ok(Command::HypershiftReleased) => {
                            let releases = state.lookup.hypershift_disabled();
                            if let Err(error) =
                                crate::mapping::guarded(|| state.release_mode_mappings(releases))
                            {
                                state.emit(Event::Failure(format!("{error:#}")));
                            }
                            if let Err(error) =
                                crate::mapping::guarded(|| state.hypershift_hardware_released())
                            {
                                state.emit(Event::Failure(format!("{error:#}")));
                            }
                        }
                        Ok(Command::RedirectActivated(reply)) => {
                            let result = crate::mapping::guarded(|| {
                                let mut errors = Vec::new();
                                if let Err(error) = state.executor.prepare_redirect() {
                                    errors.push(format!("Stop keyboard timers: {error:#}"));
                                }
                                if let Err(error) = state.release_hardware() {
                                    errors.push(format!("Release hardware mappings: {error:#}"));
                                }
                                let releases = state.lookup.drain_releases();
                                if let Err(error) = state.released(releases) {
                                    errors.push(format!("Release input mappings: {error:#}"));
                                }
                                state.composites.clear();
                                state.snaptap.clear();
                                if let Err(error) = state.executor.redirect_activated() {
                                    errors.push(format!("Reset redirected input: {error:#}"));
                                }
                                ensure!(errors.is_empty(), "{}", errors.join("; "));
                                Ok(())
                            });
                            let _ = reply.send(result.map_err(|e| format!("{e:#}")));
                        }
                        Ok(Command::HeldCount(reply)) => {
                            let _ = reply.send(state.lookup.held_count());
                        }
                        Ok(Command::Stop(reply)) => {
                            let result = crate::mapping::guarded(|| state.stop());
                            let _ = reply.send(result.map_err(|e| format!("{e:#}")));
                            stopping = true;
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            if let Err(error) = crate::mapping::guarded(|| state.stop()) {
                                eprintln!("Device owner cleanup: {error:#}");
                            }
                            stopping = true;
                        }
                    }
                    if stopping {
                        break;
                    }
                    if let Err(error) = crate::mapping::guarded(|| state.poll()) {
                        state.emit(Event::Failure(format!("{error:#}")));
                    }
                }
            })?;
        match initialized.recv() {
            Ok(Ok(())) => Ok(Self {
                sender,
                events,
                overflow,
                cancellation,
                worker: Some(worker),
                cleanup_result: None,
            }),
            result => {
                let _ = worker.join();
                anyhow::bail!("Mapping device initialization: {result:?}");
            }
        }
    }
    pub fn input(&self, packet: driver::Packet) -> Result<()> {
        self.sender
            .try_send(Command::Input(packet))
            .context("Mapping device input queue is incomplete")
    }
    pub fn hardware_input(&self, packet: hardware::Packet) -> Result<()> {
        self.sender
            .try_send(Command::Hardware(packet))
            .context("Mapping hardware input queue is incomplete")
    }
    pub fn events(&mut self) -> Result<Vec<Event>> {
        ensure!(
            !self.overflow.load(Ordering::Acquire),
            "Mapping device event queue overflowed"
        );
        Ok(self.events.try_iter().collect())
    }
    fn control(
        &self,
        create: impl FnOnce(mpsc::SyncSender<Result<(), String>>) -> Command,
    ) -> Result<()> {
        let (reply, receive) = mpsc::sync_channel(1);
        self.sender
            .send(create(reply))
            .context("Mapping device stopped")?;
        receive
            .recv()
            .context("Mapping device reply disconnected")?
            .map_err(anyhow::Error::msg)
    }
    pub fn publish(&self, tables: Tables<Output>, config: NativeHidDeviceConfig) -> Result<()> {
        self.control(|reply| Command::Tables(tables, config, reply))
    }
    pub fn mapping(&self, enabled: bool) -> Result<()> {
        self.control(|reply| Command::Mapping(enabled, reply))
    }
    pub fn shortcuts(&self, enabled: bool) -> Result<()> {
        self.control(|reply| Command::Shortcuts(enabled, reply))
    }
    pub fn held_count(&self) -> Result<u8> {
        let (reply, receive) = mpsc::sync_channel(1);
        self.sender
            .send(Command::HeldCount(reply))
            .context("Mapping device stopped")?;
        receive
            .recv()
            .context("Mapping held count reply disconnected")
    }
    pub fn hypershift_released(&self) -> Result<()> {
        self.sender
            .send(Command::HypershiftReleased)
            .context("Mapping device stopped")
    }
    pub fn redirect_activated(&self) -> Result<()> {
        self.control(Command::RedirectActivated)
    }
    pub fn input_observation(&self, enabled: bool) -> Result<()> {
        self.control(|reply| Command::InputObservation(enabled, reply))
    }
    pub fn input_subscriber(&self, observer: Option<Uuid>) -> Result<()> {
        self.control(|reply| Command::InputSubscriber(observer, reply))
    }
    pub fn gamepad_observation(&self, control: gamepad_observation::Control) -> Result<()> {
        self.control(|reply| Command::GamepadObservation(control, reply))
    }
    pub fn analog_observation(&self, control: analog_observation::Control) -> Result<()> {
        self.control(|reply| Command::AnalogObservation(control, reply))
    }
    pub fn snaptap_observation(&self, observer: Option<Uuid>) -> Result<()> {
        self.control(|reply| Command::SnapTapObservation(observer, reply))
    }
    pub fn stop(&mut self) -> Result<()> {
        let Some(worker) = self.worker.take() else {
            return self
                .cleanup_result
                .clone()
                .unwrap_or(Ok(()))
                .map_err(anyhow::Error::msg);
        };
        self.cancellation.cancel();
        let result = self.control(Command::Stop);
        let joined = worker
            .join()
            .map_err(|_| anyhow::anyhow!("Mapping device worker panicked"));
        let result = match (result, joined) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(format!("{error:#}")),
            (Err(error), Err(join)) => Err(format!("{error:#}; {join:#}")),
        };
        self.cleanup_result = Some(result.clone());
        result.map_err(anyhow::Error::msg)
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            eprintln!("Mapping device cleanup: {error:#}");
        }
    }
}
