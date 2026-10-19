//! Retained filter-driver stream and source-defined hook/redirect controls.
use anyhow::{Context, Result, ensure};
use razer_device::mapping_native_output::{
    CompletionPoll, DriverInputReport, DriverOutputError, MappingOutputTransport, SubmissionId,
};
use razer_device::mapping_native_session::NativeSubmissionId;
use razer_device::mapping_native_session::{
    NativeCapabilitySet, NativeCompletion, NativeInputPoll, NativeOperation, NativeRoute,
    NativeSession, NativeSessionState, NativeSessionTransport,
    hooks::{self, HookInput, RedirectKind},
};
use std::{
    collections::BTreeSet,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

#[path = "driver/rotation.rs"]
mod rotation;
use razer_device::mapping_native_rotation::RotationRequest;
pub(crate) use rotation::Rotation;

/// Values are sampled on the driver thread at completed input consumption,
/// before an actor queue or a device task can delay processing.
#[derive(Clone, Debug)]
pub(super) struct Packet {
    pub bytes: Vec<u8>,
    pub modifiers: razer_device::mapping_native_events::NativeModifierSnapshot,
    pub time_tick: u64,
}
impl Packet {
    pub(super) fn capture(bytes: Vec<u8>) -> Result<Self> {
        let time_tick = razer_platform::event_clock::tick()?;
        Ok(Self {
            bytes,
            time_tick,
            modifiers: capture_modifiers(),
        })
    }
}

pub(super) fn capture_modifiers() -> razer_device::mapping_native_events::NativeModifierSnapshot {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetKeyState;
    // GetKeyState reads this thread's keyboard snapshot and owns no pointers.
    let pressed = |key| unsafe { GetKeyState(key) } < 0;
    razer_device::mapping_native_events::NativeModifierSnapshot {
        left_alt: pressed(164),
        left_control: pressed(162),
        left_shift: pressed(160),
        left_windows: pressed(91),
        right_alt: pressed(165),
        right_control: pressed(163),
        right_shift: pressed(161),
        right_windows: pressed(92),
    }
}

pub(super) struct Driver<T: NativeSessionTransport> {
    session: NativeSession<T>,
    hooks: Vec<HookInput>,
    redirects: Vec<RedirectKind>,
    notifications: bool,
    enabled: Option<bool>,
    events: VecDeque<Packet>,
    stream_error: Option<String>,
    motion: razer_device::mapping_native_events::NativeEventRuntime,
}

impl<T: NativeSessionTransport> Driver<T> {
    pub fn open(route: NativeRoute, transport: T) -> Result<Self> {
        let product = route.product();
        let mut session =
            NativeSession::new(route, product, NativeCapabilitySet::empty(), transport)?;
        session.open()?;
        let mut driver = Self {
            session,
            hooks: Vec::new(),
            redirects: Vec::new(),
            notifications: false,
            enabled: None,
            events: VecDeque::new(),
            stream_error: None,
            motion: Default::default(),
        };
        // AD0F0 queues these controls in this order after connection, before
        // publishing driver-ready. Retain their actual completions separately
        // from merely having accepted the persistent input read.
        let hooks = driver.set_enabled(true);
        let notifications = driver.notifications(true);
        let mut failures = Vec::new();
        if let Err(error) = hooks {
            failures.push(format!("Enable hooks: {error:#}"));
        }
        if let Err(error) = notifications {
            failures.push(format!("Enable notifications: {error:#}"));
        }
        if !failures.is_empty() {
            driver.stream_error = Some(failures.join("; "));
        }
        Ok(driver)
    }

    fn collect_input(&mut self) -> Result<()> {
        if self.session.state() != NativeSessionState::Connected {
            return Ok(());
        }
        match self.session.poll_input(Duration::ZERO)? {
            NativeInputPoll::Pending => {}
            NativeInputPoll::Completed { output, rearmed } => {
                ensure!(
                    self.events.len() < 512,
                    "Mapping driver input queue overflowed"
                );
                let packet = Packet::capture(output)?;
                let decoded = razer_device::mapping_native_events::decode_native_stream_packet(
                    &packet.bytes,
                )?;
                let (reports, issues) = self.motion.physical_motion(&mut self.session, &decoded);
                let mut errors = issues
                    .into_iter()
                    .map(|issue| format!("Physical mouse movement: {issue:?}"))
                    .collect::<Vec<_>>();
                errors.extend(reports.into_iter().filter_map(|report| {
                    report
                        .submission
                        .err()
                        .map(|error| format!("Physical mouse movement submit: {error:?}"))
                }));
                if !errors.is_empty() {
                    self.stream_error = Some(errors.join("; "));
                }
                self.events.push_back(packet);
                if let Err(error) = rearmed {
                    self.stream_error = Some(error.to_string());
                }
            }
            NativeInputPoll::Failed(error) => {
                self.stream_error = Some(error.to_string());
            }
        }
        Ok(())
    }

    pub fn poll(&mut self) -> Result<(Vec<Packet>, Option<String>)> {
        self.collect_input()?;
        self.poll_motion();
        Ok((self.events.drain(..).collect(), self.stream_error.take()))
    }

    fn poll_motion(&mut self) {
        self.poll_motion_for(Duration::ZERO);
    }
    fn poll_motion_for(&mut self, timeout: Duration) {
        let result = self.motion.poll_outputs(&mut self.session, timeout);
        let mut errors = Vec::new();
        for observation in result.observations {
            match observation.completion {
                Ok(NativeCompletion::Completed(Err(error))) => {
                    errors.push(format!("Physical mouse movement completion: {error}"))
                }
                Err(error) => errors.push(format!("Physical mouse movement observation: {error}")),
                _ => (),
            }
        }
        errors.extend(result.followup_reports.into_iter().filter_map(|report| {
            report
                .submission
                .err()
                .map(|error| format!("Physical mouse movement submit: {error:?}"))
        }));
        if !errors.is_empty() {
            self.stream_error = Some(errors.join("; "));
        }
    }

    fn control(&mut self, operation: NativeOperation) -> Result<Vec<u8>> {
        let NativeOperation::Ioctl {
            ioctl,
            input,
            output_bytes,
        } = operation
        else {
            anyhow::bail!("Persistent input read has its own owner");
        };
        let submission = self.session.submit_ioctl(ioctl, input, output_bytes)?;
        let deadline = Instant::now() + Duration::from_secs(4);
        loop {
            match self
                .session
                .poll_submission(submission.id(), Duration::from_millis(1))?
            {
                NativeCompletion::Completed(result) => return result.map_err(Into::into),
                NativeCompletion::Pending => {}
            }
            self.collect_input()?;
            self.poll_motion();
            if Instant::now() >= deadline {
                self.session.cancel_submission(submission.id())?;
                // Keep the buffers and handle until the kernel confirms final
                // completion. Cancellation request is not completion.
                loop {
                    match self
                        .session
                        .poll_submission(submission.id(), Duration::from_millis(16))?
                    {
                        NativeCompletion::Pending => self.collect_input()?,
                        NativeCompletion::Completed(result) => {
                            anyhow::bail!(
                                "Mapping control {ioctl:#x} timed out; final completion: {result:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) -> Result<()> {
        if self.enabled != Some(enabled) {
            self.enabled = None;
            self.control(hooks::enable_hooks(enabled))?;
            self.enabled = Some(enabled);
        }
        Ok(())
    }

    /// Source118360 clears the previous hooks before installing the complete
    /// replacement. Failed/partial installation remains owned for cleanup.
    pub fn replace_hooks(&mut self, hooks: Vec<HookInput>) -> Result<()> {
        let mut errors = Vec::new();
        for index in (0..self.hooks.len()).rev() {
            match self.control(self.hooks[index].remove()) {
                Ok(_) => {
                    self.hooks.remove(index);
                }
                Err(error) => errors.push(format!("Remove input hook: {error:#}")),
            }
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        for hook in hooks {
            self.hooks.push(hook);
            if let Err(error) = self.control(hook.install()) {
                errors.push(format!("Install input hook: {error:#}"));
            }
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }

    pub fn notifications(&mut self, enabled: bool) -> Result<()> {
        // Retain requested enable ownership even if the completion fails; the
        // source driver may have mutated before an error became observable.
        if enabled {
            self.notifications = true;
        }
        self.control(hooks::enable_notifications(enabled))?;
        self.notifications = enabled;
        Ok(())
    }

    pub fn redirect(&mut self, kind: RedirectKind, enabled: bool) -> Result<()> {
        if enabled && !self.redirects.contains(&kind) {
            self.redirects.push(kind);
        }
        self.control(hooks::redirect(kind, enabled))?;
        if !enabled {
            self.redirects.retain(|value| *value != kind);
        }
        Ok(())
    }

    pub fn shutdown(&mut self) -> Result<()> {
        if self.session.state() == NativeSessionState::Closed {
            return Ok(());
        }
        let mut errors = Vec::new();
        if self.session.state() == NativeSessionState::Connected {
            for kind in self.redirects.clone().into_iter().rev() {
                if let Err(error) = self.redirect(kind, false) {
                    errors.push(format!("Disable input redirect: {error:#}"));
                }
            }
            if self.notifications {
                if let Err(error) = self.notifications(false) {
                    errors.push(format!("Disable input notifications: {error:#}"));
                }
            }
            if let Err(error) = self.replace_hooks(Vec::new()) {
                errors.push(format!("Clear input hooks: {error:#}"));
            }
            if let Err(error) = self.set_enabled(false) {
                errors.push(format!("Disable hooks: {error:#}"));
            }
        }
        self.motion
            .begin_disconnect(&mut self.session)
            .context("Begin mapping driver drain")?;
        while self.motion.pending_output_count() != 0 {
            self.poll_motion_for(Duration::from_millis(16));
        }
        if let Some(error) = self.stream_error.take() {
            errors.push(error);
        }
        loop {
            match self.session.poll_disconnect(Duration::from_millis(16))? {
                NativeCompletion::Pending => {}
                NativeCompletion::Completed(result) => {
                    if let Err(error) = result {
                        errors.push(error.to_string());
                    }
                    break;
                }
            }
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
}

impl<T: NativeSessionTransport> Drop for Driver<T> {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            eprintln!("Mapping driver cleanup failed: {error:#}");
        }
    }
}

enum Control {
    Hooks(Vec<HookInput>),
    Redirect(RedirectKind, bool),
}
enum Command {
    RotationSubmit(
        RotationRequest,
        mpsc::SyncSender<Result<SubmissionId, DriverOutputError>>,
    ),
    RotationPoll(
        SubmissionId,
        Duration,
        mpsc::SyncSender<Result<NativeCompletion, DriverOutputError>>,
    ),
    Control(Control, mpsc::SyncSender<Result<(), String>>),
    Submit(
        DriverInputReport,
        mpsc::SyncSender<Result<SubmissionId, DriverOutputError>>,
    ),
    Poll(
        SubmissionId,
        Duration,
        mpsc::SyncSender<Result<CompletionPoll, DriverOutputError>>,
    ),
    Cancel(
        SubmissionId,
        mpsc::SyncSender<Result<(), DriverOutputError>>,
    ),
    Stop(mpsc::SyncSender<Result<(), String>>),
}

fn output_error(error: impl std::fmt::Display) -> DriverOutputError {
    DriverOutputError::Owner(error.to_string())
}

#[derive(Debug, Default)]
struct Lifetime {
    drained: Mutex<bool>,
    completed: Condvar,
}
impl Lifetime {
    fn finish(&self) {
        *self
            .drained
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = true;
        self.completed.notify_all();
    }
    fn wait(&self) {
        let mut drained = self
            .drained
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        while !*drained {
            drained = self
                .completed
                .wait(drained)
                .unwrap_or_else(|error| error.into_inner());
        }
    }
}
struct FinishLifetime(Arc<Lifetime>);
impl Drop for FinishLifetime {
    fn drop(&mut self) {
        self.0.finish();
    }
}

/// Cloneable route to the one thread that owns this connected driver handle.
/// Each execution leases a transport with its own pending-output ledger.
#[derive(Clone, Debug)]
pub(crate) struct Route {
    path: String,
    sender: mpsc::Sender<Command>,
    lifetime: Arc<Lifetime>,
}
impl Route {
    #[cfg(test)]
    pub(super) fn disconnected_for_test() -> Self {
        let (sender, _) = mpsc::channel();
        let lifetime = Arc::new(Lifetime::default());
        lifetime.finish();
        Self {
            path: "mock-driver-route".into(),
            sender,
            lifetime,
        }
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    fn control(&self, control: Control) -> Result<()> {
        let (reply, receive) = mpsc::sync_channel(1);
        self.sender
            .send(Command::Control(control, reply))
            .with_context(|| format!("Mapping driver {} stopped", self.path()))?;
        receive
            .recv()
            .context("Mapping driver control reply disconnected")?
            .map_err(anyhow::Error::msg)
    }
    pub fn replace_hooks(&self, hooks: Vec<HookInput>) -> Result<()> {
        self.control(Control::Hooks(hooks))
    }
    pub fn output(&self) -> Output {
        Output {
            sender: self.sender.clone(),
            pending: BTreeSet::new(),
            closed: false,
        }
    }
    pub fn rotation(&self) -> Rotation {
        Rotation::new(self.sender.clone(), self.lifetime.clone())
    }
}

pub(crate) struct Output {
    sender: mpsc::Sender<Command>,
    pending: BTreeSet<SubmissionId>,
    closed: bool,
}
impl MappingOutputTransport for Output {
    fn submit(&mut self, report: DriverInputReport) -> Result<SubmissionId, DriverOutputError> {
        if self.closed {
            return Err(DriverOutputError::Closed);
        }
        let (reply, receive) = mpsc::sync_channel(1);
        self.sender
            .send(Command::Submit(report, reply))
            .map_err(output_error)?;
        let id = receive.recv().map_err(output_error)??;
        self.pending.insert(id);
        Ok(id)
    }
    fn poll_completion(
        &mut self,
        id: SubmissionId,
        wait: Duration,
    ) -> Result<CompletionPoll, DriverOutputError> {
        if !self.pending.contains(&id) {
            return Err(DriverOutputError::UnknownSubmission(id));
        }
        let (reply, receive) = mpsc::sync_channel(1);
        self.sender
            .send(Command::Poll(id, wait, reply))
            .map_err(output_error)?;
        let result = receive.recv().map_err(output_error)??;
        if matches!(result, CompletionPoll::Completed(_)) {
            self.pending.remove(&id);
        }
        Ok(result)
    }
    fn cancel(&mut self, id: SubmissionId) -> Result<(), DriverOutputError> {
        if !self.pending.contains(&id) {
            return Err(DriverOutputError::UnknownSubmission(id));
        }
        let (reply, receive) = mpsc::sync_channel(1);
        self.sender
            .send(Command::Cancel(id, reply))
            .map_err(output_error)?;
        receive.recv().map_err(output_error)?
    }
    fn close(&mut self) -> Result<(), DriverOutputError> {
        if !self.pending.is_empty() {
            return Err(DriverOutputError::ExecutionPending);
        }
        self.closed = true;
        Ok(())
    }
}
impl Drop for Output {
    fn drop(&mut self) {
        let ids = self.pending.iter().copied().collect::<Vec<_>>();
        for id in ids {
            if let Err(error) = self.cancel(id) {
                eprintln!("Cancel mapping output: {error}");
            }
            loop {
                match self.poll_completion(id, Duration::from_millis(16)) {
                    Ok(CompletionPoll::Pending) => {}
                    Ok(CompletionPoll::Completed(_)) => break,
                    Err(error) => {
                        eprintln!("Drain mapping output: {error}");
                        break;
                    }
                }
            }
        }
    }
}

pub(crate) struct Owner {
    route: Route,
    input: mpsc::Receiver<Result<Packet, String>>,
    overflow: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl Owner {
    pub fn open(route: NativeRoute) -> Result<Self> {
        Self::open_with(
            route,
            super::super::mapping_native_session::WindowsMappingNativeSessionTransport::default,
        )
    }

    fn open_with<T: NativeSessionTransport + 'static>(
        route: NativeRoute,
        create: impl FnOnce() -> T + Send + 'static,
    ) -> Result<Self> {
        let path = route.path().as_str().to_owned();
        let (sender, commands) = mpsc::channel();
        let (input_sender, input) = mpsc::sync_channel(512);
        let (ready, initialized) = mpsc::sync_channel(1);
        let overflow = Arc::new(AtomicBool::new(false));
        let worker_overflow = overflow.clone();
        let lifetime = Arc::new(Lifetime::default());
        let worker_lifetime = lifetime.clone();
        let worker = thread::Builder::new()
            .name("razer-mapping-driver".into())
            .spawn(move || {
                // Declared before Driver: even unwinding drops its transport,
                // retained kernel requests and handle before publishing this.
                let _finished = FinishLifetime(worker_lifetime);
                let driver = Driver::open(route, create());
                match driver {
                    Ok(mut driver) => {
                        let _ = ready.send(Ok(()));
                        loop {
                            match commands.recv_timeout(Duration::from_millis(1)) {
                                Ok(Command::RotationSubmit(request, reply)) => {
                                    let result = driver
                                        .session
                                        .submit_ioctl(
                                            request.ioctl(),
                                            request
                                                .input()
                                                .map_or_else(Vec::new, |bytes| bytes.to_vec()),
                                            request.output_size() as usize,
                                        )
                                        .map(|submission| SubmissionId(submission.id().0))
                                        .map_err(output_error);
                                    let _ = reply.send(result);
                                }
                                Ok(Command::RotationPoll(id, wait, reply)) => {
                                    let _ = reply.send(
                                        driver
                                            .session
                                            .poll_submission(NativeSubmissionId(id.0), wait)
                                            .map_err(output_error),
                                    );
                                }
                                Ok(Command::Control(control, reply)) => {
                                    let result = match control {
                                        Control::Hooks(hooks) => driver.replace_hooks(hooks),
                                        Control::Redirect(kind, enabled) => {
                                            driver.redirect(kind, enabled)
                                        }
                                    };
                                    let _ =
                                        reply.send(result.map_err(|error| format!("{error:#}")));
                                }
                                Ok(Command::Submit(report, reply)) => {
                                    let result = driver
                                        .session
                                        .submit_input_report(report)
                                        .map(|submission| SubmissionId(submission.id().0))
                                        .map_err(output_error);
                                    let _ = reply.send(result);
                                }
                                Ok(Command::Poll(id, wait, reply)) => {
                                    let result = driver
                                        .session
                                        .poll_submission(NativeSubmissionId(id.0), wait)
                                        .map(|poll| match poll {
                                            NativeCompletion::Pending => CompletionPoll::Pending,
                                            NativeCompletion::Completed(result) => {
                                                CompletionPoll::Completed(
                                                    result.map(|_| ()).map_err(output_error),
                                                )
                                            }
                                        })
                                        .map_err(output_error);
                                    let _ = reply.send(result);
                                }
                                Ok(Command::Cancel(id, reply)) => {
                                    let _ = reply.send(
                                        driver
                                            .session
                                            .cancel_submission(NativeSubmissionId(id.0))
                                            .map_err(output_error),
                                    );
                                }
                                Ok(Command::Stop(reply)) => {
                                    let _ = reply.send(
                                        driver.shutdown().map_err(|error| format!("{error:#}")),
                                    );
                                    break;
                                }
                                Err(mpsc::RecvTimeoutError::Timeout) => {}
                                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                            }
                            let (events, error) = match driver.poll() {
                                Ok(result) => result,
                                Err(error) => (Vec::new(), Some(format!("{error:#}"))),
                            };
                            for event in events.into_iter().map(Ok).chain(error.map(Err)) {
                                if input_sender.try_send(event).is_err() {
                                    worker_overflow.store(true, Ordering::Release);
                                }
                            }
                        }
                    }
                    Err(error) => {
                        let _ = ready.send(Err(format!("{error:#}")));
                    }
                }
            })?;
        match initialized.recv() {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                let _ = worker.join();
                anyhow::bail!(error);
            }
            Err(error) => {
                let joined = worker.join();
                anyhow::bail!("Mapping driver startup disconnected: {error}; worker: {joined:?}");
            }
        }
        Ok(Self {
            route: Route {
                path,
                sender,
                lifetime,
            },
            input,
            overflow,
            worker: Some(worker),
        })
    }

    pub fn route(&self) -> Route {
        self.route.clone()
    }
    pub fn inputs(&mut self) -> Result<Vec<Result<Packet, String>>> {
        ensure!(
            !self.overflow.load(Ordering::Acquire),
            "Mapping driver input receiver overflowed"
        );
        Ok(self.input.try_iter().collect())
    }
    fn control(&self, control: Control) -> Result<()> {
        let (reply, receive) = mpsc::sync_channel(1);
        self.route
            .sender
            .send(Command::Control(control, reply))
            .context("Mapping driver stopped")?;
        receive
            .recv()
            .context("Mapping driver control reply disconnected")?
            .map_err(anyhow::Error::msg)
    }
    pub fn redirect(&self, kind: RedirectKind, enabled: bool) -> Result<()> {
        self.control(Control::Redirect(kind, enabled))
    }
    pub fn shutdown(&mut self) -> Result<()> {
        let Some(worker) = self.worker.take() else {
            return Ok(());
        };
        let (reply, receive) = mpsc::sync_channel(1);
        let result = self
            .route
            .sender
            .send(Command::Stop(reply))
            .context("Mapping driver owner disconnected")
            .and_then(|_| {
                receive
                    .recv()
                    .context("Mapping driver cleanup reply disconnected")
            })
            .and_then(|result| result.map_err(anyhow::Error::msg));
        let joined = worker
            .join()
            .map_err(|_| anyhow::anyhow!("Mapping driver worker panicked"));
        match (result, joined) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
            (Err(error), Err(join)) => Err(anyhow::anyhow!("{error:#}; {join:#}")),
        }
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            eprintln!("{error:#}");
        }
    }
}

#[cfg(test)]
#[path = "driver_tests.rs"]
mod tests;
