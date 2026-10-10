//! Shared worker wire contract and owned child-process client. No DLL or HID implementation.
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

pub const FRAME_PREFIX: &str = "RAZER_UI_SERVICE ";
pub const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum ServiceRequest {
    /// Original host process-local Map stores; never device write-back or local
    /// draft persistence. The owning host registers real view metadata first.
    HostStorageView {
        view: razer_storage::host::HostStorageView,
    },
    HostStorageClose {
        view_id: u64,
    },
    HostStorageCall {
        call: razer_storage::host::HostStorageCall,
    },
    HostStorageEvents {
        view_id: u64,
    },
    SimpleVersion,
    AudioDevices,
    /// Core Audio endpoint ID from real enumeration, never USB or product ID.
    AudioVolumeRead {
        device_id: String,
    },
    /// Original volume then mute ordering; response retains partial failure
    /// and a separate actual observation rather than fabricating atomic save.
    AudioVolumeWrite {
        device_id: String,
        mute: bool,
        volume: u8,
    },
    /// Source-recovered RzAudioUtil enumeration through the OS adapter;
    /// distinct from simple_service's AudioDevices response schema.
    AudioEndpoints {
        flow: razer_device::audio_util::AudioFlow,
    },
    HidDevices,
    /// Portable OS collections. Paths are opaque bytes; no Windows GUID or
    /// logical receiver peer is fabricated from these nodes.
    HidNodes,
    /// Observe the retained collection's actual descriptor, without sending a
    /// Razer command. Used to disambiguate source-selected transport routes.
    HidNodeReports {
        node: razer_device::backend::HidNode,
    },
    HidNodeRead {
        node: razer_device::backend::HidNode,
        product_id: u32,
        kind: razer_device::device_reads::DeviceReadKind,
    },
    /// Typed direct-device write; source capability, live identity, reply and
    /// readback are checked by the agent. No arbitrary reports or exports.
    HidNodeWrite {
        node: razer_device::backend::HidNode,
        product_id: u32,
        setting: razer_device::device_writes::DeviceWriteSetting,
    },
    HidNodeReceiverStatus {
        node: razer_device::backend::HidNode,
    },
    HidNodeDpiStagesRead {
        node: razer_device::backend::HidNode,
        product_id: u32,
    },
    HidNodeDpiStagesWrite {
        node: razer_device::backend::HidNode,
        product_id: u32,
        draft: razer_device::mouse_dpi_stages::DpiStagesDraft,
    },
    HidNodeKeyboardBrightnessRead {
        node: razer_device::backend::HidNode,
        product_id: u32,
    },
    HidNodeKeyboardBrightnessWrite {
        node: razer_device::backend::HidNode,
        product_id: u32,
        percent: u8,
    },
    /// Current Audio Mixer DSP reports; some queries use a hardware mailbox
    /// selector, so this is not a side-effect-free register snapshot.
    HidNodeMixerRead {
        node: razer_device::backend::HidNode,
        product_id: u32,
        target: razer_device::audio_mixer::MixerTarget,
    },
    HidNodeMixerWrite {
        node: razer_device::backend::HidNode,
        product_id: u32,
        target: razer_device::audio_mixer::MixerTarget,
        value: razer_device::audio_mixer::MixerValue,
    },
    HidNodeMixerRouteRead {
        node: razer_device::backend::HidNode,
        product_id: u32,
        route: razer_device::audio_mixer::MixerRoute,
    },
    HidNodeMixerRouteWrite {
        node: razer_device::backend::HidNode,
        product_id: u32,
        route: razer_device::audio_mixer::MixerRoute,
        enabled: bool,
    },
    /// Original restartAudioDriver sequence; reports IOCTL completion, not
    /// a fabricated observation of audio playback or stream state.
    HidNodeMixerRestartStreams {
        node: razer_device::backend::HidNode,
        product_id: u32,
    },
    /// Physical USB devices, including products without a HID collection.
    UsbDevices,
    /// Read-only version query on any library the generated native inventory
    /// declares. Only a no-argument string-returning `GetDLLVersion`/`GetDllVersion`
    /// prefix is accepted, so no mutation can be requested through it.
    NativeLibraryVersion {
        library: String,
        #[serde(default)]
        product_id: Option<u32>,
    },
    /// Read-only getter on a declared device library
    /// (`Get*`/`Is*`/`Has*` with one `string` argument). Mutating exports are
    /// refused in the worker; see `backend::native_read`.
    NativeLibraryGetter {
        library: String,
        export: String,
        device_id: String,
        product_id: Option<u32>,
    },
    NativeLibrarySnapshot {
        library: String,
        device_container_id: String,
        product_id: u32,
    },
    /// A source-described query, scoped to a currently observed physical path.
    DeviceRead {
        target: razer_device::device_reads::DeviceReadTarget,
        kind: razer_device::device_reads::DeviceReadKind,
    },
    DeviceWrite {
        target: razer_device::device_reads::DeviceReadTarget,
        setting: razer_device::device_writes::DeviceWriteSetting,
    },
    DeviceKeyboardBrightnessRead {
        target: razer_device::device_reads::DeviceReadTarget,
    },
    DeviceDpiStagesRead {
        target: razer_device::device_reads::DeviceReadTarget,
    },
    DeviceDpiStagesWrite {
        target: razer_device::device_reads::DeviceReadTarget,
        draft: razer_device::mouse_dpi_stages::DpiStagesDraft,
    },
    DeviceKeyboardBrightnessWrite {
        target: razer_device::device_reads::DeviceReadTarget,
        percent: u8,
    },
    /// Observe global input through the current mapping-engine recorder.
    /// These requests do not submit macros or mappings to a device.
    StartMacroRecording,
    StopMacroRecording,
    MacroRecordingEvents,
    SuspendMacroMappings,
    ResumeMacroMappings,
    ReceiverWirelessStatus {
        path: String,
        device_container_id: String,
    },
    GlobalMode,
    WheelScrollLinesRead,
    WheelScrollLinesWrite {
        lines: i32,
    },
    WindowsServiceStatus {
        name: String,
    },
    WindowsServiceStart {
        name: String,
    },
    WindowsServiceStop {
        name: String,
    },
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

/// Diagnostic CLI requests use the same bounded worker and cleanup as the UI.
/// This function is a runtime entrypoint, never a static verification command.
pub fn isolated_request(request: ServiceRequest) -> anyhow::Result<Value> {
    let mut client = ServiceClient::spawn()?;
    let result = client.request(request);
    let shutdown = if client.is_stopped() {
        Ok(Value::Null)
    } else {
        client.request(ServiceRequest::Shutdown)
    };
    match (result, shutdown) {
        (Ok(value), Ok(_)) => Ok(value),
        (Err(error), Ok(_)) => Err(error),
        (Ok(_), Err(error)) => Err(error.context("查询后的 worker 关闭失败")),
        (Err(error), Err(shutdown)) => {
            Err(error.context(format!("worker 关闭同时失败：{shutdown:#}")))
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct RequestEnvelope {
    pub id: u64,
    pub request: ServiceRequest,
}

#[derive(Serialize, Deserialize)]
pub struct ResponseEnvelope {
    pub id: u64,
    pub data: Option<Value>,
    pub error: Option<String>,
    pub fatal: bool,
}

/// A single long-lived worker. Drop terminates only this owned child process.
pub struct ServiceClient {
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
    pub fn spawn() -> anyhow::Result<Self> {
        let current = std::env::current_exe()?;
        let sibling = current.with_file_name(if cfg!(windows) {
            "razer_agent.exe"
        } else {
            "razer_agent"
        });
        // Distributions include the independent headless binary. Existing
        // root-only development commands retain their pre-GPUI worker route.
        let executable = if sibling.is_file() { sibling } else { current };
        Self::spawn_with_program(&executable)
    }

    /// Explicit child executable for hosts that package the agent separately.
    /// No DLL/backend is initialized in the caller process.
    pub fn spawn_with_program(executable: &std::path::Path) -> anyhow::Result<Self> {
        #[cfg(windows)]
        let job = job::WorkerJob::new()?;
        let mut command = Command::new(executable);
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

    pub fn is_stopped(&self) -> bool {
        self.stopped
    }

    pub fn request(&mut self, request: ServiceRequest) -> anyhow::Result<Value> {
        if self.stopped {
            bail!("设备服务已停止，请重新连接");
        }
        let shutting_down = matches!(request, ServiceRequest::Shutdown);
        let starting_recorder = matches!(request, ServiceRequest::StartMacroRecording);
        let writing_device = matches!(
            request,
            ServiceRequest::DeviceWrite { .. }
                | ServiceRequest::AudioVolumeWrite { .. }
                | ServiceRequest::HidNodeDpiStagesWrite { .. }
                | ServiceRequest::DeviceDpiStagesWrite { .. }
                | ServiceRequest::HidNodeWrite { .. }
                | ServiceRequest::HidNodeMixerWrite { .. }
                | ServiceRequest::HidNodeMixerRouteWrite { .. }
                | ServiceRequest::HidNodeMixerRestartStreams { .. }
        );
        let reading_device = matches!(
            request,
            ServiceRequest::DeviceRead { .. }
                | ServiceRequest::AudioVolumeRead { .. }
                | ServiceRequest::HidNodeDpiStagesRead { .. }
                | ServiceRequest::DeviceDpiStagesRead { .. }
                | ServiceRequest::NativeLibrarySnapshot { .. }
                | ServiceRequest::AudioEndpoints { .. }
                | ServiceRequest::HidNodeRead { .. }
                | ServiceRequest::HidNodeMixerRead { .. }
                | ServiceRequest::HidNodeMixerRouteRead { .. }
        );
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
        // Recorder setup can require initialize/register/callback/start, each
        // with its own bounded native completion. Other queries retain their
        // existing timeout; a timeout still terminates this owned worker.
        let timeout = if shutting_down {
            // Recorder stop, mapping restore, callback unregister and two
            // independent engine shutdowns can each consume CALLBACK_TIMEOUT.
            Duration::from_secs(25)
        } else if starting_recorder {
            Duration::from_secs(20)
        } else if writing_device {
            // Retained route: pre-read, setter acknowledgement, then readback.
            Duration::from_secs(30)
        } else if reading_device {
            // Relay reads bracket the device query with two bounded peer
            // observations, so the ordinary single-query limit is insufficient.
            Duration::from_secs(35)
        } else {
            self.timeout
        };
        let response = match self.responses.recv_timeout(timeout) {
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

pub fn read_frame(reader: &mut impl BufRead) -> anyhow::Result<Option<Vec<u8>>> {
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
