//! Exact notifications establish epochs before any collection can be opened.
use super::{BROKER_STOPPING, TOPOLOGY_FAILURE, TOPOLOGY_READY, collection::Pool};
use anyhow::Context as _;
use razer_device::backend::{HidBackend, HidNode};
use razer_platform::device_changes::{
    DeviceChangeMonitor, DeviceInterfaceAction, ExactDeviceChange, ExactDeviceChangeGate,
};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

const RECONCILE_INTERVAL: Duration = Duration::from_secs(2);
const RETRY_POLL: Duration = Duration::from_millis(50);

struct RetrySchedule {
    next_attempt: Duration,
}

impl RetrySchedule {
    fn after_failure(now: Duration) -> Self {
        Self {
            next_attempt: now.saturating_add(RECONCILE_INTERVAL),
        }
    }

    fn remaining(&self, now: Duration) -> Option<Duration> {
        self.next_attempt
            .checked_sub(now)
            .filter(|remaining| !remaining.is_zero())
    }
}

#[derive(Default)]
struct CleanupHistory {
    first_failure: Option<String>,
    failed_attempts: u64,
}

impl CleanupHistory {
    fn record(&mut self, error: anyhow::Error) {
        self.failed_attempts = self.failed_attempts.saturating_add(1);
        if self.first_failure.is_none() {
            self.first_failure = Some(format!("{error:#}"));
        }
    }

    fn finish(self) -> anyhow::Result<()> {
        match self.first_failure {
            None => Ok(()),
            Some(error) => Err(anyhow::anyhow!(
                "{error}; {} cleanup attempts reported failures; all notification workers reached terminal join",
                self.failed_attempts
            )),
        }
    }
}

struct JoinedMonitor<F: FnOnce() -> Result<(), String>> {
    shutdown: Option<F>,
}

impl<F: FnOnce() -> Result<(), String>> JoinedMonitor<F> {
    fn new(shutdown: F) -> Self {
        Self {
            shutdown: Some(shutdown),
        }
    }

    fn finish(&mut self) -> Result<(), String> {
        self.shutdown.take().map_or(Ok(()), |shutdown| shutdown())
    }
}

impl<F: FnOnce() -> Result<(), String>> Drop for JoinedMonitor<F> {
    fn drop(&mut self) {
        // This guard belongs only to the headless topology worker. Unwind
        // cannot detach a notification worker which still owns callbacks.
        if let Err(error) = self.finish() {
            set_failure(Some(format!(
                "Native notification cleanup during unwind: {error}"
            )));
        }
    }
}

struct TopologyTerminal(Pool);

impl Drop for TopologyTerminal {
    fn drop(&mut self) {
        TOPOLOGY_READY.store(false, Ordering::Release);
        if let Err(error) = self.0.invalidate_all() {
            set_failure(Some(format!(
                "Terminal HID topology invalidation failed: {error:#}"
            )));
        }
    }
}

fn set_failure(failure: Option<String>) {
    match TOPOLOGY_FAILURE.lock() {
        Ok(mut current) => *current = failure,
        Err(poisoned) => *poisoned.into_inner() = failure,
    }
}

fn report_failure(
    error: &anyhow::Error,
    started: &mpsc::SyncSender<Result<(), String>>,
    notified: &mut bool,
) {
    TOPOLOGY_READY.store(false, Ordering::Release);
    let error = format!("{error:#}");
    set_failure(Some(error.clone()));
    if !*notified {
        let _ = started.try_send(Err(error));
        *notified = true;
    }
}

struct ReconcileSchedule {
    next_due: Duration,
}

impl ReconcileSchedule {
    fn new(now: Duration) -> Self {
        Self {
            next_due: now.saturating_add(RECONCILE_INTERVAL),
        }
    }

    fn due(&self, now: Duration) -> bool {
        now >= self.next_due
    }

    fn completed(&mut self, now: Duration) {
        self.next_due = now.saturating_add(RECONCILE_INTERVAL);
    }
}

