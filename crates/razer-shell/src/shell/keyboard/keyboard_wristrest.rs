//! PID653 hardware module lifetime belongs to the discovered physical device,
//! independently of profile drafts and the visible Customize page.
use super::*;
use anyhow::{Context as _, ensure};
use razer_device::keyboard_wristrest::{self as wristrest, Target, Update};
use razer_discovery::{
    direct,
    discovery::{ObservedDevice, ObservedTransport},
};
use razer_ipc::{ServiceClient, ServiceRequest};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub(super) struct Session {
    operation_id: String,
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

    fn finish(mut self) {
        self.canceled.store(true, Ordering::Release);
        let _ = self.stop.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn check_scope(reply: &serde_json::Value, id: &str, target: &Target) -> anyhow::Result<()> {
    ensure!(
        reply["operation_id"] == id && reply["target"] == serde_json::to_value(target)?,
        "Wristrest service reply differs from its reader scope"
    );
    Ok(())
}

fn diagnostic(updates: &Sender<Update>, message: String) {
    eprintln!("[keyboard-wristrest] {message}");
    let _ = updates.send(Update::Diagnostic { message });
}

fn run(
    id: &str,
    route: &ObservedDevice,
    canceled: &AtomicBool,
    stop: Receiver<()>,
    updates: &Sender<Update>,
) -> anyhow::Result<()> {
    ensure!(
        !canceled.load(Ordering::Acquire),
        "Wristrest owner canceled before service creation"
    );
    let mut client = ServiceClient::spawn()?;
    let mut active = None;
    let result = (|| {
        ensure!(
            !canceled.load(Ordering::Acquire),
            "Wristrest owner canceled before route resolution"
        );
        let cap =
            wristrest::capability(route.product_id()).context("Wristrest capability is absent")?;
        let target = direct::resolve(&mut client, route, cap)?.keyboard_wristrest_target()?;
        ensure!(
            !canceled.load(Ordering::Acquire),
            "Wristrest owner canceled before reader registration"
        );
        // Retain cleanup intent before dispatch: a malformed reply can follow
        // a successfully created reader and must still cause Stop/Shutdown.
        active = Some(target.clone());
        let started = client.request(ServiceRequest::KeyboardWristrestStart {
            operation_id: id.into(),
            target: target.clone(),
        })?;
        check_scope(&started, id, &target)?;
        ensure!(
            started["started"] == true,
            "Wristrest reader was not registered"
        );
        loop {
            if canceled.load(Ordering::Acquire) {
                break;
            }
            match stop.recv_timeout(Duration::from_millis(100)) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            let reply = client.request(ServiceRequest::KeyboardWristrestPoll {
                operation_id: id.into(),
            })?;
            check_scope(&reply, id, &target)?;
            let received: Vec<Update> = serde_json::from_value(reply["updates"].clone())
                .context("Wristrest update array is invalid")?;
            if !canceled.load(Ordering::Acquire) {
                for update in received {
                    updates
                        .send(update)
                        .context("Wristrest UI owner was released")?;
                }
            }
            let terminal = reply["terminal"]
                .as_bool()
                .context("Wristrest reader terminal flag is absent")?;
            if terminal {
                active = None;
                if let Some(error) = reply["error"].as_str() {
                    anyhow::bail!("Wristrest device observation failed: {error}");
                }
                break;
            }
        }
        Ok(())
    })();
    if let Some(target) = active {
        let cleanup = (|| {
            let reply = client.request(ServiceRequest::KeyboardWristrestStop {
                operation_id: id.into(),
            })?;
            check_scope(&reply, id, &target)?;
            ensure!(
                reply["stopped"] == true,
                "Wristrest reader cleanup was not confirmed"
            );
            if let Some(error) = reply["cleanup_error"].as_str() {
                anyhow::bail!("{error}");
            }
            Ok::<_, anyhow::Error>(())
        })();
        if let Err(error) = cleanup {
            diagnostic(
                updates,
                format!("Wristrest reader cleanup failed: {error:#}"),
            );
        }
    }
    if !client.is_stopped() {
        if let Err(error) = client.request(ServiceRequest::Shutdown) {
            diagnostic(
                updates,
                format!("Wristrest service shutdown failed: {error:#}"),
            );
        }
    }
    result
}

impl AppShell {
    pub(super) fn install_keyboard_wristrest_cleanup(&mut self, _cx: &mut Context<Self>) {
        self.register_exit_cleanup(|this, cx| {
            let sessions = std::mem::take(&mut this.keyboard_wristrest);
            let retired = std::mem::take(&mut this.keyboard_wristrest_cleanup);
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
        });
    }

    pub(super) fn cancel_keyboard_wristrest(&mut self, cx: &mut Context<Self>) {
        for (identity, session) in std::mem::take(&mut self.keyboard_wristrest) {
            if let Some(worker) = session.retire() {
                self.keyboard_wristrest_cleanup
                    .push(Cleanup { identity, worker });
            }
        }
        for workspace in &self.devices {
            workspace.update(cx, |workspace, cx| {
                workspace.observe_keyboard_wristrest(None, cx)
            });
        }
    }

    pub(super) fn sync_keyboard_wristrest(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut pending = Vec::new();
        for cleanup in std::mem::take(&mut self.keyboard_wristrest_cleanup) {
            if cleanup.worker.is_finished() {
                let _ = cleanup.worker.join();
            } else {
                pending.push(cleanup);
            }
        }
        self.keyboard_wristrest_cleanup = pending;
        let desired = self
            .devices
            .iter()
            .filter_map(|workspace| {
                let device = workspace.read(cx).device(cx);
                if device.product_id != wristrest::PRODUCT_ID {
                    return None;
                }
                let routes = self
                    .device_observations
                    .iter()
                    .filter(|route| {
                        route.matches(device)
                            && route.physical_product_id() == wristrest::PRODUCT_ID
                            && route.peer_product_id().is_none()
                            && matches!(route.transport(), Some(ObservedTransport::Wired))
                    })
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
            if self.keyboard_wristrest.contains_key(&identity) {
                continue;
            }
            let revision = self.discovery_revision;
            let operation_id = format!("wristrest-{}-{revision}", workspace.entity_id().as_u64());
            let canceled = Arc::new(AtomicBool::new(false));
            let signal = canceled.clone();
            let (stop, stop_rx) = mpsc::channel();
            let (updates, updates_rx) = mpsc::channel();
            let selected = route.clone();
            let id = operation_id.clone();
            let mut startup_cleanup = Vec::new();
            let mut remaining_cleanup = Vec::new();
            for cleanup in std::mem::take(&mut self.keyboard_wristrest_cleanup) {
                if cleanup.identity == identity {
                    startup_cleanup.push(cleanup.worker);
                } else {
                    remaining_cleanup.push(cleanup);
                }
            }
            self.keyboard_wristrest_cleanup = remaining_cleanup;
            let startup_cleanup = Arc::new(std::sync::Mutex::new(Some(startup_cleanup)));
            let owned_cleanup = startup_cleanup.clone();
            let worker = match thread::Builder::new()
                .name("razer-wristrest-owner".into())
                .spawn(move || {
                    // A discovery generation cannot overlap this device's old
                    // readers. Joining happens on the new worker, never in GPUI.
                    for worker in owned_cleanup.lock().unwrap().take().unwrap_or_default() {
                        let _ = worker.join();
                    }
                    if let Err(error) = run(&id, &selected, &signal, stop_rx, &updates) {
                        if !signal.load(Ordering::Acquire) {
                            diagnostic(&updates, format!("Wristrest reader failed: {error:#}"));
                        }
                    }
                }) {
                Ok(worker) => worker,
                Err(error) => {
                    for worker in startup_cleanup.lock().unwrap().take().unwrap_or_default() {
                        self.keyboard_wristrest_cleanup.push(Cleanup {
                            identity: identity.clone(),
                            worker,
                        });
                    }
                    self.status = format!("Wristrest reader could not start: {error}");
                    cx.notify();
                    continue;
                }
            };
            let owner = identity.clone();
            let id = operation_id.clone();
            let delivery = cx.spawn_in(window, async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(100))
                        .await;
                    let mut received = Vec::new();
                    let disconnected = loop {
                        match updates_rx.try_recv() {
                            Ok(update) => received.push(update),
                            Err(mpsc::TryRecvError::Empty) => break false,
                            Err(mpsc::TryRecvError::Disconnected) => break true,
                        }
                    };
                    let current = this.update_in(cx, |this, _, cx| {
                        if this.discovery_revision != revision
                            || !this.devices.contains(&workspace)
                            || workspace.read(cx).identity(cx) != owner
                            || !this.keyboard_wristrest.get(&owner).is_some_and(|session| {
                                session.operation_id == id
                                    && !session.canceled.load(Ordering::Acquire)
                            })
                            || !this.device_observations.iter().any(|current| {
                                current.matches(workspace.read(cx).device(cx))
                                    && current.physical_product_id() == route.physical_product_id()
                                    && current.peer_product_id() == route.peer_product_id()
                                    && current.hid_node() == route.hid_node()
                                    && current.container().eq_ignore_ascii_case(route.container())
                                    && matches!(current.transport(), Some(ObservedTransport::Wired))
                            })
                        {
                            if this
                                .keyboard_wristrest
                                .get(&owner)
                                .is_some_and(|session| session.operation_id == id)
                            {
                                if let Some(worker) = this
                                    .keyboard_wristrest
                                    .remove(&owner)
                                    .and_then(Session::retire)
                                {
                                    this.keyboard_wristrest_cleanup.push(Cleanup {
                                        identity: owner.clone(),
                                        worker,
                                    });
                                }
                            }
                            if this.devices.contains(&workspace) {
                                workspace.update(cx, |workspace, cx| {
                                    workspace.observe_keyboard_wristrest(None, cx)
                                });
                            }
                            return false;
                        }
                        for update in received {
                            match update {
                                Update::Observed { observation } => {
                                    workspace.update(cx, |workspace, cx| {
                                        workspace.observe_keyboard_wristrest(Some(observation), cx)
                                    })
                                }
                                Update::Diagnostic { message } => {
                                    this.status = message;
                                    cx.notify();
                                }
                            }
                        }
                        true
                    });
                    if disconnected || !matches!(current, Ok(true)) {
                        break;
                    }
                }
            });
            self.keyboard_wristrest.insert(
                identity,
                Session {
                    operation_id,
                    canceled,
                    stop,
                    worker: Some(worker),
                    _delivery: delivery,
                },
            );
        }
    }
}
