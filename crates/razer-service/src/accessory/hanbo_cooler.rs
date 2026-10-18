//! One connection owns the ordered Hanbo thermal queue and its IPC process.
use razer_device::{
    backend::HidNode,
    hanbo_cooler::{CoolerCommand, Telemetry},
};
use razer_ipc::{ServiceClient, ServiceRequest};
use serde::Deserialize;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Current MW module 48290 `getSpeedOfFanAndPumpTimer` interval.
const TELEMETRY_INTERVAL: Duration = Duration::from_millis(5000);
const SYSTEM_INTERVAL: Duration = Duration::from_millis(1000);

pub enum Reply {
    Finished {
        generation: u64,
        result: Result<(), String>,
    },
    Restored(Result<(), String>),
    Observed(Result<Telemetry, String>),
    SystemObserved(Result<razer_device::system_monitor::SystemSample, String>),
    SystemWriteFinished(Result<(), String>),
    Stopped(Result<(), String>),
}

struct Submission {
    generation: Option<u64>,
    commands: Vec<CoolerCommand>,
    accepted_at: Instant,
}

enum Message {
    Submit(Submission),
    ActivateTelemetry,
}

struct PollSchedule {
    telemetry_interval: Option<Duration>,
    telemetry: Option<Instant>,
    system: Option<Instant>,
}

impl PollSchedule {
    fn deadline(&self) -> Option<Instant> {
        self.telemetry.into_iter().chain(self.system).min()
    }

    fn stop(&mut self) {
        self.telemetry = None;
        self.system = None;
    }

    fn activate_telemetry(&mut self) {
        if self.telemetry.is_none() {
            self.telemetry = self
                .telemetry_interval
                .map(|interval| Instant::now() + interval);
        }
    }
}

struct Owner {
    valid: Arc<AtomicBool>,
    commands: Arc<Mutex<Option<mpsc::SyncSender<Message>>>>,
    worker: Mutex<Worker>,
}

struct Worker {
    handle: Option<JoinHandle<Result<(), String>>>,
    result: Option<Result<(), String>>,
}

impl Drop for Owner {
    fn drop(&mut self) {
        self.valid.store(false, Ordering::Release);
        self.commands
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
    }
}

/// Clones borrow the same connection. Final cleanup waits off the GUI thread.
#[derive(Clone)]
pub struct Session(Arc<Owner>);

impl Session {
    pub fn start(
        node: HidNode,
        container_id: String,
        predecessors: Vec<Self>,
    ) -> anyhow::Result<(Self, async_channel::Receiver<Reply>)> {
        Self::start_with_schedule(
            move |valid| {
                for predecessor in predecessors {
                    predecessor.wait().map_err(anyhow::Error::msg)?;
                }
                anyhow::ensure!(
                    valid.load(Ordering::Acquire),
                    "Hanbo connection canceled before service creation"
                );
                let mut client = ServiceClient::spawn()?;
                client.set_request_guard(valid.clone());
                Ok(NativeTransport::new(node, container_id, client, valid))
            },
            PollSchedule {
                telemetry_interval: Some(TELEMETRY_INTERVAL),
                telemetry: None,
                system: Some(Instant::now() + SYSTEM_INTERVAL),
            },
        )
    }

    pub fn submit(&self, generation: u64, command: CoolerCommand) -> Result<(), String> {
        self.submit_batch(generation, vec![command])
    }

    pub fn submit_batch(
        &self,
        generation: u64,
        commands: Vec<CoolerCommand>,
    ) -> Result<(), String> {
        self.admit(Some(generation), commands)
    }

    /// Profile restoration shares the FIFO without completing a page edit.
    pub fn restore(&self, commands: Vec<CoolerCommand>) -> Result<(), String> {
        self.admit(None, commands)
    }

