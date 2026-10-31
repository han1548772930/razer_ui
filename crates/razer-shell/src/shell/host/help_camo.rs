//! Camera identity sessions outlive Help; local app commands follow its mount.
use super::*;
use anyhow::{Context as _, ensure};
use razer_device::{camera_serial::CameraSerialProgress, camera_submission::CameraScope};
use razer_pages::features::{
    CameraIdentityRequested, HelpCamoCommand, HelpCamoObservation, HelpCamoRequested,
};
use razer_platform::camera_apps::{self, CamoInstallation};
use razer_service::camera::{CameraSerialSession, lifecycle::Owner};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

struct SerialWork {
    id: u64,
    cancelled: Arc<AtomicBool>,
}

#[derive(Clone, Copy)]
enum AppCommand {
    Observe,
    LaunchInstalled,
}

enum AppResult {
    Installation(Result<CamoInstallation, String>),
    Launch(Result<(), String>),
}

enum Reply {
    Serial {
        connection: u64,
        id: Option<u64>,
        result: Result<CameraSerialProgress, String>,
    },
    App {
        id: u64,
        result: AppResult,
    },
}

struct Mount {
    workspace: WeakEntity<ProductWorkspace>,
    connection_revision: u64,
}
impl Mount {
    fn current(&self, shell: &AppShell, cx: &App) -> Option<Entity<ProductWorkspace>> {
        let workspace = self.workspace.upgrade()?;
        (!shell.close_requested
            && shell.devices.contains(&workspace)
            && workspace.read(cx).connection_revision() == self.connection_revision)
            .then_some(workspace)
    }
}

struct Pending {
    request: CameraIdentityRequested,
    mount: Mount,
    help: BTreeMap<u64, HelpCamoRequested>,
}
impl Pending {
    fn current(&self, shell: &AppShell, cx: &App) -> bool {
        !self.request.is_cancelled()
            && self.mount.current(shell, cx).is_some_and(|workspace| {
                workspace
                    .read(cx)
                    .accepts_camera_identity(&self.request, cx)
            })
    }
}

struct Connection {
    id: u64,
    pending: Pending,
    owner: Arc<Owner>,
    commands: Option<mpsc::Sender<SerialWork>>,
    initial_delivered: bool,
    joined: bool,
    join_error: Option<String>,
}
impl Connection {
    fn stop(&mut self) {
        self.pending
            .request
            .cancellation()
            .store(true, Ordering::Release);
        self.owner.stop();
        self.commands.take();
    }
}

struct AppJob {
    request: HelpCamoRequested,
    mount: Mount,
    owner: Arc<Owner>,
    stop: Arc<AtomicBool>,
    delivered: bool,
    joined: bool,
    join_error: Option<String>,
}
impl AppJob {
    fn current(&self, shell: &AppShell, cx: &App) -> bool {
        !self.request.is_cancelled()
            && !self.stop.load(Ordering::Acquire)
            && self
                .mount
                .current(shell, cx)
                .is_some_and(|workspace| workspace.read(cx).accepts_help_camo(&self.request, cx))
    }
}

pub(super) struct Runtime {
    next_id: u64,
    connections: BTreeMap<u64, Connection>,
    pending: BTreeMap<u64, Pending>,
    apps: BTreeMap<u64, AppJob>,
    replies: mpsc::Receiver<Reply>,
    finished: mpsc::Sender<Reply>,
    stopping: bool,
    delivery: Option<Task<()>>,
    delivery_owner: Option<Arc<()>>,
    cleanup_failures: Vec<String>,
}
impl Default for Runtime {
    fn default() -> Self {
        let (finished, replies) = mpsc::channel();
        Self {
            next_id: 0,
            connections: BTreeMap::new(),
            pending: BTreeMap::new(),
            apps: BTreeMap::new(),
            replies,
            finished,
            stopping: false,
            delivery: None,
            delivery_owner: None,
            cleanup_failures: Vec::new(),
        }
    }
}
impl Runtime {
    fn next_id(&mut self) -> anyhow::Result<u64> {
        self.next_id = self
            .next_id
            .checked_add(1)
            .context("Camera request sequence exhausted")?;
        Ok(self.next_id)
    }

