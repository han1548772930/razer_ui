//! Unpair side effects retain their own physical/view lifetime. The source
//! DUALLINK_CANCEL cancels scan/pair commands, never these scheduled retries.
use super::*;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    view: String,
    product_id: u32,
    container: String,
}

#[derive(Clone)]
pub(super) struct Scope {
    key: Key,
    valid: Arc<AtomicBool>,
}

struct Job {
    scope: Scope,
    alive: Arc<AtomicBool>,
    worker: JoinHandle<()>,
}

struct Worker {
    scope: Scope,
    alive: Arc<AtomicBool>,
    category: String,
    source_product_id: u16,
    next_transaction_sequence: u8,
    scheduled: Instant,
    operation_id: String,
    reports: Arc<Mutex<Vec<Report>>>,
    failures: Arc<CleanupFailures>,
}

#[derive(Default)]
struct State {
    scopes: BTreeMap<Key, Arc<AtomicBool>>,
    jobs: Vec<Job>,
}

pub(super) struct Report {
    pub(super) operation_id: String,
    pub(super) product_id: u32,
    pub(super) container: String,
    pub(super) state: Value,
}

#[derive(Default)]
pub(in crate::shell) struct Owners {
    state: Mutex<State>,
    reports: Arc<Mutex<Vec<Report>>>,
    closed: AtomicBool,
    shutdown_complete: AtomicBool,
    failures: Arc<CleanupFailures>,
}

