//! Read-only connection UI. The actor owns the child process, including shutdown;
//! no DLL, pipe operation, mutex wait or child destructor runs on the UI thread.
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_discovery::discovery;
use razer_discovery::discovery::DiscoverySnapshot;
use razer_ipc::ServiceClient;
use razer_ipc::ServiceRequest;
use razer_platform::device_changes::DeviceChange;
use razer_platform::device_changes::DeviceChangeMonitor;
use razer_storage::file_transfer::{Operation as ReportOperation, Transfer as ReportTransfer};
use razer_widgets::surface;
use serde_json::Value;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

#[path = "runtime_page/diagnostics.rs"]
mod diagnostics;

#[derive(Clone)]
struct RuntimeBridge(Arc<ConnectionOwner>);
struct ConnectionOwner {
    commands: mpsc::Sender<Command>,
    worker: Mutex<Option<std::thread::JoinHandle<()>>>,
}
type DiscoveryProgress = async_channel::Sender<(DiscoverySnapshot, mpsc::Sender<()>)>;
enum Command {
    Refresh(
        bool,
        DiscoveryProgress,
        Arc<AtomicBool>,
        ReportOperation,
        mpsc::Sender<Readings>,
    ),
    Disconnect(mpsc::Sender<Result<(), String>>),
}

#[derive(Clone, Copy)]
enum DisconnectIntent {
    User,
    ApplicationExit,
}

#[derive(Default)]
struct Readings {
    connected: bool,
    usb: Option<Result<Value, String>>,
    hid: Option<Result<Value, String>>,
    // Portable collection responses retain their own schema, never Windows interfaces.
    hid_nodes: Option<Result<Value, String>>,
    version: Option<Result<Value, String>>,
    audio: Option<Result<Value, String>>,
    native: Vec<NativeReading>,
    discovery: Option<Result<DiscoverySnapshot, String>>,
    services_requested: bool,
    diagnostic_report: Option<Result<std::path::PathBuf, String>>,
}

struct NativeReading {
    product_id: u32,
    container: String,
    library: String,
    result: Result<Value, String>,
}

#[derive(Clone)]
pub struct DiscoveryObserved(
    pub Result<DiscoverySnapshot, String>,
    pub Option<mpsc::Sender<()>>,
);
impl EventEmitter<DiscoveryObserved> for RuntimePanel {}
/// Values follow the initial interface observation without expiring its scope.
#[derive(Clone)]
pub struct DeviceValuesObserved(pub Result<DiscoverySnapshot, String>);
impl EventEmitter<DeviceValuesObserved> for RuntimePanel {}

impl Readings {
    fn has_partial_results(&self) -> bool {
        #[cfg(windows)]
        let interfaces = [&self.usb, &self.hid];
        #[cfg(not(windows))]
        let interfaces = [&self.hid_nodes];
        let required = if self.services_requested || self.version.is_some() || self.audio.is_some()
        {
            interfaces
                .into_iter()
                .chain([&self.version, &self.audio])
                .collect::<Vec<_>>()
        } else {
            interfaces.into_iter().collect::<Vec<_>>()
        };
        #[cfg(windows)]
        let enumeration = [(&self.usb, "devices"), (&self.hid, "interfaces")];
        #[cfg(not(windows))]
        let enumeration = [(&self.hid_nodes, "nodes")];
        required.iter().any(|result| !matches!(result, Some(Ok(_))))
            || self.native.iter().any(|reading| match &reading.result {
                Ok(value) => value["status"] != "received",
                Err(_) => true,
            })
            || self.discovery.as_ref().is_some_and(|result| match result {
                Ok(snapshot) => !snapshot.errors().is_empty(),
                Err(_) => true,
            })
            || enumeration.iter().any(|(result, key)| {
                result
                    .as_ref()
                    .and_then(|result| result.as_ref().ok())
                    .is_some_and(|value| {
                        value.get(*key).and_then(Value::as_array).is_none()
                            || value.get("complete") != Some(&Value::Bool(true))
                            || value
                                .get("failures")
                                .and_then(Value::as_array)
                                .is_some_and(|failures| !failures.is_empty())
                    })
            })
    }

    fn status(&self) -> &'static str {
        if !self.connected {
            "服务连接已结束；下方保留本次读取结果。"
        } else if self.has_partial_results() {
            "读取完成，部分信息不可用。"
        } else if self
            .discovery
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .is_some_and(|snapshot| diagnostics::read_count(snapshot) > 0)
        {
            "本次设备信息读取完成。"
        } else if !self.services_requested {
            "设备接口已更新，本次未取得设备参数。"
        } else {
            "服务信息已读取。"
        }
    }
}

