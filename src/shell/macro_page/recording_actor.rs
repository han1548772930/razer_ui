//! The dedicated actor owns ServiceClient, its pipe, DLL worker and shutdown.
//! UI cancellation only drops a sender. No blocking native work runs on GPUI.
use super::ActionItem;
use super::recording_decode::{Decoder, Options};
use crate::backend::runtime::{ServiceClient, ServiceRequest};
use serde_json::Value;
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

pub(super) enum Command {
    Stop,
    Cancel,
}
pub(super) enum Update {
    Recording,
    Progress(usize),
    Preview {
        first_row: usize,
        rows: Vec<ActionItem>,
    },
    Finished(Result<Option<Vec<ActionItem>>, String>),
}
pub(super) fn spawn(
    options: Options,
) -> Result<(mpsc::Sender<Command>, async_channel::Receiver<Update>), String> {
    let (commands, input) = mpsc::channel();
    let (output, updates) = async_channel::unbounded();
    std::thread::Builder::new()
        .name("macro-recording-session".into())
        .spawn(move || {
            let result = ServiceClient::spawn()
                .map_err(|e| format!("{e:#}"))
                .and_then(|mut client| {
                    let result = record(&mut client, &input, &output, &options);
                    // Native shutdown retries recorder stop and mapping restoration.
                    // Keep this destructor on the actor even after the view is dropped.
                    let shutdown = query(&mut client, ServiceRequest::Shutdown);
                    match (result, shutdown) {
                        (Ok(value), Ok(_)) => Ok(value),
                        (Err(error), Ok(_)) => Err(error),
                        (Ok(_), Err(error)) => Err(format!("录制会话关闭失败：{error}")),
                        (Err(error), Err(cleanup)) => {
                            Err(format!("{error}；会话关闭失败：{cleanup}"))
                        }
                    }
                });
            let _ = output.send_blocking(Update::Finished(result));
        })
        .map_err(|e| format!("无法创建录制线程：{e}"))?;
    Ok((commands, updates))
}
fn query(client: &mut ServiceClient, request: ServiceRequest) -> Result<Value, String> {
    client.request(request).map_err(|e| format!("{e:#}"))
}
fn record(
    client: &mut ServiceClient,
    input: &mpsc::Receiver<Command>,
    output: &async_channel::Sender<Update>,
    options: &Options,
) -> Result<Option<Vec<ActionItem>>, String> {
    let mut stop_requested = match input.try_recv() {
        Ok(Command::Cancel) | Err(mpsc::TryRecvError::Disconnected) => return Ok(None),
        Ok(Command::Stop) => true,
        Err(mpsc::TryRecvError::Empty) => false,
    };
    query(client, ServiceRequest::StartMacroRecording)?;
    let mut deadline = Some(Instant::now() + Duration::from_secs(12));
    let mut started = false;
    let mut stopping = false;
    let mut cancelled = false;
    let mut decoder = Decoder::new(options);
    let mut preview = Decoder::for_preview(options);
    let mut preview_at = Instant::now();
    let mut preview_dirty = false;
    let mut item_count = 0usize;
    let mut bytes = 0usize;
    loop {
        match input.try_recv() {
            Ok(Command::Stop) => stop_requested = true,
            Ok(Command::Cancel) | Err(mpsc::TryRecvError::Disconnected) => {
                stop_requested = true;
                cancelled = true;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
        if started && stop_requested && !stopping {
            query(client, ServiceRequest::StopMacroRecording)?;
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
                    if !already_stopped {
                        query(client, ServiceRequest::SuspendMacroMappings)?;
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
                    // Escape key-up follows the original recorder abort path.
                    if items
                        .iter()
                        .any(|v| v["action"] == "keyup" && v["vkCode"] == 27)
                    {
                        stop_requested = true;
                        cancelled = true;
                    }
                    decoder.push(event)?;
                    preview.push(event)?;
                    preview_dirty = true;
                }
                Some("stopped") => {
                    if !started || event["mode"] != "kSoftware" {
                        return Err("收到不匹配的录制 stopped 事件".into());
                    }
                    let result = if cancelled {
                        Ok(None)
                    } else {
                        decoder.finish(stop_requested).map(Some)
                    };
                    query(client, ServiceRequest::ResumeMacroMappings)?;
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