    pub(super) fn cancel_all(&mut self) {
        self.stopping = true;
        for connection in self.connections.values_mut() {
            connection.stop();
        }
        for pending in self.pending.values() {
            pending
                .request
                .cancellation()
                .store(true, Ordering::Release);
        }
        self.pending.clear();
        for job in self.apps.values() {
            job.stop.store(true, Ordering::Release);
            job.owner.stop();
        }
    }

    pub(super) fn resume_after_cancelled_exit(&mut self) {
        self.stopping = false;
    }

    fn start(&mut self, pending: Pending) -> anyhow::Result<()> {
        ensure!(!self.stopping, "Camera identity owner is stopping");
        let id = self.next_id()?;
        let owner = Arc::new(Owner::new());
        let (commands, incoming) = mpsc::channel();
        let scope = pending.request.scope().clone();
        let cancelled = pending.request.cancellation();
        let finished = self.finished.clone();
        owner.spawn("camera-identity", cancelled.clone(), move || {
            run_serial_worker(
                id,
                scope,
                cancelled,
                incoming,
                finished,
                |scope, validate| CameraSerialSession::initialize(scope, &validate),
            );
        })?;
        for (&id, request) in &pending.help {
            let _ = commands.send(SerialWork {
                id,
                cancelled: request.cancellation(),
            });
        }
        self.connections.insert(
            pending.request.owner().as_u64(),
            Connection {
                id,
                pending,
                owner,
                commands: Some(commands),
                initial_delivered: false,
                joined: false,
                join_error: None,
            },
        );
        Ok(())
    }

    fn poll(&mut self) -> Vec<Reply> {
        for connection in self.connections.values_mut() {
            connection.joined = reap_before_delivery(
                &connection.owner,
                &mut connection.join_error,
                &mut self.cleanup_failures,
            );
        }
        for job in self.apps.values_mut() {
            job.joined =
                reap_before_delivery(&job.owner, &mut job.join_error, &mut self.cleanup_failures);
        }
        // Completed threads have been joined before their final reply drain.
        let mut replies: Vec<_> = self.replies.try_iter().collect();
        for connection in self.connections.values().filter(|entry| entry.joined) {
            let error = connection
                .join_error
                .as_deref()
                .unwrap_or("Camera identity worker closed without a result");
            if !connection.initial_delivered
                && !replies.iter().any(|reply| matches!(reply, Reply::Serial { connection: id, id: None, .. } if *id == connection.id))
            {
                replies.push(Reply::Serial { connection: connection.id, id: None, result: Err(error.into()) });
            }
            for &id in connection.pending.help.keys() {
                if !replies.iter().any(|reply| matches!(reply, Reply::Serial { connection: owner, id: Some(job), .. } if *owner == connection.id && *job == id)) {
                    replies.push(Reply::Serial { connection: connection.id, id: Some(id), result: Err(error.into()) });
                }
            }
        }
        for (&id, job) in self
            .apps
            .iter()
            .filter(|(_, job)| !job.delivered && job.joined)
        {
            if !replies
                .iter()
                .any(|reply| matches!(reply, Reply::App { id: job, .. } if *job == id))
            {
                let error = job
                    .join_error
                    .clone()
                    .unwrap_or_else(|| "Camo application worker closed without a result".into());
                let result = match job.request.command() {
                    HelpCamoCommand::Observe => AppResult::Installation(Err(error)),
                    _ => AppResult::Launch(Err(error)),
                };
                replies.push(Reply::App { id, result });
            }
        }
        replies
    }

    fn retire_finished(&mut self) {
        // A different camera owner may reap the registry after this batch's
        // reply drain. Only retire joins observed before that drain.
        self.connections.retain(|_, connection| !connection.joined);
        self.apps.retain(|_, job| !job.joined);
    }