impl RuntimeBridge {
    fn start() -> Result<Self, String> {
        Self::start_worker(move |commands| {
            let mut client: Option<ServiceClient> = None;
            while let Ok(command) = commands.recv() {
                match command {
                    Command::Refresh(services, progress, guard, report, reply) => {
                        if !guard.load(Ordering::Acquire) {
                            let _ = reply.send(Readings::default());
                            continue;
                        }
                        let mut result = match client.as_mut() {
                            Some(client) => {
                                client.set_request_guard(guard.clone());
                                read_services(client, services, &progress, &guard)
                            }
                            None => match ServiceClient::spawn() {
                                Ok(mut connection) => {
                                    connection.set_request_guard(guard.clone());
                                    let result =
                                        read_services(&mut connection, services, &progress, &guard);
                                    client = Some(connection);
                                    result
                                }
                                Err(error) => {
                                    let error = format!("{error:#}");
                                    Readings {
                                        connected: false,
                                        #[cfg(windows)]
                                        usb: Some(Err(error.clone())),
                                        #[cfg(windows)]
                                        hid: Some(Err(error.clone())),
                                        #[cfg(not(windows))]
                                        hid_nodes: Some(Err(error.clone())),
                                        version: services.then(|| Err(error.clone())),
                                        audio: services.then(|| Err(error.clone())),
                                        native: Vec::new(),
                                        discovery: Some(Err(error)),
                                        services_requested: services,
                                        diagnostic_report: None,
                                        ..Readings::default()
                                    }
                                }
                            },
                        };
                        if !guard.load(Ordering::Acquire) {
                            result.connected = false;
                        }
                        if !result.connected {
                            client.take();
                        }
                        if guard.load(Ordering::Acquire) {
                            result.diagnostic_report = Some(
                                result
                                    .write_diagnostic_report(&report)
                                    .map_err(|error| format!("{error:#}")),
                            );
                        }
                        let _ = reply.send(result);
                    }
                    Command::Disconnect(reply) => {
                        let result = client.take().map_or(Ok(()), |mut client| {
                            if client.is_stopped() {
                                return Ok(());
                            }
                            client
                                .request(ServiceRequest::Shutdown)
                                .map(|_| ())
                                .map_err(|error| format!("{error:#}"))
                        });
                        let _ = reply.send(result);
                        return;
                    }
                }
            }
            // Closing the UI only drops senders. Native shutdown and child.wait
            // remain here, even when an in-flight UI task has been cancelled.
            if let Some(mut client) = client {
                let _ = client.request(ServiceRequest::Shutdown);
            }
        })
    }

    fn start_worker(
        run: impl FnOnce(mpsc::Receiver<Command>) + Send + 'static,
    ) -> Result<Self, String> {
        let (commands, incoming) = mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("razer-service-connection".into())
            .spawn(move || run(incoming))
            .map_err(|error| format!("无法创建服务连接线程：{error}"))?;
        Ok(Self(Arc::new(ConnectionOwner {
            commands,
            worker: Mutex::new(Some(worker)),
        })))
    }

    fn refresh(
        &self,
        services: bool,
        progress: DiscoveryProgress,
        guard: Arc<AtomicBool>,
        report: ReportOperation,
    ) -> Result<Readings, String> {
        let (reply, response) = mpsc::channel();
        self.0
            .commands
            .send(Command::Refresh(services, progress, guard, report, reply))
            .map_err(|_| "服务连接已结束，请重新连接。".to_string())?;
        response
            // Each worker request already has a deadline. A fixed aggregate
            // timeout discarded valid results when several receivers existed.
            .recv()
            .map_err(|error| format!("服务读取未完成：{error}"))
    }

    fn disconnect(&self) -> Result<(), String> {
        let (reply, response) = mpsc::channel();
        let acknowledged = self
            .0
            .commands
            .send(Command::Disconnect(reply))
            .map_err(|_| "服务连接已经结束。".to_string())
            .and_then(|()| {
                response
                    .recv_timeout(Duration::from_secs(30))
                    .map_err(|error| format!("服务关闭未完成：{error}"))?
            });
        // A response confirms the service cleanup result. The background
        // caller also retains this owner until its actor has actually exited.
        let worker = self
            .0
            .worker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        let completed = worker.map_or(Ok(()), |worker| {
            worker
                .join()
                .map_err(|_| "服务连接线程异常结束。".to_owned())
        });
        match (acknowledged, completed) {
            (Ok(()), result) | (result, Ok(())) => result,
            (Err(acknowledged), Err(completed)) => Err(format!("{acknowledged}; {completed}")),
        }
    }
}

fn wait_for_discovery_owner(acknowledged: &mpsc::Receiver<()>, guard: &AtomicBool) -> bool {
    let started = std::time::Instant::now();
    while guard.load(Ordering::Acquire) {
        let Some(remaining) = Duration::from_secs(30).checked_sub(started.elapsed()) else {
            return false;
        };
        match acknowledged.recv_timeout(remaining.min(Duration::from_millis(50))) {
            Ok(()) => return guard.load(Ordering::Acquire),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return false,
        }
    }
    false
}

