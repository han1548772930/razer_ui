//! Retained Matcher worker. GUI owners exchange snapshots and durable-save
//! receipts; all input capture, service calls and waits stay on this thread.
use super::{Backend, Controller};
use anyhow::{Context as _, ensure};
use razer_device::{
    dpi_matcher::{InputBatch, State},
    mouse_current_dpi::DpiChange,
};
use razer_discovery::{direct, discovery::ObservedDevice};
use razer_ipc::{ServiceClient, ServiceRequest};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[path = "dpi_matcher_input.rs"]
mod input;
use input::InputMonitor;

#[derive(Clone, Debug)]
pub enum Command {
    GetState,
    Start,
    Cancel,
    Reset,
    Confirm,
    DeleteProfile(String),
    DeleteAllProfiles,
    Select(String),
    UpdateProfiles(Value),
}

impl Command {
    fn requires_unlocked_device(&self) -> bool {
        matches!(
            self,
            Self::Select(_)
                | Self::UpdateProfiles(_)
                | Self::DeleteProfile(_)
                | Self::DeleteAllProfiles
        )
    }
}

struct CommandQueue {
    receiver: Receiver<Command>,
    deferred: VecDeque<Command>,
}

impl CommandQueue {
    fn next(&mut self, locked: bool) -> Result<Command, mpsc::RecvTimeoutError> {
        if !locked && let Some(command) = self.deferred.pop_front() {
            return Ok(command);
        }
        self.receiver.recv_timeout(Duration::from_millis(16))
    }

    fn defer(&mut self, command: Command) -> anyhow::Result<()> {
        ensure!(
            self.deferred.len() < 64,
            "Matcher deferred command queue is full"
        );
        self.deferred.push_back(command);
        Ok(())
    }

    fn drain_next(&mut self) -> Option<Command> {
        self.deferred
            .pop_front()
            .or_else(|| self.receiver.try_recv().ok())
    }
}

pub enum Update {
    ReadSource(Sender<Result<razer_model::mouse_matcher::SourceRecord, String>>),
    Persist {
        mutation: razer_model::mouse_matcher::Mutation,
        reply: Sender<Result<(), String>>,
    },
    Response(Value),
    StageDraft(Value),
    Failure(String),
}

pub struct Updates(async_channel::Receiver<Update>);
impl Updates {
    /// `None` is an idle inbox; `Err` means the retained worker has finished.
    pub fn try_next(&self) -> Result<Option<Update>, UpdatesClosed> {
        match self.0.try_recv() {
            Ok(update) => Ok(Some(update)),
            Err(async_channel::TryRecvError::Empty) => Ok(None),
            Err(async_channel::TryRecvError::Closed) => Err(UpdatesClosed),
        }
    }
}
pub struct UpdatesClosed;

#[derive(Clone, Debug)]
pub struct ConnectedDevice {
    product_id: u32,
    container_id: String,
    keyboard: bool,
    english_name: Option<String>,
    chinese_name: Option<String>,
}
impl ConnectedDevice {
    pub fn from_device(device: &razer_model::model::Device) -> Self {
        Self {
            product_id: device.product_id,
            container_id: device.device_container_id.clone(),
            keyboard: device.category == razer_model::model::DeviceCategory::Keyboard,
            english_name: device.product_name.exact("en").map(str::to_owned),
            chinese_name: device.product_name.exact("zh-cn").map(str::to_owned),
        }
    }
}

pub struct Options {
    route: ObservedDevice,
    connected: Vec<ConnectedDevice>,
    pairing_records: Vec<PairingIdentity>,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PairingIdentity {
    product_id: u32,
    device_container_id: String,
    master: MasterIdentity,
}
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct MasterIdentity {
    product_id: u32,
    dongle_id: u32,
}

impl Options {
    pub fn new(
        route: ObservedDevice,
        connected: Vec<ConnectedDevice>,
        pairing: Option<&Value>,
    ) -> anyhow::Result<Self> {
        ensure!(
            route.product_id() == 190 && direct::mouse_current_dpi::available(&route),
            "Matcher has no current PID190 DPI route"
        );
        let pairing_records = match pairing {
            Some(value) => value
                .as_object()
                .context("Matcher pairing records is not an object")?
                .values()
                .cloned()
                .map(serde_json::from_value)
                .collect::<Result<Vec<_>, _>>()?,
            None => vec![],
        };
        Ok(Self {
            route,
            connected,
            pairing_records,
        })
    }

