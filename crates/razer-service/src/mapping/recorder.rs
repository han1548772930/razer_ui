//! The recorder task retains its shared mapping scope through final cleanup.
//! UI cancellation sends a command or drops its sender. No native work runs on GPUI.
use super::{Transport, WorkerGuard, guarded, shutdown};
#[cfg(test)]
use razer_ipc::KEY_CAPTURE_LIFECYCLE as LIFECYCLE;
use razer_ipc::ServiceRequest;
use razer_ipc::macro_recording::{RestoreReceipt, SuspensionReceipt};
use razer_ipc::mapping_control::{Registry, Suppression, Suspension};
use razer_model::macro_library::{
    ActionItem, RecordingGeometry,
    recording::{Callback, Capture, Decoder, Options, StopCause},
};

mod events;
mod postprocessing;
pub use postprocessing::{CommitRequest, Completion, PostProcessingRequest};
use serde_json::Value;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

pub enum Command {
    Stop,
    StopShortcut,
    Cancel,
    PreviewOptions(Options),
}
#[derive(Clone, Copy)]
pub enum StorageChange {
    SoftwareExclusive,
    Remove,
}

pub enum Update {
    WindowStorage(StorageChange),
    GlobalShortcutsRestored,
    Recording,
    Progress(usize),
    Preview {
        first_row: usize,
        rows: Vec<ActionItem>,
    },
    PreviewFailed(String),
    ObservationFailed(String),
    PostProcessing(PostProcessingRequest),
    Commit(CommitRequest),
    Finished(Result<Completion, String>),
}
pub struct Session {
    commands: mpsc::Sender<Command>,
    cancelled: Arc<AtomicBool>,
}

impl Session {
    pub fn send(&self, command: Command) -> Result<(), String> {
        if matches!(command, Command::Cancel) {
            self.cancelled.store(true, Ordering::Release);
        }
        self.commands
            .send(command)
            .map_err(|_| "宏录制会话已关闭。".into())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.send(Command::Cancel);
    }
}

pub fn spawn(options: Options) -> Result<(Session, async_channel::Receiver<Update>), String> {
    spawn_with_registry(options, super::recorder_owner::spawn, Registry::global())
}

/// Queries the retained recorder's real input owner from a background worker.
/// Mapping workers have their own process and must not query a local recorder.
pub fn mode() -> anyhow::Result<razer_platform::macro_recorder::Mode> {
    super::recorder_owner::mode()
}

#[cfg(test)]
fn spawn_with<T: Transport + 'static>(
    options: Options,
    factory: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
) -> Result<(Session, async_channel::Receiver<Update>), String> {
    spawn_with_geometry(options, factory, Registry::default(), || {
        Ok(razer_platform::macro_geometry::Snapshot {
            monitors: serde_json::json!([]),
            screen: serde_json::json!({}),
        })
    })
}

pub(super) fn spawn_with_registry<T: Transport + 'static>(
    options: Options,
    factory: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
    registry: Registry,
) -> Result<(Session, async_channel::Receiver<Update>), String> {
    spawn_with_geometry(options, factory, registry, || {
        razer_platform::macro_geometry::snapshot().map_err(|error| format!("{error:#}"))
    })
}