fn read_services(
    client: &mut ServiceClient,
    services: bool,
    progress: &DiscoveryProgress,
    guard: &AtomicBool,
) -> Readings {
    #[cfg(windows)]
    let mut readings = {
        // Enumerate physical USB and HID separately before vendor services. Neither
        // enumeration's failure discards the other one's real observations.
        let usb = client
            .request(ServiceRequest::UsbDevices)
            .map_err(|error| format!("{error:#}"));
        let hid = client
            .request(ServiceRequest::HidDevices)
            .map_err(|error| format!("{error:#}"));
        let discovered =
            discovery::discover(client, &usb, &hid).map_err(|error| format!("{error:#}"));
        Readings {
            usb: Some(usb),
            hid: Some(hid),
            discovery: Some(discovered),
            ..Readings::default()
        }
    };
    #[cfg(not(windows))]
    let mut readings = {
        let nodes = client
            .request(ServiceRequest::HidNodes)
            .map_err(|error| format!("{error:#}"));
        let discovered =
            discovery::discover_portable(client, &nodes).map_err(|error| format!("{error:#}"));
        Readings {
            hid_nodes: Some(nodes),
            discovery: Some(discovered),
            ..Readings::default()
        }
    };
    if let Some(Ok(snapshot)) = &mut readings.discovery {
        // Publish identified interfaces before slower configuration queries.
        // The UI captures profile/connection scope at this exact boundary.
        let (ready, acknowledged) = mpsc::channel();
        if progress.send_blocking((snapshot.clone(), ready)).is_ok()
            && wait_for_discovery_owner(&acknowledged, guard)
        {
            #[cfg(windows)]
            {
                let missing_interfaces = Err("Windows HID 接口观察未保留".into());
                discovery::read_device_values(
                    client,
                    readings.hid.as_ref().unwrap_or(&missing_interfaces),
                    snapshot,
                );
            }
            #[cfg(not(windows))]
            {
                // The shared reader selects the real hid_node route first. Its
                // Windows interface input is unused for portable observations;
                // neither convert nodes nor send collection keys as ContainerIds.
                let unused_windows_interfaces = Err("当前平台不使用 Windows HID 接口观察".into());
                discovery::read_device_values(client, &unused_windows_interfaces, snapshot);
            }
        } else {
            readings.discovery = Some(Err("界面尚未确认本次发现，设备参数查询未开始。".into()));
        }
    }
    if !guard.load(Ordering::Acquire) || client.is_stopped() {
        readings.services_requested = services;
        return readings;
    }
    #[cfg(windows)]
    let version = services.then(|| {
        client
            .request(ServiceRequest::SimpleVersion)
            .map_err(|error| format!("{error:#}"))
    });
    #[cfg(not(windows))]
    let version = services.then(|| Err("当前平台尚不支持 Razer Windows 服务版本查询".into()));
    #[cfg(windows)]
    let audio = services.then(|| {
        client
            .request(ServiceRequest::AudioDevices)
            .map_err(|error| format!("{error:#}"))
    });
    #[cfg(not(windows))]
    let audio = services.then(|| Err("当前平台尚不支持原生系统音频设备查询".into()));
    #[cfg(windows)]
    let mut native = Vec::new();
    #[cfg(not(windows))]
    let native = Vec::new();
    #[cfg(windows)]
    if services {
        if let Some(Ok(snapshot)) = &readings.discovery {
            for device in snapshot.devices() {
                // A collection key is not accepted by native ContainerId APIs,
                // even if a future Windows discovery path supplies HidNodes.
                if device.hid_node().is_some() {
                    continue;
                }
                for library in
                    razer_catalog::native_library::libraries_for_product(device.product_id())
                {
                    if razer_catalog::native_library::readable_exports(library, device.product_id())
                        .is_empty()
                    {
                        continue;
                    }
                    let result = client
                        .request(ServiceRequest::NativeLibrarySnapshot {
                            library: library.id.clone(),
                            device_container_id: device.container().to_owned(),
                            product_id: device.product_id(),
                        })
                        .map_err(|error| format!("{error:#}"));
                    native.push(NativeReading {
                        product_id: device.product_id(),
                        container: device.container().to_owned(),
                        library: library.id.clone(),
                        result,
                    });
                }
            }
        }
    }
    readings.connected = !client.is_stopped();
    readings.version = version;
    readings.audio = audio;
    readings.native = native;
    readings.services_requested = services;
    readings
}

pub struct RuntimePanel {
    bridge: Option<RuntimeBridge>,
    retired_bridges: Vec<RuntimeBridge>,
    request_guard: Option<Arc<AtomicBool>>,
    report_transfer: Option<ReportTransfer>,
    refresh_task: Option<Task<()>>,
    disconnect_task: Option<Task<()>>,
    disconnecting: bool,
    exit_prepared: bool,
    monitor: Option<DeviceChangeMonitor>,
    retired_monitors: Vec<DeviceChangeMonitor>,
    monitor_task: Option<Task<()>>,
    monitor_error: Option<String>,
    pending_refresh: Option<bool>,
    busy: bool,
    readings: Readings,
    status: String,
    error: Option<String>,
    details: bool,
}

impl RuntimePanel {
    pub fn new() -> Self {
        Self {
            bridge: None,
            retired_bridges: Vec::new(),
            request_guard: None,
            report_transfer: None,
            refresh_task: None,
            disconnect_task: None,
            disconnecting: false,
            exit_prepared: false,
            monitor: None,
            retired_monitors: Vec::new(),
            monitor_task: None,
            monitor_error: None,
            pending_refresh: None,
            busy: false,
            readings: Readings::default(),
            status: "尚未连接".into(),
            error: None,
            details: false,
        }
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.refresh_queries(true, cx);
    }

    pub fn discover_devices(&mut self, cx: &mut Context<Self>) {
        self.refresh_queries(false, cx);
    }

    fn monitor_device_changes(&mut self, cx: &mut Context<Self>) {
        if self.monitor.is_some() {
            return;
        }
        match DeviceChangeMonitor::start() {
            Ok((monitor, changes)) => {
                self.monitor = Some(monitor);
                self.monitor_error = None;
                self.monitor_task = Some(cx.spawn(async move |this, cx| {
                    while let Ok(event) = changes.recv().await {
                        let failed = matches!(&event, DeviceChange::Failed(_));
                        if this
                            .update(cx, |this, cx| match event {
                                DeviceChange::Ready | DeviceChange::Changed => {
                                    this.refresh_queries(false, cx);
                                }
                                DeviceChange::Failed(error) => {
                                    this.monitor_error = Some(error);
                                    this.retired_monitors.extend(this.monitor.take());
                                    cx.notify();
                                }
                            })
                            .is_err()
                            || failed
                        {
                            break;
                        }
                    }
                }));
            }
            Err(error) => self.monitor_error = Some(error),
        }
    }

