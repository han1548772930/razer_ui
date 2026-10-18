//! Mounted Help report requests, native-worker lifetime and local file export.
use super::*;
use anyhow::{Context as _, ensure};
use razer_device::system_info::{Data, MAXIMUM_REPORT_BYTES, Query, Response, SystemDetails};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_pages::features::{HelpSystemInfoCommand, HelpSystemInfoRequested};
use razer_platform::file_dialog::{SaveFileDialog, SaveFileOptions};
use razer_storage::file_transfer::{Operation, Publication, Transfer};
use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
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

struct Work {
    id: u64,
    product_id: u32,
    query: Query,
    file_name: String,
    window: isize,
    cancelled: Arc<AtomicBool>,
    operation: Operation,
}
struct Session {
    request: HelpSystemInfoRequested,
    workspace: WeakEntity<ProductWorkspace>,
    device: Device,
    connection_revision: u64,
    transfer: Transfer,
    cancelled: Arc<AtomicBool>,
}
impl Session {
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        self.transfer.cancel();
    }
    fn current(&self, shell: &AppShell, cx: &App) -> bool {
        !shell.close_requested
            && !self.request.is_cancelled()
            && !self.cancelled.load(Ordering::Acquire)
            && self.workspace.upgrade().is_some_and(|workspace| {
                shell.devices.contains(&workspace)
                    && workspace.read(cx).connection_revision() == self.connection_revision
                    && workspace
                        .read(cx)
                        .accepts_help_system_info(&self.request, cx)
                    && same_device(workspace.read(cx).device(cx), &self.device)
            })
    }
}
fn same_device(current: &Device, owner: &Device) -> bool {
    current.product_id == owner.product_id
        && current.serial_number == owner.serial_number
        && current.device_container_id == owner.device_container_id
        && current.edition_id == owner.edition_id
        && current.layout_id == owner.layout_id
}
enum Outcome {
    Display(SystemDetails),
    Saved(PathBuf),
    Cancelled,
}
struct Reply {
    id: u64,
    result: Result<Outcome, String>,
}

