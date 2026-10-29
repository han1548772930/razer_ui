use super::super::shutdown::CleanupProgress;
use anyhow::{Context as _, ensure};
use razer_ipc::{
    RequestEnvelope, ResponseEnvelope, ServiceRequest,
    hid_broker::{
        BrokerEndpoint, BrokerInstanceId, BrokerRequestId, ClientFrame, ClientHello,
        CloseSessionReceipt, FrameReader, FrameWriter, ServerFrame, ServerHello, SessionIdentity,
        read_frame, write_frame,
    },
};
use std::{
    collections::HashSet,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc as thread_channel,
    },
    thread::{self, JoinHandle as Thread},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::{Notify, mpsc, watch},
    task::{JoinHandle, JoinSet},
    time::{Instant, timeout},
};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const WRITE_TIMEOUT: Duration = Duration::from_secs(30);
const IDLE_GRACE: Duration = Duration::from_secs(2);
const DETACHED_RESTORE_RETRY: Duration = Duration::from_secs(1);
const MAX_SESSION_COMMANDS: usize = 65_536;
const MAX_SESSIONS: usize = 128;
static UNJOINED_EXECUTORS: AtomicUsize = AtomicUsize::new(0);
static EXECUTOR_FAILURES: AtomicUsize = AtomicUsize::new(0);
static EXECUTOR_RELEASED: Notify = Notify::const_new();
type RetainedExecutor = Arc<Mutex<Option<Thread<()>>>>;
static RETAINED_EXECUTORS: Mutex<Vec<RetainedExecutor>> = Mutex::new(Vec::new());

trait SessionExecutor: 'static {
    fn request(&mut self, envelope: RequestEnvelope, canceled: &AtomicBool) -> ResponseEnvelope;
    fn close(&mut self) -> CleanupProgress;
}

impl SessionExecutor for super::super::session::SessionRuntime {
    fn request(&mut self, envelope: RequestEnvelope, _canceled: &AtomicBool) -> ResponseEnvelope {
        self.request(envelope)
    }

    fn close(&mut self) -> CleanupProgress {
        self.close_progress(0)
    }
}

enum Work {
    Request {
        request_id: BrokerRequestId,
        envelope: RequestEnvelope,
    },
    Close,
}

enum Completion {
    Response {
        request_id: BrokerRequestId,
        envelope: ResponseEnvelope,
    },
    Closed {
        state: CloseState,
    },
}

enum CloseState {
    Pending(String),
    Released(Result<(), String>),
}

fn close_state(progress: CleanupProgress) -> CloseState {
    match progress {
        CleanupProgress::Released(response) => CloseState::Released(cleanup_result(&response)),
        CleanupProgress::Pending(response) => CloseState::Pending(
            cleanup_result(&response)
                .err()
                .unwrap_or_else(|| "Native owner release is still pending".to_owned()),
        ),
    }
}

fn attempt_close(executor: &mut impl SessionExecutor) -> CloseState {
    match catch_unwind(AssertUnwindSafe(|| executor.close())) {
        Ok(progress) => close_state(progress),
        Err(_) => CloseState::Pending(
            "broker service cleanup panicked; native ownership is unresolved".to_owned(),
        ),
    }
}

struct ExecutorOwner {
    work: Option<thread_channel::SyncSender<Work>>,
    completions: mpsc::Receiver<Completion>,
    thread: Option<Thread<()>>,
    canceled: Arc<AtomicBool>,
}