    fn is_initial_main(&self, product_id: u32) -> bool {
        let Some(cap) = razer_device::mouse_current_dpi::capability(190) else {
            return false;
        };
        product_id == cap.wired_pid()
            || cap.dongle_pid() == Some(product_id)
            || self.pairing_records.iter().any(|record| {
                record.master.dongle_id == product_id
                    && record.device_container_id == self.route.container()
            })
    }

    fn confirmation_name(&self, container: &str) -> anyhow::Result<Value> {
        let mut device = self
            .connected
            .iter()
            .find(|device| device.container_id == container);
        if let Some(source) =
            device.filter(|device| matches!(device.product_id, 179 | 164) || device.keyboard)
        {
            if let Some(record) = self
                .pairing_records
                .iter()
                .find(|record| record.master.product_id == source.product_id)
            {
                device = self
                    .connected
                    .iter()
                    .find(|device| device.product_id == record.product_id);
            }
        }
        let computer = razer_platform::system::computer_name()?;
        let english = device
            .and_then(|device| device.english_name.as_deref())
            .unwrap_or(&computer);
        let chinese = device
            .and_then(|device| device.chinese_name.as_deref())
            .unwrap_or(english);
        // 32807 returns this locale object through W$ without flattening it.
        Ok(json!({"en":english,"zh-cn":chinese}))
    }
}

pub struct Handle {
    commands: SyncSender<Command>,
    canceled: Arc<AtomicBool>,
    closing: Arc<AtomicBool>,
    worker: Option<JoinHandle<WorkerExit>>,
    completion: Completion,
}

struct WorkerExit {
    operation: anyhow::Result<()>,
    cleanup: anyhow::Result<()>,
    released: anyhow::Result<()>,
}

fn merge_results(results: impl IntoIterator<Item = anyhow::Result<()>>) -> anyhow::Result<()> {
    let errors = results
        .into_iter()
        .filter_map(Result::err)
        .map(|error| format!("{error:#}"))
        .collect::<Vec<_>>();
    ensure!(errors.is_empty(), "{}", errors.join("; "));
    Ok(())
}

#[derive(Clone, Default)]
pub struct Completion(Arc<CompletedWorker>);

#[derive(Default)]
struct CompletedWorker {
    result: Mutex<Option<CleanupOutcome>>,
    finished: AtomicBool,
    released: AtomicBool,
    ready: Condvar,
}

struct CleanupOutcome {
    cleanup: Result<(), String>,
    released: Result<(), String>,
}

impl Completion {
    pub fn is_finished(&self) -> bool {
        self.0.finished.load(Ordering::Acquire)
    }

    pub fn is_released(&self) -> bool {
        self.0.released.load(Ordering::Acquire)
    }

    pub fn cleanup_error(&self) -> Option<String> {
        let outcome = self
            .0
            .result
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let outcome = outcome.as_ref()?;
        let errors = [&outcome.cleanup, &outcome.released]
            .into_iter()
            .filter_map(|result| result.as_ref().err())
            .cloned()
            .collect::<Vec<_>>();
        (!errors.is_empty()).then(|| errors.join("; "))
    }

    fn publish(&self, cleanup: &anyhow::Result<()>, released: &anyhow::Result<()>) {
        let mut outcome = self
            .0
            .result
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let stringify = |result: &anyhow::Result<()>| {
            result
                .as_ref()
                .map(|_| ())
                .map_err(|error| format!("{error:#}"))
        };
        *outcome = Some(CleanupOutcome {
            cleanup: stringify(cleanup),
            released: stringify(released),
        });
        self.0.released.store(released.is_ok(), Ordering::Release);
        self.0.finished.store(true, Ordering::Release);
        self.0.ready.notify_all();
    }