    fn admit(&self, generation: Option<u64>, commands: Vec<CoolerCommand>) -> Result<(), String> {
        if commands.is_empty() {
            return Err("Hanbo command batch is empty".into());
        }
        let admission = self
            .0
            .commands
            .lock()
            .map_err(|_| "Hanbo admission lock poisoned")?;
        if !self.0.valid.load(Ordering::Acquire) {
            return Err("Hanbo connection is canceled".into());
        }
        admission
            .as_ref()
            .ok_or("Hanbo connection is draining")?
            .try_send(Message::Submit(Submission {
                generation,
                commands,
                accepted_at: Instant::now(),
            }))
            .map_err(|error| format!("Hanbo command queue unavailable: {error}"))
    }

    pub fn activate_telemetry(&self) -> Result<(), String> {
        let commands = self
            .0
            .commands
            .lock()
            .map_err(|_| "Hanbo admission lock poisoned")?;
        if !self.0.valid.load(Ordering::Acquire) {
            return Err("Hanbo connection is canceled".into());
        }
        commands
            .as_ref()
            .ok_or("Hanbo connection is draining")?
            .try_send(Message::ActivateTelemetry)
            .map_err(|error| format!("Hanbo telemetry activation unavailable: {error}"))
    }

    pub fn is_accepting(&self) -> bool {
        self.0.valid.load(Ordering::Acquire)
            && self
                .0
                .commands
                .lock()
                .is_ok_and(|commands| commands.is_some())
    }

    pub fn same_owner(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Closing admission preserves all accepted NORMAL-category source tasks.
    pub fn finish(&self) {
        self.0
            .commands
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
    }

    pub fn cancel(&self) {
        self.0.valid.store(false, Ordering::Release);
        self.finish();
    }

    /// Call after finish/cancel from the background lifecycle owner.
    pub fn wait(&self) -> Result<(), String> {
        let mut worker = self
            .0
            .worker
            .lock()
            .map_err(|_| "Hanbo join lock poisoned")?;
        if let Some(result) = &worker.result {
            return result.clone();
        }
        let handle = worker
            .handle
            .take()
            .ok_or("Hanbo worker handle is absent")?;
        let result = handle
            .join()
            .unwrap_or_else(|_| Err("Hanbo worker panicked during cleanup".into()));
        worker.result = Some(result.clone());
        result
    }

    #[cfg(test)]
    fn start_with<T: Transport + Send + 'static>(
        factory: impl FnOnce(Arc<AtomicBool>) -> anyhow::Result<T> + Send + 'static,
    ) -> anyhow::Result<(Self, async_channel::Receiver<Reply>)> {
        Self::start_with_interval(factory, None)
    }

    #[cfg(test)]
    fn start_with_interval<T: Transport + Send + 'static>(
        factory: impl FnOnce(Arc<AtomicBool>) -> anyhow::Result<T> + Send + 'static,
        interval: Option<Duration>,
    ) -> anyhow::Result<(Self, async_channel::Receiver<Reply>)> {
        Self::start_with_schedule(
            factory,
            PollSchedule {
                telemetry_interval: interval,
                telemetry: interval.map(|interval| Instant::now() + interval),
                system: None,
            },
        )
    }

    fn start_with_schedule<T: Transport + Send + 'static>(
        factory: impl FnOnce(Arc<AtomicBool>) -> anyhow::Result<T> + Send + 'static,
        schedule: PollSchedule,
    ) -> anyhow::Result<(Self, async_channel::Receiver<Reply>)> {
        let (commands, input) = mpsc::sync_channel(128);
        let admission = Arc::new(Mutex::new(Some(commands)));
        let worker_admission = admission.clone();
        let (output, replies) = async_channel::unbounded();
        let valid = Arc::new(AtomicBool::new(true));
        let signal = valid.clone();
        let worker = std::thread::Builder::new()
            .name("hanbo-thermal-owner".into())
            .spawn(move || {
                let result = run(
                    factory,
                    signal.clone(),
                    input,
                    &output,
                    worker_admission,
                    schedule,
                );
                signal.store(false, Ordering::Release);
                let _ = output.send_blocking(Reply::Stopped(result.clone()));
                result
            })?;
        Ok((
            Self(Arc::new(Owner {
                valid,
                commands: admission,
                worker: Mutex::new(Worker {
                    handle: Some(worker),
                    result: None,
                }),
            })),
            replies,
        ))
    }
}

