//! Retained input lookup and output tasks for one device registration.
use super::{Cancellation, driver};
use anyhow::{Context, Result, ensure};
use razer_device::{
    mapping_native_action::Output,
    mapping_native_composite::{Dispatch, State as CompositeState},
    mapping_native_events::{NativeStreamKind, decode_native_stream_packet},
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
    time::Duration,
};
use uuid::Uuid;

pub(super) trait Executor: 'static {
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
    },
    Failure(String),
}
enum Command {
    Input(driver::Packet),
    Tables(Tables<Output>, mpsc::SyncSender<Result<(), String>>),
    Mapping(bool, mpsc::SyncSender<Result<(), String>>),
    Shortcuts(bool, mpsc::SyncSender<Result<(), String>>),
    HypershiftReleased,
    RedirectActivated(mpsc::SyncSender<Result<(), String>>),
    Stop(mpsc::SyncSender<Result<(), String>>),
}
pub(super) struct Owner {
    sender: mpsc::SyncSender<Command>,
    events: mpsc::Receiver<Event>,
    overflow: Arc<AtomicBool>,
    cancellation: Arc<Cancellation>,
    worker: Option<JoinHandle<()>>,
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
    route: driver::Route,
    executor: E,
    caller: Uuid,
    modes: Arc<Mutex<ModeState>>,
    mapping_enabled: bool,
    shortcuts_enabled: bool,
    events: mpsc::SyncSender<Event>,
    overflow: Arc<AtomicBool>,
}
impl<E: Executor> Worker<E> {
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
        let mut pending = std::collections::VecDeque::from([Dispatch {
            resolved: value,
            contribution_id: None,
        }]);
        let mut errors = Vec::new();
        while let Some(Dispatch {
            resolved,
            contribution_id,
        }) = pending.pop_front()
        {
            if matches!(
                resolved.output,
                razer_device::mapping_native_lookup::ResolvedOutput::Mapped(
                    Output::Cascade(_) | Output::Dpad(_)
                )
            ) {
                let lookup = &self.lookup;
                match self
                    .composites
                    .apply(resolved, |id| lookup.held_by_stateless(id))
                {
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
                let state = self
                    .modes
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Mapping modes poisoned"))?;
                let modes = Modes {
                    mapping_enabled: self.mapping_enabled,
                    global_shortcuts_enabled: self.shortcuts_enabled,
                    hypershift: state.hypershift(),
                    otfs: state.otfs(),
                    gaming: state.gaming(self.caller)?,
                };
                drop(state);
                if let Some(resolved) = self.lookup.resolve(input, modes) {
                    self.execute_resolved(resolved, packet.time_tick, false)?;
                }
            }
            NativeStreamKind::Notify => self.emit(Event::Input {
                input,
                tick: packet.time_tick,
                redirected: false,
            }),
            NativeStreamKind::Redirect => {
                if matches!(input.kind, InputKind::Mouse { id: 0, data: 0 }) {
                    return Ok(());
                }
                self.emit(Event::Input {
                    input,
                    tick: packet.time_tick,
                    redirected: true,
                });
            }
            NativeStreamKind::Unknown(_) => {}
        }
        Ok(())
    }
    fn stop(&mut self) -> Result<()> {
        let mut errors = Vec::new();
        if let Err(error) = self.executor.prepare_redirect() {
            errors.push(format!("Stop keyboard timers: {error:#}"));
        }
        let releases = self.lookup.drain_releases();
        if let Err(error) = self.released(releases) {
            errors.push(format!("Release input mappings: {error:#}"));
        }
        self.composites.clear();
        if let Err(error) = self.executor.close() {
            errors.push(format!("Stop device execution: {error:#}"));
        }
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
                    route,
                    executor,
                    caller,
                    modes,
                    mapping_enabled: true,
                    shortcuts_enabled: true,
                    events: event_sender,
                    overflow: worker_overflow,
                };
                let _ = ready.send(Ok(()));
                loop {
                    let mut stopping = false;
                    match commands.recv_timeout(Duration::from_millis(4)) {
                        Ok(Command::Input(packet)) => {
                            if let Err(error) = crate::mapping::guarded(|| state.input(packet)) {
                                state.emit(Event::Failure(format!("{error:#}")));
                            }
                        }
                        Ok(Command::Tables(tables, reply)) => {
                            let result = crate::mapping::guarded(|| {
                                state.route.replace_hooks(hooks(&tables))?;
                                state.lookup.replace_tables(tables);
                                Ok(())
                            });
                            let _ = reply.send(result.map_err(|e| format!("{e:#}")));
                        }
                        Ok(Command::Mapping(enabled, reply)) => {
                            let result = crate::mapping::guarded(|| {
                                let releases = state.lookup.mapping_changed(enabled);
                                state.mapping_enabled = enabled;
                                state.released(releases)
                            });
                            let _ = reply.send(result.map_err(|e| format!("{e:#}")));
                        }
                        Ok(Command::Shortcuts(enabled, reply)) => {
                            state.shortcuts_enabled = enabled;
                            let _ = reply.send(Ok(()));
                        }
                        Ok(Command::HypershiftReleased) => {
                            let releases = state.lookup.hypershift_disabled();
                            if let Err(error) = crate::mapping::guarded(|| state.released(releases))
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
                                let releases = state.lookup.drain_releases();
                                if let Err(error) = state.released(releases) {
                                    errors.push(format!("Release input mappings: {error:#}"));
                                }
                                state.composites.clear();
                                if let Err(error) = state.executor.redirect_activated() {
                                    errors.push(format!("Reset redirected input: {error:#}"));
                                }
                                ensure!(errors.is_empty(), "{}", errors.join("; "));
                                Ok(())
                            });
                            let _ = reply.send(result.map_err(|e| format!("{e:#}")));
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
                    if let Err(error) = crate::mapping::guarded(|| state.executor.poll()) {
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
    pub fn publish(&self, tables: Tables<Output>) -> Result<()> {
        self.control(|reply| Command::Tables(tables, reply))
    }
    pub fn mapping(&self, enabled: bool) -> Result<()> {
        self.control(|reply| Command::Mapping(enabled, reply))
    }
    pub fn shortcuts(&self, enabled: bool) -> Result<()> {
        self.control(|reply| Command::Shortcuts(enabled, reply))
    }
    pub fn hypershift_released(&self) -> Result<()> {
        self.sender
            .send(Command::HypershiftReleased)
            .context("Mapping device stopped")
    }
    pub fn redirect_activated(&self) -> Result<()> {
        self.control(Command::RedirectActivated)
    }
    pub fn stop(&mut self) -> Result<()> {
        let Some(worker) = self.worker.take() else {
            return Ok(());
        };
        self.cancellation.cancel();
        let result = self.control(Command::Stop);
        let joined = worker
            .join()
            .map_err(|_| anyhow::anyhow!("Mapping device worker panicked"));
        match (result, joined) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
            (Err(error), Err(join)) => anyhow::bail!("{error:#}; {join:#}"),
        }
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            eprintln!("Mapping device cleanup: {error:#}");
        }
    }
}
