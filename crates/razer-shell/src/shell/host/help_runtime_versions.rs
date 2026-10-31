//! Mounted Help reads the shell's real host store and the app's local SDK records.
use super::*;
use anyhow::{Context as _, ensure};
use razer_pages::features::{
    HelpRuntimeVersionsObservation, HelpRuntimeVersionsRequest, HelpRuntimeVersionsStatus,
};
use razer_storage::host::{HostStorage, HostStorageCall, HostStorageView, HostStoreKind};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

static ACTIVE_WORKERS: AtomicUsize = AtomicUsize::new(0);
pub(super) fn active_workers() -> usize {
    ACTIVE_WORKERS.load(Ordering::Acquire)
}
struct WorkerActivity;
impl WorkerActivity {
    fn new() -> Self {
        ACTIVE_WORKERS.fetch_add(1, Ordering::AcqRel);
        Self
    }
}
impl Drop for WorkerActivity {
    fn drop(&mut self) {
        ACTIVE_WORKERS.fetch_sub(1, Ordering::AcqRel);
    }
}
struct Worker {
    handle: JoinHandle<()>,
    activity: WorkerActivity,
}
impl Worker {
    fn is_finished(&self) -> bool {
        self.handle.is_finished()
    }
    fn join(self) -> thread::Result<()> {
        let Self { handle, activity } = self;
        let result = handle.join();
        drop(activity);
        result
    }
}

struct Work {
    id: u64,
    product_id: u32,
    container_id: String,
    middleware: Result<Option<String>, String>,
    cancelled: Arc<AtomicBool>,
}
struct Session {
    request: HelpRuntimeVersionsRequest,
    workspace: WeakEntity<ProductWorkspace>,
    device: Device,
    connection_revision: u64,
}
impl Session {
    fn cancel(&self) {
        self.request.cancellation().store(true, Ordering::Release);
    }
    fn current(&self, shell: &AppShell, cx: &App) -> bool {
        !shell.close_requested
            && !self.request.is_cancelled()
            && self.workspace.upgrade().is_some_and(|workspace| {
                shell.devices.contains(&workspace)
                    && workspace.read(cx).connection_revision() == self.connection_revision
                    && workspace
                        .read(cx)
                        .accepts_help_runtime_versions(&self.request, cx)
                    && same_device(workspace.read(cx).device(cx), &self.device)
            })
    }
}
fn same_device(current: &Device, owner: &Device) -> bool {
    current.product_id == owner.product_id
        && current.real_product_id == owner.real_product_id
        && current.serial_number == owner.serial_number
        && current.device_container_id == owner.device_container_id
        && current.edition_id == owner.edition_id
        && current.layout_id == owner.layout_id
}
struct Reply {
    id: u64,
    result: Result<Option<HelpRuntimeVersionsStatus>, String>,
}

