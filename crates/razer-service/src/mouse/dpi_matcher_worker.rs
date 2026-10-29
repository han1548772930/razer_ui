//! Retained Matcher worker. GUI owners exchange snapshots and durable-save
//! receipts; all input capture, service calls and waits stay on this thread.
use super::{Backend, Controller, Response};
use anyhow::{Context as _, ensure};
use razer_device::{
    dpi_matcher::{InputBatch, State},
    dpi_matcher_storage::{Change, ProfileName, ProfileUpdate, SourceData},
    mouse_current_dpi::SourceStages,
};
use razer_discovery::{direct, discovery::ObservedDevice};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_storage::receiver_pairing::PairingSnapshot;
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
    UpdateProfiles(ProfileUpdate),
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
        loop {
            let command = self
                .deferred
                .pop_front()
                .or_else(|| self.receiver.try_recv().ok())?;
            if !matches!(command, Command::Start) {
                return Some(command);
            }
        }
    }
}

pub enum Update {
    ReadSource(Sender<Result<razer_model::mouse_matcher::SourceRecord, String>>),
    Persist {
        mutation: razer_model::mouse_matcher::Mutation,
        reply: Sender<Result<razer_model::mouse_matcher::SourceRecord, String>>,
    },
    Response(Response),
    StageDraft(SourceStages),
    StageRequest {
        stages: SourceStages,
        reply: Sender<Result<(), String>>,
    },
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
    pairing_records: PairingSnapshot,
}

impl Options {
    pub fn new(
        route: ObservedDevice,
        connected: Vec<ConnectedDevice>,
        pairing: Option<&Value>,
    ) -> anyhow::Result<Self> {
        let product = razer_device::dpi_matcher::Product::new(route.product_id())?;
        ensure!(
            direct::mouse_current_dpi::available(&route),
            "Matcher has no current DPI route for logical PID{}",
            product.product_id()
        );
        let pairing_records = match pairing {
            Some(value) => PairingSnapshot::from_value(value)?,
            None => PairingSnapshot::default(),
        };
        Ok(Self {
            route,
            connected,
            pairing_records,
        })
    }

    fn is_initial_main(&self, product_id: u32) -> anyhow::Result<bool> {
        let product = razer_device::dpi_matcher::Product::new(self.route.product_id())?;
        Ok(product.accepts_initial_main_product(product_id)
            || self
                .pairing_records
                .has_input_monitoring_master(product_id, self.route.container())?)
    }

    fn confirmation_name(&self, container: &str) -> anyhow::Result<Value> {
        reference_name(
            razer_device::dpi_matcher::Product::new(self.route.product_id())?,
            &self.connected,
            &self.pairing_records,
            container,
            razer_platform::system::computer_name,
        )
    }
}