pub(super) struct Topology {
    stopped: Arc<AtomicBool>,
    worker: Option<JoinHandle<anyhow::Result<()>>>,
}

impl Topology {
    pub(super) fn start(pool: Pool) -> anyhow::Result<Self> {
        let stopped = Arc::new(AtomicBool::new(false));
        let signal = stopped.clone();
        let (started, startup) = mpsc::sync_channel(1);
        let worker = std::thread::Builder::new()
            .name("hid-broker-topology".into())
            .spawn(move || {
                let _terminal = TopologyTerminal(pool.clone());
                let result = observe(pool.clone(), signal, &started);
                TOPOLOGY_READY.store(false, Ordering::Release);
                let invalidated = pool.invalidate_all();
                if let Err(error) = &result {
                    let _ = started.try_send(Err(format!("{error:#}")));
                }
                match (result, invalidated) {
                    (Ok(()), Ok(())) => Ok(()),
                    (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
                    (Err(error), Err(cleanup)) => {
                        Err(error.context(format!("HID invalidation also failed: {cleanup:#}")))
                    }
                }
            })?;
        let owner = Self {
            stopped,
            worker: Some(worker),
        };
        match startup
            .recv()
            .context("HID topology worker exited before initialization")?
        {
            Ok(()) => Ok(owner),
            // A failed first attempt leaves the supervisor owned and retrying.
            // Non-HID local services can start while exact epochs are closed.
            Err(_) => Ok(owner),
        }
    }

    fn finish(&mut self) -> anyhow::Result<()> {
        self.stopped.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| anyhow::anyhow!("HID topology worker panicked"))??;
        }
        Ok(())
    }

    pub(super) fn shutdown(mut self) -> anyhow::Result<()> {
        self.finish()
    }
}

impl Drop for Topology {
    fn drop(&mut self) {
        // This owner only exists in the headless service, never on a UI thread.
        if let Err(error) = self.finish() {
            eprintln!("HID topology cleanup: {error:#}");
        }
    }
}

fn scan() -> anyhow::Result<HashMap<Vec<u8>, HidNode>> {
    let mut native = razer_hid::NativeBackend::new()?;
    let mut nodes = HashMap::new();
    for node in native.enumerate()? {
        if !matches!(node.vendor_id, 0x1532 | 0x068e) {
            continue;
        }
        let key = path_key(&node.path);
        anyhow::ensure!(
            nodes.insert(key, node).is_none(),
            "HID scan contains duplicate collection paths"
        );
    }
    Ok(nodes)
}

fn path_key(path: &[u8]) -> Vec<u8> {
    #[cfg(windows)]
    {
        path.iter().map(u8::to_ascii_lowercase).collect()
    }
    #[cfg(not(windows))]
    {
        path.to_vec()
    }
}

fn refresh(pool: &Pool, previous: &mut HashMap<Vec<u8>, HidNode>) -> anyhow::Result<()> {
    let current = scan()?;
    apply_inventory(pool, previous, current)
}