#[derive(Default)]
pub(super) struct Runtime {
    next_id: u64,
    sessions: BTreeMap<u64, Session>,
    commands: Option<mpsc::Sender<Work>>,
    replies: Option<mpsc::Receiver<Reply>>,
    worker: Option<Worker>,
    pending: VecDeque<Work>,
    delivery: Option<Task<()>>,
    delivery_owner: Option<Arc<()>>,
    stopping: bool,
}
impl Runtime {
    pub(super) fn cancel_all(&mut self) {
        self.stopping = true;
        for session in self.sessions.values() {
            session.cancel();
        }
        for work in &self.pending {
            work.cancelled.store(true, Ordering::Release);
        }
        self.pending.clear();
        self.commands.take();
    }
    pub(super) fn resume_after_cancelled_exit(&mut self) {
        self.stopping = false;
    }
    fn start(&mut self) -> anyhow::Result<()> {
        ensure!(!self.stopping, "Help version owner is stopping");
        if self.worker.is_some() {
            return Ok(());
        }
        let (commands, incoming) = mpsc::channel::<Work>();
        let (finished, replies) = mpsc::channel();
        let activity = WorkerActivity::new();
        let handle = thread::Builder::new()
            .name("help-runtime-versions".into())
            .spawn(move || {
                while let Ok(work) = incoming.recv() {
                    let id = work.id;
                    let result = run_work_with(work, read_apps);
                    let _ = finished.send(Reply {
                        id,
                        result: Ok(result),
                    });
                }
            })?;
        self.commands = Some(commands);
        self.replies = Some(replies);
        self.worker = Some(Worker { handle, activity });
        Ok(())
    }
    fn admit(&mut self, work: Work) -> anyhow::Result<()> {
        ensure!(!self.stopping, "Help version owner is stopping");
        if let Some(commands) = &self.commands {
            commands
                .send(work)
                .context("Help version worker rejected the request")?;
        } else {
            self.pending.push_back(work);
        }
        Ok(())
    }
    fn drain(&mut self) -> Vec<Reply> {
        self.replies
            .as_ref()
            .map(|replies| replies.try_iter().collect())
            .unwrap_or_default()
    }
    fn poll(&mut self) -> Vec<Reply> {
        let mut replies = self.drain();
        if !self.worker.as_ref().is_some_and(Worker::is_finished) {
            return replies;
        }
        let panicked = self.worker.take().unwrap().join().is_err();
        // A reply may arrive after the initial nonblocking drain and before
        // the thread finishes. Join before dropping its final reply channel.
        replies.extend(self.drain());
        self.commands.take();
        self.replies.take();
        for id in self.sessions.keys() {
            if !replies.iter().any(|reply| reply.id == *id)
                && !self.pending.iter().any(|work| work.id == *id)
            {
                replies.push(Reply {
                    id: *id,
                    result: Err(if panicked {
                        "Help version worker panicked before completing its read"
                    } else {
                        "Help version worker closed without a read result"
                    }
                    .into()),
                });
            }
        }
        if !self.stopping && !self.pending.is_empty() {
            match self.start() {
                Ok(()) => {
                    while let Some(work) = self.pending.pop_front() {
                        let id = work.id;
                        if let Err(error) = self.admit(work) {
                            replies.push(Reply {
                                id,
                                result: Err(format!("{error:#}")),
                            });
                        }
                    }
                }
                Err(error) => {
                    replies.extend(self.pending.drain(..).map(|work| Reply {
                        id: work.id,
                        result: Err(format!("{error:#}")),
                    }));
                }
            }
        }
        replies
    }
    fn retire_if_idle(&mut self) {
        if self.sessions.is_empty() && self.pending.is_empty() {
            self.commands.take();
        }
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.cancel_all();
        if let Some(worker) = self.worker.take() {
            if worker.is_finished() {
                if worker.join().is_err() {
                    eprintln!("[help-runtime-versions] worker panicked during release");
                }
            } else {
                // The process cleanup registry retains the actual worker and
                // its activity permit until join, even if thread creation for
                // this release fails and cleanup must be deferred.
                let cleanup = razer_service::camera::lifecycle::Owner::new();
                if let Err(error) = cleanup.cleanup("help-runtime-versions-release", move || {
                    worker.join().map_err(|_| {
                        anyhow::anyhow!("Help version worker panicked during retained release")
                    })
                }) {
                    eprintln!("[help-runtime-versions] retained cleanup: {error:#}");
                }
            }
        }
    }
}

struct ReadView<'a> {
    storage: &'a mut HostStorage,
    id: u64,
}
impl Drop for ReadView<'_> {
    fn drop(&mut self) {
        self.storage.close_view(self.id);
    }
}
fn read_window_versions(
    storage: &mut HostStorage,
    owner: EntityId,
) -> Result<Option<String>, String> {
    let id = owner.as_u64();
    storage.register_view(HostStorageView {
        id,
        url: format!("razer-local:help/{id}"),
        remote: false,
        destroyed: false,
        crashed: false,
        disposed: false,
        tracked_url: false,
    });
    let view = ReadView { storage, id };
    let reply = view
        .storage
        .call(HostStorageCall {
            store: HostStoreKind::Window,
            sender_id: id,
            action: "getWindowStorageItem".into(),
            payload: serde_json::json!({"key":"mwVersion"}),
            target_url_array: Vec::new(),
        })
        .map_err(|error| format!("Help middleware storage read failed: {error:#}"))?;
    if !reply.defined {
        return Ok(None);
    }
    match reply.value {
        serde_json::Value::String(raw) => Ok(Some(raw)),
        _ => Err("Help middleware storage returned a non-string aggregate".into()),
    }
}
fn read_apps() -> Result<Option<String>, String> {
    razer_storage::source_local::path()
        .and_then(|path| razer_storage::source_local::get_raw(&path, "apps"))
        .map_err(|error| format!("Help application storage read failed: {error:#}"))
}
fn run_work_with(
    work: Work,
    read_apps: impl FnOnce() -> Result<Option<String>, String>,
) -> Option<HelpRuntimeVersionsStatus> {
    if work.cancelled.load(Ordering::Acquire) {
        return None;
    }
    let apps = read_apps();
    if work.cancelled.load(Ordering::Acquire) {
        return None;
    }
    let status = HelpRuntimeVersionsStatus::from_storage(
        work.product_id,
        &work.container_id,
        work.middleware,
        apps,
    );
    (!work.cancelled.load(Ordering::Acquire)).then_some(status)
}