    fn refresh_queries(&mut self, services: bool, cx: &mut Context<Self>) {
        if self.exit_prepared || self.disconnecting {
            return;
        }
        if self.busy {
            // USB/HID arrival often occurs while enumeration is still running.
            // Keep one follow-up scan and preserve any explicit service refresh.
            self.pending_refresh = Some(self.pending_refresh.unwrap_or(false) || services);
            return;
        }
        if self.bridge.is_none() {
            match RuntimeBridge::start() {
                Ok(bridge) => self.bridge = Some(bridge),
                Err(error) => {
                    self.status = "连接失败；已有结果未更新。".into();
                    self.error = Some(error.clone());
                    cx.emit(DiscoveryObserved(Err(error), None));
                    cx.notify();
                    return;
                }
            }
        }
        self.monitor_device_changes(cx);
        self.busy = true;
        self.error = None;
        self.status = if services {
            "正在读取设备接口与服务信息…"
        } else {
            "正在发现设备…"
        }
        .into();
        let bridge = self.bridge.clone().expect("retained runtime bridge");
        let guard = Arc::new(AtomicBool::new(true));
        self.request_guard = Some(guard.clone());
        let transfer = ReportTransfer::new();
        let report = transfer.accept().expect("new report request is open");
        self.report_transfer = Some(transfer);
        let (progress, snapshots) = async_channel::bounded(1);
        cx.notify();
        self.refresh_task = Some(cx.spawn(async move |this, cx| {
            let request_guard = guard.clone();
            let request = cx.background_spawn(async move {
                let readings = bridge.refresh(services, progress, request_guard, report)?;
                Ok::<_, String>((bridge, readings))
            });
            let mut identified = false;
            // Consume progress before final completion, even if every query
            // finishes before the UI gets its next turn.
            while let Ok((snapshot, ready)) = snapshots.recv().await {
                if !guard.load(Ordering::Acquire) {
                    return;
                }
                identified = true;
                if this
                    .update(cx, |this, cx| {
                        cx.emit(DiscoveryObserved(Ok(snapshot), Some(ready)));
                        this.status = "设备接口已更新，正在读取设备信息…".into();
                        cx.notify();
                    })
                    .is_err()
                {
                    return;
                }
            }
            let result = request.await;
            if !guard.load(Ordering::Acquire) {
                return;
            }
            let _ = this.update(cx, |this, cx| {
                this.request_guard = None;
                this.report_transfer = None;
                this.refresh_task = None;
                this.busy = false;
                match result {
                    Ok((bridge, readings)) => {
                        if let Some(observation) = &readings.discovery {
                            match (identified, observation) {
                                (true, Ok(snapshot)) => {
                                    cx.emit(DeviceValuesObserved(Ok(snapshot.clone())))
                                }
                                _ => cx.emit(DiscoveryObserved(observation.clone(), None)),
                            }
                        }
                        if readings.connected {
                            this.bridge = Some(bridge);
                        } else {
                            this.retired_bridges.push(bridge);
                            this.bridge = None;
                        }
                        this.status = readings.status().into();
                        this.readings = readings;
                    }
                    Err(error) => {
                        cx.emit(DiscoveryObserved(Err(error.clone()), None));
                        this.retired_bridges.extend(this.bridge.take());
                        this.readings.connected = false;
                        this.status = "连接失败；已有结果未更新。".into();
                        this.error = Some(error);
                    }
                }
                cx.notify();
                if let Some(services) = this.pending_refresh.take() {
                    this.refresh_queries(services, cx);
                }
            });
        }));
    }

    fn disconnect(&mut self, cx: &mut Context<Self>) {
        self.begin_disconnect(DisconnectIntent::User, cx);
    }

    fn begin_disconnect(&mut self, intent: DisconnectIntent, cx: &mut Context<Self>) {
        if self.disconnecting {
            return;
        }
        if self.bridge.is_none()
            && self.retired_bridges.is_empty()
            && self.monitor.is_none()
            && self.retired_monitors.is_empty()
        {
            return;
        }
        let mut bridges = std::mem::take(&mut self.retired_bridges);
        bridges.extend(self.bridge.take());
        self.cancel_read();
        self.monitor_task = None;
        let mut monitors = std::mem::take(&mut self.retired_monitors);
        monitors.extend(self.monitor.take());
        self.monitor_error = None;
        self.pending_refresh = None;
        self.busy = true;
        self.disconnecting = true;
        self.status = "正在断开连接…".into();
        self.error = None;
        cx.notify();
        self.disconnect_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let mut observation_errors = Vec::new();
                    for monitor in monitors {
                        if let Err(error) = monitor.shutdown() {
                            observation_errors.push(error);
                        }
                    }
                    let observation = if observation_errors.is_empty() {
                        Ok(())
                    } else {
                        Err(observation_errors.join("; "))
                    };
                    let mut connection_errors = Vec::new();
                    for bridge in bridges {
                        if let Err(error) = bridge.disconnect() {
                            connection_errors.push(error);
                        }
                    }
                    let connection = if connection_errors.is_empty() {
                        Ok(())
                    } else {
                        Err(connection_errors.join("; "))
                    };
                    match (observation, connection) {
                        (Ok(()), result) | (result, Ok(())) => result,
                        (Err(observation), Err(connection)) => {
                            Err(format!("{observation}; {connection}"))
                        }
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if matches!(intent, DisconnectIntent::ApplicationExit) {
                    if let Err(error) = &result {
                        eprintln!("[device discovery cleanup] {error}");
                    }
                }
                this.finish_disconnect(result);
                // A user disconnect expires routes. Final application cleanup
                // must not start page reactions after their owners have closed.
                if matches!(intent, DisconnectIntent::User) {
                    cx.emit(DiscoveryObserved(
                        Err("服务连接已断开，此前设备观察已过期".into()),
                        None,
                    ));
                }
                cx.notify();
            });
        }));
    }

    fn finish_disconnect(&mut self, result: Result<(), String>) {
        self.disconnect_task = None;
        self.busy = false;
        self.disconnecting = false;
        self.readings.connected = false;
        self.status = if result.is_ok() {
            "已断开；下方保留最近读取的信息。"
        } else {
            "断开未正常完成；下方保留最近读取的信息。"
        }
        .into();
        self.error = result.err();
    }

    fn cancel_read(&mut self) {
        if let Some(transfer) = self.report_transfer.take() {
            transfer.cancel();
        }
        if let Some(guard) = self.request_guard.take() {
            guard.store(false, Ordering::Release);
        }
        self.refresh_task = None;
    }

    pub fn prepare_exit(&mut self) {
        // The discovery connection is distinct from each device session. Keep
        // its observations valid while accepted product work drains; only the
        // final application cleanup closes it after product work has drained.
        self.exit_prepared = true;
        self.cancel_read();
        self.pending_refresh = None;
        if !self.disconnecting {
            self.busy = false;
        }
    }

    pub fn cancel_exit(&mut self, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.exit_prepared) {
            if self.bridge.is_some() || self.monitor.is_some() {
                self.discover_devices(cx);
            }
            cx.notify();
        }
    }

    pub fn shutdown_pending(&self) -> bool {
        self.disconnecting
    }

    pub fn shutdown_for_app_quit(&mut self, cx: &mut Context<Self>) -> Option<Task<()>> {
        self.exit_prepared = true;
        self.begin_disconnect(DisconnectIntent::ApplicationExit, cx);
        self.disconnect_task.take()
    }
}

