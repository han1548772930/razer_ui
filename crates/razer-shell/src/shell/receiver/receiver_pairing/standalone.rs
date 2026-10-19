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
    outcome: Arc<Mutex<Option<Result<(), String>>>>,
}
#[derive(Default)]
struct Jobs {
    active: BTreeMap<(Owner, String), Job>,
    retired: Vec<Job>,
    diagnostics: BTreeMap<Owner, Vec<String>>,
    failures: Arc<CleanupFailures>,
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
    jobs: Arc<Mutex<Jobs>>,
    completions: Arc<Mutex<Vec<Completion>>>,
    reconnects: Arc<reconnect::Owners>,
    closed: AtomicBool,
    shutdown_complete: AtomicBool,
    failures: Arc<CleanupFailures>,
}
struct RequestGuard(Arc<AtomicBool>);
impl Drop for RequestGuard {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}
impl Transport {
    pub(in crate::shell) fn new(executor: BackgroundExecutor) -> Self {
        let failures = Arc::new(CleanupFailures::default());
        Self {
            executor,
            jobs: Arc::new(Mutex::new(Jobs {
                failures: failures.clone(),
                ..Jobs::default()
            })),
            completions: Arc::new(Mutex::new(Vec::new())),
            reconnects: Arc::new(reconnect::Owners::with_failures(failures.clone())),
            closed: AtomicBool::new(false),
            shutdown_complete: AtomicBool::new(false),
            failures,
        }
    }
    pub(super) fn replacement(&self) -> anyhow::Result<Self> {
        ensure!(
            self.shutdown_complete.load(Ordering::Acquire) && self.reconnects.shutdown_complete(),
            "接收器旧所有者清理尚未结束"
        );
        if let Some(error) = self.cleanup_failure() {
            anyhow::bail!("接收器旧所有者清理失败：{error}");
        }
        Ok(Self::new(self.executor.clone()))
    }
    pub(super) fn cleanup_failure(&self) -> Option<String> {
        self.failures.summary()
    }
    pub(super) fn cleanup_witness(&self, product_id: u32, container: &str) -> CleanupWitness {
        CleanupWitness::new(self.failures.clone(), product_id, container)
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
    pub(super) fn stop_admission(&self) {
        self.closed.store(true, Ordering::Release);
        self.reconnects.stop_admission();
        self.cancel_all();
    }
    pub(super) fn shutdown(&self) {
        self.stop_admission();
        let (active, retired, diagnostics) = {
            let mut jobs = self
                .jobs
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let active = std::mem::take(&mut jobs.active);
            let retired = std::mem::take(&mut jobs.retired);
            (active, retired, std::mem::take(&mut jobs.diagnostics))
        };
        let mut joined = Jobs {
            diagnostics,
            failures: self.failures.clone(),
            ..Jobs::default()
        };
        for job in active.into_values().chain(retired) {
            record_finished_job(&mut joined, job);
        }
        let mut completions = self
            .completions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (owner, diagnostics) in joined.diagnostics {
            if diagnostics.is_empty() {
                continue;
            }
            completions.push(Completion {
                product_id: owner.product_id,
                container: owner.container,
                projection: None,
                cache_changed: false,
                diagnostic: Some(diagnostics.join("; ")),
            });
        }
        self.shutdown_complete.store(true, Ordering::Release);
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
            .filter(|job| &job.owner == owner && !job.worker.is_finished())
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
        publish_refresh: bool,
    ) -> anyhow::Result<Pin<Box<dyn Future<Output = Result<Value, String>> + Send>>> {
        ensure!(!self.closed.load(Ordering::Acquire), "配对服务所有者已关闭");
        let owner = Owner::parse(&info)?;
        if operation == "DUALLINK_CANCEL" {
            let result = self.cancel_owner(&owner)?;
            if result["worker_cancellation_requested"]
                .as_u64()
                .is_some_and(|count| count > 0)
            {
                return Ok(Box::pin(async move { Ok(result) }));
            }
        }
        ensure!(
            matches!(
                operation,
                "DUALLINK_BIND_INFO"
                    | "DUALLINK_CANCEL"
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
        ensure!(!self.closed.load(Ordering::Acquire), "配对服务所有者已关闭");
        prune_finished_jobs(&mut jobs);
        let cleanup_failures = self.failures.for_owner(owner.product_id, &owner.container);
        ensure!(
            cleanup_failures.is_empty(),
            "配对前次资源清理失败：{}",
            cleanup_failures.join("; ")
        );
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
        let mut cleanup = self.cleanup_witness(owner.product_id, &owner.container);
        let (sender, receiver) = mpsc::channel();
        let outcome = Arc::new(Mutex::new(None));
        let worker_outcome = outcome.clone();
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
                    &mut cleanup.result,
                );
                drop(cleanup);
                if let Ok(mut outcome) = worker_outcome.lock() {
                    *outcome = Some(
                        result
                            .as_ref()
                            .map(|_| ())
                            .map_err(|error| format!("{error:#}")),
                    );
                }
                if source_sent || !signal.load(Ordering::Acquire) {
                    if let Ok((value, projection)) = &result {
                        if let Ok(mut completions) = completions.lock() {
                            completions.push(Completion {
                                product_id: worker_owner.product_id,
                                container: worker_owner.container.clone(),
                                projection: projection.clone().filter(|_| publish_refresh),
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
                outcome,
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
    pub(in crate::shell) fn cancel_product_blocking(
        &self,
        master: Value,
    ) -> Result<Vec<String>, String> {
        let owner = Owner::parse(&master).map_err(|error| format!("{error:#}"))?;
        self.cancel_owner(&owner)
            .map_err(|error| format!("{error:#}"))?;
        loop {
            match finish_product_jobs(&self.jobs, &owner) {
                Ok(Some(diagnostics)) => return Ok(diagnostics),
                Err(error) => return Err(format!("{error:#}")),
                Ok(None) => thread::sleep(Duration::from_millis(25)),
            }
        }
    }
    /// Cancellation is a request, while the retained Jobs registry owns every
    /// worker until its full service shutdown and thread join have completed.
    /// Superseding this future cannot hide an unfinished physical predecessor.
    pub(in crate::shell) fn cancel_product_and_wait(
        &self,
        master: Value,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<String>, String>> + Send>> {
        self.product_cleanup(master, true)
    }
    pub(in crate::shell) fn finish_product_and_wait(
        &self,
        master: Value,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<String>, String>> + Send>> {
        self.product_cleanup(master, false)
    }
    fn product_cleanup(
        &self,
        master: Value,
        cancel: bool,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<String>, String>> + Send>> {
        let owner = match Owner::parse(&master) {
            Ok(owner) => owner,
            Err(error) => return Box::pin(async move { Err(format!("{error:#}")) }),
        };
        if cancel && let Err(error) = self.cancel_owner(&owner) {
            return Box::pin(async move { Err(format!("{error:#}")) });
        }
        let jobs = self.jobs.clone();
        let executor = self.executor.clone();
        Box::pin(async move {
            loop {
                let result = finish_product_jobs(&jobs, &owner);
                match result {
                    Ok(Some(diagnostics)) => return Ok(diagnostics),
                    Err(error) => return Err(format!("{error:#}")),
                    Ok(None) => executor.timer(Duration::from_millis(25)).await,
                }
            }
        })
    }
    /// A mounted product retains its own physical connection scope and refresh
    /// loop. Its completion must not publish through the standalone window.
    pub(in crate::shell) fn request_product(
        &self,
        master: Value,
        operation: &'static str,
        payload: Value,
    ) -> Pin<Box<dyn Future<Output = Result<Value, String>> + Send>> {
        match self.start(master, operation, payload, false, false) {
            Ok(future) => future,
            Err(error) => Box::pin(async move { Err(format!("{error:#}")) }),
        }
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
        match self.start(master, operation, payload, external, true) {
            Ok(future) => future,
            Err(error) => Box::pin(async move { Err(format!("{error:#}")) }),
        }
    }
}

fn finish_product_jobs(jobs: &Mutex<Jobs>, owner: &Owner) -> anyhow::Result<Option<Vec<String>>> {
    let mut jobs = jobs
        .lock()
        .map_err(|_| anyhow::anyhow!("Pairing cleanup lock poisoned"))?;
    let keys = jobs
        .active
        .iter()
        .filter(|(_, job)| &job.owner == owner && job.worker.is_finished())
        .map(|(key, _)| key.clone())
        .collect::<Vec<_>>();
    let mut finished = Vec::new();
    for key in keys {
        if let Some(job) = jobs.active.remove(&key) {
            finished.push(job);
        }
    }
    let mut pending = Vec::new();
    for job in jobs.retired.drain(..) {
        if &job.owner == owner && job.worker.is_finished() {
            finished.push(job);
        } else {
            pending.push(job);
        }
    }
    jobs.retired = pending;
    for job in finished {
        record_finished_job(&mut jobs, job);
    }
    if jobs
        .active
        .values()
        .chain(&jobs.retired)
        .any(|job| &job.owner == owner)
    {
        return Ok(None);
    }
    let mut diagnostics = jobs.diagnostics.get(owner).cloned().unwrap_or_default();
    diagnostics.extend(jobs.failures.for_owner(owner.product_id, &owner.container));
    Ok(Some(diagnostics))
}

fn record_finished_job(jobs: &mut Jobs, job: Job) {
    if job.worker.join().is_err() {
        jobs.failures.record(
            job.owner.product_id,
            &job.owner.container,
            "Pairing predecessor panicked".into(),
        );
        return;
    }
    match job.outcome.lock() {
        Ok(mut outcome) => match outcome.take() {
            Some(Err(error)) => jobs.diagnostics.entry(job.owner).or_default().push(error),
            Some(Ok(())) => {}
            None => jobs.failures.record(
                job.owner.product_id,
                &job.owner.container,
                "Pairing predecessor omitted its completion".into(),
            ),
        },
        Err(_) => jobs.failures.record(
            job.owner.product_id,
            &job.owner.container,
            "Pairing predecessor result lock poisoned".into(),
        ),
    }
}

fn prune_finished_jobs(jobs: &mut Jobs) {
    let keys = jobs
        .active
        .iter()
        .filter(|(_, job)| job.worker.is_finished())
        .map(|(key, _)| key.clone())
        .collect::<Vec<_>>();
    for key in keys {
        if let Some(job) = jobs.active.remove(&key) {
            record_finished_job(jobs, job);
        }
    }
    let mut pending = Vec::new();
    for job in std::mem::take(&mut jobs.retired) {
        if job.worker.is_finished() {
            record_finished_job(jobs, job);
        } else {
            pending.push(job);
        }
    }
    jobs.retired = pending;
}

fn perform(
    owner: &Owner,
    event: &DockPairingEvent,
    canceled: &AtomicBool,
    reconnect_owner: (&reconnect::Owners, &reconnect::Scope),
    source_response: &mut dyn FnMut(UnpairStep),
    cleanup: &mut Result<(), String>,
) -> anyhow::Result<(Value, Option<ReceiverQueryProjection>)> {
    if event.kind() == "DUALLINK_BIND_INFO" {
        ensure!(!canceled.load(Ordering::Acquire), "配对读取已取消");
        let mut client = ServiceClient::spawn()?;
        *cleanup = Err("receiver read service cleanup receipt is missing".into());
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
        let shutdown = finish_pairing_service(&mut client, cleanup);
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
        cleanup,
    )?;
    if event.kind() == "DUALLINK_CANCEL" {
        ensure!(
            actual["operation"] == "cancel"
                && actual["completion_evidence"] == "feature_acknowledgement",
            "取消缺少主实例 scan4 确认"
        );
        return Ok((actual, None));
    }
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
    fn standalone_pairing_successful_cleanup_resumes_after_normal_operation_cancellation() {
        let executor = BackgroundExecutor::new(Arc::new(gpui_kit::TestDispatcher::new(0)));
        let old = Transport::new(executor);
        let owner = Owner {
            product_id: 164,
            container: "physical-owner".into(),
        };
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        let outcome = Arc::new(Mutex::new(None));
        let result = outcome.clone();
        let (ready, admitted) = mpsc::channel();
        let worker = thread::spawn(move || {
            ready.send(()).unwrap();
            while !signal.load(Ordering::Acquire) {
                thread::yield_now();
            }
            *result.lock().unwrap() = Some(Err("operation canceled".into()));
        });
        old.jobs.lock().unwrap().retired.push(Job {
            owner,
            canceled,
            outcome,
            worker,
        });
        admitted.recv_timeout(Duration::from_secs(1)).unwrap();
        old.stop_admission();
        assert!(old.reconnects.scope("late", 164, "physical-owner").is_err());
        assert!(
            old.replacement().is_err(),
            "an unfinished owner cannot be replaced"
        );
        old.reconnects.shutdown();
        old.shutdown();
        assert!(old.jobs.lock().unwrap().retired.is_empty());
        let completion = old.take_completions();
        assert_eq!(completion.len(), 1);
        assert_eq!(
            completion[0].diagnostic.as_deref(),
            Some("operation canceled")
        );
        assert!(!completion[0].cache_changed);
        assert!(completion[0].projection.is_none());

        assert!(old.cleanup_failure().is_none());
        let replacement = old.replacement().unwrap();
        assert!(
            replacement
                .reconnects
                .scope("resumed", 164, "physical-owner")
                .is_ok()
        );
        assert!(
            old.start(Value::Null, "DUALLINK_BIND_INFO", json!({}), false, true)
                .is_err()
        );
        assert!(
            old.reconnects
                .scope("resumed", 164, "physical-owner")
                .is_err()
        );
        assert!(!Arc::ptr_eq(&old.reconnects, &replacement.reconnects));
        old.shutdown();
        assert!(old.take_completions().is_empty());
        replacement.shutdown();
    }

    #[::core::prelude::v1::test]
    fn standalone_pairing_cleanup_failure_survives_delivery_and_blocks_replacement() {
        let executor = BackgroundExecutor::new(Arc::new(gpui_kit::TestDispatcher::new(0)));
        let old = Transport::new(executor);
        let container = "{11111111-2222-3333-4444-555555555555}";
        let owner = Owner {
            product_id: 164,
            container: container.into(),
        };
        let outcome = Arc::new(Mutex::new(None));
        let result = outcome.clone();
        let mut cleanup = old.cleanup_witness(164, container);
        let worker = thread::spawn(move || {
            cleanup.result = Err("service release failed".into());
            drop(cleanup);
            *result.lock().unwrap() = Some(Ok(()));
        });
        old.jobs.lock().unwrap().retired.push(Job {
            owner: owner.clone(),
            canceled: Arc::new(AtomicBool::new(false)),
            outcome,
            worker,
        });
        loop {
            if let Some(errors) = finish_product_jobs(&old.jobs, &owner).unwrap() {
                assert_eq!(errors, ["service release failed"]);
                break;
            }
            thread::yield_now();
        }
        assert!(
            old.take_completions().is_empty(),
            "cleanup failure is not an operation completion"
        );
        assert!(old.reconnects.scope("next-view", 164, container).is_err());
        assert!(
            old.start(
                json!({"productId":164,"deviceContainerId":container}),
                "DUALLINK_BIND_INFO",
                json!({}),
                false,
                true
            )
            .is_err()
        );
        old.reconnects.shutdown();
        old.shutdown();
        assert!(old.shutdown_complete.load(Ordering::Acquire));
        assert!(old.reconnects.shutdown_complete());
        assert!(
            old.replacement()
                .err()
                .unwrap()
                .to_string()
                .contains("service release failed")
        );
        assert!(old.take_completions().is_empty());
        old.shutdown();
        assert!(
            old.cleanup_failure()
                .unwrap()
                .contains("service release failed")
        );
        assert!(old.replacement().is_err());
    }
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

    #[::core::prelude::v1::test]
    fn hyperpolling_runtime_pairing_cleanup_joins_and_retains_failed_predecessor() {
        let owner = Owner::parse(
            &json!({"productId":179,"deviceContainerId":"{11111111-2222-3333-4444-555555555555}"}),
        )
        .unwrap();
        let (release, gate) = mpsc::channel();
        let canceled = Arc::new(AtomicBool::new(false));
        let outcome = Arc::new(Mutex::new(None));
        let completion = outcome.clone();
        let worker = thread::spawn(move || {
            gate.recv().unwrap();
            *completion.lock().unwrap() = Some(Err("old cleanup failed".into()));
        });
        let mut jobs = Jobs::default();
        jobs.retired.push(Job {
            owner: owner.clone(),
            canceled,
            outcome,
            worker,
        });
        let jobs = Mutex::new(jobs);
        assert!(finish_product_jobs(&jobs, &owner).unwrap().is_none());
        release.send(()).unwrap();
        let diagnostics = loop {
            if let Some(diagnostics) = finish_product_jobs(&jobs, &owner).unwrap() {
                break diagnostics;
            }
            thread::yield_now();
        };
        assert_eq!(diagnostics, ["old cleanup failed"]);
        assert!(jobs.lock().unwrap().retired.is_empty());
        // A later successful observation must not erase a previous cleanup
        // failure or turn it into an acknowledgement for the new operation.
        assert_eq!(
            finish_product_jobs(&jobs, &owner).unwrap().unwrap(),
            diagnostics
        );
    }

    #[::core::prelude::v1::test]
    fn hyperpolling_runtime_global_prune_preserves_each_owners_completion() {
        let owner = |product_id, container: &str| Owner {
            product_id,
            container: container.to_owned(),
        };
        let active_owner = owner(164, "{11111111-2222-3333-4444-555555555555}");
        let retired_owner = owner(241, "{22222222-2222-3333-4444-555555555555}");
        let pending_owner = owner(179, "{33333333-2222-3333-4444-555555555555}");
        let completed_job = |owner: Owner, error: &str| {
            let outcome = Arc::new(Mutex::new(None));
            let completion = outcome.clone();
            let error = error.to_owned();
            let worker = thread::spawn(move || {
                *completion.lock().unwrap() = Some(Err(error));
            });
            while !worker.is_finished() {
                thread::yield_now();
            }
            Job {
                owner,
                canceled: Arc::new(AtomicBool::new(false)),
                outcome,
                worker,
            }
        };
        let (release, gate) = mpsc::channel();
        let pending_outcome = Arc::new(Mutex::new(None));
        let completion = pending_outcome.clone();
        let pending = Job {
            owner: pending_owner.clone(),
            canceled: Arc::new(AtomicBool::new(false)),
            outcome: pending_outcome,
            worker: thread::spawn(move || {
                gate.recv().unwrap();
                *completion.lock().unwrap() = Some(Ok(()));
            }),
        };
        let mut jobs = Jobs::default();
        jobs.active.insert(
            (active_owner.clone(), "MOUSE".to_owned()),
            completed_job(active_owner.clone(), "active cleanup failed"),
        );
        jobs.retired.push(completed_job(
            retired_owner.clone(),
            "retired cleanup failed",
        ));
        jobs.retired.push(pending);

        prune_finished_jobs(&mut jobs);

        assert!(jobs.active.is_empty());
        assert_eq!(jobs.retired.len(), 1);
        assert_eq!(jobs.retired[0].owner, pending_owner);
        assert!(!jobs.retired[0].canceled.load(Ordering::Acquire));
        let jobs = Mutex::new(jobs);
        assert_eq!(
            finish_product_jobs(&jobs, &active_owner).unwrap().unwrap(),
            ["active cleanup failed"]
        );
        assert_eq!(
            finish_product_jobs(&jobs, &retired_owner).unwrap().unwrap(),
            ["retired cleanup failed"]
        );
        assert!(
            finish_product_jobs(&jobs, &pending_owner)
                .unwrap()
                .is_none()
        );
        release.send(()).unwrap();
        loop {
            if let Some(diagnostics) = finish_product_jobs(&jobs, &pending_owner).unwrap() {
                assert!(diagnostics.is_empty());
                break;
            }
            thread::yield_now();
        }
    }
}