fn reference_name(
    product: razer_device::dpi_matcher::Product,
    connected: &[ConnectedDevice],
    pairing: &PairingSnapshot,
    container: &str,
    computer: impl FnOnce() -> anyhow::Result<String>,
) -> anyhow::Result<Value> {
    let mut device = connected
        .iter()
        .find(|device| device.container_id == container);
    if let Some(source) = device.filter(|device| {
        matches!(device.product_id, 179 | 164)
            || device.keyboard
            || product.is_dual_link_reference(device.product_id)
    }) {
        if let Some(paired) = pairing.paired_device_for_master(source.product_id)? {
            device = connected
                .iter()
                .find(|device| Some(device.product_id) == paired.product_id());
        }
    }
    let computer = computer()?;
    let english = device
        .and_then(|device| device.english_name.as_deref())
        .unwrap_or(&computer);
    let chinese = device
        .and_then(|device| device.chinese_name.as_deref())
        .unwrap_or(english);
    Ok(json!({"en":english,"zh-cn":chinese}))
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
                let errors = [&result.cleanup, &result.released]
                    .into_iter()
                    .filter_map(|result| result.as_ref().err())
                    .cloned()
                    .collect::<Vec<_>>();
                ensure!(
                    errors.is_empty(),
                    "Prior Matcher cleanup failed: {}",
                    errors.join("; ")
                );
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
        let product = razer_device::dpi_matcher::Product::new(options.route.product_id())
            .expect("Matcher options validate their logical product");
        Self::spawn(predecessors, move |commands, updates, canceled, closing| {
            run(Some(options), product, commands, updates, canceled, closing)
        })
    }

    /// Redux library normalization owns local persistence even without a mounted
    /// capture component or a current HID route. This owner never starts capture.
    pub fn start_profiles(
        product: razer_device::dpi_matcher::Product,
        predecessors: Vec<Completion>,
    ) -> (Self, Updates) {
        Self::spawn(predecessors, move |commands, updates, canceled, closing| {
            run(None, product, commands, updates, canceled, closing)
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
    options: Option<&'a Options>,
    client: Option<ServiceClient>,
    updates: &'a async_channel::Sender<Update>,
    canceled: &'a AtomicBool,
    live_source: Option<SourceData>,
    monitor: Option<InputMonitor>,
    lifecycle: Option<std::sync::MutexGuard<'static, ()>>,
    device_task: Option<razer_ipc::device_tasks::Lease>,
}
impl NativeBackend<'_> {
    fn monitor_started(&self) -> bool {
        self.monitor
            .as_ref()
            .is_some_and(InputMonitor::possibly_started)
    }
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
        wait_reply(reply, self.canceled)
    }

    fn client(&mut self) -> anyhow::Result<&mut ServiceClient> {
        ensure!(
            self.options.is_some(),
            "Matcher profile owner cannot open native input transport"
        );
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
        self.monitor
            .as_mut()
            .context("Matcher input monitor is absent")?
            .events(|request| client.request(request))
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

fn wait_reply<T>(reply: Receiver<Result<T, String>>, canceled: &AtomicBool) -> anyhow::Result<T> {
    loop {
        ensure!(
            !canceled.load(Ordering::Acquire),
            "Matcher owner was canceled"
        );
        match reply.recv_timeout(Duration::from_millis(25)) {
            Ok(result) => return result.map_err(anyhow::Error::msg),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                anyhow::bail!("Matcher page response owner closed")
            }
        }
    }
}

impl Backend for NativeBackend<'_> {
    fn read_source_data(&mut self) -> anyhow::Result<SourceData> {
        let (send, reply) = mpsc::channel();
        self.emit(Update::ReadSource(send))?;
        let source = self.wait(reply)?;
        let mut data = SourceData::from_document(source.document())?;
        if let Some(previous) = &self.live_source {
            data.retain_live(previous);
        }
        self.live_source = Some(data.clone());
        Ok(data)
    }

    fn start_input_monitoring(&mut self) -> anyhow::Result<()> {
        self.check()?;
        ensure!(
            self.options.is_some() && self.monitor.is_some(),
            "Matcher profile owner cannot start input capture"
        );
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
        self.monitor
            .as_mut()
            .context("Matcher input monitor is absent")?
            .start(|request| client.request(request))
    }

    fn stop_input_monitoring(&mut self) -> anyhow::Result<()> {
        if self.monitor_started() {
            self.client()?;
            let client = self
                .client
                .as_mut()
                .context("Matcher native service is absent")?;
            self.monitor
                .as_mut()
                .context("Matcher input monitor is absent")?
                .stop(|request| client.request(request))?;
        }
        Ok(())
    }

    fn acquire_matcher_locks(&mut self) -> anyhow::Result<()> {
        self.check()?;
        ensure!(
            self.monitor_started() && self.lifecycle.is_some(),
            "Matcher capture lease is absent"
        );
        let scope = self
            .options
            .context("Matcher capture options are absent")?
            .route
            .device_task_scope()?;
        self.device_task = Some(razer_ipc::device_tasks::Registry::global().try_exclusive(scope)?);
        Ok(())
    }

    fn release_matcher_locks(&mut self) -> anyhow::Result<()> {
        ensure!(
            !self.monitor_started(),
            "Matcher monitor has not acknowledged stop"
        );
        self.device_task.take();
        self.lifecycle.take();
        Ok(())
    }

    fn submit_dpi_stages(&mut self, stages: &SourceStages) -> anyhow::Result<()> {
        self.check()?;
        let (reply, received) = mpsc::channel();
        self.emit(Update::StageRequest {
            stages: stages.clone(),
            reply,
        })?;
        self.wait(received)
    }

    fn persist_source_data(&mut self, change: &Change) -> anyhow::Result<()> {
        let (send, reply) = mpsc::channel();
        self.emit(Update::ReadSource(send))?;
        let before = self.wait(reply)?;
        let mutation = razer_model::mouse_matcher::Mutation::from_change(&before, change)?;
        let (send, reply) = mpsc::channel();
        self.emit(Update::Persist {
            mutation,
            reply: send,
        })?;
        let saved = self.wait(reply)?;
        let mut source = SourceData::from_document(saved.document())?;
        if let Some(previous) = &self.live_source {
            source.retain_live(previous);
        }
        source.apply(change);
        self.live_source = Some(source);
        Ok(())
    }

    fn publish_stage_draft(&mut self, stages: &SourceStages) -> anyhow::Result<()> {
        self.emit(Update::StageDraft(stages.clone()))
    }

    fn publish_calibration(
        &mut self,
        calibration: razer_device::dpi_matcher::Calibration,
    ) -> anyhow::Result<()> {
        self.emit(Update::Response(Response::Calibration(calibration)))
    }

    fn report_failure(&mut self, error: String) -> anyhow::Result<()> {
        self.emit(Update::Failure(error))
    }

    fn confirmation_identity(
        &mut self,
        source_container: &str,
        _: Option<u32>,
    ) -> anyhow::Result<(String, ProfileName)> {
        self.check()?;
        Ok((
            uuid::Uuid::new_v4().to_string(),
            serde_json::from_value(
                self.options
                    .context("Matcher capture options are absent")?
                    .confirmation_name(source_container)?,
            )?,
        ))
    }
}

