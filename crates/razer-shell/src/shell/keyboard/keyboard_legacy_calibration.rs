//! Retained physical connection owner for current 740/746 magnetic calibration.
//! Modal unmount only releases capture; device-wide serial records outlive it.
use super::*;
use razer_pages::features::keyboard_products::{
    KeyboardActuationRequested, KeyboardLegacyCalibrationHostRequested,
    LegacyCalibrationHostCommand,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
};
#[path = "keyboard_legacy_calibration_worker.rs"]
mod worker;
use worker::{Command, Reply};
struct Owner {
    identity: String,
    route: String,
    valid: Arc<AtomicBool>,
    commands: mpsc::Sender<Command>,
    replies: mpsc::Receiver<Reply>,
    mounted: bool,
    host_active: bool,
    modal_epoch: u64,
    pending: BTreeSet<u64>,
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.valid.store(false, Ordering::Release);
    }
}
#[derive(Default)]
pub(super) struct State {
    owners: BTreeMap<EntityId, Owner>,
    workers: Vec<JoinHandle<()>>,
    retry: BTreeMap<EntityId, u64>,
    mounts: BTreeMap<EntityId, u64>,
    modal_epoch: u64,
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}
impl AppShell {
    pub(super) fn install_keyboard_legacy_calibration_cleanup(&mut self, cx: &mut Context<Self>) {
        self.subscriptions.push(cx.on_app_quit(|shell, cx| {
            shell.keyboard_legacy_calibration.owners.clear();
            shell.keyboard_legacy_calibration.mounts.clear();
            let workers = std::mem::take(&mut shell.keyboard_legacy_calibration.workers);
            let cleanup = cx.background_executor().spawn(async move {
                for worker in workers {
                    if worker.join().is_err() {
                        eprintln!("[keyboard magnetic calibration] worker panicked during shutdown")
                    }
                }
            });
            async move { cleanup.await }
        }));
    }
    fn calibration_host_active(
        &self,
        workspace: &Entity<ProductWorkspace>,
        window: &Window,
        cx: &Context<Self>,
    ) -> bool {
        use razer_model::host_window_focus::{query_focus, selected_focus};
        let selected = host_tabs::HostTabs::frame_name(&self.location);
        let frame =
            host_tabs::HostTabs::frame_name(&Location::Device(workspace.read(cx).identity(cx)));
        if window.is_window_active() {
            selected_focus("synapse", &frame, Some(&selected))
        } else {
            match host_tabs::host_window::status(window) {
                Ok(status) => query_focus("synapse", Some(&frame), status, Some(&selected)),
                Err(error) => {
                    eprintln!("[keyboard magnetic calibration] window status: {error:#}");
                    false
                }
            }
        }
    }
    fn calibration_diagnostic(&mut self, error: String, cx: &mut Context<Self>) {
        eprintln!("[keyboard magnetic calibration] {error}");
        self.status = format!("键盘校准：{error}");
        cx.notify();
    }
    pub(super) fn sync_keyboard_legacy_calibration(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.keyboard_legacy_calibration.mounts.retain(|id, _| {
            self.devices
                .iter()
                .any(|workspace| workspace.entity_id() == *id)
        });
        let candidates = self
            .devices
            .iter()
            .filter_map(|workspace| {
                let view = workspace.read(cx);
                let device = view.device(cx);
                if ![740, 746].contains(&device.product_id) {
                    return None;
                }
                let routes = self
                    .device_observations
                    .iter()
                    .filter(|route| {
                        route.matches(device)
                            && route.product_id() == route.physical_product_id()
                            && route.peer_product_id().is_none()
                            && matches!(
                                route.transport(),
                                Some(razer_discovery::discovery::ObservedTransport::Wired)
                            )
                    })
                    .collect::<Vec<_>>();
                let [observed] = routes.as_slice() else { return None };
                Some((
                    workspace.clone(),
                    view.identity(cx),
                    format!(
                        "{}:{}",
                        observed.container(),
                        observed.physical_product_id()
                    ),
                    (**observed).clone(),
                ))
            })
            .collect::<Vec<_>>();
        let stale = self
            .keyboard_legacy_calibration
            .owners
            .iter()
            .filter_map(|(id, owner)| {
                (!candidates.iter().any(|(workspace, identity, route, _)| {
                    workspace.entity_id() == *id
                        && *identity == owner.identity
                        && *route == owner.route
                }))
                .then_some(*id)
            })
            .collect::<Vec<_>>();
        for id in stale {
            self.keyboard_legacy_calibration.owners.remove(&id);
            if let Some(workspace) = self
                .devices
                .iter()
                .find(|workspace| workspace.entity_id() == id)
            {
                workspace.update(cx, |workspace, cx| {
                    workspace.observe_keyboard_legacy_calibration_capture(false, window, cx)
                });
            }
        }
        for (workspace, identity, route, observed) in candidates {
            let id = workspace.entity_id();
            if self.keyboard_legacy_calibration.owners.contains_key(&id)
                || self
                    .keyboard_legacy_calibration
                    .retry
                    .get(&id)
                    .is_some_and(|retry| *retry > now())
            {
                continue;
            }
            let valid = Arc::new(AtomicBool::new(true));
            let signal = valid.clone();
            let (commands, receiver) = mpsc::channel();
            let (sender, replies) = mpsc::channel();
            let url = format!("razer-local:keyboard-calibration/{id:?}");
            match thread::Builder::new()
                .name("keyboard-magnetic-calibration".into())
                .spawn(move || worker::run(observed, signal, receiver, sender, url))
            {
                Ok(worker) => {
                    self.keyboard_legacy_calibration.workers.push(worker);
                    let modal_epoch = self
                        .keyboard_legacy_calibration
                        .mounts
                        .get(&id)
                        .copied()
                        .unwrap_or(0);
                    let mounted = modal_epoch != 0;
                    let host_active =
                        mounted && self.calibration_host_active(&workspace, window, cx);
                    if mounted {
                        let _ = commands.send(Command::Mount(modal_epoch, host_active));
                    }
                    self.keyboard_legacy_calibration.owners.insert(
                        id,
                        Owner {
                            identity,
                            route,
                            valid,
                            commands,
                            replies,
                            mounted,
                            host_active,
                            modal_epoch,
                            pending: BTreeSet::new(),
                        },
                    );
                }
                Err(error) => {
                    self.keyboard_legacy_calibration
                        .retry
                        .insert(id, now().saturating_add(1_000));
                    self.calibration_diagnostic(error.to_string(), cx);
                }
            }
        }
        let focus = self
            .devices
            .iter()
            .filter(|workspace| {
                self.keyboard_legacy_calibration
                    .owners
                    .get(&workspace.entity_id())
                    .is_some_and(|owner| owner.mounted)
            })
            .map(|workspace| {
                (
                    workspace.entity_id(),
                    self.calibration_host_active(workspace, window, cx),
                )
            })
            .collect::<Vec<_>>();
        for (id, active) in focus {
            if let Some(owner) = self.keyboard_legacy_calibration.owners.get_mut(&id) {
                if owner.host_active != active {
                    owner.host_active = active;
                    let _ = owner.commands.send(Command::Active(active));
                }
            }
        }
        let mut replies = Vec::new();
        for (id, owner) in &self.keyboard_legacy_calibration.owners {
            loop {
                match owner.replies.try_recv() {
                    Ok(reply) => {
                        let stopped = matches!(&reply, Reply::Stopped);
                        replies.push((*id, reply));
                        if stopped {
                            break;
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        replies.push((*id, Reply::Stopped));
                        break;
                    }
                }
            }
        }
        for (id, reply) in replies {
            let reply = match reply {
                Reply::At(epoch, reply) => {
                    if self
                        .keyboard_legacy_calibration
                        .owners
                        .get(&id)
                        .is_none_or(|owner| owner.modal_epoch != epoch)
                    {
                        continue;
                    }
                    *reply
                }
                reply => reply,
            };
            let workspace = self
                .devices
                .iter()
                .find(|workspace| workspace.entity_id() == id)
                .cloned();
            match reply {
                Reply::At(_, _) => {}
                Reply::Capture(initialized) => {
                    if let Some(workspace) = workspace {
                        workspace.update(cx, |workspace, cx| {
                            workspace.observe_keyboard_legacy_calibration_capture(
                                initialized,
                                window,
                                cx,
                            )
                        });
                    }
                }
                Reply::Input(input) => {
                    if let Some(workspace) = workspace {
                        workspace.update(cx, |workspace, cx| {
                            workspace.observe_keyboard_legacy_calibration_input(&input, window, cx)
                        });
                    }
                }
                Reply::Foreground(name) => {
                    if let Some(workspace) = workspace {
                        let active = self.calibration_host_active(&workspace, window, cx);
                        workspace.update(cx, |workspace, cx| {
                            workspace
                                .observe_keyboard_legacy_calibration_foreground(&name, active, cx)
                        });
                    }
                }
                Reply::Messages(messages) => {
                    if let Some(workspace) = workspace {
                        for message in messages {
                            workspace.update(cx, |workspace, cx| {
                                workspace.observe_keyboard_legacy_calibration(
                                    &message.action,
                                    &message.payload,
                                    cx,
                                )
                            });
                        }
                    }
                }
                Reply::Finished(generation, result) => {
                    if let Some(owner) = self.keyboard_legacy_calibration.owners.get_mut(&id) {
                        owner.pending.remove(&generation);
                    }
                    if let Some(workspace) = workspace {
                        workspace.update(cx, |workspace, cx| {
                            workspace.finish_keyboard_actuation(generation, result, cx)
                        });
                    }
                }
                Reply::Diagnostic(error) => self.calibration_diagnostic(error, cx),
                Reply::Stopped => {
                    if let Some(owner) = self.keyboard_legacy_calibration.owners.remove(&id) {
                        if let Some(workspace) = &workspace {
                            for generation in &owner.pending {
                                workspace.update(cx, |workspace, cx| {
                                    workspace.finish_keyboard_actuation(
                                        *generation,
                                        Err("键盘校准通信已结束".into()),
                                        cx,
                                    )
                                });
                            }
                        }
                    }
                    self.keyboard_legacy_calibration
                        .retry
                        .insert(id, now().saturating_add(1_000));
                    if let Some(workspace) = workspace {
                        workspace.update(cx, |workspace, cx| {
                            workspace.observe_keyboard_legacy_calibration_capture(false, window, cx)
                        });
                    }
                }
            }
        }
        for worker in std::mem::take(&mut self.keyboard_legacy_calibration.workers) {
            if worker.is_finished() {
                if worker.join().is_err() {
                    self.calibration_diagnostic("校准通信线程异常退出".into(), cx);
                }
            } else {
                self.keyboard_legacy_calibration.workers.push(worker);
            }
        }
    }
    pub(super) fn request_keyboard_legacy_calibration(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: &KeyboardLegacyCalibrationHostRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let id = workspace.entity_id();
        if workspace.read(cx).device(cx).product_id != request.product_id
            || !matches!(request.product_id, 740 | 746)
        {
            return;
        }
        match request.command {
            LegacyCalibrationHostCommand::Mount => {
                self.keyboard_legacy_calibration.modal_epoch = self
                    .keyboard_legacy_calibration
                    .modal_epoch
                    .wrapping_add(1)
                    .max(1);
                self.keyboard_legacy_calibration
                    .mounts
                    .insert(id, self.keyboard_legacy_calibration.modal_epoch);
                if let Some(owner) = self.keyboard_legacy_calibration.owners.get_mut(&id) {
                    owner.modal_epoch = self.keyboard_legacy_calibration.modal_epoch;
                }
            }
            LegacyCalibrationHostCommand::Unmount => {
                self.keyboard_legacy_calibration.mounts.remove(&id);
            }
            _ => {}
        }
        self.sync_keyboard_legacy_calibration(window, cx);
        let active = self.calibration_host_active(&workspace, window, cx);
        let modal_epoch = self
            .keyboard_legacy_calibration
            .mounts
            .get(&id)
            .copied()
            .unwrap_or(0);
        let result = self
            .keyboard_legacy_calibration
            .owners
            .get_mut(&workspace.entity_id())
            .ok_or_else(|| "没有当前磁性校准键盘连接".to_owned())
            .and_then(|owner| {
                let command = match request.command {
                    LegacyCalibrationHostCommand::Mount => {
                        owner.mounted = true;
                        owner.host_active = active;
                        owner.modal_epoch = modal_epoch;
                        Command::Mount(modal_epoch, active)
                    }
                    LegacyCalibrationHostCommand::Initialize => Command::Initialize,
                    LegacyCalibrationHostCommand::Cleanup => Command::Cleanup,
                    LegacyCalibrationHostCommand::Unmount => {
                        owner.mounted = false;
                        Command::Unmount
                    }
                };
                owner
                    .commands
                    .send(command)
                    .map_err(|_| "校准通信线程已结束".to_owned())
            });
        if let Err(error) = result {
            self.calibration_diagnostic(error, cx);
        }
    }
    pub(super) fn submit_keyboard_legacy_calibration_intent(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: &KeyboardActuationRequested,
        cx: &mut Context<Self>,
    ) -> bool {
        if !matches!(
            request.payload()["type"].as_str(),
            Some(
                "ON_START_BOTTOM_ANALOG_CALIBRATION"
                    | "ON_START_TOP_ANALOG_CALIBRATION"
                    | "ON_VERIFY_BOTTOM_ANALOG_CALIBRATION"
                    | "ON_VERIFY_TOP_ANALOG_CALIBRATION"
                    | "ON_STOP_CALIBRATION"
            )
        ) {
            return false;
        }
        let result = self
            .keyboard_legacy_calibration
            .owners
            .get_mut(&workspace.entity_id())
            .ok_or_else(|| "没有当前磁性校准键盘连接".to_owned())
            .and_then(|owner| {
                owner
                    .commands
                    .send(Command::Intent(
                        request.generation(),
                        request.payload().clone(),
                    ))
                    .map_err(|_| "校准通信线程已结束".to_owned())?;
                owner.pending.insert(request.generation());
                Ok(())
            });
        if let Err(error) = result {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_keyboard_actuation(request.generation(), Err(error), cx)
            });
        }
        true
    }
}