    fn wait(&self, mut canceled: impl FnMut() -> bool) -> anyhow::Result<bool> {
        let mut outcome = self
            .0
            .result
            .lock()
            .map_err(|_| anyhow::anyhow!("Matcher cleanup result poisoned"))?;
        loop {
            if canceled() {
                return Ok(false);
            }
            if let Some(result) = outcome.as_ref() {
                result
                    .released
                    .as_ref()
                    .map_err(|error| anyhow::anyhow!("Prior Matcher cleanup failed: {error}"))?;
                return Ok(true);
            }
            (outcome, _) = self
                .0
                .ready
                .wait_timeout(outcome, Duration::from_millis(16))
                .map_err(|_| anyhow::anyhow!("Matcher cleanup result poisoned while waiting"))?;
        }
    }
}

impl Handle {
    /// Prior workers must finish native stop, shutdown and thread joining before
    /// this replacement may request the same input/device leases.
    pub fn start(options: Options, predecessors: Vec<Completion>) -> (Self, Updates) {
        Self::spawn(predecessors, move |commands, updates, canceled, closing| {
            run(options, commands, updates, canceled, closing)
        })
    }

    fn spawn(
        predecessors: Vec<Completion>,
        run: impl FnOnce(
            Receiver<Command>,
            async_channel::Sender<Update>,
            Arc<AtomicBool>,
            Arc<AtomicBool>,
        ) -> WorkerExit
        + Send
        + 'static,
    ) -> (Self, Updates) {
        let (commands, receiver) = mpsc::sync_channel(64);
        let (updates, output) = async_channel::bounded(64);
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        let closing = Arc::new(AtomicBool::new(false));
        let drain = closing.clone();
        let worker = thread::spawn(move || {
            for predecessor in predecessors {
                match predecessor.wait(|| signal.load(Ordering::Acquire)) {
                    Ok(true) => {}
                    Ok(false) => {
                        return WorkerExit {
                            operation: Ok(()),
                            cleanup: Ok(()),
                            released: Ok(()),
                        };
                    }
                    Err(error) => {
                        let _ = updates.try_send(Update::Failure(format!("{error:#}")));
                        return WorkerExit {
                            operation: Ok(()),
                            cleanup: Ok(()),
                            released: Err(error),
                        };
                    }
                }
            }
            run(receiver, updates, signal, drain)
        });
        (
            Self {
                commands,
                canceled,
                closing,
                worker: Some(worker),
                completion: Completion::default(),
            },
            Updates(output),
        )
    }

    pub fn send(&self, command: Command) -> anyhow::Result<()> {
        ensure!(
            !self.canceled.load(Ordering::Acquire) && !self.closing.load(Ordering::Acquire),
            "Matcher owner was canceled"
        );
        self.commands
            .try_send(command)
            .context("Matcher command queue is full or its worker has stopped")
    }

    pub fn cancel(&self) {
        self.canceled.store(true, Ordering::Release);
    }

    /// Stop admission while allowing accepted commands and save receipts to
    /// finish. Capture stops at the next command boundary; no result is saved
    /// unless an already accepted command requested it.
    pub fn begin_exit(&mut self) {
        self.closing.store(true, Ordering::Release);
    }

    pub fn completion(&self) -> Completion {
        self.completion.clone()
    }

