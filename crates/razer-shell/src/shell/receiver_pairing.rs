//! Real 164/241 Dock commands with a retained, cancellable service worker.
use super::*;
use anyhow::{Context as _, ensure};
use razer_device::receiver_pairing::PairingAction;
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_pages::features::{DockPairingEvent, DockPairingObservation};
use serde_json::{Value, json};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub(super) struct Session {
    operation_id: u64,
    canceled: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    _delivery: Task<()>,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.canceled.store(true, Ordering::Release);
    }
}
impl Session {
    fn finish(mut self) {
        self.canceled.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
enum Delivery {
    Started,
    Finished(Result<Value, String>),
}

// Serialize local document adaptation across this process's physical owners.
// This is an application file guard, not the vendor's hardware mutex.
static CACHE_LOCK: Mutex<()> = Mutex::new(());

fn persist_confirmed(
    product_id: u32,
    container: &str,
    event: &DockPairingEvent,
    result: &mut Value,
) {
    if !matches!(
        event.kind(),
        "DUALLINK_BIND_DEVICE" | "DUALLINK_UNBIND_DEVICE"
    ) {
        return;
    }
    let persisted = (|| -> anyhow::Result<()> {
        let _guard = CACHE_LOCK
            .lock()
            .map_err(|_| anyhow::anyhow!("配对本地缓存锁不可用"))?;
        let path = razer_storage::receiver_pairing::local_path();
        let mut cache = razer_storage::receiver_pairing::PairingCache::load(&path)?;
        let (category, action) = action(event)?;
        match action {
            PairingAction::Pair { dongle_id } => {
                ensure!(
                    result["completion_evidence"] == "hardware_event_54_status_2",
                    "本地配对保存缺少真实成功事件"
                );
                let peer_product = event.payload()["device"]["productId"]
                    .as_u64()
                    .and_then(|id| u32::try_from(id).ok())
                    .context("配对候选缺少原目录产品 ID")?;
                cache.add_confirmed(
                    u16::try_from(product_id)?,
                    peer_product,
                    dongle_id,
                    &category,
                    container,
                )?;
            }
            PairingAction::Unpair { dongle_id } => {
                ensure!(
                    result["binding_readback"].is_object(),
                    "解绑本地清理缺少真实绑定回读"
                );
                cache.remove_confirmed(dongle_id, container)?;
            }
            PairingAction::Scan => unreachable!(),
        }
        cache.save(&path)?;
        result["local_pairing_state"] = cache.state();
        Ok(())
    })();
    result["local_cache_persisted"] = json!(persisted.is_ok());
    if let Err(error) = persisted {
        // Hardware success precedes ye/ge persistence in the source. Do not
        // rewrite an observed hardware outcome as a failed Pair/Unpair command.
        result["local_cache_error"] = json!(format!("{error:#}"));
    }
}

pub(super) mod route;

fn action(event: &DockPairingEvent) -> anyhow::Result<(String, PairingAction)> {
    let p = event.payload();
    let category = p["category"]
        .as_str()
        .or_else(|| p["device"]["category"].as_str())
        .unwrap_or("MOUSE");
    ensure!(matches!(category, "MOUSE" | "KEYBOARD"), "配对类别无效");
    let dongle = |value: &Value| -> anyhow::Result<u16> {
        let id = value
            .as_u64()
            .and_then(|id| u16::try_from(id).ok())
            .context("配对动作缺少真实 dongle ID")?;
        ensure!(id != 0 && id != u16::MAX, "配对 dongle ID 无效");
        Ok(id)
    };
    Ok((
        category.into(),
        match event.kind() {
            "DUALLINK_SCAN_DEVICE" => {
                ensure!(p["status"].as_u64() == Some(1), "扫描 status 不是源开始值");
                PairingAction::Scan
            }
            "DUALLINK_BIND_DEVICE" => {
                ensure!(p["mode"].as_u64() == Some(1), "配对 mode 不是源开始值");
                PairingAction::Pair {
                    dongle_id: dongle(&p["device"]["dongleId"])?,
                }
            }
            "DUALLINK_UNBIND_DEVICE" => PairingAction::Unpair {
                dongle_id: dongle(&p["productId"])?,
            },
            _ => anyhow::bail!("动作没有当前配对提交链"),
        },
    ))
}

fn run(
    product_id: u32,
    container: &str,
    event: &DockPairingEvent,
    canceled: &AtomicBool,
    delivery: &mpsc::Sender<Delivery>,
) -> anyhow::Result<Value> {
    let (category, action) = action(event)?;
    ensure!(
        matches!(product_id, 164 | 241),
        "该接收器的配对链尚未完成当前原码核验"
    );
    ensure!(!canceled.load(Ordering::Acquire), "配对请求已取消，未启动");
    let mut client = ServiceClient::spawn()?;
    let id = format!("dock-{}", event.session());
    let result = (|| {
        let path = route::command_path(&mut client, product_id, container)?;
        ensure!(!canceled.load(Ordering::Acquire), "配对请求已取消，未提交");
        let reply = client.request(ServiceRequest::ReceiverPairingStart {
            operation_id: id.clone(),
            path: path.clone(),
            device_container_id: container.into(),
            category: category.clone(),
            action: action.clone(),
        })?;
        ensure!(
            reply["operation_id"] == id && reply["state"] == "running",
            "配对启动响应身份不符"
        );
        let _ = delivery.send(Delivery::Started);
        let started = Instant::now();
        let mut cancel_sent = false;
        loop {
            if canceled.load(Ordering::Acquire) && !cancel_sent {
                let reply = client.request(ServiceRequest::ReceiverPairingCancel {
                    operation_id: id.clone(),
                })?;
                ensure!(
                    reply["operation_id"] == id && reply["state"] == "canceling",
                    "取消响应身份不符"
                );
                cancel_sent = true;
            }
            let reply = client.request(ServiceRequest::ReceiverPairingPoll {
                operation_id: id.clone(),
            })?;
            ensure!(reply["operation_id"] == id, "配对完成响应身份不符");
            match reply["state"].as_str() {
                Some("running" | "canceling") => {}
                Some("failed" | "canceled") => anyhow::bail!(
                    "{}",
                    reply["error"]
                        .as_str()
                        .unwrap_or("配对失败，设备状态未确认")
                ),
                Some("completed") => {
                    ensure!(
                        !cancel_sent && !canceled.load(Ordering::Acquire),
                        "配对取消后结果不再发布"
                    );
                    let mut result = reply["result"].clone();
                    ensure!(
                        result["product_id"].as_u64() == Some(u64::from(product_id))
                            && result["path"] == path
                            && result["category"] == category
                            && result["device_container_id"]
                                .as_str()
                                .is_some_and(|id| id.eq_ignore_ascii_case(container)),
                        "配对结果的实际设备身份不匹配"
                    );
                    if let PairingAction::Unpair { dongle_id } = action {
                        // Extra application confirmation after the source ACK;
                        // this query is separate from local runtime/cache cleanup.
                        let observed = client.request(ServiceRequest::ReceiverWirelessStatus {
                            path: path.clone(),
                            device_container_id: container.into(),
                        })?;
                        razer_discovery::discovery::project_receiver_query(
                            product_id, container, &observed,
                        )?;
                        let rows = observed["devices"]
                            .as_array()
                            .context("解绑回读缺少实际绑定列表")?;
                        ensure!(!rows.iter().any(|row| row["product_id"].as_u64() == Some(u64::from(dongle_id))), "解绑命令后原绑定仍存在");
                        result["binding_readback"] = observed;
                    } else if matches!(action, PairingAction::Pair { .. }) {
                        // Real connection refresh is separate from successful
                        // pairing and the original runtime/profile publication.
                        match client
                            .request(ServiceRequest::ReceiverWirelessStatus {
                                path: path.clone(),
                                device_container_id: container.into(),
                            })
                            .and_then(|observed| {
                                razer_discovery::discovery::project_receiver_query(
                                    product_id, container, &observed,
                                )?;
                                Ok(observed)
                            }) {
                            Ok(observed) => result["binding_readback"] = observed,
                            Err(error) => {
                                result["binding_readback_error"] = json!(format!("{error:#}"))
                            }
                        }
                    }
                    return Ok(result);
                }
                _ => anyhow::bail!("未知配对运行状态"),
            }
            // Local worker budget, not an original device timing constant.
            ensure!(
                started.elapsed() < Duration::from_secs(100),
                "配对运行及取消清理等待超时"
            );
            thread::sleep(Duration::from_millis(50));
        }
    })();
    // Controller shutdown cancels and joins its operation. A failed shutdown
    // cannot be silently presented as a fully cleaned-up successful submission.
    let shutdown = client.request(ServiceRequest::Shutdown);
    match (result, shutdown) {
        (Ok(mut result), Ok(_)) => {
            ensure!(
                !canceled.load(Ordering::Acquire),
                "配对取消后不再提交本地缓存"
            );
            persist_confirmed(product_id, container, event, &mut result);
            Ok(result)
        }
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error.context("配对进程清理未确认")),
    }
}

impl AppShell {
    pub(super) fn retire_receiver_pairing(&mut self, identity: &str) {
        if let Some(mut session) = self.receiver_pairing_operations.remove(identity) {
            session.canceled.store(true, Ordering::Release);
            if let Some(worker) = session.worker.take() {
                self.receiver_pairing_cleanup.push(worker);
            }
        }
        // Only finished threads are joined on the UI thread. Running cleanup
        // remains retained so final app shutdown can wait for every old owner.
        let mut pending = Vec::new();
        for worker in self.receiver_pairing_cleanup.drain(..) {
            if worker.is_finished() {
                let _ = worker.join();
            } else {
                pending.push(worker);
            }
        }
        self.receiver_pairing_cleanup = pending;
    }