impl Drop for RuntimePanel {
    fn drop(&mut self) {
        self.cancel_read();
    }
}

impl Render for RuntimePanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let connected = self.bridge.is_some();
        surface::panel("服务连接", cx)
            .id("runtime-panel")
            .test_support()
            .w_full()
            .gap_3()
            .child(surface::note(
                "显示本次设备接口、设备信息和服务查询结果。",
                cx,
            ))
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_3()
                    .child(
                        Button::new("runtime-connect")
                            .label(if connected {
                                "刷新信息"
                            } else {
                                "连接并读取"
                            })
                            .primary()
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                    )
                    .child(
                        Button::new("runtime-disconnect")
                            .label("断开连接")
                            .outline()
                            .disabled(
                                self.disconnecting
                                    || (!connected
                                        && self.retired_bridges.is_empty()
                                        && self.monitor.is_none()
                                        && self.retired_monitors.is_empty()),
                            )
                            .on_click(cx.listener(|this, _, _, cx| this.disconnect(cx))),
                    ),
            )
            .child(
                div()
                    .id("runtime-status")
                    .test_support()
                    .role(Role::Status)
                    .aria_label(self.status.clone())
                    .child(self.status.clone()),
            )
            .when_some(self.error.clone(), |this, error| {
                this.child(
                    div()
                        .id("runtime-error")
                        .test_support()
                        .aria_label(error.clone())
                        .text_color(cx.theme().danger)
                        .child(error),
                )
            })
            .when_some(self.monitor_error.clone(), |this, error| {
                this.child(
                    div()
                        .id("runtime-device-monitor-error")
                        .role(Role::Status)
                        .text_color(cx.theme().danger)
                        .child(error),
                )
            })
            .when_some(self.readings.hid.as_ref(), |this, result| {
                this.child(hid_result(result, cx))
            })
            .when_some(self.readings.hid_nodes.as_ref(), |this, result| {
                this.child(hid_nodes_result(result, cx))
            })
            .when_some(self.readings.usb.as_ref(), |this, result| {
                this.child(usb_result(result, cx))
            })
            .when_some(self.readings.discovery.as_ref(), |view, result| {
                let label = match result {
                    Ok(snapshot) => format!(
                        "识别到 {} 项产品接口或接收器关联，读取 {} 项设备信息。{}",
                        snapshot.devices().len(),
                        diagnostics::read_count(snapshot),
                        snapshot.errors().join("；")
                    ),
                    Err(error) => format!("产品发现失败：{error}"),
                };
                view.child(
                    div()
                        .id("runtime-discovery-status")
                        .test_support()
                        .role(Role::Status)
                        .aria_label(label.clone())
                        .child(label),
                )
                .when_some(result.as_ref().ok(), |view, snapshot| {
                    view.children(snapshot.devices().iter().filter_map(|device| {
                        device.read_values().map(|values| {
                            diagnostics::device_values(device.product_id(), values, cx)
                        })
                    }))
                })
            })
            .when_some(self.readings.version.as_ref(), |this, result| {
                this.child(reading("服务版本", result, cx))
            })
            .when_some(self.readings.audio.as_ref(), |this, result| {
                this.child(reading("音频设备", result, cx))
            })
            .children(self.readings.native.iter().map(|observation| {
                let label = match &observation.result {
                    Ok(value) => format!(
                        "产品 {} · {}：读取 {} 项，{} 项失败。",
                        observation.product_id,
                        observation.library,
                        value["fields"].as_object().map_or(0, |fields| fields.len()),
                        value["errors"].as_object().map_or(0, |fields| fields.len())
                    ),
                    Err(error) => format!(
                        "产品 {} · {} 读取失败：{error}",
                        observation.product_id, observation.library
                    ),
                };
                let fields = observation
                    .result
                    .as_ref()
                    .ok()
                    .and_then(|value| value["fields"].as_object());
                let errors = observation
                    .result
                    .as_ref()
                    .ok()
                    .and_then(|value| value["errors"].as_object());
                div()
                    .child(surface::note(label, cx))
                    .when(self.details, |view| {
                        view.children(fields.into_iter().flat_map(|fields| fields.iter()).map(
                            |(export, value)| {
                                div()
                                    .whitespace_normal()
                                    .child(format!("{export}：{}", display_value(&value["value"])))
                            },
                        ))
                        .children(
                            errors.into_iter().flat_map(|errors| errors.iter()).map(
                                |(export, error)| {
                                    div()
                                        .whitespace_normal()
                                        .text_color(cx.theme().danger)
                                        .child(format!("{export}：{}", display_value(error)))
                                },
                            ),
                        )
                    })
                    .into_any_element()
            }))
            .when(
                self.readings.hid.is_some() || self.readings.hid_nodes.is_some(),
                |this| {
                    this.child(
                        Button::new("runtime-details")
                            .ghost()
                            .label(if self.details {
                                "收起读取详情"
                            } else {
                                "查看读取详情"
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.details = !this.details;
                                cx.notify();
                            })),
                    )
                    .when(self.details, |this| {
                        this.when_some(self.readings.diagnostic_report.as_ref(), |view, report| {
                            let label = match report {
                                Ok(path) => format!("本次设备发现详情已记录：{}", path.display()),
                                Err(error) => format!("设备发现详情未能记录：{error}"),
                            };
                            view.child(surface::note(label, cx))
                        })
                        .children(
                            [
                                ("USB 设备", &self.readings.usb),
                                ("HID 接口", &self.readings.hid),
                                ("HID collection", &self.readings.hid_nodes),
                                ("版本", &self.readings.version),
                                ("音频", &self.readings.audio),
                            ]
                            .into_iter()
                            .filter_map(|(label, result)| {
                                result.as_ref().and_then(|result| result.as_ref().ok()).map(
                                    |value| {
                                        v_flex()
                                            .gap_2()
                                            .child(label)
                                            .child(
                                                div()
                                                    .w_full()
                                                    .whitespace_normal()
                                                    .text_size(surface::css(12.))
                                                    .child(display_value(value)),
                                            )
                                            .into_any_element()
                                    },
                                )
                            }),
                        )
                    })
                },
            )
    }
}

