//! Current 190/229/239 MW owner: protocol serial, local profile, SET/UI/storage,
//! and hardware notifications. Getter receipts stay outside source UI state.
use super::*;
use anyhow::{Context as _, ensure};
use razer_device::mouse_indicator_led::{self as indicator, Change, Notification, Quality, Report};
use razer_discovery::{
    direct::mouse_indicator_led as direct,
    discovery::{ObservedDevice, ObservedTransport},
};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_model::model::PowerStatus;
use razer_pages::features::mouse_products::{
    MouseReceiverAction as Action, MouseReceiverIndicatorRequested as Request,
    MouseReceiverMonitorRequested as Monitor,
};
use razer_storage::{receiver_indicator as storage, source_local};
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
#[path = "mouse_receiver_legacy.rs"]
mod legacy;

enum Command {
    Request {
        request: Request,
        profile: Value,
        runtime: Vec<Value>,
    },
    Stop,
}
enum Update {
    Initialized(Request, Value),
    Finished(Request, Option<Report>),
    Quality(Quality),
    Battery(PowerStatus),
    Reinitialize,
    Diagnostic(String),
    LegacyReport(razer_device::mouse_receiver_legacy::Report),
    LegacyBindingInfo(Request, Value),
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum BatteryRead {
    Setup,
    Timer,
    Hardware,
    Wireless,
}
pub(super) struct Session {
    monitor: Monitor,
    operation_id: String,
    commands: Sender<Command>,
    pending: Vec<Request>,
    macro_exclusive: Arc<AtomicBool>,
    legacy_report: Option<razer_device::mouse_receiver_legacy::Report>,
    worker: Option<JoinHandle<()>>,
    _delivery: Task<()>,
}
pub(super) struct Cleanup {
    owner: EntityId,
    worker: JoinHandle<()>,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.monitor.guard().store(false, Ordering::Release);
        let _ = self.commands.send(Command::Stop);
    }
}
impl Session {
    fn retire(mut self) -> Option<JoinHandle<()>> {
        self.monitor.guard().store(false, Ordering::Release);
        let _ = self.commands.send(Command::Stop);
        self.worker.take()
    }
}
fn validate(guard: &AtomicBool) -> anyhow::Result<()> {
    ensure!(
        guard.load(Ordering::Acquire),
        "Receiver owner scope canceled"
    );
    Ok(())
}
fn diagnostic(updates: &Sender<Update>, error: impl std::fmt::Display) {
    let message = error.to_string();
    eprintln!("[mouse-receiver] {message}");
    let _ = updates.send(Update::Diagnostic(message));
}
fn supported(route: &ObservedDevice) -> bool {
    (indicator::capability(route.product_id())
        .is_some_and(|cap| cap.accepts(route.physical_product_id(), route.peer_product_id()))
        && matches!(
            route.transport(),
            Some(ObservedTransport::Wired | ObservedTransport::Dongle)
        ))
        || (razer_device::mouse_receiver_legacy::accepts(
            route.product_id(),
            route.physical_product_id(),
            route.peer_product_id(),
        ) && route.transport() == Some(ObservedTransport::Dongle))
        || (razer_device::mouse_receiver_legacy::accepts_change(
            route.product_id(),
            route.physical_product_id(),
            route.peer_product_id(),
            &razer_device::mouse_receiver_legacy::Change::ReadBindings,
        ) && matches!(
            route.transport(),
            Some(ObservedTransport::Wired | ObservedTransport::Dongle)
        ))
}
fn same_route(a: &ObservedDevice, b: &ObservedDevice) -> bool {
    a.product_id() == b.product_id()
        && a.physical_product_id() == b.physical_product_id()
        && a.peer_product_id() == b.peer_product_id()
        && a.hid_node() == b.hid_node()
        && a.container().eq_ignore_ascii_case(b.container())
        && a.transport() == b.transport()
        && a.connection() == b.connection()
}