pub(super) fn spawn_with_geometry<T: Transport + 'static>(
    options: Options,
    factory: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
    registry: Registry,
    mut geometry: impl FnMut() -> Result<razer_platform::macro_geometry::Snapshot, String>
    + Send
    + 'static,
) -> Result<(Session, async_channel::Receiver<Update>), String> {
    let (commands, input) = mpsc::channel();
    let (output, updates) = async_channel::unbounded();
    let cancelled = Arc::new(AtomicBool::new(false));
    let cancellation = cancelled.clone();
    let worker = WorkerGuard::start();
    std::thread::Builder::new()
        .name("macro-recording-session".into())
        .spawn(move || {
            let _worker = worker;
            // Recorder, focused capture and calibration share native input
            // suppression. Keep the lease until restoration and client Drop.
            let _lifecycle = match super::lock_lifecycle(&cancellation) {
                Ok(guard) => guard,
                Err(error) => {
                    let result = if cancellation.load(Ordering::Acquire) {
                        Ok(Completion::Cancelled)
                    } else {
                        Err(format!("{error:#}"))
                    };
                    let _ = output.send_blocking(Update::Finished(result));
                    return;
                }
            };
            if cancellation.load(Ordering::Acquire) {
                let _ = output.send_blocking(Update::Finished(Ok(Completion::Cancelled)));
                return;
            }
            let result = guarded(factory)
                .map_err(|error| format!("{error:#}"))
                .and_then(|mut client| {
                    let mut suppression = None;
                    let result = guarded(|| {
                        record(
                            &mut client,
                            &input,
                            &output,
                            &options,
                            &registry,
                            &mut suppression,
                            &cancellation,
                            &mut geometry,
                        )
                        .map_err(anyhow::Error::msg)
                    })
                    .map_err(|error| format!("{error:#}"));
                    // Native shutdown retries recorder stop and mapping restoration.
                    // Keep this destructor on the actor even after the view is dropped.
                    let shutdown = shutdown(&mut client);
                    drop(client);
                    let restoration = suppression
                        .as_mut()
                        .map(Suspension::release)
                        .unwrap_or(Ok(()));
                    let shutdown = match (shutdown, restoration) {
                        (Ok(()), result) => result,
                        (Err(error), Ok(())) => Err(error),
                        (Err(error), Err(restoration)) => {
                            Err(format!("{error}; mapping owner restoration: {restoration}"))
                        }
                    };
                    match (result, shutdown) {
                        (Ok(value), Ok(_)) => Ok(value),
                        (Err(error), Ok(_)) => Err(error),
                        (Ok(_), Err(error)) => Err(format!("录制会话关闭失败：{error}")),
                        (Err(error), Err(cleanup)) => {
                            Err(format!("{error}；会话关闭失败：{cleanup}"))
                        }
                    }
                });
            let result = match result {
                Ok(Completion::Recorded { count }) if cancellation.load(Ordering::Acquire) => {
                    Ok(Completion::CancelledAfterCommit { count })
                }
                result => result,
            };
            let _ = output.send_blocking(Update::Finished(result));
        })
        .map_err(|e| format!("无法创建录制线程：{e}"))?;
    Ok((
        Session {
            commands,
            cancelled,
        },
        updates,
    ))
}
fn query(client: &mut impl Transport, request: ServiceRequest) -> Result<Value, String> {
    client.request(request).map_err(|e| format!("{e:#}"))
}