    /// Join only on a background executor, after asking the owner to stop.
    pub fn finish(mut self) -> anyhow::Result<()> {
        self.cancel();
        let joined = self
            .worker
            .take()
            .context("Matcher worker was already joined")?
            .join()
            .map_err(|_| anyhow::anyhow!("Matcher worker panicked"));
        match joined {
            Ok(exit) => {
                self.completion.publish(&exit.cleanup, &exit.released);
                merge_results([exit.operation, exit.cleanup, exit.released])
            }
            Err(error) => {
                let result = Err(error);
                self.completion.publish(&result, &result);
                result
            }
        }
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        self.cancel();
    }
}

struct NativeBackend<'a> {
    options: &'a Options,
    client: Option<ServiceClient>,
    updates: &'a async_channel::Sender<Update>,
    canceled: &'a AtomicBool,
    source: Option<razer_model::mouse_matcher::SourceRecord>,
    monitor: InputMonitor,
    lifecycle: Option<std::sync::MutexGuard<'static, ()>>,
    device_task: Option<razer_ipc::device_tasks::Lease>,
}
impl NativeBackend<'_> {
    fn check(&self) -> anyhow::Result<()> {
        ensure!(
            !self.canceled.load(Ordering::Acquire),
            "Matcher owner was canceled"
        );
        Ok(())
    }

    fn emit(&self, mut update: Update) -> anyhow::Result<()> {
        loop {
            self.check()?;
            match self.updates.try_send(update) {
                Ok(()) => return Ok(()),
                Err(async_channel::TrySendError::Closed(_)) => {
                    anyhow::bail!("Matcher page observer closed")
                }
                Err(async_channel::TrySendError::Full(pending)) => update = pending,
            }
            thread::sleep(Duration::from_millis(16));
        }
    }

    fn wait<T>(&self, reply: Receiver<Result<T, String>>) -> anyhow::Result<T> {
        loop {
            self.check()?;
            match reply.recv_timeout(Duration::from_millis(25)) {
                Ok(result) => return result.map_err(anyhow::Error::msg),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    anyhow::bail!("Matcher page response owner closed")
                }
            }
        }
    }

    fn client(&mut self) -> anyhow::Result<&mut ServiceClient> {
        if self.client.is_none() {
            self.client = Some(ServiceClient::spawn()?);
        }
        self.client
            .as_mut()
            .context("Matcher native service is absent")
    }

    fn events(&mut self) -> anyhow::Result<Vec<InputBatch>> {
        self.client()?;
        let client = self
            .client
            .as_mut()
            .context("Matcher native service is absent")?;
        self.monitor.events(|request| client.request(request))
    }

    fn shutdown(&mut self) -> anyhow::Result<()> {
        let result = match &mut self.client {
            Some(client) if !client.is_stopped() => {
                #[derive(serde::Deserialize)]
                struct Shutdown {
                    stopped: bool,
                }
                let receipt: Shutdown =
                    serde_json::from_value(client.request(ServiceRequest::Shutdown)?)?;
                ensure!(
                    receipt.stopped,
                    "Matcher service cleanup was not acknowledged"
                );
                Ok(())
            }
            _ => Ok(()),
        };
        result
    }

    fn release_transport(&mut self) -> anyhow::Result<()> {
        if let Some(client) = self.client.as_mut() {
            client.finish_termination()?;
        }
        self.client.take();
        self.device_task.take();
        self.lifecycle.take();
        Ok(())
    }
}