    pub(super) fn cancel_receiver_pairing(&mut self) {
        let owners: Vec<_> = self.receiver_pairing_operations.keys().cloned().collect();
        for identity in owners {
            self.retire_receiver_pairing(&identity);
        }
    }

    fn dock_pairing_result(
        product_id: u32,
        event: &DockPairingEvent,
        result: Value,
    ) -> Result<Value, String> {
        let result = (|| -> anyhow::Result<Value> {
            match event.kind() {
                "DUALLINK_SCAN_DEVICE" => {
                    ensure!(
                        result["operation"] == "scan"
                            && result["completion_evidence"] == "hardware_event_55_status_2",
                        "扫描缺少真实完成事件"
                    );
                    let scanned: Vec<razer_device::receiver_pairing::ScanCandidate> =
                        serde_json::from_value(result["candidates"].clone())?;
                    razer_discovery::receiver_pairing::project_scan_candidates(
                        u16::try_from(product_id)?,
                        &scanned,
                    )
                }
                "DUALLINK_BIND_DEVICE" => {
                    ensure!(
                        result["operation"] == "pair"
                            && result["completion_evidence"] == "hardware_event_54_status_2"
                            && result["pair"]["hardware_status"].as_u64() == Some(2)
                            && result["pair"]["dongle_id"] == event.payload()["device"]["dongleId"],
                        "配对缺少匹配的真实完成事件"
                    );
                    // Source Ee reads edition/layout after event54. Failed reads
                    // use explicit source defaults; scan metadata is not reused
                    // as a successful post-pair observation.
                    let mut device = event.payload()["device"].clone();
                    let metadata = &result["post_pair_metadata"];
                    let reading = &metadata["reading"];
                    let fallback = &metadata["source_fallback"];
                    let edition = reading["edition_id"]
                        .as_u64()
                        .or_else(|| fallback["edition_id"].as_u64())
                        .context("配对完成缺少 edition 读取或原默认值")?;
                    let mut layout = reading["keyboard_layout"]
                        .as_u64()
                        .or_else(|| fallback["keyboard_layout"].as_u64())
                        .context("配对完成缺少 layout 读取或原默认值")?;
                    if device["category"] == "MOUSE" && layout == 0 {
                        layout = metadata["mouse_layout_followup"]["status"]
                            .as_u64()
                            .filter(|layout| *layout != 0)
                            .unwrap_or(layout);
                    }
                    ensure!(edition <= 255 && layout <= 255, "配对元数据超过原字节范围");
                    device["editionId"] = json!(edition);
                    device["layoutId"] = json!(layout);
                    Ok(
                        json!({"device":device,"hardware_confirmed":true,"post_pair_metadata":metadata,"post_pair_metadata_observed":reading.is_object(),"metadata_complete":metadata["metadata_complete"],"runtime_published":false,"local_cache_persisted":result["local_cache_persisted"],"local_cache_error":result["local_cache_error"]}),
                    )
                }
                "DUALLINK_UNBIND_DEVICE" => {
                    ensure!(
                        result["operation"] == "unpair" && result["binding_readback"].is_object(),
                        "解绑缺少实际绑定回读"
                    );
                    Ok(
                        json!({"productId":event.payload()["productId"],"binding_readback":result["binding_readback"],"runtime_cleanup_complete":false,"local_cache_persisted":result["local_cache_persisted"],"local_cache_error":result["local_cache_error"]}),
                    )
                }
                _ => anyhow::bail!("未知配对页面操作"),
            }
        })();
        result.map_err(|error| format!("{error:#}"))
    }
    pub(super) fn install_receiver_pairing_cleanup(&mut self, cx: &mut Context<Self>) {
        self.subscriptions.push(cx.on_app_quit(|this, cx| {
            let sessions = std::mem::take(&mut this.receiver_pairing_operations);
            let retired = std::mem::take(&mut this.receiver_pairing_cleanup);
            for session in sessions.values() {
                session.canceled.store(true, Ordering::Release);
            }
            let cleanup = cx.background_executor().spawn(async move {
                for (_, session) in sessions {
                    session.finish();
                }
                for worker in retired {
                    let _ = worker.join();
                }
            });
            async move { cleanup.await }
        }));
    }