impl Owners {
    pub(super) fn with_failures(failures: Arc<CleanupFailures>) -> Self {
        Self {
            state: Mutex::new(State::default()),
            reports: Arc::new(Mutex::new(Vec::new())),
            closed: AtomicBool::new(false),
            shutdown_complete: AtomicBool::new(false),
            failures,
        }
    }
    pub(super) fn shutdown_complete(&self) -> bool {
        self.shutdown_complete.load(Ordering::Acquire)
    }
    pub(super) fn scope(
        &self,
        view: &str,
        product_id: u32,
        container: &str,
    ) -> anyhow::Result<Scope> {
        ensure!(!self.closed.load(Ordering::Acquire), "独立重连所有者已关闭");
        let errors = self.failures.for_owner(product_id, container);
        ensure!(
            errors.is_empty(),
            "重连前次资源清理失败：{}",
            errors.join("; ")
        );
        let key = Key {
            view: view.into(),
            product_id,
            container: container.to_ascii_lowercase(),
        };
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("重连所有者状态锁不可用"))?;
        ensure!(!self.closed.load(Ordering::Acquire), "独立重连所有者已关闭");
        let errors = self.failures.for_owner(product_id, container);
        ensure!(
            errors.is_empty(),
            "重连前次资源清理失败：{}",
            errors.join("; ")
        );
        let valid = state
            .scopes
            .entry(key.clone())
            .or_insert_with(|| Arc::new(AtomicBool::new(true)))
            .clone();
        Ok(Scope { key, valid })
    }

    pub(super) fn start(
        &self,
        scope: &Scope,
        category: &str,
        source_product_id: Option<u16>,
        next_transaction_sequence: u8,
    ) -> anyhow::Result<()> {
        self.start_with(
            scope,
            category,
            source_product_id,
            next_transaction_sequence,
            run,
        )
    }

    fn start_with(
        &self,
        scope: &Scope,
        category: &str,
        source_product_id: Option<u16>,
        next_transaction_sequence: u8,
        execute: impl FnOnce(&Worker) -> anyhow::Result<()> + Send + 'static,
    ) -> anyhow::Result<()> {
        ensure!(
            !self.closed.load(Ordering::Acquire) && scope.valid.load(Ordering::Acquire),
            "页面或物理重连所有者已过期"
        );
        let physical_product = u16::try_from(scope.key.product_id)?;
        let factory = razer_device::receiver_pairing::for_source(
            physical_product,
            category,
            source_product_id,
        )?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("重连所有者状态锁不可用"))?;
        ensure!(
            !self.closed.load(Ordering::Acquire) && scope.valid.load(Ordering::Acquire),
            "重连所有者已退出，未启动"
        );
        let errors = self
            .failures
            .for_owner(scope.key.product_id, &scope.key.container);
        ensure!(
            errors.is_empty(),
            "重连前次资源清理失败：{}",
            errors.join("; ")
        );
        let mut pending = Vec::new();
        for job in state.jobs.drain(..) {
            if job.worker.is_finished() {
                join_job(job, &self.failures);
            } else {
                pending.push(job);
            }
        }
        state.jobs = pending;
        let errors = self
            .failures
            .for_owner(scope.key.product_id, &scope.key.container);
        ensure!(
            errors.is_empty(),
            "重连前次资源清理失败：{}",
            errors.join("; ")
        );
        let alive = Arc::new(AtomicBool::new(true));
        let scope = scope.clone();
        // Current241 f2(category,true) constructs a fresh factory on every
        // Unpair. Its timer retains that instance, including repeated calls
        // with the same category. A later Unpair does not dispose earlier jobs.
        let work = Worker {
            scope: scope.clone(),
            alive: alive.clone(),
            category: factory.category,
            source_product_id: factory.transport.source_product_id,
            next_transaction_sequence,
            scheduled: Instant::now(),
            operation_id: format!("unpair-reconnect-{}", uuid::Uuid::new_v4()),
            reports: self.reports.clone(),
            failures: self.failures.clone(),
        };
        let worker = thread::Builder::new().name("receiver-reconnect-client".into()).spawn(move || {
            let result = execute(&work);
            if let Err(error) = result {
                if let Ok(mut reports) = work.reports.lock() {
                    reports.push(Report {
                        operation_id: work.operation_id,
                        product_id: work.scope.key.product_id,
                        container: work.scope.key.container.clone(),
                        state: json!({"state":if work.scope.valid.load(Ordering::Acquire)&&work.alive.load(Ordering::Acquire){"failed"}else{"disposed"},"error":format!("{error:#}")}),
                    });
                }
            }
        })?;
        state.jobs.push(Job {
            scope,
            alive,
            worker,
        });
        Ok(())
    }

    /// Called by the real close-tab path, including retained workspaces.
    pub(in crate::shell) fn close_view(&self, view: &str) {
        {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.scopes.retain(|key, valid| {
                if key.view == view {
                    valid.store(false, Ordering::Release);
                    false
                } else {
                    true
                }
            });
            for job in &state.jobs {
                if job.scope.key.view == view {
                    job.alive.store(false, Ordering::Release);
                }
            }
        }
    }

    /// A slave binding refresh is not a physical master disconnect.
    pub(in crate::shell) fn retain_physical(&self, observed: impl Fn(u32, &str) -> bool) {
        {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.scopes.retain(|key, valid| {
                if observed(key.product_id, &key.container) {
                    true
                } else {
                    valid.store(false, Ordering::Release);
                    false
                }
            });
            for job in &state.jobs {
                if !job.scope.valid.load(Ordering::Acquire) {
                    job.alive.store(false, Ordering::Release);
                }
            }
        }
    }

    pub(super) fn take_reports(&self) -> Vec<Report> {
        // The app-wide delivery owner keeps running after a retained tab hides.
        // Join finished workers there without blocking the UI on native I/O.
        if let Ok(mut state) = self.state.lock() {
            let mut pending = Vec::new();
            for job in state.jobs.drain(..) {
                if job.worker.is_finished() {
                    join_job(job, &self.failures);
                } else {
                    pending.push(job);
                }
            }
            state.jobs = pending;
        }
        self.reports
            .lock()
            .map(|mut reports| std::mem::take(&mut *reports))
            .unwrap_or_default()
    }

    pub(super) fn stop_admission(&self) {
        self.closed.store(true, Ordering::Release);
    }

    pub(super) fn shutdown(&self) {
        self.stop_admission();
        {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for valid in state.scopes.values() {
                valid.store(false, Ordering::Release);
            }
            state.scopes.clear();
            let jobs = std::mem::take(&mut state.jobs);
            for job in &jobs {
                job.alive.store(false, Ordering::Release);
            }
            drop(state);
            for job in jobs {
                join_job(job, &self.failures);
            }
        }
        self.shutdown_complete.store(true, Ordering::Release);
    }
}

fn join_job(job: Job, failures: &CleanupFailures) {
    if job.worker.join().is_err() {
        failures.record(
            job.scope.key.product_id,
            &job.scope.key.container,
            "receiver reconnect worker panicked before cleanup completed".into(),
        );
    }
}