#[derive(Default)]
pub(super) struct Runtime {
    next_id: u64,
    sessions: BTreeMap<u64, Session>,
    commands: Option<mpsc::Sender<Work>>,
    replies: Option<mpsc::Receiver<Reply>>,
    worker: Option<JoinHandle<Option<PathBuf>>>,
    delivery: Option<Task<()>>,
    delivery_owner: Option<Arc<()>>,
    stopping: bool,
    diagnostic: Option<String>,
    pending: VecDeque<Work>,
    directory: Option<PathBuf>,
}
impl Runtime {
    pub(super) fn cancel_all(&mut self) {
        self.stopping = true;
        for session in self.sessions.values() {
            session.cancel();
        }
        // The worker drains cancelled queued operations and their file permits,
        // then exits after the current native/dialog/file cleanup completes.
        self.commands.take();
        self.pending.clear();
    }
    pub(super) fn resume_after_cancelled_exit(&mut self) {
        self.stopping = false;
        // Old scopes stay cancelled. Requests admitted during cleanup wait
        // until the closed channel's worker has been joined before restart.
    }
    fn start(&mut self) -> anyhow::Result<()> {
        ensure!(!self.stopping, "Help system-info owner is stopping");
        if self.worker.is_some() {
            return Ok(());
        }
        let (commands, incoming) = mpsc::channel();
        let (finished, replies) = mpsc::channel();
        let activity = WorkerActivity::new();
        let initial_directory = self.directory.take();
        let worker = thread::Builder::new()
            .name("help-system-info-client".into())
            .spawn(move || {
                let _activity = activity;
                let mut directory = initial_directory;
                while let Ok(work) = incoming.recv() {
                    let result =
                        run_work(&work, &mut directory).map_err(|error| format!("{error:#}"));
                    let _ = finished.send(Reply {
                        id: work.id,
                        result,
                    });
                }
                directory
            })?;
        self.commands = Some(commands);
        self.replies = Some(replies);
        self.worker = Some(worker);
        Ok(())
    }
    fn admit(&mut self, work: Work) -> anyhow::Result<()> {
        if let Some(commands) = &self.commands {
            commands
                .send(work)
                .context("Help report worker rejected the request")?;
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
    fn reap(&mut self) {
        if self.worker.as_ref().is_some_and(JoinHandle::is_finished) {
            match self.worker.take().unwrap().join() {
                Ok(directory) => self.directory = directory,
                Err(_) => {
                    self.diagnostic = Some("Help system-info worker panicked during cleanup".into())
                }
            }
            self.commands.take();
            self.replies.take();
            self.sessions
                .retain(|id, _| self.pending.iter().any(|work| work.id == *id));
            if !self.stopping && !self.pending.is_empty() {
                match self.start() {
                    Ok(()) => {
                        while let Some(work) = self.pending.pop_front() {
                            if let Err(error) = self.admit(work) {
                                self.diagnostic = Some(format!("{error:#}"));
                            }
                        }
                    }
                    Err(error) => {
                        self.diagnostic = Some(format!("{error:#}"));
                        self.pending.clear();
                        self.sessions.clear();
                    }
                }
            }
        }
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.cancel_all();
        self.reap();
        // During normal exit the shell waits for active_workers and its pump
        // reaps this handle. Unexpected owner release still closes admission;
        // every native/COM/file resource stays owned by the draining worker.
    }
}

fn query_native(product_id: u32, query: Query, cancelled: &AtomicBool) -> anyhow::Result<Response> {
    ensure!(
        !cancelled.load(Ordering::Acquire),
        "Help system-info request was cancelled"
    );
    let mut client = ServiceClient::spawn()?;
    // These getters have no native cancellation API. Let an admitted getter
    // return through its verified cleanup; discard its result after unmount.
    // The IPC timeout still terminates a hung exclusive worker explicitly.
    let result = client
        .request(ServiceRequest::HelpSystemInfo { product_id, query })
        .and_then(|value| {
            serde_json::from_value::<Response>(value).context("Invalid system-info worker response")
        })
        .and_then(|response| {
            response.validate(product_id, query)?;
            Ok(response)
        });
    let shutdown = if client.is_stopped() {
        Ok(())
    } else {
        client.request(ServiceRequest::Shutdown).map(|_| ())
    };
    let mut errors = Vec::new();
    if let Err(error) = shutdown {
        errors.push(format!("service shutdown: {error:#}"));
    }
    let mut termination_failed = false;
    while let Err(error) = client.finish_termination() {
        if !termination_failed {
            errors.push(format!("service termination: {error:#}"));
            termination_failed = true;
        }
        // Keep every unreaped process/pipe handle on this background owner.
        // Exit remains pending until the OS confirms termination and joins.
        thread::park_timeout(Duration::from_millis(50));
    }
    match (result, errors.is_empty()) {
        (Ok(response), true) => Ok(response),
        (Err(error), true) => Err(error),
        (Ok(_), false) => anyhow::bail!("{}", errors.join("; ")),
        (Err(error), false) => Err(error.context(errors.join("; "))),
    }
}

fn run_work(work: &Work, directory: &mut Option<PathBuf>) -> anyhow::Result<Outcome> {
    run_work_with(
        work,
        directory,
        query_native,
        |window, options, cancelled| {
            let dialog = SaveFileDialog::start(window, options, cancelled)?;
            dialog.blocking_result()
        },
    )
}

fn run_work_with(
    work: &Work,
    directory: &mut Option<PathBuf>,
    mut query: impl FnMut(u32, Query, &AtomicBool) -> anyhow::Result<Response>,
    mut save_dialog: impl FnMut(
        isize,
        SaveFileOptions,
        Arc<AtomicBool>,
    ) -> anyhow::Result<Option<PathBuf>>,
) -> anyhow::Result<Outcome> {
    if work.cancelled.load(Ordering::Acquire) {
        return Ok(Outcome::Cancelled);
    }
    work.operation.check().map_err(anyhow::Error::msg)?;
    let response = query(work.product_id, work.query, &work.cancelled)?;
    response.validate(work.product_id, work.query)?;
    if work.cancelled.load(Ordering::Acquire) {
        return Ok(Outcome::Cancelled);
    }
    match response.data {
        Data::Display(details) => Ok(Outcome::Display(details)),
        Data::Full(report) => {
            let bytes = report.file_bytes()?;
            let path = save_dialog(
                work.window,
                SaveFileOptions {
                    title: "Save as".into(),
                    file_name: work.file_name.clone(),
                    button_label: "Save file".into(),
                    filter_name: "Synapse 4".into(),
                    extension: "synapse4".into(),
                    directory: directory.clone(),
                },
                work.cancelled.clone(),
            )?;
            let Some(path) = path else {
                return Ok(Outcome::Cancelled);
            };
            *directory = path.parent().map(PathBuf::from);
            if work.cancelled.load(Ordering::Acquire) {
                return Ok(Outcome::Cancelled);
            }
            work.operation
                .write(&path, &bytes, MAXIMUM_REPORT_BYTES, Publication::Replace)
                .map_err(anyhow::Error::msg)?;
            Ok(Outcome::Saved(path))
        }
    }
}

fn window_handle(window: &Window) -> anyhow::Result<isize> {
    #[cfg(windows)]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let handle =
            HasWindowHandle::window_handle(window).map_err(|error| anyhow::anyhow!("{error}"))?;
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            anyhow::bail!("Help file export requires its owning Win32 window");
        };
        Ok(handle.hwnd.get())
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        anyhow::bail!("Local Blade report export requires the Windows adapter")
    }
}