enum TransportState {
    Active,
    HidStopped { diagnostic: String },
}

trait Transport {
    fn state(&self) -> TransportState;
    fn submit(&mut self, command: CoolerCommand) -> anyhow::Result<()>;
    fn observe(&mut self) -> anyhow::Result<Telemetry>;
    fn observe_system(&mut self)
    -> anyhow::Result<Vec<razer_device::system_monitor::SystemSample>>;
    fn shutdown(&mut self) -> anyhow::Result<()>;
}

struct NativeTransport {
    node: HidNode,
    client: ServiceClient,
    hid_failure: Option<String>,
    monitor_client: Option<ServiceClient>,
    monitor_owner: String,
    monitor_started: bool,
    monitor_error: Option<anyhow::Error>,
    monitor_samples: Vec<razer_device::system_monitor::SystemSample>,
}

impl NativeTransport {
    fn new(
        node: HidNode,
        container_id: String,
        mut client: ServiceClient,
        valid: Arc<AtomicBool>,
    ) -> Self {
        let monitor_owner = uuid::Uuid::new_v4().to_string();
        let device_information = read_startup(
            &mut client,
            &node,
            razer_device::hanbo_cooler::StartupQuery::DeviceInformation,
        );
        let mut hid_failure = None;
        if let Err(error) = &device_information {
            eprintln!("[hanbo startup information read] {error:#}");
            if client.is_stopped() {
                hid_failure = Some(format!("{error:#}"));
            }
        }
        let device_ready = device_information.is_ok();
        let mut monitor_client = None;
        let initialized = (|| -> anyhow::Result<_> {
            device_information?;
            anyhow::ensure!(
                valid.load(Ordering::Acquire),
                "Hanbo connection canceled before monitor service creation"
            );
            // HID cancellation can terminate its process. The monitor keeps a
            // separate process so its native callbacks and window still receive
            // the verified shutdown sequence after that cancellation.
            monitor_client = Some(ServiceClient::spawn()?);
            anyhow::ensure!(
                valid.load(Ordering::Acquire),
                "Hanbo connection canceled before monitor startup"
            );
            let monitor = monitor_client
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("Hanbo monitor service owner is absent"))?;
            let observation: razer_device::system_monitor::MonitorObservation =
                serde_json::from_value(monitor.request(ServiceRequest::SystemMonitorStart {
                    owner_guid: monitor_owner.clone(),
                    container_id,
                    product_id: razer_device::hanbo_cooler::PRODUCT_ID,
                })?)?;
            observation.validate_for(&monitor_owner)?;
            for diagnostic in observation.diagnostics {
                eprintln!("[system monitor] {diagnostic}");
            }
            Ok(observation.samples)
        })();
        if device_ready && valid.load(Ordering::Acquire) {
            match read_startup(
                &mut client,
                &node,
                razer_device::hanbo_cooler::StartupQuery::ErrorcodeInfo,
            ) {
                Ok(razer_device::hanbo_cooler::StartupReading::ErrorcodeInfo(errors))
                    if errors.has_errors() =>
                {
                    eprintln!("[hanbo startup errors] {errors:?}");
                }
                Ok(_) => {}
                Err(error) => {
                    eprintln!("[hanbo startup error read] {error:#}");
                    if client.is_stopped() {
                        hid_failure = Some(format!("{error:#}"));
                    }
                }
            }
        }
        let (monitor_started, monitor_error, monitor_samples) = match initialized {
            Ok(samples) => (true, None, samples),
            Err(error) => (false, Some(error), Vec::new()),
        };
        Self {
            node,
            client,
            hid_failure,
            monitor_client,
            monitor_owner,
            monitor_started,
            monitor_error,
            monitor_samples,
        }
    }

    fn hid_request(&mut self, request: ServiceRequest) -> anyhow::Result<serde_json::Value> {
        let result = self.client.request(request);
        if self.client.is_stopped() {
            self.hid_failure = Some(match &result {
                Err(error) => format!("{error:#}"),
                Ok(_) => "Hanbo HID service stopped after its response".into(),
            });
        }
        result
    }
}