fn reading(label: &str, result: &Result<Value, String>, cx: &App) -> AnyElement {
    let text = match result {
        Ok(Value::String(value)) if !value.trim().is_empty() => {
            format!("{label}：{}", limit_text(value, 500))
        }
        Ok(Value::Array(items)) => format!("{label}：已收到 {} 项信息，可展开查看。", items.len()),
        Ok(Value::Object(_)) => format!("{label}：已收到服务信息，可展开查看。"),
        Ok(_) => format!("{label}：服务未提供可显示的信息。"),
        Err(error) => format!("{label}读取失败：{error}"),
    };
    div()
        .whitespace_normal()
        .when(result.is_err(), |this| this.text_color(cx.theme().danger))
        .child(text)
        .into_any_element()
}

fn hid_nodes_result(result: &Result<Value, String>, cx: &App) -> AnyElement {
    let Ok(value) = result else {
        return reading("HID collection", result, cx);
    };
    let Ok(nodes) =
        serde_json::from_value::<Vec<razer_device::backend::HidNode>>(value["nodes"].clone())
    else {
        return div()
            .text_color(cx.theme().danger)
            .child("HID collection 响应格式无法识别。")
            .into_any_element();
    };
    // A successful hidapi response is an observation, not proof that all
    // devices were enumerated. Keep the backend's unknown completeness visible.
    let summary = format!(
        "观察到 {} 项 HID collection；后端未报告枚举完整性，同一设备可能包含多个 collection。",
        nodes.len()
    );
    v_flex()
        .gap_2()
        .child(
            div()
                .id("runtime-hid-nodes-summary")
                .test_support()
                .role(Role::Status)
                .aria_label(summary.clone())
                .child(summary),
        )
        .children(nodes.into_iter().enumerate().map(|(index, node)| {
            let product = node
                .product
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "未提供产品名称".into());
            let serial = node
                .serial_number
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "未提供".into());
            let identity = format!(
                "VID/PID {:04X}:{:04X} · 接口 {} · Usage {:04X}:{:04X}",
                node.vendor_id, node.product_id, node.interface_number, node.usage_page, node.usage
            );
            v_flex()
                .id(SharedString::from(format!("runtime-hid-node-{index}")))
                .test_support()
                .role(Role::ListItem)
                .aria_label(format!("{product} · {identity} · {serial}"))
                .gap_1()
                .child(product)
                .child(surface::note(format!("序列号：{serial} · {identity}"), cx))
                .into_any_element()
        }))
        .into_any_element()
}