impl Backend for NativeBackend<'_> {
    fn read_source_data(&mut self) -> anyhow::Result<Value> {
        let (send, reply) = mpsc::channel();
        self.emit(Update::ReadSource(send))?;
        let source = self.wait(reply)?;
        let data = source.document().clone();
        self.source = Some(source);
        Ok(data)
    }

    fn start_input_monitoring(&mut self) -> anyhow::Result<()> {
        self.check()?;
        // Focused capture, software recording and Matcher share this actual
        // native input lifecycle. An unrelated owner is an immediate conflict.
        self.lifecycle = Some(razer_ipc::KEY_CAPTURE_LIFECYCLE.try_lock().map_err(|_| {
            anyhow::anyhow!("MACRO_RECORDING_RUNNING: input capture lifecycle is occupied")
        })?);
        self.client()?;
        let client = self
            .client
            .as_mut()
            .context("Matcher native service is absent")?;
        self.monitor.start(|request| client.request(request))
    }

    fn stop_input_monitoring(&mut self) -> anyhow::Result<()> {
        if self.monitor.possibly_started() {
            self.client()?;
            let client = self
                .client
                .as_mut()
                .context("Matcher native service is absent")?;
            self.monitor.stop(|request| client.request(request))?;
        }
        Ok(())
    }

    fn acquire_matcher_locks(&mut self) -> anyhow::Result<()> {
        self.check()?;
        ensure!(
            self.monitor.possibly_started() && self.lifecycle.is_some(),
            "Matcher capture lease is absent"
        );
        let scope = self.options.route.device_task_scope()?;
        self.device_task = Some(razer_ipc::device_tasks::Registry::global().try_exclusive(scope)?);
        Ok(())
    }

    fn release_matcher_locks(&mut self) -> anyhow::Result<()> {
        ensure!(
            !self.monitor.possibly_started(),
            "Matcher monitor has not acknowledged stop"
        );
        self.device_task.take();
        self.lifecycle.take();
        Ok(())
    }

    fn submit_dpi_stages(&mut self, stages: &Value) -> anyhow::Result<()> {
        #[derive(serde::Deserialize)]
        struct Stages {
            stages: Vec<razer_device::mouse_dpi_stages::DpiStage>,
            active: usize,
            enable: bool,
        }
        self.check()?;
        self.emit(Update::StageDraft(stages.clone()))?;
        let stages: Stages = serde_json::from_value(stages.clone())?;
        let draft = razer_device::mouse_dpi_stages::DpiStagesDraft {
            enabled: stages.enable,
            active_stage: stages.active,
            stages: stages.stages,
        };
        let change = DpiChange {
            stages: Some(draft),
            active: None,
            profile_guid: None,
            initialize: false,
        };
        let route = self.options.route.clone();
        let _device_task = razer_ipc::device_tasks::Registry::global()
            .shared(route.device_task_scope()?, || {
                self.canceled.load(Ordering::Acquire)
            })?;
        let report = direct::mouse_current_dpi::request(self.client()?, &route, Some(change))?;
        ensure!(
            !report.acknowledgements.is_empty(),
            "Matcher DPI setter has no device acknowledgement"
        );
        self.check()
    }

    fn persist_source_data(&mut self, data: &Value) -> anyhow::Result<()> {
        let before = self
            .source
            .as_ref()
            .context("Matcher has no observed storage revision")?;
        let mutation = razer_model::mouse_matcher::Mutation::from_source(before, data)?;
        let (send, reply) = mpsc::channel();
        self.emit(Update::Persist {
            mutation,
            reply: send,
        })?;
        self.wait(reply)?;
        Ok(())
    }

    fn confirmation_identity(
        &mut self,
        source_container: &str,
        _: Option<u32>,
    ) -> anyhow::Result<(String, Value)> {
        self.check()?;
        Ok((
            uuid::Uuid::new_v4().to_string(),
            self.options.confirmation_name(source_container)?,
        ))
    }
}