fn apply_inventory(
    pool: &Pool,
    previous: &mut HashMap<Vec<u8>, HidNode>,
    current: HashMap<Vec<u8>, HidNode>,
) -> anyhow::Result<()> {
    for (path, old) in previous.iter() {
        if current.get(path) != Some(old) {
            pool.invalidate_path(&old.path)?;
        }
    }
    *previous = current;
    pool.reconcile()?;
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
enum SnapshotDisposition {
    Stale,
    Unchanged,
    Changed,
}

fn snapshot_disposition<T: PartialEq>(
    captured_revision: u64,
    current_revision: Option<u64>,
    before: &T,
    after: &T,
) -> SnapshotDisposition {
    if current_revision != Some(captured_revision) {
        SnapshotDisposition::Stale
    } else if before == after {
        SnapshotDisposition::Unchanged
    } else {
        SnapshotDisposition::Changed
    }
}

fn observe(
    pool: Pool,
    stopped: Arc<AtomicBool>,
    started: &mpsc::SyncSender<Result<(), String>>,
) -> anyhow::Result<()> {
    let mut notified = false;
    let mut history = CleanupHistory::default();
    while !stopped.load(Ordering::Acquire) && !BROKER_STOPPING.load(Ordering::Acquire) {
        TOPOLOGY_READY.store(false, Ordering::Release);
        let result = match DeviceChangeMonitor::start_exact() {
            Err(error) => {
                let error = anyhow::Error::msg(error);
                report_failure(&error, started, &mut notified);
                if let Err(cleanup) = pool.invalidate_all() {
                    history.record(cleanup);
                }
                Err(error)
            }
            Ok((monitor, events)) => {
                let gate = monitor.exact_gate();
                let mut retained = JoinedMonitor::new(move || monitor.shutdown());
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let gate = gate.context("Exact HID monitor omitted its admission gate")?;
                    pool.set_topology_gate(gate.clone())?;
                    observe_stream(&pool, &events, &gate, &stopped, started, &mut notified)
                }))
                .unwrap_or_else(|_| Err(anyhow::anyhow!("HID topology consumer panicked")));
                TOPOLOGY_READY.store(false, Ordering::Release);
                if let Err(error) = &result {
                    report_failure(error, started, &mut notified);
                }
                if let Err(cleanup) = pool.invalidate_all() {
                    history.record(cleanup);
                }
                // Normal, failed and panicked consumers all wait for the same
                // worker's actual unregister completion before a successor.
                if let Err(cleanup) = retained.finish() {
                    let cleanup = anyhow::Error::msg(cleanup);
                    report_failure(&cleanup, started, &mut notified);
                    history.record(cleanup);
                }
                result
            }
        };
        match result {
            Ok(()) => break,
            Err(error) => {
                eprintln!("HID topology lost; rebuilding exact observation: {error:#}");
            }
        }
        if !wait_before_retry(&stopped) {
            break;
        }
    }
    history.finish()
}

fn wait_before_retry(stopped: &AtomicBool) -> bool {
    let origin = Instant::now();
    let schedule = RetrySchedule::after_failure(Duration::ZERO);
    while let Some(remaining) = schedule.remaining(origin.elapsed()) {
        if stopped.load(Ordering::Acquire) || BROKER_STOPPING.load(Ordering::Acquire) {
            return false;
        }
        std::thread::park_timeout(remaining.min(RETRY_POLL));
    }
    !stopped.load(Ordering::Acquire) && !BROKER_STOPPING.load(Ordering::Acquire)
}

