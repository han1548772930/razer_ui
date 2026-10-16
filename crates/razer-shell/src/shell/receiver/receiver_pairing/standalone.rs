//! Current local multipairing commands, physical owner routing and cleanup.
use super::*;
use razer_app_pages::pairing_page::PairingTransport;
use razer_discovery::discovery::ReceiverQueryProjection;
use std::{collections::BTreeMap, future::Future, pin::Pin};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Owner {
    product_id: u32,
    container: String,
}
impl Owner {
    fn parse(info: &Value) -> anyhow::Result<Self> {
        let number = |key: &str| {
            info[key]
                .as_u64()
                .or_else(|| info[key].as_str()?.parse().ok())
        };
        let product_id = number("dongleId")
            .filter(|id| *id > 0)
            .or_else(|| number("productId"))
            .and_then(|id| u32::try_from(id).ok())
            .context("配对主设备缺少实际产品 ID")?;
        let container = info["deviceContainerId"]
            .as_str()
            .or_else(|| info["containerId"].as_str())
            .context("配对主设备缺少物理 ContainerId")?;
        ensure!(
            !uuid::Uuid::parse_str(container)?.is_nil(),
            "配对主设备 ContainerId 为零"
        );
        ensure!(
            u16::try_from(product_id)
                .ok()
                .and_then(razer_device::receiver_capabilities::capability)
                .is_some(),
            "当前物理主设备没有已核验接收器能力"
        );
        Ok(Self {
            product_id,
            container: container.to_owned(),
        })
    }
}

