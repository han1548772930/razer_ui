//! Isolated access to statically verified Razer service APIs.
//!
//! The UI owns `ServiceClient`; only `run_worker` loads vendor DLLs. Requests
//! block, so callers must use a background task. See docs/re/10-runtime-integration.md.
use anyhow::{Context as _, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, Sender},
    time::Duration,
};

#[cfg(windows)]
#[path = "runtime_job.rs"]
mod job;
#[cfg(windows)]
#[path = "runtime_native.rs"]
mod native;

const FRAME_PREFIX: &str = "RAZER_UI_SERVICE ";
const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(4);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub(crate) enum ServiceRequest {
    SimpleVersion,
    AudioDevices,
    HidDevices,
    GlobalMode,
    GlobalShortcuts,
    RegisterShortcut {
        vkey_code: u32,
        modifiers: u32,
        argument: String,
    },
    UnregisterShortcut {
        vkey_code: u32,
        modifiers: u32,
    },
    /// Caller supplies the original generateAppEngineMappings result and hash.
    /// This does not accept an arbitrary UI mapping object.
    SubmitGlobalShortcutMappings {
        app_engine: Value,
    },
    ShortcutEvents,
    Shutdown,
}

#[derive(Serialize, Deserialize)]
struct RequestEnvelope {
    id: u64,
    request: ServiceRequest,
}

#[derive(Serialize, Deserialize)]
struct ResponseEnvelope {
    id: u64,
    data: Option<Value>,
    error: Option<String>,
    fatal: bool,
}

/// A single long-lived worker. Drop terminates only this owned child process.
pub(crate) struct ServiceClient {
    child: Option<Child>,
    #[cfg(windows)]
    job: Option<job::WorkerJob>,
    requests: Option<Sender<Vec<u8>>>,
    responses: Receiver<ResponseEnvelope>,
    timeout: Duration,
    next_id: u64,
    stopped: bool,
}

impl ServiceClient {
    pub(crate) fn spawn() -> anyhow::Result<Self> {
        #[cfg(windows)]
        let job = job::WorkerJob::new()?;
        let mut command = Command::new(std::env::current_exe()?);
        command
            .arg("--service-worker")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW: do not expose a console for a background service.
            command.creation_flags(0x0800_0000);
        }
        let mut child = command.spawn().context("无法启动设备服务子进程")?;
        #[cfg(windows)]
        if let Err(error) = job.assign(&child) {
            return Err(with_cleanup_error(error, terminate_child(child)));
        }
        let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            return Err(with_cleanup_error(
                anyhow::anyhow!("设备服务通信管道不可用"),
                terminate_child(child),
            ));
        };
        let (requests, pending) = mpsc::channel::<Vec<u8>>();
        // Writing a pipe can block if startup hangs before the worker reads it.
        // Keep it off the request thread so recv_timeout covers the full request.
        if let Err(error) = std::thread::Builder::new()
            .name("razer-service-input".into())
            .spawn(move || {
                while let Ok(frame) = pending.recv() {
                    if stdin.write_all(&frame).and_then(|_| stdin.flush()).is_err() {
                        break;
                    }
                }
            })
        {
            return Err(with_cleanup_error(
                anyhow::Error::new(error).context("无法创建设备服务输入线程"),
                terminate_child(child),
            ));
        }
        let (sender, responses) = mpsc::channel();
        if let Err(error) = std::thread::Builder::new()
            .name("razer-service-output".into())
            .spawn(move || {
                let mut reader = BufReader::new(stdout);
                while let Ok(Some(line)) = read_frame(&mut reader) {
                    // Vendor diagnostics do not satisfy our prefixed response frame.
                    let Some(json) = line.strip_prefix(FRAME_PREFIX.as_bytes()) else {
                        continue;
                    };
                    if let Ok(response) = serde_json::from_slice::<ResponseEnvelope>(json) {
                        if sender.send(response).is_err() {
                            break;
                        }
                    }
                }
            })
        {
            drop(requests);
            return Err(with_cleanup_error(
                anyhow::Error::new(error).context("无法创建设备服务输出线程"),
                terminate_child(child),
            ));
        }
        Ok(Self {
            child: Some(child),
            #[cfg(windows)]
            job: Some(job),
            requests: Some(requests),
            responses,
            // First registration can perform initialization, event registration,
            // and registration itself (three bounded native callbacks).
            timeout: Duration::from_secs(15),
            next_id: 1,
            stopped: false,
        })
    }

    pub(crate) fn is_stopped(&self) -> bool {
        self.stopped
    }

    pub(crate) fn request(&mut self, request: ServiceRequest) -> anyhow::Result<Value> {
        if self.stopped {
            bail!("设备服务已停止，请重新连接");
        }
        let shutting_down = matches!(request, ServiceRequest::Shutdown);
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let mut frame = serde_json::to_vec(&RequestEnvelope { id, request })?;
        if frame.len() >= MAX_FRAME_BYTES {
            bail!("设备服务请求过大");
        }
        frame.push(b'\n');
        let write = self
            .requests
            .as_ref()
            .context("设备服务已关闭")?
            .send(frame);
        if let Err(error) = write {
            let error = anyhow::Error::new(error).context("设备服务连接中断");
            return Err(self.stop_after_error(error));
        }
        let response = match self.responses.recv_timeout(self.timeout) {
            Ok(response) if response.id == id => response,
            Ok(_) => {
                return Err(
                    self.stop_after_error(anyhow::anyhow!("设备服务响应序号不匹配，连接已关闭"))
                );
            }
            Err(error) => {
                let error = anyhow::Error::new(error).context("设备服务超时或已退出；连接已关闭");
                return Err(self.stop_after_error(error));
            }
        };
        let stop = if response.fatal || shutting_down {
            self.stop()
        } else {
            Ok(())
        };
        if let Some(error) = response.error {
            if let Err(cleanup) = stop {
                bail!("{error}；关闭设备服务失败：{cleanup:#}");
            }
            bail!("{error}");
        }
        stop?;
        response.data.context("设备服务返回了空响应")
    }

    fn stop_after_error(&mut self, error: anyhow::Error) -> anyhow::Error {
        with_cleanup_error(error, self.stop())
    }

    fn stop(&mut self) -> anyhow::Result<()> {
        self.requests.take();
        self.stopped = true;
        let result = self.child.take().map(terminate_child).unwrap_or(Ok(()));
        // Job closure also terminates the worker if explicit kill failed. OS
        // process exit closes this handle even if Rust Drop never gets to run.
        #[cfg(windows)]
        self.job.take();
        result
    }
}

