//! Cancellable worker lifecycle for source-backed receiver commands/events.
//! Completion describes hardware evidence only; profile publication is separate.
use super::{hid, hid_transport, receiver, receiver_events};
use anyhow::{Context as _, ensure};
use razer_device::receiver_pairing::{PairingAction, PairingCapability, Session, for_source};
use serde_json::{Value, json};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

// Local application timeout, not a firmware or source-protocol constant.
const OPERATION_BUDGET: Duration = Duration::from_secs(90);

struct Operation {
    id: String,
    canceled: Arc<AtomicBool>,
    thread: Option<JoinHandle<Result<Value, String>>>,
    result: Option<Result<Value, String>>,
}

#[derive(Default)]
pub(super) struct Controller {
    active: Option<Operation>,
}

impl Controller {
    pub(super) fn start(
        &mut self,
        id: String,
        path: String,
        container: String,
        category: String,
        action: PairingAction,
        source_product_id: Option<u16>,
    ) -> anyhow::Result<Value> {
        ensure!(
            !id.is_empty() && id.len() <= 128 && !id.contains('\0'),
            "配对操作 ID 无效"
        );
        ensure!(self.active.is_none(), "之前的配对操作尚未领取完成结果");
        ensure!(
            receiver::valid_container(&container),
            "配对 ContainerId 格式无效"
        );
        ensure!(
            matches!(category.as_str(), "MOUSE" | "KEYBOARD"),
            "配对产品类别无效"
        );
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = Arc::clone(&canceled);
        let worker = thread::Builder::new()
            .name("receiver-pairing".into())
            .spawn(move || {
                execute(
                    &path,
                    &container,
                    &category,
                    action,
                    source_product_id,
                    &signal,
                )
                .map_err(|error| format!("{error:#}"))
            })?;
        self.active = Some(Operation {
            id: id.clone(),
            canceled,
            thread: Some(worker),
            result: None,
        });
        Ok(
            json!({"operation_id":id,"state":"running","application_budget_ms":OPERATION_BUDGET.as_millis() as u64}),
        )
    }

    pub(super) fn poll(&mut self, id: &str) -> anyhow::Result<Value> {
        let operation = self.active.as_mut().context("没有配对操作")?;
        ensure!(operation.id == id, "配对操作 ID 不匹配");
        if operation
            .thread
            .as_ref()
            .is_some_and(JoinHandle::is_finished)
        {
            let worker = operation.thread.take().unwrap();
            operation.result = Some(
                worker
                    .join()
                    .unwrap_or_else(|_| Err("配对线程异常终止；未确认设备写回".into())),
            );
        }
        let Some(result) = operation.result.take() else {
            return Ok(
                json!({"operation_id":id,"state":if operation.canceled.load(Ordering::Acquire) {"canceling"} else {"running"}}),
            );
        };
        let canceled = operation.canceled.load(Ordering::Acquire);
        self.active = None;
        Ok(match result {
            Ok(result) => json!({"operation_id":id,"state":"completed","result":result}),
            Err(error) => {
                json!({"operation_id":id,"state":if canceled {"canceled"} else {"failed"},"error":error})
            }
        })
    }

    pub(super) fn cancel(&mut self, id: &str) -> anyhow::Result<Value> {
        let operation = self.active.as_ref().context("没有配对操作")?;
        ensure!(operation.id == id, "配对操作 ID 不匹配");
        operation.canceled.store(true, Ordering::Release);
        // This is acceptance of cancellation. Poll reports command/reader
        // cleanup completion, including failures; it never claims immediate save.
        Ok(json!({"operation_id":id,"state":"canceling"}))
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        if let Some(mut operation) = self.active.take() {
            operation.canceled.store(true, Ordering::Release);
            if let Some(worker) = operation.thread.take() {
                // Shutdown must wait for the original cancel command and the
                // retained input handles to finish instead of leaking a reader.
                let _ = worker.join();
            }
        }
    }
}

