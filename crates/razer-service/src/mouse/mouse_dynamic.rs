//! Dynamic setters and input-monitor ownership stay on a background worker.
//! Source setter acknowledgement and independent GET diagnostics are distinct.
use crate::mouse_worker::{Cleanup, check, guarded, release, shutdown};
use anyhow::Context as _;
use razer_device::mouse_dynamic::{
    Report, Setting,
    input::{self, Speed},
};
use razer_discovery::{direct, discovery::ObservedDevice};
use razer_ipc::{
    ServiceClient,
    mouse_input_monitoring::{InputEvent, Session, StartAcknowledgement},
};
use std::{
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{SyncSender, sync_channel},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug)]
pub enum Outcome {
    Device(Report),
    MonitoringStopped,
}
pub type Completion = crate::mouse_worker::Completion<Outcome>;

pub fn device(
    route: ObservedDevice,
    change: Option<Setting>,
    active: Arc<AtomicBool>,
) -> Completion {
    crate::mouse_worker::device(
        route,
        active,
        direct::mouse_dynamic::available,
        |client, route| direct::mouse_dynamic::request(client, route, change).map(Outcome::Device),
    )
}

trait Monitor {
    fn start(&mut self) -> anyhow::Result<()>;
    fn repeat_start(&mut self) -> anyhow::Result<StartAcknowledgement>;
    fn events(&mut self) -> anyhow::Result<Vec<InputEvent>>;
    fn stop(
        &mut self,
        observe: &mut dyn FnMut(Vec<InputEvent>) -> anyhow::Result<()>,
    ) -> anyhow::Result<()>;
    fn shutdown(&mut self) -> anyhow::Result<()>;
    fn terminate(&mut self) -> anyhow::Result<()>;
}
struct NativeMonitor {
    client: ServiceClient,
    session: Session,
}
impl Monitor for NativeMonitor {
    fn stop(
        &mut self,
        observe: &mut dyn FnMut(Vec<InputEvent>) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        self.session
            .stop_with(|request| self.client.request(request), observe)
    }
    fn shutdown(&mut self) -> anyhow::Result<()> {
        shutdown(&mut self.client)
    }
    fn terminate(&mut self) -> anyhow::Result<()> {
        self.client.finish_termination()
    }
    fn start(&mut self) -> anyhow::Result<()> {
        self.session.start(|request| self.client.request(request))
    }
    fn repeat_start(&mut self) -> anyhow::Result<StartAcknowledgement> {
        self.session
            .repeat_start(|request| self.client.request(request))
    }
    fn events(&mut self) -> anyhow::Result<Vec<InputEvent>> {
        self.session.poll(|request| self.client.request(request))
    }
}

const MAX_PENDING_STARTS: usize = 16;
type StartResult = Result<StartAcknowledgement, String>;

/// The capture operation retains this handle. START requests run on its original
/// blocking worker and never create another service client or native runner.
pub struct MonitorControl {
    starts: async_channel::Sender<async_channel::Sender<StartResult>>,
}
pub struct MonitorRequests {
    starts: async_channel::Receiver<async_channel::Sender<StartResult>>,
}
pub struct StartReply {
    completed: async_channel::Receiver<StartResult>,
}
pub fn monitor_requests() -> (MonitorControl, MonitorRequests) {
    let (starts, receiver) = async_channel::bounded(MAX_PENDING_STARTS);
    (
        MonitorControl { starts },
        MonitorRequests { starts: receiver },
    )
}
impl MonitorControl {
    /// Admission is immediate; awaiting the reply performs no I/O on the caller.
    pub fn request_start(&self) -> anyhow::Result<StartReply> {
        let (completed, receiver) = async_channel::bounded(1);
        self.starts
            .try_send(completed)
            .map_err(|error| match error {
                async_channel::TrySendError::Closed(_) => {
                    anyhow::anyhow!("Dynamic capture no longer accepts START")
                }
                async_channel::TrySendError::Full(_) => {
                    anyhow::anyhow!("Dynamic capture START queue is full")
                }
            })?;
        Ok(StartReply {
            completed: receiver,
        })
    }
}
impl StartReply {
    pub async fn receive(self) -> StartResult {
        self.completed
            .recv()
            .await
            .map_err(|_| "Dynamic capture ended before START acknowledgement".to_owned())?
    }
}
impl MonitorRequests {
    fn dispatch(&self, backend: &mut impl Monitor) {
        for _ in 0..MAX_PENDING_STARTS {
            let Ok(completed) = self.starts.try_recv() else {
                break;
            };
            let result = guarded(|| backend.repeat_start());
            let _ = completed.try_send(result);
        }
    }

