//! Retained Help reset sessions and product-specific local reset routing.
use super::*;
use anyhow::{Context as _, ensure};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_pages::features::HelpResetRequest;
use serde_json::Value;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
#[path = "help_hardware_reset.rs"]
pub(super) mod hardware;
#[path = "receiver_profile_save.rs"]
pub(super) mod local;
pub(super) struct Session {
    request: HelpResetRequest,
    canceled: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    _delivery: Option<Task<()>>,
}
impl Session {
    fn matches(&self, request: HelpResetRequest, canceled: &Arc<AtomicBool>) -> bool {
        self.request.same_operation(request) && Arc::ptr_eq(&self.canceled, canceled)
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.canceled.store(true, Ordering::Release);
    }
}

impl AppShell {
    fn retire_help_reset(&mut self, key: &str) {
        if let Some(mut session) = self.receiver_reset.remove(key) {
            session.canceled.store(true, Ordering::Release);
            if let Some(worker) = session.worker.take() {
                self.receiver_reset_cleanup.push(worker);
            }
        }
        let mut pending = Vec::new();
        for worker in self.receiver_reset_cleanup.drain(..) {
            if worker.is_finished() {
                let _ = worker.join();
            } else {
                pending.push(worker);
            }
        }
        self.receiver_reset_cleanup = pending;
    }
    pub(super) fn install_receiver_reset_cleanup(&mut self, _cx: &mut Context<Self>) {
        self.register_exit_cleanup(|this, cx| {
            let mut selections = std::mem::take(&mut this.receiver_profile_selections);
            let mut sessions = std::mem::take(&mut this.receiver_reset);
            let mut retired = std::mem::take(&mut this.receiver_reset_cleanup);
            for session in selections.values_mut() {
                session.canceled.store(true, Ordering::Release);
                if let Some(worker) = session.worker.take() {
                    retired.push(worker);
                }
            }
            for session in sessions.values_mut() {
                session.canceled.store(true, Ordering::Release);
                if let Some(worker) = session.worker.take() {
                    retired.push(worker);
                }
            }
            let cleanup = cx.background_executor().spawn(async move {
                for worker in retired {
                    let _ = worker.join();
                }
                drop(sessions);
                drop(selections);
            });
            async move { cleanup.await }
        });
    }
    pub(super) fn request_help_reset(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: HelpResetRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested || !self.devices.contains(&workspace) {
            return;
        }
        if request.is_audio_streams() {
            self.request_audio_stream_reset(workspace, request, window, cx);
            return;
        }
        if !workspace.read(cx).help_reset_matches(request, cx) {
            return;
        }
        if request.hardware_reset_message().is_some() {
            if workspace.read(cx).device(cx).product_id
                == razer_device::blackshark_reset::PRODUCT_ID
            {
                self.request_help_hardware_reset1398(workspace, request, window, cx);
                return;
            }
            // The current mounted firmware component submits ON_RESET_DEVICE
            // then {msgName:uiRequest,msgData:resetHardware}. Neither adapter
            // exists for these products; do not turn a UI draft into an ack.
            workspace.update(cx, |workspace, cx| {
                workspace.finish_help_reset(
                    request,
                    None,
                    Some("Current ON_RESET_DEVICE and resetHardware middleware adapters are not implemented; device was not reset".into()),
                    window,
                    cx,
                );
            });
            return;
        }
        if request.oled_factory_reset_message().is_some() {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_help_reset(
                    request,
                    None,
                    Some(if request.oled_source_clone_failure() {
                        "Current ON_ADD_PROFILE and ON_OLED_FACTORY_RESET_DEVICE adapters are not implemented; static ON_RESET_OLED initializer retains an uncloneable lazy function"
                    } else {
                        "Current ON_RESET_DEVICE and ON_OLED_FACTORY_RESET_DEVICE adapters are not implemented; device was not reset"
                    }.into()),
                    window,
                    cx,
                );
            });
            return;
        }
        if request.oled_reset_obm().is_some() {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_help_reset(
                    request,
                    None,
                    Some("Current OLED resetOled submission adapter is not implemented; device was not reset".into()),
                    window,
                    cx,
                );
            });
            return;
        }
        let key = workspace.read(cx).identity(cx);
        self.retire_help_reset(&key);
        let device = workspace.read(cx).snapshot(cx);
        if request.creates_default_profile() {
            // These mounted Help owners call addProfile, not a hardware-reset
            // action. Its unique name requires the real owner Redux profiles
            // and asynchronously obtained source computerName. Local UI drafts
            // are not that state and must never stand in for this submission.
            workspace.update(cx, |workspace, cx| {
                workspace.finish_help_reset(
                    request,
                    None,
                    Some("Current ON_ADD_PROFILE owner-state and persistence adapter is not implemented; no default profile was submitted".into()),
                    window,
                    cx,
                );
            });
            return;
        }
        if request.source_action() != Some("ON_RESET_DEVICE")
            || !matches!(device.product_id, 164 | 241)
        {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_help_reset(
                    request,
                    None,
                    Some(format!(
                        "{} 的设备提交适配尚未实现；未重置设备",
                        request.source_action().unwrap_or("Help reset")
                    )),
                    window,
                    cx,
                );
            });
            return;
        }
        self.request_receiver_profile(workspace, local::Operation::Help(request), window, cx);
    }
    pub(super) fn cancel_help_reset(
        &mut self,
        workspace: &Entity<ProductWorkspace>,
        request: HelpResetRequest,
        cx: &App,
    ) {
        let key = if request.is_audio_streams() {
            format!(
                "{}:audio-streams:{}:{}",
                workspace.read(cx).identity(cx),
                request.owner().as_u64(),
                request.generation()
            )
        } else {
            workspace.read(cx).identity(cx)
        };
        if self
            .receiver_reset
            .get(&key)
            .is_some_and(|session| session.request.same_operation(request))
        {
            self.retire_help_reset(&key);
        }
    }

    fn request_audio_stream_reset(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: HelpResetRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !workspace.read(cx).help_reset_matches(request, cx) {
            return;
        }
        let device = workspace.read(cx).device(cx);
        if device.product_id != 1342 {
            return;
        }
        let observations = self
            .device_observations
            .iter()
            .filter(|route| {
                route.matches(device)
                    && route.peer_product_id().is_none()
                    && matches!(
                        route.transport(),
                        Some(razer_discovery::discovery::ObservedTransport::Wired)
                    )
            })
            .collect::<Vec<_>>();
        let [observation] = observations.as_slice() else {
            workspace.update(cx, |view, cx| {
                view.finish_help_reset(
                    request,
                    None,
                    Some("Audio restart 缺少唯一的有线设备观察".into()),
                    window,
                    cx,
                )
            });
            return;
        };
        let observation = (**observation).clone();
        let container = device.device_container_id.clone();
        let revision = workspace.read(cx).connection_revision();
        let key = format!(
            "{}:audio-streams:{}:{}",
            workspace.read(cx).identity(cx),
            request.owner().as_u64(),
            request.generation()
        );
        self.retire_help_reset(&key);
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        let observed = observation.clone();
        let (sender, receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("audio-stream-reset-client".into())
            .spawn(move || {
                let result = super::audio_mixer::restart_audio_streams(&observation, &signal)
                    .map_err(|error| format!("{error:#}"));
                let _ = sender.send(result);
            });
        let worker = match worker {
            Ok(worker) => worker,
            Err(error) => {
                workspace.update(cx, |view, cx| {
                    view.finish_help_reset(
                        request,
                        None,
                        Some(format!("Audio restart 任务启动失败：{error}")),
                        window,
                        cx,
                    )
                });
                return;
            }
        };
        let delivery_key = key.clone();
        let delivery_signal = canceled.clone();
        let _delivery = cx.spawn_in(window, async move |shell, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                let result = match receiver.try_recv() {
                    Ok(result) => Some(result),
                    Err(mpsc::TryRecvError::Empty) => None,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        Some(Err("Audio restart 工作线程已结束，缺少回执".into()))
                    }
                };
                let terminal = result.is_some();
                let current = shell.update_in(cx, |shell, window, cx| {
                    if !shell
                        .receiver_reset
                        .get(&delivery_key)
                        .is_some_and(|session| session.matches(request, &delivery_signal))
                    {
                        return false;
                    }
                    let current = shell.devices.contains(&workspace)
                        && workspace.read(cx).connection_revision() == revision
                        && workspace.read(cx).device(cx).product_id == 1342
                        && workspace.read(cx).device(cx).device_container_id == container
                        && shell.device_observations.iter().any(|current| {
                            current.matches(workspace.read(cx).device(cx))
                                && current.transport() == observed.transport()
                                && current.physical_product_id() == observed.physical_product_id()
                                && current.peer_product_id() == observed.peer_product_id()
                                && current.hid_node() == observed.hid_node()
                        })
                        && workspace.read(cx).help_reset_matches(request, cx);
                    if !current {
                        shell.retire_help_reset(&delivery_key);
                        return false;
                    }
                    if let Some(result) = result {
                        shell.retire_help_reset(&delivery_key);
                        let error = result.err();
                        if let Some(error) = &error {
                            shell.status = error.clone();
                        }
                        // No profile replacement, audio-state readback, or
                        // local persistence is part of the source action.
                        workspace.update(cx, |view, cx| {
                            view.finish_help_reset(request, None, error, window, cx)
                        });
                    }
                    true
                });
                if current.is_err() {
                    delivery_signal.store(true, Ordering::Release);
                    let _ = shell.update(cx, |shell, _| {
                        if shell
                            .receiver_reset
                            .get(&delivery_key)
                            .is_some_and(|session| session.matches(request, &delivery_signal))
                        {
                            shell.retire_help_reset(&delivery_key);
                        }
                    });
                }
                if terminal || !matches!(current, Ok(true)) {
                    break;
                }
            }
        });
        self.receiver_reset.insert(
            key,
            Session {
                request,
                canceled,
                worker: Some(worker),
                _delivery: Some(_delivery),
            },
        );
    }
}