fn run(
    options: Option<Options>,
    product: razer_device::dpi_matcher::Product,
    commands: Receiver<Command>,
    updates: async_channel::Sender<Update>,
    canceled: Arc<AtomicBool>,
    closing: Arc<AtomicBool>,
) -> WorkerExit {
    let mut backend = NativeBackend {
        options: options.as_ref(),
        client: None,
        updates: &updates,
        canceled: &canceled,
        live_source: None,
        monitor: options
            .as_ref()
            .map(|_| InputMonitor::new(uuid::Uuid::new_v4().to_string())),
        lifecycle: None,
        device_task: None,
    };
    let mut controller = Controller::new(product);
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
                    Some(command) => Ok(command),
                    None => break,
                }
            } else {
                commands.next(controller.locked)
            };
            let starting = matches!(&command, Ok(Command::Start));
            let profiles_command = options.is_none() && command.is_ok();
            if starting && closing.load(Ordering::Acquire) {
                continue;
            }
            let result = match command {
                Ok(command)
                    if options.is_none() && !matches!(command, Command::UpdateProfiles(_)) =>
                {
                    Err(anyhow::anyhow!(
                        "Matcher profile owner accepts only UpdateProfiles"
                    ))
                }
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
                    if !controller.monitoring && backend.monitor_started() {
                        backend.stop_input_monitoring()?;
                    }
                    if !controller.monitoring {
                        backend.release_matcher_locks()?;
                    }
                    if starting && options.is_some() {
                        backend.emit(Update::Response(Response::Calibration(
                            controller.response(),
                        )))?;
                    }
                }
            }
            if profiles_command {
                break;
            }
            if !closing.load(Ordering::Acquire) && controller.state() == State::Calibrating {
                for batch in backend.events()? {
                    let initial_is_main = options
                        .as_ref()
                        .context("Matcher capture options are absent")?
                        .is_initial_main(batch.product_id)?;
                    let response = controller.ingest(
                        batch,
                        initial_is_main,
                        elapsed_ms(started),
                        &mut backend,
                    )?;
                    backend.emit(Update::Response(Response::Calibration(response)))?;
                    if controller.state() != State::Calibrating {
                        break;
                    }
                }
                if let Some(response) = controller.tick(elapsed_ms(started), &mut backend)? {
                    backend.emit(Update::Response(Response::Calibration(response)))?;
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

    fn profiles_device() -> razer_model::model::Device {
        let mut device = razer_model::product_preview::registered_preview(190).unwrap();
        device.active_profile = "p".into();
        device.profiles = vec![razer_model::model::Profile {
            id: "p".into(),
            guid: "p".into(),
            name: "Profile".into(),
            settings: None,
            dpi_stages: None,
            source_settings: Some(json!({
                "guid":"p", "selectedSensitivityMatcher":"second", "profileExtension":true
            })),
        }];
        device.source_device_settings = Some(json!({
            "version":7, "deviceExtension":true,
            "sensitivityMatcher":{"libraryExtension":true,"profiles":[
                {"guid":"first","name":"Mouse"},
                {"guid":"second","name":"Mouse"}
            ]}
        }));
        device
    }

    #[test]
    fn dpi_matcher_profiles_worker_persists_without_a_route_or_native_capture() {
        let mut device = profiles_device();
        let payload = ProfileUpdate::from_payload(&json!({
            "sensitivityMatcher":{"libraryExtension":true,"profiles":[
                {"guid":"first","name":"Mouse"},
                {"guid":"second","name":"Mouse 1"}
            ]},
            "selectedSensitivityMatcher":"second"
        }))
        .unwrap();
        let (mut handle, updates) = Handle::start_profiles(
            razer_device::dpi_matcher::Product::new(190).unwrap(),
            vec![],
        );
        handle
            .send(Command::UpdateProfiles(payload.clone()))
            .unwrap();
        for _ in 0..2 {
            let Update::ReadSource(reply) = updates.0.recv_blocking().unwrap() else {
                panic!("local profile update must read its current saved source");
            };
            reply
                .send(Ok(
                    razer_model::mouse_matcher::source_record(&device).unwrap()
                ))
                .unwrap();
        }
        let (mutation, reply) = match updates.0.recv_blocking().unwrap() {
            Update::Persist { mutation, reply } => (mutation, reply),
            Update::Failure(error) => {
                panic!("local profile update failed before durable persistence: {error}");
            }
            _ => panic!("local profile update must request durable persistence"),
        };
        assert!(mutation.is_profile_update());
        mutation.apply(&mut device).unwrap();
        assert_eq!(
            device.source_device_settings.as_ref().unwrap()["sensitivityMatcher"],
            payload.library().file_value()
        );
        assert_eq!(
            device.source_device_settings.as_ref().unwrap()["version"],
            8
        );
        assert_eq!(
            device.profiles[0].source_settings.as_ref().unwrap()["profileExtension"],
            true
        );
        reply
            .send(Ok(
                razer_model::mouse_matcher::source_record(&device).unwrap()
            ))
            .unwrap();
        let Update::Response(Response::Profiles(observed)) = updates.0.recv_blocking().unwrap()
        else {
            panic!("durable receipt must publish the exact profile response");
        };
        assert_eq!(observed, payload);
        handle.begin_exit();
        let completion = handle.completion();
        handle.finish().unwrap();
        assert!(completion.is_finished());
        assert!(completion.is_released());
        assert!(completion.cleanup_error().is_none());
    }

    #[test]
    fn dpi_matcher_profiles_worker_rejects_capture_commands_and_releases_on_cancel() {
        let (handle, updates) = Handle::start_profiles(
            razer_device::dpi_matcher::Product::new(190).unwrap(),
            vec![],
        );
        handle.send(Command::Start).unwrap();
        let Update::Failure(error) = updates.0.recv_blocking().unwrap() else {
            panic!("profile owner must reject a command before any native action");
        };
        assert_eq!(error, "Matcher profile owner accepts only UpdateProfiles");
        let completion = handle.completion();
        handle.finish().unwrap();
        assert!(completion.is_finished());
        assert!(completion.is_released());
        assert!(completion.cleanup_error().is_none());
    }

    fn named_device(product_id: u32, container: &str, name: &str) -> ConnectedDevice {
        ConnectedDevice {
            product_id,
            container_id: container.into(),
            keyboard: false,
            english_name: Some(name.into()),
            chinese_name: Some(format!("{name} localized")),
        }
    }

    #[test]
    fn dpi_matcher_reference_names_preserve_each_products_source_value_shape() {
        let connected = [named_device(70, "reference", "Mouse")];
        for pid in [190, 192, 222, 226, 229, 239] {
            let product = razer_device::dpi_matcher::Product::new(pid).unwrap();
            let pairing = PairingSnapshot::default();
            let name = reference_name(product, &connected, &pairing, "reference", || {
                Ok("Host".into())
            })
            .unwrap();
            let missing =
                reference_name(
                    product,
                    &connected,
                    &pairing,
                    "absent",
                    || Ok("Host".into()),
                )
                .unwrap();
            assert_eq!(name, json!({"en":"Mouse","zh-cn":"Mouse localized"}));
            assert_eq!(missing, json!({"en":"Host","zh-cn":"Host"}));
        }
    }

    #[test]
    fn dpi_matcher_reference_pairing_uses_first_match_and_current_refresh_branch() {
        for pid in [190, 192, 222, 226, 229, 239] {
            let product = razer_device::dpi_matcher::Product::new(pid).unwrap();
            for (master, keyboard) in [
                (179, false),
                (164, false),
                (241, false),
                (700, true),
                (70, false),
            ] {
                let mut source = named_device(master, "reference", "Master");
                source.keyboard = keyboard;
                let connected = [
                    source,
                    named_device(71, "first", "First"),
                    named_device(80, "second", "Second"),
                ];
                let pairing = PairingSnapshot::from_value(&json!([
                    {"productId":71,"master":{"productId":master}},
                    {"productId":80,"master":{"productId":master}}
                ]))
                .unwrap();
                let name = reference_name(product, &connected, &pairing, "reference", || {
                    Ok("Host".into())
                })
                .unwrap();
                let paired = matches!(master, 179 | 164 | 241) || keyboard;
                let expected = if paired { "First" } else { "Master" };
                assert_eq!(
                    name,
                    json!({"en":expected,"zh-cn":format!("{expected} localized")})
                );
            }
        }
    }

    #[test]
    fn dpi_matcher_reference_missing_peer_uses_observed_host_and_missing_master_is_error() {
        let connected = [named_device(179, "reference", "Master")];
        for pid in [190, 192, 222, 226, 229, 239] {
            let product = razer_device::dpi_matcher::Product::new(pid).unwrap();
            let pairing =
                PairingSnapshot::from_value(&json!([{"productId":70,"master":{"productId":179}}]))
                    .unwrap();
            let name = reference_name(product, &connected, &pairing, "reference", || {
                Ok("Host".into())
            })
            .unwrap();
            assert_eq!(name, json!({"en":"Host","zh-cn":"Host"}));
            let invalid = PairingSnapshot::from_value(&json!([{"productId":70}])).unwrap();
            assert!(
                reference_name(product, &connected, &invalid, "reference", || panic!(
                    "invalid pairing must precede the host query"
                ))
                .is_err()
            );
        }
    }

    #[test]
    fn dpi_matcher_pairing_snapshot_cannot_admit_an_unsupported_receiver_route() {
        const OWNER: &str = "{11111111-2222-3333-4444-555555555555}";
        let projection = razer_discovery::discovery::project_receiver_query(
            164,
            OWNER,
            &json!({
                "query":"receiver_wireless_status_v2", "vendor_id":5426,
                "product_id":164, "claim_interface":0, "report_bytes":91,
                "path":"observed-hid-collection", "device_instance_id":"observed-device-instance",
                "device_container_id":OWNER, "device_count":1,
                "devices":[{"product_id":191,"status":1}]
            }),
        )
        .unwrap();
        let [route] = projection.devices() else {
            panic!("one current audited Matcher peer expected")
        };
        assert_eq!(route.product_id(), 190);
        for pairing in [
            json!([{"productId":190,"deviceContainerId":OWNER,"master":{"productId":164,"dongleId":164}}]),
            json!({"binding":{"productId":190,"deviceContainerId":OWNER,"master":{"productId":164,"dongleId":164}}}),
        ] {
            assert!(
                PairingSnapshot::from_value(&pairing)
                    .unwrap()
                    .has_input_monitoring_master(164, OWNER)
                    .unwrap()
            );
            assert!(Options::new(route.clone(), vec![], Some(&pairing)).is_err());
        }
    }

    #[test]
    fn dpi_matcher_exit_drains_only_accepted_commands_and_does_not_restart_capture() {
        let (send, receiver) = mpsc::channel();
        let mut queue = CommandQueue {
            receiver,
            deferred: VecDeque::new(),
        };
        queue.defer(Command::Select("first".into())).unwrap();
        queue
            .defer(Command::DeleteProfile("second".into()))
            .unwrap();
        send.send(Command::Start).unwrap();
        send.send(Command::Confirm).unwrap();
        send.send(Command::UpdateProfiles(ProfileUpdate::new(
            razer_device::dpi_matcher_storage::Library::new(vec![]),
            None,
        )))
        .unwrap();
        assert!(matches!(queue.drain_next(), Some(Command::Select(guid)) if guid == "first"));
        assert!(
            matches!(queue.drain_next(), Some(Command::DeleteProfile(guid)) if guid == "second")
        );
        assert!(matches!(queue.drain_next(), Some(Command::Confirm)));
        assert!(matches!(
            queue.drain_next(),
            Some(Command::UpdateProfiles(_))
        ));
        assert!(queue.drain_next().is_none());
    }

    #[test]
    fn dpi_matcher_exit_keeps_accepted_persistence_wait_until_real_receipt() {
        for receipt in [Ok(()), Err("disk commit failed".to_owned())] {
            let (ready, waiting) = mpsc::channel();
            let (reply, persistence) = mpsc::channel();
            let (observed, completed) = mpsc::channel();
            let (mut handle, _updates) = Handle::spawn(vec![], move |_, _, canceled, closing| {
                ready.send((canceled.clone(), closing)).unwrap();
                let result = wait_reply(persistence, &canceled);
                observed
                    .send(
                        result
                            .as_ref()
                            .map(|_| ())
                            .map_err(|error| error.to_string()),
                    )
                    .unwrap();
                WorkerExit {
                    operation: result,
                    cleanup: Ok(()),
                    released: Ok(()),
                }
            });
            let (canceled, closing) = waiting.recv().unwrap();
            handle.begin_exit();
            assert!(closing.load(Ordering::Acquire));
            assert!(!canceled.load(Ordering::Acquire));
            assert!(handle.send(Command::DeleteAllProfiles).is_err());
            assert!(completed.try_recv().is_err());
            reply.send(receipt.clone()).unwrap();
            assert_eq!(completed.recv().unwrap(), receipt);
            let completion = handle.completion();
            assert_eq!(handle.finish().is_ok(), receipt.is_ok());
            assert!(completion.is_finished());
            assert!(completion.is_released());
            assert!(completion.cleanup_error().is_none());
        }
    }

    #[test]
    fn dpi_matcher_exit_disconnected_reply_and_replaced_owner_are_real_failures() {
        let (send, reply) = mpsc::channel::<Result<(), String>>();
        drop(send);
        let canceled = AtomicBool::new(false);
        assert!(
            wait_reply(reply, &canceled)
                .unwrap_err()
                .to_string()
                .contains("response owner closed")
        );
        let (_send, reply) = mpsc::channel::<Result<(), String>>();
        canceled.store(true, Ordering::Release);
        assert!(
            wait_reply(reply, &canceled)
                .unwrap_err()
                .to_string()
                .contains("owner was canceled")
        );
    }

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
        let (handle, _updates) = Handle::spawn(vec![], move |_, _, _, _| {
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
        let (handle, updates) = Handle::spawn(vec![failed], |_, _, _, _| {
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
        let (handle, _updates) = Handle::spawn(vec![], |_, _, _, _| WorkerExit {
            operation: Err(anyhow::anyhow!("owner canceled while awaiting source")),
            cleanup: Ok(()),
            released: Ok(()),
        });
        let completion = handle.completion();
        assert!(handle.finish().is_err());
        assert!(completion.cleanup_error().is_none());
        assert!(completion.wait(|| false).unwrap());
        let (accepted, observed) = mpsc::channel();
        let (replacement, _updates) = Handle::spawn(vec![completion], move |_, _, _, _| {
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
        let (handle, _updates) = Handle::spawn(vec![], |_, _, _, _| WorkerExit {
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
        assert!(
            completion
                .wait(|| false)
                .unwrap_err()
                .to_string()
                .contains("native stop was not acknowledged")
        );
        let (replacement, updates) = Handle::spawn(vec![completion], |_, _, _, _| {
            panic!("successful transport release cannot replace failed native cleanup")
        });
        assert!(matches!(
            updates.0.recv_blocking().unwrap(),
            Update::Failure(error) if error.contains("native stop was not acknowledged")
        ));
        assert!(
            replacement
                .finish()
                .unwrap_err()
                .to_string()
                .contains("native stop was not acknowledged")
        );
    }

    #[test]
    fn dpi_matcher_join_preserves_both_cleanup_and_transport_failures() {
        let (handle, _updates) = Handle::spawn(vec![], |_, _, _, _| WorkerExit {
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