    fn reject(&self, reason: &str) {
        self.starts.close();
        while let Ok(completed) = self.starts.try_recv() {
            let _ = completed.try_send(Err(reason.to_owned()));
        }
    }
}
impl Drop for MonitorRequests {
    fn drop(&mut self) {
        self.reject("Dynamic capture ended before START acknowledgement");
    }
}

fn admits(route: &ObservedDevice, event: &InputEvent) -> anyhow::Result<bool> {
    admits_with(route.product_id(), event, |product_id| {
        // qY/IN reads the current local pairing records for every indirect PID.
        // Its container is the page's route, not the native event's container.
        let cache = razer_storage::receiver_pairing::PairingCache::load(
            &razer_storage::receiver_pairing::local_path()?,
        )?;
        cache.has_input_monitoring_master(product_id, route.container())
    })
}

fn admits_with(
    product_id: u32,
    event: &InputEvent,
    paired_master: impl FnOnce(u32) -> anyhow::Result<bool>,
) -> anyhow::Result<bool> {
    if input::is_direct_product(product_id, event.product_id())? {
        return Ok(true);
    }
    paired_master(event.product_id())
}

#[derive(Clone, Copy)]
struct ClockSample {
    timer: Duration,
    wall_millis: i128,
}
impl ClockSample {
    fn capture(started: Instant) -> Self {
        let wall_millis = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(elapsed) => elapsed.as_millis() as i128,
            Err(error) => {
                let elapsed = error.duration();
                -(elapsed.as_millis() as i128) - i128::from(elapsed.subsec_nanos() % 1_000_000 != 0)
            }
        };
        Self {
            timer: started.elapsed(),
            wall_millis,
        }
    }
}

struct SpeedPublication {
    speed: Speed,
    pending: bool,
    last_call: Option<i128>,
    last_publish: i128,
    deadline: Option<Duration>,
}
impl SpeedPublication {
    const WAIT_MILLIS: f64 = 1000. / 60.;

    fn new(product_id: u32) -> anyhow::Result<Self> {
        Ok(Self {
            speed: Speed::new(product_id)?,
            pending: false,
            last_call: None,
            last_publish: 0,
            deadline: None,
        })
    }

    fn should_publish(&self, wall_millis: i128) -> bool {
        self.last_call.is_none_or(|last_call| {
            let elapsed = wall_millis - last_call;
            elapsed < 0
                || elapsed as f64 >= Self::WAIT_MILLIS
                || (wall_millis - self.last_publish) as f64 >= Self::WAIT_MILLIS
        })
    }

    fn schedule(&mut self, timer: Duration, delay_millis: f64) {
        self.deadline = Some(timer + Duration::from_secs_f64(delay_millis / 1000.));
    }

    fn publish(&mut self, wall_millis: i128) -> f64 {
        self.pending = false;
        self.last_publish = wall_millis;
        self.speed.publish()
    }

    fn stop_acknowledged(&mut self) {
        // K/H resets movement after the awaited STOP, without cancelling or
        // flushing N. The reset includes lastTimeTick's numeric-zero sentinel.
        self.speed.publish();
    }

    fn observe(
        &mut self,
        json_event: &str,
        admitted: bool,
        now: impl FnOnce() -> ClockSample,
    ) -> anyhow::Result<Option<f64>> {
        self.speed.observe_filtered(json_event, admitted)?;
        let now = now();
        let invoke = self.should_publish(now.wall_millis);
        self.last_call = Some(now.wall_millis);
        self.pending = true;
        if invoke {
            // Both the leading edge and maxWait invocation schedule a timer,
            // even when this call consumes the only pending publication.
            self.schedule(now.timer, Self::WAIT_MILLIS);
            return Ok(Some(self.publish(now.wall_millis)));
        }
        if self.deadline.is_none() {
            self.schedule(now.timer, Self::WAIT_MILLIS);
        }
        Ok(None)
    }

    fn publish_due(&mut self, now: ClockSample) -> Option<f64> {
        if self.deadline.is_none_or(|deadline| now.timer < deadline) {
            return None;
        }
        // Source debounce tests Date.now() separately from the timer. Clock
        // rollback invokes a pending trailing edge instead of postponing it.
        if self.should_publish(now.wall_millis) {
            self.deadline = None;
            return self.pending.then(|| self.publish(now.wall_millis));
        }
        let since_call = now.wall_millis
            - self
                .last_call
                .expect("A publication timer requires a preceding callback");
        let since_publish = now.wall_millis - self.last_publish;
        self.schedule(
            now.timer,
            (Self::WAIT_MILLIS - since_call as f64).min(Self::WAIT_MILLIS - since_publish as f64),
        );
        None
    }
}

