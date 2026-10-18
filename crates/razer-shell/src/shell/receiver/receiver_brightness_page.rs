//! Current receiver Lighting events: local source profile then direct device
//! setting. Local persistence and the genuine device response stay separate.
use super::*;
use anyhow::ensure;
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_pages::features::{ReceiverBrightnessReadRequested, ReceiverBrightnessRequested};
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
pub(super) struct Session {
    generation: u64,
    canceled: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    _delivery: Option<Task<()>>,
}
impl Session {
    pub(in crate::shell) fn matches(&self, generation: u64, canceled: &Arc<AtomicBool>) -> bool {
        self.generation == generation && Arc::ptr_eq(&self.canceled, canceled)
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.canceled.store(true, Ordering::Release);
    }
}
struct Completion {
    response: Value,
    error: Option<String>,
}

fn execute_read(device: Device, cancel: &Arc<AtomicBool>) -> anyhow::Result<Completion> {
    let guard = || -> anyhow::Result<()> {
        ensure!(
            !cancel.load(Ordering::Acquire),
            "Receiver brightness read canceled"
        );
        Ok(())
    };
    guard()?;
    let mut client = ServiceClient::spawn()?;
    client.set_cancellation_guard(cancel.clone());
    let result = (|| -> anyhow::Result<Completion> {
        let path = super::receiver_pairing::route::command_path(
            &mut client,
            device.product_id,
            &device.device_container_id,
        )?;
        guard()?;
        let response = client.request(ServiceRequest::ReceiverBrightnessRead {
            path: path.clone(),
            device_container_id: device.device_container_id.clone(),
        })?;
        guard()?;
        ensure!(
            response["product_id"] == device.product_id
                && response["path"] == path
                && response["device_container_id"] == device.device_container_id,
            "Receiver brightness read belongs to a different owner"
        );
        let error = response["result"]["error"].as_str().map(str::to_owned);
        Ok(Completion { response, error })
    })();
    let shutdown = super::receiver_reset::local::shutdown(&mut client);
    match (result, shutdown) {
        (Ok(mut result), Err(error)) => {
            let cleanup = format!("Communication process cleanup failed: {error:#}");
            result.error = Some(
                result
                    .error
                    .map_or(cleanup.clone(), |error| format!("{error}; {cleanup}")),
            );
            Ok(result)
        }
        (Err(error), Err(cleanup)) => Err(anyhow::anyhow!(
            "{error:#}; worker shutdown failed: {cleanup:#}"
        )),
        (result, Ok(_)) => result,
    }
}
impl AppShell {
    pub(in crate::shell) fn retire_receiver_brightness(&mut self, key: &str) {
        if let Some(mut session) = self.receiver_brightness_page.remove(key) {
            session.canceled.store(true, Ordering::Release);
            if let Some(worker) = session.worker.take() {
                self.receiver_brightness_cleanup.push(worker);
            }
        }
        let mut pending = Vec::new();
        for worker in self.receiver_brightness_cleanup.drain(..) {
            if worker.is_finished() {
                let _ = worker.join();
            } else {
                pending.push(worker);
            }
        }
        self.receiver_brightness_cleanup = pending;
    }
    pub(in crate::shell) fn begin_receiver_brightness_profile(
        &mut self,
        key: String,
        generation: u64,
        canceled: Arc<AtomicBool>,
    ) {
        self.receiver_brightness_page.insert(
            key,
            Session {
                generation,
                canceled,
                worker: None,
                _delivery: None,
            },
        );
    }
    pub(in crate::shell) fn retain_receiver_brightness_worker(
        &mut self,
        key: &str,
        worker: JoinHandle<()>,
        delivery: Task<()>,
    ) {
        if let Some(session) = self.receiver_brightness_page.get_mut(key) {
            if let Some(previous) = session.worker.replace(worker) {
                self.receiver_brightness_cleanup.push(previous);
            }
            session._delivery = Some(delivery);
        } else {
            self.receiver_brightness_cleanup.push(worker);
        }
    }
    pub(super) fn install_receiver_brightness_cleanup(&mut self, _cx: &mut Context<Self>) {
        self.register_exit_cleanup(|this, cx| {
            let mut sessions = std::mem::take(&mut this.receiver_brightness_page);
            let mut workers = std::mem::take(&mut this.receiver_brightness_cleanup);
            for session in sessions.values_mut() {
                session.canceled.store(true, Ordering::Release);
                if let Some(worker) = session.worker.take() {
                    workers.push(worker);
                }
            }
            let cleanup = cx.background_executor().spawn(async move {
                for worker in workers {
                    let _ = worker.join();
                }
                drop(sessions);
            });
            async move { cleanup.await }
        });
    }
    pub(super) fn receiver_brightness_write(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: ReceiverBrightnessRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if workspace.read(cx).receiver_brightness_matches(
            request.generation(),
            Some(request.percent()),
            true,
            cx,
        ) {
            self.request_receiver_profile(
                workspace,
                super::receiver_reset::local::Operation::Brightness(request),
                window,
                cx,
            );
        }
    }
    pub(super) fn receiver_brightness_read(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: ReceiverBrightnessReadRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.receiver_brightness_request(workspace, request.generation(), None, window, cx);
    }
    fn receiver_brightness_request(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        generation: u64,
        request: Option<ReceiverBrightnessRequested>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let percent = request.map(|request| request.percent());
        if !workspace
            .read(cx)
            .receiver_brightness_matches(generation, percent, true, cx)
        {
            return;
        }
        let device = workspace.read(cx).snapshot(cx);
        let key = workspace.read(cx).identity(cx);
        self.retire_receiver_brightness(&key);
        let route = match super::device_discovery::receiver_route_for_owner(
            &self.device_observations,
            &device,
            &device.device_container_id,
            device.product_id,
        ) {
            Ok(route) => route,
            Err(error) => {
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_receiver_brightness(
                        generation,
                        percent,
                        None,
                        None,
                        None,
                        Some(format!("{error:#}")),
                        false,
                        window,
                        cx,
                    )
                });
                return;
            }
        };
        let revision = self.discovery_revision;
        let product = device.product_id;
        let container = device.device_container_id.clone();
        let profile = device.active_profile.clone();
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        let (sender, receiver) = mpsc::channel();
        let worker = match thread::Builder::new()
            .name("receiver-brightness-page".into())
            .spawn(move || {
                let _ = sender
                    .send(execute_read(device, &signal).map_err(|error| format!("{error:#}")));
            }) {
            Ok(worker) => worker,
            Err(error) => {
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_receiver_brightness(
                        generation,
                        percent,
                        None,
                        None,
                        None,
                        Some(error.to_string()),
                        false,
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
                        Some(Err("Brightness worker disconnected".into()))
                    }
                };
                let terminal = result.is_some();
                let keep = shell.update_in(cx, |shell, window, cx| {
                    if !shell
                        .receiver_brightness_page
                        .get(&delivery_key)
                        .is_some_and(|session| session.generation == generation)
                    {
                        return false;
                    }
                    let current = shell.devices.contains(&workspace)
                        && shell.discovery_revision == revision
                        && workspace.read(cx).device(cx).product_id == product
                        && workspace.read(cx).device(cx).device_container_id == container
                        && workspace.read(cx).device(cx).active_profile == profile
                        && shell.device_observations.iter().any(|observed| {
                            observed.matches(workspace.read(cx).device(cx))
                                && route.matches(observed)
                        })
                        && workspace
                            .read(cx)
                            .receiver_brightness_matches(generation, percent, true, cx);
                    if !current && result.is_none() {
                        if let Some(session) = shell.receiver_brightness_page.get(&delivery_key) {
                            session.canceled.store(true, Ordering::Release);
                        }
                        return true;
                    }
                    if let Some(result) = result {
                        shell.retire_receiver_brightness(&delivery_key);
                        let (outcome, submitted_requested, observed, error) = match result {
                            Ok(result) => {
                                // Source TaskRunner publishes its requested value after
                                // the pre-read/conditional setter. Keep that completion
                                // separate from an actual getter observation.
                                let submitted_requested = if percent.is_some()
                                    && result.response["result"]["source_completed"] == true
                                {
                                    result.response["result"]["requested_percent"]
                                        .as_u64()
                                        .and_then(|n| u8::try_from(n).ok())
                                } else {
                                    None
                                };
                                let observed = if percent.is_some() {
                                    result.response["result"]["observed"]["percent"].as_u64()
                                } else {
                                    result.response["result"]["percent"].as_u64()
                                }
                                .and_then(|n| u8::try_from(n).ok());
                                (None, submitted_requested, observed, result.error)
                            }
                            Err(error) => (None, None, None, Some(error)),
                        };
                        if current {
                            shell.status = error.clone().unwrap_or_else(|| {
                                match (submitted_requested, observed) {
                                    (Some(value), _) => format!("设备亮度原链提交完成：{value}%"),
                                    (None, Some(value)) => format!("设备实际亮度：{value}%"),
                                    (None, None) => "设备亮度未确认".into(),
                                }
                            });
                        }
                        workspace.update(cx, |workspace, cx| {
                            workspace.finish_receiver_brightness(
                                generation,
                                percent,
                                outcome,
                                submitted_requested,
                                observed,
                                error,
                                current,
                                window,
                                cx,
                            )
                        });
                        cx.notify();
                    }
                    true
                });
                if terminal || !matches!(keep, Ok(true)) {
                    break;
                }
            }
        });
        self.receiver_brightness_page.insert(
            key,
            Session {
                generation,
                canceled,
                worker: Some(worker),
                _delivery: Some(_delivery),
            },
        );
    }
}