fn read_startup(
    client: &mut ServiceClient,
    node: &HidNode,
    query: razer_device::hanbo_cooler::StartupQuery,
) -> anyhow::Result<razer_device::hanbo_cooler::StartupReading> {
    let observation: razer_ipc::HanboStartupObservation =
        serde_json::from_value(client.request(ServiceRequest::HidNodeHanboStartupRead {
            node: node.clone(),
            product_id: razer_device::hanbo_cooler::PRODUCT_ID,
            query,
        })?)?;
    observation.validate_for(node, query)?;
    Ok(observation.reading)
}

impl Transport for NativeTransport {
    fn state(&self) -> TransportState {
        if self.client.is_stopped() {
            TransportState::HidStopped {
                diagnostic: self
                    .hid_failure
                    .clone()
                    .unwrap_or_else(|| "Hanbo HID service is stopped".into()),
            }
        } else {
            TransportState::Active
        }
    }

    fn submit(&mut self, command: CoolerCommand) -> anyhow::Result<()> {
        #[derive(Deserialize)]
        struct Receipt {
            node: HidNode,
            product_id: u32,
            command: CoolerCommand,
            acknowledged: bool,
        }
        let receipt: Receipt =
            serde_json::from_value(self.hid_request(ServiceRequest::HidNodeHanboCooler {
                node: self.node.clone(),
                product_id: razer_device::hanbo_cooler::PRODUCT_ID,
                command: command.clone(),
            })?)?;
        anyhow::ensure!(
            receipt.acknowledged
                && receipt.product_id == razer_device::hanbo_cooler::PRODUCT_ID
                && receipt.node == self.node
                && receipt.command == command,
            "Hanbo command acknowledgement differs from the submitted connection and command"
        );
        Ok(())
    }

    fn observe(&mut self) -> anyhow::Result<Telemetry> {
        let observation: razer_ipc::HanboCoolerObservation =
            serde_json::from_value(self.hid_request(ServiceRequest::HidNodeHanboCoolerRead {
                node: self.node.clone(),
                product_id: razer_device::hanbo_cooler::PRODUCT_ID,
            })?)?;
        observation.validate_for(&self.node)?;
        Ok(observation.telemetry)
    }

    fn observe_system(
        &mut self,
    ) -> anyhow::Result<Vec<razer_device::system_monitor::SystemSample>> {
        if let Some(error) = self.monitor_error.take() {
            return Err(error);
        }
        if !self.monitor_started {
            return Ok(Vec::new());
        }
        let monitor = self
            .monitor_client
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Hanbo monitor service owner is absent"))?;
        let observation: razer_device::system_monitor::MonitorObservation =
            serde_json::from_value(monitor.request(ServiceRequest::SystemMonitorPoll {
                owner_guid: self.monitor_owner.clone(),
            })?)?;
        observation.validate_for(&self.monitor_owner)?;
        for diagnostic in observation.diagnostics {
            eprintln!("[system monitor] {diagnostic}");
        }
        let mut samples = std::mem::take(&mut self.monitor_samples);
        samples.extend(observation.samples);
        Ok(samples)
    }

