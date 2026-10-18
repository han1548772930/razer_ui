//! The dedicated actor owns ServiceClient, its pipe, DLL worker and shutdown.
//! UI cancellation sends a command or drops its sender. No native work runs on GPUI.
use super::{Transport, WorkerGuard, guarded, shutdown};
#[cfg(test)]
use razer_ipc::KEY_CAPTURE_LIFECYCLE as LIFECYCLE;
use razer_ipc::ServiceRequest;
use razer_ipc::macro_recording::{RestoreReceipt, SuspensionReceipt};
use razer_ipc::mapping_control::{Registry, Suppression, Suspension};
use razer_model::macro_library::{
    ActionItem, RecordingGeometry,
    recording::{Decoder, Options},
};
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
    Cancel,
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
    Finished(Result<Option<Vec<ActionItem>>, String>),
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
    spawn_with_registry(options, factory, Registry::default())
}

pub(super) fn spawn_with_registry<T: Transport + 'static>(
    options: Options,
    factory: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
    registry: Registry,
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
                        Ok(None)
                    } else {
                        Err(format!("{error:#}"))
                    };
                    let _ = output.send_blocking(Update::Finished(result));
                    return;
                }
            };
            if cancellation.load(Ordering::Acquire) {
                let _ = output.send_blocking(Update::Finished(Ok(None)));
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
                Ok(_) if cancellation.load(Ordering::Acquire) => Ok(None),
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
    cancellation: &AtomicBool,
) -> Result<Option<Vec<ActionItem>>, String> {
    let mut stop_requested = match input.try_recv() {
        Ok(Command::Cancel) | Err(mpsc::TryRecvError::Disconnected) => return Ok(None),
        Ok(Command::Stop) => true,
        Err(mpsc::TryRecvError::Empty) => false,
    };
    let before = (options.movement_mode != 0)
        .then(razer_platform::macro_geometry::snapshot)
        .transpose()
        .map_err(|error| format!("Monitor observation before recording failed: {error:#}"))?;
    accepted(query(client, ServiceRequest::StartMacroRecording)?)?;
    let mut deadline = Some(Instant::now() + Duration::from_secs(12));
    let mut started = false;
    let mut stopping = false;
    let mut cancelled = false;
    let mut decoder = Decoder::new(options);
    let mut preview = Decoder::for_preview(options);
    if let Some(before) = &before {
        let geometry = RecordingGeometry {
            monitors_before: before.monitors.clone(),
            monitors_after: before.monitors.clone(),
            screen_before: before.screen.clone(),
            screen_after: before.screen.clone(),
        };
        decoder.set_geometry(geometry.clone());
        preview.set_geometry(geometry);
    }
    let mut preview_at = Instant::now();
    let mut preview_dirty = false;
    let mut item_count = 0usize;
    let mut bytes = 0usize;
    loop {
        drain_commands(input, &mut stop_requested, &mut cancelled);
        if started && stop_requested && !stopping {
            // stoppingRecorder invokes removeWindowStorageItem before dr();
            // it does not wait for the host-storage operation's Promise.
            let _ = output.send_blocking(Update::WindowStorage(StorageChange::Remove));
            accepted(query(client, ServiceRequest::StopMacroRecording)?)?;
            stopping = true;
            deadline = Some(Instant::now() + Duration::from_secs(12));
        }
        let batch = query(client, ServiceRequest::MacroRecordingEvents)?;
        if batch["overflow"].as_bool() != Some(false) {
            return Err("原生录制队列溢出，未提交不完整录制".into());
        }
        let updates = batch["events"]
            .as_array()
            .ok_or("原生录制事件响应格式错误")?;
        if let Some(event) = updates.iter().find(|event| event["kind"] == "error") {
            return Err(event["error"].as_str().unwrap_or("原生录制回调错误").into());
        }
        let already_stopped = updates
            .iter()
            .any(|event| event["kind"] == "stopped" && event["mode"] == "kSoftware");
        for event in updates {
            match event["kind"].as_str() {
                Some("started") => {
                    if started || event["mode"] != "kSoftware" {
                        return Err("收到不匹配的录制 started 事件".into());
                    }
                    started = true;
                    decoder.push(event)?;
                    preview.push(event)?;
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
                    deadline = None;
                    if !already_stopped {
                        let _ = output.send_blocking(Update::Recording);
                    }
                }
                Some("item") => {
                    bytes = bytes.saturating_add(event.to_string().len());
                    if bytes > 32 * 1024 * 1024 {
                        return Err("录制原始事件超过 32 MiB，未提交不完整录制".into());
                    }
                    if !started {
                        return Err("原生录制在 started 前返回输入事件".into());
                    }
                    let items = event["event"].as_array().ok_or("原生录制内容不是数组")?;
                    item_count = item_count
                        .checked_add(items.len())
                        .ok_or("录制事件数量溢出")?;
                    if item_count > 100_000 {
                        return Err("录制超过 100000 个输入事件，未提交不完整录制".into());
                    }
                    // Current ON_MACRO_ITEM's first transition is unconditional
                    // recordingHandler. Its later ESC abort guard is unreachable;
                    // preserve the key instead of inventing an abort transition.
                    decoder.push(event)?;
                    preview.push(event)?;
                    preview_dirty = true;
                }
                Some("stopped") => {
                    if !started || event["mode"] != "kSoftware" {
                        return Err("收到不匹配的录制 stopped 事件".into());
                    }
                    // Cancel may have arrived while this blocking event
                    // request was in flight, including behind a queued Stop.
                    drain_commands(input, &mut stop_requested, &mut cancelled);
                    let result = if cancelled {
                        Ok(None)
                    } else {
                        if let Some(before) = &before {
                            let after =
                                razer_platform::macro_geometry::snapshot().map_err(|error| {
                                    format!("Monitor observation after recording failed: {error:#}")
                                })?;
                            // Nr compares the returned root fields as written;
                            // retain the complete native MonitorRect envelope.
                            if ["top", "left", "width", "height"]
                                .iter()
                                .any(|key| before.screen.get(key) != after.screen.get(key))
                            {
                                return Err("Monitor setting has been changed".into());
                            }
                            decoder.set_geometry(RecordingGeometry {
                                monitors_before: before.monitors.clone(),
                                monitors_after: after.monitors,
                                screen_before: before.screen.clone(),
                                screen_after: after.screen,
                            });
                        }
                        decoder.finish(stop_requested).map(Some)
                    };
                    let restored: RestoreReceipt =
                        serde_json::from_value(query(client, ServiceRequest::ResumeMacroMappings)?)
                            .map_err(|error| format!("录制映射恢复响应格式错误：{error}"))?;
                    if let Some(suppression) = suppression.as_mut() {
                        suppression.release()?;
                    }
                    if restored.global_shortcuts().result() {
                        let _ = output.send_blocking(Update::GlobalShortcutsRestored);
                    }
                    return result;
                }
                Some("error") => {
                    return Err(event["error"].as_str().unwrap_or("原生录制回调错误").into());
                }
                _ => return Err("原生录制返回未知事件".into()),
            }
        }
        if !updates.is_empty() {
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
        if updates.len() < 512 {
            match input.recv_timeout(Duration::from_millis(25)) {
                Ok(Command::Stop) => stop_requested = true,
                Ok(Command::Cancel) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                    stop_requested = true;
                    cancelled = true;
                }
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
    cancelled: &mut bool,
) {
    loop {
        match input.try_recv() {
            Ok(Command::Stop) => *stop_requested = true,
            Ok(Command::Cancel) => {
                *stop_requested = true;
                *cancelled = true;
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                *stop_requested = true;
                *cancelled = true;
                return;
            }
            Err(mpsc::TryRecvError::Empty) => return,
        }
    }
}
