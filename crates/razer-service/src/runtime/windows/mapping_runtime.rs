//! Independent MacroData and execution ownership. All OS handles are created,
//! used and released on their worker; service callers retain cancellation and
//! join handles rather than transferring native pointers across threads.
#[path = "mapping_runtime/acceleration.rs"]
mod acceleration;
#[path = "mapping_runtime/advanced.rs"]
mod advanced;
#[path = "mapping_runtime/audio.rs"]
mod audio;
#[path = "mapping_runtime/backend.rs"]
mod backend;
#[path = "mapping_runtime/capture.rs"]
mod capture;
#[path = "mapping_runtime/clipboard.rs"]
mod clipboard;
#[path = "mapping_runtime/connections.rs"]
mod connections;
#[path = "mapping_runtime/device.rs"]
mod device;
#[path = "mapping_runtime/direct.rs"]
mod direct;
#[path = "mapping_runtime/display.rs"]
mod display;
#[path = "mapping_runtime/driver.rs"]
mod driver;
#[path = "mapping_runtime/executor.rs"]
mod executor;
#[path = "mapping_runtime/hardware.rs"]
mod hardware;
#[path = "mapping_runtime/identity.rs"]
mod identity;
#[path = "mapping_runtime/input.rs"]
mod input;
#[path = "mapping_runtime/launch.rs"]
mod launch;
#[path = "mapping_runtime/philips.rs"]
mod philips;
#[path = "mapping_runtime/system.rs"]
mod system;

pub(super) fn client() -> Result<crate::mapping::owner::Client> {
    static REGISTRY: std::sync::OnceLock<crate::mapping::owner::Registry> =
        std::sync::OnceLock::new();
    REGISTRY
        .get_or_init(Default::default)
        .acquire(|| Ok(backend::Backend::new(executor::Executor::open)))
}

