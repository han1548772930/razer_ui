//! The mounted Uma middleware owns initialization and later indicator writes.
//! Its retained worker closes the service and joins before its owner is released.
use super::*;
use anyhow::{Context as _, ensure};
use razer_device::hyperpolling_receiver::{
    self as uma, Command as DeviceCommand, Reading, Submission,
};
use razer_discovery::{
    discovery::ReceiverQueryProjection,
    receiver::{ReceiverRoute, hyperpolling as direct},
};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_pages::features::HyperpollingIndicatorRequested as Request;
use razer_storage::hyperpolling_profile::IndicatorEdit;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

static EPOCHS: AtomicU64 = AtomicU64::new(1);
#[path = "hyperpolling_runtime/pairing.rs"]
pub(super) mod pairing;

enum Command {
    Set(Request),
    Bindings {
        session: u64,
        valid: Arc<AtomicBool>,
    },
    Stop,
}
enum Update {
    Serial(String),
    Edition(uma::Edition),
    Peers(ReceiverQueryProjection),
    Bindings(u64, Result<ReceiverQueryProjection, String>),
    InitialStatus(u8),
    Submitted(Request, Submission),
    Unconfirmed(Request, String),
    Diagnostic(String),
    Stopped,
}

pub(super) struct Session {
    workspace: WeakEntity<ProductWorkspace>,
    connection_revision: u64,
    epoch: u64,
    route: ReceiverRoute,
    serial: Option<String>,
    metadata: Option<uma::Edition>,
    active: Option<Request>,
    stopped: bool,
    pairing_active: Arc<AtomicU64>,
    canceled: Arc<AtomicBool>,
    commands: Sender<Command>,
    worker: Option<JoinHandle<()>>,
    _delivery: Task<()>,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.canceled.store(true, Ordering::Release);
        if let Some(request) = &self.active {
            request.guard().store(false, Ordering::Release);
        }
        let _ = self.commands.send(Command::Stop);
    }
}
pub(super) struct Retired {
    identity: String,
    worker: JoinHandle<()>,
}

pub(super) struct LocalSave {
    identity: String,
    workspace: Entity<ProductWorkspace>,
    connection_revision: u64,
    request: Request,
    edit: IndicatorEdit,
}
impl LocalSave {
    pub(super) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        (&self.identity, self.workspace.downgrade())
    }
}

fn validate(canceled: &AtomicBool) -> anyhow::Result<()> {
    ensure!(!canceled.load(Ordering::Acquire), "Uma owner canceled");
    Ok(())
}
fn diagnostic(sender: &Sender<Update>, error: impl std::fmt::Display) {
    let _ = sender.send(Update::Diagnostic(error.to_string()));
}

trait Transport {
    fn command(&mut self, command: DeviceCommand) -> anyhow::Result<Reading>;
    fn peers(&mut self) -> anyhow::Result<ReceiverQueryProjection>;
    fn edition_retry_delay(&mut self) {
        thread::sleep(Duration::from_millis(100));
    }
    fn healthy(&self) -> bool {
        true
    }
}
struct ServiceTransport<'a> {
    client: &'a mut ServiceClient,
    route: &'a ReceiverRoute,
    target: uma::Target,
}
impl Transport for ServiceTransport<'_> {
    fn command(&mut self, command: DeviceCommand) -> anyhow::Result<Reading> {
        direct::execute(self.client, &self.target, command)
    }
    fn peers(&mut self) -> anyhow::Result<ReceiverQueryProjection> {
        self.route.query(self.client)
    }
    fn healthy(&self) -> bool {
        !self.client.is_stopped()
    }
}

