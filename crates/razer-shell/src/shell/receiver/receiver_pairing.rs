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
    SourceResponse(Value),
    LocalPersistence(Value),
    Finished(Result<Value, String>),
}

enum UnpairStep {
    SourceResponse(Value),
    LocalPersistence(Value),
}

// Serialize local document adaptation across this process's physical owners.
// This is an application file guard, not the vendor's hardware mutex.
static CACHE_LOCK: Mutex<()> = Mutex::new(());

// Current164/241 u(success) precedes the independent p()/ge()/500ms
// reconnect path. An additional getter cannot revoke that source response.
fn validate_unpair_completion(result: &Value, dongle_id: u16) -> anyhow::Result<()> {
    ensure!(
        result["operation"] == "unpair" && result["source_ui_response"] != false,
        "解绑缺少原命令完成响应"
    );
    match result["completion_evidence"].as_str() {
        Some("feature_acknowledgement") => ensure!(
            result["acknowledgement"]["dongle_id"].as_u64() == Some(u64::from(dongle_id)),
            "解绑确认与原请求 dongle ID 不符"
        ),
        Some("binding_query_after_failed_ack_2000ms") => ensure!(
            result["removed_dongle_id"].as_u64() == Some(u64::from(dongle_id))
                && result["observed_bindings"].is_array()
                && result["command_error"].is_string(),
            "解绑缺少原 2000ms 恢复查询证据"
        ),
        Some("source_restore_after_binding_query_error_2000ms") => ensure!(
            result["product_id"] == 193
                && result["removed_dongle_id"].as_u64() == Some(u64::from(dongle_id))
                && result["command_error"].is_string()
                && result["binding_query_error"].is_string()
                && result["source_response_error"] == result["command_error"]
                && result["source_response_payload"] == json!({})
                && result["hardware_confirmed"] == false
                && result["observed_bindings"].is_null(),
            "Current192 query-error source restoration has no hardware confirmation"
        ),
        _ => anyhow::bail!("解绑没有原 ACK 或失败后的恢复查询证据"),
    }
    Ok(())
}

fn remove_source_unpair_cache(
    cache: &mut razer_storage::receiver_pairing::PairingCache,
    container: &str,
    dongle_id: u16,
    result: &Value,
) -> anyhow::Result<()> {
    validate_unpair_completion(result, dongle_id)?;
    cache.remove_confirmed(dongle_id, container)?;
    Ok(())
}

fn record_unpair_readback(
    product_id: u32,
    container: &str,
    dongle_id: u16,
    result: &mut Value,
    observed: anyhow::Result<Value>,
) {
    let validated = observed.and_then(|observed| {
        let validation = (|| {
            razer_discovery::discovery::project_receiver_query(product_id, container, &observed)?;
            let rows = observed["devices"]
                .as_array()
                .context("解绑额外回读缺少实际绑定列表")?;
            ensure!(
                !rows
                    .iter()
                    .any(|row| row["product_id"].as_u64() == Some(u64::from(dongle_id))),
                "解绑 ACK 后额外回读仍包含原绑定"
            );
            Ok(())
        })();
        // Preserve even a stale or malformed response as diagnostics, without
        // turning it into source UI state or a successful empty device read.
        result["binding_readback"] = observed;
        validation
    });
    result["binding_readback_confirmed"] = json!(validated.is_ok());
    if let Err(error) = validated {
        result["binding_readback_error"] = json!(format!("{error:#}"));
    }
}

fn receiver_readback_for_refresh<'a>(
    event: &DockPairingEvent,
    result: &'a Value,
) -> Option<&'a Value> {
    if event.kind() == "DUALLINK_UNBIND_DEVICE" {
        // The source response is already determined by its ACK/recovery path.
        // The extra Rust query has no getter-to-source-UI refresh call chain.
        return None;
    }
    result
        .get("binding_readback")
        .filter(|value| value.is_object())
}

