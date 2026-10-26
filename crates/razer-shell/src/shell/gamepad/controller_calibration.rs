//! Current trigger intent -> owned worker -> genuine progress/storage observations.
use super::*;
use anyhow::{Context as _, ensure};
use razer_ipc::{ServiceClient, ServiceRequest, ShutdownConfirmation};
use razer_pages::features::gamepad_products::{
    TriggerCalibrationIntent, TriggerCalibrationObservation,
};
use serde_json::Value;
use std::{
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

type WorkerThread = JoinHandle<anyhow::Result<CleanupReceipt>>;

#[derive(Clone, Debug, PartialEq, Eq)]
struct CleanupOwner {
    workspace: String,
    scope: razer_ipc::device_tasks::Scope,
    physical_product_id: u32,
}
impl CleanupOwner {
    fn same_device(&self, other: &Self) -> bool {
        self.workspace.eq_ignore_ascii_case(&other.workspace)
            && self.scope == other.scope
            && self.physical_product_id == other.physical_product_id
    }
}

pub(super) struct Worker {
    owner: CleanupOwner,
    thread: WorkerThread,
}

pub(super) struct Session {
    generation: u64,
    canceled: Arc<AtomicBool>,
    worker: Option<Worker>,
    _pump: Task<()>,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.canceled.store(true, Ordering::Release);
    }
}
impl Session {
    fn retire(mut self) -> Option<Worker> {
        self.canceled.store(true, Ordering::Release);
        self.worker.take()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct CleanupReceipt {
    history: Vec<String>,
}
impl CleanupReceipt {
    fn record_history(&self) {
        for diagnostic in &self.history {
            log::warn!("[controller-calibration cleanup confirmed; prior diagnostic] {diagnostic}");
        }
    }
}

type CleanupResult = Result<CleanupReceipt, String>;

pub(super) struct StopCleanup {
    owners: Vec<CleanupOwner>,
    task: Option<Task<()>>,
    completion: Arc<CleanupCompletion>,
}
impl StopCleanup {
    fn failed(owner: CleanupOwner, error: String) -> Self {
        let completion = Arc::new(CleanupCompletion::default());
        completion.complete(Err(error));
        Self {
            owners: vec![owner],
            task: None,
            completion,
        }
    }

    fn outcome(&self) -> Option<CleanupResult> {
        self.completion.outcome()
    }

    async fn finish(self) -> CleanupResult {
        if let Some(task) = self.task {
            task.await;
        }
        self.completion.finished().await
    }
}

fn stop_barriers(stopped: &[StopCleanup], owner: &CleanupOwner) -> Vec<Arc<CleanupCompletion>> {
    stopped
        .iter()
        .filter(|stopped| stopped.owners.iter().any(|old| old.same_device(owner)))
        .map(|stopped| Arc::clone(&stopped.completion))
        .collect()
}

#[derive(Default)]
struct CleanupCompletion {
    outcome: Mutex<Option<CleanupResult>>,
    ready: Condvar,
    waiters: Mutex<Vec<std::task::Waker>>,
    #[cfg(test)]
    wait_started: Option<SyncSender<()>>,
}
impl CleanupCompletion {
    fn complete(&self, outcome: CleanupResult) {
        let mut slot = self
            .outcome
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *slot = Some(outcome);
        let waiters = std::mem::take(
            &mut *self
                .waiters
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        drop(slot);
        self.ready.notify_all();
        for waiter in waiters {
            waiter.wake();
        }
    }

    fn outcome(&self) -> Option<CleanupResult> {
        match self.outcome.lock() {
            Ok(outcome) => outcome.clone(),
            Err(_) => Some(Err("Calibration cleanup result lock failed".into())),
        }
    }

    fn wait(&self) -> CleanupResult {
        let mut outcome = self
            .outcome
            .lock()
            .map_err(|_| "Calibration cleanup result lock failed".to_owned())?;
        loop {
            if let Some(outcome) = outcome.as_ref() {
                return outcome.clone();
            }
            #[cfg(test)]
            if let Some(waiting) = &self.wait_started {
                let _ = waiting.try_send(());
            }
            outcome = self
                .ready
                .wait(outcome)
                .map_err(|_| "Calibration cleanup result wait failed".to_owned())?;
        }
    }

    async fn finished(&self) -> CleanupResult {
        std::future::poll_fn(|cx| {
            let outcome = match self.outcome.lock() {
                Ok(outcome) => outcome,
                Err(_) => {
                    return std::task::Poll::Ready(Err(
                        "Calibration cleanup result lock failed".into()
                    ));
                }
            };
            if let Some(outcome) = outcome.as_ref() {
                return std::task::Poll::Ready(outcome.clone());
            }
            let mut waiters = match self.waiters.lock() {
                Ok(waiters) => waiters,
                Err(_) => {
                    return std::task::Poll::Ready(Err(
                        "Calibration cleanup waiter lock failed".into()
                    ));
                }
            };
            if !waiters.iter().any(|waiter| waiter.will_wake(cx.waker())) {
                waiters.push(cx.waker().clone());
            }
            std::task::Poll::Pending
        })
        .await
    }
}

struct ExitCleanup {
    workers: Vec<(Worker, Arc<CleanupCompletion>)>,
    stopped: Vec<StopCleanup>,
}
impl ExitCleanup {
    fn retain(workers: Vec<Worker>, stopped: Vec<StopCleanup>) -> (Self, Vec<StopCleanup>) {
        let mut retained = stopped
            .iter()
            .map(|stopped| StopCleanup {
                owners: stopped.owners.clone(),
                task: None,
                completion: Arc::clone(&stopped.completion),
            })
            .collect::<Vec<_>>();
        let workers = workers
            .into_iter()
            .map(|worker| {
                let completion = Arc::new(CleanupCompletion::default());
                retained.push(StopCleanup {
                    owners: vec![worker.owner.clone()],
                    task: None,
                    completion: Arc::clone(&completion),
                });
                (worker, completion)
            })
            .collect();
        (Self { workers, stopped }, retained)
    }

    async fn finish(self) -> Vec<String> {
        let mut errors = Vec::new();
        for stopped in self.stopped {
            match stopped.finish().await {
                Ok(receipt) => receipt.record_history(),
                Err(error) => errors.push(error),
            }
        }
        for (worker, completion) in self.workers {
            let outcome = join_worker(worker.thread);
            completion.complete(outcome.clone());
            match outcome {
                Ok(receipt) => receipt.record_history(),
                Err(error) => errors.push(error),
            }
        }
        errors
    }
}

struct RunOutcome {
    operation: anyhow::Result<()>,
    cleanup: anyhow::Result<CleanupReceipt>,
}
impl RunOutcome {
    fn failure(&self) -> Option<String> {
        let errors = self
            .operation
            .as_ref()
            .err()
            .into_iter()
            .chain(self.cleanup.as_ref().err())
            .map(|error| format!("{error:#}"))
            .collect::<Vec<_>>();
        (!errors.is_empty()).then(|| errors.join("; "))
    }
}

fn join_worker(worker: WorkerThread) -> CleanupResult {
    worker
        .join()
        .map_err(|_| "Calibration cleanup thread failed".to_owned())?
        .map_err(|error| format!("{error:#}"))
}

fn join_workers(workers: impl IntoIterator<Item = WorkerThread>) -> CleanupResult {
    combine_cleanup_results(workers.into_iter().map(join_worker))
}

fn combine_cleanup_results(results: impl IntoIterator<Item = CleanupResult>) -> CleanupResult {
    let mut receipt = CleanupReceipt::default();
    let mut errors = Vec::new();
    for result in results {
        match result {
            Ok(value) => receipt.history.extend(value.history),
            Err(error) => errors.push(error),
        }
    }
    if errors.is_empty() {
        Ok(receipt)
    } else {
        errors.extend(receipt.history);
        Err(errors.join("; "))
    }
}

fn cleanup_outcome(
    cancellation: anyhow::Result<()>,
    shutdown: anyhow::Result<Value>,
) -> anyhow::Result<CleanupReceipt> {
    let errors = [cancellation, shutdown.map(|_| ())]
        .into_iter()
        .filter_map(|outcome| outcome.err().map(|error| format!("{error:#}")))
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(CleanupReceipt::default())
    } else {
        Err(anyhow::anyhow!(errors.join("; ")))
    }
}

fn owns_observations(
    current_generation: u64,
    current_owner: &Arc<AtomicBool>,
    generation: u64,
    owner: &Arc<AtomicBool>,
) -> bool {
    current_generation == generation && Arc::ptr_eq(current_owner, owner)
}

struct CalibrationOperation<'a> {
    node: &'a razer_device::backend::HidNode,
    product_id: u32,
    part: u8,
    id: &'a str,
}

enum Event {
    Accepted,
    Poll(Value),
    Failure(String),
    CleanupDiagnostic(String),
}
fn send(events: &SyncSender<Event>, event: Event) -> anyhow::Result<()> {
    events
        .try_send(event)
        .context("Calibration observation consumer unavailable")
}

struct TerminationReceipt {
    history: Vec<String>,
}

fn finish_service_termination(
    terminate: &mut dyn FnMut() -> anyhow::Result<()>,
    wait: &mut dyn FnMut(),
    events: &SyncSender<Event>,
) -> TerminationReceipt {
    let mut failures = Vec::new();
    loop {
        match terminate() {
            Ok(()) => return TerminationReceipt { history: failures },
            Err(error) => {
                let error = format!("{error:#}");
                if !failures.contains(&error) {
                    let _ = send(
                        events,
                        Event::CleanupDiagnostic(format!(
                            "Calibration service termination is still pending: {error}"
                        )),
                    );
                    log::warn!("[controller-calibration service cleanup] {error}");
                    failures.push(error);
                }
                wait();
            }
        }
    }
}

fn confirmed_cleanup(
    protocol: anyhow::Result<CleanupReceipt>,
    confirmation: Option<ShutdownConfirmation>,
    termination: TerminationReceipt,
) -> anyhow::Result<CleanupReceipt> {
    let mut receipt = match protocol {
        Ok(receipt) => receipt,
        Err(error) => CleanupReceipt {
            history: vec![format!("{error:#}")],
        },
    };
    receipt.history.extend(termination.history);
    if confirmation.is_some() {
        receipt.record_history();
        Ok(receipt)
    } else {
        let mut errors = vec!["Calibration service teardown has no confirmed reply".to_owned()];
        errors.extend(receipt.history);
        Err(anyhow::anyhow!(errors.join("; ")))
    }
}

fn run(
    route: razer_discovery::discovery::ObservedDevice,
    part: u8,
    canceled: &AtomicBool,
    events: &SyncSender<Event>,
) -> RunOutcome {
    if canceled.load(Ordering::Acquire) {
        return RunOutcome {
            operation: Ok(()),
            cleanup: Ok(CleanupReceipt::default()),
        };
    }
    let mut client = match ServiceClient::spawn() {
        Ok(client) => client,
        Err(error) => {
            return RunOutcome {
                operation: Err(error),
                cleanup: Ok(CleanupReceipt::default()),
            };
        }
    };
    let mut outcome = match razer_discovery::controller_calibration::resolve(&mut client, &route) {
        Ok(node) => {
            let operation_id = uuid::Uuid::new_v4().to_string();
            run_protocol(
                &mut |request| client.request(request),
                CalibrationOperation {
                    node: &node,
                    product_id: route.product_id(),
                    part,
                    id: &operation_id,
                },
                canceled,
                events,
                &mut || thread::sleep(Duration::from_millis(33)),
            )
        }
        Err(error) => RunOutcome {
            operation: Err(error),
            cleanup: cleanup_outcome(Ok(()), client.request(ServiceRequest::Shutdown)),
        },
    };
    // A shutdown response can still leave an unreaped process or pipe worker.
    // Keep this actual client on its owner thread until termination is observed;
    // cancellation must not drop those handles or release the successor barrier.
    let termination = finish_service_termination(
        &mut || client.finish_termination(),
        &mut || thread::sleep(Duration::from_millis(50)),
        events,
    );
    outcome.cleanup =
        confirmed_cleanup(outcome.cleanup, client.shutdown_confirmation(), termination);
    outcome
}

fn run_protocol(
    request: &mut dyn FnMut(ServiceRequest) -> anyhow::Result<Value>,
    operation: CalibrationOperation<'_>,
    canceled: &AtomicBool,
    events: &SyncSender<Event>,
    wait: &mut dyn FnMut(),
) -> RunOutcome {
    let CalibrationOperation {
        node,
        product_id,
        part,
        id: operation_id,
    } = operation;
    let mut started = false;
    let mut terminal = false;
    let result = (|| {
        if canceled.load(Ordering::Acquire) {
            return Ok(());
        }
        let reply = request(ServiceRequest::ControllerTriggerCalibrationStart {
            operation_id: operation_id.to_owned(),
            node: node.clone(),
            product_id,
            part_id: part,
        })?;
        started = true;
        ensure!(
            reply["operation_id"] == operation_id
                && reply["part_id"] == part
                && reply["state"] == "running"
                && reply["terminal"] == false,
            "Calibration acceptance does not match the request"
        );
        send(events, Event::Accepted)?;
        let mut cancel_sent = false;
        loop {
            if canceled.load(Ordering::Acquire) && !cancel_sent {
                let reply = request(ServiceRequest::ControllerTriggerCalibrationCancel {
                    operation_id: operation_id.to_owned(),
                })?;
                ensure!(
                    reply["operation_id"] == operation_id && reply["state"] == "canceling",
                    "Calibration cancellation does not match the request"
                );
                cancel_sent = true;
            }
            let reply = request(ServiceRequest::ControllerTriggerCalibrationPoll {
                operation_id: operation_id.to_owned(),
            })?;
            ensure!(
                reply["operation_id"] == operation_id && reply["observations"].is_array(),
                "Calibration poll does not match the request"
            );
            terminal = reply["terminal"]
                .as_bool()
                .context("Missing calibration terminal observation")?;
            if terminal {
                ensure!(
                    matches!(
                        reply["state"].as_str(),
                        Some("completed" | "failed" | "canceled")
                    ),
                    "Unknown calibration terminal state"
                );
                if reply["state"] == "completed" {
                    ensure!(
                        reply["result"]["node"] == serde_json::to_value(&node)?
                            && reply["result"]["product_id"] == product_id
                            && reply["result"]["part_id"] == part
                            && reply["result"]["device_write_verified"] == true,
                        "Calibration completion lacks exact device readback"
                    );
                }
            } else {
                ensure!(
                    matches!(reply["state"].as_str(), Some("running" | "canceling")),
                    "Unknown calibration running state"
                );
            }
            // Apply observed progress before handling its terminal outcome.
            if !reply["observations"].as_array().unwrap().is_empty() || terminal {
                send(events, Event::Poll(reply))?;
            }
            if terminal {
                break;
            }
            wait();
        }
        Ok::<_, anyhow::Error>(())
    })();
    let cancellation = if started && !terminal {
        request(ServiceRequest::ControllerTriggerCalibrationCancel {
            operation_id: operation_id.to_owned(),
        })
        .and_then(|reply| {
            ensure!(
                reply["operation_id"] == operation_id && reply["state"] == "canceling",
                "Calibration cleanup cancellation does not match the request"
            );
            Ok(())
        })
        .context("Calibration cancellation cleanup failed")
    } else {
        Ok(())
    };
    // Worker Drop cancels and joins any operation before Shutdown replies.
    let shutdown = request(ServiceRequest::Shutdown).context("Calibration service shutdown failed");
    RunOutcome {
        operation: result,
        cleanup: cleanup_outcome(cancellation, shutdown),
    }
}

fn run_after_cleanup(
    previous: impl IntoIterator<Item = WorkerThread>,
    stopped: &[Arc<CleanupCompletion>],
    canceled: &AtomicBool,
    events: &SyncSender<Event>,
    operation: impl FnOnce() -> RunOutcome,
) -> anyhow::Result<CleanupReceipt> {
    let cleanup = combine_cleanup_results(
        std::iter::once(join_workers(previous))
            .chain(stopped.iter().map(|completion| completion.wait())),
    );
    let previous = match cleanup {
        Ok(receipt) => receipt,
        Err(error) => {
            let _ = send(events, Event::Failure(error.clone()));
            return Err(anyhow::anyhow!(error));
        }
    };
    if canceled.load(Ordering::Acquire) {
        return Ok(previous);
    }
    let outcome = operation();
    if let Some(error) = outcome.failure() {
        let _ = send(events, Event::Failure(error));
    }
    // A failed calibration with confirmed cleanup can be retried. Only cleanup
    // failure prevents the next owner from using this device/service boundary.
    combine_cleanup_results([
        Ok(previous),
        outcome.cleanup.map_err(|error| format!("{error:#}")),
    ])
    .map_err(anyhow::Error::msg)
}

impl AppShell {
    fn report_controller_calibration_cleanup(&mut self, error: String, cx: &mut Context<Self>) {
        self.status = format!("手柄校准清理未确认：{error}");
        self.defer_service_exit_cancellation();
        cx.notify();
    }