/// Current31468.gn reads serial/edition before initDualLink; the empty serial
/// gate follows that query. Indicator GET enters the ordinary storage/SET queue.
#[derive(Default)]
struct Initialization {
    serial_observed: bool,
    complete: bool,
}
fn initialize(
    transport: &mut impl Transport,
    canceled: &AtomicBool,
    updates: &Sender<Update>,
    state: &mut Initialization,
) -> anyhow::Result<()> {
    validate(canceled)?;
    if !state.serial_observed {
        match transport.command(DeviceCommand::Serial) {
            Ok(Reading::Serial(serial)) => {
                updates.send(Update::Serial(serial.source_identity()))?;
                state.serial_observed = true;
            }
            Ok(_) => anyhow::bail!("Uma serial response type differs"),
            Err(error) => diagnostic(updates, format!("Uma serial read failed: {error:#}")),
        }
        validate(canceled)?;
        // getEdition retries three times before runtime's catch chooses its
        // source-supported edition0. Failed reads do not become observations.
        for attempt in 0..3 {
            match transport.command(DeviceCommand::Edition) {
                Ok(Reading::Edition(metadata)) => {
                    updates.send(Update::Edition(metadata))?;
                    break;
                }
                Ok(_) => anyhow::bail!("Uma edition response type differs"),
                Err(error) => {
                    if attempt == 2 {
                        diagnostic(updates, format!("Uma edition read failed: {error:#}"));
                    } else {
                        transport.edition_retry_delay();
                    }
                    validate(canceled)?;
                }
            }
        }
    }
    validate(canceled)?;
    match transport.peers() {
        Ok(peers) => {
            updates.send(Update::Peers(peers))?;
        }
        Err(error) => diagnostic(updates, error),
    }
    if !state.serial_observed {
        return Ok(());
    }
    validate(canceled)?;
    match transport.command(DeviceCommand::Indicator)? {
        Reading::Indicator {
            status: Some(status),
        } if status != 0 => {
            updates.send(Update::InitialStatus(status))?;
        }
        Reading::Indicator { .. } => {}
        _ => anyhow::bail!("Uma indicator response type differs"),
    }
    state.complete = true;
    Ok(())
}

/// Each SET has its own cancellable IPC process. Superseding one request can
/// terminate its remaining retries without invalidating the connection owner.
fn submit(
    route: &ReceiverRoute,
    canceled: &AtomicBool,
    request: &Request,
) -> anyhow::Result<Submission> {
    validate(canceled)?;
    ensure!(request.is_valid(), "Uma indicator request replaced");
    let mut client = ServiceClient::spawn()?;
    client.set_request_guard(request.guard());
    let result = (|| {
        let target = direct::resolve(&mut client, route)?;
        validate(canceled)?;
        ensure!(
            request.is_valid(),
            "Uma indicator request replaced before SET"
        );
        match direct::execute(
            &mut client,
            &target,
            DeviceCommand::SetIndicator {
                status: request.status(),
            },
        )? {
            Reading::Submitted(receipt) => Ok(receipt),
            _ => anyhow::bail!("Uma submission response type differs"),
        }
    })();
    let cleanup = super::receiver_reset::local::shutdown(&mut client);
    match (result, cleanup) {
        (Ok(mut receipt), Err(error)) => {
            let message = format!("Uma SET worker cleanup is unconfirmed: {error:#}");
            receipt.error = Some(
                receipt
                    .error
                    .map_or(message.clone(), |error| format!("{error}; {message}")),
            );
            Ok(receipt)
        }
        (Err(error), Err(cleanup)) => {
            Err(anyhow::anyhow!("{error:#}; Uma SET cleanup: {cleanup:#}"))
        }
        (result, Ok(_)) => result,
    }
}

fn join_predecessors(predecessors: Vec<JoinHandle<()>>) -> anyhow::Result<()> {
    let mut failed = false;
    for predecessor in predecessors {
        failed |= predecessor.join().is_err();
    }
    ensure!(!failed, "Previous Uma owner panicked during cleanup");
    Ok(())
}