    fn idle(&self) -> bool {
        self.connections.is_empty() && self.pending.is_empty() && self.apps.is_empty()
    }

    fn take_cleanup(&mut self) -> Cleanup {
        let mut owners: Vec<_> = std::mem::take(&mut self.connections)
            .into_values()
            .map(|entry| entry.owner)
            .collect();
        owners.extend(
            std::mem::take(&mut self.apps)
                .into_values()
                .map(|entry| entry.owner),
        );
        Cleanup {
            owners,
            failures: std::mem::take(&mut self.cleanup_failures),
        }
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.cancel_all();
        let cleanup = self.take_cleanup();
        if cleanup.owners.is_empty() && cleanup.failures.is_empty() {
            return;
        }
        // The cleanup registry owns this join even when a release thread cannot
        // be started immediately. COM resources still drop on their worker.
        let release = Owner::new();
        if let Err(error) = release.cleanup("camera-identity-release", move || cleanup.drain()) {
            eprintln!("[camera-identity] retained release: {error:#}");
        }
    }
}

struct Cleanup {
    owners: Vec<Arc<Owner>>,
    failures: Vec<String>,
}

fn reap_before_delivery(
    owner: &Owner,
    failure: &mut Option<String>,
    history: &mut Vec<String>,
) -> bool {
    let mut capture = |result: anyhow::Result<()>| {
        if let Err(error) = result {
            let error = format!("{error:#}");
            history.push(error.clone());
            *failure = Some(error);
        }
    };
    capture(owner.reap_finished());
    let joined = owner.active_workers() == 0;
    if joined {
        // Global registry reaping may have completed between the first reap
        // and the count. Its failure belongs to this same completed join.
        capture(owner.reap_finished());
    }
    joined
}

impl Cleanup {
    fn drain(self) -> anyhow::Result<()> {
        let mut failures = self.failures;
        for owner in self.owners {
            if let Err(error) = owner.shutdown() {
                failures.push(format!("{error:#}"));
            }
        }
        ensure!(
            failures.is_empty(),
            "Camera identity cleanup failed: {}",
            failures.join("; ")
        );
        Ok(())
    }
}