struct Owner<'a> {
    id: &'a str,
    observed: &'a ObservedDevice,
    valid: &'a AtomicBool,
    updates: &'a Sender<Update>,
    route: direct::Route,
    path: PathBuf,
    serial: String,
    local_tasks: Vec<JoinHandle<()>>,
    connected: bool,
    battery_level: i32,
    battery_session: direct::BatterySession,
    last_battery_status: Option<String>,
    setup_battery_started: bool,
    external_power: Option<(Instant, bool)>,
    battery_tick: Option<Instant>,
}
impl Owner<'_> {
    fn finish_setup(&mut self, client: &mut ServiceClient) -> anyhow::Result<()> {
        if self.setup_battery_started {
            return Ok(());
        }
        let capability = indicator::capability(self.observed.product_id())
            .context("Receiver source is absent")?;
        // setupDevice's dongle serial gate precedes profile initialization.
        // A wired serial failure still reaches the battery setup. fe() catches
        // every indicator-init failure, including an absent local cache.
        if !capability.initialization_allowed(self.observed.physical_product_id(), &self.serial)? {
            return Ok(());
        }
        self.setup_battery_started = true;
        self.read_battery(client, BatteryRead::Setup)?;
        // START_CHECKING_BATTERY enqueues a task immediately, even when setup's
        // observation was off, and retains the interval after a failed getter.
        self.battery_tick = Some(Instant::now() + Duration::from_millis(150_000));
        self.read_battery(client, BatteryRead::Timer).map(|_| ())
    }
    fn update_battery_status(&mut self, status: &str) {
        // I() receives UPDATE_DEVICE_STATUS only while the source batt actor
        // exists. Repeated status strings do not reset its interval.
        if !self.setup_battery_started || self.last_battery_status.as_deref() == Some(status) {
            return;
        }
        self.last_battery_status = Some(status.into());
        match status {
            "off" => self.battery_tick = None,
            "Charging" | "NoCharge_BatteryFull" => {
                self.battery_tick = Some(Instant::now() + Duration::from_millis(150_000))
            }
            _ => {}
        }
    }
    fn read_battery(
        &mut self,
        client: &mut ServiceClient,
        cause: BatteryRead,
    ) -> anyhow::Result<bool> {
        validate(self.valid)?;
        let initial = cause == BatteryRead::Setup;
        let valid = self.valid;
        let battery = direct::read_battery_state_in_session(
            client,
            &self.route,
            &mut self.battery_session,
            !initial,
            || validate(valid),
        )?;
        let published = initial || battery.is_success;
        if published {
            self.battery_level = i32::from(battery.source_level);
            let charging_status = battery
                .charging_status
                .clone()
                .context("Receiver battery query omitted its source state")?;
            if !initial && matches!(cause, BatteryRead::Timer | BatteryRead::Wireless) {
                self.update_battery_status(&charging_status);
            }
            self.updates.send(Update::Battery(PowerStatus {
                level: self.battery_level,
                charging_status,
            }))?;
        }
        if let Some(error) = battery.error {
            diagnostic(self.updates, error);
        }
        Ok(published)
    }
    fn request(
        &mut self,
        client: &mut ServiceClient,
        request: &Request,
        profile: &Value,
    ) -> anyhow::Result<Option<Report>> {
        validate(self.valid)?;
        ensure!(request.is_valid(), "Receiver setting request was canceled");
        let capability = indicator::capability(self.observed.product_id())
            .context("Receiver source is absent")?;
        ensure!(
            capability.initialization_allowed(self.observed.physical_product_id(), &self.serial)?,
            "Receiver protocol serial query failed; source dongle initialization is gated"
        );
        let active = storage::ensure_cache(
            &self.path,
            self.observed.product_id(),
            &self.serial,
            profile,
        )?;
        let active = match active {
            Some(active) if active.profile["guid"] == profile["guid"] => Some(active),
            Some(_) => Some(storage::bind_local_profile(
                &self.path,
                self.observed.product_id(),
                &self.serial,
                profile,
            )?),
            None => None,
        };
        match request.action() {
            Action::Initialize => {
                // The source reads the active local profile; it never asks a
                // configuration getter to replace that profile or UI draft.
                let configuration = active
                    .as_ref()
                    .and_then(|active| active.profile.get("indicatorLedConfiguration"));
                let Some(configuration) = configuration else {
                    diagnostic(
                        self.updates,
                        "Active source profile has no indicatorLedConfiguration; initialization skipped",
                    );
                    return Ok(None);
                };
                let (configuration, bytes) =
                    indicator::prepare_source_configuration(configuration)?;
                let path = self.path.clone();
                let serial = self.serial.clone();
                let product = self.observed.product_id();
                let updates = self.updates.clone();
                let publish = self.updates;
                let tasks = &mut self.local_tasks;
                let valid = self.valid;
                let report = direct::initialize_raw(client, &self.route, bytes, |_| {
                    validate(valid)?;
                    ensure!(
                        request.is_valid(),
                        "Receiver init owner canceled after SET acknowledgement"
                    );
                    publish.send(Update::Initialized(request.clone(), configuration.clone()))?;
                    // MW deliberately does not await this local write before
                    // querying quality. Retain it for error reporting/cleanup.
                    tasks.push(
                        thread::Builder::new()
                            .name("razer-receiver-profile-save".into())
                            .spawn(move || {
                                if let Err(error) = storage::save_initialization(
                                    &path,
                                    product,
                                    &serial,
                                    &configuration,
                                ) {
                                    diagnostic(
                                        &updates,
                                        format!("Receiver init local persistence: {error:#}"),
                                    );
                                }
                            })?,
                    );
                    Ok(())
                })?;
                Ok(Some(report))
            }
            Action::Set { configuration } => {
                // Optimistic source UI is already committed. A failed storage
                // or device operation reports diagnostics without rolling it back.
                storage::save_user_selection(
                    &self.path,
                    self.observed.product_id(),
                    &self.serial,
                    configuration,
                )?;
                validate(self.valid)?;
                ensure!(
                    request.is_valid(),
                    "Receiver setter canceled before device access"
                );
                let (_, bytes) = indicator::prepare_source_configuration(configuration)?;
                Ok(Some(direct::execute(
                    client,
                    &self.route,
                    Change::SetRaw(bytes),
                )?))
            }
            Action::LegacyInitialize | Action::LegacySet { .. } | Action::LegacyBindingInfo => {
                anyhow::bail!("Single receiver LED action reached the three-LED owner")
            }
        }
    }
    fn notification(
        &mut self,
        client: &mut ServiceClient,
        notification: Notification,
    ) -> anyhow::Result<()> {
        match notification {
            Notification::ConnectionQuality { quality } if self.connected => {
                self.updates.send(Update::Quality(quality))?;
            }
            Notification::BatteryChanged {
                external_power_removed,
            } if self.connected => {
                if self.read_battery(client, BatteryRead::Hardware)? && external_power_removed {
                    self.external_power =
                        Some((Instant::now() + Duration::from_millis(500), false));
                }
            }
            Notification::ExternalPowerChanged { connected } if self.connected => {
                self.external_power =
                    Some((Instant::now() + Duration::from_millis(500), connected));
            }
            Notification::WirelessChanged {
                connected,
                product_id,
            } => {
                let dongle = indicator::capability(self.observed.product_id())
                    .context("Receiver source is absent")?
                    .dongle_id();
                if connected && product_id.is_some_and(|product| product != 0 && product != dongle)
                {
                    return Ok(());
                }
                self.connected = connected;
                if connected {
                    if self.serial.is_empty() {
                        let serial = direct::read_serial_number(client, &self.route)?;
                        self.serial = serial.runtime_serial_number;
                        if let Some(error) = serial.error {
                            diagnostic(self.updates, error);
                        }
                        if !self.serial.is_empty() {
                            self.updates.send(Update::Reinitialize)?;
                        }
                        // The no-serial branch delegates to setupOnDeviceReady;
                        // it does not also take the ready-serial battery path.
                        return Ok(());
                    }
                    self.read_battery(client, BatteryRead::Wireless)?;
                } else {
                    self.external_power = None;
                    self.update_battery_status("off");
                    self.updates.send(Update::Battery(PowerStatus {
                        level: self.battery_level,
                        charging_status: "off".into(),
                    }))?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn run(
    id: &str,
    observed: &ObservedDevice,
    guard: Arc<AtomicBool>,
    macro_exclusive: Arc<AtomicBool>,
    commands: Receiver<Command>,
    updates: &Sender<Update>,
) -> anyhow::Result<()> {
    if razer_device::mouse_receiver_legacy::capability(observed.product_id()).is_some() {
        return legacy::run(observed, guard, macro_exclusive, commands, updates);
    }
    validate(&guard)?;
    let mut client = ServiceClient::spawn()?;
    client.set_request_guard(guard.clone());
    let mut registered = false;
    let mut local_tasks = Vec::new();
    let result = (|| {
        let route = direct::resolve(&mut client, observed)?;
        let serial = direct::read_serial_number(&mut client, &route)?;
        if let Some(error) = serial.error {
            diagnostic(updates, error);
        }
        validate(&guard)?;
        // Preserve cleanup intent even if the Start reply is malformed.
        registered = true;
        direct::start(&mut client, &route, id.into())?;
        let mut owner = Owner {
            id,
            observed,
            valid: &guard,
            updates,
            route,
            path: source_local::path(),
            serial: serial.runtime_serial_number,
            local_tasks: Vec::new(),
            connected: true,
            battery_level: 0,
            battery_session: direct::BatterySession::default(),
            last_battery_status: None,
            setup_battery_started: false,
            external_power: None,
            battery_tick: None,
        };
        let mut next_poll = Instant::now();
        let result = (|| {
            loop {
                if validate(&guard).is_err() {
                    break;
                }
                match commands.recv_timeout(Duration::from_millis(20)) {
                    Ok(Command::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Ok(Command::Request {
                        request, profile, ..
                    }) => {
                        let initialization = matches!(request.action(), Action::Initialize);
                        let result = owner.request(&mut client, &request, &profile);
                        let report = match result {
                            Ok(report) => report,
                            Err(error) => {
                                diagnostic(updates, format!("Receiver request: {error:#}"));
                                None
                            }
                        };
                        updates.send(Update::Finished(request, report))?;
                        if initialization {
                            // Source fe() swallows feature-init failures; its
                            // later battery initialization still runs.
                            if let Err(error) = owner.finish_setup(&mut client) {
                                diagnostic(updates, format!("Receiver battery setup: {error:#}"));
                            }
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
                if owner
                    .external_power
                    .is_some_and(|(deadline, _)| deadline <= Instant::now())
                {
                    let (_, connected) = owner.external_power.take().unwrap();
                    // Current MW sends this action, but the current UI only
                    // handles SET_CHARGING_MODE (without MW_). Preserve the
                    // unhandled action as diagnostics; do not invent a reducer
                    // update. Its subsequent battery getter does publish.
                    diagnostic(
                        updates,
                        format!(
                            "MW_SET_CHARGING_MODE: {}",
                            if connected { "DOCK" } else { "NONE" }
                        ),
                    );
                    owner.read_battery(&mut client, BatteryRead::Hardware)?;
                }
                if owner
                    .battery_tick
                    .is_some_and(|deadline| deadline <= Instant::now())
                {
                    owner.battery_tick = Some(Instant::now() + Duration::from_millis(150_000));
                    owner.read_battery(&mut client, BatteryRead::Timer)?;
                }
                if registered && next_poll <= Instant::now() {
                    let batch = direct::poll(&mut client, owner.id)?;
                    next_poll = Instant::now() + Duration::from_millis(100);
                    for notification in batch.notifications {
                        owner.notification(&mut client, notification)?;
                    }
                    if batch.terminal {
                        registered = false;
                        if let Some(error) = batch.error {
                            diagnostic(updates, error);
                        }
                    }
                }
                if client.is_stopped() {
                    anyhow::bail!("Receiver service stopped");
                }
            }
            Ok::<_, anyhow::Error>(())
        })();
        local_tasks = std::mem::take(&mut owner.local_tasks);
        result
    })();
    // Only cleanup requests may outlive the source scope. Let them release the
    // existing registration even when the UI guard has already been revoked.
    client.set_request_guard(Arc::new(AtomicBool::new(true)));
    if registered && !client.is_stopped() {
        if let Err(error) = direct::stop(&mut client, id) {
            diagnostic(updates, format!("Receiver notification cleanup: {error:#}"));
        }
    }
    if !client.is_stopped() {
        if let Err(error) = client.request(ServiceRequest::Shutdown) {
            diagnostic(updates, format!("Receiver service cleanup: {error:#}"));
        }
    }
    for task in local_tasks {
        if task.join().is_err() {
            diagnostic(updates, "Receiver local persistence worker terminated");
        }
    }
    result
}

impl AppShell {
    pub(super) fn install_mouse_receiver_indicator_cleanup(&mut self, cx: &mut Context<Self>) {
        self.subscriptions.push(cx.on_app_quit(|this, cx| {
            let sessions = std::mem::take(&mut this.mouse_receiver_indicator);
            let mut retired = std::mem::take(&mut this.mouse_receiver_indicator_cleanup);
            for (owner, session) in sessions {
                if let Some(worker) = session.retire() {
                    retired.push(Cleanup { owner, worker });
                }
            }
            let cleanup = cx.background_executor().spawn(async move {
                for cleanup in retired {
                    let _ = cleanup.worker.join();
                }
            });
            async move { cleanup.await }
        }));
    }
    pub(super) fn cancel_mouse_receiver_indicator(&mut self) {
        for (owner, session) in std::mem::take(&mut self.mouse_receiver_indicator) {
            if let Some(worker) = session.retire() {
                self.mouse_receiver_indicator_cleanup
                    .push(Cleanup { owner, worker });
            }
        }
    }
    pub(super) fn start_mouse_receiver_indicator(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        monitor: Monitor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested || !workspace.read(cx).receiver_monitor_matches(&monitor, cx) {
            return;
        }
        let owner = workspace.entity_id();
        if let Some(old) = self
            .mouse_receiver_indicator
            .remove(&owner)
            .and_then(Session::retire)
        {
            self.mouse_receiver_indicator_cleanup
                .push(Cleanup { owner, worker: old });
        }
        let device = workspace.read(cx).device(cx);
        let routes = self
            .device_observations
            .iter()
            .filter(|route| route.matches(device) && supported(route))
            .cloned()
            .collect::<Vec<_>>();
        let [route] = routes.as_slice() else {
            eprintln!(
                "[mouse-receiver] No unique current physical route for product {}",
                device.product_id
            );
            return;
        };
        let route = route.clone();
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let operation_id = format!(
            "mouse-receiver-{}-{}-{}",
            owner.as_u64(),
            self.discovery_revision,
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let (commands, command_rx) = mpsc::channel();
        let (updates, update_rx) = mpsc::channel();
        let mut earlier = Vec::new();
        let mut pending = Vec::new();
        for cleanup in std::mem::take(&mut self.mouse_receiver_indicator_cleanup) {
            if cleanup.owner == owner {
                earlier.push(cleanup.worker);
            } else if cleanup.worker.is_finished() {
                let _ = cleanup.worker.join();
            } else {
                pending.push(cleanup);
            }
        }
        self.mouse_receiver_indicator_cleanup = pending;
        let earlier = Arc::new(Mutex::new(Some(earlier)));
        let owned_earlier = earlier.clone();
        let id = operation_id.clone();
        let selected = route.clone();
        let guard = monitor.guard();
        let macro_exclusive = Arc::new(AtomicBool::new(self.receiver_macro_exclusive(cx)));
        let worker_macro_exclusive = macro_exclusive.clone();
        let worker = match thread::Builder::new()
            .name("razer-mouse-receiver-owner".into())
            .spawn(move || {
                for worker in owned_earlier.lock().unwrap().take().unwrap_or_default() {
                    let _ = worker.join();
                }
                if let Err(error) = run(
                    &id,
                    &selected,
                    guard,
                    worker_macro_exclusive,
                    command_rx,
                    &updates,
                ) {
                    diagnostic(&updates, format!("Receiver owner: {error:#}"));
                }
            }) {
            Ok(worker) => worker,
            Err(error) => {
                for worker in earlier.lock().unwrap().take().unwrap_or_default() {
                    self.mouse_receiver_indicator_cleanup
                        .push(Cleanup { owner, worker });
                }
                eprintln!("[mouse-receiver] Cannot start owner: {error}");
                return;
            }
        };
        let id = operation_id.clone();
        let selected_monitor = monitor.clone();
        let delivery = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(20))
                    .await;
                let received = update_rx.try_iter().collect::<Vec<_>>();
                let current = this.update_in(cx, |this, window, cx| {
                    let current = this.devices.contains(&workspace)
                        && workspace
                            .read(cx)
                            .receiver_monitor_matches(&selected_monitor, cx)
                        && this
                            .device_observations
                            .iter()
                            .any(|current| same_route(current, &route))
                        && this
                            .mouse_receiver_indicator
                            .get(&owner)
                            .is_some_and(|session| session.operation_id == id);
                    if !current {
                        if this
                            .mouse_receiver_indicator
                            .get(&owner)
                            .is_some_and(|session| session.operation_id == id)
                        {
                            if let Some(worker) = this
                                .mouse_receiver_indicator
                                .remove(&owner)
                                .and_then(Session::retire)
                            {
                                this.mouse_receiver_indicator_cleanup
                                    .push(Cleanup { owner, worker });
                            }
                        }
                        return false;
                    }
                    for update in received {
                        match update {
                            Update::Initialized(request, configuration) => {
                                workspace.update(cx, |workspace, cx| {
                                    workspace.observe_receiver_initialization(
                                        &request,
                                        configuration,
                                        window,
                                        cx,
                                    )
                                })
                            }
                            Update::Finished(request, report) => {
                                if let Some(session) = this.mouse_receiver_indicator.get_mut(&owner)
                                {
                                    session
                                        .pending
                                        .retain(|pending| pending.sequence() != request.sequence());
                                }
                                workspace.update(cx, |workspace, cx| {
                                    workspace.finish_receiver_indicator(&request, report, cx)
                                });
                            }
                            Update::Quality(quality) => workspace.update(cx, |workspace, cx| {
                                workspace.observe_receiver_quality(&selected_monitor, quality, cx)
                            }),
                            Update::Battery(battery) => {
                                workspace.update(cx, |workspace, cx| {
                                    workspace.observe_receiver_battery(
                                        &selected_monitor,
                                        battery,
                                        cx,
                                    )
                                });
                                cx.notify();
                            }
                            Update::Reinitialize => workspace.update(cx, |workspace, cx| {
                                workspace.restart_receiver_initialization(&selected_monitor, cx)
                            }),
                            Update::Diagnostic(message) => {
                                eprintln!("[mouse-receiver] {message}");
                            }
                            Update::LegacyReport(report) => {
                                if let Some(session) = this.mouse_receiver_indicator.get_mut(&owner)
                                {
                                    session.legacy_report = Some(report);
                                }
                            }
                            Update::LegacyBindingInfo(request, paired) => {
                                if workspace
                                    .read(cx)
                                    .receiver_indicator_request_matches(&request, cx)
                                {
                                    let device = workspace.read(cx).device(cx).clone();
                                    workspace.update(cx, |workspace, cx| {
                                        workspace.observe_mouse_customize(
                                            &device,
                                            &serde_json::json!({"pairedInfo":paired}),
                                            cx,
                                        );
                                    });
                                }
                            }
                        }
                    }
                    let finished = this
                        .mouse_receiver_indicator
                        .get(&owner)
                        .and_then(|session| session.worker.as_ref())
                        .is_some_and(JoinHandle::is_finished);
                    if finished {
                        if let Some(mut session) = this.mouse_receiver_indicator.remove(&owner) {
                            for request in std::mem::take(&mut session.pending) {
                                workspace.update(cx, |workspace, cx| {
                                    workspace.finish_receiver_indicator(&request, None, cx)
                                });
                            }
                            if let Some(worker) = session.retire() {
                                let _ = worker.join();
                            }
                        }
                    }
                    !finished
                });
                if !matches!(current, Ok(true)) {
                    break;
                }
            }
        });
        self.mouse_receiver_indicator.insert(
            owner,
            Session {
                monitor,
                operation_id,
                commands,
                pending: Vec::new(),
                macro_exclusive,
                legacy_report: None,
                worker: Some(worker),
                _delivery: delivery,
            },
        );
    }
    pub(super) fn sync_receiver_macro_access(&mut self, cx: &App) {
        self.refresh_macro_recorder_storage();
        let exclusive = self.receiver_macro_exclusive(cx);
        for session in self.mouse_receiver_indicator.values() {
            session.macro_exclusive.store(exclusive, Ordering::Release);
        }
    }
    pub(super) fn receiver_runtime_layouts(&self, cx: &App) -> Vec<Value> {
        self.devices.iter().filter_map(|workspace| {
            let workspace=workspace.read(cx);let device=workspace.device(cx);
            device.dashboard.sidepad_layout.as_ref().map(|sidepad|serde_json::json!({"productId":device.product_id,"deviceContainerId":device.device_container_id,"sidepadLayout":sidepad}))
        }).collect()
    }
    pub(super) fn submit_mouse_receiver_indicator(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: Request,
        cx: &mut Context<Self>,
    ) {
        self.sync_receiver_macro_access(cx);
        if self.close_requested
            || !workspace
                .read(cx)
                .receiver_indicator_request_matches(&request, cx)
        {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_receiver_indicator(&request, None, cx)
            });
            return;
        }
        let mut profile = request.profile().clone();
        if !matches!(request.action(), Action::LegacyBindingInfo) {
            let device = workspace.read(cx).device(cx);
            let Some(active) = device
                .profiles
                .iter()
                .find(|profile| profile.id == device.active_profile)
            else {
                eprintln!("[mouse-receiver] Active local profile is absent");
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_receiver_indicator(&request, None, cx)
                });
                return;
            };
            profile["guid"] = serde_json::json!(active.guid);
            profile["name"] = serde_json::json!(active.name);
        }
        let runtime = self.receiver_runtime_layouts(cx);
        let dispatched = self
            .mouse_receiver_indicator
            .get_mut(&workspace.entity_id())
            .filter(|session| session.monitor.matches_scope(request.scope()))
            .is_some_and(|session| {
                if session
                    .commands
                    .send(Command::Request {
                        request: request.clone(),
                        profile,
                        runtime,
                    })
                    .is_err()
                {
                    return false;
                }
                session.pending.push(request.clone());
                true
            });
        if !dispatched {
            eprintln!("[mouse-receiver] Setting has no live source receiver owner");
            workspace.update(cx, |workspace, cx| {
                workspace.finish_receiver_indicator(&request, None, cx)
            });
        }
    }
}