impl ExecutorOwner {
    fn start<F, E>(identity: &SessionIdentity, factory: F) -> anyhow::Result<Self>
    where
        F: FnOnce() -> E + Send + 'static,
        E: SessionExecutor,
    {
        let (work, commands) = thread_channel::sync_channel(1);
        let (completion, completions) = mpsc::channel(1);
        let canceled = Arc::new(AtomicBool::new(false));
        let request_canceled = canceled.clone();
        let session = identity.session.as_uuid();
        let thread = thread::Builder::new()
            .name("hid-broker-session".to_owned())
            .spawn(move || {
                // COM and all other thread-affine state are created and released on
                // this OS thread. No native runtime crosses the thread boundary.
                let mut executor = Some(factory());
                let mut cleanup_confirmed = false;
                loop {
                    match commands.recv() {
                        Ok(Work::Request {
                            request_id,
                            envelope,
                        }) => {
                            let id = envelope.id;
                            let shutdown = matches!(envelope.request, ServiceRequest::Shutdown);
                            let response = super::with_request_context(
                                session,
                                request_canceled.clone(),
                                || match catch_unwind(AssertUnwindSafe(|| {
                                    executor
                                        .as_mut()
                                        .unwrap()
                                        .request(envelope, &request_canceled)
                                })) {
                                    Ok(response) => response,
                                    Err(_) => ResponseEnvelope {
diagnostics: None,
                                        id,
                                        data: None,
                                        error: Some(
                                            "broker service request panicked; cleanup is required"
                                                .to_owned(),
                                        ),
                                        fatal: true,
                                    },
                                },
                            );
                            if shutdown && cleanup_result(&response).is_ok() {
                                cleanup_confirmed = true;
                            }
                            if completion
                                .blocking_send(Completion::Response {
                                    request_id,
                                    envelope: response,
                                })
                                .is_err()
                            {
                                break;
                            }
                        }
                        Ok(Work::Close) => {
                            request_canceled.store(false, Ordering::Release);
                            let state = if cleanup_confirmed {
                                CloseState::Released(Ok(()))
                            } else {
                                super::with_request_context(
                                    session,
                                    request_canceled.clone(),
                                    || attempt_close(executor.as_mut().unwrap()),
                                )
                            };
                            if let CloseState::Released(result) = state {
                                if result.is_err() { EXECUTOR_FAILURES.fetch_add(1, Ordering::AcqRel); }
                                drop(executor.take());
                                let _ = completion.blocking_send(Completion::Closed { state: CloseState::Released(result) });
                                return;
                            }
                            if completion
                                .blocking_send(Completion::Closed { state })
                                .is_err()
                            {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                // An unexpected receiver loss cannot release live native owners.
                // Retain them on this thread until their actual restoration succeeds.
                loop {
                    request_canceled.store(false, Ordering::Release);
                    let state = if cleanup_confirmed {
                        CloseState::Released(Ok(()))
                    } else {
                        super::with_request_context(session, request_canceled.clone(), || {
                            attempt_close(executor.as_mut().unwrap())
                        })
                    };
                    match state {
                        CloseState::Released(result) => {
                            if let Err(error) = result {
                                EXECUTOR_FAILURES.fetch_add(1, Ordering::AcqRel);
                                eprintln!("[HID broker session] native owners released with restoration failure: {error}");
                            }
                            drop(executor.take());
                            return;
                        }
                        CloseState::Pending(error) => {
                            eprintln!(
                                "[HID broker session] restoration remains unconfirmed: {error}"
                            );
                        }
                    }
                    thread::sleep(DETACHED_RESTORE_RETRY);
                }
            })
            .context("broker session thread could not be started")?;
        UNJOINED_EXECUTORS.fetch_add(1, Ordering::AcqRel);
        Ok(Self {
            work: Some(work),
            completions,
            thread: Some(thread),
            canceled,
        })
    }

    fn send(&self, work: Work) -> anyhow::Result<()> {
        self.work
            .as_ref()
            .context("broker session executor is closed")?
            .try_send(work)
            .map_err(|error| {
                anyhow::anyhow!("broker session executor queue rejected work: {error}")
            })
    }

    async fn join(&mut self) -> anyhow::Result<()> {
        self.work.take();
        if let Some(thread) = self.thread.take() {
            tokio::task::spawn_blocking(move || join_executor(thread))
                .await
                .context("broker session join task failed")?
                .map_err(|_| anyhow::anyhow!("broker session thread panicked"))?;
        }
        Ok(())
    }
}

fn join_executor(thread: Thread<()>) -> std::thread::Result<()> {
    let result = thread.join();
    if result.is_err() {
        EXECUTOR_FAILURES.fetch_add(1, Ordering::AcqRel);
    }
    UNJOINED_EXECUTORS.fetch_sub(1, Ordering::AcqRel);
    EXECUTOR_RELEASED.notify_one();
    result
}

impl Drop for ExecutorOwner {
    fn drop(&mut self) {
        self.canceled.store(true, Ordering::Release);
        self.work.take();
        let Some(thread) = self.thread.take() else {
            return;
        };
        let retained = Arc::new(Mutex::new(Some(thread)));
        let reaper = retained.clone();
        if let Err(error) = thread::Builder::new()
            .name("hid-session-reaper".to_owned())
            .spawn(move || {
                let thread = reaper
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .take();
                if let Some(thread) = thread {
                    if join_executor(thread).is_err() {
                        eprintln!("[HID broker session] retained executor panicked");
                    }
                }
            })
        {
            eprintln!("[HID broker session] executor reaper failed; owner retained: {error}");
            RETAINED_EXECUTORS
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(retained);
        }
    }
}

fn cleanup_result(response: &ResponseEnvelope) -> Result<(), String> {
    if let Some(error) = &response.error {
        return Err(error.clone());
    }
    if response.fatal {
        return Err("broker cleanup returned a fatal response".to_owned());
    }
    #[derive(serde::Deserialize)]
    struct Receipt {
        stopped: bool,
    }
    let data = response
        .data
        .as_ref()
        .ok_or_else(|| "broker cleanup returned no receipt".to_owned())?;
    let receipt: Receipt = serde_json::from_value(data.clone())
        .map_err(|error| format!("broker cleanup receipt is invalid: {error}"))?;
    if receipt.stopped {
        Ok(())
    } else {
        Err("broker did not confirm its session restoration".to_owned())
    }
}

struct Pending {
    request_id: BrokerRequestId,
    envelope_id: u64,
}

async fn serve_stream<S, F, E>(
    mut stream: S,
    endpoint: BrokerEndpoint,
    instance: BrokerInstanceId,
    mut stopping: watch::Receiver<bool>,
    factory: F,
) -> anyhow::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    F: FnOnce() -> E + Send + 'static,
    E: SessionExecutor,
{
    let hello: ClientHello = timeout(HANDSHAKE_TIMEOUT, read_frame(&mut stream))
        .await
        .context("broker client handshake timed out")??;
    hello.validate(endpoint.id(), instance)?;
    let hello = ServerHello::new(&hello, instance)?;
    timeout(HANDSHAKE_TIMEOUT, write_frame(&mut stream, &hello))
        .await
        .context("broker server handshake write timed out")??;
    serve_session(stream, hello.identity, &mut stopping, factory).await
}

async fn serve_session<S, F, E>(
    stream: S,
    identity: SessionIdentity,
    stopping: &mut watch::Receiver<bool>,
    factory: F,
) -> anyhow::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    F: FnOnce() -> E + Send + 'static,
    E: SessionExecutor,
{
    let mut executor = ExecutorOwner::start(&identity, factory)?;
    let (reader, writer) = tokio::io::split(stream);
    let (incoming, mut frames) = mpsc::channel(4);
    let mut reader: JoinHandle<anyhow::Result<()>> = tokio::spawn(async move {
        let mut reader = FrameReader::new(reader);
        loop {
            let frame = reader.read::<ClientFrame>().await;
            let failed = frame.is_err();
            if incoming.send(frame).await.is_err() || failed {
                return Ok(());
            }
        }
    });
    let (outgoing, mut responses) = mpsc::channel::<ServerFrame>(2);
    let mut writer: JoinHandle<anyhow::Result<()>> = tokio::spawn(async move {
        let mut writer = FrameWriter::new(writer);
        while let Some(response) = responses.recv().await {
            timeout(WRITE_TIMEOUT, writer.write(&response))
                .await
                .context("broker client response write timed out")??;
        }
        Ok(())
    });
    let mut commands = HashSet::new();
    let mut pending: Option<Pending> = None;
    let mut last_completed = None;
    let mut close_request = None;
    let mut closing = false;
    let mut cleanup_running = false;
    let mut connected = true;
    let mut writer_finished = false;
    let mut failure = None;
    let mut cleanup_not_before = Instant::now();
    let mut stopping_seen = false;
    loop {
        tokio::select! {
            incoming = frames.recv(), if connected => {
                let frame = match incoming {
                    Some(Ok(frame)) => frame,
                    Some(Err(error)) => { failure = Some(error); connected = false; closing = true; executor.canceled.store(true, Ordering::Release); continue; }
                    None => { connected = false; closing = true; executor.canceled.store(true, Ordering::Release); continue; }
                };
                let command = (|| -> anyhow::Result<()> {
                    ensure!(frame.identity() == &identity, "broker client session identity mismatch");
                    match frame {
                        ClientFrame::Request { request_id, envelope, .. } => {
                            ensure!(!closing && pending.is_none(), "broker session already owns a request or is closing");
                            register_command(&mut commands, request_id)?;
                            executor.canceled.store(false, Ordering::Release);
                            let envelope_id = envelope.id;
                            closing = matches!(envelope.request, ServiceRequest::Shutdown);
                            executor.send(Work::Request { request_id, envelope })?;
                            pending = Some(Pending { request_id, envelope_id });
                        }
                        ClientFrame::Cancel { request_id, .. } => {
                            if pending.as_ref().is_some_and(|pending| pending.request_id == request_id) {
                                executor.canceled.store(true, Ordering::Release);
                            } else {
                                ensure!(last_completed == Some(request_id), "broker cancel UUID is not the active or just-completed request");
                            }
                        }
                        ClientFrame::CloseSession { request_id, .. } => {
                            ensure!(close_request.is_none() && !cleanup_running, "broker cleanup request is already pending");
                            register_command(&mut commands, request_id)?;
                            close_request = Some(request_id);
                            closing = true;
                            if pending.is_some() { executor.canceled.store(true, Ordering::Release); }
                        }
                    }
                    Ok(())
                })();
                if let Err(error) = command {
                    failure = Some(error); connected = false; closing = true;
                    executor.canceled.store(true, Ordering::Release);
                }
            }
            completion = executor.completions.recv() => {
                match completion {
                    Some(Completion::Response { request_id, envelope }) => {
                        let valid = pending.as_ref().is_some_and(|pending| pending.request_id == request_id && pending.envelope_id == envelope.id);
                        if !valid {
                            failure = Some(anyhow::anyhow!("broker executor response association mismatch"));
                            connected = false; closing = true;
                        }
                        pending = None;
                        last_completed = Some(request_id);
                        if envelope.fatal { closing = true; }
                        if connected && outgoing.try_send(ServerFrame::Response { identity: identity.clone(), request_id, envelope }).is_err() {
                            failure = Some(anyhow::anyhow!("broker client response queue is unavailable"));
                            connected = false; closing = true;
                        }
                    }
                    Some(Completion::Closed { state }) => {
                        cleanup_running = false;
                        match state {
                            CloseState::Released(result) => {
                                if let Err(error) = executor.join().await {
                                    failure = Some(error);
                                    break;
                                }
                                if connected {
                                    if let Some(request_id) = close_request.take() {
                                        if outgoing.try_send(ServerFrame::SessionClosed {
                                            identity: identity.clone(), request_id,
                                            receipt: match &result {
                                                Ok(()) => CloseSessionReceipt::confirmed(),
                                                Err(error) => CloseSessionReceipt::restoration_failed(error.clone()),
                                            },
                                        }).is_err() {
                                            failure = Some(anyhow::anyhow!("broker cleanup receipt could not be queued"));
                                        }
                                    }
                                }
                                if let Err(error) = result { failure = Some(anyhow::anyhow!("{error}")); }
                                break;
                            }
                            CloseState::Pending(error) => {
                                if connected {
                                    if let Some(request_id) = close_request.take() {
                                        if outgoing.try_send(ServerFrame::SessionClosed {
                                            identity: identity.clone(), request_id,
                                            receipt: CloseSessionReceipt::pending(error.clone()),
                                        }).is_err() { connected = false; }
                                    }
                                }
                                eprintln!("[HID broker session] cleanup remains unconfirmed: {error}");
                                cleanup_not_before = Instant::now() + DETACHED_RESTORE_RETRY;
                            }
                        }
                    }
                    None => {
                        failure = Some(match executor.join().await {
                            Ok(()) => anyhow::anyhow!("broker executor exited without confirming session cleanup"),
                            Err(error) => error,
                        });
                        break;
                    }
                }
            }
            result = &mut writer, if !writer_finished => {
                writer_finished = true;
                match result {
                    Ok(Ok(())) => {},
                    Ok(Err(error)) => failure = Some(error),
                    Err(error) => failure = Some(anyhow::anyhow!("broker response writer failed: {error}")),
                }
                connected = false; closing = true;
                executor.canceled.store(true, Ordering::Release);
            }
            _ = stopping.changed(), if !stopping_seen => {
                stopping_seen = true;
                connected = false; closing = true;
                executor.canceled.store(true, Ordering::Release);
            }
            _ = tokio::time::sleep_until(cleanup_not_before), if closing && !cleanup_running && pending.is_none() && !connected => {}
        }
        if closing
            && pending.is_none()
            && !cleanup_running
            && (close_request.is_some() || !connected)
            && (connected || Instant::now() >= cleanup_not_before)
        {
            executor.canceled.store(false, Ordering::Release);
            if let Err(error) = executor.send(Work::Close) {
                failure = Some(error);
                if let Err(error) = executor.join().await {
                    failure = Some(error);
                }
                break;
            }
            cleanup_running = true;
        }
    }
    reader.abort();
    let _ = (&mut reader).await;
    drop(outgoing);
    if !writer_finished {
        match writer.await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => failure = Some(error),
            Err(error) => failure = Some(anyhow::anyhow!("broker response writer failed: {error}")),
        }
    }
    if let Some(error) = failure {
        Err(error)
    } else {
        Ok(())
    }
}

fn register_command(
    commands: &mut HashSet<BrokerRequestId>,
    id: BrokerRequestId,
) -> anyhow::Result<()> {
    ensure!(
        commands.len() < MAX_SESSION_COMMANDS,
        "broker session command limit was reached"
    );
    ensure!(commands.insert(id), "broker command UUID was reused");
    Ok(())
}

pub(super) fn run(endpoint: BrokerEndpoint, instance: BrokerInstanceId) -> anyhow::Result<()> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("broker listener runtime could not be created")?
        .block_on(run_listener(endpoint, instance))
}

#[cfg(windows)]
async fn run_listener(endpoint: BrokerEndpoint, instance: BrokerInstanceId) -> anyhow::Result<()> {
    let mut listener = endpoint.bind_windows_pipe(true)?;
    let mut sessions = JoinSet::new();
    let (stopping, _) = watch::channel(false);
    let mut idle_deadline = Instant::now() + IDLE_GRACE;
    let mut failure = None;
    let mut session_failures = Vec::new();
    loop {
        tokio::select! {
            accepted = listener.connect() => {
                if let Err(error) = accepted { failure = Some(anyhow::Error::new(error)); break; }
                let next = match endpoint.bind_windows_pipe(false) {
                    Ok(next) => next,
                    Err(error) => { failure = Some(error); break; }
                };
                let connected = std::mem::replace(&mut listener, next);
                if !session_available(sessions.len()) { drop(connected); continue; }
                if let Err(error) = endpoint.verify_windows_client(&connected) {
                    eprintln!("[HID broker peer rejected] {error:#}");
                    continue;
                }
                sessions.spawn(serve_stream(connected, endpoint.clone(), instance, stopping.subscribe(), super::super::session::SessionRuntime::new));
            }
            completed = sessions.join_next(), if !sessions.is_empty() => {
                log_session(completed, &mut session_failures);
                if sessions.is_empty() { idle_deadline = Instant::now() + IDLE_GRACE; }
            }
            _ = EXECUTOR_RELEASED.notified(), if sessions.is_empty() => { idle_deadline = Instant::now() + IDLE_GRACE; }
            _ = tokio::time::sleep_until(idle_deadline), if sessions.is_empty() && UNJOINED_EXECUTORS.load(Ordering::Acquire) == 0 => break,
        }
    }
    if failure.is_some() {
        let _ = stopping.send(true);
    }
    while let Some(result) = sessions.join_next().await {
        log_session(Some(result), &mut session_failures);
    }
    let cleanup = prepare_listener_shutdown();
    drop(listener);
    listener_result(failure, session_failures, cleanup, Ok(()))
}

#[cfg(unix)]
async fn run_listener(endpoint: BrokerEndpoint, instance: BrokerInstanceId) -> anyhow::Result<()> {
    use std::os::unix::fs::MetadataExt;
    let listener = endpoint.bind_unix_listener()?;
    let metadata = std::fs::symlink_metadata(endpoint.as_unix_path())?;
    let mut sessions = JoinSet::new();
    let (stopping, _) = watch::channel(false);
    let mut idle_deadline = Instant::now() + IDLE_GRACE;
    let mut failure = None;
    let mut session_failures = Vec::new();
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let (connected, _) = match accepted {
                    Ok(connected) => connected,
                    Err(error) => { failure = Some(anyhow::Error::new(error)); break; }
                };
                if !session_available(sessions.len()) { drop(connected); continue; }
                if let Err(error) = endpoint.verify_unix_peer(&connected) {
                    eprintln!("[HID broker peer rejected] {error:#}");
                    continue;
                }
                sessions.spawn(serve_stream(connected, endpoint.clone(), instance, stopping.subscribe(), super::super::session::SessionRuntime::new));
            }
            completed = sessions.join_next(), if !sessions.is_empty() => {
                log_session(completed, &mut session_failures);
                if sessions.is_empty() { idle_deadline = Instant::now() + IDLE_GRACE; }
            }
            _ = EXECUTOR_RELEASED.notified(), if sessions.is_empty() => { idle_deadline = Instant::now() + IDLE_GRACE; }
            _ = tokio::time::sleep_until(idle_deadline), if sessions.is_empty() && UNJOINED_EXECUTORS.load(Ordering::Acquire) == 0 => break,
        }
    }
    if failure.is_some() {
        let _ = stopping.send(true);
    }
    while let Some(result) = sessions.join_next().await {
        log_session(Some(result), &mut session_failures);
    }
    let cleanup = prepare_listener_shutdown();
    drop(listener);
    let release = endpoint.release_unix_listener(metadata.dev(), metadata.ino());
    listener_result(failure, session_failures, cleanup, release)
}