trait SerialReader {
    fn read(
        &mut self,
        validate: &impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<CameraSerialProgress>;
}
impl SerialReader for CameraSerialSession {
    fn read(
        &mut self,
        validate: &impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<CameraSerialProgress> {
        CameraSerialSession::read(self, validate)
    }
}

fn run_serial_worker<S: SerialReader>(
    connection: u64,
    scope: CameraScope,
    cancelled: Arc<AtomicBool>,
    incoming: mpsc::Receiver<SerialWork>,
    finished: mpsc::Sender<Reply>,
    open: impl FnOnce(CameraScope, &dyn Fn() -> anyhow::Result<()>) -> anyhow::Result<S>,
) {
    let validate = || {
        ensure!(
            !cancelled.load(Ordering::Acquire),
            "Camera connection canceled"
        );
        Ok(())
    };
    let mut session = validate()
        .and_then(|_| open(scope, &validate))
        .map_err(|error| format!("{error:#}"));
    let initial = session
        .as_mut()
        .map_err(|error| error.clone())
        .and_then(|session| {
            session
                .read(&validate)
                .map_err(|error| format!("{error:#}"))
        });
    let _ = finished.send(Reply::Serial {
        connection,
        id: None,
        result: initial,
    });
    while !cancelled.load(Ordering::Acquire) {
        let work = match incoming.recv_timeout(Duration::from_millis(50)) {
            Ok(work) => work,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        let check = || {
            validate()?;
            ensure!(
                !work.cancelled.load(Ordering::Acquire),
                "Help serial request canceled"
            );
            Ok(())
        };
        let result = check().map_err(|error| format!("{error:#}")).and_then(|_| {
            session
                .as_mut()
                .map_err(|error| error.clone())
                .and_then(|session| session.read(&check).map_err(|error| format!("{error:#}")))
        });
        let _ = finished.send(Reply::Serial {
            connection,
            id: Some(work.id),
            result,
        });
    }
    // The session is destroyed before the lifecycle registry can join us.
    drop(session);
}

fn run_app_work(
    command: AppCommand,
    cancelled: &AtomicBool,
    stop: &AtomicBool,
    observe: impl FnOnce() -> CamoInstallation,
    launch: impl FnOnce(&dyn Fn() -> anyhow::Result<()>) -> anyhow::Result<()>,
) -> AppResult {
    let validate = || {
        ensure!(
            !cancelled.load(Ordering::Acquire) && !stop.load(Ordering::Acquire),
            "Help Camo request canceled"
        );
        Ok(())
    };
    match command {
        AppCommand::Observe => AppResult::Installation(
            (|| {
                validate()?;
                let observation = observe();
                validate()?;
                Ok(observation)
            })()
            .map_err(|error: anyhow::Error| format!("{error:#}")),
        ),
        AppCommand::LaunchInstalled => AppResult::Launch(
            validate()
                .and_then(|_| launch(&validate))
                .and_then(|_| validate())
                .map_err(|error| format!("{error:#}")),
        ),
    }
}

impl AppShell {
    pub(super) fn install_help_camo_cleanup(&mut self, _cx: &mut Context<Self>) {
        self.register_exit_cleanup(|shell, cx| {
            let mut runtime = std::mem::take(&mut shell.help_camo);
            runtime.cancel_all();
            runtime.delivery.take();
            runtime.delivery_owner.take();
            let cleanup = runtime.take_cleanup();
            drop(runtime);
            let cleanup = cx.background_executor().spawn(async move {
                if let Err(error) = cleanup.drain() {
                    eprintln!("[camera-identity] exit cleanup: {error:#}");
                }
            });
            async move { cleanup.await }
        });
    }

    pub(super) fn request_camera_identity(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: CameraIdentityRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested
            || self.help_camo.stopping
            || !workspace.read(cx).accepts_camera_identity(&request, cx)
        {
            return;
        }
        let key = request.owner().as_u64();
        if self.help_camo.connections.get(&key).is_some_and(|entry| {
            !entry.owner.is_stopped() && entry.pending.request.same_connection(&request)
        }) || self
            .help_camo
            .pending
            .get(&key)
            .is_some_and(|pending| pending.request.same_connection(&request))
        {
            return;
        }
        let pending = Pending {
            request: request.clone(),
            mount: Mount {
                workspace: workspace.downgrade(),
                connection_revision: workspace.read(cx).connection_revision(),
            },
            help: BTreeMap::new(),
        };
        if let Some(connection) = self.help_camo.connections.get_mut(&key) {
            connection.stop();
            if let Some(previous) = self.help_camo.pending.insert(key, pending) {
                previous
                    .request
                    .cancellation()
                    .store(true, Ordering::Release);
            }
        } else if let Err(error) = self.help_camo.start(pending) {
            workspace.update(cx, |workspace, cx| {
                workspace.observe_camera_identity(&request, Err(format!("{error:#}")), cx);
            });
        }
        self.install_help_camo_pump(window, cx);
    }

    pub(super) fn request_help_camo(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: HelpCamoRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested || !workspace.read(cx).accepts_help_camo(&request, cx) {
            return;
        }
        let result = (|| -> anyhow::Result<()> {
            ensure!(!self.help_camo.stopping, "Help Camo owner is stopping");
            let id = self.help_camo.next_id()?;
            if *request.command() == HelpCamoCommand::GetSerialNumber {
                let identity = workspace
                    .read(cx)
                    .camera_identity_request(cx)
                    .context("Camera has no live identity session")?;
                self.request_camera_identity(workspace.clone(), identity.clone(), window, cx);
                let key = identity.owner().as_u64();
                if let Some(pending) = self.help_camo.pending.get_mut(&key) {
                    ensure!(
                        pending.request.same_connection(&identity),
                        "Camera connection changed before Help retry"
                    );
                    pending.help.insert(id, request.clone());
                } else {
                    let connection = self
                        .help_camo
                        .connections
                        .get_mut(&key)
                        .context("Camera identity worker is unavailable")?;
                    ensure!(
                        connection.pending.request.same_connection(&identity)
                            && !connection.owner.is_stopped(),
                        "Camera identity worker is stopping"
                    );
                    connection
                        .commands
                        .as_ref()
                        .context("Camera identity command channel is closed")?
                        .send(SerialWork {
                            id,
                            cancelled: request.cancellation(),
                        })
                        .context("Camera serial worker rejected Help retry")?;
                    connection.pending.help.insert(id, request.clone());
                }
            } else {
                let owner = Arc::new(Owner::new());
                let stop = Arc::new(AtomicBool::new(false));
                let worker_stop = stop.clone();
                let cancelled = request.cancellation();
                let command = match request.command() {
                    HelpCamoCommand::Observe => AppCommand::Observe,
                    HelpCamoCommand::LaunchInstalled => AppCommand::LaunchInstalled,
                    HelpCamoCommand::GetSerialNumber => {
                        unreachable!("serial work is routed to its connection")
                    }
                };
                let finished = self.help_camo.finished.clone();
                owner.spawn("help-camo-local-app", stop.clone(), move || {
                    let result = run_app_work(
                        command,
                        &cancelled,
                        &worker_stop,
                        camera_apps::observe_camo,
                        |validate| camera_apps::launch_camo_from_help(&validate),
                    );
                    let _ = finished.send(Reply::App { id, result });
                })?;
                self.help_camo.apps.insert(
                    id,
                    AppJob {
                        request: request.clone(),
                        mount: Mount {
                            workspace: workspace.downgrade(),
                            connection_revision: workspace.read(cx).connection_revision(),
                        },
                        owner,
                        stop,
                        delivered: false,
                        joined: false,
                        join_error: None,
                    },
                );
            }
            Ok(())
        })();
        if let Err(error) = result {
            workspace.update(cx, |workspace, cx| {
                let device = workspace.device(cx).clone();
                workspace.observe_help_camo(
                    &device,
                    HelpCamoObservation::failed(request, format!("{error:#}")),
                    cx,
                );
            });
        }
        self.install_help_camo_pump(window, cx);
    }

    fn poll_help_camo(&mut self, cx: &mut Context<Self>) {
        let canceled: Vec<_> = self
            .help_camo
            .connections
            .iter()
            .filter_map(|(&key, entry)| (!entry.pending.current(self, cx)).then_some(key))
            .collect();
        for key in canceled {
            if let Some(connection) = self.help_camo.connections.get_mut(&key) {
                connection.stop();
            }
        }
        for job in self.help_camo.apps.values() {
            if !job.current(self, cx) {
                job.stop.store(true, Ordering::Release);
                job.owner.stop();
            }
        }
        for reply in self.help_camo.poll() {
            match reply {
                Reply::Serial {
                    connection,
                    id,
                    result,
                } => {
                    let Some((&key, entry)) = self
                        .help_camo
                        .connections
                        .iter()
                        .find(|(_, entry)| entry.id == connection)
                    else {
                        continue;
                    };
                    let current = entry.pending.current(self, cx);
                    let request = entry.pending.request.clone();
                    let workspace = entry.pending.mount.workspace.upgrade();
                    let help = {
                        let entry = self.help_camo.connections.get_mut(&key).unwrap();
                        match id {
                            Some(id) => entry.pending.help.remove(&id),
                            None => {
                                entry.initial_delivered = true;
                                None
                            }
                        }
                    };
                    if !current {
                        continue;
                    }
                    let Some(workspace) = workspace else { continue };
                    workspace.update(cx, |workspace, cx| {
                        if let Some(help) = help {
                            let device = workspace.device(cx).clone();
                            workspace.observe_help_camo(
                                &device,
                                HelpCamoObservation::serial(help, result),
                                cx,
                            );
                        } else if id.is_none() {
                            workspace.observe_camera_identity(&request, result, cx);
                        }
                    });
                }
                Reply::App { id, result } => {
                    let Some(job) = self.help_camo.apps.get(&id) else {
                        continue;
                    };
                    let current = job.current(self, cx);
                    let workspace = job.mount.workspace.upgrade();
                    let request = job.request.clone();
                    self.help_camo.apps.get_mut(&id).unwrap().delivered = true;
                    if !current {
                        continue;
                    }
                    let Some(workspace) = workspace else { continue };
                    let observation = match result {
                        AppResult::Installation(Ok(observed)) => {
                            HelpCamoObservation::installation(request, observed)
                        }
                        AppResult::Installation(Err(error)) => {
                            HelpCamoObservation::failed(request, error)
                        }
                        AppResult::Launch(result) => HelpCamoObservation::launched(request, result),
                    };
                    workspace.update(cx, |workspace, cx| {
                        let device = workspace.device(cx).clone();
                        workspace.observe_help_camo(&device, observation, cx);
                    });
                }
            }
        }
        self.help_camo.retire_finished();
        let ready: Vec<_> = self
            .help_camo
            .pending
            .keys()
            .filter(|key| !self.help_camo.connections.contains_key(key))
            .copied()
            .collect();
        for key in ready {
            let pending = self.help_camo.pending.remove(&key).unwrap();
            if self.help_camo.stopping || !pending.current(self, cx) {
                continue;
            }
            let request = pending.request.clone();
            let workspace = pending.mount.workspace.upgrade();
            let help: Vec<_> = pending.help.values().cloned().collect();
            if let Err(error) = self.help_camo.start(pending) {
                if let Some(workspace) = workspace {
                    workspace.update(cx, |workspace, cx| {
                        workspace.observe_camera_identity(&request, Err(format!("{error:#}")), cx);
                        let device = workspace.device(cx).clone();
                        for request in help {
                            workspace.observe_help_camo(
                                &device,
                                HelpCamoObservation::failed(request, format!("{error:#}")),
                                cx,
                            );
                        }
                    });
                }
            }
        }
    }

    fn install_help_camo_pump(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.help_camo.delivery.is_some() || self.help_camo.idle() {
            return;
        }
        let owner = Arc::new(());
        self.help_camo.delivery_owner = Some(owner.clone());
        self.help_camo.delivery = Some(cx.spawn_in(window, async move |shell, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                let keep = shell.update_in(cx, |shell, _, cx| {
                    if !shell
                        .help_camo
                        .delivery_owner
                        .as_ref()
                        .is_some_and(|current| Arc::ptr_eq(current, &owner))
                    {
                        return false;
                    }
                    shell.poll_help_camo(cx);
                    if shell.help_camo.idle() {
                        shell.help_camo.delivery = None;
                        shell.help_camo.delivery_owner = None;
                        if shell.close_requested && shell.save_task.is_none() {
                            shell.continue_save_queue(true, cx);
                        }
                        return false;
                    }
                    true
                });
                if keep.is_err() {
                    let _ = shell.update(cx, |shell, _| {
                        if shell
                            .help_camo
                            .delivery_owner
                            .as_ref()
                            .is_some_and(|current| Arc::ptr_eq(current, &owner))
                        {
                            shell.help_camo.cancel_all();
                            shell.help_camo.delivery = None;
                            shell.help_camo.delivery_owner = None;
                        }
                    });
                }
                if !matches!(keep, Ok(true)) {
                    break;
                }
            }
        }));
    }
}

#[cfg(test)]
#[path = "help_camo_tests.rs"]
mod tests;