impl AppShell {
    pub(super) fn install_help_system_info_cleanup(&mut self, _cx: &mut Context<Self>) {
        self.register_exit_cleanup(|shell, cx| {
            let mut runtime = std::mem::take(&mut shell.help_system_info);
            runtime.cancel_all();
            runtime.delivery.take();
            runtime.delivery_owner.take();
            let cleanup = cx.background_executor().spawn(async move {
                if let Some(worker) = runtime.worker.take() {
                    if worker.join().is_err() {
                        eprintln!("[help-system-info] worker panicked during exit cleanup");
                    }
                }
                drop(runtime);
            });
            async move { cleanup.await }
        });
    }
    pub(super) fn request_help_system_info(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: HelpSystemInfoRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let result = (|| -> anyhow::Result<()> {
            ensure!(
                !self.close_requested && workspace.read(cx).accepts_help_system_info(&request, cx),
                "Mounted Help report request has expired"
            );
            if request.command() == HelpSystemInfoCommand::Observe
                && self.help_system_info.sessions.values().any(|session| {
                    !session.cancelled.load(Ordering::Acquire)
                        && session.request.command() == HelpSystemInfoCommand::Observe
                        && session.request.same_mount(&request)
                })
            {
                return Ok(());
            }
            self.help_system_info.start()?;
            let query = match request.command() {
                HelpSystemInfoCommand::Observe => Query::Display,
                HelpSystemInfoCommand::SaveLogs => Query::Full,
            };
            let window = if query == Query::Full {
                window_handle(window)?
            } else {
                0
            };
            let transfer = Transfer::new();
            let operation = transfer.accept().map_err(anyhow::Error::msg)?;
            self.help_system_info.next_id = self
                .help_system_info
                .next_id
                .checked_add(1)
                .context("Help report request sequence exhausted")?;
            let id = self.help_system_info.next_id;
            let tick = SystemTime::now()
                .duration_since(UNIX_EPOCH)?
                .as_millis()
                .try_into()
                .context("System time exceeds the report filename range")?;
            let cancelled = Arc::new(AtomicBool::new(false));
            let work = Work {
                id,
                product_id: request.product_id(),
                query,
                file_name: request.file_name(tick),
                window,
                cancelled: cancelled.clone(),
                operation,
            };
            self.help_system_info.admit(work)?;
            self.help_system_info.sessions.insert(
                id,
                Session {
                    request: request.clone(),
                    workspace: workspace.downgrade(),
                    device: workspace.read(cx).device(cx).clone(),
                    connection_revision: workspace.read(cx).connection_revision(),
                    transfer,
                    cancelled,
                },
            );
            Ok(())
        })();
        if let Err(error) = result {
            let error = format!("{error:#}");
            eprintln!("[help-system-info] {error}");
            self.help_system_info.diagnostic = Some(error.clone());
            if request.command() == HelpSystemInfoCommand::Observe {
                workspace.update(cx, |view, cx| {
                    let device = view.device(cx).clone();
                    view.observe_help_system_info(&device, &request, Err(error), cx)
                });
            }
        }
        self.install_help_system_info_pump(window, cx);
    }