fn metadata_target(
    snapshot: &Value,
    before: &Value,
    cap: &PairingCapability,
) -> anyhow::Result<Value> {
    ensure!(snapshot["complete"] == true, "配对元数据接口枚举未完成");
    let interface = cap
        .metadata_claim_interface
        .unwrap_or(cap.transport.claim_interface);
    let selected = snapshot["interfaces"]
        .as_array()
        .context("元数据接口列表缺失")?
        .iter()
        .filter(|row| {
            row["vendor_id"] == cap.transport.vendor_id
                && row["product_id"] == cap.product_id
                && row["claim_interface"] == interface
                && row["device_container_id"]
                    .as_str()
                    .zip(before["device_container_id"].as_str())
                    .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
        })
        .collect::<Vec<_>>();
    let [target] = selected.as_slice() else {
        anyhow::bail!("配对元数据接口缺失或不唯一（interface {interface}）");
    };
    ensure!(
        target["path"].as_str().is_some_and(|path| {
            path.is_ascii()
                && !path.contains('\0')
                && path.to_ascii_lowercase().starts_with(r"\\?\hid#")
        }) && target["device_instance_id"]
            .as_str()
            .is_some_and(|id| !id.is_empty())
            && target["feature_report_bytes"]
                .as_u64()
                .is_some_and(|count| count > 0),
        "元数据接口缺少真实路径、实例或 Feature 长度"
    );
    Ok((*target).clone())
}