enum HistoryCommand {
    Callback {
        json: String,
        admitted: bool,
        completed: SyncSender<Result<(), String>>,
    },
    StopAcknowledged {
        completed: SyncSender<()>,
    },
}

/// Blocking capture workers submit callbacks to the connection's retained
/// asynchronous history. Acknowledgements return parse failures to that worker.
#[derive(Clone)]
pub struct SpeedHistoryInput {
    commands: async_channel::Sender<HistoryCommand>,
    observer: Arc<AtomicBool>,
}
impl SpeedHistoryInput {
    /// START removes and registers the same source callback, retaining one
    /// subscription even when PID226's native monitor is already running.
    pub fn register_observer(&self) -> anyhow::Result<()> {
        anyhow::ensure!(!self.is_closed(), "Dynamic speed history was closed");
        self.observer.store(true, Ordering::Release);
        if self.is_closed() {
            self.observer.store(false, Ordering::Release);
            anyhow::bail!("Dynamic speed history was closed");
        }
        Ok(())
    }

    pub fn unregister_observer(&self) {
        self.observer.store(false, Ordering::Release);
    }

    fn has_observer(&self) -> bool {
        self.observer.load(Ordering::Acquire)
    }

    fn observe(&self, json: &str, admitted: bool) -> anyhow::Result<()> {
        if self.is_closed() {
            anyhow::bail!("Dynamic speed history was closed");
        }
        if !self.has_observer() {
            return Ok(());
        }
        let (completed, result) = sync_channel(1);
        self.commands
            .send_blocking(HistoryCommand::Callback {
                json: json.to_owned(),
                admitted,
                completed,
            })
            .map_err(|_| anyhow::anyhow!("Dynamic speed history was closed"))?;
        result
            .recv()
            .context("Dynamic speed history ended before callback acknowledgement")?
            .map_err(anyhow::Error::msg)
    }