fn log_session(
    result: Option<Result<anyhow::Result<()>, tokio::task::JoinError>>,
    failures: &mut Vec<String>,
) {
    match result {
        Some(Ok(Ok(()))) | None => {}
        Some(Ok(Err(error))) => {
            let error = format!("Broker session: {error:#}");
            eprintln!("{error}");
            failures.push(error);
        }
        Some(Err(error)) => {
            let error = format!("Broker session task: {error}");
            eprintln!("{error}");
            failures.push(error);
        }
    }
}

fn listener_result(
    failure: Option<anyhow::Error>,
    mut failures: Vec<String>,
    cleanup: anyhow::Result<()>,
    release: anyhow::Result<()>,
) -> anyhow::Result<()> {
    if let Some(error) = failure {
        failures.push(format!("Broker listener: {error:#}"));
    }
    for (owner, result) in [("Native cleanup", cleanup), ("Endpoint release", release)] {
        if let Err(error) = result {
            failures.push(format!("{owner}: {error:#}"));
        }
    }
    if EXECUTOR_FAILURES.load(Ordering::Acquire) != 0 {
        failures.push(
            "Broker executor released with an unconfirmed restoration or thread failure".to_owned(),
        );
    }
    ensure!(failures.is_empty(), "{}", failures.join("; "));
    Ok(())
}

fn session_available(active: usize) -> bool {
    active < MAX_SESSIONS
}

fn prepare_listener_shutdown() -> anyhow::Result<()> {
    // The exclusive listener remains alive while underlying native owners drain;
    // otherwise another process could acquire the same installation endpoint.
    prepare_listener_shutdown_with(
        || super::prepare_shutdown(),
        || thread::sleep(DETACHED_RESTORE_RETRY),
    )
}

fn prepare_listener_shutdown_with(
    mut prepare: impl FnMut() -> super::ShutdownProgress,
    mut wait: impl FnMut(),
) -> anyhow::Result<()> {
    let mut failures = Vec::new();
    loop {
        let (released, observed) = match prepare() {
            super::ShutdownProgress::Pending { failures } => (false, failures),
            super::ShutdownProgress::Released { failures } => (true, failures),
        };
        for error in observed {
            if !failures.contains(&error) {
                failures.push(error);
            }
        }
        if released {
            ensure!(failures.is_empty(), "{}", failures.join("; "));
            return Ok(());
        }
        if !failures.is_empty() {
            eprintln!(
                "[HID broker shutdown] native owners remain pending: {}",
                failures.join("; ")
            );
        }
        wait();
    }
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
