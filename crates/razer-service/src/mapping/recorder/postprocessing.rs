//! Nr reads the current editor before decoding; h.nu acknowledges its append
//! before the recording machine resumes mappings and global shortcuts.
use super::*;

pub(super) struct Geometry {
    pub before: Result<razer_platform::macro_geometry::Snapshot, String>,
    pub after: Result<razer_platform::macro_geometry::Snapshot, String>,
}

impl Geometry {
    fn recorded(self, movement_mode: u8) -> Result<Option<RecordingGeometry>, String> {
        let (before, after) = match (self.before, self.after) {
            (Ok(before), Ok(after)) => (before, after),
            _ if movement_mode == 0 => return Ok(None),
            (Err(before), Err(after)) => return Err(format!("{before}; {after}")),
            (Err(error), Ok(_)) | (Ok(_), Err(error)) => return Err(error),
        };
        // Nr compares these four top-level source properties. Some native
        // screen envelopes only carry MonitorRect; absent properties remain
        // unobserved here instead of being filled from that separate object.
        #[derive(serde::Deserialize, PartialEq)]
        struct ComparedScreen {
            top: Option<serde_json::Number>,
            left: Option<serde_json::Number>,
            width: Option<serde_json::Number>,
            height: Option<serde_json::Number>,
        }
        if movement_mode != 0 {
            let first: ComparedScreen = serde_json::from_value(before.screen.clone())
                .map_err(|error| format!("Invalid first monitor observation: {error}"))?;
            let second: ComparedScreen = serde_json::from_value(after.screen.clone())
                .map_err(|error| format!("Invalid second monitor observation: {error}"))?;
            if first != second {
                return Err("Monitor setting has been changed".into());
            }
        }
        Ok(Some(RecordingGeometry {
            monitors_before: before.monitors,
            monitors_after: after.monitors,
            screen_before: before.screen,
            screen_after: after.screen,
        }))
    }
}

pub struct PostProcessingRequest {
    reply: mpsc::Sender<Result<Options, String>>,
}

impl PostProcessingRequest {
    pub fn complete(self, options: Result<Options, String>) -> Result<(), String> {
        self.reply
            .send(options)
            .map_err(|_| "录制后处理会话已关闭。".into())
    }
}

pub struct CommitRequest {
    rows: Vec<ActionItem>,
    reply: mpsc::Sender<Result<CommitOutcome, String>>,
    cancelled: Arc<AtomicBool>,
    gate: Arc<CommitGate>,
}

enum CommitOutcome {
    Cancelled,
    Applied(usize),
}

#[derive(Default)]
struct CommitGate(std::sync::atomic::AtomicU8);

impl CommitGate {
    const WAITING: u8 = 0;
    const APPLYING: u8 = 1;
    const CANCELLED: u8 = 2;