use anyhow::{Context, Result, ensure};
use razer_device::macro_runtime::{
    Action, Completion, Execution, KeyboardExecutor, KeyboardTiming, Library, Poll, Publication,
    scheduler::{Dispatch, Mode, Request, Scheduler, Transition},
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Condvar, Mutex, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use uuid::Uuid;
use windows_sys::Win32::{
    Security::Cryptography::{BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom},
    UI::WindowsAndMessaging::SystemParametersInfoW,
};

pub use audio::Observation as AudioObservation;
pub use input::Receipt as InputReceipt;

#[derive(Clone, Debug)]
pub struct Parameters {
    pub repeat: u32,
    pub(crate) program: Program,
    /// An observed filter-driver route, never a HID collection or saved path.
    pub(crate) driver_route: Option<driver::Route>,
    pub(crate) mapped_key: Option<
        razer_device::mapping_native_lookup::Resolved<razer_device::mapping_native_action::Output>,
    >,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Program {
    Macro,
    Phased { delay_ms: u32 },
    Turbo { delay_ms: u32 },
}

enum ExecutionProgram {
    Macro(Execution),
    Advanced(razer_device::macro_runtime::advanced::Execution),
}

pub struct Start {
    pub caller: Uuid,
    pub guid: String,
    pub parameters: Parameters,
}

#[derive(Debug)]
pub enum Submission {
    Started(Uuid),
    CancellationRequested(Uuid),
    Queued,
    Unchanged,
}

#[derive(Debug)]
pub struct Finished {
    pub id: Uuid,
    pub completion: Completion,
    pub last_input: Option<InputReceipt>,
    pub completed_inputs: u64,
    pub audio_observations: Vec<AudioObservation>,
}

impl Finished {
    fn into_errors(self, expected_id: Uuid) -> Vec<String> {
        let mut errors = self
            .completion
            .execution_error
            .into_iter()
            .chain(self.completion.operation_errors)
            .chain(self.completion.cleanup_errors)
            .collect::<Vec<_>>();
        if self.id != expected_id {
            errors.push(format!(
                "Mapping execution receipt {} belongs to another execution; expected {expected_id}",
                self.id,
            ));
        }
        if self.completion.omitted_operation_errors != 0 {
            errors.push(format!(
                "{} additional mapping operation errors were omitted",
                self.completion.omitted_operation_errors,
            ));
        }
        if !errors.is_empty() {
            let last_input = self.last_input.as_ref().map(ToString::to_string);
            let audio = self
                .audio_observations
                .iter()
                .map(|observation| {
                    format!(
                        "{}: scalar={} muted={}",
                        observation.endpoint, observation.scalar, observation.muted
                    )
                })
                .collect::<Vec<_>>();
            errors.push(format!(
                "Execution {}: completed_inputs={} last_input={last_input:?} audio={audio:?}",
                self.id, self.completed_inputs,
            ));
        }
        errors
    }
}

#[derive(Debug)]
pub struct Shutdown {
    pub executions: Vec<(Uuid, Result<Finished>)>,
    pub cleanup_errors: Vec<String>,
}

#[derive(Default)]
struct Cancellation {
    stopped: AtomicBool,
    gate: Mutex<()>,
    wake: Condvar,
}
impl Cancellation {
    fn cancel(&self) {
        let _guard = self.gate.lock().unwrap_or_else(|error| error.into_inner());
        self.stopped.store(true, Ordering::Release);
        self.wake.notify_all();
    }
    fn is_cancelled(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
    }
    fn wait(&self, duration: Duration) {
        let guard = self.gate.lock().unwrap_or_else(|error| error.into_inner());
        let _unused = self
            .wake
            .wait_timeout_while(guard, duration, |_| !self.is_cancelled());
    }
}

struct Running {
    caller: Uuid,
    cancellation: Arc<Cancellation>,
    thread: JoinHandle<Result<Finished>>,
}

pub struct State {
    library: Arc<RwLock<Library>>,
    advanced: Arc<RwLock<razer_device::macro_runtime::advanced::Library>>,
    completion_callers: BTreeMap<Uuid, Uuid>,
    running: BTreeMap<Uuid, Running>,
    scheduler: Scheduler<Parameters>,
    pending_completions: Vec<(Uuid, Result<Finished>)>,
    clipboard: Option<clipboard::Service>,
    acceleration: acceleration::Owner,
    stopping: bool,
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}
impl State {
    pub fn new() -> Self {
        Self {
            library: Arc::new(RwLock::new(Library::default())),
            advanced: Arc::new(RwLock::new(
                razer_device::macro_runtime::advanced::Library::default(),
            )),
            completion_callers: BTreeMap::new(),
            running: BTreeMap::new(),
            scheduler: Scheduler::default(),
            pending_completions: Vec::new(),
            clipboard: None,
            acceleration: acceleration::Owner::default(),
            stopping: false,
        }
    }

    pub fn publish(&mut self, document: &Value) -> Result<Publication> {
        ensure!(!self.stopping, "Mapping execution owner is stopping");
        self.library
            .write()
            .map_err(|_| anyhow::anyhow!("Macro library lock poisoned"))?
            .publish(document)
    }

    pub fn publish_storage(&mut self, key: &str, value: &Value) -> Result<()> {
        use razer_device::macro_runtime::advanced::TurboSource;
        match key {
            "synapseMacros" => {
                self.publish(value)?;
            }
            "synapseDefaultTurbos" => {
                self.advanced
                    .write()
                    .map_err(|_| anyhow::anyhow!("Advanced macro library poisoned"))?
                    .publish_turbos(TurboSource::Default, value)?;
            }
            "synapseTurbos" => {
                self.advanced
                    .write()
                    .map_err(|_| anyhow::anyhow!("Advanced macro library poisoned"))?
                    .publish_turbos(TurboSource::User, value)?;
            }
            "synapsePhasedMacros" => {
                self.advanced
                    .write()
                    .map_err(|_| anyhow::anyhow!("Advanced macro library poisoned"))?
                    .publish_phased(value)?;
            }
            _ => anyhow::bail!("Unknown mapping engine storage key {key}"),
        }
        Ok(())
    }

    fn observed_completions(&mut self) -> Vec<(Uuid, Option<Uuid>, Result<Finished>)> {
        self.take_completed()
            .into_iter()
            .map(|(id, result)| (id, self.completion_callers.remove(&id), result))
            .collect()
    }

    pub fn submit(&mut self, request: Start, mode: Mode) -> Result<Submission> {
        ensure!(!self.stopping, "Mapping execution owner is stopping");
        match self.scheduler.submit(
            Request {
                caller: request.caller,
                guid: request.guid,
                payload: request.parameters,
            },
            mode,
        )? {
            Transition::Start(dispatch) => {
                let id = dispatch.id;
                if let Err(error) = self.spawn(dispatch) {
                    let next = self.scheduler.complete(id)?;
                    self.launch_queued(next);
                    return Err(error);
                }
                Ok(Submission::Started(id))
            }
            Transition::Cancel(id) => {
                self.cancel(id)?;
                Ok(Submission::CancellationRequested(id))
            }
            Transition::Queued => Ok(Submission::Queued),
            Transition::Unchanged => Ok(Submission::Unchanged),
        }
    }

    fn spawn(&mut self, dispatch: Dispatch<Parameters>) -> Result<()> {
        let id = dispatch.id;
        let request = dispatch.request;
        let execution = match request.payload.program {
            Program::Macro => ExecutionProgram::Macro(Execution::start(
                &*self
                    .library
                    .read()
                    .map_err(|_| anyhow::anyhow!("Macro library lock poisoned"))?,
                &request.guid,
                request.payload.repeat,
            )?),
            Program::Phased { delay_ms } => ExecutionProgram::Advanced(
                razer_device::macro_runtime::advanced::Execution::phased(
                    &*self
                        .advanced
                        .read()
                        .map_err(|_| anyhow::anyhow!("Advanced macro library poisoned"))?,
                    &request.guid,
                    delay_ms,
                )?,
            ),
            Program::Turbo { delay_ms } => {
                ExecutionProgram::Advanced(razer_device::macro_runtime::advanced::Execution::turbo(
                    &*self
                        .advanced
                        .read()
                        .map_err(|_| anyhow::anyhow!("Advanced macro library poisoned"))?,
                    &request.guid,
                    request.payload.repeat,
                    delay_ms,
                )?)
            }
        };
        let clipboard = self.clipboard_client()?;
        let cancellation = Arc::new(Cancellation::default());
        let worker_cancel = cancellation.clone();
        let library = self.library.clone();
        let acceleration = self.acceleration.clone();
        let thread = thread::Builder::new()
            .name("razer-macro-execution".into())
            .spawn(move || match execution {
                ExecutionProgram::Macro(execution) => run(
                    id,
                    execution,
                    library,
                    request.payload.driver_route,
                    worker_cancel,
                    clipboard,
                    acceleration,
                ),
                ExecutionProgram::Advanced(execution) => advanced::run(
                    id,
                    execution,
                    request.payload.driver_route,
                    worker_cancel,
                    clipboard,
                    acceleration,
                ),
            })
            .context("Start macro execution worker")?;
        self.completion_callers.insert(id, request.caller);
        self.running.insert(
            id,
            Running {
                caller: request.caller,
                cancellation,
                thread,
            },
        );
        Ok(())
    }

    fn launch_queued(&mut self, mut next: Option<Dispatch<Parameters>>) {
        while let Some(dispatch) = next {
            let id = dispatch.id;
            match self.spawn(dispatch) {
                Ok(()) => return,
                Err(error) => self.pending_completions.push((id, Err(error))),
            }
            match self.scheduler.complete(id) {
                Ok(dispatch) => next = dispatch,
                Err(error) => {
                    self.pending_completions.push((id, Err(error)));
                    return;
                }
            }
        }
    }

    pub fn cancel_caller(&mut self, caller: Uuid) -> Result<()> {
        let mut errors = Vec::new();
        for id in self.scheduler.stop_matching(caller, |_| true) {
            if let Err(error) = self.cancel(id) {
                errors.push(format!("Macro cancellation {id}: {error:#}"));
            }
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }

    pub fn interrupt_macro(
        &mut self,
        caller: Uuid,
        mapped_key: &razer_device::mapping_native_lookup::Resolved<
            razer_device::mapping_native_action::Output,
        >,
    ) -> Result<()> {
        // 5D890 removes queued executions and cancels current/running entries
        // only when their device and full native mapped-key identity match.
        let ids = self.scheduler.stop_matching(caller, |parameters| {
            parameters.mapped_key.as_ref().is_some_and(|running| {
                razer_device::mapping_native_action::matches_interrupted_macro(running, mapped_key)
            })
        });
        let mut errors = Vec::new();
        for id in ids {
            if let Err(error) = self.cancel(id) {
                errors.push(format!("{error:#}"));
            }
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }

    fn clipboard_client(&mut self) -> Result<clipboard::Client> {
        if self.clipboard.is_none() {
            self.clipboard = Some(clipboard::Service::start()?);
        }
        Ok(self.clipboard.as_ref().unwrap().client())
    }

    pub fn drain_caller(&mut self, caller: Uuid) -> Result<()> {
        let mut errors = Vec::new();
        if let Err(error) = self.cancel_caller(caller) {
            errors.push(format!("{error:#}"));
        }
        // Real workers remain owned even if the scheduler lost their identities.
        // Signal all of them before joining any one cancellation barrier.
        for run in self.running.values().filter(|run| run.caller == caller) {
            run.cancellation.cancel();
        }
        let ids = self
            .running
            .iter()
            .filter_map(|(id, run)| (run.caller == caller).then_some(*id))
            .collect::<Vec<_>>();
        for id in ids {
            let run = self.running.remove(&id).unwrap();
            let result = join(run.thread);
            if let Err(error) = &result {
                errors.push(format!("Macro execution {id}: {error:#}"));
            }
            if let Ok(finished) = &result {
                errors.extend(finished.completion.cleanup_errors.iter().cloned());
            }
            self.pending_completions.push((id, result));
            match self.scheduler.complete(id) {
                Ok(next) => self.launch_queued(next),
                Err(error) => errors.push(format!("Macro scheduler completion: {error:#}")),
            }
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }

    pub fn cancel(&self, id: Uuid) -> Result<()> {
        self.running
            .get(&id)
            .context("Macro execution owner changed")?
            .cancellation
            .cancel();
        Ok(())
    }

    pub fn take_completed(&mut self) -> Vec<(Uuid, Result<Finished>)> {
        let finished = self
            .running
            .iter()
            .filter_map(|(id, run)| run.thread.is_finished().then_some(*id))
            .collect::<Vec<_>>();
        let mut completions = std::mem::take(&mut self.pending_completions);
        for id in finished {
            let run = self.running.remove(&id).unwrap();
            let result = join(run.thread);
            match self.scheduler.complete(id) {
                Ok(next) => self.launch_queued(next),
                Err(error) => self.pending_completions.push((id, Err(error))),
            }
            completions.push((id, result));
        }
        completions.append(&mut self.pending_completions);
        completions
    }

    pub fn shutdown(&mut self) -> Shutdown {
        self.stopping = true;
        self.scheduler.begin_drain();
        for run in self.running.values() {
            run.cancellation.cancel();
        }
        let mut cleanup_errors = Vec::new();
        let mut executions = std::mem::take(&mut self.pending_completions);
        for (id, run) in std::mem::take(&mut self.running) {
            executions.push((id, join(run.thread)));
            if let Err(error) = self.scheduler.complete(id) {
                cleanup_errors.push(format!("Macro scheduling cleanup: {error:#}"));
            }
        }
        if !self.scheduler.is_idle() {
            cleanup_errors.push("Macro scheduling cleanup still has active owners".into());
        }
        if let Some(mut service) = self.clipboard.take() {
            if let Err(error) = service.shutdown() {
                cleanup_errors.push(format!("Macro clipboard cleanup failed: {error:#}"));
            }
        }
        match self.library.write() {
            Ok(mut library) => library.clear(),
            Err(error) => {
                error.into_inner().clear();
                cleanup_errors.push("Macro library lock was poisoned during cleanup".into());
            }
        }
        match self.advanced.write() {
            Ok(mut library) => library.clear(),
            Err(error) => {
                error.into_inner().clear();
                cleanup_errors
                    .push("Advanced macro library lock was poisoned during cleanup".into());
            }
        }
        self.completion_callers.clear();
        Shutdown {
            executions,
            cleanup_errors,
        }
    }
}

#[cfg(test)]
#[path = "mapping_runtime/lifecycle_tests.rs"]
mod lifecycle_tests;
impl Drop for State {
    fn drop(&mut self) {
        let result = self.shutdown();
        for error in result.cleanup_errors {
            eprintln!("{error}");
        }
        for (id, result) in result.executions {
            if let Err(error) = result {
                eprintln!("Macro {id} cleanup failed: {error:#}");
            }
        }
    }
}

fn join(thread: JoinHandle<Result<Finished>>) -> Result<Finished> {
    thread
        .join()
        .map_err(|_| anyhow::anyhow!("Macro execution worker panicked"))?
}

fn timing() -> KeyboardTiming {
    fn query(action: u32) -> Option<u32> {
        let mut value = 0u32;
        (unsafe { SystemParametersInfoW(action, 0, (&mut value as *mut u32).cast(), 0) } != 0)
            .then_some(value)
    }
    KeyboardTiming::from_system_parameters(query(0x16), query(0x0a))
}

fn random(min: u32, max: u32) -> Result<u32> {
    let width = u64::from(max) - u64::from(min) + 1;
    let limit = (1u64 << 32) / width * width;
    loop {
        let mut bytes = [0; 4];
        let status = unsafe {
            BCryptGenRandom(
                std::ptr::null_mut(),
                bytes.as_mut_ptr(),
                4,
                BCRYPT_USE_SYSTEM_PREFERRED_RNG,
            )
        };
        ensure!(status >= 0, "Macro random source failed: {status:#x}");
        let value = u64::from(u32::from_le_bytes(bytes));
        if value < limit {
            return Ok((u64::from(min) + value % width) as u32);
        }
    }
}

fn run(
    id: Uuid,
    mut execution: Execution,
    library: Arc<RwLock<Library>>,
    driver_route: Option<driver::Route>,
    cancel: Arc<Cancellation>,
    clipboard: clipboard::Client,
    acceleration: acceleration::Owner,
) -> Result<Finished> {
    let started = Instant::now();
    let mut input = input::Input::open(driver_route.as_ref(), cancel.clone(), acceleration)?;
    let mut keyboard = KeyboardExecutor::new(timing());
    let mut audio_observations = Vec::new();
    let mut timer_errors = Vec::new();
    let mut omitted_timer_errors = 0u64;
    let tick = || u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<Completion> {
            loop {
                if cancel.is_cancelled() {
                    execution.cancel();
                }
                let now = tick();
                for error in keyboard.poll(now, &mut |key| input.keyboard(key)) {
                    if timer_errors.len() < 64 {
                        timer_errors.push(error);
                    } else {
                        omitted_timer_errors = omitted_timer_errors.saturating_add(1);
                    }
                }
                let state = {
                    let library = match library.read() {
                        Ok(library) => library,
                        Err(error) => {
                            execution.fail("Macro library lock poisoned".into())?;
                            error.into_inner()
                        }
                    };
                    execution.poll(now, &library, &mut random)
                };
                match state {
                    Poll::Action(pending) => {
                        input.cancellable = !pending.cleanup;
                        let result = match pending.action {
                            Action::Keyboard(key) => keyboard
                                .execute(key, false, tick(), &mut |key| input.keyboard(key))
                                .map_err(anyhow::Error::msg),
                            Action::Mouse(mouse) => {
                                input.send(razer_device::mapping_native_input::NativeInput::Mouse(
                                    mouse.input(),
                                ))
                            }
                            Action::MouseMove {
                                mode,
                                x,
                                y,
                                origin_epoch,
                            } => input.movement(mode, x, y, origin_epoch),
                            Action::Launch(launch) => launch::execute(&launch),
                            Action::Clipboard(text) => clipboard.paste(text, driver_route.clone()),
                            Action::Audio(action) => audio::execute(action).and_then(|report| {
                                audio_observations = report.observations;
                                ensure!(report.errors.is_empty(), "{}", report.errors.join("; "));
                                Ok(())
                            }),
                        };
                        execution.acknowledge(
                            pending.ticket,
                            result.map_err(|error| format!("{error:#}")),
                        )?;
                    }
                    Poll::WaitingUntil(deadline) => {
                        let deadline = keyboard
                            .next_deadline_us()
                            .map_or(deadline, |key| key.min(deadline));
                        cancel.wait(Duration::from_micros(deadline.saturating_sub(tick())));
                    }
                    Poll::Progress => thread::yield_now(),
                    Poll::WaitingForCompletion => {
                        anyhow::bail!("Macro output completion owner was lost")
                    }
                    Poll::Complete(completion) => return Ok(completion),
                }
            }
        }))
        .unwrap_or_else(|_| Err(anyhow::anyhow!("Macro execution owner panicked")));
    let cleanup = input.close();
    keyboard.stop();
    let mut completion = match result {
        Ok(completion) => completion,
        Err(error) => {
            return match cleanup {
                Ok(()) => Err(error),
                Err(cleanup) => Err(anyhow::anyhow!("{error:#}; input cleanup: {cleanup:#}")),
            };
        }
    };
    if let Err(error) = cleanup {
        completion.cleanup_errors.push(format!("{error:#}"));
    }

    for error in input.operation_errors.drain(..).chain(timer_errors) {
        completion.record_operation_error(error);
    }
    completion.omitted_operation_errors = completion
        .omitted_operation_errors
        .saturating_add(omitted_timer_errors);
    Ok(Finished {
        id,
        completion,
        last_input: input.last_receipt.take(),
        completed_inputs: input.completed,
        audio_observations,
    })
}