fn post_pair_metadata(
    before: &Value,
    container: &str,
    category: &str,
    cap: &PairingCapability,
    events: &mut receiver_events::Events,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> Value {
    let result = (|| -> anyhow::Result<Value> {
        validate()?;
        let target = metadata_target(
            &hid::enumerate_for_product(cap.transport.vendor_id, cap.product_id)?,
            before,
            cap,
        )?;
        let path = target["path"].as_str().unwrap();
        let bytes = target["feature_report_bytes"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .context("元数据 Feature 长度无效")?;
        let metadata_device = hid_transport::open(path, bytes)?;
        let metadata_validate = || {
            validate()?;
            let current = metadata_target(
                &hid::enumerate_collection(cap.transport.vendor_id, cap.product_id, path)?,
                before,
                cap,
            )?;
            ensure!(current == target, "配对后的元数据物理接口已变化");
            Ok(())
        };
        let mut session = Session::new(&metadata_device, events, cap)?;
        let (edition, edition_error) = match session.get_edition(&metadata_validate) {
            Ok(reading) => (Some(reading), None),
            Err(error) => (None, Some(format!("{error:#}"))),
        };
        let layout = edition.as_ref().map(|reading| reading.keyboard_layout);
        let mut metadata = json!({"source":"getEdition","reading":edition,"read_error":edition_error,"metadata_complete":layout.is_some(),"layout_source":if layout.is_some(){"edition_response_byte_0"}else{"unobserved"}, "transport":{"path":path,"device_container_id":container,"device_instance_id":target["device_instance_id"],"claim_interface":target["claim_interface"],"feature_report_bytes":bytes,"source_class":cap.metadata_source_class,"transaction_prefix":cap.metadata_prefix}});
        if category == "MOUSE" && cap.metadata_mouse_layout {
            // The source still requests the hardware module after an edition
            // failure. Its reply cannot establish the missing edition byte.
            let used = layout.is_none_or(|value| value == 0);
            let (status, error) = if used {
                match session.get_mouse_layout(&metadata_validate) {
                    Ok(status) => (Some(status), None),
                    Err(error) => (None, Some(format!("{error:#}"))),
                }
            } else {
                (None, None)
            };
            metadata["mouse_layout_followup"] =
                json!({"source":"getHWModule","status":status,"used":used,"read_error":error});
            metadata["metadata_complete"] = json!(layout.is_some() && (!used || status.is_some()));
        }
        Ok(metadata)
    })();
    result.unwrap_or_else(|error| json!({"source":"getEdition","reading":null,"read_error":format!("{error:#}"),"metadata_complete":false,"layout_source":"unobserved"}))
}

fn execute(
    path: &str,
    container: &str,
    category: &str,
    action: PairingAction,
    source_product_id: Option<u16>,
    canceled: &AtomicBool,
) -> anyhow::Result<Value> {
    let started = Instant::now();
    let (before, query_cap) =
        receiver::select_target(&hid::enumerate_metadata()?, path, container)?;
    let cap = for_source(query_cap.product_id, category, source_product_id)?;
    let query_cap =
        razer_device::receiver_capabilities::for_source(query_cap.product_id, source_product_id)?;
    let identity = || {
        let (current, current_cap) = receiver::select_target(
            &hid::enumerate_collection(query_cap.vendor_id, query_cap.product_id, path)?,
            path,
            container,
        )?;
        ensure!(
            current["device_instance_id"] == before["device_instance_id"]
                && current_cap.product_id == query_cap.product_id,
            "配对操作中的接收器实例变化"
        );
        Ok(())
    };
    let validate = || {
        ensure!(!canceled.load(Ordering::Acquire), "配对操作已取消");
        ensure!(started.elapsed() < OPERATION_BUDGET, "配对操作等待超时");
        identity()
    };
    let _receiver_lock = receiver::ReceiverLock::acquire(container, &validate)?;
    validate()?;
    let feature_bytes = before["feature_report_bytes"]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .context("缺少实际 Feature 长度")?;
    let device = hid_transport::open(path, feature_bytes)?;
    let mut events = receiver_events::Events::new(query_cap.product_id, container);
    let mut session = Session::new(&device, &mut events, &cap)?;
    let deadline = started + OPERATION_BUDGET;
    let mut next_transaction_sequence = None;
    let result = match action {
        PairingAction::Cancel => {
            session.cancel(&validate)?;
            json!({"operation":"cancel","completion_evidence":"feature_acknowledgement"})
        }
        PairingAction::Scan => {
            json!({"operation":"scan","candidates":session.scan(deadline, &validate, &identity)?,"completion_evidence":"hardware_event_55_status_2"})
        }
        PairingAction::Pair { dongle_id } => {
            let pair = session.pair(dongle_id, 1, deadline, &validate, &identity)?;
            next_transaction_sequence = Some(session.next_transaction_sequence());
            // Metadata creates a separate source factory session, which needs
            // the event owner after this pairing session releases its borrow.
            drop(session);
            let callback_catalog = (cap.transport.source_product_id == 756)
                .then(|| razer_device::receiver_pairing::source756_pair_catalog_device(dongle_id));
            if let Some(Err(error)) = callback_catalog {
                // The real hardware event remains evidence, but current756's
                // callback throws before metadata and never posts MW_RESPONSE_UI.
                json!({"operation":"pair","pair":pair,"completion_evidence":"hardware_event_54_status_2","source_ui_response":false,"source_callback_error":format!("{error:#}"),"runtime_published":false})
            } else {
                //756 ye defaults to interface2 because scalar DeviceInfo.claimInterface
                // has no [dongleId] property. It must open a separate fresh collection.
                let metadata =
                    post_pair_metadata(&before, container, category, &cap, &mut events, &validate);
                validate()?;
                json!({"operation":"pair","pair":pair,"completion_evidence":"hardware_event_54_status_2","source_ui_response":true,"runtime_published":false,"post_pair_metadata":metadata})
            }
        }
        PairingAction::Unpair { dongle_id } => {
            let unpaired = session.unpair(dongle_id, &validate);
            // Capture even a rejected setter's consumed sequence before the
            // separate binding-query recovery and reconnect factory start.
            next_transaction_sequence = Some(session.next_transaction_sequence());
            drop(session);
            match unpaired {
                Ok(ack) => {
                    json!({"operation":"unpair","acknowledgement":ack,"completion_evidence":"feature_acknowledgement","runtime_cleanup_complete":false})
                }
                Err(command_error) => {
                    // Te waits exactly 2000ms after a rejected command, then
                    // re-queries binding info. Preserve that recovery instead
                    // of inventing an acknowledgement or swallowing failure.
                    let legacy192 = cap.transport.source_product_id == 192;
                    let recovery_validate = || {
                        ensure!(!canceled.load(Ordering::Acquire), "配对操作已取消");
                        ensure!(started.elapsed() < OPERATION_BUDGET, "配对操作等待超时");
                        if !legacy192 {
                            identity()?;
                        }
                        Ok::<_, anyhow::Error>(())
                    };
                    let recovery_at = Instant::now() + Duration::from_millis(2000);
                    while let Some(remaining) = recovery_at.checked_duration_since(Instant::now()) {
                        recovery_validate()?;
                        thread::sleep(remaining.min(Duration::from_millis(50)));
                    }
                    recovery_validate()?;
                    let transaction = receiver::next_transaction(path, container, &query_cap)?;
                    let observed = if legacy192 {
                        use razer_device::mouse_receiver_legacy as legacy;
                        let binding_cap =
                            legacy::binding_capability(192, u32::from(query_cap.product_id))
                                .context("Current192 recovery binding factory is absent")?;
                        legacy::execute(
                            &device,
                            binding_cap,
                            &legacy::Change::ReadBindings,
                            || Ok(transaction),
                            &validate,
                        )
                        .and_then(|report| {
                            report
                                .bindings
                                .context("Current192 recovery getter omitted its response")
                        })
                        .and_then(|rows| {
                            // Te catches the entire he() owner, including
                            // local presentation JSON and USB role lookup
                            // after a nonempty raw getter. Empty results
                            // perform none of those independent reads.
                            razer_device::receiver_pairing::source192_binding_info(
                                container,
                                &rows,
                                || {
                                    razer_storage::source_local::get_raw(
                                        &razer_storage::source_local::path()?,
                                        "connectedDeviceInfo",
                                    )
                                },
                                hid::enumerate_metadata,
                                &[],
                            )?;
                            Ok(rows)
                        })
                    } else {
                        razer_device::device_query::read_receiver(
                            &device,
                            &query_cap,
                            transaction,
                            &validate,
                        )
                    };
                    // Te's he().catch first sends the original setter error,
                    // then its null branch sends payload:{} and starts p().
                    // This is source restoration, never a successful read or ACK.
                    let query_error = match &observed {
                        Ok(_) => None,
                        Err(error) => Some(format!("{error:#}")),
                    };
                    if legacy192 && query_error.is_some() {
                        ensure!(!canceled.load(Ordering::Acquire), "配对操作已取消");
                        ensure!(started.elapsed() < OPERATION_BUDGET, "配对操作等待超时");
                        json!({"operation":"unpair","acknowledgement":null,"removed_dongle_id":dongle_id,"completion_evidence":"source_restore_after_binding_query_error_2000ms","command_error":format!("{command_error:#}"),"binding_query_error":query_error,"source_response_error":format!("{command_error:#}"),"source_response_payload":{},"hardware_confirmed":false,"runtime_cleanup_complete":false})
                    } else {
                        let observed = observed.with_context(|| {
                            format!("取消配对命令失败且重读失败：{command_error:#}")
                        })?;
                        let bindings: Vec<_> = observed
                            .iter()
                            .filter(|(pid, _)| {
                                *pid != 0 && *pid != u16::MAX && *pid != query_cap.product_id
                            })
                            .collect();
                        let still_bound = if legacy192 {
                            razer_device::mouse_receiver_legacy::has_source_bindings(&observed)
                        } else if query_cap.product_id == 241 {
                            bindings.iter().any(|(pid, _)| *pid == dongle_id)
                        } else {
                            !bindings.is_empty()
                        };
                        ensure!(
                            !still_bound,
                            "取消配对失败，原绑定仍存在：{command_error:#}"
                        );
                        let mut recovered = json!({"operation":"unpair","acknowledgement":null,"removed_dongle_id":dongle_id,"completion_evidence":"binding_query_after_failed_ack_2000ms","command_error":format!("{command_error:#}"),"observed_bindings":observed,"runtime_cleanup_complete":false});
                        if legacy192 {
                            recovered["source_response_payload"] = json!({});
                        }
                        recovered
                    }
                }
            }
        }
    };
    if result["completion_evidence"] != "source_restore_after_binding_query_error_2000ms" {
        identity()?;
    }
    let mut result = result;
    if matches!(action, PairingAction::Unpair { .. }) {
        result["next_transaction_sequence"] = json!(next_transaction_sequence);
    }
    result.as_object_mut().unwrap().extend(json!({"path":path,"device_container_id":container,"device_instance_id":before["device_instance_id"],"product_id":query_cap.product_id,"category":category,"event_provider":events.metadata(),"elapsed_ms":started.elapsed().as_millis() as u64}).as_object().unwrap().clone());
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source756_metadata_selects_fresh_interface_two_and_rejects_ambiguity() {
        let cap = for_source(757, "MOUSE", Some(756)).unwrap();
        let container = "{11111111-2222-3333-4444-555555555555}";
        let row = |interface, path: &str| json!({"vendor_id":5426,"product_id":757,"claim_interface":interface,"device_container_id":container,"feature_report_bytes":91,"device_instance_id":format!("instance-{interface}"),"path":path});
        let command = row(3, r"\\?\hid#command");
        let metadata = row(2, r"\\?\hid#metadata");
        let snapshot = json!({"complete":true,"interfaces":[command.clone(),metadata.clone()]});
        assert_eq!(
            metadata_target(&snapshot, &command, &cap).unwrap(),
            metadata
        );
        assert!(
            metadata_target(
                &json!({"complete":true,"interfaces":[command.clone()]}),
                &command,
                &cap
            )
            .is_err()
        );
        assert!(
            metadata_target(
                &json!({"complete":true,"interfaces":[metadata.clone(),row(2,r"\\?\hid#other")]}),
                &command,
                &cap
            )
            .is_err()
        );
        assert!(
            metadata_target(
                &json!({"complete":false,"interfaces":[metadata]}),
                &command,
                &cap
            )
            .is_err()
        );
    }
}