fn hid_result(result: &Result<Value, String>, cx: &App) -> AnyElement {
    let Ok(value) = result else {
        return reading("设备接口", result, cx);
    };
    let Some(interfaces) = value.get("interfaces").and_then(Value::as_array) else {
        return div()
            .text_color(cx.theme().danger)
            .child("设备接口响应格式无法识别。")
            .into_any_element();
    };
    let failures = value
        .get("failures")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let incomplete = value.get("complete") != Some(&Value::Bool(true));
    let summary = if incomplete {
        format!(
            "部分接口读取完成：已读取到 {} 个 Razer 设备接口，枚举未完成。",
            interfaces.len()
        )
    } else {
        format!(
            "读取到 {} 个 Razer 设备接口；同一设备可能包含多个接口。",
            interfaces.len()
        )
    };
    v_flex()
        .gap_2()
        .child(
            div()
                .id("runtime-hid-summary")
                .test_support()
                .role(Role::Status)
                .aria_label(summary.clone())
                .child(summary),
        )
        .children(interfaces.iter().map(|item| {
            let product = item
                .get("product")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .unwrap_or("未提供产品名称");
            let serial = item
                .get("serial_number")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .unwrap_or("未提供");
            let vid = item
                .get("vendor_id")
                .and_then(Value::as_u64)
                .map(|value| format!("{value:04X}"))
                .unwrap_or_else(|| "未知".into());
            let pid = item
                .get("product_id")
                .and_then(Value::as_u64)
                .map(|value| format!("{value:04X}"))
                .unwrap_or_else(|| "未知".into());
            v_flex()
                .id(SharedString::from(format!(
                    "runtime-hid-{}",
                    item["path"].as_str().unwrap_or("missing-path")
                )))
                .test_support()
                .role(Role::ListItem)
                .aria_label(format!("{product} · USB {vid}:{pid} · {serial}"))
                .gap_1()
                .child(product.to_string())
                .child(surface::note(
                    format!("序列号：{serial} · USB {vid}:{pid}"),
                    cx,
                ))
                .into_any_element()
        }))
        .when(failures > 0, |this| {
            this.child(
                div()
                    .text_color(cx.theme().danger)
                    .child(format!("另有 {failures} 项接口读取错误，详情可展开查看。")),
            )
        })
        .into_any_element()
}

fn usb_result(result: &Result<Value, String>, cx: &App) -> AnyElement {
    let summary = match result {
        Err(error) => format!("USB 设备枚举失败：{error}"),
        Ok(value) => match value["devices"].as_array() {
            None => "USB 设备响应格式无法识别。".into(),
            Some(devices) => {
                let count = devices
                    .iter()
                    .filter(|item| item["vendor_id"] == 5426)
                    .count();
                let failures = value["failures"].as_array().map_or(0, Vec::len);
                let state = if value["complete"] == true {
                    "枚举已结束"
                } else {
                    "枚举未完成"
                };
                format!(
                    "发现 {count} 项 Razer USB 设备；{state}，{failures} 项读取错误。配置尚未读取。"
                )
            }
        },
    };
    div()
        .id("runtime-usb-summary")
        .test_support()
        .role(Role::Status)
        .aria_label(summary.clone())
        .when(result.is_err(), |this| this.text_color(cx.theme().danger))
        .child(summary)
        .into_any_element()
}