    pub(super) fn submit_dock_pairing(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        event: &DockPairingEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let identity = workspace.read(cx).identity(cx);
        let operation_owner = identity.clone();
        self.retire_receiver_pairing(&identity);
        let device = workspace.read(cx).device(cx);
        let container = device.device_container_id.clone();
        let product_id = device.real_product_id;
        let route = device_discovery::receiver_route_for_owner(
            &self.device_observations,
            device,
            &container,
            product_id,
        );
        let route = match route {
            Ok(route) => route,
            Err(error) => {
                workspace.update(cx, |workspace, cx| {
                    workspace.observe_dock_pairing(
                        DockPairingObservation::result(
                            event.session(),
                            event.kind(),
                            Err(format!("{error:#}")),
                        ),
                        cx,
                    )
                });
                return;
            }
        };
        let revision = self.discovery_revision;
        let session = event.session();
        let kind = event.kind().to_owned();
        let requested = event.clone();
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        let (sender, receiver) = mpsc::channel();
        let worker_event = event.clone();
        let worker_container = container.clone();
        let worker = thread::Builder::new()
            .name("dock-pairing-client".into())
            .spawn(move || {
                let result = run(
                    product_id,
                    &worker_container,
                    &worker_event,
                    &signal,
                    &sender,
                )
                .map_err(|error| format!("{error:#}"));
                let _ = sender.send(Delivery::Finished(result));
            });
        let worker = match worker {
            Ok(worker) => worker,
            Err(error) => {
                workspace.update(cx, |workspace, cx| {
                    workspace.observe_dock_pairing(
                        DockPairingObservation::result(session, &kind, Err(error.to_string())),
                        cx,
                    )
                });
                return;
            }
        };
        let _delivery = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                let mut deliveries = Vec::new();
                loop {
                    match receiver.try_recv() {
                        Ok(delivery) => deliveries.push(delivery),
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            if !deliveries
                                .iter()
                                .any(|delivery| matches!(delivery, Delivery::Finished(_)))
                            {
                                deliveries.push(Delivery::Finished(Err(
                                    "配对工作线程已断开，未确认操作结果。".into(),
                                )));
                            }
                            break;
                        }
                    }
                }
                let mut finished = false;
                let current = this.update_in(cx, |this, window, cx| {
                    // A late completion must never retire a replacement operation.
                    let owns_operation = this
                        .receiver_pairing_operations
                        .get(&identity)
                        .is_some_and(|operation| operation.operation_id == session);
                    if !owns_operation {
                        return false;
                    }
                    let current = this.discovery_revision == revision
                        && this.devices.contains(&workspace)
                        && workspace.read(cx).identity(cx) == identity
                        && this.device_observations.iter().any(|observed| {
                            observed.matches(workspace.read(cx).device(cx))
                                && route.matches(observed)
                        });
                    if !current {
                        this.retire_receiver_pairing(&identity);
                        return false;
                    }
                    for delivery in deliveries {
                        match delivery {
                            Delivery::Started => {
                                workspace.update(cx, |workspace, cx| {
                                    workspace.observe_dock_pairing(
                                        DockPairingObservation::progress(session, &kind),
                                        cx,
                                    )
                                });
                            }
                            Delivery::Finished(result) => {
                                finished = true;
                                this.retire_receiver_pairing(&identity);
                                if let Ok(value) = &result {
                                    if let Some(error) = value["local_cache_error"].as_str() {
                                        this.status = format!(
                                            "设备操作已确认，本地配对缓存保存失败：{error}"
                                        );
                                    }
                                    if let Some(error) =
                                        value["post_pair_metadata"]["read_error"].as_str()
                                    {
                                        this.status = format!(
                                            "配对硬件事件已确认，后续设备元数据读取失败：{error}"
                                        );
                                    }
                                }
                                let projection = result
                                    .as_ref()
                                    .ok()
                                    .filter(|value| value["binding_readback"].is_object())
                                    .map(|value| {
                                        razer_discovery::discovery::project_receiver_query(
                                            product_id,
                                            &container,
                                            &value["binding_readback"],
                                        )
                                    });
                                let result = result.and_then(|result| {
                                    Self::dock_pairing_result(product_id, &requested, result)
                                });
                                let accepted = workspace.update(cx, |workspace, cx| {
                                    workspace.observe_dock_pairing(
                                        DockPairingObservation::result(session, &kind, result),
                                        cx,
                                    )
                                });
                                if accepted {
                                    if let Some(Ok(projection)) = projection {
                                        this.publish_receiver_query(
                                            &container,
                                            product_id,
                                            Some(&projection),
                                            window,
                                            cx,
                                        );
                                    }
                                }
                            }
                        }
                    }
                    if finished {
                        cx.notify();
                    }
                    true
                });
                if finished || !matches!(current, Ok(true)) {
                    break;
                }
            }
        });
        self.receiver_pairing_operations.insert(
            operation_owner,
            Session {
                operation_id: session,
                canceled,
                worker: Some(worker),
                _delivery,
            },
        );
    }
}