    fn install_help_system_info_pump(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.help_system_info.delivery.is_some() || self.help_system_info.worker.is_none() {
            return;
        }
        let owner = Arc::new(());
        self.help_system_info.delivery_owner = Some(owner.clone());
        self.help_system_info.delivery = Some(cx.spawn_in(window, async move |shell, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                let keep = shell.update_in(cx, |shell, _, cx| {
                    if !shell
                        .help_system_info
                        .delivery_owner
                        .as_ref()
                        .is_some_and(|current| Arc::ptr_eq(current, &owner))
                    {
                        return false;
                    }
                    let cancelled = shell
                        .help_system_info
                        .sessions
                        .iter()
                        .filter_map(|(id, session)| (!session.current(shell, cx)).then_some(*id))
                        .collect::<Vec<_>>();
                    for id in cancelled {
                        if let Some(session) = shell.help_system_info.sessions.get(&id) {
                            session.cancel();
                        }
                    }
                    let replies = shell.help_system_info.drain();
                    for reply in replies {
                        let Some(session) = shell.help_system_info.sessions.remove(&reply.id)
                        else {
                            continue;
                        };
                        if !session.current(shell, cx) {
                            continue;
                        }
                        let result = match reply.result {
                            Ok(Outcome::Display(details)) => Some(Ok(details)),
                            Ok(Outcome::Saved(path)) => {
                                eprintln!(
                                    "[help-system-info] saved local report: {}",
                                    path.display()
                                );
                                None
                            }
                            Ok(Outcome::Cancelled) => None,
                            Err(error) => {
                                eprintln!("[help-system-info] {error}");
                                shell.help_system_info.diagnostic = Some(error.clone());
                                (session.request.command() == HelpSystemInfoCommand::Observe)
                                    .then_some(Err(error))
                            }
                        };
                        if let (Some(result), Some(workspace)) =
                            (result, session.workspace.upgrade())
                        {
                            workspace.update(cx, |view, cx| {
                                view.observe_help_system_info(
                                    &session.device,
                                    &session.request,
                                    result,
                                    cx,
                                );
                            });
                        }
                    }
                    shell.help_system_info.reap();
                    if shell.help_system_info.worker.is_none() {
                        shell.help_system_info.delivery = None;
                        shell.help_system_info.delivery_owner = None;
                        if shell.close_requested && shell.save_task.is_none() {
                            shell.continue_save_queue(true, cx);
                        }
                        return false;
                    }
                    true
                });
                if keep.is_err() {
                    // A destroyed window cannot service this pump. Invalidate
                    // only its owner, then retain the draining worker for the
                    // shell's cleanup hook or a replacement window's pump.
                    let _ = shell.update(cx, |shell, _| {
                        if shell
                            .help_system_info
                            .delivery_owner
                            .as_ref()
                            .is_some_and(|current| Arc::ptr_eq(current, &owner))
                        {
                            shell.help_system_info.cancel_all();
                            shell.help_system_info.delivery = None;
                            shell.help_system_info.delivery_owner = None;
                            if !shell.close_requested {
                                shell.help_system_info.resume_after_cancelled_exit();
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
#[path = "help_system_info_tests.rs"]
mod tests;