    fn shutdown(&mut self) -> anyhow::Result<()> {
        fn stop(client: &mut ServiceClient) -> anyhow::Result<()> {
            #[derive(Deserialize)]
            struct Receipt {
                stopped: bool,
            }
            let receipt: Receipt =
                serde_json::from_value(client.request(ServiceRequest::Shutdown)?)?;
            anyhow::ensure!(
                receipt.stopped,
                "Hanbo service did not acknowledge shutdown"
            );
            Ok(())
        }

        let mut failures = Vec::new();
        if let Some(monitor) = self.monitor_client.as_mut() {
            if let Err(error) = guarded(|| stop(monitor)) {
                failures.push(format!("System monitor cleanup failed: {error}"));
            }
        }
        let hid_cleanup = if self.client.is_stopped() {
            let result = guarded(|| self.client.finish_termination());
            if result.is_ok() {
                eprintln!(
                    "[hanbo cleanup] HID worker termination and pipe release confirmed; in-flight device writes and native shutdown remain unconfirmed"
                );
            } else {
                eprintln!(
                    "[hanbo cleanup] HID worker termination remains incomplete; in-flight device writes and native shutdown remain unconfirmed"
                );
            }
            result
        } else {
            guarded(|| stop(&mut self.client))
        };
        if let Err(error) = hid_cleanup {
            failures.push(format!("HID service cleanup failed: {error}"));
        }
        anyhow::ensure!(failures.is_empty(), "{}", failures.join("; "));
        Ok(())
    }
}

fn guarded<T>(operation: impl FnOnce() -> anyhow::Result<T>) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation))
        .map_err(|_| "Hanbo operation panicked".to_owned())?
        .map_err(|error| format!("{error:#}"))
}

fn close_stopped_hid(
    transport: &impl Transport,
    valid: &AtomicBool,
    admission: &Mutex<Option<mpsc::SyncSender<Message>>>,
    schedule: &mut PollSchedule,
) -> Option<String> {
    let TransportState::HidStopped { diagnostic } = transport.state() else {
        return None;
    };
    valid.store(false, Ordering::Release);
    admission
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    schedule.stop();
    Some(diagnostic)
}

fn run<T: Transport>(
    factory: impl FnOnce(Arc<AtomicBool>) -> anyhow::Result<T>,
    valid: Arc<AtomicBool>,
    input: mpsc::Receiver<Message>,
    output: &async_channel::Sender<Reply>,
    admission: Arc<Mutex<Option<mpsc::SyncSender<Message>>>>,
    mut schedule: PollSchedule,
) -> Result<(), String> {
    let mut transport = match guarded(|| factory(valid.clone())) {
        Ok(transport) => transport,
        Err(error) => {
            valid.store(false, Ordering::Release);
            admission
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
            for message in input {
                if let Message::Submit(submission) = message {
                    complete_submission(output, submission.generation, Err(error.clone()));
                }
            }
            return Err(error);
        }
    };
    let mut last_temperatures = None;
    let mut terminal = close_stopped_hid(&transport, &valid, &admission, &mut schedule);
    loop {
        if !valid.load(Ordering::Acquire)
            || admission
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_none()
        {
            schedule.stop();
        }
        let message = if let Some(deadline) = schedule.deadline() {
            match input.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(submission) => submission,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if valid.load(Ordering::Acquire)
                        && admission
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .is_some()
                    {
                        terminal = poll_due(
                            &mut transport,
                            &mut schedule,
                            Instant::now(),
                            output,
                            &mut last_temperatures,
                            &valid,
                            &admission,
                        );
                    }
                    continue;
                }
            }
        } else {
            let Ok(submission) = input.recv() else {
                break;
            };
            submission
        };
        let submission = match message {
            Message::ActivateTelemetry => {
                if valid.load(Ordering::Acquire)
                    && admission
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .is_some()
                {
                    schedule.activate_telemetry();
                }
                continue;
            }
            Message::Submit(submission) => submission,
        };
        if schedule
            .deadline()
            .is_some_and(|deadline| submission.accepted_at >= deadline)
            && valid.load(Ordering::Acquire)
            && admission
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_some()
        {
            terminal = poll_due(
                &mut transport,
                &mut schedule,
                submission.accepted_at,
                output,
                &mut last_temperatures,
                &valid,
                &admission,
            );
        }
        let mut failures = Vec::new();
        for command in submission.commands {
            if let Some(diagnostic) = &terminal {
                failures.push(format!(
                    "Hanbo HID service stopped; remaining device tasks were not submitted: {diagnostic}"
                ));
                break;
            }
            let result = if valid.load(Ordering::Acquire) {
                guarded(|| transport.submit(command))
            } else {
                Err("Hanbo connection was replaced or canceled".into())
            };
            if let Err(error) = result {
                failures.push(error);
            }
            terminal = close_stopped_hid(&transport, &valid, &admission, &mut schedule);
        }
        let result = if failures.is_empty() {
            Ok(())
        } else {
            Err(failures.join("; "))
        };
        complete_submission(output, submission.generation, result);
    }
    let cleanup = guarded(|| transport.shutdown());
    let destruction = guarded(|| {
        drop(transport);
        Ok(())
    });
    match (cleanup, destruction) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(error), Err(other)) => Err(format!("{error}; transport release: {other}")),
    }
}