fn display_value(value: &Value) -> String {
    limit_text(
        &serde_json::to_string_pretty(value).unwrap_or_default(),
        16_000,
    )
}
fn limit_text(value: &str, limit: usize) -> String {
    let mut chars = value.chars();
    let mut text: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        text.push_str("\n…内容较长，后续部分已省略");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::{Readings, RuntimePanel, limit_text};
    use gpui_kit::component::Root;
    use gpui_kit::{AppContext, TestAppContext, px, size};
    use serde_json::json;

    #[test]
    fn cancelled_discovery_owner_rejects_even_buffered_acknowledgement() {
        let (sender, receiver) = std::sync::mpsc::channel();
        sender.send(()).unwrap();
        let guard = std::sync::atomic::AtomicBool::new(false);
        assert!(!super::wait_for_discovery_owner(&receiver, &guard));
    }

    #[test]
    fn discovery_owner_wait_is_interruptible_before_acknowledgement() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let guard = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let worker_guard = guard.clone();
        let worker =
            std::thread::spawn(move || super::wait_for_discovery_owner(&receiver, &worker_guard));
        guard.store(false, std::sync::atomic::Ordering::Release);
        assert!(!worker.join().unwrap());
        drop(sender);
    }

    #[test]
    fn runtime_owner_drop_invalidates_inflight_request_scope() {
        let mut panel = RuntimePanel::new();
        let guard = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        panel.request_guard = Some(guard.clone());
        drop(panel);
        assert!(!guard.load(std::sync::atomic::Ordering::Acquire));
    }

    #[test]
    fn cancelling_a_busy_runtime_retains_previous_observations() {
        let mut panel = RuntimePanel::new();
        panel.busy = true;
        panel.readings.hid = Some(Ok(json!({"interfaces": [{"path": "observed"}]})));
        let guard = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        panel.request_guard = Some(guard.clone());
        panel.cancel_read();
        assert!(!guard.load(std::sync::atomic::Ordering::Acquire));
        assert!(panel.request_guard.is_none());
        assert_eq!(
            panel.readings.hid.as_ref().unwrap().as_ref().unwrap()["interfaces"][0]["path"],
            "observed"
        );
    }

    #[test]
    fn exit_preparation_preserves_the_discovery_connection_until_final_cleanup() {
        let mut panel = RuntimePanel::new();
        let (commands, incoming) = std::sync::mpsc::channel();
        panel.bridge = Some(super::RuntimeBridge(std::sync::Arc::new(
            super::ConnectionOwner {
                commands,
                worker: std::sync::Mutex::new(None),
            },
        )));
        panel.busy = true;
        panel.pending_refresh = Some(true);
        panel.readings.connected = true;
        panel.readings.hid = Some(Ok(json!({"interfaces": [{"path": "observed"}]})));
        let guard = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        panel.request_guard = Some(guard.clone());

        panel.prepare_exit();

        assert!(!guard.load(std::sync::atomic::Ordering::Acquire));
        assert!(panel.request_guard.is_none());
        assert!(panel.pending_refresh.is_none());
        assert!(panel.bridge.is_some());
        assert!(panel.readings.connected);
        assert!(!panel.shutdown_pending());
        assert!(matches!(
            incoming.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
        assert_eq!(
            panel.readings.hid.as_ref().unwrap().as_ref().unwrap()["interfaces"][0]["path"],
            "observed"
        );
    }

    #[test]
    fn exit_preparation_keeps_an_existing_explicit_disconnect_pending() {
        let mut panel = RuntimePanel::new();
        panel.busy = true;
        panel.disconnecting = true;
        panel.prepare_exit();
        assert!(panel.shutdown_pending());
        assert!(panel.busy);
    }

    #[test]
    fn acknowledged_shutdown_still_waits_for_and_reports_a_panicked_connection_worker() {
        let bridge = super::RuntimeBridge::start_worker(|commands| {
            let super::Command::Disconnect(reply) = commands.recv().unwrap() else {
                panic!("expected disconnect");
            };
            reply.send(Ok(())).unwrap();
            panic!("simulated connection cleanup panic");
        })
        .unwrap();
        assert_eq!(bridge.disconnect().unwrap_err(), "服务连接线程异常结束。");
        assert!(bridge.0.worker.lock().unwrap().is_none());
    }

    #[test]
    fn connection_shutdown_preserves_both_the_service_and_worker_failure() {
        let bridge = super::RuntimeBridge::start_worker(|commands| {
            let super::Command::Disconnect(reply) = commands.recv().unwrap() else {
                panic!("expected disconnect");
            };
            reply
                .send(Err("simulated service shutdown failure".into()))
                .unwrap();
            panic!("simulated connection cleanup panic");
        })
        .unwrap();
        let error = bridge.disconnect().unwrap_err();
        assert!(error.contains("simulated service shutdown failure"));
        assert!(error.contains("服务连接线程异常结束。"));
    }
    #[test]
    fn diagnostics_keep_unicode_boundaries_and_signal_truncation() {
        assert_eq!(limit_text("鼠标🎮", 3), "鼠标🎮");
        assert_eq!(
            limit_text("鼠标🎮abc", 3),
            "鼠标🎮\n…内容较长，后续部分已省略"
        );
    }

    #[test]
    fn incomplete_hid_and_query_failures_never_report_full_success() {
        for hid in [
            json!({"interfaces":[],"failures":[],"complete":false}),
            json!({"interfaces":[],"failures":[{"reason":"denied"}],"complete":true}),
            json!({"unexpected":[]}),
        ] {
            let readings = Readings {
                connected: true,
                hid: Some(Ok(hid)),
                ..Readings::default()
            };
            assert!(readings.status().contains("部分信息不可用"));
        }
        let readings = Readings {
            connected: true,
            version: Some(Err("unavailable".into())),
            ..Readings::default()
        };
        assert!(readings.status().contains("部分信息不可用"));
    }

    #[test]
    fn failed_shutdown_keeps_error_and_does_not_claim_clean_disconnect() {
        let mut panel = RuntimePanel::new();
        panel.busy = true;
        panel.readings = Readings {
            connected: true,
            hid: Some(Ok(json!({"interfaces":[],"failures":[],"complete":true}))),
            ..Readings::default()
        };
        panel.finish_disconnect(Err("shutdown timeout".into()));
        assert!(!panel.busy);
        assert!(!panel.readings.connected);
        assert!(panel.status.contains("未正常完成"));
        assert_eq!(panel.error.as_deref(), Some("shutdown timeout"));
        assert!(panel.readings.hid.is_some());
    }

    #[gpui_kit::test]
    fn partial_enumeration_is_visible_without_starting_a_service(cx: &mut TestAppContext) {
        use gpui_kit::test::TestWindowExt;
        cx.update(gpui_kit::init);
        let handle = cx.open_window(size(px(1000.), px(800.)), |window, cx| {
            let panel = cx.new(|_| {
                let mut panel = RuntimePanel::new();
                panel.readings = Readings {
                    connected: true,
                    usb: Some(Ok(json!({"devices":[{"vendor_id":5426,"product_id":3592}],"failures":[],"complete":true}))),
                    hid: Some(Ok(json!({"interfaces":[],"failures":[],"complete":false}))),
                    ..Readings::default()
                };
                panel.status = panel.readings.status().into();
                panel
            });
            Root::new(panel, window, cx)
        });
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(
                window
                    .find("runtime-status")
                    .label()
                    .unwrap()
                    .contains("部分信息不可用")
            );
            assert!(
                window
                    .find("runtime-hid-summary")
                    .label()
                    .unwrap()
                    .contains("枚举未完成")
            );
            assert!(
                window
                    .find("runtime-usb-summary")
                    .label()
                    .unwrap()
                    .contains("发现 1 项")
            );
            assert_eq!(window.find("runtime-disconnect").disabled(), Some(true));
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn connection_page_does_not_start_services_on_render(cx: &mut TestAppContext) {
        use gpui_kit::test::TestWindowExt;
        cx.update(gpui_kit::init);
        let handle = cx.open_window(size(px(1000.), px(800.)), |window, cx| {
            let panel = cx.new(|_| RuntimePanel::new());
            Root::new(panel, window, cx)
        });
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("runtime-connect").disabled(), Some(false));
            assert_eq!(window.find("runtime-disconnect").disabled(), Some(true));
            assert_eq!(
                window.find("runtime-status").label().as_deref(),
                Some("尚未连接")
            );
        })
        .unwrap();
    }
}