struct Job {
    owner: Owner,
    canceled: Arc<AtomicBool>,
    worker: JoinHandle<()>,
}
#[derive(Default)]
struct Jobs {
    active: BTreeMap<(Owner, String), Job>,
    retired: Vec<Job>,
}
pub(super) struct Completion {
    pub(super) product_id: u32,
    pub(super) container: String,
    pub(super) projection: Option<ReceiverQueryProjection>,
    pub(super) cache_changed: bool,
    pub(super) diagnostic: Option<String>,
}
pub(in crate::shell) struct Transport {
    executor: BackgroundExecutor,
    jobs: Mutex<Jobs>,
    completions: Arc<Mutex<Vec<Completion>>>,
    reconnects: Arc<reconnect::Owners>,
    closed: AtomicBool,
}
struct RequestGuard(Arc<AtomicBool>);
impl Drop for RequestGuard {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}
impl Transport {
    pub(in crate::shell) fn new(executor: BackgroundExecutor) -> Self {
        Self {
            executor,
            jobs: Mutex::new(Jobs::default()),
            completions: Arc::new(Mutex::new(Vec::new())),
            reconnects: Arc::new(reconnect::Owners::default()),
            closed: AtomicBool::new(false),
        }
    }
    pub(in crate::shell) fn reconnect_owners(&self) -> &Arc<reconnect::Owners> {
        &self.reconnects
    }
    pub(in crate::shell) fn cancel_all(&self) {
        if let Ok(jobs) = self.jobs.lock() {
            for job in jobs.active.values().chain(&jobs.retired) {
                job.canceled.store(true, Ordering::Release);
            }
        }
    }
    pub(super) fn shutdown(&self) {
        self.closed.store(true, Ordering::Release);
        self.cancel_all();
        if let Ok(mut jobs) = self.jobs.lock() {
            let active = std::mem::take(&mut jobs.active);
            let retired = std::mem::take(&mut jobs.retired);
            drop(jobs);
            for job in active.into_values().chain(retired) {
                let _ = job.worker.join();
            }
        }
    }
    pub(super) fn take_completions(&self) -> Vec<Completion> {
        self.completions
            .lock()
            .map(|mut values| std::mem::take(&mut *values))
            .unwrap_or_default()
    }
    fn cancel_owner(&self, owner: &Owner) -> anyhow::Result<Value> {
        let jobs = self
            .jobs
            .lock()
            .map_err(|_| anyhow::anyhow!("配对工作线程锁不可用"))?;
        let mut count = 0;
        for job in jobs
            .active
            .values()
            .chain(&jobs.retired)
            .filter(|job| &job.owner == owner)
        {
            job.canceled.store(true, Ordering::Release);
            count += 1;
        }
        // Source CANCEL has no response handler. This is a worker receipt,
        // never a manufactured device acknowledgement or cleanup success.
        Ok(json!({"worker_cancellation_requested":count}))
    }
    fn start(
        &self,
        info: Value,
        operation: &'static str,
        payload: Value,
        external: bool,
    ) -> anyhow::Result<Pin<Box<dyn Future<Output = Result<Value, String>> + Send>>> {
        ensure!(!self.closed.load(Ordering::Acquire), "配对服务所有者已关闭");
        let owner = Owner::parse(&info)?;
        if operation == "DUALLINK_CANCEL" {
            let result = self.cancel_owner(&owner)?;
            return Ok(Box::pin(async move { Ok(result) }));
        }
        ensure!(
            matches!(
                operation,
                "DUALLINK_BIND_INFO"
                    | "DUALLINK_SCAN_DEVICE"
                    | "DUALLINK_BIND_DEVICE"
                    | "DUALLINK_UNBIND_DEVICE"
            ),
            "配对操作没有当前本地调用链"
        );
        ensure!(
            !external || operation == "DUALLINK_UNBIND_DEVICE",
            "外部配对只能向实际原owner解除绑定"
        );
        let event = DockPairingEvent::request(operation, payload);
        let category = if operation == "DUALLINK_BIND_INFO" {
            "read".to_owned()
        } else {
            // External Unpair retains the caller's category; the selected
            // physical master owns the namespace, without a756 override.
            action(&event, None)?.0
        };
        let key = (owner.clone(), category);
        let mut jobs = self
            .jobs
            .lock()
            .map_err(|_| anyhow::anyhow!("配对工作线程锁不可用"))?;
        let finished: Vec<_> = jobs
            .active
            .iter()
            .filter(|(_, job)| job.worker.is_finished())
            .map(|(key, _)| key.clone())
            .collect();
        for key in finished {
            if let Some(job) = jobs.active.remove(&key) {
                let _ = job.worker.join();
            }
        }
        let mut pending = Vec::new();
        for job in jobs.retired.drain(..) {
            if job.worker.is_finished() {
                let _ = job.worker.join();
            } else {
                pending.push(job);
            }
        }
        jobs.retired = pending;
        ensure!(
            jobs.active.len() + jobs.retired.len() < 8,
            "前次配对清理尚未完成，请稍候"
        );
        if let Some(job) = jobs.active.remove(&key) {
            job.canceled.store(true, Ordering::Release);
            jobs.retired.push(job);
        }
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        let worker_owner = owner.clone();
        let completions = self.completions.clone();
        let reconnects = self.reconnects.clone();
        let reconnect_scope =
            reconnects.scope("standalone-pairing", owner.product_id, &owner.container)?;
        let (sender, receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("standalone-pairing".into())
            .spawn(move || {
                let mut source_sent = false;
                let mut early_persistence = false;
                let mut source_response = |step| match step {
                    UnpairStep::SourceResponse(value) => {
                        source_sent = true;
                        let _ = sender.send(AppShell::dock_pairing_result(
                            worker_owner.product_id,
                            &event,
                            value,
                        ));
                    }
                    UnpairStep::LocalPersistence(value) => {
                        early_persistence = true;
                        if let Ok(mut completions) = completions.lock() {
                            completions.push(Completion {
                                product_id: worker_owner.product_id,
                                container: worker_owner.container.clone(),
                                projection: None,
                                cache_changed: value["local_cache_persisted"] == true,
                                diagnostic: value["local_cache_error"]
                                    .as_str()
                                    .or_else(|| value["reconnect_error"].as_str())
                                    .map(str::to_owned),
                            });
                        }
                    }
                };
                let result = perform(
                    &worker_owner,
                    &event,
                    &signal,
                    (&reconnects, &reconnect_scope),
                    &mut source_response,
                );
                if source_sent || !signal.load(Ordering::Acquire) {
                    if let Ok((value, projection)) = &result {
                        if let Ok(mut completions) = completions.lock() {
                            completions.push(Completion {
                                product_id: worker_owner.product_id,
                                container: worker_owner.container.clone(),
                                projection: projection.clone(),
                                cache_changed: !early_persistence
                                    && value["local_cache_persisted"] == true,
                                diagnostic: value["local_cache_error"]
                                    .as_str()
                                    .or_else(|| value["binding_readback_error"].as_str())
                                    .or_else(|| value["reconnect_error"].as_str())
                                    .or_else(|| value["pairing_worker_cleanup_error"].as_str())
                                    .map(str::to_owned),
                            });
                        }
                    }
                    if !source_sent {
                        let _ = sender.send(
                            result
                                .map(|(value, _)| value)
                                .map_err(|error| format!("{error:#}")),
                        );
                    }
                }
            })?;
        jobs.active.insert(
            key,
            Job {
                owner,
                canceled: canceled.clone(),
                worker,
            },
        );
        let executor = self.executor.clone();
        let guard = RequestGuard(canceled);
        Ok(Box::pin(async move {
            let _guard = guard;
            loop {
                match receiver.try_recv() {
                    Ok(value) => return value,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        return Err("配对工作线程已取消或断开，结果未确认".into());
                    }
                    Err(mpsc::TryRecvError::Empty) => {
                        executor.timer(Duration::from_millis(25)).await
                    }
                }
            }
        }))
    }
}
impl PairingTransport for Transport {
    fn request(
        &self,
        master: Value,
        operation: &'static str,
        payload: Value,
        external: bool,
    ) -> Pin<Box<dyn Future<Output = Result<Value, String>> + Send>> {
        match self.start(master, operation, payload, external) {
            Ok(future) => future,
            Err(error) => Box::pin(async move { Err(format!("{error:#}")) }),
        }
    }
}