fn accepted(value: Value) -> Result<(), String> {
    #[derive(serde::Deserialize)]
    struct Receipt {
        accepted: bool,
    }
    let receipt: Receipt =
        serde_json::from_value(value).map_err(|error| format!("录制响应格式错误：{error}"))?;
    if !receipt.accepted {
        return Err("录制操作未被服务接受。".into());
    }
    Ok(())
}
fn record(
    client: &mut impl Transport,
    input: &mpsc::Receiver<Command>,
    output: &async_channel::Sender<Update>,
    options: &Options,
    registry: &Registry,
    suppression: &mut Option<Suspension>,
    cancellation: &Arc<AtomicBool>,
    geometry: &mut impl FnMut() -> Result<razer_platform::macro_geometry::Snapshot, String>,
) -> Result<Completion, String> {
    let mut stop_cause = StopCause::Button;
    let mut preview = Decoder::for_preview(options);
    let mut stop_requested = match input.try_recv() {
        Ok(Command::Cancel) | Err(mpsc::TryRecvError::Disconnected) => {
            return Ok(Completion::Cancelled);
        }
        Ok(Command::Stop) => true,
        Ok(Command::StopShortcut) => {
            stop_cause = StopCause::Shortcut;
            true
        }
        Err(mpsc::TryRecvError::Empty) => false,
        Ok(Command::PreviewOptions(options)) => {
            preview.update_preview_options(&options);
            false
        }
    };
    accepted(query(client, ServiceRequest::StartMacroRecording)?)?;
    let mut deadline = Some(Instant::now() + Duration::from_secs(12));
    let mut started = false;
    let mut stopping = false;
    let mut cancelled = false;
    let mut capture = Capture::default();
    let mut before = None;
    let mut preview_at = Instant::now();
    let mut preview_dirty = false;
    let mut item_count = 0usize;
    let mut bytes = 0usize;
    loop {
        drain_commands(
            input,
            &mut stop_requested,
            &mut stop_cause,
            &mut cancelled,
            &mut preview,
        );
        if started && stop_requested && !stopping {
            // stoppingRecorder invokes removeWindowStorageItem before dr();
            // it does not wait for the host-storage operation's Promise.
            let _ = output.send_blocking(Update::WindowStorage(StorageChange::Remove));
            accepted(query(client, ServiceRequest::StopMacroRecording)?)?;
            stopping = true;
            deadline = Some(Instant::now() + Duration::from_secs(12));
        }
        let updates = events::parse(query(client, ServiceRequest::MacroRecordingEvents)?)?;
        if let Some(error) = updates.iter().find_map(|event| match event {
            events::Event::Failed(error) => Some(error),
            _ => None,
        }) {
            return Err(error.clone());
        }
        let already_stopped = updates.iter().any(|event| {
            matches!(
                event,
                events::Event::Stopped {
                    mode: razer_platform::macro_recorder::Mode::Software
                }
            )
        });
        let update_count = updates.len();
        for event in updates {
            match event {
                events::Event::Started { mode, callback } => {
                    if started || mode != razer_platform::macro_recorder::Mode::Software {
                        return Err("收到不匹配的录制 started 事件".into());
                    }
                    started = true;
                    preview.push_callback(&callback)?;
                    capture.push(callback);
                    // disablingMapping starts only after native RECORDING.
                    // A button press / startingRecorder alone holds no lock.
                    let _ = output
                        .send_blocking(Update::WindowStorage(StorageChange::SoftwareExclusive));
                    if !already_stopped {
                        *suppression = Some(
                            registry
                                .suspend_cancelable(Suppression::SoftwareRecording, cancellation)?,
                        );
                        let receipt: SuspensionReceipt = serde_json::from_value(query(
                            client,
                            ServiceRequest::SuspendMacroMappings,
                        )?)
                        .map_err(|error| format!("录制映射暂停响应格式错误：{error}"))?;
                        if !receipt.suspended() {
                            return Err("录制映射暂停未被服务确认。".into());
                        }
                    }
                    let observed = geometry().map_err(|error| {
                        format!("Monitor observation before recording failed: {error}")
                    });
                    match &observed {
                        Ok(observed) => preview.set_geometry(RecordingGeometry {
                            monitors_before: observed.monitors.clone(),
                            monitors_after: observed.monitors.clone(),
                            screen_before: observed.screen.clone(),
                            screen_after: observed.screen.clone(),
                        }),
                        Err(error) => {
                            let _ = output.send_blocking(Update::ObservationFailed(error.clone()));
                        }
                    }
                    before = Some(observed);
                    deadline = None;
                    if !already_stopped {
                        let _ = output.send_blocking(Update::Recording);
                    }
                }
                events::Event::Items {
                    bytes: event_bytes,
                    callback,
                } => {
                    bytes = bytes.saturating_add(event_bytes);
                    if bytes > 32 * 1024 * 1024 {
                        return Err("录制原始事件超过 32 MiB，未提交不完整录制".into());
                    }
                    if !started {
                        return Err("原生录制在 started 前返回输入事件".into());
                    }
                    item_count = item_count
                        .checked_add(callback.input_count())
                        .ok_or("录制事件数量溢出")?;
                    if item_count > 100_000 {
                        return Err("录制超过 100000 个输入事件，未提交不完整录制".into());
                    }
                    // Current ON_MACRO_ITEM's first transition is unconditional
                    // recordingHandler. Its later ESC abort guard is unreachable;
                    // preserve the key instead of inventing an abort transition.
                    if let Err(error) = preview.push_callback(&callback) {
                        let _ = output.send_blocking(Update::PreviewFailed(error));
                    }
                    capture.push(callback);
                    preview_dirty = true;
                }
                events::Event::Stopped { mode } => {
                    if !started || mode != razer_platform::macro_recorder::Mode::Software {
                        return Err("收到不匹配的录制 stopped 事件".into());
                    }
                    // Cancel may have arrived while this blocking event
                    // request was in flight, including behind a queued Stop.
                    drain_commands(
                        input,
                        &mut stop_requested,
                        &mut stop_cause,
                        &mut cancelled,
                        &mut preview,
                    );
                    let result = if cancelled {
                        Ok(Completion::Cancelled)
                    } else {
                        let before = before.take().ok_or("录制前屏幕状态未观测")?;
                        let after = geometry().map_err(|error| {
                            format!("Monitor observation after recording failed: {error}")
                        });
                        if let Err(error) = &after {
                            let _ = output.send_blocking(Update::ObservationFailed(error.clone()));
                        }
                        postprocessing::process(
                            capture,
                            postprocessing::Geometry { before, after },
                            options,
                            stop_requested.then_some(stop_cause),
                            input,
                            output,
                            cancellation,
                        )
                    };
                    let restoration = (|| {
                        let restored: RestoreReceipt = serde_json::from_value(query(
                            client,
                            ServiceRequest::ResumeMacroMappings,
                        )?)
                        .map_err(|error| format!("录制映射恢复响应格式错误：{error}"))?;
                        if let Some(suppression) = suppression.as_mut() {
                            suppression.release()?;
                        }
                        if restored.global_shortcuts().result() {
                            let _ = output.send_blocking(Update::GlobalShortcutsRestored);
                        }
                        Ok::<_, String>(())
                    })();
                    return match (result, restoration) {
                        (Ok(result), Ok(())) => Ok(result),
                        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
                        (Err(error), Err(restore)) => {
                            Err(format!("{error}；录制恢复失败：{restore}"))
                        }
                    };
                }
                events::Event::Failed(error) => return Err(error),
            }
        }
        if update_count != 0 {
            let _ = output.send_blocking(Update::Progress(item_count));
        }
        if preview_dirty && preview_at.elapsed() >= Duration::from_millis(50) && preview.len() > 0 {
            let _ = output.send_blocking(Update::Preview {
                first_row: preview.dropped(),
                rows: preview.preview_since(0),
            });
            preview_at = Instant::now();
            preview_dirty = false;
        }
        if deadline.is_some_and(|limit| Instant::now() >= limit) {
            return Err(if started {
                "停止录制未收到 stopped 事件"
            } else {
                "开始录制未收到 started 事件"
            }
            .into());
        }
        if update_count < 512 {
            match input.recv_timeout(Duration::from_millis(25)) {
                Ok(Command::Stop) => stop_requested = true,
                Ok(Command::StopShortcut) => {
                    stop_requested = true;
                    stop_cause = StopCause::Shortcut;
                }
                Ok(Command::Cancel) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                    stop_requested = true;
                    cancelled = true;
                }
                Ok(Command::PreviewOptions(options)) => preview.update_preview_options(&options),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
    }
}

#[cfg(test)]
mod tests;

fn drain_commands(
    input: &mpsc::Receiver<Command>,
    stop_requested: &mut bool,
    stop_cause: &mut StopCause,
    cancelled: &mut bool,
    preview: &mut Decoder,
) {
    loop {
        match input.try_recv() {
            Ok(Command::Stop) => *stop_requested = true,
            Ok(Command::StopShortcut) => {
                *stop_requested = true;
                *stop_cause = StopCause::Shortcut;
            }
            Ok(Command::Cancel) => {
                *stop_requested = true;
                *cancelled = true;
            }
            Ok(Command::PreviewOptions(options)) => preview.update_preview_options(&options),
            Err(mpsc::TryRecvError::Disconnected) => {
                *stop_requested = true;
                *cancelled = true;
                return;
            }
            Err(mpsc::TryRecvError::Empty) => return,
        }
    }
}
