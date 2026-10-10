//! Current receiver Lighting events: local source profile then direct device
//! setting. Local persistence and the genuine device response stay separate.
use super::*;
use anyhow::{Context as _, ensure};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_pages::features::{
    HelpResetOutcome, ReceiverBrightnessReadRequested, ReceiverBrightnessRequested,
};
use serde_json::{Value, json};
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
    _delivery: Task<()>,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.canceled.store(true, Ordering::Release);
    }
}
struct Completion {
    outcome: Option<HelpResetOutcome>,
    response: Value,
    error: Option<String>,
}
fn execute(
    device: Device,
    request: Option<ReceiverBrightnessRequested>,
    cancel: &AtomicBool,
) -> anyhow::Result<Completion> {
    let guard = || -> anyhow::Result<()> {
        ensure!(
            !cancel.load(Ordering::Acquire),
            "Receiver brightness page scope canceled"
        );
        Ok(())
    };
    guard()?;
    let mut client = ServiceClient::spawn()?;
    let result = (|| -> anyhow::Result<Completion> {
        let path = super::receiver_pairing::route::command_path(
            &mut client,
            device.product_id,
            &device.device_container_id,
        )?;
        guard()?;
        let mut outcome = None;
        let mut storage = None;
        // Editing follows taskMakerSetBrightness: load real serial-owned
        // document, mutate active brightness, persist, then enqueue device.
        if let Some(request) = request {
            let identity = client.request(ServiceRequest::ReceiverIdentityRead {
                path: path.clone(),
                device_container_id: device.device_container_id.clone(),
            })?;
            ensure!(
                identity["product_id"] == device.product_id
                    && identity["path"] == path
                    && identity["device_container_id"] == device.device_container_id,
                "Brightness serial owner differs"
            );
            let serial = identity["serial_number"]
                .as_str()
                .filter(|s| !s.is_empty())
                .context("Source serial absent")?
                .to_owned();
            let local = razer_storage::receiver_reset::local_path(
                device.product_id,
                &device.device_container_id,
                &serial,
            )?;
            let _lock = super::receiver_reset::DOCUMENT_LOCK
                .lock()
                .map_err(|_| anyhow::anyhow!("Receiver source storage lock unavailable"))?;
            guard()?;
            let previous = razer_storage::receiver_reset::load(
                &local,
                device.product_id,
                &device.device_container_id,
                &serial,
            )?;
            let computer = std::env::var("COMPUTERNAME").unwrap_or_default();
            let current = match &previous {
                Some(value) => value.clone(),
                None => razer_storage::receiver_reset::initial_document(
                    device.product_id,
                    device.edition_id,
                    &serial,
                    &computer,
                    &uuid::Uuid::new_v4().to_string(),
                )?,
            };
            let cache = razer_storage::receiver_reset::prepare_serial_cache(
                device.product_id,
                device.edition_id,
                &current,
                &serial,
                &computer,
                &uuid::Uuid::new_v4().to_string(),
            )?;
            let guid = cache["activeProfile"]
                .as_str()
                .context("Source active profile absent")?;
            // Existing source UI draft IDs cannot replace the actual serial's
            // active GUID. An original imported GUID must match that owner.
            if device.profiles.iter().any(|p| {
                p.guid == device.active_profile
                    && cache["profiles"]
                        .as_array()
                        .is_some_and(|a| a.iter().any(|p| p["guid"] == device.active_profile))
            }) {
                ensure!(
                    guid == device.active_profile,
                    "UI active source profile changed before brightness mutation"
                );
            }
            let document = razer_storage::receiver_reset::brightness_document(
                &cache,
                guid,
                request.enabled(),
                request.value(),
            )?;
            guard()?;
            razer_storage::receiver_reset::persist_document(
                &local,
                device.product_id,
                &device.device_container_id,
                &serial,
                previous.as_ref(),
                &document,
            )?;
            outcome = Some(HelpResetOutcome {
                source_document: document.clone(),
                serial_number: serial.clone(),
            });
            storage = Some((local, serial, document));
        }
        let response = (|| -> anyhow::Result<Value> {
            guard()?;
            let response = client.request(match request {
                Some(request) => ServiceRequest::ReceiverBrightnessWrite {
                    path: path.clone(),
                    device_container_id: device.device_container_id.clone(),
                    percent: request.percent(),
                },
                None => ServiceRequest::ReceiverBrightnessRead {
                    path: path.clone(),
                    device_container_id: device.device_container_id.clone(),
                },
            })?;
            ensure!(
                response["product_id"] == device.product_id
                    && response["path"] == path
                    && response["device_container_id"] == device.device_container_id,
                "Brightness device response owner differs"
            );
            Ok(response)
        })();
        let (response, mut error) = match response {
            Ok(response) => {
                let error = response["result"]["error"].as_str().map(str::to_owned);
                (response, error)
            }
            Err(error) => (Value::Null, Some(format!("{error:#}"))),
        };
        if let Some((local, serial, document)) = storage {
            if let Err(receipt) = razer_storage::receiver_reset::record_refresh(
                &local,
                device.product_id,
                &device.device_container_id,
                &serial,
                &document,
                json!({"ON_SET_BRIGHTNESS":{"response":response,"error":error}}),
            ) {
                let detail =
                    format!("Brightness result receipt could not be persisted: {receipt:#}");
                error = Some(error.map_or(detail.clone(), |error| format!("{error}; {detail}")));
            }
        }
        Ok(Completion {
            outcome,
            response,
            error,
        })
    })();
    let shutdown = client.request(ServiceRequest::Shutdown);
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
        (result, _) => result,
    }
}
impl AppShell {
    fn retire_receiver_brightness(&mut self, key: &str) {
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
    pub(super) fn install_receiver_brightness_cleanup(&mut self, cx: &mut Context<Self>) {
        self.subscriptions.push(cx.on_app_quit(|this, cx| {
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
        }));
    }
    pub(super) fn receiver_brightness_write(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: ReceiverBrightnessRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.receiver_brightness_request(
            workspace,
            request.generation(),
            Some(request),
            window,
            cx,
        );
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
                    .send(execute(device, request, &signal).map_err(|error| format!("{error:#}")));
            }) {
            Ok(worker) => worker,
            Err(error) => {
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_receiver_brightness(
                        generation,
                        percent,
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
                        let (outcome, observed, error) = match result {
                            Ok(result) => {
                                let observed = if percent.is_some() {
                                    result.response["result"]["observed"]["percent"].as_u64()
                                } else {
                                    result.response["result"]["percent"].as_u64()
                                }
                                .and_then(|n| u8::try_from(n).ok());
                                (result.outcome, observed, result.error)
                            }
                            Err(error) => (None, None, Some(error)),
                        };
                        if current {
                            shell.status = error.clone().unwrap_or_else(|| match observed {
                                Some(value) => format!("设备实际亮度：{value}%"),
                                None => "设备亮度未确认".into(),
                            });
                        }
                        workspace.update(cx, |workspace, cx| {
                            workspace.finish_receiver_brightness(
                                generation, percent, outcome, observed, error, current, window, cx,
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
                _delivery,
            },
        );
    }
}