fn bindings(
    route: &ReceiverRoute,
    canceled: &AtomicBool,
    valid: Arc<AtomicBool>,
) -> anyhow::Result<ReceiverQueryProjection> {
    validate(canceled)?;
    ensure!(
        valid.load(Ordering::Acquire),
        "Uma binding request canceled"
    );
    let mut client = ServiceClient::spawn()?;
    client.set_request_guard(valid.clone());
    let result = route.query(&mut client).and_then(|projection| {
        validate(canceled)?;
        ensure!(
            valid.load(Ordering::Acquire),
            "Uma binding request canceled after query"
        );
        Ok(projection)
    });
    let cleanup = super::receiver_reset::local::shutdown(&mut client);
    match (result, cleanup) {
        (Err(error), Err(cleanup)) => Err(anyhow::anyhow!(
            "{error:#}; binding worker cleanup: {cleanup:#}"
        )),
        (Err(error), _) | (_, Err(error)) => Err(error),
        (Ok(projection), Ok(_)) => Ok(projection),
    }
}

fn run(
    route: ReceiverRoute,
    canceled: Arc<AtomicBool>,
    commands: Receiver<Command>,
    updates: Sender<Update>,
    predecessors: Arc<std::sync::Mutex<Vec<JoinHandle<()>>>>,
    pairing_active: Arc<AtomicU64>,
) {
    let result = (|| -> anyhow::Result<()> {
        let predecessors = std::mem::take(
            &mut *predecessors
                .lock()
                .map_err(|_| anyhow::anyhow!("Previous Uma cleanup lock poisoned"))?,
        );
        join_predecessors(predecessors)?;
        validate(&canceled)?;
        let mut client = ServiceClient::spawn()?;
        client.set_cancellation_guard(canceled.clone());
        let result = (|| -> anyhow::Result<()> {
            let target = direct::resolve(&mut client, &route)?;
            let mut transport = ServiceTransport {
                client: &mut client,
                route: &route,
                target,
            };
            let mut initialization = Initialization::default();
            if let Err(error) = initialize(&mut transport, &canceled, &updates, &mut initialization)
            {
                diagnostic(&updates, error);
            }
            let mut next_observation = Instant::now() + Duration::from_secs(2);
            let mut pending_set = None;
            while !canceled.load(Ordering::Acquire) {
                ensure!(
                    transport.healthy(),
                    "Uma observation process terminated; reconnection is required"
                );
                match commands.recv_timeout(Duration::from_millis(100)) {
                    Ok(Command::Set(request)) => {
                        pending_set = Some(request);
                    }
                    Ok(Command::Bindings { session, valid }) => {
                        let result = bindings(&route, &canceled, valid)
                            .map_err(|error| format!("{error:#}"));
                        updates.send(Update::Bindings(session, result))?;
                    }
                    Ok(Command::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
                if pairing_active.load(Ordering::Acquire) == 0 {
                    if let Some(request) = pending_set.take() {
                        if !request.is_valid() {
                            continue;
                        }
                        validate(&canceled)?;
                        match submit(&route, &canceled, &request) {
                            Ok(report) => {
                                updates.send(Update::Submitted(request, report))?;
                            }
                            Err(error) => {
                                updates.send(Update::Unconfirmed(request, format!("{error:#}")))?;
                            }
                        }
                    }
                }
                if Instant::now() >= next_observation && pairing_active.load(Ordering::Acquire) == 0
                {
                    validate(&canceled)?;
                    // Native observation may be absent or coalesced. These are
                    // bounded physical queries, not fabricated hardware events.
                    if initialization.complete {
                        match transport.peers() {
                            Ok(peers) => {
                                updates.send(Update::Peers(peers))?;
                            }
                            Err(error) => diagnostic(&updates, error),
                        }
                    } else if let Err(error) =
                        initialize(&mut transport, &canceled, &updates, &mut initialization)
                    {
                        diagnostic(&updates, error);
                    }
                    next_observation = Instant::now() + Duration::from_secs(2);
                }
            }
            Ok(())
        })();
        let cleanup = if client.is_stopped() {
            client.finish_termination()
        } else {
            client.request(ServiceRequest::Shutdown).map(|_| ())
        };
        result.and(cleanup)
    })();
    if let Err(error) = result {
        diagnostic(&updates, error);
    }
    let _ = updates.send(Update::Stopped);
}

#[cfg(test)]
#[path = "hyperpolling_runtime/tests.rs"]
mod tests;

impl AppShell {
    fn retire_hyperpolling(&mut self, identity: &str) {
        if let Some(mut session) = self.hyperpolling_receivers.remove(identity) {
            session.canceled.store(true, Ordering::Release);
            let _ = session.commands.send(Command::Stop);
            if let Some(request) = &session.active {
                request.guard().store(false, Ordering::Release);
            }
            if let Some(worker) = session.worker.take() {
                self.hyperpolling_cleanup.push(Retired {
                    identity: identity.into(),
                    worker,
                });
            }
        }
        let mut pending = Vec::new();
        for retired in self.hyperpolling_cleanup.drain(..) {
            if retired.worker.is_finished() {
                let _ = retired.worker.join();
            } else {
                pending.push(retired);
            }
        }
        self.hyperpolling_cleanup = pending;
    }
    pub(super) fn sync_hyperpolling_receivers(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let candidates = self
            .devices
            .iter()
            .filter(|workspace| workspace.read(cx).device(cx).product_id == 179)
            .cloned()
            .collect::<Vec<_>>();
        let absent = self
            .hyperpolling_receivers
            .keys()
            .filter(|identity| {
                !candidates
                    .iter()
                    .any(|workspace| workspace.read(cx).identity(cx) == **identity)
            })
            .cloned()
            .collect::<Vec<_>>();
        for identity in absent {
            self.retire_hyperpolling(&identity);
            self.hyperpolling_pairing.remove(&identity);
        }
        for workspace in candidates {
            let identity = workspace.read(cx).identity(cx);
            let connection_revision = workspace.read(cx).connection_revision();
            let device = workspace.read(cx).device(cx);
            let route = super::device_discovery::receiver_route_for_owner(
                &self.device_observations,
                device,
                &device.device_container_id,
                device.real_product_id,
            )
            .ok();
            if !self.close_requested
                && route.as_ref().is_some_and(|route| {
                    self.hyperpolling_receivers
                        .get(&identity)
                        .is_some_and(|owner| {
                            !owner.stopped
                                && owner.connection_revision == connection_revision
                                && owner.route == *route
                                && owner
                                    .workspace
                                    .upgrade()
                                    .is_some_and(|bound| bound == workspace)
                        })
                })
            {
                continue;
            }
            self.retire_hyperpolling(&identity);
            self.hyperpolling_pairing.remove(&identity);
            workspace.update(cx, |workspace, cx| {
                workspace.begin_hyperpolling_connection(None, cx)
            });
            let Some(route) = route.filter(|_| !self.close_requested) else {
                continue;
            };
            let epoch = EPOCHS.fetch_add(1, Ordering::Relaxed);
            let canceled = Arc::new(AtomicBool::new(false));
            let signal = canceled.clone();
            let worker_route = route.clone();
            let pairing_active = Arc::new(AtomicU64::new(0));
            let worker_pairing = pairing_active.clone();
            let (sender, receiver) = mpsc::channel();
            let (updates, delivery) = mpsc::channel();
            let mut predecessors = Vec::new();
            let mut unrelated = Vec::new();
            for retired in self.hyperpolling_cleanup.drain(..) {
                if retired.identity == identity {
                    predecessors.push(retired.worker);
                } else {
                    unrelated.push(retired);
                }
            }
            self.hyperpolling_cleanup = unrelated;
            let predecessors = Arc::new(std::sync::Mutex::new(predecessors));
            let worker_predecessors = predecessors.clone();
            let worker = thread::Builder::new()
                .name("hyperpolling-receiver".into())
                .spawn(move || {
                    run(
                        worker_route,
                        signal,
                        receiver,
                        updates,
                        worker_predecessors,
                        worker_pairing,
                    )
                });
            let worker = match worker {
                Ok(worker) => worker,
                Err(error) => {
                    if let Ok(mut predecessors) = predecessors.lock() {
                        for worker in predecessors.drain(..) {
                            self.hyperpolling_cleanup.push(Retired {
                                identity: identity.clone(),
                                worker,
                            });
                        }
                    }
                    self.status = format!("无法启动接收器初始化：{error}");
                    continue;
                }
            };
            workspace.update(cx, |workspace, cx| {
                workspace.begin_hyperpolling_connection(Some(epoch), cx)
            });
            let weak_workspace = workspace.downgrade();
            let owner_key = identity.clone();
            let owner_route = route.clone();
            let task = cx.spawn_in(window, async move |shell, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(25))
                        .await;
                    let mut incoming = Vec::new();
                    while let Ok(update) = delivery.try_recv() {
                        incoming.push(update);
                    }
                    let current = shell
                        .update_in(cx, |shell, window, cx| {
                            let Some(workspace) = weak_workspace.upgrade() else {
                                if shell
                                    .hyperpolling_receivers
                                    .get(&owner_key)
                                    .is_some_and(|session| session.epoch == epoch)
                                {
                                    shell.retire_hyperpolling(&owner_key);
                                }
                                return false;
                            };
                            if !shell
                                .hyperpolling_receivers
                                .get(&owner_key)
                                .is_some_and(|session| session.epoch == epoch)
                            {
                                return false;
                            }
                            let matches = shell
                                .hyperpolling_receivers
                                .get(&owner_key)
                                .is_some_and(|session| session.epoch == epoch)
                                && shell.devices.contains(&workspace)
                                && workspace.read(cx).identity(cx) == owner_key
                                && workspace.read(cx).connection_revision() == connection_revision
                                && shell.device_observations.iter().any(|observed| {
                                    observed.matches(workspace.read(cx).device(cx))
                                        && owner_route.matches(observed)
                                });
                            if !matches {
                                shell.retire_hyperpolling(&owner_key);
                                workspace.update(cx, |workspace, cx| {
                                    workspace.begin_hyperpolling_connection(None, cx)
                                });
                                return false;
                            }
                            for update in incoming {
                                match update {
                                    Update::Serial(serial) => {
                                        if let Some(session) =
                                            shell.hyperpolling_receivers.get_mut(&owner_key)
                                        {
                                            session.serial = Some(serial.clone());
                                        }
                                        workspace.update(cx, |workspace, cx| {
                                            workspace.observe_hyperpolling_identity(&serial, cx)
                                        });
                                    }
                                    Update::Edition(metadata) => {
                                        if let Some(session) =
                                            shell.hyperpolling_receivers.get_mut(&owner_key)
                                        {
                                            session.metadata = Some(metadata);
                                        }
                                    }
                                    Update::Peers(peers) => {
                                        let container = workspace
                                            .read(cx)
                                            .device(cx)
                                            .device_container_id
                                            .clone();
                                        shell.publish_receiver_query(
                                            &container,
                                            179,
                                            Some(&peers),
                                            window,
                                            cx,
                                        );
                                    }
                                    Update::Bindings(session, result) => shell
                                        .deliver_hyperpolling_bindings(
                                            &owner_key, &workspace, session, result, window, cx,
                                        ),
                                    Update::InitialStatus(status) => {
                                        workspace.update(cx, |workspace, cx| {
                                            workspace.initialize_hyperpolling_indicator(
                                                epoch, status, cx,
                                            )
                                        })
                                    }
                                    Update::Submitted(request, report) => {
                                        if let Some(session) =
                                            shell.hyperpolling_receivers.get_mut(&owner_key)
                                        {
                                            if session.active.as_ref().is_some_and(|active| {
                                                active.sequence() == request.sequence()
                                            }) {
                                                session.active = None;
                                            }
                                        }
                                        if let Some(error) = &report.error {
                                            shell.status =
                                                format!("接收器指示灯写入未确认：{error}");
                                        }
                                        workspace.update(cx, |workspace, cx| {
                                            workspace.finish_hyperpolling_indicator(
                                                &request,
                                                Some(report),
                                                cx,
                                            )
                                        });
                                    }
                                    Update::Unconfirmed(request, error) => {
                                        shell.status = format!("接收器指示灯交付未确认：{error}");
                                        workspace.update(cx, |workspace, cx| {
                                            workspace
                                                .finish_hyperpolling_indicator(&request, None, cx)
                                        });
                                    }
                                    Update::Diagnostic(error) => {
                                        shell.status = format!("接收器初始化或通信未确认：{error}");
                                    }
                                    Update::Stopped => {
                                        shell.hyperpolling_pairing.remove(&owner_key);
                                        if let Some(session) =
                                            shell.hyperpolling_receivers.get_mut(&owner_key)
                                        {
                                            session.stopped = true;
                                        }
                                        workspace.update(cx, |workspace, cx| {
                                            workspace.begin_hyperpolling_connection(None, cx)
                                        });
                                    }
                                }
                            }
                            if shell
                                .hyperpolling_receivers
                                .get(&owner_key)
                                .is_some_and(|session| session.stopped)
                            {
                                return false;
                            }
                            cx.notify();
                            true
                        })
                        .unwrap_or(false);
                    if !current {
                        // A dead owned process is recovered on the same verified
                        // physical route; replacement still waits for its join.
                        cx.background_executor().timer(Duration::from_secs(2)).await;
                        let _ =
                            shell.update_in(cx, |shell, window, cx| {
                                if shell.hyperpolling_receivers.get(&owner_key).is_some_and(
                                    |session| session.epoch == epoch && session.stopped,
                                ) {
                                    shell.sync_hyperpolling_receivers(window, cx);
                                }
                            });
                        break;
                    }
                }
            });
            self.hyperpolling_receivers.insert(
                identity,
                Session {
                    workspace: workspace.downgrade(),
                    connection_revision,
                    epoch,
                    route,
                    serial: None,
                    metadata: None,
                    active: None,
                    stopped: false,
                    pairing_active,
                    canceled,
                    commands: sender,
                    worker: Some(worker),
                    _delivery: task,
                },
            );
        }
    }
    pub(super) fn install_hyperpolling_cleanup(&mut self) {
        self.register_exit_cleanup(|shell, cx| {
            let sessions = std::mem::take(&mut shell.hyperpolling_receivers);
            shell.hyperpolling_pairing.clear();
            let mut retired = std::mem::take(&mut shell.hyperpolling_cleanup);
            for session in sessions.values() {
                session.canceled.store(true, Ordering::Release);
                if let Some(request) = &session.active {
                    request.guard().store(false, Ordering::Release);
                }
                let _ = session.commands.send(Command::Stop);
            }
            let cleanup = cx.background_executor().spawn(async move {
                for (identity, mut session) in sessions {
                    if let Some(worker) = session.worker.take() {
                        retired.push(Retired { identity, worker });
                    }
                }
                for retired in retired {
                    let _ = retired.worker.join();
                }
            });
            async move { cleanup.await }
        });
    }
    pub(super) fn persist_hyperpolling_indicator(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: Request,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let identity = workspace.read(cx).identity(cx);
        self.refresh_macro_recorder_storage();
        // Current41374.w grants no deviceAccess while the macro recorder is
        // exclusive.25947.Eo nests its local save inside that same condition.
        if self.receiver_macro_exclusive(cx) {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_hyperpolling_indicator(&request, None, cx)
            });
            return;
        }
        let ready = !self.close_requested
            && self.storage_error.is_none()
            && workspace
                .read(cx)
                .hyperpolling_request_matches(&request, cx)
            && self
                .hyperpolling_receivers
                .get(&identity)
                .is_some_and(|session| {
                    session.epoch == request.epoch() && session.serial.is_some()
                });
        if !ready {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_hyperpolling_indicator(&request, None, cx)
            });
            return;
        }
        let edit = match IndicatorEdit::capture(workspace.read(cx).device(cx), request.status()) {
            Ok(edit) => edit,
            Err(error) => {
                self.status = format!("接收器本地配置未保存：{error:#}");
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_hyperpolling_indicator(&request, None, cx)
                });
                return;
            }
        };
        let connection_revision = workspace.read(cx).connection_revision();
        let file = self.workspace_file(
            self.devices
                .iter()
                .map(|owner| owner.read(cx).saved_snapshot(cx))
                .collect(),
            cx,
        );
        let save = self.prepare_save(
            window.window_handle(),
            file,
            Some(ProductSave::Hyperpolling(LocalSave {
                identity,
                workspace,
                connection_revision,
                request,
                edit,
            })),
            cx,
        );
        self.pending_saves.push_back(save);
        self.start_pending_save(cx);
    }
    pub(super) fn rebase_hyperpolling_save(
        &self,
        save: &mut PreparedSave,
        cx: &App,
    ) -> anyhow::Result<()> {
        let Some(ProductSave::Hyperpolling(local)) = &save.product else {
            return Ok(());
        };
        ensure!(
            local.workspace.read(cx).connection_revision() == local.connection_revision
                && local
                    .workspace
                    .read(cx)
                    .hyperpolling_request_matches(&local.request, cx),
            "接收器指示灯请求已过期"
        );
        let target = save
            .file
            .devices
            .iter_mut()
            .find(|device| device.workspace_identity() == local.identity)
            .context("接收器本地保存没有所属设备")?;
        local.edit.rebase(target)
    }
    pub(super) fn finish_hyperpolling_save(
        &mut self,
        save: LocalSave,
        succeeded: bool,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if !succeeded || !save.request.is_valid() || self.close_requested {
            save.workspace.update(cx, |workspace, cx| {
                workspace.finish_hyperpolling_indicator(&save.request, None, cx)
            });
            return;
        }
        let shell = cx.entity().downgrade();
        cx.defer(move |cx| {
            let owner = save.workspace.clone();
            let request = save.request.clone();
            let launched = window.update(cx, |_, window, cx| {
                shell.update(cx, |shell, cx| {
                    shell.refresh_macro_recorder_storage();
                    if shell.receiver_macro_exclusive(cx) {
                        return false;
                    }
                    let Some(session) = shell.hyperpolling_receivers.get(&save.identity) else {
                        return false;
                    };
                    if session.epoch != save.request.epoch()
                        || session.connection_revision != save.connection_revision
                        || !save
                            .workspace
                            .read(cx)
                            .hyperpolling_request_matches(&save.request, cx)
                    {
                        return false;
                    }
                    if !save.workspace.update(cx, |workspace, cx| {
                        workspace.publish_hyperpolling_indicator(&save.request, window, cx)
                    }) {
                        return false;
                    }
                    let sender = session.commands.clone();
                    if let Some(session) = shell.hyperpolling_receivers.get_mut(&save.identity) {
                        session.active = Some(save.request.clone());
                    }
                    sender.send(Command::Set(save.request)).is_ok()
                })
            });
            if !matches!(launched, Ok(Ok(true))) {
                owner.update(cx, |workspace, cx| {
                    workspace.finish_hyperpolling_indicator(&request, None, cx)
                });
            }
        });
    }
}