fn observe_stream(
    pool: &Pool,
    events: &async_channel::Receiver<ExactDeviceChange>,
    gate: &ExactDeviceChangeGate,
    stopped: &AtomicBool,
    started: &mpsc::SyncSender<Result<(), String>>,
    announced: &mut bool,
) -> anyhow::Result<()> {
    let mut sequence = 0u64;
    let mut ready = false;
    let mut needs_resume = false;
    let mut inventory = HashMap::new();
    let origin = Instant::now();
    let mut reconciliation = ReconcileSchedule::new(Duration::ZERO);
    while !stopped.load(Ordering::Acquire) && !BROKER_STOPPING.load(Ordering::Acquire) {
        let event = match events.try_recv() {
            Ok(event) => event,
            Err(async_channel::TryRecvError::Empty) => {
                if ready {
                    if needs_resume {
                        // Publication and this final consumed-prefix check use
                        // one lock. A callback arriving after Empty invalidates
                        // admission before exposing its next exact event.
                        let resumed = gate
                            .try_resume(sequence, || {
                                pool.resume_after_reconcile()
                                    .map_err(|error| format!("{error:#}"))
                            })
                            .map_err(anyhow::Error::msg)?;
                        if !resumed {
                            continue;
                        }
                        needs_resume = false;
                    }
                    if reconciliation.due(origin.elapsed()) {
                        let Some(captured_revision) = gate.current_revision() else {
                            continue;
                        };
                        // Notification delivery improves latency; a bounded
                        // scan independently checks currently observable IDs.
                        // A snapshot cannot prove a same-identity reinsert.
                        let current = match scan() {
                            Ok(current) => current,
                            Err(error) => {
                                gate.suspend_for_reconcile(sequence);
                                return Err(
                                    error.context("Bounded HID identity reconciliation failed")
                                );
                            }
                        };
                        match snapshot_disposition(
                            captured_revision,
                            gate.current_revision(),
                            &inventory,
                            &current,
                        ) {
                            SnapshotDisposition::Stale => continue,
                            SnapshotDisposition::Unchanged => {
                                pool.reconcile()?;
                            }
                            SnapshotDisposition::Changed => {
                                apply_inventory(pool, &mut inventory, current)?;
                            }
                        }
                        reconciliation.completed(origin.elapsed());
                        continue;
                    }
                    TOPOLOGY_READY.store(gate.current_revision().is_some(), Ordering::Release);
                    if gate.current_revision().is_some() {
                        set_failure(None);
                    }
                    if !*announced {
                        started
                            .send(Ok(()))
                            .context("HID startup owner disappeared")?;
                        *announced = true;
                    }
                }
                // Notification delivery stays independently owned during this
                // bounded wait, including invalidation while native I/O blocks.
                std::thread::park_timeout(std::time::Duration::from_millis(10));
                continue;
            }
            Err(async_channel::TryRecvError::Closed) => {
                anyhow::bail!("Exact HID notification stream closed")
            }
        };
        TOPOLOGY_READY.store(false, Ordering::Release);
        needs_resume = true;
        match event {
            ExactDeviceChange::Ready { sequence: observed } => {
                check_sequence(&mut sequence, observed)?;
                anyhow::ensure!(!ready, "Duplicate HID registration barrier");
                ready = true;
                // Interface notifications received during the scan remain in
                // the exact queue and are replayed before admission opens.
                refresh(pool, &mut inventory)?;
                reconciliation.completed(origin.elapsed());
            }
            ExactDeviceChange::Interface {
                sequence: observed,
                action,
                path_utf16,
            } => {
                check_sequence(&mut sequence, observed)?;
                let path = String::from_utf16(&path_utf16)
                    .context("Device interface path is not valid UTF-16")?;
                if path.to_ascii_lowercase().starts_with(r"\\?\hid#") {
                    // Arrival invalidates too: a remove+reinsert may reuse the
                    // identical path, descriptor, container and serial values.
                    pool.invalidate_path(path.as_bytes())?;
                    if action == DeviceInterfaceAction::Removal {
                        inventory.remove(&path_key(path.as_bytes()));
                    }
                }
                if ready {
                    refresh(pool, &mut inventory)?;
                    reconciliation.completed(origin.elapsed());
                }
            }
            ExactDeviceChange::Lost {
                after_sequence,
                reason,
            } => {
                anyhow::ensure!(
                    after_sequence == sequence,
                    "Lost HID notification sequence differs"
                );
                anyhow::bail!("Exact HID notifications lost: {reason:?}");
            }
            ExactDeviceChange::Failed(error) => {
                anyhow::bail!("Exact HID notification failure: {error}")
            }
        }
    }
    Ok(())
}