fn run(
    options: Options,
    commands: Receiver<Command>,
    updates: async_channel::Sender<Update>,
    canceled: Arc<AtomicBool>,
    closing: Arc<AtomicBool>,
) -> WorkerExit {
    let mut backend = NativeBackend {
        options: &options,
        client: None,
        updates: &updates,
        canceled: &canceled,
        source: None,
        monitor: InputMonitor::new(uuid::Uuid::new_v4().to_string()),
        lifecycle: None,
        device_task: None,
    };
    let mut controller = Controller::default();
    let mut commands = CommandQueue {
        receiver: commands,
        deferred: VecDeque::new(),
    };
    let started = Instant::now();
    let run = (|| -> anyhow::Result<()> {
        while !canceled.load(Ordering::Acquire) {
            let draining = closing.load(Ordering::Acquire);
            if draining {
                controller.cleanup(&mut backend)?;
                backend.stop_input_monitoring()?;
            }
            // Source ordinary tasks wait for the exclusive calibration lock.
            // Keep them queued while continuing input polling and cancellation;
            // blocking this same owner on a shared lease would deadlock it.
            let command = if draining {
                match commands.drain_next() {
                    Some(Command::Start) => continue,
                    Some(command) => Ok(command),
                    None => break,
                }
            } else {
                commands.next(controller.locked)
            };
            let starting = matches!(&command, Ok(Command::Start));
            let result = match command {
                Ok(command) if controller.locked && command.requires_unlocked_device() => {
                    if let Err(error) = commands.defer(command) {
                        backend.emit(Update::Failure(format!("{error:#}")))?;
                    }
                    Ok(vec![])
                }
                Ok(Command::Select(guid)) => controller.select(&guid, &mut backend),
                Ok(Command::UpdateProfiles(payload)) => {
                    controller.update_profiles(&payload, &mut backend)
                }
                Ok(command) => {
                    let (kind, payload) = match command {
                        Command::GetState => ("GET_STATE", None),
                        Command::Start => ("START", None),
                        Command::Cancel => ("CANCEL", None),
                        Command::Reset => ("RESET", None),
                        Command::Confirm => ("CONFIRM", None),
                        Command::DeleteProfile(guid) => ("DELETE_PROFILE", Some(json!(guid))),
                        Command::DeleteAllProfiles => ("DELETE_ALL_PROFILES", None),
                        Command::Select(_) | Command::UpdateProfiles(_) => unreachable!(),
                    };
                    controller.command(kind, payload.as_ref(), &mut backend)
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => Ok(vec![]),
            };
            match result {
                Ok(responses) => {
                    for response in responses {
                        backend.emit(Update::Response(response))?;
                    }
                }
                Err(error) => {
                    backend.emit(Update::Failure(format!("{error:#}")))?;
                    if !controller.monitoring && backend.monitor.possibly_started() {
                        backend.stop_input_monitoring()?;
                    }
                    if !controller.monitoring {
                        backend.release_matcher_locks()?;
                    }
                    if starting {
                        backend.emit(Update::Response(controller.response()))?;
                    }
                }
            }
            if !draining && controller.state() == State::Calibrating {
                for batch in backend.events()? {
                    let initial_is_main = options.is_initial_main(batch.product_id);
                    let response = controller.ingest(
                        batch,
                        initial_is_main,
                        elapsed_ms(started),
                        &mut backend,
                    )?;
                    backend.emit(Update::Response(response))?;
                    if controller.state() != State::Calibrating {
                        break;
                    }
                }
                if let Some(response) = controller.tick(elapsed_ms(started), &mut backend)? {
                    backend.emit(Update::Response(response))?;
                }
            }
        }
        Ok(())
    })();
    if let Err(error) = &run {
        let _ = backend.emit(Update::Failure(format!("{error:#}")));
    }
    // A partially started native monitor is also stopped. Its flags can outlive
    // the portable coordinator's successful-start flag after an ACK failure.
    let cleanup = controller.cleanup(&mut backend);
    let partial = backend.stop_input_monitoring();
    let shutdown = backend.shutdown();
    let released = backend.release_transport();
    WorkerExit {
        operation: run,
        cleanup: merge_results([cleanup, partial, shutdown]),
        released,
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpi_matcher_queued_edits_preserve_order_without_blocking_cancel() {
        let (send, receiver) = mpsc::channel();
        let mut commands = CommandQueue {
            receiver,
            deferred: VecDeque::new(),
        };
        send.send(Command::Select("first".into())).unwrap();
        send.send(Command::DeleteProfile("second".into())).unwrap();
        send.send(Command::Cancel).unwrap();
        for _ in 0..2 {
            let command = commands.next(true).unwrap();
            assert!(command.requires_unlocked_device());
            commands.defer(command).unwrap();
        }
        assert!(matches!(commands.next(true).unwrap(), Command::Cancel));
        assert!(matches!(commands.next(false).unwrap(), Command::Select(guid) if guid == "first"));
        assert!(
            matches!(commands.next(false).unwrap(), Command::DeleteProfile(guid) if guid == "second")
        );
        for _ in 0..64 {
            commands.defer(Command::DeleteAllProfiles).unwrap();
        }
        assert!(commands.defer(Command::DeleteAllProfiles).is_err());
        send.send(Command::Reset).unwrap();
        assert!(matches!(commands.next(true).unwrap(), Command::Reset));
    }

    #[test]
    fn dpi_matcher_handoff_waits_for_cleanup_and_join_receipt() {
        let (entered, started) = mpsc::channel();
        let (release, stopped) = mpsc::channel();
        let (handle, _updates) = Handle::spawn(vec![], move |_, _, _| {
            entered.send(()).unwrap();
            stopped.recv().unwrap();
            WorkerExit {
                operation: Ok(()),
                cleanup: Ok(()),
                released: Ok(()),
            }
        });
        started.recv().unwrap();
        let completion = handle.completion();
        let retired = thread::spawn(move || handle.finish());
        let (checking, checked) = mpsc::channel();
        let (ready, observed) = mpsc::channel();
        let pending = completion.clone();
        let replacement = thread::spawn(move || {
            let mut checking = Some(checking);
            let accepted = pending
                .wait(|| {
                    if let Some(checking) = checking.take() {
                        checking.send(()).unwrap();
                    }
                    false
                })
                .unwrap();
            ready.send(accepted).unwrap();
        });
        checked.recv().unwrap();
        assert!(!completion.is_finished());
        assert!(observed.try_recv().is_err());
        release.send(()).unwrap();
        assert!(observed.recv().unwrap());
        replacement.join().unwrap();
        retired.join().unwrap().unwrap();
        assert!(completion.is_finished());
    }

    #[test]
    fn dpi_matcher_handoff_preserves_cleanup_failure_and_allows_wait_cancellation() {
        let pending = Completion::default();
        assert!(!pending.wait(|| true).unwrap());
        let failed = Completion::default();
        failed.publish(
            &Ok(()),
            &Err(anyhow::anyhow!("native stop not acknowledged")),
        );
        let (handle, updates) = Handle::spawn(vec![failed], |_, _, _| {
            panic!("failed cleanup must reject replacement")
        });
        let update = updates.0.recv_blocking().unwrap();
        assert!(
            matches!(update, Update::Failure(error) if error.contains("native stop not acknowledged"))
        );
        assert!(
            handle
                .finish()
                .unwrap_err()
                .to_string()
                .contains("native stop not acknowledged")
        );
    }

    #[test]
    fn dpi_matcher_canceled_operation_does_not_block_a_clean_replacement() {
        let (handle, _updates) = Handle::spawn(vec![], |_, _, _| WorkerExit {
            operation: Err(anyhow::anyhow!("owner canceled while awaiting source")),
            cleanup: Ok(()),
            released: Ok(()),
        });
        let completion = handle.completion();
        assert!(handle.finish().is_err());
        assert!(completion.cleanup_error().is_none());
        assert!(completion.wait(|| false).unwrap());
        let (accepted, observed) = mpsc::channel();
        let (replacement, _updates) = Handle::spawn(vec![completion], move |_, _, _| {
            accepted.send(()).unwrap();
            WorkerExit {
                operation: Ok(()),
                cleanup: Ok(()),
                released: Ok(()),
            }
        });
        observed.recv().unwrap();
        replacement.finish().unwrap();
    }

    #[test]
    fn dpi_matcher_cleanup_failure_survives_successful_transport_release() {
        let (handle, _updates) = Handle::spawn(vec![], |_, _, _| WorkerExit {
            operation: Ok(()),
            cleanup: Err(anyhow::anyhow!("native stop was not acknowledged")),
            released: Ok(()),
        });
        let completion = handle.completion();
        assert!(!completion.is_finished());
        assert!(completion.cleanup_error().is_none());
        assert!(handle.finish().is_err());
        assert!(completion.is_finished());
        assert!(completion.is_released());
        assert!(
            completion
                .cleanup_error()
                .unwrap()
                .contains("native stop was not acknowledged")
        );
        assert!(completion.wait(|| false).unwrap());
    }

    #[test]
    fn dpi_matcher_join_preserves_both_cleanup_and_transport_failures() {
        let (handle, _updates) = Handle::spawn(vec![], |_, _, _| WorkerExit {
            operation: Err(anyhow::anyhow!("measurement failed")),
            cleanup: Err(anyhow::anyhow!("monitor stop failed")),
            released: Err(anyhow::anyhow!("transport join failed")),
        });
        let completion = handle.completion();
        assert!(handle.finish().is_err());
        assert!(completion.is_finished());
        assert!(!completion.is_released());
        let cleanup = completion.cleanup_error().unwrap();
        assert!(cleanup.contains("monitor stop failed"));
        assert!(cleanup.contains("transport join failed"));
        assert!(!cleanup.contains("measurement failed"));
        assert!(completion.wait(|| false).is_err());
    }
}