fn complete_submission(
    output: &async_channel::Sender<Reply>,
    generation: Option<u64>,
    result: Result<(), String>,
) {
    let reply = match generation {
        Some(generation) => Reply::Finished { generation, result },
        None => Reply::Restored(result),
    };
    let _ = output.send_blocking(reply);
}

fn poll_due<T: Transport>(
    transport: &mut T,
    schedule: &mut PollSchedule,
    through: Instant,
    output: &async_channel::Sender<Reply>,
    last_temperatures: &mut Option<(Option<f64>, Option<f64>)>,
    valid: &AtomicBool,
    admission: &Mutex<Option<mpsc::SyncSender<Message>>>,
) -> Option<String> {
    while let Some(deadline) = schedule.deadline().filter(|deadline| *deadline <= through) {
        if let Some(diagnostic) = close_stopped_hid(transport, valid, admission, schedule) {
            return Some(diagnostic);
        }
        if !valid.load(Ordering::Acquire) {
            schedule.stop();
            break;
        }
        if schedule.telemetry == Some(deadline) {
            let _ = output.send_blocking(Reply::Observed(guarded(|| transport.observe())));
            if let Some(diagnostic) = close_stopped_hid(transport, valid, admission, schedule) {
                return Some(diagnostic);
            }
            schedule.telemetry = schedule
                .telemetry_interval
                .map(|interval| Instant::now() + interval);
        } else {
            match guarded(|| transport.observe_system()) {
                Err(error) => {
                    let _ = output.send_blocking(Reply::SystemObserved(Err(error)));
                }
                Ok(samples) => {
                    for sample in samples {
                        let temperatures = (sample.cpu_temperature, sample.gpu_temperature);
                        let changed = last_temperatures.as_ref() != Some(&temperatures);
                        let _ = output.send_blocking(Reply::SystemObserved(Ok(sample)));
                        if changed {
                            *last_temperatures = Some(temperatures);
                            let result = if valid.load(Ordering::Acquire) {
                                guarded(|| {
                                    transport.submit(CoolerCommand::for_temperatures(
                                        temperatures.0,
                                        temperatures.1,
                                    )?)
                                })
                            } else {
                                Err("Hanbo connection canceled before temperature submission"
                                    .into())
                            };
                            let _ = output.send_blocking(Reply::SystemWriteFinished(result));
                            if let Some(diagnostic) =
                                close_stopped_hid(transport, valid, admission, schedule)
                            {
                                return Some(diagnostic);
                            }
                        }
                    }
                }
            }
            schedule.system = Some(Instant::now() + SYSTEM_INTERVAL);
        }
    }
    None
}

#[cfg(test)]
#[path = "hanbo_cooler/tests.rs"]
mod tests;