impl AppShell {
    pub(super) fn install_help_runtime_versions_cleanup(&mut self, _cx: &mut Context<Self>) {
        self.register_exit_cleanup(|shell, cx| {
            let mut runtime = std::mem::take(&mut shell.help_runtime_versions);
            runtime.cancel_all();
            runtime.delivery.take();
            runtime.delivery_owner.take();
            let cleanup = cx.background_executor().spawn(async move {
                if let Some(worker) = runtime.worker.take()
                    && worker.join().is_err()
                {
                    eprintln!("[help-runtime-versions] worker panicked during exit cleanup");
                }
                drop(runtime);
            });
            async move { cleanup.await }
        });
    }
    pub(super) fn request_help_runtime_versions(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: HelpRuntimeVersionsRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested
            || !workspace
                .read(cx)
                .accepts_help_runtime_versions(&request, cx)
        {
            return;
        }
        if self
            .help_runtime_versions
            .sessions
            .values()
            .any(|session| !session.request.is_cancelled() && session.request.same_mount(&request))
        {
            return;
        }
        for session in self.help_runtime_versions.sessions.values() {
            if session.request.owner() == request.owner() {
                session.cancel();
            }
        }
        let result = (|| -> anyhow::Result<()> {
            self.help_runtime_versions.start()?;
            self.help_runtime_versions.next_id = self
                .help_runtime_versions
                .next_id
                .checked_add(1)
                .context("Help version request sequence exhausted")?;
            let id = self.help_runtime_versions.next_id;
            let middleware = read_window_versions(&mut self.host_storage, request.owner());
            self.help_runtime_versions.admit(Work {
                id,
                product_id: request.product_id(),
                container_id: request.container_id().to_owned(),
                middleware,
                cancelled: request.cancellation(),
            })?;
            self.help_runtime_versions.sessions.insert(
                id,
                Session {
                    request: request.clone(),
                    workspace: workspace.downgrade(),
                    device: workspace.read(cx).device(cx).clone(),
                    connection_revision: workspace.read(cx).connection_revision(),
                },
            );
            Ok(())
        })();
        if let Err(error) = result {
            workspace.update(cx, |view, cx| {
                let device = view.device(cx).clone();
                view.observe_help_runtime_versions(
                    &device,
                    HelpRuntimeVersionsObservation::failed(request, format!("{error:#}")),
                    cx,
                );
            });
        }
        self.install_help_runtime_versions_pump(window, cx);
    }
    fn install_help_runtime_versions_pump(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.help_runtime_versions.delivery.is_some()
            || self.help_runtime_versions.worker.is_none()
        {
            return;
        }
        let owner = Arc::new(());
        self.help_runtime_versions.delivery_owner = Some(owner.clone());
        self.help_runtime_versions.delivery = Some(cx.spawn_in(window, async move |shell, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                let keep = shell.update_in(cx, |shell, _, cx| {
                    if !shell
                        .help_runtime_versions
                        .delivery_owner
                        .as_ref()
                        .is_some_and(|current| Arc::ptr_eq(current, &owner))
                    {
                        return false;
                    }
                    for session in shell.help_runtime_versions.sessions.values() {
                        if !session.current(shell, cx) {
                            session.cancel();
                        }
                    }
                    for reply in shell.help_runtime_versions.poll() {
                        let Some(session) = shell.help_runtime_versions.sessions.remove(&reply.id)
                        else {
                            continue;
                        };
                        if !session.current(shell, cx) {
                            continue;
                        }
                        let observation = match reply.result {
                            Ok(Some(status)) => HelpRuntimeVersionsObservation::from_status(
                                session.request.clone(),
                                status,
                            ),
                            Ok(None) => continue,
                            Err(error) => HelpRuntimeVersionsObservation::failed(
                                session.request.clone(),
                                error,
                            ),
                        };
                        if let Some(workspace) = session.workspace.upgrade() {
                            workspace.update(cx, |view, cx| {
                                view.observe_help_runtime_versions(
                                    &session.device,
                                    observation,
                                    cx,
                                );
                            });
                        }
                    }
                    shell.help_runtime_versions.retire_if_idle();
                    if shell.help_runtime_versions.worker.is_none() {
                        shell.help_runtime_versions.delivery = None;
                        shell.help_runtime_versions.delivery_owner = None;
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
                            .help_runtime_versions
                            .delivery_owner
                            .as_ref()
                            .is_some_and(|current| Arc::ptr_eq(current, &owner))
                        {
                            shell.help_runtime_versions.cancel_all();
                            shell.help_runtime_versions.delivery = None;
                            shell.help_runtime_versions.delivery_owner = None;
                            if !shell.close_requested {
                                shell.help_runtime_versions.resume_after_cancelled_exit();
                            }
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
#[path = "help_runtime_versions_tests.rs"]
mod tests;