    fn begin(&self) -> bool {
        self.0
            .compare_exchange(
                Self::WAITING,
                Self::APPLYING,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    fn cancel_pending(&self) -> bool {
        match self.0.compare_exchange(
            Self::WAITING,
            Self::CANCELLED,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => true,
            Err(state) => state == Self::CANCELLED,
        }
    }
}

impl CommitRequest {
    pub fn complete(
        self,
        apply: impl FnOnce(Vec<ActionItem>) -> Result<usize, String>,
    ) -> Result<(), String> {
        if self.cancelled.load(Ordering::Acquire) || !self.gate.begin() {
            self.gate.cancel_pending();
            return self
                .reply
                .send(Ok(CommitOutcome::Cancelled))
                .map_err(|_| "录制提交确认会话已关闭。".into());
        }
        if self.cancelled.load(Ordering::Acquire) {
            return self
                .reply
                .send(Ok(CommitOutcome::Cancelled))
                .map_err(|_| "录制提交确认会话已关闭。".into());
        }
        self.reply
            .send(apply(self.rows).map(CommitOutcome::Applied))
            .map_err(|_| "录制提交确认会话已关闭。".into())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Completion {
    Cancelled,
    Recorded { count: usize },
    CancelledAfterCommit { count: usize },
}

fn wait_for_reply<T>(
    reply: &mpsc::Receiver<Result<T, String>>,
    input: &mpsc::Receiver<Command>,
    cancelled: &Arc<AtomicBool>,
    commit: Option<&CommitGate>,
) -> Result<Option<T>, String> {
    loop {
        if commit.is_some() {
            match reply.try_recv() {
                Ok(result) => return result.map(Some),
                Err(mpsc::TryRecvError::Disconnected) => {
                    if cancelled.load(Ordering::Acquire)
                        && commit.is_none_or(CommitGate::cancel_pending)
                    {
                        return Ok(None);
                    }
                    return Err("录制页面未完成提交确认。".into());
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let mut cancellation_requested = cancelled.load(Ordering::Acquire);
        loop {
            match input.try_recv() {
                Ok(Command::Cancel) | Err(mpsc::TryRecvError::Disconnected) => {
                    cancellation_requested = true;
                    break;
                }
                Ok(Command::Stop | Command::StopShortcut | Command::PreviewOptions(_)) => {}
                Err(mpsc::TryRecvError::Empty) => break,
            }
        }
        if cancellation_requested && commit.is_none_or(CommitGate::cancel_pending) {
            return Ok(None);
        }
        match reply.recv_timeout(Duration::from_millis(25)) {
            Ok(_) if commit.is_none() && cancelled.load(Ordering::Acquire) => return Ok(None),
            Ok(result) => return result.map(Some),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if cancelled.load(Ordering::Acquire)
                    && commit.is_none_or(CommitGate::cancel_pending)
                {
                    return Ok(None);
                }
                return Err("录制页面未完成后处理或提交确认。".into());
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

pub(super) fn process(
    capture: razer_model::macro_library::recording::Capture,
    geometry: Geometry,
    initial_options: &Options,
    stop: Option<StopCause>,
    input: &mpsc::Receiver<Command>,
    output: &async_channel::Sender<Update>,
    cancelled: &Arc<AtomicBool>,
) -> Result<Completion, String> {
    let (reply, pending) = mpsc::channel();
    if output
        .send_blocking(Update::PostProcessing(PostProcessingRequest { reply }))
        .is_err()
    {
        return Ok(Completion::Cancelled);
    }
    let Some(mut options) = wait_for_reply(&pending, input, cancelled, None)? else {
        return Ok(Completion::Cancelled);
    };
    // _r and Nr use the recording machine's shortcut, carried by its payload.
    options.shortcut = initial_options.shortcut.clone();
    let geometry = geometry.recorded(options.movement_mode)?;
    let Some(rows) = capture.process_cancelable(&options, geometry, stop, || {
        cancelled.load(Ordering::Acquire)
    })?
    else {
        return Ok(Completion::Cancelled);
    };
    if cancelled.load(Ordering::Acquire) {
        return Ok(Completion::Cancelled);
    }
    let (reply, pending) = mpsc::channel();
    let gate = Arc::new(CommitGate::default());
    if output
        .send_blocking(Update::Commit(CommitRequest {
            rows,
            reply,
            cancelled: cancelled.clone(),
            gate: gate.clone(),
        }))
        .is_err()
    {
        return Ok(Completion::Cancelled);
    }
    Ok(
        match wait_for_reply(&pending, input, cancelled, Some(&gate))? {
            Some(CommitOutcome::Applied(count)) if cancelled.load(Ordering::Acquire) => {
                Completion::CancelledAfterCommit { count }
            }
            Some(CommitOutcome::Applied(count)) => Completion::Recorded { count },
            None | Some(CommitOutcome::Cancelled) => Completion::Cancelled,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_after_apply_begins_waits_for_its_acknowledgement() {
        let gate = Arc::new(CommitGate::default());
        assert!(gate.begin());
        let (reply, pending) = mpsc::channel();
        let (commands, input) = mpsc::channel();
        let (completed, wait_completed) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(true));
        commands.send(Command::Cancel).unwrap();
        let worker = std::thread::spawn(move || {
            let result = wait_for_reply(&pending, &input, &cancelled, Some(&gate));
            completed.send(result).unwrap();
        });
        assert!(matches!(
            wait_completed.recv_timeout(Duration::from_millis(75)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        reply.send(Ok(7usize)).unwrap();
        assert_eq!(
            wait_completed.recv_timeout(Duration::from_secs(2)).unwrap(),
            Ok(Some(7))
        );
        worker.join().unwrap();
    }
}