fn perform(
    owner: &Owner,
    event: &DockPairingEvent,
    canceled: &AtomicBool,
    reconnect_owner: (&reconnect::Owners, &reconnect::Scope),
    source_response: &mut dyn FnMut(UnpairStep),
) -> anyhow::Result<(Value, Option<ReceiverQueryProjection>)> {
    if event.kind() == "DUALLINK_BIND_INFO" {
        ensure!(!canceled.load(Ordering::Acquire), "配对读取已取消");
        let mut client = ServiceClient::spawn()?;
        let result = (|| {
            let path = route::command_path(&mut client, owner.product_id, &owner.container)?;
            ensure!(!canceled.load(Ordering::Acquire), "配对读取已取消，未提交");
            let value = client.request(ServiceRequest::ReceiverWirelessStatus {
                path,
                device_container_id: owner.container.clone(),
                source_product_id: None,
            })?;
            let projection = razer_discovery::discovery::project_receiver_query(
                owner.product_id,
                &owner.container,
                &value,
            )?;
            ensure!(!canceled.load(Ordering::Acquire), "配对读取已取消，未发布");
            Ok((projection.pairing_payload()?, Some(projection)))
        })();
        let shutdown = client.request(ServiceRequest::Shutdown);
        return match (result, shutdown) {
            (Ok(value), Ok(_)) => Ok(value),
            (Err(error), _) | (_, Err(error)) => Err(error),
        };
    }
    let (sender, _receiver) = mpsc::channel();
    let actual = run(
        owner.product_id,
        &owner.container,
        event,
        None,
        None,
        canceled,
        &sender,
        reconnect_owner,
        source_response,
    )?;
    let projection = receiver_readback_for_refresh(event, &actual)
        .map(|value| {
            razer_discovery::discovery::project_receiver_query(
                owner.product_id,
                &owner.container,
                value,
            )
        })
        .transpose()?;
    let projected = AppShell::dock_pairing_result(owner.product_id, event, actual)
        .map_err(anyhow::Error::msg)?;
    Ok((projected, projection))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn owner_uses_actual_radio_and_requires_external_physical_scope() {
        let owner = Owner::parse(
            &json!({"productId":164,"deviceContainerId":"{11111111-2222-3333-4444-555555555555}"}),
        )
        .unwrap();
        assert_eq!(owner.product_id, 164);
        assert!(Owner::parse(&json!({"productId":164})).is_err());
        assert!(Owner::parse(&json!({"productId":164,"deviceContainerId":"{00000000-0000-0000-0000-000000000000}"})).is_err());
    }
}