    pub(super) fn install_controller_calibration_cleanup(&mut self, _cx: &mut Context<Self>) {
        self.register_exit_cleanup(|this, cx| {
            let sessions = std::mem::take(&mut this.controller_calibrations);
            let mut retired = std::mem::take(&mut this.controller_calibration_cleanup);
            let stopped = std::mem::take(&mut this.controller_calibration_stop_cleanup);
            for (_, session) in sessions {
                retired.extend(session.retire());
            }
            let (cleanup, retained) = ExitCleanup::retain(retired, stopped);
            this.controller_calibration_stop_cleanup = retained;
            let cleanup = cx.background_executor().spawn(cleanup.finish());
            cx.spawn(async move |this, cx| {
                let errors = cleanup.await;
                if !errors.is_empty() {
                    let error = errors.join("; ");
                    eprintln!("[controller-calibration cleanup] {error}");
                    let _ = this.update(cx, |this, cx| {
                        this.report_controller_calibration_cleanup(error, cx)
                    });
                }
            })
        });
    }

    pub(super) fn request_controller_calibration(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: TriggerCalibrationIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let generation = request.generation();
        if workspace.read(cx).trigger_calibration_generation(cx) != Some(generation) {
            return;
        }
        if request.payload()["action"] != "stop" {
            self.initialize_controller_calibration_focus(&workspace, window, cx);
        }
        let identity = workspace.read(cx).identity(cx);
        // Retain failed cleanup receipts so another Start cannot erase an
        // unconfirmed service lifetime. Successful receipts release their owner.
        let mut pending = Vec::new();
        for worker in std::mem::take(&mut self.controller_calibration_cleanup) {
            if worker.thread.is_finished() {
                match join_worker(worker.thread) {
                    Ok(receipt) => receipt.record_history(),
                    Err(error) => {
                        self.report_controller_calibration_cleanup(error.clone(), cx);
                        if !self
                            .controller_calibration_stop_cleanup
                            .iter()
                            .any(|stopped| {
                                stopped
                                    .owners
                                    .iter()
                                    .any(|owner| owner.same_device(&worker.owner))
                                    && matches!(stopped.outcome(), Some(Err(_)))
                            })
                        {
                            self.controller_calibration_stop_cleanup
                                .push(StopCleanup::failed(worker.owner, error));
                        }
                    }
                }
            } else {
                pending.push(worker);
            }
        }
        self.controller_calibration_cleanup = pending;
        let mut pending = Vec::new();
        for stopped in std::mem::take(&mut self.controller_calibration_stop_cleanup) {
            match stopped.outcome() {
                Some(Ok(receipt)) => receipt.record_history(),
                Some(Err(error)) => {
                    self.report_controller_calibration_cleanup(error, cx);
                    pending.push(stopped);
                }
                None => pending.push(stopped),
            }
        }
        self.controller_calibration_stop_cleanup = pending;
        let previous = self
            .controller_calibrations
            .remove(&identity)
            .and_then(Session::retire);
        if request.payload()["action"] == "stop" {
            // Stop acknowledgement follows actual worker cleanup; it is never
            // converted into a device completion or synthetic Step_Error.
            let (finished, receipt) = mpsc::channel();
            let completion = Arc::new(CleanupCompletion::default());
            let published = Arc::clone(&completion);
            let mut workers = Vec::new();
            let mut remaining = Vec::new();
            for worker in std::mem::take(&mut self.controller_calibration_cleanup) {
                if worker.owner.workspace.eq_ignore_ascii_case(&identity) {
                    workers.push(worker);
                } else {
                    remaining.push(worker);
                }
            }
            self.controller_calibration_cleanup = remaining;
            workers.extend(previous);
            let mut owners = workers
                .iter()
                .map(|worker| worker.owner.clone())
                .collect::<Vec<_>>();
            let mut predecessors = Vec::new();
            let mut remaining = Vec::new();
            for stopped in std::mem::take(&mut self.controller_calibration_stop_cleanup) {
                if stopped
                    .owners
                    .iter()
                    .any(|owner| owner.workspace.eq_ignore_ascii_case(&identity))
                {
                    owners.extend(stopped.owners.iter().cloned());
                    predecessors.push(stopped);
                } else {
                    remaining.push(stopped);
                }
            }
            self.controller_calibration_stop_cleanup = remaining;
            owners.sort_by(|left, right| {
                left.workspace
                    .cmp(&right.workspace)
                    .then_with(|| left.scope.cmp(&right.scope))
                    .then_with(|| left.physical_product_id.cmp(&right.physical_product_id))
            });
            owners.dedup();
            let cleanup = cx.background_executor().spawn(async move {
                let mut outcomes = Vec::new();
                for predecessor in predecessors {
                    outcomes.push(predecessor.finish().await);
                }
                outcomes.push(join_workers(
                    workers.into_iter().map(|worker| worker.thread),
                ));
                let outcome = combine_cleanup_results(outcomes);
                published.complete(outcome.clone());
                let _ = finished.send(outcome);
            });
            self.controller_calibration_stop_cleanup.push(StopCleanup {
                owners,
                task: Some(cleanup),
                completion,
            });
            cx.spawn_in(window, async move |_, cx| {
                let outcome = loop {
                    match receipt.try_recv() {
                        Ok(outcome) => break outcome,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            break Err("Calibration cleanup result unavailable".into());
                        }
                        Err(mpsc::TryRecvError::Empty) => {
                            cx.background_executor()
                                .timer(Duration::from_millis(33))
                                .await
                        }
                    }
                };
                let _ = workspace.update(cx, |workspace, cx| {
                    workspace.finish_trigger_calibration_submission(
                        generation,
                        outcome.map(|_| ()),
                        cx,
                    )
                });
            })
            .detach();
            return;
        }
        let device = workspace.read(cx).device(cx);
        let routes = self
            .device_observations
            .iter()
            .filter(|route| route.matches(device))
            .collect::<Vec<_>>();
        let [route] = routes.as_slice() else {
            self.controller_calibration_cleanup.extend(previous);
            workspace.update(cx, |workspace, cx| {
                workspace.finish_trigger_calibration_submission(
                    generation,
                    Err("No unique current calibration device identity".into()),
                    cx,
                )
            });
            return;
        };
        let route = (**route).clone();
        let scope = match route.device_task_scope() {
            Ok(scope) => scope,
            Err(error) => {
                self.controller_calibration_cleanup.extend(previous);
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_trigger_calibration_submission(
                        generation,
                        Err(format!("{error:#}")),
                        cx,
                    )
                });
                return;
            }
        };
        let cleanup_owner = CleanupOwner {
            workspace: identity.clone(),
            scope,
            physical_product_id: route.physical_product_id(),
        };
        let stopped = stop_barriers(&self.controller_calibration_stop_cleanup, &cleanup_owner);
        let mut predecessors = previous.into_iter().collect::<Vec<_>>();
        let mut remaining = Vec::new();
        for worker in std::mem::take(&mut self.controller_calibration_cleanup) {
            if worker.owner.same_device(&cleanup_owner) {
                predecessors.push(worker);
            } else {
                remaining.push(worker);
            }
        }
        self.controller_calibration_cleanup = remaining;
        let container = device.device_container_id.clone();
        let physical_pid = route.physical_product_id();
        let retained_node = route.hid_node().cloned();
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = Arc::clone(&canceled);
        let (events, receiver) = mpsc::sync_channel(512);
        // Keep the retired handle recoverable if OS thread creation fails.
        let predecessor = Arc::new(std::sync::Mutex::new(Some(predecessors)));
        let previous_worker = Arc::clone(&predecessor);
        let worker = match thread::Builder::new()
            .name("controller-calibration-owner".into())
            .spawn(move || {
                let previous = previous_worker
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Calibration predecessor lock failed"))?
                    .take()
                    .context("Calibration predecessors already taken")?;
                run_after_cleanup(
                    previous.into_iter().map(|worker| worker.thread),
                    &stopped,
                    &signal,
                    &events,
                    || run(route, request.part(), &signal, &events),
                )
            }) {
            Ok(worker) => worker,
            Err(error) => {
                self.controller_calibration_cleanup
                    .extend(predecessor.lock().unwrap().take().into_iter().flatten());
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_trigger_calibration_submission(
                        generation,
                        Err(error.to_string()),
                        cx,
                    )
                });
                return;
            }
        };
        let owner = identity.clone();
        let observation_owner = Arc::clone(&canceled);
        let pump = cx.spawn_in(window, async move |this, cx| {
            let mut sequence = 0_u64;
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(33))
                    .await;
                let mut incoming = Vec::new();
                let disconnected = loop {
                    match receiver.try_recv() {
                        Ok(event) => incoming.push(event),
                        Err(mpsc::TryRecvError::Empty) => break false,
                        Err(mpsc::TryRecvError::Disconnected) => break true,
                    }
                };
                let keep = this.update_in(cx, |this, window, cx| {
                    if !this.controller_calibrations.get(&owner).is_some_and(|s| {
                        owns_observations(s.generation, &s.canceled, generation, &observation_owner)
                    }) {
                        return false;
                    }
                    let current = this.devices.contains(&workspace)
                        && workspace.read(cx).identity(cx) == owner
                        && workspace.read(cx).trigger_calibration_generation(cx)
                            == Some(generation)
                        && this.device_observations.iter().any(|route| {
                            route.matches(workspace.read(cx).device(cx))
                                && route.container().eq_ignore_ascii_case(&container)
                                && route.physical_product_id() == physical_pid
                                && route.hid_node() == retained_node.as_ref()
                        });
                    if !current {
                        workspace.update(cx, |workspace, cx| {
                            workspace.finish_trigger_calibration_submission(
                                generation,
                                Err("Calibration device identity changed".into()),
                                cx,
                            )
                        });
                    } else {
                        for event in incoming {
                            match event {
                                Event::Accepted => {
                                    workspace.update(cx, |w, cx| {
                                        w.finish_trigger_calibration_submission(
                                            generation,
                                            Ok(()),
                                            cx,
                                        )
                                    });
                                }
                                Event::Failure(error) => {
                                    workspace.update(cx, |w, cx| {
                                        w.finish_trigger_calibration_submission(
                                            generation,
                                            Err(error),
                                            cx,
                                        )
                                    });
                                }
                                Event::CleanupDiagnostic(diagnostic) => {
                                    log::warn!(
                                        "[controller-calibration cleanup pending] {diagnostic}"
                                    );
                                }
                                Event::Poll(reply) => {
                                    for value in reply["observations"].as_array().unwrap() {
                                        sequence = sequence.wrapping_add(1);
                                        let observation = match value["kind"].as_str() {
                                            Some("progress") => value["payload"]["step"]
                                                .as_i64()
                                                .and_then(|s| i8::try_from(s).ok())
                                                .zip(value["payload"]["isStepValid"].as_bool())
                                                .and_then(|(step, valid)| {
                                                    TriggerCalibrationObservation::progress(
                                                        generation, sequence, step, valid, None,
                                                    )
                                                }),
                                            Some("movement") => {
                                                value["payload"]["t"].as_f64().and_then(|t| {
                                                    TriggerCalibrationObservation::user_movement(
                                                        generation, sequence, t,
                                                    )
                                                })
                                            }
                                            // Source storage listener ignores null newValue;
                                            // removal must preserve its last actual trigger t.
                                            Some("remove_movement") => None,
                                            _ => None,
                                        };
                                        if let Some(observation) = observation {
                                            workspace.update(cx, |w, cx| {
                                                w.observe_trigger_calibration(
                                                    observation,
                                                    window,
                                                    cx,
                                                )
                                            });
                                        }
                                    }
                                    if reply["terminal"] == true && reply["state"] == "failed" {
                                        let error = reply["result"]["error"]
                                            .as_str()
                                            .or_else(|| reply["error"].as_str())
                                            .unwrap_or("Calibration operation failed")
                                            .to_owned();
                                        workspace.update(cx, |w, cx| {
                                            w.finish_trigger_calibration_submission(
                                                generation,
                                                Err(error),
                                                cx,
                                            )
                                        });
                                    }
                                }
                            }
                        }
                    }
                    if !current || disconnected {
                        this.controller_calibration_cleanup.extend(
                            this.controller_calibrations
                                .remove(&owner)
                                .and_then(Session::retire),
                        );
                        return false;
                    }
                    true
                });
                if !matches!(keep, Ok(true)) {
                    break;
                }
            }
        });
        self.controller_calibrations.insert(
            identity,
            Session {
                generation,
                canceled,
                worker: Some(Worker {
                    owner: cleanup_owner,
                    thread: worker,
                }),
                _pump: pump,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{cell::Cell, future::Future as _};

    fn node() -> razer_device::backend::HidNode {
        razer_device::backend::HidNode {
            path: b"calibration-fixture-current".to_vec(),
            vendor_id: 5426,
            product_id: 2676,
            usage_page: 1,
            usage: 5,
            interface_number: 1,
            release_number: 1,
            serial_number: Some("fixture-current".into()),
            manufacturer: None,
            product: None,
        }
    }

    fn accepted() -> Value {
        json!({"operation_id":"fixture-operation","part_id":3,"state":"running","terminal":false})
    }

    fn operation(node: &razer_device::backend::HidNode) -> CalibrationOperation<'_> {
        CalibrationOperation {
            node,
            product_id: 2676,
            part: 3,
            id: "fixture-operation",
        }
    }

    fn ready<T>(future: impl std::future::Future<Output = T>) -> T {
        let mut future = std::pin::pin!(future);
        let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
        match future.as_mut().poll(&mut cx) {
            std::task::Poll::Ready(value) => value,
            std::task::Poll::Pending => panic!("Expected a completed cleanup receipt"),
        }
    }

    #[::core::prelude::v1::test]
    fn retired_worker_keeps_shutdown_failure_after_observation_consumer_has_gone() {
        let (events, consumer) = mpsc::sync_channel(8);
        drop(consumer);
        let worker = thread::spawn(move || {
            run_after_cleanup(None, &[], &AtomicBool::new(false), &events, || {
                let mut trace = Vec::new();
                let outcome = run_protocol(
                    &mut |request| {
                        trace.push(request);
                        match trace.last().unwrap() {
                            ServiceRequest::ControllerTriggerCalibrationStart { .. } => {
                                Ok(accepted())
                            }
                            ServiceRequest::ControllerTriggerCalibrationCancel { .. } => {
                                Ok(json!({"operation_id":"fixture-operation","state":"canceling"}))
                            }
                            ServiceRequest::Shutdown => {
                                Err(anyhow::anyhow!("actual service teardown failure"))
                            }
                            _ => panic!("No polling after consumer removal"),
                        }
                    },
                    operation(&node()),
                    &AtomicBool::new(false),
                    &events,
                    &mut || panic!("No timer after consumer removal"),
                );
                assert!(matches!(
                    trace.as_slice(),
                    [
                        ServiceRequest::ControllerTriggerCalibrationStart { .. },
                        ServiceRequest::ControllerTriggerCalibrationCancel { .. },
                        ServiceRequest::Shutdown
                    ]
                ));
                outcome
            })
        });
        let failure = join_worker(worker).unwrap_err();
        assert!(failure.contains("actual service teardown failure"));
    }

    #[::core::prelude::v1::test]
    fn operation_cancel_and_shutdown_errors_are_all_retained() {
        let (events, _consumer) = mpsc::sync_channel(8);
        let mut trace = Vec::new();
        let outcome = run_protocol(
            &mut |request| {
                trace.push(request);
                match trace.last().unwrap() {
                    ServiceRequest::ControllerTriggerCalibrationStart { .. } => Ok(accepted()),
                    ServiceRequest::ControllerTriggerCalibrationPoll { .. } => {
                        Err(anyhow::anyhow!("poll transport failed"))
                    }
                    ServiceRequest::ControllerTriggerCalibrationCancel { .. } => {
                        Err(anyhow::anyhow!("cancel transport failed"))
                    }
                    ServiceRequest::Shutdown => Err(anyhow::anyhow!("shutdown transport failed")),
                    _ => panic!("Unexpected request"),
                }
            },
            operation(&node()),
            &AtomicBool::new(false),
            &events,
            &mut || panic!("No wait after poll failure"),
        );
        let failure = outcome.failure().unwrap();
        for expected in [
            "poll transport failed",
            "cancel transport failed",
            "shutdown transport failed",
        ] {
            assert!(failure.contains(expected), "{failure}");
        }
        assert!(outcome.cleanup.is_err());
        assert!(matches!(trace.last(), Some(ServiceRequest::Shutdown)));
    }

    #[::core::prelude::v1::test]
    fn stop_waits_for_current_operation_cancellation_then_shutdown() {
        let canceled = AtomicBool::new(false);
        let (events, consumer) = mpsc::sync_channel(8);
        let mut trace = Vec::new();
        let outcome = run_protocol(
            &mut |request| {
                trace.push(request);
                match trace.last().unwrap() {
                    ServiceRequest::ControllerTriggerCalibrationStart { .. } => {
                        canceled.store(true, Ordering::Release);
                        Ok(accepted())
                    }
                    ServiceRequest::ControllerTriggerCalibrationCancel { operation_id } => {
                        assert_eq!(operation_id, "fixture-operation");
                        Ok(json!({"operation_id":"fixture-operation","state":"canceling"}))
                    }
                    ServiceRequest::ControllerTriggerCalibrationPoll { .. } => Ok(
                        json!({"operation_id":"fixture-operation","state":"canceled","terminal":true,"observations":[]}),
                    ),
                    ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
                    _ => panic!("Unexpected request"),
                }
            },
            operation(&node()),
            &canceled,
            &events,
            &mut || panic!("Terminal cancellation does not wait"),
        );
        assert!(outcome.operation.is_ok() && outcome.cleanup.is_ok());
        assert!(matches!(
            trace.as_slice(),
            [
                ServiceRequest::ControllerTriggerCalibrationStart { .. },
                ServiceRequest::ControllerTriggerCalibrationCancel { .. },
                ServiceRequest::ControllerTriggerCalibrationPoll { .. },
                ServiceRequest::Shutdown
            ]
        ));
        assert!(matches!(consumer.try_recv().unwrap(), Event::Accepted));
        assert!(
            matches!(consumer.try_recv().unwrap(), Event::Poll(value) if value["state"] == "canceled")
        );
    }

    #[::core::prelude::v1::test]
    fn successful_terminal_readback_still_reports_service_shutdown_failure() {
        let (events, consumer) = mpsc::sync_channel(8);
        let node = node();
        let outcome = run_protocol(
            &mut |request| match request {
                ServiceRequest::ControllerTriggerCalibrationStart { .. } => Ok(accepted()),
                ServiceRequest::ControllerTriggerCalibrationPoll { .. } => Ok(
                    json!({"operation_id":"fixture-operation","state":"completed","terminal":true,"observations":[],"result":{"node":node,"product_id":2676,"part_id":3,"device_write_verified":true}}),
                ),
                ServiceRequest::Shutdown => Err(anyhow::anyhow!("shutdown receipt missing")),
                _ => panic!("Confirmed terminal result must not request another cancellation"),
            },
            operation(&node),
            &AtomicBool::new(false),
            &events,
            &mut || panic!("Terminal success does not wait"),
        );
        assert!(outcome.operation.is_ok());
        assert!(
            outcome
                .cleanup
                .unwrap_err()
                .to_string()
                .contains("shutdown receipt missing")
        );
        assert!(matches!(consumer.try_recv().unwrap(), Event::Accepted));
        assert!(
            matches!(consumer.try_recv().unwrap(), Event::Poll(value) if value["state"] == "completed")
        );
    }

    #[::core::prelude::v1::test]
    fn replaced_operation_waits_for_cleanup_and_does_not_open_after_cleanup_failure() {
        let (released, release) = mpsc::channel();
        let previous = thread::spawn(move || {
            release.recv().unwrap();
            Err(anyhow::anyhow!("previous service cleanup failed"))
        });
        let (events, consumer) = mpsc::sync_channel(8);
        let (completed, completion) = mpsc::channel();
        let next = thread::spawn(move || {
            let opened = Cell::new(false);
            let result = run_after_cleanup(
                Some(previous),
                &[],
                &AtomicBool::new(false),
                &events,
                || {
                    opened.set(true);
                    RunOutcome {
                        operation: Ok(()),
                        cleanup: Ok(CleanupReceipt::default()),
                    }
                },
            );
            completed.send(opened.get()).unwrap();
            result
        });
        assert!(matches!(
            completion.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        released.send(()).unwrap();
        assert!(
            join_worker(next)
                .unwrap_err()
                .contains("previous service cleanup failed")
        );
        assert!(!completion.recv().unwrap());
        assert!(
            matches!(consumer.try_recv().unwrap(), Event::Failure(error) if error.contains("previous service cleanup failed"))
        );
    }

    #[::core::prelude::v1::test]
    fn business_failure_with_confirmed_cleanup_allows_retry_and_old_owner_stays_distinct() {
        let (events, consumer) = mpsc::sync_channel(8);
        let first = run_after_cleanup(None, &[], &AtomicBool::new(false), &events, || RunOutcome {
            operation: Err(anyhow::anyhow!("range readback mismatch")),
            cleanup: Ok(CleanupReceipt::default()),
        });
        assert!(first.is_ok());
        assert!(
            matches!(consumer.try_recv().unwrap(), Event::Failure(error) if error.contains("range readback mismatch"))
        );
        let old_owner = Arc::new(AtomicBool::new(false));
        let new_owner = Arc::new(AtomicBool::new(false));
        assert!(owns_observations(7, &old_owner, 7, &old_owner));
        assert!(!owns_observations(8, &new_owner, 7, &old_owner));
        assert!(!owns_observations(7, &new_owner, 7, &old_owner));
        let restarted = Cell::new(false);
        run_after_cleanup(None, &[], &AtomicBool::new(false), &events, || {
            restarted.set(true);
            RunOutcome {
                operation: Ok(()),
                cleanup: Ok(CleanupReceipt::default()),
            }
        })
        .unwrap();
        assert!(restarted.get());
    }

    #[::core::prelude::v1::test]
    fn canceled_successor_releases_previous_owner_without_starting_new_work() {
        let previous = thread::spawn(|| Ok(CleanupReceipt::default()));
        let (events, consumer) = mpsc::sync_channel(8);
        run_after_cleanup(Some(previous), &[], &AtomicBool::new(true), &events, || {
            panic!("Canceled successor must not acquire a service")
        })
        .unwrap();
        assert!(matches!(
            consumer.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }

    #[::core::prelude::v1::test]
    fn stop_collects_every_worker_failure_and_thread_panic() {
        let first = thread::spawn(|| Err(anyhow::anyhow!("first service cleanup failed")));
        let panic =
            thread::spawn(|| -> anyhow::Result<CleanupReceipt> { panic!("cleanup panic fixture") });
        let last = thread::spawn(|| Err(anyhow::anyhow!("last service cleanup failed")));
        let failure = join_workers([first, panic, last]).unwrap_err();
        for expected in [
            "first service cleanup failed",
            "Calibration cleanup thread failed",
            "last service cleanup failed",
        ] {
            assert!(failure.contains(expected), "{failure}");
        }
    }

    #[::core::prelude::v1::test]
    fn start_after_stop_waits_for_the_exact_cleanup_receipt_and_preserves_its_failure() {
        for succeeds in [true, false] {
            let (waiting, waiter) = mpsc::sync_channel(1);
            let completion = Arc::new(CleanupCompletion {
                wait_started: Some(waiting),
                ..Default::default()
            });
            let barrier = Arc::clone(&completion);
            let (opened, opening) = mpsc::channel();
            let (events, consumer) = mpsc::sync_channel(8);
            let worker_events = events.clone();
            let successor = thread::spawn(move || {
                run_after_cleanup(
                    None,
                    &[barrier],
                    &AtomicBool::new(false),
                    &worker_events,
                    || {
                        opened.send(()).unwrap();
                        RunOutcome {
                            operation: Ok(()),
                            cleanup: Ok(CleanupReceipt::default()),
                        }
                    },
                )
            });
            waiter.recv().unwrap();
            assert!(matches!(opening.try_recv(), Err(mpsc::TryRecvError::Empty)));
            assert!(matches!(
                consumer.try_recv(),
                Err(mpsc::TryRecvError::Empty)
            ));
            assert!(!successor.is_finished());
            let receipt = if succeeds {
                Ok(CleanupReceipt::default())
            } else {
                Err("stopped service did not release".into())
            };
            completion.complete(receipt.clone());
            assert_eq!(join_worker(successor), receipt);
            if succeeds {
                opening.recv().unwrap();
                assert!(matches!(
                    consumer.try_recv(),
                    Err(mpsc::TryRecvError::Empty)
                ));
            } else {
                assert!(matches!(
                    opening.try_recv(),
                    Err(mpsc::TryRecvError::Disconnected)
                ));
                assert!(matches!(consumer.try_recv().unwrap(), Event::Failure(error)
                    if error.contains("stopped service did not release")));
                assert_eq!(completion.wait(), receipt);
            }
            drop(events);
        }
    }

    #[::core::prelude::v1::test]
    fn cancellation_while_waiting_for_stop_releases_the_predecessor_without_reopening() {
        let (waiting, waiter) = mpsc::sync_channel(1);
        let completion = Arc::new(CleanupCompletion {
            wait_started: Some(waiting),
            ..Default::default()
        });
        let barrier = Arc::clone(&completion);
        let canceled = Arc::new(AtomicBool::new(false));
        let cancellation = Arc::clone(&canceled);
        let (events, consumer) = mpsc::sync_channel(8);
        let worker_events = events.clone();
        let successor = thread::spawn(move || {
            run_after_cleanup(None, &[barrier], &cancellation, &worker_events, || {
                panic!("Canceled successor must not reopen after Stop")
            })
        });
        waiter.recv().unwrap();
        assert!(matches!(
            consumer.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(!successor.is_finished());
        canceled.store(true, Ordering::Release);
        completion.complete(Ok(CleanupReceipt::default()));
        join_worker(successor).unwrap();
        assert!(matches!(
            consumer.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        drop(events);
    }

    #[::core::prelude::v1::test]
    fn stop_barriers_are_scoped_to_workspace_collection_and_physical_product() {
        let owner = CleanupOwner {
            workspace: "2676:current-container".into(),
            scope: razer_ipc::device_tasks::Scope::collection(2676, &node()),
            physical_product_id: 2676,
        };
        let completion = Arc::new(CleanupCompletion::default());
        let stopped = vec![StopCleanup {
            owners: vec![owner.clone()],
            task: None,
            completion: Arc::clone(&completion),
        }];
        let matching = stop_barriers(&stopped, &owner);
        assert_eq!(matching.len(), 1);
        assert!(Arc::ptr_eq(&matching[0], &completion));

        let mut another_workspace = owner.clone();
        another_workspace.workspace = "2676:another-container".into();
        let mut replaced_collection = owner.clone();
        let mut new_node = node();
        new_node.path = b"new-connection-generation".to_vec();
        replaced_collection.scope = razer_ipc::device_tasks::Scope::collection(2676, &new_node);
        let mut another_product = owner.clone();
        another_product.physical_product_id = 2680;
        for unrelated in [another_workspace, replaced_collection, another_product] {
            assert!(stop_barriers(&stopped, &unrelated).is_empty());
        }
    }

    #[::core::prelude::v1::test]
    fn canceled_exit_retains_each_actual_owner_receipt_and_clean_owners_can_retry() {
        let owner = CleanupOwner {
            workspace: "2676:failed-owner".into(),
            scope: razer_ipc::device_tasks::Scope::collection(2676, &node()),
            physical_product_id: 2676,
        };
        let mut clean_owner = owner.clone();
        clean_owner.workspace = "2676:clean-owner".into();
        let mut stopped_owner = owner.clone();
        stopped_owner.workspace = "2676:stopped-owner".into();
        let workers = vec![
            Worker {
                owner: owner.clone(),
                thread: thread::spawn(|| Err(anyhow::anyhow!("exit worker cleanup failed"))),
            },
            Worker {
                owner: clean_owner.clone(),
                thread: thread::spawn(|| Ok(CleanupReceipt::default())),
            },
        ];
        let stopped = vec![StopCleanup::failed(
            stopped_owner.clone(),
            "earlier Stop failed".into(),
        )];
        let (exit, retained) = ExitCleanup::retain(workers, stopped);
        assert_eq!(retained.len(), 3);
        assert!(stop_barriers(&retained, &owner)[0].outcome().is_none());
        let failures = ready(exit.finish());
        assert_eq!(failures.len(), 2);
        assert!(
            failures
                .iter()
                .any(|error| error == "exit worker cleanup failed")
        );
        assert!(failures.iter().any(|error| error == "earlier Stop failed"));

        let (events, _consumer) = mpsc::sync_channel(8);
        for failed_owner in [owner, stopped_owner] {
            let barriers = stop_barriers(&retained, &failed_owner);
            assert_eq!(barriers.len(), 1);
            assert!(
                run_after_cleanup(None, &barriers, &AtomicBool::new(false), &events, || {
                    panic!("Canceling exit must not erase the failed device lifetime")
                })
                .is_err()
            );
        }
        let opened = Cell::new(false);
        run_after_cleanup(
            None,
            &stop_barriers(&retained, &clean_owner),
            &AtomicBool::new(false),
            &events,
            || {
                opened.set(true);
                RunOutcome {
                    operation: Ok(()),
                    cleanup: Ok(CleanupReceipt::default()),
                }
            },
        )
        .unwrap();
        assert!(opened.get());
    }

    #[::core::prelude::v1::test]
    fn retained_exit_receipt_waits_asynchronously_and_wakes_on_actual_completion() {
        struct WakeCount(std::sync::atomic::AtomicUsize);
        impl std::task::Wake for WakeCount {
            fn wake(self: Arc<Self>) {
                self.0.fetch_add(1, Ordering::AcqRel);
            }
        }
        let counter = Arc::new(WakeCount(std::sync::atomic::AtomicUsize::new(0)));
        let waker = std::task::Waker::from(Arc::clone(&counter));
        let mut cx = std::task::Context::from_waker(&waker);
        let completion = CleanupCompletion::default();
        let mut receipt = std::pin::pin!(completion.finished());
        assert!(receipt.as_mut().poll(&mut cx).is_pending());
        assert!(receipt.as_mut().poll(&mut cx).is_pending());
        completion.complete(Err("real retained cleanup failure".into()));
        assert_eq!(counter.0.load(Ordering::Acquire), 1);
        assert_eq!(
            receipt.as_mut().poll(&mut cx),
            std::task::Poll::Ready(Err("real retained cleanup failure".into()))
        );
    }

    #[::core::prelude::v1::test]
    fn termination_failures_keep_the_actual_resource_owner_until_reaping_succeeds() {
        struct ResourceOwner(Arc<AtomicBool>);
        impl Drop for ResourceOwner {
            fn drop(&mut self) {
                self.0.store(false, Ordering::Release);
            }
        }
        let alive = Arc::new(AtomicBool::new(true));
        let resource = ResourceOwner(Arc::clone(&alive));
        let (waiting, waiter) = mpsc::channel();
        let (released, release) = mpsc::channel();
        let (events, consumer) = mpsc::sync_channel(8);
        let worker = thread::spawn(move || {
            let mut attempts = 0;
            let outcome = finish_service_termination(
                &mut || {
                    assert!(resource.0.load(Ordering::Acquire));
                    attempts += 1;
                    match attempts {
                        1 => Err(anyhow::anyhow!("process still owned")),
                        2 => Err(anyhow::anyhow!("pipe join still owned")),
                        3 => Ok(()),
                        _ => panic!("Termination must stop after confirmed release"),
                    }
                },
                &mut || {
                    waiting.send(()).unwrap();
                    release.recv().unwrap();
                },
                &events,
            );
            drop(resource);
            Ok(CleanupReceipt {
                history: outcome.history,
            })
        });
        for _ in 0..2 {
            waiter.recv().unwrap();
            assert!(alive.load(Ordering::Acquire));
            assert!(!worker.is_finished());
            released.send(()).unwrap();
        }
        let receipt = join_worker(worker).unwrap();
        assert_eq!(
            receipt.history,
            ["process still owned", "pipe join still owned"]
        );
        assert!(!alive.load(Ordering::Acquire));
        let diagnostics = consumer.try_iter().collect::<Vec<_>>();
        assert_eq!(diagnostics.len(), 2);
        assert!(
            diagnostics
                .iter()
                .all(|event| matches!(event, Event::CleanupDiagnostic(_)))
        );

        let completion = Arc::new(CleanupCompletion::default());
        completion.complete(Ok(receipt.clone()));
        let (events, _consumer) = mpsc::sync_channel(8);
        let restarted = Cell::new(false);
        let next = run_after_cleanup(
            None,
            &[completion],
            &AtomicBool::new(false),
            &events,
            || {
                restarted.set(true);
                RunOutcome {
                    operation: Ok(()),
                    cleanup: Ok(CleanupReceipt::default()),
                }
            },
        )
        .unwrap();
        assert!(restarted.get());
        assert_eq!(next, receipt);
    }

    #[::core::prelude::v1::test]
    fn final_process_release_cannot_replace_a_missing_service_teardown_confirmation() {
        let receipt = confirmed_cleanup(
            Err(anyhow::anyhow!("actual native owner cleanup rejected")),
            None,
            TerminationReceipt {
                history: vec!["process release retried".into()],
            },
        )
        .unwrap_err();
        let error = format!("{receipt:#}");
        assert!(error.contains("no confirmed reply"));
        assert!(error.contains("actual native owner cleanup rejected"));
        assert!(error.contains("process release retried"));

        let completion = Arc::new(CleanupCompletion::default());
        completion.complete(Err(error));
        let (events, _consumer) = mpsc::sync_channel(8);
        assert!(
            run_after_cleanup(
                None,
                &[completion],
                &AtomicBool::new(false),
                &events,
                || { panic!("OS process release cannot acknowledge native service restoration") }
            )
            .is_err()
        );
    }
}