fn persist_confirmed(
    product_id: u32,
    container: &str,
    event: &DockPairingEvent,
    result: &mut Value,
) {
    if !matches!(
        event.kind(),
        "DUALLINK_BIND_DEVICE" | "DUALLINK_UNBIND_DEVICE"
    ) || result["source_ui_response"] == false
    {
        return;
    }
    let persisted = (|| -> anyhow::Result<()> {
        let _guard = CACHE_LOCK
            .lock()
            .map_err(|_| anyhow::anyhow!("配对本地缓存锁不可用"))?;
        let path = razer_storage::receiver_pairing::local_path();
        let mut cache = razer_storage::receiver_pairing::PairingCache::load(&path)?;
        let category = result["category"]
            .as_str()
            .context("配对保存缺少已验证 factory 类别")?;
        let (category, action) = action(event, Some(category))?;
        match action {
            PairingAction::Pair { dongle_id } => {
                ensure!(
                    result["operation"] == "pair"
                        && result["completion_evidence"] == "hardware_event_54_status_2"
                        && result["pair"]["hardware_status"] == 2
                        && result["pair"]["dongle_id"] == dongle_id,
                    "本地配对保存缺少真实成功事件"
                );
                let peer_product = event.payload()["device"]["productId"]
                    .as_u64()
                    .and_then(|id| u32::try_from(id).ok())
                    .context("配对候选缺少原目录产品 ID")?;
                cache.add_confirmed_for_source(
                    u16::try_from(product_id)?,
                    peer_product,
                    dongle_id,
                    &category,
                    container,
                    result["source_product_id"]
                        .as_u64()
                        .and_then(|id| u16::try_from(id).ok())
                        .or_else(|| (product_id == 757).then_some(756)),
                )?;
            }
            PairingAction::Unpair { dongle_id } => {
                remove_source_unpair_cache(&mut cache, container, dongle_id, result)?;
            }
            PairingAction::Scan | PairingAction::Cancel => unreachable!(),
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

fn reconnect_after_persistence(
    product_id: u32,
    container: &str,
    event: &DockPairingEvent,
    result: &mut Value,
    owner: (&reconnect::Owners, &reconnect::Scope),
    category: &str,
    source_product_id: Option<u16>,
) {
    persist_confirmed(product_id, container, event, result);
    if reconnect_ready(event, result) {
        let scheduled = (|| {
            let sequence = result["next_transaction_sequence"]
                .as_u64()
                .and_then(|n| u8::try_from(n).ok())
                .context("解绑缺少原factory下一事务序号")?;
            owner
                .0
                .start(owner.1, category, source_product_id, sequence)
        })();
        match scheduled {
            Ok(()) => result["reconnect_scheduled"] = json!(true),
            Err(error) => result["reconnect_error"] = json!(format!("{error:#}")),
        }
    }
}

fn reconnect_ready(event: &DockPairingEvent, result: &Value) -> bool {
    let Ok((_, PairingAction::Unpair { dongle_id })) = action(event, None) else {
        return false;
    };
    result["local_cache_persisted"] == true && validate_unpair_completion(result, dongle_id).is_ok()
}

#[path = "receiver_pairing/reconnect.rs"]
pub(super) mod reconnect;
#[path = "receiver_pairing/route.rs"]
pub(super) mod route;
#[path = "receiver_pairing/standalone.rs"]
pub(super) mod standalone;

fn action(
    event: &DockPairingEvent,
    category_override: Option<&str>,
) -> anyhow::Result<(String, PairingAction)> {
    let p = event.payload();
    let category = category_override
        .or_else(|| p["category"].as_str())
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
            "DUALLINK_CANCEL" => PairingAction::Cancel,
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
    source_product_id: Option<u16>,
    category_override: Option<&str>,
    canceled: &AtomicBool,
    delivery: &mpsc::Sender<Delivery>,
    reconnect_owner: (&reconnect::Owners, &reconnect::Scope),
    source_response: &mut dyn FnMut(UnpairStep),
) -> anyhow::Result<Value> {
    //756's opposite-category factory owns the command category, including an
    // unknown scan candidate whose source presentation category is empty.
    let category_override = category_override
        .or_else(|| (source_product_id == Some(756) || product_id == 757).then_some("MOUSE"));
    let (category, action) = action(event, category_override)?;
    razer_device::receiver_pairing::for_source(
        u16::try_from(product_id)?,
        &category,
        source_product_id,
    )?;
    ensure!(
        product_id != 756 || matches!(action, PairingAction::Cancel),
        "有线756没有配对 UI 提交分支"
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
            source_product_id,
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
                    result["source_product_id"] = json!(source_product_id);
                    if result["source_ui_response"] == false {
                        // Current756 aborts its event54 callback before metadata,
                        // UI publication and X cache adaptation for this catalog
                        // miss. Preserve the real event and independent diagnostics.
                        return Ok(result);
                    }
                    if let PairingAction::Unpair { dongle_id } = action {
                        validate_unpair_completion(&result, dongle_id)?;
                        if result["source_response_error"].is_string() {
                            let mut error_response = result.clone();
                            error_response["source_response_intermediate_error"] = json!(true);
                            source_response(UnpairStep::SourceResponse(error_response));
                        }
                        // u(ACK) responds before ge()/p(), without awaiting an
                        // extra getter or successful independent reconnection.
                        source_response(UnpairStep::SourceResponse(result.clone()));
                        reconnect_after_persistence(
                            product_id,
                            container,
                            event,
                            &mut result,
                            reconnect_owner,
                            &category,
                            source_product_id,
                        );
                        source_response(UnpairStep::LocalPersistence(result.clone()));
                        let observed = client.request(ServiceRequest::ReceiverWirelessStatus {
                            path: path.clone(),
                            device_container_id: container.into(),
                            source_product_id,
                        });
                        record_unpair_readback(
                            product_id,
                            container,
                            dongle_id,
                            &mut result,
                            observed,
                        );
                    } else if matches!(action, PairingAction::Pair { .. }) {
                        // Real connection refresh is separate from successful
                        // pairing and the original runtime/profile publication.
                        match client
                            .request(ServiceRequest::ReceiverWirelessStatus {
                                path: path.clone(),
                                device_container_id: container.into(),
                                source_product_id,
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
                    match razer_storage::source_local::get(
                        &razer_storage::source_local::path(),
                        "connectedDeviceInfo",
                    ) {
                        Ok(Some(Value::Array(records))) => {
                            result["source_connected_device_info"] = json!(records)
                        }
                        Ok(None) => {}
                        Ok(Some(_)) => {
                            result["source_connected_device_info_error"] =
                                json!("connectedDeviceInfo 缺少源数组")
                        }
                        Err(error) => {
                            result["source_connected_device_info_error"] =
                                json!(format!("{error:#}"))
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
        (Ok(mut result), cleanup) if matches!(action, PairingAction::Unpair { .. }) => {
            // Source u(ACK) precedes ge()/its independent500ms timer. Later
            // source CANCEL or Rust cleanup failures cannot revoke that ACK.
            if let Err(error) = cleanup {
                result["pairing_worker_cleanup_error"] = json!(format!("{error:#}"));
            }
            Ok(result)
        }
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
    pub(super) fn install_standalone_pairing_transport(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let transport = self.pairing_transport.clone();
        self.pairing.update(cx, |page, cx| {
            page.set_transport(Some(transport.clone()), window, cx)
        });
        self.pairing_transport_delivery = Some(cx.spawn_in(window, async move |shell, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let completions = transport.take_completions();
                let reconnect_reports = transport.reconnect_owners().take_reports();
                if completions.is_empty() && reconnect_reports.is_empty() {
                    continue;
                }
                if shell
                    .update_in(cx, |shell, window, cx| {
                        for report in reconnect_reports {
                            // Diagnostics belong to this separate local owner;
                            // no getter or retry outcome mutates source pairing UI.
                            let owner = report.operation_id;
                            let diagnostic = shell
                                .receiver_reconnect_diagnostics
                                .entry(owner)
                                .or_insert_with(|| json!({}));
                            diagnostic
                                .as_object_mut()
                                .unwrap()
                                .extend(report.state.as_object().unwrap().clone());
                            diagnostic["physical_product_id"] = json!(report.product_id);
                            diagnostic["device_container_id"] = json!(report.container);
                            if let Some(error) = report.state["error"]
                                .as_str()
                                .or_else(|| report.state["outcome"]["last_error"].as_str())
                            {
                                shell.status = format!(
                                    "接收器 {}/{} 独立重连或清理未确认：{error}",
                                    report.product_id, report.container
                                );
                            }
                        }
                        let mut reload_cache = false;
                        for completion in completions {
                            if let Some(diagnostic) = completion.diagnostic {
                                shell.status =
                                    format!("设备操作已确认，后续本地处理未确认：{diagnostic}");
                            }
                            reload_cache |= completion.cache_changed;
                            if let Some(projection) = completion.projection {
                                shell.publish_receiver_query(
                                    &completion.container,
                                    completion.product_id,
                                    Some(&projection),
                                    window,
                                    cx,
                                );
                            }
                        }
                        if reload_cache {
                            shell.load_local_pairing_context(window, cx);
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
    }
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
        self.pairing_transport.cancel_all();
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
                    razer_discovery::receiver_pairing::project_scan_candidates_for_source(
                        u16::try_from(product_id)?,
                        result["source_product_id"]
                            .as_u64()
                            .and_then(|id| u16::try_from(id).ok()),
                        &scanned,
                        result["source_connected_device_info"]
                            .as_array()
                            .map_or(&[][..], Vec::as_slice),
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
                    ensure!(
                        result["source_ui_response"] != false,
                        "原配对回调没有发布 UI 响应"
                    );
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
                    if result["category"] == "MOUSE" && layout == 0 {
                        layout = metadata["mouse_layout_followup"]["status"]
                            .as_u64()
                            .filter(|layout| *layout != 0)
                            .unwrap_or(layout);
                    }
                    ensure!(edition <= 255 && layout <= 255, "配对元数据超过原字节范围");
                    if result["source_product_id"] == 756 || product_id == 757 {
                        let dongle = event.payload()["device"]["dongleId"]
                            .as_u64()
                            .and_then(|id| u16::try_from(id).ok())
                            .context("756 配对候选缺少真实 radio ID")?;
                        let scanned_edition = event.payload()["device"]["editionId"]
                            .as_u64()
                            .and_then(|id| u8::try_from(id).ok())
                            .context("756 配对候选缺少原扫描 edition 字节")?;
                        device = razer_discovery::receiver_pairing::project_source756_pair_device(
                            dongle,
                            scanned_edition,
                            edition as u8,
                            layout as u8,
                            result["source_connected_device_info"]
                                .as_array()
                                .map_or(&[][..], Vec::as_slice),
                        )?;
                    } else {
                        device["editionId"] = json!(edition);
                        device["layoutId"] = json!(layout);
                        if let Some(connected) = result["source_connected_device_info"]
                            .as_array()
                            .and_then(|rows| {
                                rows.iter()
                                    .find(|row| row["productId"] == device["productId"])
                            })
                        {
                            device = connected.clone();
                            device["dongleId"] = event.payload()["device"]["dongleId"].clone();
                        }
                    }
                    Ok(
                        json!({"device":device,"hardware_confirmed":true,"post_pair_metadata":metadata,"post_pair_metadata_observed":reading.is_object(),"metadata_complete":metadata["metadata_complete"],"runtime_published":false,"local_cache_persisted":result["local_cache_persisted"],"local_cache_error":result["local_cache_error"]}),
                    )
                }
                "DUALLINK_UNBIND_DEVICE" => {
                    let (_, PairingAction::Unpair { dongle_id }) = action(event, None)? else {
                        unreachable!()
                    };
                    validate_unpair_completion(&result, dongle_id)?;
                    if result["source_response_intermediate_error"] == true {
                        anyhow::bail!(
                            "{}",
                            result["source_response_error"]
                                .as_str()
                                .context("Missing original unpair error")?
                        );
                    }
                    if product_id == 193 && result["source_response_payload"] == json!({}) {
                        // Failed-ACK recovery sends the literal empty payload.
                        // Adding a radio ID here would make TopView delete a
                        // row that current VA deliberately leaves in place.
                        return Ok(json!({}));
                    }
                    let source_product = if product_id == 193 {
                        // Current192 VA removes rows by the response's radio
                        // ID. The event's logical PID is a different field.
                        json!(dongle_id)
                    } else {
                        event.payload()["productId"].clone()
                    };
                    Ok(
                        json!({"productId":source_product,"binding_readback":result["binding_readback"],"binding_readback_confirmed":result["binding_readback_confirmed"],"binding_readback_error":result["binding_readback_error"],"runtime_cleanup_complete":false,"local_cache_persisted":result["local_cache_persisted"],"local_cache_error":result["local_cache_error"],"reconnect_scheduled":result["reconnect_scheduled"],"reconnect_error":result["reconnect_error"],"pairing_worker_cleanup_error":result["pairing_worker_cleanup_error"]}),
                    )
                }
                _ => anyhow::bail!("未知配对页面操作"),
            }
        })();
        result.map_err(|error| format!("{error:#}"))
    }
    pub(super) fn install_receiver_pairing_cleanup(&mut self, _cx: &mut Context<Self>) {
        self.register_exit_cleanup(|this, cx| {
            let sessions = std::mem::take(&mut this.receiver_pairing_operations);
            let retired = std::mem::take(&mut this.receiver_pairing_cleanup);
            let standalone = this.pairing_transport.clone();
            let reconnect = standalone.reconnect_owners().clone();
            for session in sessions.values() {
                session.canceled.store(true, Ordering::Release);
            }
            let cleanup = cx.background_executor().spawn(async move {
                reconnect.shutdown();
                standalone.shutdown();
                for (_, session) in sessions {
                    session.finish();
                }
                for worker in retired {
                    let _ = worker.join();
                }
            });
            async move { cleanup.await }
        });
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
        let logical_product_id = device.product_id;
        let route = device_discovery::dock_route_for_workspace(
            &self.device_observations,
            device,
            self.local_pairing_records.as_ref(),
        );
        let route = match route {
            Ok(route) => route,
            Err(error) => {
                if event.kind() == "DUALLINK_CANCEL" {
                    self.status = format!("配对取消命令未提交：{error:#}");
                    cx.notify();
                } else {
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
                }
                return;
            }
        };
        let product_id = route.product_id();
        let source_product_id = if logical_product_id == 756 { Some(756) } else { None };
        // Current756 V redirects slave Unpair to its master's middleware.
        // That caller's category factory and prefix must replace756's namespace.
        let forwarded_unpair = logical_product_id == 756
            && product_id != 757
            && product_id != 756
            && event.kind() == "DUALLINK_UNBIND_DEVICE";
        let source_product_id = if forwarded_unpair {
            Some(product_id as u16)
        } else {
            source_product_id
        };
        let revision = self.discovery_revision;
        let reconnects = self.pairing_transport.reconnect_owners().clone();
        let reconnect_scope = match reconnects.scope(&identity, product_id, &container) {
            Ok(scope) => scope,
            Err(error) => {
                self.status = format!("配对资源所有者不可用：{error:#}");
                cx.notify();
                return;
            }
        };
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
                let mut source_response = |step| {
                    let _ = sender.send(match step {
                        UnpairStep::SourceResponse(value) => Delivery::SourceResponse(value),
                        UnpairStep::LocalPersistence(value) => Delivery::LocalPersistence(value),
                    });
                };
                let result = run(
                    product_id,
                    &worker_container,
                    &worker_event,
                    source_product_id,
                    forwarded_unpair.then_some("KEYBOARD"),
                    &signal,
                    &sender,
                    (&reconnects, &reconnect_scope),
                    &mut source_response,
                )
                .map_err(|error| format!("{error:#}"));
                let _ = sender.send(Delivery::Finished(result));
            });
        let worker = match worker {
            Ok(worker) => worker,
            Err(error) => {
                if kind == "DUALLINK_CANCEL" {
                    self.status = format!("配对取消工作线程未启动：{error}");
                    cx.notify();
                } else {
                    workspace.update(cx, |workspace, cx| {
                        workspace.observe_dock_pairing(
                            DockPairingObservation::result(session, &kind, Err(error.to_string())),
                            cx,
                        )
                    });
                }
                return;
            }
        };
        let _delivery = cx.spawn_in(window, async move |this, cx| {
            let mut source_response_applied = false;
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
                                if kind == "DUALLINK_CANCEL" {
                                    continue;
                                }
                                workspace.update(cx, |workspace, cx| {
                                    workspace.observe_dock_pairing(
                                        DockPairingObservation::progress(session, &kind),
                                        cx,
                                    )
                                });
                            }
                            Delivery::SourceResponse(value) => {
                                let intermediate =
                                    value["source_response_intermediate_error"] == true;
                                let result =
                                    Self::dock_pairing_result(product_id, &requested, value);
                                source_response_applied |= workspace.update(cx, |workspace, cx| {
                                    workspace.observe_dock_pairing(
                                        if intermediate {
                                            DockPairingObservation::intermediate_result(
                                                session, &kind, result,
                                            )
                                        } else {
                                            DockPairingObservation::result(session, &kind, result)
                                        },
                                        cx,
                                    )
                                });
                            }
                            Delivery::LocalPersistence(value) => {
                                if value["local_cache_persisted"] == true {
                                    this.load_local_pairing_context(window, cx);
                                }
                                if let Some(error) = value["local_cache_error"]
                                    .as_str()
                                    .or_else(|| value["reconnect_error"].as_str())
                                {
                                    this.status =
                                        format!("解绑响应已处理，后续缓存或重连未确认：{error}");
                                }
                                cx.notify();
                            }
                            Delivery::Finished(result) => {
                                finished = true;
                                this.retire_receiver_pairing(&identity);
                                // Current DUALLINK_CANCEL only awaits scan(4)
                                // and logs failures. It has no MW_RESPONSE_UI;
                                // cleanup must not invent progress/results or
                                // reload source state on a canceled dialog.
                                if kind == "DUALLINK_CANCEL" {
                                    if let Err(error) = result {
                                        this.status = format!("配对取消或清理未确认：{error}");
                                    }
                                    continue;
                                }
                                if let Ok(value) = &result {
                                    if value["source_ui_response"] == false {
                                        let error = value["source_callback_error"]
                                            .as_str()
                                            .unwrap_or("原配对回调未发布 UI 响应");
                                        this.status =
                                            format!("配对硬件事件已确认，原源码回调中断：{error}");
                                        continue;
                                    }
                                }
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
                                    if let Some(error) = value["binding_readback_error"].as_str() {
                                        this.status =
                                            format!("设备操作已确认，额外绑定回读未确认：{error}");
                                    }
                                    if let Some(error) = value["reconnect_error"]
                                        .as_str()
                                        .or_else(|| value["pairing_worker_cleanup_error"].as_str())
                                    {
                                        this.status = format!(
                                            "解绑响应已处理，后续重连或资源清理未确认：{error}"
                                        );
                                    }
                                }
                                let projection = result
                                    .as_ref()
                                    .ok()
                                    .and_then(|value| {
                                        receiver_readback_for_refresh(&requested, value)
                                    })
                                    .map(|value| {
                                        razer_discovery::discovery::project_receiver_query(
                                            product_id, &container, value,
                                        )
                                    });
                                let accepted = if source_response_applied {
                                    true
                                } else {
                                    let result = result.and_then(|result| {
                                        Self::dock_pairing_result(product_id, &requested, result)
                                    });
                                    workspace.update(cx, |workspace, cx| {
                                        workspace.observe_dock_pairing(
                                            DockPairingObservation::result(session, &kind, result),
                                            cx,
                                        )
                                    })
                                };
                                if accepted {
                                    if !source_response_applied {
                                        this.load_local_pairing_context(window, cx);
                                    }
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

#[cfg(test)]
mod tests {
    use super::*;
    const OWNER: &str = "{11111111-2222-3333-4444-555555555555}";

    fn unpair_ack(dongle_id: u16) -> Value {
        json!({"operation":"unpair","category":"MOUSE","acknowledgement":{"dongle_id":dongle_id},"completion_evidence":"feature_acknowledgement","runtime_cleanup_complete":false})
    }

    #[::core::prelude::v1::test]
    fn receiver_reconnect_requires_source_unpair_ack_and_successful_owner_cache_persistence() {
        let event = DockPairingEvent::request(
            "DUALLINK_UNBIND_DEVICE",
            json!({"productId":183,"category":"MOUSE"}),
        );
        let mut result = unpair_ack(183);
        assert!(!reconnect_ready(&event, &result));
        result["local_cache_persisted"] = json!(true);
        assert!(reconnect_ready(&event, &result));
        let cancel = DockPairingEvent::request("DUALLINK_CANCEL", json!({"category":"MOUSE"}));
        assert!(!reconnect_ready(&cancel, &result));
        result["local_cache_persisted"] = json!(false);
        assert!(!reconnect_ready(&event, &result));
        result["local_cache_persisted"] = json!(true);
        result["acknowledgement"]["dongle_id"] = json!(184);
        assert!(!reconnect_ready(&event, &result));
        result["acknowledgement"]["dongle_id"] = json!(183);
        result["source_ui_response"] = json!(false);
        assert!(!reconnect_ready(&event, &result));
    }

    fn readback_reply(product_id: u16, dongle_ids: &[u16]) -> Value {
        let cap = razer_device::receiver_capabilities::capability(product_id).unwrap();
        json!({"query":"receiver_wireless_status_v2","vendor_id":cap.vendor_id,"product_id":product_id,"claim_interface":cap.claim_interface,"report_bytes":cap.report_bytes,"path":"simulated-query-collection","device_instance_id":"simulated-query-instance","device_container_id":OWNER,"device_count":dongle_ids.len(),"devices":dongle_ids.iter().map(|dongle_id|json!({"product_id":dongle_id,"status":1})).collect::<Vec<_>>()})
    }

    #[::core::prelude::v1::test]
    fn unpair_ack_preserves_ui_and_owner_cache_cleanup_after_extra_query_failures() {
        let event = DockPairingEvent::request(
            "DUALLINK_UNBIND_DEVICE",
            json!({"productId":183,"category":"MOUSE"}),
        );
        let other_owner = "{21111111-2222-3333-4444-555555555555}";
        let mut wrong_scope = readback_reply(164, &[]);
        wrong_scope["device_container_id"] = json!(other_owner);
        for observed in [
            Err(anyhow::anyhow!("simulated getter failure after ACK")),
            Ok(readback_reply(164, &[183])),
            Ok(wrong_scope),
        ] {
            let mut result = unpair_ack(183);
            record_unpair_readback(164, OWNER, 183, &mut result, observed);
            assert_eq!(result["binding_readback_confirmed"], false);
            assert!(result["binding_readback_error"].is_string());
            assert!(receiver_readback_for_refresh(&event, &result).is_none());

            let mut cache = razer_storage::receiver_pairing::PairingCache::default();
            for container in [OWNER, other_owner] {
                cache
                    .add_confirmed(164, 182, 183, "MOUSE", container)
                    .unwrap();
                cache
                    .observe_runtime_serial(182, container, "observed-serial")
                    .unwrap();
            }
            remove_source_unpair_cache(&mut cache, OWNER, 183, &result).unwrap();
            assert!(cache.records().get(format!("5426/183/{OWNER}")).is_none());
            assert!(
                cache
                    .records()
                    .get(format!("5426/183/{other_owner}"))
                    .is_some()
            );
            assert!(
                cache.state()["observed_serials"]
                    .get(format!("5426/183/{OWNER}"))
                    .is_none()
            );
            assert_eq!(
                cache.state()["observed_serials"][format!("5426/183/{other_owner}")],
                "observed-serial"
            );
            let ui = AppShell::dock_pairing_result(164, &event, result).unwrap();
            assert_eq!(ui["productId"], 183);
            assert!(ui["binding_readback_error"].is_string());
            assert_eq!(ui["runtime_cleanup_complete"], false);
        }
    }

    #[::core::prelude::v1::test]
    fn unpair_completion_requires_matching_ack_or_source_recovery() {
        let event = DockPairingEvent::request(
            "DUALLINK_UNBIND_DEVICE",
            json!({"productId":183,"category":"KEYBOARD"}),
        );
        let mut cache = razer_storage::receiver_pairing::PairingCache::default();
        cache.add_confirmed(241, 182, 183, "MOUSE", OWNER).unwrap();
        let before = cache.records();
        let mut wrong_ack = unpair_ack(171);
        wrong_ack["binding_readback"] = readback_reply(241, &[]);
        assert!(remove_source_unpair_cache(&mut cache, OWNER, 183, &wrong_ack).is_err());
        assert_eq!(cache.records(), before);
        assert!(AppShell::dock_pairing_result(241, &event, wrong_ack).is_err());

        let recovered = json!({"operation":"unpair","completion_evidence":"binding_query_after_failed_ack_2000ms","removed_dongle_id":183,"command_error":"simulated rejected ACK","observed_bindings":[]});
        assert!(AppShell::dock_pairing_result(241, &event, recovered.clone()).is_ok());
        remove_source_unpair_cache(&mut cache, OWNER, 183, &recovered).unwrap();
        assert!(cache.records().as_object().unwrap().is_empty());
        for (field, invalid) in [
            ("removed_dongle_id", json!(171)),
            ("observed_bindings", Value::Null),
            ("command_error", Value::Null),
            ("source_ui_response", json!(false)),
        ] {
            let mut invalid_recovery = recovered.clone();
            invalid_recovery[field] = invalid;
            assert!(
                validate_unpair_completion(&invalid_recovery, 183).is_err(),
                "{field}"
            );
        }
        assert_eq!(action(&event, None).unwrap().0, "KEYBOARD");
    }

    #[::core::prelude::v1::test]
    fn source192_unpair_recovery_publishes_literal_empty_payload_without_inventing_a_read() {
        let event = DockPairingEvent::request(
            "DUALLINK_UNBIND_DEVICE",
            json!({"productId":713,"category":"KEYBOARD"}),
        );
        let empty_query = json!({"operation":"unpair","product_id":193,"completion_evidence":"binding_query_after_failed_ack_2000ms","removed_dongle_id":713,"command_error":"setter rejected","observed_bindings":[[193,1],[65535,0]],"source_response_payload":{}});
        assert_eq!(
            AppShell::dock_pairing_result(193, &event, empty_query).unwrap(),
            json!({})
        );
        let failed_query = json!({"operation":"unpair","product_id":193,"completion_evidence":"source_restore_after_binding_query_error_2000ms","removed_dongle_id":713,"command_error":"setter rejected","binding_query_error":"getter rejected","source_response_error":"setter rejected","source_response_payload":{},"hardware_confirmed":false});
        validate_unpair_completion(&failed_query, 713).unwrap();
        assert_eq!(
            AppShell::dock_pairing_result(193, &event, failed_query.clone()).unwrap(),
            json!({})
        );
        assert!(failed_query["observed_bindings"].is_null());
        assert!(receiver_readback_for_refresh(&event, &failed_query).is_none());
        let mut intermediate = failed_query.clone();
        intermediate["source_response_intermediate_error"] = json!(true);
        assert_eq!(
            AppShell::dock_pairing_result(193, &event, intermediate).unwrap_err(),
            "setter rejected"
        );
        for (field, value) in [
            ("product_id", json!(241)),
            ("hardware_confirmed", json!(true)),
            ("observed_bindings", json!([])),
            ("source_response_error", json!("different error")),
        ] {
            let mut invalid = failed_query.clone();
            invalid[field] = value;
            assert!(
                validate_unpair_completion(&invalid, 713).is_err(),
                "{field}"
            );
        }
    }

    #[::core::prelude::v1::test]
    fn extra_empty_unpair_readback_stays_diagnostic() {
        let event = DockPairingEvent::request(
            "DUALLINK_UNBIND_DEVICE",
            json!({"productId":183,"category":"MOUSE"}),
        );
        let observed = readback_reply(164, &[]);
        let mut result = unpair_ack(183);
        record_unpair_readback(164, OWNER, 183, &mut result, Ok(observed.clone()));
        assert_eq!(result["binding_readback"], observed);
        assert_eq!(result["binding_readback_confirmed"], true);
        assert!(result["binding_readback_error"].is_null());
        assert!(receiver_readback_for_refresh(&event, &result).is_none());
        assert!(AppShell::dock_pairing_result(164, &event, result.clone()).is_ok());
        let pair = DockPairingEvent::request(
            "DUALLINK_BIND_DEVICE",
            json!({"mode":1,"device":{"productId":182,"dongleId":183,"category":"MOUSE"}}),
        );
        assert_eq!(
            receiver_readback_for_refresh(&pair, &result),
            Some(&observed)
        );
    }

    #[::core::prelude::v1::test]
    fn source_factory_category_precedes_unknown_scan_presentation_validation() {
        let event = DockPairingEvent::request(
            "DUALLINK_BIND_DEVICE",
            json!({"mode":1,"device":{"productId":0,"dongleId":60000,"category":"","editionId":129}}),
        );
        assert!(action(&event, None).is_err());
        let (category, action) = action(&event, Some("MOUSE")).unwrap();
        assert_eq!(category, "MOUSE");
        assert!(matches!(action, PairingAction::Pair { dongle_id: 60000 }));
        let unpair = DockPairingEvent::request(
            "DUALLINK_UNBIND_DEVICE",
            json!({"productId":713,"category":"MOUSE"}),
        );
        assert_eq!(
            super::action(&unpair, Some("KEYBOARD")).unwrap().0,
            "KEYBOARD"
        );
    }

    #[::core::prelude::v1::test]
    fn source756_pair_ui_rebuilds_unregistered_identity_after_matching_event() {
        let event = DockPairingEvent::request(
            "DUALLINK_BIND_DEVICE",
            json!({"mode":1,"device":{"productId":181,"dongleId":180,"category":"MOUSE","editionId":128,"layoutId":0,"productName":{"en":"scan name"}}}),
        );
        let result = json!({"operation":"pair","source_product_id":756,"category":"MOUSE","source_ui_response":true,"completion_evidence":"hardware_event_54_status_2","pair":{"hardware_status":2,"dongle_id":180},"post_pair_metadata":{"reading":{"edition_id":5,"keyboard_layout":6},"metadata_complete":true},"local_cache_persisted":true});
        let ui = AppShell::dock_pairing_result(241, &event, result.clone()).unwrap();
        assert_eq!(ui["device"]["productId"], 0);
        assert_eq!(ui["device"]["dongleId"], 180);
        assert_eq!(ui["device"]["editionId"], 5);
        assert_eq!(ui["device"]["layoutId"], 6);
        assert_eq!(
            ui["device"]["productName"]["en"],
            "Razer Naga V2 Hyperspeed"
        );
        assert_eq!(ui["post_pair_metadata_observed"], true);
        let mut mismatch = result.clone();
        mismatch["pair"]["dongle_id"] = json!(183);
        assert!(AppShell::dock_pairing_result(241, &event, mismatch).is_err());
        let mut callback_abort = result;
        callback_abort["source_ui_response"] = json!(false);
        assert!(AppShell::dock_pairing_result(241, &event, callback_abort).is_err());
    }
}