fn check_sequence(previous: &mut u64, observed: u64) -> anyhow::Result<()> {
    anyhow::ensure!(
        previous.checked_add(1) == Some(observed),
        "Exact HID notification sequence is discontinuous"
    );
    *previous = observed;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_retries_have_bounded_logical_backoff_after_each_failed_attempt() {
        let first = RetrySchedule::after_failure(Duration::from_secs(5));
        assert_eq!(
            first.remaining(Duration::from_secs(5)),
            Some(Duration::from_secs(2))
        );
        assert_eq!(
            first.remaining(Duration::from_millis(6_999)),
            Some(Duration::from_millis(1))
        );
        assert_eq!(first.remaining(Duration::from_secs(7)), None);
        let repeated_failure = RetrySchedule::after_failure(Duration::from_millis(7_300));
        assert_eq!(
            repeated_failure.remaining(Duration::from_millis(7_300)),
            Some(Duration::from_secs(2))
        );
        assert_eq!(
            repeated_failure.remaining(Duration::from_millis(9_300)),
            None
        );
    }

    #[test]
    fn headless_monitor_scope_waits_for_terminal_cleanup_during_consumer_unwind() {
        let (entered, cleanup_entered) = mpsc::channel();
        let (release, may_release) = mpsc::channel();
        let cleaned = Arc::new(AtomicBool::new(false));
        let cleanup_complete = cleaned.clone();
        let worker = std::thread::spawn(move || {
            std::panic::catch_unwind(move || {
                let _monitor = JoinedMonitor::new(move || {
                    entered.send(()).unwrap();
                    may_release.recv().unwrap();
                    cleanup_complete.store(true, Ordering::Release);
                    Ok(())
                });
                panic!("simulated topology consumer panic");
            })
        });
        let entered = cleanup_entered.recv_timeout(Duration::from_secs(5));
        let premature_terminal = worker.is_finished();
        let premature_release = cleaned.load(Ordering::Acquire);
        release.send(()).unwrap();
        let outcome = worker.join().unwrap();
        assert!(entered.is_ok());
        assert!(!premature_terminal);
        assert!(!premature_release);
        assert!(outcome.is_err());
        assert!(cleaned.load(Ordering::Acquire));
    }

    #[test]
    fn explicit_monitor_join_runs_once_and_preserves_terminal_cleanup_failure() {
        let calls = std::cell::Cell::new(0);
        let mut monitor = JoinedMonitor::new(|| {
            calls.set(calls.get() + 1);
            Err("simulated first unregister failed; all resources now released".into())
        });
        assert!(
            monitor
                .finish()
                .unwrap_err()
                .contains("all resources now released")
        );
        assert_eq!(monitor.finish(), Ok(()));
        drop(monitor);
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn cleanup_history_preserves_failures_after_recovered_monitors_reach_terminal_join() {
        let mut history = CleanupHistory::default();
        history.record(anyhow::anyhow!("first native unregister failure"));
        history.record(anyhow::anyhow!("later invalidation failure"));
        let error = history.finish().unwrap_err().to_string();
        assert!(error.contains("first native unregister failure"));
        assert!(error.contains("2 cleanup attempts reported failures"));
        assert!(error.contains("all notification workers reached terminal join"));
    }

    #[test]
    fn bounded_reconciliation_uses_logical_time_and_defers_after_each_completed_scan() {
        let mut schedule = ReconcileSchedule::new(Duration::from_secs(7));
        assert!(!schedule.due(Duration::from_secs(7)));
        assert!(!schedule.due(Duration::from_millis(8_999)));
        assert!(schedule.due(Duration::from_secs(9)));
        schedule.completed(Duration::from_millis(9_600));
        assert!(!schedule.due(Duration::from_millis(11_599)));
        assert!(schedule.due(Duration::from_millis(11_600)));
        assert!(schedule.due(Duration::from_secs(20)));
    }

    #[test]
    fn unchanged_periodic_snapshots_preserve_the_current_connection_and_stale_scans_are_discarded()
    {
        let inventory = ["collection-a", "collection-b"];
        assert_eq!(
            snapshot_disposition(7, Some(7), &inventory, &inventory),
            SnapshotDisposition::Unchanged
        );
        let replacement = ["collection-a", "replacement-b"];
        assert_eq!(
            snapshot_disposition(7, Some(7), &inventory, &replacement),
            SnapshotDisposition::Changed
        );
        for revision in [None, Some(8)] {
            assert_eq!(
                snapshot_disposition(7, revision, &inventory, &replacement),
                SnapshotDisposition::Stale
            );
            assert_eq!(
                snapshot_disposition(7, revision, &inventory, &inventory),
                SnapshotDisposition::Stale
            );
        }
    }
}
