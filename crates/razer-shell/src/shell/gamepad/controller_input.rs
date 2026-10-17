//! Retained controller hardware-event ownership follows the discovered physical
//! device. Preview products have no observed route and cannot start a listener.
use super::*;
use anyhow::{Context as _, ensure};
use razer_device::controller_input::{Event, Sample};
use razer_discovery::discovery::{ObservedDevice, ObservedTransport};
use razer_ipc::{ServiceClient, ServiceRequest};
use serde_json::Value;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender, SyncSender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

enum Update {
    Samples(Vec<Sample>),
    Notifications(Vec<Event>),
    Diagnostic(String),
}
pub(super) struct Session {
    owner: String,
    canceled: Arc<AtomicBool>,
    stop: Sender<()>,
    worker: Option<JoinHandle<()>>,
    _delivery: Task<()>,
}
pub(super) struct Cleanup {
    identity: String,
    worker: JoinHandle<()>,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.canceled.store(true, Ordering::Release);
        let _ = self.stop.send(());
    }
}
impl Session {
    fn retire(mut self) -> Option<JoinHandle<()>> {
        self.canceled.store(true, Ordering::Release);
        let _ = self.stop.send(());
        self.worker.take()
    }
    fn finish(self) {
        if let Some(worker) = self.retire() {
            let _ = worker.join();
        }
    }
}
fn supported(route: &ObservedDevice) -> bool {
    route.peer_product_id().is_none()
        && matches!(
            route.transport(),
            Some(ObservedTransport::Wired | ObservedTransport::Dongle)
        )
        && razer_device::controller_input::capability(route.product_id())
            .is_some_and(|source| source.accepts(route.physical_product_id()))
}
fn check_reply(
    reply: &Value,
    route: &ObservedDevice,
    owner: &str,
    expected: Option<&Value>,
) -> anyhow::Result<()> {
    ensure!(
        reply["owner_guid"] == owner && reply["product_id"] == route.product_id(),
        "Controller input owner changed"
    );
    let info = &reply["device_info"];
    ensure!(
        info["vendorId"] == 0x1532
            && info["productId"] == route.physical_product_id()
            && info["guid"] == owner
            && info["containerId"]
                .as_str()
                .is_some_and(|container| container.eq_ignore_ascii_case(route.container()))
            && info["usbInstanceId"]
                .as_str()
                .is_some_and(|instance| !instance.is_empty()),
        "Controller input physical identity differs"
    );
    ensure!(
        expected.is_none_or(|expected| expected == info),
        "Controller input USB instance changed"
    );
    Ok(())
}
fn diagnostic(updates: &SyncSender<Update>, message: String) {
    eprintln!("[controller-input] {message}");
    let _ = updates.try_send(Update::Diagnostic(message));
}
fn run(
    owner: &str,
    route: &ObservedDevice,
    canceled: &AtomicBool,
    stop: Receiver<()>,
    updates: &SyncSender<Update>,
) -> anyhow::Result<()> {
    // Current84910.Ke attempts registration four times, with1/2/3-second waits.
    // Each attempt fully releases its owned worker before a replacement starts.
    for attempt in 1..=4 {
        let mut registered = false;
        let result = run_attempt(owner, route, canceled, &stop, updates, &mut registered);
        match result {
            Ok(()) => return Ok(()),
            Err(error) if registered || attempt == 4 => return Err(error),
            Err(error) => diagnostic(
                updates,
                format!("Controller input registration attempt {attempt} failed: {error:#}"),
            ),
        }
        if canceled.load(Ordering::Acquire) {
            return Ok(());
        }
        match stop.recv_timeout(Duration::from_secs(attempt)) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
    unreachable!("fourth source registration attempt returns")
}
fn run_attempt(
    owner: &str,
    route: &ObservedDevice,
    canceled: &AtomicBool,
    stop: &Receiver<()>,
    updates: &SyncSender<Update>,
    registered: &mut bool,
) -> anyhow::Result<()> {
    ensure!(
        !canceled.load(Ordering::Acquire),
        "Controller input owner canceled before startup"
    );
    let mut client = ServiceClient::spawn()?;
    let mut device_info = None;
    let result = (|| {
        ensure!(
            !canceled.load(Ordering::Acquire),
            "Controller input owner canceled before registration"
        );
        let reply = client.request(ServiceRequest::ControllerInputStart {
            product_id: route.product_id(),
            physical_product_id: route.physical_product_id(),
            device_container_id: route.container().into(),
            owner_guid: owner.into(),
        })?;
        check_reply(&reply, route, owner, None)?;
        ensure!(
            reply["started"] == true,
            "Controller input registration was not acknowledged"
        );
        device_info = Some(reply["device_info"].clone());
        *registered = true;
        loop {
            if canceled.load(Ordering::Acquire) {
                break;
            }
            match stop.recv_timeout(Duration::from_millis(8)) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            if canceled.load(Ordering::Acquire) {
                break;
            }
            let reply = client.request(ServiceRequest::ControllerInputPoll {
                owner_guid: owner.into(),
            })?;
            check_reply(&reply, route, owner, device_info.as_ref())?;
            let samples: Vec<Sample> = serde_json::from_value(reply["samples"].clone())
                .context("Controller sample array is invalid")?;
            let notifications: Vec<Event> = serde_json::from_value(reply["notifications"].clone())
                .context("Controller notification array is invalid")?;
            if !canceled.load(Ordering::Acquire) {
                if !samples.is_empty() {
                    updates
                        .try_send(Update::Samples(samples))
                        .context("Controller input UI queue is unavailable or full")?;
                }
                if !notifications.is_empty() {
                    updates
                        .try_send(Update::Notifications(notifications))
                        .context("Controller notification UI queue is unavailable or full")?;
                }
            }
        }
        Ok(())
    })();
    if device_info.is_some() && !client.is_stopped() {
        let cleanup = (|| {
            let reply = client.request(ServiceRequest::ControllerInputStop {
                owner_guid: owner.into(),
            })?;
            check_reply(&reply, route, owner, device_info.as_ref())?;
            ensure!(
                reply["stopped"] == true,
                "Controller input cleanup was not acknowledged"
            );
            Ok::<_, anyhow::Error>(())
        })();
        if let Err(error) = cleanup {
            diagnostic(
                updates,
                format!("Controller input cleanup failed: {error:#}"),
            );
        }
    }
    if !client.is_stopped() {
        if let Err(error) = client.request(ServiceRequest::Shutdown) {
            diagnostic(
                updates,
                format!("Controller input service shutdown failed: {error:#}"),
            );
        }
    }
    result
}

impl AppShell {
    pub(super) fn install_controller_input_cleanup(&mut self, cx: &mut Context<Self>) {
        self.subscriptions.push(cx.on_app_quit(|this, cx| {
            let sessions = std::mem::take(&mut this.controller_inputs);
            let retired = std::mem::take(&mut this.controller_input_cleanup);
            for session in sessions.values() {
                session.canceled.store(true, Ordering::Release);
                let _ = session.stop.send(());
            }
            let cleanup = cx.background_executor().spawn(async move {
                for (_, session) in sessions {
                    session.finish();
                }
                for cleanup in retired {
                    let _ = cleanup.worker.join();
                }
            });
            async move { cleanup.await }
        }));
    }
    pub(super) fn cancel_controller_inputs(&mut self, cx: &mut Context<Self>) {
        for (identity, session) in std::mem::take(&mut self.controller_inputs) {
            if let Some(worker) = session.retire() {
                self.controller_input_cleanup
                    .push(Cleanup { identity, worker });
            }
        }
        for workspace in &self.devices {
            workspace.update(cx, |workspace, cx| {
                workspace.observe_controller_input(None, &[], cx)
            });
        }
        cx.notify();
    }
    pub(super) fn sync_controller_inputs(&mut self, cx: &mut Context<Self>) {
        let mut pending = Vec::new();
        for cleanup in std::mem::take(&mut self.controller_input_cleanup) {
            if cleanup.worker.is_finished() {
                let _ = cleanup.worker.join();
            } else {
                pending.push(cleanup);
            }
        }
        self.controller_input_cleanup = pending;
        if self.close_requested {
            return;
        }
        let desired = self
            .devices
            .iter()
            .filter_map(|workspace| {
                let device = workspace.read(cx).device(cx);
                let routes = self
                    .device_observations
                    .iter()
                    .filter(|route| route.matches(device) && supported(route))
                    .collect::<Vec<_>>();
                let [route] = routes.as_slice() else {
                    return None;
                };
                Some((
                    workspace.read(cx).identity(cx),
                    workspace.clone(),
                    (**route).clone(),
                ))
            })
            .collect::<Vec<_>>();
        for (identity, workspace, route) in desired {
            if self.controller_inputs.contains_key(&identity) {
                continue;
            }
            let revision = self.discovery_revision;
            let owner = uuid::Uuid::new_v4().to_string();
            let canceled = Arc::new(AtomicBool::new(false));
            let signal = canceled.clone();
            let (stop, stop_rx) = mpsc::channel();
            let (updates, updates_rx) = mpsc::sync_channel(64);
            let mut retired = Vec::new();
            let mut remaining = Vec::new();
            for cleanup in std::mem::take(&mut self.controller_input_cleanup) {
                if cleanup.identity == identity {
                    retired.push(cleanup.worker);
                } else {
                    remaining.push(cleanup);
                }
            }
            self.controller_input_cleanup = remaining;
            let retired = Arc::new(std::sync::Mutex::new(Some(retired)));
            let owned_retired = retired.clone();
            let id = owner.clone();
            let selected = route.clone();
            let worker = match thread::Builder::new()
                .name("razer-controller-input-owner".into())
                .spawn(move || {
                    for worker in owned_retired.lock().unwrap().take().unwrap_or_default() {
                        let _ = worker.join();
                    }
                    if let Err(error) = run(&id, &selected, &signal, stop_rx, &updates) {
                        if !signal.load(Ordering::Acquire) {
                            diagnostic(
                                &updates,
                                format!("Controller input observation failed: {error:#}"),
                            );
                        }
                    }
                }) {
                Ok(worker) => worker,
                Err(error) => {
                    for worker in retired.lock().unwrap().take().unwrap_or_default() {
                        self.controller_input_cleanup.push(Cleanup {
                            identity: identity.clone(),
                            worker,
                        });
                    }
                    self.status = format!("Controller input worker could not start: {error}");
                    cx.notify();
                    continue;
                }
            };
            let identity_copy = identity.clone();
            let id = owner.clone();
            let delivery = cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(8))
                        .await;
                    let mut received = Vec::new();
                    let disconnected = loop {
                        match updates_rx.try_recv() {
                            Ok(update) => received.push(update),
                            Err(mpsc::TryRecvError::Empty) => break false,
                            Err(mpsc::TryRecvError::Disconnected) => break true,
                        }
                    };
                    let current = this.update(cx, |this, cx| {
                        let current = !this.close_requested
                            && this.discovery_revision == revision
                            && this.devices.contains(&workspace)
                            && workspace.read(cx).identity(cx) == identity_copy
                            && this
                                .controller_inputs
                                .get(&identity_copy)
                                .is_some_and(|session| {
                                    session.owner == id && !session.canceled.load(Ordering::Acquire)
                                })
                            && this.device_observations.iter().any(|current| {
                                current.matches(workspace.read(cx).device(cx))
                                    && current.physical_product_id() == route.physical_product_id()
                                    && current.hid_node() == route.hid_node()
                                    && current.container().eq_ignore_ascii_case(route.container())
                                    && supported(current)
                            });
                        if !current {
                            if this
                                .controller_inputs
                                .get(&identity_copy)
                                .is_some_and(|session| session.owner == id)
                            {
                                if let Some(worker) = this
                                    .controller_inputs
                                    .remove(&identity_copy)
                                    .and_then(Session::retire)
                                {
                                    this.controller_input_cleanup.push(Cleanup {
                                        identity: identity_copy.clone(),
                                        worker,
                                    });
                                }
                                // The retiring owner may only clear its own
                                // observations. A replacement stream can already
                                // have delivered input to this workspace.
                                if this.devices.contains(&workspace)
                                    && workspace.read(cx).identity(cx) == identity_copy
                                {
                                    workspace.update(cx, |workspace, cx| {
                                        workspace.observe_controller_input(None, &[], cx)
                                    });
                                    cx.notify();
                                }
                            }
                            return false;
                        }
                        for update in received {
                            match update {
                                Update::Samples(samples) => {
                                    for sample in samples {
                                        workspace.update(cx, |workspace, cx| {
                                            workspace.observe_controller_input(
                                                Some(&sample),
                                                &[],
                                                cx,
                                            )
                                        });
                                    }
                                }
                                Update::Notifications(events) => {
                                    this.forward_controller_lighting_input(
                                        &identity_copy,
                                        &events,
                                        cx,
                                    );
                                    workspace.update(cx, |workspace, cx| {
                                        workspace.observe_controller_input(None, &events, cx)
                                    });
                                    // Dashboard cards read the same Device as the
                                    // product header; wake their retained owner.
                                    cx.notify();
                                }
                                Update::Diagnostic(message) => {
                                    this.status = message;
                                    cx.notify();
                                }
                            }
                        }
                        if disconnected {
                            workspace.update(cx, |workspace, cx| {
                                workspace.observe_controller_input(None, &[], cx)
                            });
                            cx.notify();
                        }
                        true
                    });
                    if disconnected || !matches!(current, Ok(true)) {
                        break;
                    }
                }
            });
            self.controller_inputs.insert(
                identity,
                Session {
                    owner,
                    canceled,
                    stop,
                    worker: Some(worker),
                    _delivery: delivery,
                },
            );
        }
    }
}