fn with_cleanup_error(error: anyhow::Error, cleanup: anyhow::Result<()>) -> anyhow::Error {
    match cleanup {
        Ok(()) => error,
        Err(cleanup) => anyhow::anyhow!("{error:#}；关闭设备服务失败：{cleanup:#}"),
    }
}

impl Drop for ServiceClient {
    fn drop(&mut self) {
        if !self.stopped {
            let _ = self.stop();
        }
    }
}

// Terminate only our owned child. `wait` can block in the OS, so even after a
// successful kill it belongs to a detached reaper, never the request thread.
fn terminate_child(mut child: Child) -> anyhow::Result<()> {
    if child.try_wait().ok().flatten().is_some() {
        return Ok(());
    }
    if let Err(error) = child.kill() {
        // Exiting between try_wait and kill is an ordinary race.
        if child.try_wait().ok().flatten().is_none() {
            return Err(error).context("无法结束设备服务子进程");
        }
        return Ok(());
    }
    std::thread::Builder::new()
        .name("razer-service-reaper".into())
        .spawn(move || {
            let _ = child.wait();
        })
        .context("无法创建设备服务回收线程")?;
    Ok(())
}

fn read_frame(reader: &mut impl BufRead) -> anyhow::Result<Option<Vec<u8>>> {
    let mut bytes = Vec::new();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return if bytes.is_empty() {
                Ok(None)
            } else {
                bail!("不完整的设备服务数据帧")
            };
        }
        let end = available.iter().position(|byte| *byte == b'\n');
        let length = end.map_or(available.len(), |index| index + 1);
        if bytes.len() + length > MAX_FRAME_BYTES {
            bail!("设备服务数据帧过大");
        }
        bytes.extend_from_slice(&available[..length]);
        reader.consume(length);
        if end.is_some() {
            // Vendor diagnostics can use a legacy Windows code page. Only a
            // prefixed JSON response needs UTF-8; unrelated bytes are ignored.
            return Ok(Some(bytes));
        }
    }
}

/// `main` must dispatch `--service-worker` here before initializing GPUI, then
/// exit with this status. Never invoke this function from a UI callback.
pub(crate) fn run_worker() -> i32 {
    #[cfg(not(windows))]
    {
        2
    }
    #[cfg(windows)]
    {
        let mut runtime = native::NativeRuntime::new();
        let input = std::io::stdin();
        let mut input = input.lock();
        loop {
            let envelope = match read_frame(&mut input) {
                Ok(Some(line)) => match serde_json::from_slice::<RequestEnvelope>(&line) {
                    Ok(envelope) => envelope,
                    Err(_) => return 2,
                },
                Ok(None) => return 0,
                Err(_) => return 2,
            };
            let shutdown = matches!(envelope.request, ServiceRequest::Shutdown);
            let result = runtime.request(envelope.request);
            let fatal = runtime.is_poisoned();
            let response = match result {
                Ok(data) => ResponseEnvelope {
                    id: envelope.id,
                    data: Some(data),
                    error: None,
                    fatal,
                },
                Err(error) => ResponseEnvelope {
                    id: envelope.id,
                    data: None,
                    error: Some(format!("{error:#}")),
                    fatal,
                },
            };
            let Ok(json) = serde_json::to_string(&response) else {
                return 2;
            };
            let output = std::io::stdout();
            let mut output = output.lock();
            if writeln!(output, "{FRAME_PREFIX}{json}")
                .and_then(|_| output.flush())
                .is_err()
            {
                return 2;
            }
            if fatal || shutdown {
                return if fatal { 3 } else { 0 };
            }
        }
    }
}
