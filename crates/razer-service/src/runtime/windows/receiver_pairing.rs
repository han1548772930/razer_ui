//! Cancellable worker lifecycle for source-backed receiver commands/events.
//! Completion describes hardware evidence only; profile publication is separate.
use super::{hid, hid_transport, receiver, receiver_events};
use anyhow::{Context as _, ensure};
use razer_device::receiver_pairing::{PairingAction, Session, capability};
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
                execute(&path, &container, &category, action, &signal)
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

fn execute(
    path: &str,
    container: &str,
    category: &str,
    action: PairingAction,
    canceled: &AtomicBool,
) -> anyhow::Result<Value> {
    let started = Instant::now();
    let (before, query_cap) =
        receiver::select_target(&hid::enumerate_metadata()?, path, container)?;
    let cap =
        capability(query_cap.product_id, category).context("产品和目标类别没有当前配对命令证据")?;
    let _receiver_lock = receiver::ReceiverLock::acquire(container)?;
    let identity = || {
        let (current, current_cap) = receiver::select_target(
            &hid::enumerate_for_product(query_cap.vendor_id, query_cap.product_id)?,
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
    validate()?;
    let feature_bytes = before["feature_report_bytes"]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .context("缺少实际 Feature 长度")?;
    let device = hid_transport::open(path, feature_bytes)?;
    let mut events = receiver_events::Events::new(query_cap.product_id, container);
    let mut session = Session::new(&device, &mut events, cap)?;
    let deadline = started + OPERATION_BUDGET;
    let result = match action {
        PairingAction::Scan => {
            json!({"operation":"scan","candidates":session.scan(deadline, &validate, &identity)?,"completion_evidence":"hardware_event_55_status_2"})
        }
        PairingAction::Pair { dongle_id } => {
            let pair = session.pair(dongle_id, 1, deadline, &validate, &identity)?;
            // Ee uses a new post-pair category factory (f2(false), or the
            // cached opposite-category factory on164), not the scan instance.
            let mut metadata_session = Session::new(&device, &mut events, cap)?;
            let (edition, edition_error) = match metadata_session.get_edition(&validate) {
                Ok(reading) => (Some(reading), None),
                Err(error) => (None, Some(format!("{error:#}"))),
            };
            validate()?;
            let edition_layout = edition.as_ref().map(|reading| reading.keyboard_layout);
            let mut value = json!({"operation":"pair","pair":pair,"completion_evidence":"hardware_event_54_status_2","runtime_published":false,"post_pair_metadata":{"source":"getEdition","reading":edition,"read_error":edition_error,"source_fallback":if edition_layout.is_none(){json!({"edition_id":0,"keyboard_layout":0})}else{Value::Null},"metadata_complete":edition_layout.is_some(),"layout_source":"edition_response_byte_0"}});
            if category == "MOUSE" {
                let used = edition_layout.unwrap_or(0) == 0;
                let (sidepad_layout, sidepad_error) = if used {
                    match metadata_session.get_mouse_layout(&validate) {
                        Ok(status) => (Some(status), None),
                        Err(error) => (None, Some(format!("{error:#}"))),
                    }
                } else {
                    (None, None)
                };
                validate()?;
                value["post_pair_metadata"]["mouse_layout_followup"] = json!({"source":"getHWModule","status":sidepad_layout,"used":used,"read_error":sidepad_error});
                value["post_pair_metadata"]["metadata_complete"] =
                    json!(edition_layout.is_some() && (!used || sidepad_layout.is_some()));
            }
            value
        }
        PairingAction::Unpair { dongle_id } => {
            match session.unpair(dongle_id, &validate) {
                Ok(ack) => {
                    json!({"operation":"unpair","acknowledgement":ack,"completion_evidence":"feature_acknowledgement","runtime_cleanup_complete":false})
                }
                Err(command_error) => {
                    // Te waits exactly 2000ms after a rejected command, then
                    // re-queries binding info. Preserve that recovery instead
                    // of inventing an acknowledgement or swallowing failure.
                    let recovery_at = Instant::now() + Duration::from_millis(2000);
                    while let Some(remaining) = recovery_at.checked_duration_since(Instant::now()) {
                        validate()?;
                        thread::sleep(remaining.min(Duration::from_millis(50)));
                    }
                    validate()?;
                    let transaction = receiver::next_transaction(path, container, query_cap)?;
                    let observed = razer_device::device_query::read_receiver(
                        &device,
                        query_cap,
                        transaction,
                        &validate,
                    )
                    .with_context(|| format!("取消配对命令失败且重读失败：{command_error:#}"))?;
                    let bindings: Vec<_> = observed
                        .iter()
                        .filter(|(pid, _)| {
                            *pid != 0 && *pid != u16::MAX && *pid != query_cap.product_id
                        })
                        .collect();
                    let still_bound = if query_cap.product_id == 241 {
                        bindings.iter().any(|(pid, _)| *pid == dongle_id)
                    } else {
                        !bindings.is_empty()
                    };
                    ensure!(
                        !still_bound,
                        "取消配对失败，原绑定仍存在：{command_error:#}"
                    );
                    json!({"operation":"unpair","acknowledgement":null,"removed_dongle_id":dongle_id,"completion_evidence":"binding_query_after_failed_ack_2000ms","command_error":format!("{command_error:#}"),"observed_bindings":observed,"runtime_cleanup_complete":false})
                }
            }
        }
    };
    identity()?;
    let mut result = result;
    result.as_object_mut().unwrap().extend(json!({"path":path,"device_container_id":container,"device_instance_id":before["device_instance_id"],"product_id":query_cap.product_id,"category":category,"event_provider":events.metadata(),"elapsed_ms":started.elapsed().as_millis() as u64}).as_object().unwrap().clone());
    Ok(result)
}