    fn stop_acknowledged(&self) -> anyhow::Result<()> {
        let (completed, result) = sync_channel(1);
        if self
            .commands
            .send_blocking(HistoryCommand::StopAcknowledged { completed })
            .is_err()
        {
            // A disposed connection has no movement history left to reset.
            return Ok(());
        }
        match result.recv() {
            Ok(()) => Ok(()),
            Err(_) if self.is_closed() => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    pub fn close(&self) {
        self.observer.store(false, Ordering::Release);
        self.commands.close();
    }

    pub fn is_closed(&self) -> bool {
        self.commands.is_closed()
    }
}

/// One mutable speed/throttle owner per page connection, independent of native
/// START/STOP I/O. Idle histories await messages without occupying a thread.
pub struct SpeedHistory {
    updates: SpeedPublication,
    commands: async_channel::Receiver<HistoryCommand>,
}
impl SpeedHistory {
    pub fn new(product_id: u32) -> anyhow::Result<(SpeedHistoryInput, Self)> {
        let updates = SpeedPublication::new(product_id)?;
        let (commands, receiver) = async_channel::bounded(1);
        let observer = Arc::new(AtomicBool::new(true));
        Ok((
            SpeedHistoryInput { commands, observer },
            Self {
                updates,
                commands: receiver,
            },
        ))
    }

    pub async fn run<F: Future<Output = ()>>(
        self,
        publish: impl FnMut(f64),
        timer: impl FnMut(Duration) -> F,
    ) {
        let started = Instant::now();
        self.run_with(publish, timer, || ClockSample::capture(started))
            .await;
    }

    async fn run_with<F: Future<Output = ()>>(
        mut self,
        mut publish: impl FnMut(f64),
        mut timer: impl FnMut(Duration) -> F,
        mut now: impl FnMut() -> ClockSample,
    ) {
        while !self.commands.is_closed() {
            let command = if let Some(deadline) = self.updates.deadline {
                tokio::select! {
                    command = self.commands.recv() => command.ok(),
                    _ = timer(deadline.saturating_sub(now().timer)) => {
                        if !self.commands.is_closed() {
                            if let Some(speed) = self.updates.publish_due(now()) {
                                publish(speed);
                            }
                        }
                        continue;
                    }
                }
            } else {
                self.commands.recv().await.ok()
            };
            if self.commands.is_closed() {
                break;
            }
            match command {
                Some(HistoryCommand::Callback {
                    json,
                    admitted,
                    completed,
                }) => {
                    let result = guarded(|| {
                        if let Some(speed) = self.updates.observe(&json, admitted, &mut now)? {
                            publish(speed);
                        }
                        Ok(())
                    });
                    let _ = completed.send(result);
                }
                Some(HistoryCommand::StopAcknowledged { completed }) => {
                    self.updates.stop_acknowledged();
                    let _ = completed.send(());
                }
                None => break,
            }
        }
    }
}

struct AcknowledgedMonitor<'a, B, A, O, S> {
    backend: &'a mut B,
    start_attempted: bool,
    admit: A,
    observe: O,
    stopped: S,
}
impl<
    B: Monitor,
    A: Fn(&InputEvent) -> anyhow::Result<bool>,
    O: FnMut(&str, bool) -> anyhow::Result<()>,
    S: FnMut() -> anyhow::Result<()>,
> Cleanup for AcknowledgedMonitor<'_, B, A, O, S>
{
    fn stop(&mut self) -> anyhow::Result<()> {
        self.backend.stop(&mut |events| {
            for event in events {
                (self.observe)(event.json_event(), (self.admit)(&event)?)?;
            }
            Ok(())
        })?;
        if self.start_attempted {
            (self.stopped)()?;
        }
        Ok(())
    }
    fn shutdown(&mut self) -> anyhow::Result<()> {
        self.backend.shutdown()
    }
    fn terminate(&mut self) -> anyhow::Result<()> {
        self.backend.terminate()
    }
}

fn capture_with(
    backend: &mut impl Monitor,
    product_id: u32,
    active: &AtomicBool,
    commands: &MonitorRequests,
    admit: impl Fn(&InputEvent) -> anyhow::Result<bool>,
    observe: impl FnMut(&str, bool) -> anyhow::Result<()>,
    stopped: impl FnMut() -> anyhow::Result<()>,
    mut started: impl FnMut(),
    mut wait: impl FnMut(),
) -> Completion {
    let mut backend = AcknowledgedMonitor {
        backend,
        start_attempted: false,
        admit,
        observe,
        stopped,
    };
    let mut start_acknowledged = false;
    let result = guarded(|| {
        check(active)?;
        anyhow::ensure!(
            razer_device::mouse_dynamic::capability(product_id).is_some(),
            "Dynamic movement product is unsupported"
        );
        backend.start_attempted = true;
        backend.backend.start()?;
        start_acknowledged = true;
        started();
        while active.load(Ordering::Acquire) {
            commands.dispatch(backend.backend);
            if !active.load(Ordering::Acquire) {
                break;
            }
            for event in backend.backend.events()? {
                // The connection-scoped registration controls whether the
                // source callback still consumes events after STOP admission.
                let admitted = (backend.admit)(&event)?;
                (backend.observe)(event.json_event(), admitted)?;
            }
            wait();
        }
        Ok(Outcome::MonitoringStopped)
    });
    // Close admission before draining accepted STARTs. They precede the one
    // native STOP even when the UI canceled while an earlier request was in I/O.
    commands.starts.close();
    if start_acknowledged {
        commands.dispatch(backend.backend);
    } else {
        commands.reject(
            result
                .as_ref()
                .err()
                .map_or("Dynamic capture has no acknowledged START", String::as_str),
        );
    }
    let cleanup_failure = release(&mut backend, &mut wait);
    Completion {
        result,
        cleanup_failure,
    }
}

pub fn monitor(
    route: ObservedDevice,
    active: Arc<AtomicBool>,
    history: SpeedHistoryInput,
    commands: MonitorRequests,
    started: impl FnMut(),
) -> Completion {
    // No canceled request guard is installed on this client: the same retained
    // owner must still send STOP after its page guard becomes false.
    let lease = guarded(|| {
        check(&active)?;
        razer_ipc::KEY_CAPTURE_LIFECYCLE
            .try_lock()
            .map_err(|_| anyhow::anyhow!("Input capture lifecycle is occupied"))
    });
    let _lease = match lease {
        Ok(lease) => lease,
        Err(error) => {
            return Completion {
                result: Err(error),
                cleanup_failure: None,
            };
        }
    };
    let backend = guarded(|| {
        check(&active)?;
        Ok(NativeMonitor {
            client: ServiceClient::spawn()?,
            session: Session::new(uuid::Uuid::new_v4().to_string())?,
        })
    });
    let mut backend = match backend {
        Ok(backend) => backend,
        Err(error) => {
            return Completion {
                result: Err(error),
                cleanup_failure: None,
            };
        }
    };
    let completion = capture_with(
        &mut backend,
        route.product_id(),
        &active,
        &commands,
        |event| {
            if history.is_closed() || !history.has_observer() {
                Ok(false)
            } else {
                admits(&route, event)
            }
        },
        |json, admitted| {
            if history.is_closed() {
                Ok(())
            } else {
                history.observe(json, admitted)
            }
        },
        || history.stop_acknowledged(),
        started,
        || std::thread::sleep(Duration::from_millis(8)),
    );
    drop(backend);
    completion
}

#[cfg(test)]
#[path = "mouse_dynamic_tests.rs"]
mod tests;
