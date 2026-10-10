//! Current Help task orchestration: real serial -> source document -> local
//! persistence -> UI refresh. Device refresh gaps remain an explicit failure.
use super::*;
use anyhow::{Context as _, ensure};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_pages::features::{HelpResetOutcome, HelpResetRequest};
use serde_json::Value;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
pub(super) static DOCUMENT_LOCK: Mutex<()> = Mutex::new(());
pub(super) struct Session {
    request: HelpResetRequest,
    canceled: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    _delivery: Task<()>,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.canceled.store(true, Ordering::Release);
    }
}

fn reset(device: Device, canceled: &AtomicBool) -> anyhow::Result<(HelpResetOutcome, String)> {
    ensure!(
        matches!(device.product_id, 164 | 241) && device.real_product_id == device.product_id,
        "This Help reset has no current source-audited direct receiver owner"
    );
    ensure!(
        uuid::Uuid::parse_str(&device.device_container_id).is_ok_and(|id| !id.is_nil()),
        "Receiver container is not a real owner"
    );
    ensure!(
        matches!(
            device.dashboard.connection_observation,
            Some(
                razer_model::model::DeviceConnectionObservation::UsbPresent
                    | razer_model::model::DeviceConnectionObservation::HidPresent
            )
        ),
        "Receiver is not currently connected"
    );
    let guard = || -> anyhow::Result<()> {
        ensure!(
            !canceled.load(Ordering::Acquire),
            "Reset canceled before local submission"
        );
        Ok(())
    };
    guard()?;
    let mut client = ServiceClient::spawn()?;
    let result = (|| -> anyhow::Result<(HelpResetOutcome, String)> {
        let command_path = super::receiver_pairing::route::command_path(
            &mut client,
            device.product_id,
            &device.device_container_id,
        )?;
        let identity = client.request(ServiceRequest::ReceiverIdentityRead {
            path: command_path.clone(),
            device_container_id: device.device_container_id.clone(),
        })?;
        guard()?;
        ensure!(
            identity["product_id"] == device.product_id
                && identity["device_container_id"] == device.device_container_id,
            "Serial query returned a different receiver owner"
        );
        let serial = identity["serial_number"]
            .as_str()
            .filter(|value| !value.is_empty())
            .context("Device serial query returned no source serial")?;
        // The real command serial owns storage; saved UI serial is never a substitute.
        let path = razer_storage::receiver_reset::local_path(
            device.product_id,
            &device.device_container_id,
            serial,
        )?;
        let _lock = DOCUMENT_LOCK
            .lock()
            .map_err(|_| anyhow::anyhow!("Receiver document storage lock unavailable"))?;
        guard()?;
        let computer = std::env::var("COMPUTERNAME").unwrap_or_default();
        let document = match razer_storage::receiver_reset::load(
            &path,
            device.product_id,
            &device.device_container_id,
            serial,
        )? {
            Some(document) => document,
            None => {
                // Original startup's proven no-local-data branch. Do not copy
                // rendered source_settings, local-profile IDs or slider snapshots.
                razer_storage::receiver_reset::initial_document(
                    device.product_id,
                    device.edition_id,
                    serial,
                    &computer,
                    &uuid::Uuid::new_v4().to_string(),
                )?
            }
        };
        let cache = razer_storage::receiver_reset::prepare_serial_cache(
            device.product_id,
            device.edition_id,
            &document,
            serial,
            &computer,
            &uuid::Uuid::new_v4().to_string(),
        )?;
        let plan = razer_storage::receiver_reset::ResetPlan::prepare(
            device.product_id,
            device.edition_id,
            &cache,
            serial,
            &device.device_container_id,
            &computer,
            &uuid::Uuid::new_v4().to_string(),
        )?;
        guard()?;
        razer_storage::receiver_reset::persist(
            &path,
            device.product_id,
            &device.device_container_id,
            serial,
            &document,
            &plan,
        )?;
        // Source Fk queues brightness/effects/mappings. Record only real
        // responses; do not count local persistence as any of these acks.
        let brightness = (|| -> anyhow::Result<Value> {
            guard()?;
            let settings = &plan.new_profile()["brightness"];
            let percent = if settings["isEnabled"] == false {
                0
            } else {
                settings["value"]
                    .as_u64()
                    .filter(|value| *value <= 100)
                    .and_then(|value| u8::try_from(value).ok())
                    .context("Source default brightness is not a percent")?
            };
            let result = client.request(ServiceRequest::ReceiverBrightnessWrite {
                path: command_path.clone(),
                device_container_id: device.device_container_id.clone(),
                percent,
            })?;
            ensure!(
                result["product_id"] == device.product_id
                    && result["path"] == command_path
                    && result["device_container_id"] == device.device_container_id,
                "Brightness refresh returned a different receiver owner"
            );
            Ok(result)
        })();
        let mut message = match &brightness {
            Ok(result) if result["result"]["source_completed"] == true => {
                "默认配置已保存，原亮度提交链已完成；灯效和映射刷新尚未完成，不能确认全部重置。"
                    .to_owned()
            }
            Ok(result) => {
                let detail = result["result"]["error"]
                    .as_str()
                    .filter(|value| !value.is_empty());
                match detail {
                    Some(error) => format!(
                        "默认配置已保存；设备亮度尚未确认：{error}。灯效和映射刷新尚未完成。"
                    ),
                    None => "默认配置已保存；设备亮度尚未确认，灯效和映射刷新尚未完成。".to_owned(),
                }
            }
            Err(error) => {
                format!("默认配置已保存；设备亮度刷新失败：{error:#}。灯效和映射刷新尚未完成。")
            }
        };
        let receipt = serde_json::json!({"ON_INIT_BRIGHTNESS":match brightness{
            Ok(result)=>serde_json::json!({"response":result}),
            Err(error)=>serde_json::json!({"error":format!("{error:#}")}),
        },"ON_SET_EFFECTS":{"submitted":false},"ON_SET_KEYMAPPING":{"submitted":false}});
        if let Err(error) = razer_storage::receiver_reset::record_refresh(
            &path,
            device.product_id,
            &device.device_container_id,
            serial,
            plan.document(),
            receipt,
        ) {
            message.push_str(&format!(" 刷新结果未能保存：{error:#}"));
        }
        Ok((
            HelpResetOutcome {
                source_document: plan.document().clone(),
                serial_number: serial.to_owned(),
            },
            message,
        ))
    })();
    let shutdown = client.request(ServiceRequest::Shutdown);
    match (result, shutdown) {
        (Ok(value), Ok(_)) => Ok(value),
        (Err(error), Ok(_)) => Err(error),
        (Ok((document, mut message)), Err(error)) => {
            // Persistence and any device response already happened. Preserve
            // that partial outcome when cleanup fails instead of presenting a
            // false pre-submission failure or losing the saved UI state.
            message.push_str(&format!(" 服务清理失败：{error:#}"));
            Ok((document, message))
        }
        (Err(error), Err(cleanup)) => Err(anyhow::anyhow!(
            "{error:#}; worker shutdown failed: {cleanup:#}"
        )),
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
    pub(super) fn install_receiver_reset_cleanup(&mut self, cx: &mut Context<Self>) {
        self.subscriptions.push(cx.on_app_quit(|this, cx| {
            let mut sessions = std::mem::take(&mut this.receiver_reset);
            let mut retired = std::mem::take(&mut this.receiver_reset_cleanup);
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
            });
            async move { cleanup.await }
        }));
    }
    pub(super) fn request_help_reset(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: HelpResetRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if request.is_audio_streams() {
            self.request_audio_stream_reset(workspace, request, window, cx);
            return;
        }
        if !workspace.read(cx).help_reset_matches(request, cx) {
            return;
        }
        let key = workspace.read(cx).identity(cx);
        self.retire_help_reset(&key);
        let device = workspace.read(cx).snapshot(cx);
        let owner_product = device.product_id;
        let owner_container = device.device_container_id.clone();
        let route = device_discovery::receiver_route_for_owner(
            &self.device_observations,
            &device,
            &owner_container,
            owner_product,
        );
        let route = match route {
            Ok(route) => route,
            Err(error) => {
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_help_reset(
                        request,
                        None,
                        Some(format!("无法恢复默认配置：{error:#}")),
                        window,
                        cx,
                    )
                });
                return;
            }
        };
        let revision = self.discovery_revision;
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        let (sender, receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("receiver-reset-client".into())
            .spawn(move || {
                let result = reset(device, &signal).map_err(|error| format!("{error:#}"));
                let _ = sender.send(result);
            });
        let worker = match worker {
            Ok(worker) => worker,
            Err(error) => {
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_help_reset(
                        request,
                        None,
                        Some(format!("无法启动重置任务：{error}")),
                        window,
                        cx,
                    )
                });
                return;
            }
        };
        let delivery_key = key.clone();
        let delivery_workspace = workspace.clone();
        let _delivery = cx.spawn_in(window, async move |shell, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                let result = match receiver.try_recv() {
                    Ok(result) => Some(result),
                    Err(mpsc::TryRecvError::Empty) => None,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        Some(Err("重置工作线程已断开，无法确认结果。".into()))
                    }
                };
                let terminal = result.is_some();
                let current = shell.update_in(cx, |shell, window, cx| {
                    // Old delivery never removes a replacement session.
                    let owns = shell
                        .receiver_reset
                        .get(&delivery_key)
                        .is_some_and(|session| {
                            session.request.generation() == request.generation()
                        });
                    if !owns {
                        return false;
                    }
                    let current = shell.discovery_revision == revision
                        && shell.devices.contains(&delivery_workspace)
                        && delivery_workspace.read(cx).device(cx).product_id == owner_product
                        && delivery_workspace.read(cx).device(cx).device_container_id
                            == owner_container
                        && shell.device_observations.iter().any(|observed| {
                            observed.matches(delivery_workspace.read(cx).device(cx))
                                && route.matches(observed)
                        })
                        && delivery_workspace.read(cx).help_reset_matches(request, cx);
                    if !current {
                        shell.retire_help_reset(&delivery_key);
                        return false;
                    }
                    if let Some(result) = result {
                        shell.retire_help_reset(&delivery_key);
                        let (document, error) = match result {
                            Ok((document, gap)) => (Some(document), gap),
                            Err(error) => (None, format!("无法恢复默认配置：{error}")),
                        };
                        shell.status = error.clone();
                        delivery_workspace.update(cx, |workspace, cx| {
                            workspace.finish_help_reset(request, document, Some(error), window, cx)
                        });
                        cx.notify();
                    }
                    true
                });
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
                _delivery,
            },
        );
    }
    pub(super) fn cancel_help_reset(
        &mut self,
        workspace: &Entity<ProductWorkspace>,
        request: HelpResetRequest,
        cx: &App,
    ) {
        let key = if request.is_audio_streams() {
            format!(
                "{}:audio-streams:{}",
                workspace.read(cx).identity(cx),
                request.generation()
            )
        } else {
            workspace.read(cx).identity(cx)
        };
        if self
            .receiver_reset
            .get(&key)
            .is_some_and(|session| session.request.generation() == request.generation())
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
        let revision = self.discovery_revision;
        let key = format!(
            "{}:audio-streams:{}",
            workspace.read(cx).identity(cx),
            request.generation()
        );
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
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
                    if !shell.receiver_reset.contains_key(&delivery_key) {
                        return false;
                    }
                    let current = shell.discovery_revision == revision
                        && shell.devices.contains(&workspace)
                        && workspace.read(cx).device(cx).product_id == 1342
                        && workspace.read(cx).device(cx).device_container_id == container
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
                _delivery,
            },
        );
    }
}