impl Drop for Owners {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn run(worker: &Worker) -> anyhow::Result<()> {
    let scope = &worker.scope;
    let id = worker.operation_id.as_str();
    let reports = &worker.reports;
    let valid = || scope.valid.load(Ordering::Acquire) && worker.alive.load(Ordering::Acquire);
    ensure!(valid(), "重连所有者已退出，未创建服务");
    let mut cleanup = CleanupWitness::new(
        worker.failures.clone(),
        scope.key.product_id,
        &scope.key.container,
    );
    let mut client = ServiceClient::spawn()?;
    cleanup.result = Err("receiver reconnect service cleanup receipt is missing".into());
    let started = (|| {
        ensure!(valid(), "重连所有者已退出，未启动");
        let receipt = client.request(ServiceRequest::ReceiverReconnectStart {
            operation_id: id.to_owned(),
            product_id: u16::try_from(scope.key.product_id)?,
            device_container_id: scope.key.container.clone(),
            category: worker.category.clone(),
            next_transaction_sequence: worker.next_transaction_sequence,
            remaining_delay_ms: Duration::from_millis(500)
                .saturating_sub(worker.scheduled.elapsed())
                .as_millis() as u64,
            source_product_id: Some(worker.source_product_id),
        })?;
        ensure!(receipt["operation_id"] == id, "重连启动响应身份不匹配");
        let mut last_state = Value::Null;
        loop {
            if !valid() {
                client.request(ServiceRequest::ReceiverReconnectCancel {
                    operation_id: id.to_owned(),
                })?;
                break;
            }
            let update = client.request(ServiceRequest::ReceiverReconnectPoll {
                operation_id: id.to_owned(),
            })?;
            ensure!(update["operation_id"] == id, "重连状态响应身份不匹配");
            let next = &update["reconnect"];
            ensure!(next.is_object(), "重连状态缺少源操作结果");
            if next != &last_state {
                last_state = next.clone();
                if let Ok(mut reports) = reports.lock() {
                    reports.push(Report {
                        operation_id: id.to_owned(),
                        product_id: scope.key.product_id,
                        container: scope.key.container.clone(),
                        state: next.clone(),
                    });
                }
            }
            if matches!(next["state"].as_str(), Some("failed" | "canceled"))
                || (next["state"] == "completed" && next["feature_handle_retained"] != true)
            {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
        Ok(())
    })();
    let shutdown = finish_pairing_service(&mut client, &mut cleanup.result);
    match (started, shutdown) {
        (Ok(()), Ok(_)) => {
            if let Ok(mut reports) = reports.lock() {
                reports.push(Report { operation_id: id.to_owned(), product_id: scope.key.product_id, container: scope.key.container.clone(), state: json!({"state":"disposed","service_cleanup_complete":true,"feature_handle_retained":false}) });
            }
            Ok(())
        }
        (Err(error), _) => Err(error),
        (_, Err(error)) => Err(error.context("独立重连服务清理未确认")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn receiver_reconnect_cleanup_failure_survives_reports_and_blocks_physical_successor() {
        let owners = Owners::default();
        let scope = owners.scope("original", 164, "physical-owner").unwrap();
        owners
            .start_with(&scope, "MOUSE", None, 1, |work| {
                let mut cleanup = CleanupWitness::new(
                    work.failures.clone(),
                    work.scope.key.product_id,
                    &work.scope.key.container,
                );
                cleanup.result = Err("reconnect service release failed".into());
                Ok(())
            })
            .unwrap();
        loop {
            let reports = owners.take_reports();
            assert!(
                reports.is_empty(),
                "cleanup failure is not an operation failure report"
            );
            if owners.state.lock().unwrap().jobs.is_empty() {
                break;
            }
            thread::yield_now();
        }
        assert!(owners.scope("replacement", 164, "PHYSICAL-OWNER").is_err());
        assert!(
            owners
                .start_with(&scope, "MOUSE", None, 2, |_| Ok(()))
                .is_err()
        );
        assert!(
            owners
                .scope("unrelated", 164, "other-physical-owner")
                .is_ok()
        );
        owners.shutdown();
        assert!(owners.shutdown_complete());
        assert!(
            owners
                .failures
                .summary()
                .unwrap()
                .contains("reconnect service release failed")
        );
        assert!(owners.take_reports().is_empty());
        assert!(owners.failures.summary().is_some());
    }
    #[::core::prelude::v1::test]
    fn view_close_and_physical_disconnect_expire_pending_reconnect_scopes_only() {
        let owners = Owners::default();
        let first = owners.scope("device-one", 164, "owner-one").unwrap();
        let peer = owners.scope("device-two", 241, "owner-two").unwrap();
        owners.close_view("device-one");
        assert!(!first.valid.load(Ordering::Acquire));
        assert!(peer.valid.load(Ordering::Acquire));
        let reopened = owners.scope("device-one", 164, "OWNER-ONE").unwrap();
        assert!(reopened.valid.load(Ordering::Acquire));
        owners.retain_physical(|product, container| product == 241 && container == "owner-two");
        assert!(!reopened.valid.load(Ordering::Acquire));
        assert!(peer.valid.load(Ordering::Acquire));
        owners.shutdown();
        assert!(!peer.valid.load(Ordering::Acquire));
        assert!(owners.scope("late-view", 164, "owner-one").is_err());
    }

    #[::core::prelude::v1::test]
    fn receiver_reconnect_tab_device_and_app_cleanup_release_owned_simulated_workers() {
        struct SimulatedHandle(Arc<AtomicBool>);
        impl Drop for SimulatedHandle {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }
        for lifecycle in ["tab", "device", "app"] {
            let owners = Owners::default();
            let scope = owners
                .scope("retained-view", 164, "physical-owner")
                .unwrap();
            let valid = scope.valid.clone();
            let alive = Arc::new(AtomicBool::new(true));
            let signal = alive.clone();
            let released = Arc::new(AtomicBool::new(false));
            let release = released.clone();
            let (ready_sender, ready) = mpsc::channel();
            let worker = thread::spawn(move || {
                let _handle = SimulatedHandle(release);
                ready_sender.send(()).unwrap();
                while valid.load(Ordering::Acquire) && signal.load(Ordering::Acquire) {
                    thread::yield_now();
                }
            });
            owners.state.lock().unwrap().jobs.push(Job {
                scope,
                alive,
                worker,
            });
            ready.recv_timeout(Duration::from_secs(1)).unwrap();
            assert!(!released.load(Ordering::Acquire));
            match lifecycle {
                "tab" => owners.close_view("retained-view"),
                "device" => owners.retain_physical(|_, _| false),
                "app" => owners.shutdown(),
                _ => unreachable!(),
            }
            let until = Instant::now() + Duration::from_secs(1);
            while !owners.state.lock().unwrap().jobs.is_empty() && Instant::now() < until {
                owners.take_reports();
                thread::yield_now();
            }
            assert!(
                owners.state.lock().unwrap().jobs.is_empty(),
                "{lifecycle} cleanup retained a worker"
            );
            assert!(
                released.load(Ordering::Acquire),
                "{lifecycle} cleanup retained a simulated handle"
            );
        }
    }

    #[::core::prelude::v1::test]
    fn unpair_factory_instances_coexist_until_tab_device_or_app_disposal() {
        struct SimulatedHandle(Arc<std::sync::atomic::AtomicUsize>);
        impl Drop for SimulatedHandle {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::Release);
            }
        }

        for lifecycle in ["tab", "device", "app"] {
            let owners = Owners::default();
            let scope = owners
                .scope("dual-receiver", 241, "physical-owner")
                .unwrap();
            let released = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let (ready_sender, ready) = mpsc::channel();
            for (category, caller, sequence) in [
                ("MOUSE", None, 1),
                ("KEYBOARD", None, 2),
                ("MOUSE", Some(241), 3),
                ("MOUSE", Some(756), 4),
            ] {
                let ready_sender = ready_sender.clone();
                let released = released.clone();
                owners
                    .start_with(&scope, category, caller, sequence, move |work| {
                        let _handle = SimulatedHandle(released);
                        ready_sender.send((
                            work.operation_id.clone(),
                            work.category.clone(),
                            work.source_product_id,
                            work.next_transaction_sequence,
                        ))?;
                        while work.scope.valid.load(Ordering::Acquire)
                            && work.alive.load(Ordering::Acquire)
                        {
                            thread::yield_now();
                        }
                        Ok(())
                    })
                    .unwrap();
            }
            let observed: Vec<_> = (0..4)
                .map(|_| ready.recv_timeout(Duration::from_secs(1)).unwrap())
                .collect();
            let ids: std::collections::BTreeSet<_> =
                observed.iter().map(|(id, _, _, _)| id).collect();
            assert_eq!(
                ids.len(),
                4,
                "each Unpair retains a distinct factory instance"
            );
            let factories: BTreeMap<_, _> = observed
                .into_iter()
                .map(|(_, category, caller, sequence)| (sequence, (category, caller)))
                .collect();
            assert_eq!(
                factories,
                BTreeMap::from([
                    (1, ("MOUSE".into(), 241)),
                    (2, ("KEYBOARD".into(), 241)),
                    (3, ("MOUSE".into(), 241)),
                    (4, ("MOUSE".into(), 756)),
                ]),
                "category, resolved caller and transaction sequence belong to each instance"
            );
            {
                let state = owners.state.lock().unwrap();
                assert_eq!(state.jobs.len(), 4);
                assert!(state.jobs.iter().all(|job| {
                    job.alive.load(Ordering::Acquire) && job.scope.valid.load(Ordering::Acquire)
                }));
            }
            assert_eq!(released.load(Ordering::Acquire), 0);

            match lifecycle {
                "tab" => owners.close_view("dual-receiver"),
                "device" => owners.retain_physical(|_, _| false),
                "app" => owners.shutdown(),
                _ => unreachable!(),
            }
            let until = Instant::now() + Duration::from_secs(1);
            while !owners.state.lock().unwrap().jobs.is_empty() && Instant::now() < until {
                owners.take_reports();
                thread::yield_now();
            }
            assert!(
                owners.state.lock().unwrap().jobs.is_empty(),
                "{lifecycle} cleanup retained an independent factory worker"
            );
            assert_eq!(
                released.load(Ordering::Acquire),
                4,
                "{lifecycle} cleanup did not release all simulated factory handles"
            );
        }
    }
}
