use super::*;
use serde_json::json;
use std::{
    rc::Rc,
    sync::{
        OnceLock,
        atomic::{AtomicU64, AtomicUsize},
    },
};

type Inventory = BTreeMap<Vec<u8>, Arc<MockState>>;
static INVENTORY: OnceLock<Mutex<Inventory>> = OnceLock::new();

fn inventory() -> &'static Mutex<Inventory> {
    INVENTORY.get_or_init(Mutex::default)
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Observed {
    Send(Vec<u8>),
    Get,
    Output(Vec<u8>),
}

#[derive(Default)]
struct Gate {
    state: Mutex<(bool, bool)>,
    changed: Condvar,
}

impl Gate {
    fn enter(&self) {
        let mut state = self.state.lock().unwrap();
        state.0 = true;
        self.changed.notify_all();
        while !state.1 {
            state = self.changed.wait(state).unwrap();
        }
    }

    fn wait_entered(&self) {
        let state = self.state.lock().unwrap();
        let (state, _) = self
            .changed
            .wait_timeout_while(state, Duration::from_secs(3), |state| !state.0)
            .unwrap();
        assert!(state.0, "native test barrier was never entered");
    }

    fn release(&self) {
        self.state.lock().unwrap().1 = true;
        self.changed.notify_all();
    }
}

struct ReleaseGate(Arc<Gate>);
impl Drop for ReleaseGate {
    fn drop(&mut self) {
        self.0.release();
    }
}

struct CancelOnDrop(Arc<AtomicBool>);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

struct MockState {
    node: HidNode,
    opened: AtomicUsize,
    closed: AtomicUsize,
    observed: Mutex<Vec<Observed>>,
    opening: Mutex<Option<Arc<Gate>>>,
    sending: Mutex<Option<Arc<Gate>>>,
    resolving: Mutex<Option<Arc<Gate>>>,
    gates: Mutex<Vec<Arc<Gate>>>,
}

struct Fixture(Arc<MockState>);

impl Fixture {
    fn new(product_id: u16) -> Self {
        let node = HidNode {
            path: format!("mock-collection-{}", Uuid::new_v4()).into_bytes(),
            vendor_id: 5426,
            product_id,
            usage_page: 1,
            usage: 2,
            interface_number: 0,
            release_number: 1,
            serial_number: None,
            manufacturer: None,
            product: None,
        };
        let state = Arc::new(MockState {
            node,
            opened: AtomicUsize::new(0),
            closed: AtomicUsize::new(0),
            observed: Mutex::new(Vec::new()),
            opening: Mutex::new(None),
            sending: Mutex::new(None),
            resolving: Mutex::new(None),
            gates: Mutex::new(Vec::new()),
        });
        inventory()
            .lock()
            .unwrap()
            .insert(state.node.path.clone(), state.clone());
        Self(state)
    }

    fn gate(&self, target: &Mutex<Option<Arc<Gate>>>) -> Arc<Gate> {
        let gate = Arc::new(Gate::default());
        *target.lock().unwrap() = Some(gate.clone());
        self.0.gates.lock().unwrap().push(gate.clone());
        gate
    }

    fn observed(&self) -> Vec<Observed> {
        self.0.observed.lock().unwrap().clone()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for gate in self.0.gates.lock().unwrap().iter() {
            gate.release();
        }
        inventory().lock().unwrap().remove(&self.0.node.path);
    }
}

fn find(node: &HidNode) -> anyhow::Result<Arc<MockState>> {
    inventory()
        .lock()
        .unwrap()
        .get(&node.path)
        .cloned()
        .context("Test collection is not present")
}

fn identity(node: &HidNode) -> anyhow::Result<CollectionIdentity> {
    let state = find(node)?;
    let gate = state.resolving.lock().unwrap().take();
    if let Some(gate) = gate {
        gate.enter();
    }
    Ok(CollectionIdentity {
        node: node.clone(),
        platform: PlatformIdentity::Windows {
            container_id: "actual-test-container".into(),
            hid_instance_id: format!("instance-{}", String::from_utf8_lossy(&node.path)),
            parent_instance_id: "actual-test-parent".into(),
        },
    })
}

struct MockBackend(Rc<()>);

fn raw_factory() -> anyhow::Result<Box<dyn HidBackend>> {
    Ok(Box::new(MockBackend(Rc::new(()))))
}

impl HidBackend for MockBackend {
    fn enumerate(&mut self) -> anyhow::Result<Vec<HidNode>> {
        Ok(inventory()
            .lock()
            .unwrap()
            .values()
            .map(|state| state.node.clone())
            .collect())
    }

    fn open(&mut self, node: &HidNode) -> anyhow::Result<Box<dyn FeatureTransport>> {
        let state = find(node)?;
        let gate = state.opening.lock().unwrap().take();
        if let Some(gate) = gate {
            gate.enter();
        }
        state.opened.fetch_add(1, Ordering::AcqRel);
        Ok(Box::new(MockFeature {
            state,
            thread: thread::current().id(),
            _local: self.0.clone(),
        }))
    }

    fn open_interrupt(&mut self, node: &HidNode) -> anyhow::Result<Box<dyn InterruptTransport>> {
        let state = find(node)?;
        let gate = state.opening.lock().unwrap().take();
        if let Some(gate) = gate {
            gate.enter();
        }
        Ok(Box::new(MockInterrupt {
            _state: state,
            thread: thread::current().id(),
            _local: self.0.clone(),
        }))
    }
}

fn lengths() -> ReportLengths {
    ReportLengths {
        input: BTreeMap::from([(0, 4)]),
        output: BTreeMap::from([(0, 4)]),
        feature: BTreeMap::from([(0, 4)]),
    }
}

struct MockFeature {
    state: Arc<MockState>,
    thread: thread::ThreadId,
    _local: Rc<()>,
}

impl FeatureTransport for MockFeature {
    fn hid_node(&self) -> Option<&HidNode> {
        Some(&self.state.node)
    }

    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        assert_eq!(self.thread, thread::current().id());
        self.state
            .observed
            .lock()
            .unwrap()
            .push(Observed::Send(report.to_vec()));
        let gate = self.state.sending.lock().unwrap().take();
        if let Some(gate) = gate {
            gate.enter();
        }
        ensure!(
            report.first() != Some(&0xee),
            "scripted native Feature failure"
        );
        Ok(())
    }

    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        assert_eq!(self.thread, thread::current().id());
        self.state.observed.lock().unwrap().push(Observed::Get);
        report[..3].copy_from_slice(&[0x7f, 0x12, 0x34]);
        Ok(3)
    }

    fn write_output_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        assert_eq!(self.thread, thread::current().id());
        self.state
            .observed
            .lock()
            .unwrap()
            .push(Observed::Output(report.to_vec()));
        Ok(2)
    }

    fn report_lengths(&self) -> anyhow::Result<ReportLengths> {
        Ok(lengths())
    }

    fn metadata(&self) -> Value {
        json!({"actual_mock_handle": String::from_utf8_lossy(&self.state.node.path)})
    }
}

impl Drop for MockFeature {
    fn drop(&mut self) {
        assert_eq!(self.thread, thread::current().id());
        self.state.closed.fetch_add(1, Ordering::AcqRel);
    }
}

struct MockInterrupt {
    _state: Arc<MockState>,
    thread: thread::ThreadId,
    _local: Rc<()>,
}

impl InterruptTransport for MockInterrupt {
    fn read(&self, report: &mut [u8], _: u32) -> anyhow::Result<usize> {
        assert_eq!(self.thread, thread::current().id());
        report[..2].copy_from_slice(&[0x81, 0x53]);
        Ok(2)
    }

    fn report_lengths(&self) -> ReportLengths {
        lengths()
    }
}

fn paused_pool() -> Pool {
    let pool = Pool::new(raw_factory, identity);
    pool.install_topology_gate(TopologyGate::Scripted(Arc::new(AtomicU64::new(1))))
        .unwrap();
    pool
}

fn pool() -> Pool {
    let pool = paused_pool();
    pool.resume_after_reconcile().unwrap();
    pool
}

fn topology_revision(pool: &Pool) -> Arc<AtomicU64> {
    let state = pool.inner.state.lock().unwrap();
    match state.topology_gate.as_ref().unwrap() {
        TopologyGate::Scripted(revision) => revision.clone(),
        TopologyGate::Exact(_) => {
            panic!("collection tests must not start native topology observation")
        }
    }
}

fn open(pool: &Pool, node: &HidNode) -> Box<dyn FeatureTransport> {
    pool.backend_for(Uuid::new_v4(), Arc::new(AtomicBool::new(false)))
        .unwrap()
        .open(node)
        .unwrap()
}

fn wait_queued(pool: &Pool, node: &HidNode) {
    let entry = pool.entry(node).unwrap();
    let state = entry.state.lock().unwrap();
    let (state, _) = entry
        .changed
        .wait_timeout_while(state, Duration::from_secs(3), |state| {
            state.queued.is_empty()
        })
        .unwrap();
    assert!(
        !state.queued.is_empty(),
        "second exchange never entered FIFO"
    );
}

fn wait_command_queued(pool: &Pool) {
    let gates: Vec<_> = pool
        .inner
        .commands
        .0
        .lock()
        .unwrap()
        .values()
        .filter_map(Weak::upgrade)
        .collect();
    assert_eq!(
        gates.len(),
        1,
        "one source command must already hold admission"
    );
    let gate = &gates[0];
    let state = gate.state.lock().unwrap();
    let (state, _) = gate
        .changed
        .wait_timeout_while(state, Duration::from_secs(3), |state| {
            state.queued.is_empty()
        })
        .unwrap();
    assert!(
        !state.queued.is_empty(),
        "source command never entered FIFO"
    );
}

fn stop(pool: &Pool) {
    pool.begin_shutdown().unwrap();
    reap(pool);
}

fn reap(pool: &Pool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !pool.is_drained().unwrap() {
        pool.reconcile().unwrap();
        assert!(
            Instant::now() < deadline,
            "native test actors never terminated"
        );
        thread::yield_now();
    }
}

#[test]
fn batch_out_interval_is_checked_once_before_its_first_feature_send() {
    let mut progress = LeaseProgress::default();
    let mut clock = SourceClock::default();
    clock.native_return(Duration::from_millis(10), NativeReturn::Get);
    let interval = progress.out_interval(ExchangeTiming::Batch).unwrap();
    assert_eq!(
        clock.out_wait(Duration::from_millis(11), interval),
        Some(Duration::from_millis(2))
    );
    progress.dispatched(ExchangeTiming::Batch, true);
    progress.returned(ExchangeTiming::Batch, true);
    assert_eq!(progress.out_interval(ExchangeTiming::Batch), None);
    assert_eq!(
        progress.out_interval(ExchangeTiming::MutexOut {
            interval: Duration::from_millis(8)
        }),
        Some(Duration::from_millis(8))
    );
    progress.finish_batch(&mut clock, Duration::from_millis(20), ExchangeTiming::Batch);
    assert_eq!(
        clock.out_wait(Duration::from_millis(21), Duration::from_millis(2)),
        Some(Duration::from_millis(2))
    );
}

#[test]
fn a_batch_native_error_does_not_publish_a_normal_batch_return() {
    let mut progress = LeaseProgress::default();
    let mut clock = SourceClock::default();
    clock.native_return(Duration::from_millis(10), NativeReturn::Get);
    progress.dispatched(ExchangeTiming::Batch, true);
    progress.returned(ExchangeTiming::Batch, false);
    progress.finish_batch(&mut clock, Duration::from_millis(20), ExchangeTiming::Batch);
    assert_eq!(
        clock.out_wait(Duration::from_millis(21), Duration::from_millis(2)),
        None
    );
}

#[test]
fn initial_topology_barrier_prevents_native_open() {
    let pool = paused_pool();
    let fixture = Fixture::new(190);
    let mut backend = pool
        .backend_for(Uuid::new_v4(), Arc::new(AtomicBool::new(false)))
        .unwrap();
    assert!(backend.open(&fixture.0.node).is_err());
    assert_eq!(fixture.0.opened.load(Ordering::Acquire), 0);
    pool.resume_after_reconcile().unwrap();
    let handle = backend.open(&fixture.0.node).unwrap();
    assert_eq!(fixture.0.opened.load(Ordering::Acquire), 1);
    drop(handle);
    stop(&pool);
}

#[test]
fn a_pool_without_an_exact_topology_owner_cannot_resume_or_open() {
    let pool = Pool::new(raw_factory, identity);
    let fixture = Fixture::new(190);
    assert!(pool.resume_after_reconcile().is_err());
    let mut backend = pool
        .backend_for(Uuid::new_v4(), Arc::new(AtomicBool::new(false)))
        .unwrap();
    assert!(backend.open(&fixture.0.node).is_err());
    assert_eq!(fixture.0.opened.load(Ordering::Acquire), 0);
    stop(&pool);
}

#[test]
fn publisher_revocation_closes_admission_before_the_consumer_invalidates() {
    let pool = pool();
    let revision = topology_revision(&pool);
    let fixture = Fixture::new(190);
    let mut backend = pool
        .backend_for(Uuid::new_v4(), Arc::new(AtomicBool::new(false)))
        .unwrap();
    revision.store(0, Ordering::Release);
    assert!(backend.open(&fixture.0.node).is_err());
    pool.resume_after_reconcile().unwrap();
    assert!(backend.open(&fixture.0.node).is_err());
    assert_eq!(fixture.0.opened.load(Ordering::Acquire), 0);
    revision.store(2, Ordering::Release);
    backend
        .open(&fixture.0.node)
        .unwrap()
        .send_feature(&[0x30])
        .unwrap();
    stop(&pool);
}

#[test]
fn a_new_topology_revision_during_resolution_rejects_the_previous_snapshot() {
    let pool = pool();
    let revision = topology_revision(&pool);
    let fixture = Fixture::new(190);
    let gate = fixture.gate(&fixture.0.resolving);
    thread::scope(|scope| {
        let _release = ReleaseGate(gate.clone());
        let pool_ref = &pool;
        let node = fixture.0.node.clone();
        let opening = scope.spawn(move || {
            let mut backend = pool_ref
                .backend_for(Uuid::new_v4(), Arc::new(AtomicBool::new(false)))
                .unwrap();
            assert!(backend.open(&node).is_err());
        });
        gate.wait_entered();
        revision.store(0, Ordering::Release);
        revision.store(2, Ordering::Release);
        gate.release();
        opening.join().unwrap();
    });
    assert_eq!(fixture.0.opened.load(Ordering::Acquire), 0);
    assert_eq!(pool.pending_count().unwrap(), 0);
    stop(&pool);
}

#[test]
fn topology_revocation_during_native_io_waits_for_return_and_rejects_the_old_result() {
    let pool = pool();
    let revision = topology_revision(&pool);
    let fixture = Fixture::new(190);
    let old = open(&pool, &fixture.0.node);
    let old_epoch = pool.entry(&fixture.0.node).unwrap().epoch;
    let gate = fixture.gate(&fixture.0.sending);
    let returned = Arc::new(AtomicBool::new(false));
    thread::scope(|scope| {
        let _release = ReleaseGate(gate.clone());
        let pool_ref = &pool;
        let node = fixture.0.node.clone();
        let returned_signal = returned.clone();
        let io = scope.spawn(move || {
            let transport = open(pool_ref, &node);
            assert!(transport.send_feature(&[0x10]).is_err());
            returned_signal.store(true, Ordering::Release);
        });
        gate.wait_entered();
        revision.store(0, Ordering::Release);
        assert!(!returned.load(Ordering::Acquire));
        revision.store(2, Ordering::Release);
        assert_eq!(pool.reconcile().unwrap(), 0);
        assert_eq!(fixture.0.closed.load(Ordering::Acquire), 0);
        let mut backend = pool_ref
            .backend_for(Uuid::new_v4(), Arc::new(AtomicBool::new(false)))
            .unwrap();
        assert!(backend.open(&fixture.0.node).is_err());
        gate.release();
        io.join().unwrap();
    });
    reap(&pool);
    let new = open(&pool, &fixture.0.node);
    assert_ne!(pool.entry(&fixture.0.node).unwrap().epoch, old_epoch);
    assert!(old.send_feature(&[0x20]).is_err());
    new.send_feature(&[0x30]).unwrap();
    assert_eq!(
        fixture.observed(),
        [Observed::Send(vec![0x10]), Observed::Send(vec![0x30])]
    );
    stop(&pool);
}

#[test]
fn resuming_a_new_revision_never_revives_an_old_lease_or_interrupt_proxy() {
    let pool = pool();
    let revision = topology_revision(&pool);
    let fixture = Fixture::new(190);
    let feature = open(&pool, &fixture.0.node);
    let lease = feature
        .begin_exchange(ExchangeOptions::mutex_out(Duration::ZERO))
        .unwrap();
    let mut backend = pool
        .backend_for(Uuid::new_v4(), Arc::new(AtomicBool::new(false)))
        .unwrap();
    let interrupt = backend.open_interrupt(&fixture.0.node).unwrap();
    revision.store(0, Ordering::Release);
    revision.store(2, Ordering::Release);
    assert!(lease.retry_release().is_err());
    assert!(lease.finish().is_err());
    assert!(feature.send_feature(&[0x20]).is_err());
    assert!(interrupt.read(&mut [0; 4], 1).is_err());
    assert!(fixture.observed().is_empty());
    reap(&pool);
    stop(&pool);
}

#[test]
fn replacing_a_topology_owner_invalidates_even_an_equal_numeric_revision() {
    let pool = pool();
    let fixture = Fixture::new(190);
    let old = open(&pool, &fixture.0.node);
    let old_epoch = pool.entry(&fixture.0.node).unwrap().epoch;
    pool.install_topology_gate(TopologyGate::Scripted(Arc::new(AtomicU64::new(1))))
        .unwrap();
    pool.resume_after_reconcile().unwrap();
    assert!(old.send_feature(&[0x20]).is_err());
    reap(&pool);
    let new = open(&pool, &fixture.0.node);
    assert_ne!(pool.entry(&fixture.0.node).unwrap().epoch, old_epoch);
    new.send_feature(&[0x30]).unwrap();
    stop(&pool);
}

#[test]
fn separate_proxies_share_one_native_handle_and_preserve_counts_bytes_and_errors() {
    let pool = pool();
    let fixture = Fixture::new(190);
    let a = open(&pool, &fixture.0.node);
    let b = open(&pool, &fixture.0.node);
    assert_eq!(fixture.0.opened.load(Ordering::Acquire), 1);
    a.send_feature(&[0x11]).unwrap();
    let mut report = [0, 9, 9, 9];
    assert_eq!(b.get_feature(&mut report).unwrap(), 3);
    assert_eq!(report, [0x7f, 0x12, 0x34, 9]);
    assert_eq!(b.write_output_counted(&[1, 2, 3, 4]).unwrap(), 2);
    assert!(
        format!("{:#}", a.send_feature(&[0xee]).unwrap_err())
            .contains("scripted native Feature failure")
    );
    drop((a, b));
    assert_eq!(fixture.0.closed.load(Ordering::Acquire), 0);
    stop(&pool);
    assert_eq!(fixture.0.closed.load(Ordering::Acquire), 1);
}

#[test]
fn an_exchange_keeps_get_with_its_out_until_explicit_finish() {
    let pool = pool();
    let fixture = Fixture::new(190);
    let a = open(&pool, &fixture.0.node);
    let lease = a
        .begin_exchange(ExchangeOptions::mutex_out(Duration::ZERO))
        .unwrap();
    lease.transport().send_feature(&[0x10]).unwrap();
    thread::scope(|scope| {
        let node = fixture.0.node.clone();
        let pool = &pool;
        let b = scope.spawn(move || {
            let b = open(pool, &node);
            let lease = b.begin_exchange(ExchangeOptions::plain()).unwrap();
            lease.transport().send_feature(&[0x20]).unwrap();
            lease.finish().unwrap();
        });
        wait_queued(pool, &fixture.0.node);
        let mut report = [0; 4];
        lease.transport().get_feature(&mut report).unwrap();
        assert_eq!(
            fixture.observed(),
            [Observed::Send(vec![0x10]), Observed::Get]
        );
        lease.finish().unwrap();
        b.join().unwrap();
    });
    assert_eq!(
        fixture.observed(),
        [
            Observed::Send(vec![0x10]),
            Observed::Get,
            Observed::Send(vec![0x20])
        ]
    );
    stop(&pool);
}

#[test]
fn retry_releases_the_hid_cycle_and_reenters_fifo_with_a_new_request() {
    let pool = pool();
    let fixture = Fixture::new(191);
    let a = open(&pool, &fixture.0.node);
    let lease = a
        .begin_exchange(ExchangeOptions::mutex_out(Duration::ZERO))
        .unwrap();
    lease.transport().send_feature(&[0x10]).unwrap();
    thread::scope(|scope| {
        let node = fixture.0.node.clone();
        let pool = &pool;
        let b = scope.spawn(move || {
            let b = open(pool, &node);
            b.send_feature(&[0x20]).unwrap();
        });
        wait_queued(pool, &fixture.0.node);
        lease.retry_release().unwrap();
        lease.transport().send_feature(&[0x30]).unwrap();
        lease.finish().unwrap();
        b.join().unwrap();
    });
    assert_eq!(
        fixture.observed(),
        [
            Observed::Send(vec![0x10]),
            Observed::Send(vec![0x20]),
            Observed::Send(vec![0x30])
        ]
    );
    stop(&pool);
}

#[test]
fn same_source_commands_keep_the_outer_scope_across_a_hid_retry() {
    let pool = pool();
    let first = Fixture::new(190);
    let second = Fixture::new(190);
    let a = open(&pool, &first.0.node);
    let options = ExchangeOptions::mutex_out(Duration::ZERO).with_source_scope(
        SourceCommandScope::Protocol25 {
            topology: Protocol25Topology::Normal,
        },
    );
    let lease = a.begin_exchange(options.clone()).unwrap();
    thread::scope(|scope| {
        let node = second.0.node.clone();
        let pool = &pool;
        let b = scope.spawn(move || {
            let b = open(pool, &node);
            let lease = b.begin_exchange(options).unwrap();
            lease.transport().send_feature(&[0x20]).unwrap();
            lease.finish().unwrap();
        });
        wait_command_queued(pool);
        lease.retry_release().unwrap();
        assert!(second.observed().is_empty());
        lease.transport().send_feature(&[0x10]).unwrap();
        lease.finish().unwrap();
        b.join().unwrap();
    });
    assert_eq!(first.observed(), [Observed::Send(vec![0x10])]);
    assert_eq!(second.observed(), [Observed::Send(vec![0x20])]);
    stop(&pool);
}

#[test]
fn queued_cancellation_never_issues_native_io() {
    let pool = pool();
    let fixture = Fixture::new(190);
    let a = open(&pool, &fixture.0.node);
    let lease = a.begin_exchange(ExchangeOptions::plain()).unwrap();
    let canceled = Arc::new(AtomicBool::new(false));
    thread::scope(|scope| {
        let _cancel = CancelOnDrop(canceled.clone());
        let node = fixture.0.node.clone();
        let pool = &pool;
        let canceled_b = canceled.clone();
        let b = scope.spawn(move || {
            let b = open(pool, &node);
            assert!(
                b.begin_exchange(ExchangeOptions::plain().with_cancellation(canceled_b))
                    .is_err()
            );
        });
        wait_queued(pool, &fixture.0.node);
        canceled.store(true, Ordering::Release);
        b.join().unwrap();
    });
    assert!(fixture.observed().is_empty());
    lease.finish().unwrap();
    stop(&pool);
}

#[test]
fn caller_validation_cancels_an_inner_waiter_without_cancelling_the_active_holder() {
    let pool = pool();
    let fixture = Fixture::new(190);
    let a = open(&pool, &fixture.0.node);
    let lease = a.begin_exchange(ExchangeOptions::plain()).unwrap();
    let canceled = Arc::new(AtomicBool::new(false));
    thread::scope(|scope| {
        let _cancel = CancelOnDrop(canceled.clone());
        let pool_ref = &pool;
        let node = fixture.0.node.clone();
        let signal = canceled.clone();
        let (sent, received) = mpsc::sync_channel(1);
        let waiting = scope.spawn(move || {
            let b = open(pool_ref, &node);
            let owner_thread = thread::current().id();
            let local = Rc::new(());
            let validate = Rc::new(move || {
                assert_eq!(thread::current().id(), owner_thread);
                assert_eq!(Rc::strong_count(&local), 1);
                ensure!(
                    !signal.load(Ordering::Acquire),
                    "domain operation canceled while queued"
                );
                Ok(())
            });
            let result = b.begin_exchange_validated(ExchangeOptions::plain(), validate);
            sent.send(result.err().map(|error| format!("{error:#}")))
                .unwrap();
        });
        wait_queued(&pool, &fixture.0.node);
        canceled.store(true, Ordering::Release);
        let error = received
            .recv_timeout(Duration::from_secs(3))
            .expect("domain cancellation waited for the active holder")
            .unwrap();
        assert!(error.contains("domain operation canceled while queued"));
        waiting.join().unwrap();
        assert!(
            pool.entry(&fixture.0.node)
                .unwrap()
                .state
                .lock()
                .unwrap()
                .queued
                .is_empty()
        );
        lease.transport().send_feature(&[0x10]).unwrap();
        lease.finish().unwrap();
    });
    assert_eq!(fixture.observed(), [Observed::Send(vec![0x10])]);
    stop(&pool);
}

#[test]
fn caller_validation_cancels_a_source_command_waiter_and_removes_only_its_identity() {
    let pool = pool();
    let first = Fixture::new(190);
    let second = Fixture::new(190);
    let a = open(&pool, &first.0.node);
    let options = ExchangeOptions::plain().with_source_scope(SourceCommandScope::Protocol25 {
        topology: Protocol25Topology::Normal,
    });
    let lease = a.begin_exchange(options.clone()).unwrap();
    let canceled = Arc::new(AtomicBool::new(false));
    thread::scope(|scope| {
        let _cancel = CancelOnDrop(canceled.clone());
        let pool_ref = &pool;
        let node = second.0.node.clone();
        let signal = canceled.clone();
        let waiting_options = options.clone();
        let (sent, received) = mpsc::sync_channel(1);
        let waiting = scope.spawn(move || {
            let b = open(pool_ref, &node);
            let owner_thread = thread::current().id();
            let validate = Rc::new(move || {
                assert_eq!(thread::current().id(), owner_thread);
                ensure!(
                    !signal.load(Ordering::Acquire),
                    "domain command canceled while queued"
                );
                Ok(())
            });
            let result = b.begin_exchange_validated(waiting_options, validate);
            sent.send(result.err().map(|error| format!("{error:#}")))
                .unwrap();
        });
        wait_command_queued(&pool);
        canceled.store(true, Ordering::Release);
        let error = received
            .recv_timeout(Duration::from_secs(3))
            .expect("domain cancellation waited for another source command")
            .unwrap();
        assert!(error.contains("domain command canceled while queued"));
        waiting.join().unwrap();
        assert!(second.observed().is_empty());
        lease.transport().send_feature(&[0x10]).unwrap();
        lease.finish().unwrap();
    });
    let next = open(&pool, &second.0.node);
    let next = next.begin_exchange(options).unwrap();
    next.transport().send_feature(&[0x20]).unwrap();
    next.finish().unwrap();
    assert_eq!(first.observed(), [Observed::Send(vec![0x10])]);
    assert_eq!(second.observed(), [Observed::Send(vec![0x20])]);
    stop(&pool);
}

#[test]
fn caller_validation_cancels_a_retry_requeued_behind_another_holder() {
    let pool = pool();
    let fixture = Fixture::new(190);
    let canceled = Arc::new(AtomicBool::new(false));
    let hold_next = Arc::new(Gate::default());
    thread::scope(|scope| {
        let _cancel = CancelOnDrop(canceled.clone());
        let _release = ReleaseGate(hold_next.clone());
        let pool_ref = &pool;
        let node = fixture.0.node.clone();
        let signal = canceled.clone();
        let (first_sent, first_received) = mpsc::sync_channel(1);
        let (retry_sent, retry_received) = mpsc::sync_channel(1);
        let (done_sent, done_received) = mpsc::sync_channel(1);
        let first = scope.spawn(move || {
            let transport = open(pool_ref, &node);
            let validate = Rc::new(move || {
                ensure!(
                    !signal.load(Ordering::Acquire),
                    "domain retry canceled while queued"
                );
                Ok(())
            });
            let lease = transport
                .begin_exchange_validated(ExchangeOptions::plain(), validate)
                .unwrap();
            lease.transport().send_feature(&[0x10]).unwrap();
            first_sent.send(()).unwrap();
            retry_received.recv_timeout(Duration::from_secs(3)).unwrap();
            let error = lease.retry_release().unwrap_err();
            done_sent.send(format!("{error:#}")).unwrap();
            drop(lease);
        });
        first_received.recv_timeout(Duration::from_secs(3)).unwrap();
        let node = fixture.0.node.clone();
        let holding = hold_next.clone();
        let second = scope.spawn(move || {
            let transport = open(pool_ref, &node);
            let lease = transport.begin_exchange(ExchangeOptions::plain()).unwrap();
            holding.enter();
            lease.transport().send_feature(&[0x20]).unwrap();
            lease.finish().unwrap();
        });
        wait_queued(&pool, &fixture.0.node);
        retry_sent.send(()).unwrap();
        hold_next.wait_entered();
        wait_queued(&pool, &fixture.0.node);
        canceled.store(true, Ordering::Release);
        assert!(
            done_received
                .recv_timeout(Duration::from_secs(3))
                .expect("retry cancellation waited for the next holder")
                .contains("domain retry canceled while queued")
        );
        first.join().unwrap();
        hold_next.release();
        second.join().unwrap();
    });
    assert_eq!(
        fixture.observed(),
        [Observed::Send(vec![0x10]), Observed::Send(vec![0x20])]
    );
    stop(&pool);
}

#[test]
fn caller_validation_preserves_native_return_and_canceled_finish_forces_cleanup() {
    let pool = pool();
    let fixture = Fixture::new(190);
    let gate = fixture.gate(&fixture.0.sending);
    let canceled = Arc::new(AtomicBool::new(false));
    let options = ExchangeOptions::mutex_out(Duration::ZERO).with_source_scope(
        SourceCommandScope::Protocol25 {
            topology: Protocol25Topology::Normal,
        },
    );
    thread::scope(|scope| {
        let _cancel = CancelOnDrop(canceled.clone());
        let _release = ReleaseGate(gate.clone());
        let pool_ref = &pool;
        let node = fixture.0.node.clone();
        let signal = canceled.clone();
        let worker_options = options.clone();
        let (sent, received) = mpsc::sync_channel(1);
        let worker = scope.spawn(move || {
            let transport = open(pool_ref, &node);
            let validate = Rc::new(move || {
                ensure!(
                    !signal.load(Ordering::Acquire),
                    "domain operation canceled after native dispatch"
                );
                Ok(())
            });
            let lease = transport
                .begin_exchange_validated(worker_options, validate)
                .unwrap();
            let packet_result = lease.transport().send_feature(&[0x10]);
            let finish = lease.finish();
            sent.send((
                packet_result.map_err(|error| format!("{error:#}")),
                finish.err().map(|error| format!("{error:#}")),
            ))
            .unwrap();
        });
        gate.wait_entered();
        let entry = pool.entry(&fixture.0.node).unwrap();
        let control = entry.state.lock().unwrap().active.clone().unwrap();
        canceled.store(true, Ordering::Release);
        assert!(matches!(
            received.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(!control.forced.load(Ordering::Acquire));
        gate.release();
        let (packet_result, finish_error) = received
            .recv_timeout(Duration::from_secs(3))
            .expect("caller did not consume the terminal native result");
        assert!(packet_result.is_ok(), "{packet_result:?}");
        assert!(
            finish_error
                .unwrap()
                .contains("domain operation canceled after native dispatch")
        );
        worker.join().unwrap();
        assert!(control.forced.load(Ordering::Acquire));
    });
    let recovery = open(&pool, &fixture.0.node);
    let lease = recovery.begin_exchange(options).unwrap();
    lease.transport().send_feature(&[0x20]).unwrap();
    lease.finish().unwrap();
    assert_eq!(
        fixture.observed(),
        [Observed::Send(vec![0x10]), Observed::Send(vec![0x20])]
    );
    assert_eq!(fixture.0.opened.load(Ordering::Acquire), 1);
    stop(&pool);
}

#[test]
fn caller_validation_can_leave_a_native_open_wait_without_releasing_its_blocked_actor() {
    let pool = pool();
    let fixture = Fixture::new(190);
    let gate = fixture.gate(&fixture.0.opening);
    let canceled = Arc::new(AtomicBool::new(false));
    thread::scope(|scope| {
        let _cancel = CancelOnDrop(canceled.clone());
        let _release = ReleaseGate(gate.clone());
        let pool_ref = &pool;
        let node = fixture.0.node.clone();
        let signal = canceled.clone();
        let (sent, received) = mpsc::sync_channel(1);
        let opening = scope.spawn(move || {
            let mut backend = pool_ref
                .backend_for(Uuid::new_v4(), Arc::new(AtomicBool::new(false)))
                .unwrap();
            let owner_thread = thread::current().id();
            let validate = Rc::new(move || {
                assert_eq!(thread::current().id(), owner_thread);
                ensure!(
                    !signal.load(Ordering::Acquire),
                    "domain native open canceled"
                );
                Ok(())
            });
            let result = backend.open_validated(&node, validate);
            sent.send(result.err().map(|error| format!("{error:#}")))
                .unwrap();
        });
        gate.wait_entered();
        canceled.store(true, Ordering::Release);
        assert!(
            received
                .recv_timeout(Duration::from_secs(3))
                .expect("caller cancellation waited for native open terminal return")
                .unwrap()
                .contains("domain native open canceled")
        );
        opening.join().unwrap();
        assert_eq!(pool.reconcile().unwrap(), 0);
        assert_eq!(pool.pending_count().unwrap(), 1);
        assert_eq!(fixture.0.closed.load(Ordering::Acquire), 0);
        gate.release();
    });
    let recovery = open(&pool, &fixture.0.node);
    recovery.send_feature(&[0x20]).unwrap();
    assert_eq!(fixture.0.opened.load(Ordering::Acquire), 1);
    stop(&pool);
    assert_eq!(fixture.0.closed.load(Ordering::Acquire), 1);
}

#[test]
fn caller_validation_can_leave_a_native_interrupt_open_wait_before_terminal_return() {
    let pool = pool();
    let fixture = Fixture::new(190);
    let _feature = open(&pool, &fixture.0.node);
    let gate = fixture.gate(&fixture.0.opening);
    let canceled = Arc::new(AtomicBool::new(false));
    thread::scope(|scope| {
        let _cancel = CancelOnDrop(canceled.clone());
        let _release = ReleaseGate(gate.clone());
        let pool_ref = &pool;
        let node = fixture.0.node.clone();
        let signal = canceled.clone();
        let (sent, received) = mpsc::sync_channel(1);
        let opening = scope.spawn(move || {
            let mut backend = pool_ref
                .backend_for(Uuid::new_v4(), Arc::new(AtomicBool::new(false)))
                .unwrap();
            let validate = Rc::new(move || {
                ensure!(
                    !signal.load(Ordering::Acquire),
                    "domain interrupt open canceled"
                );
                Ok(())
            });
            let result = backend.open_interrupt_validated(&node, validate);
            sent.send(result.err().map(|error| format!("{error:#}")))
                .unwrap();
        });
        gate.wait_entered();
        canceled.store(true, Ordering::Release);
        assert!(
            received
                .recv_timeout(Duration::from_secs(3))
                .expect("interrupt cancellation waited for native open terminal return")
                .unwrap()
                .contains("domain interrupt open canceled")
        );
        opening.join().unwrap();
        assert_eq!(pool.reconcile().unwrap(), 0);
        assert_eq!(pool.pending_count().unwrap(), 1);
        gate.release();
    });
    let mut backend = pool
        .backend_for(Uuid::new_v4(), Arc::new(AtomicBool::new(false)))
        .unwrap();
    let recovery = backend.open_interrupt(&fixture.0.node).unwrap();
    assert_eq!(recovery.read(&mut [0; 4], 1).unwrap(), 2);
    stop(&pool);
}

#[test]
fn blocked_native_io_retains_its_epoch_until_terminal_join_and_reopen_rejects_old_proxy() {
    let pool = pool();
    let fixture = Fixture::new(190);
    let old = open(&pool, &fixture.0.node);
    let old_epoch = pool.entry(&fixture.0.node).unwrap().epoch;
    let gate = fixture.gate(&fixture.0.sending);
    let completed = Arc::new(AtomicBool::new(false));
    thread::scope(|scope| {
        let _release = ReleaseGate(gate.clone());
        let pool_ref = &pool;
        let node = fixture.0.node.clone();
        let done = completed.clone();
        let io = scope.spawn(move || {
            let handle = open(pool_ref, &node);
            assert!(handle.send_feature(&[0x10]).is_err());
            done.store(true, Ordering::Release);
        });
        gate.wait_entered();
        pool.invalidate_path(&fixture.0.node.path).unwrap();
        assert!(!completed.load(Ordering::Acquire));
        assert_eq!(pool.reconcile().unwrap(), 0);
        assert_eq!(pool.pending_count().unwrap(), 1);
        let mut backend = pool
            .backend_for(Uuid::new_v4(), Arc::new(AtomicBool::new(false)))
            .unwrap();
        assert!(backend.open(&fixture.0.node).is_err());
        gate.release();
        io.join().unwrap();
    });
    let deadline = Instant::now() + Duration::from_secs(3);
    while pool.pending_count().unwrap() != 0 {
        pool.reconcile().unwrap();
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    let new = open(&pool, &fixture.0.node);
    assert_ne!(pool.entry(&fixture.0.node).unwrap().epoch, old_epoch);
    assert!(old.send_feature(&[0x20]).is_err());
    new.send_feature(&[0x30]).unwrap();
    assert_eq!(fixture.0.opened.load(Ordering::Acquire), 2);
    stop(&pool);
    assert_eq!(fixture.0.closed.load(Ordering::Acquire), 2);
}

#[test]
fn invalidation_does_not_wait_for_blocked_native_initialization() {
    let pool = pool();
    let fixture = Fixture::new(190);
    let gate = fixture.gate(&fixture.0.opening);
    thread::scope(|scope| {
        let _release = ReleaseGate(gate.clone());
        let pool_ref = &pool;
        let node = fixture.0.node.clone();
        let opening = scope.spawn(move || {
            let mut backend = pool_ref
                .backend_for(Uuid::new_v4(), Arc::new(AtomicBool::new(false)))
                .unwrap();
            assert!(backend.open(&node).is_err());
        });
        gate.wait_entered();
        let (sent, received) = mpsc::sync_channel(1);
        let path = fixture.0.node.path.clone();
        scope.spawn(move || {
            pool_ref.invalidate_path(&path).unwrap();
            sent.send(()).unwrap();
        });
        received
            .recv_timeout(Duration::from_secs(1))
            .expect("topology invalidation blocked on native initialization");
        assert_eq!(pool.reconcile().unwrap(), 0);
        gate.release();
        opening.join().unwrap();
    });
    stop(&pool);
}

#[test]
fn topology_change_during_identity_resolution_cannot_publish_an_old_actor() {
    let pool = pool();
    let fixture = Fixture::new(190);
    let gate = fixture.gate(&fixture.0.resolving);
    thread::scope(|scope| {
        let _release = ReleaseGate(gate.clone());
        let pool_ref = &pool;
        let node = fixture.0.node.clone();
        let opening = scope.spawn(move || {
            let mut backend = pool_ref
                .backend_for(Uuid::new_v4(), Arc::new(AtomicBool::new(false)))
                .unwrap();
            assert!(backend.open(&node).is_err());
        });
        gate.wait_entered();
        pool.invalidate_path(&fixture.0.node.path).unwrap();
        gate.release();
        opening.join().unwrap();
    });
    assert_eq!(fixture.0.opened.load(Ordering::Acquire), 0);
    assert_eq!(pool.pending_count().unwrap(), 0);
    stop(&pool);
}

#[test]
fn interrupt_actor_does_not_wait_for_a_feature_exchange_to_release() {
    let pool = pool();
    let fixture = Fixture::new(190);
    let feature = open(&pool, &fixture.0.node);
    let lease = feature.begin_exchange(ExchangeOptions::plain()).unwrap();
    let mut backend = pool
        .backend_for(Uuid::new_v4(), Arc::new(AtomicBool::new(false)))
        .unwrap();
    let interrupt = backend.open_interrupt(&fixture.0.node).unwrap();
    let mut report = [0; 4];
    assert_eq!(interrupt.read(&mut report, 1).unwrap(), 2);
    assert_eq!(report, [0x81, 0x53, 0, 0]);
    lease.finish().unwrap();
    stop(&pool);
}

#[test]
fn stale_and_foreign_packet_signatures_are_rejected_without_native_io() {
    let pool = pool();
    let fixture = Fixture::new(190);
    let entry = pool.entry(&fixture.0.node).unwrap();
    entry
        .wait_ready_validated(&AtomicBool::new(false), &no_validation)
        .unwrap();
    let lease = entry
        .acquire(
            Uuid::new_v4(),
            Arc::new(AtomicBool::new(false)),
            ExchangeOptions::plain(),
            &no_validation,
        )
        .unwrap();
    for signature in [
        Signature {
            owner: Uuid::new_v4(),
            ..lease.control.signature
        },
        Signature {
            request: Uuid::new_v4(),
            ..lease.control.signature
        },
        Signature {
            epoch: Uuid::new_v4(),
            ..lease.control.signature
        },
    ] {
        let (response, received) = mpsc::sync_channel(1);
        lease
            .sender
            .send(ActorRequest {
                signature,
                operation: ActorOperation::Packet(Packet::Send {
                    bytes: vec![0x20],
                    collection_max: false,
                }),
                response,
            })
            .unwrap();
        assert!(received.recv().unwrap().is_err());
    }
    assert!(fixture.observed().is_empty());
    lease.finish().unwrap();
    stop(&pool);
}

#[test]
fn a_forced_guard_drop_releases_the_queue_without_native_io() {
    let pool = pool();
    let fixture = Fixture::new(190);
    let a = open(&pool, &fixture.0.node);
    let lease = a.begin_exchange(ExchangeOptions::plain()).unwrap();
    thread::scope(|scope| {
        let node = fixture.0.node.clone();
        let pool = &pool;
        let next = scope.spawn(move || open(pool, &node).send_feature(&[0x20]).unwrap());
        wait_queued(pool, &fixture.0.node);
        drop(lease);
        next.join().unwrap();
    });
    assert_eq!(fixture.observed(), [Observed::Send(vec![0x20])]);
    stop(&pool);
}

#[test]
fn exchange_queue_is_bounded_and_canceled_waiters_leave_no_native_work() {
    let pool = pool();
    let fixture = Fixture::new(190);
    let a = open(&pool, &fixture.0.node);
    let lease = a.begin_exchange(ExchangeOptions::plain()).unwrap();
    let canceled = Arc::new(AtomicBool::new(false));
    thread::scope(|scope| {
        let _cancel = CancelOnDrop(canceled.clone());
        let mut waiters = Vec::new();
        for _ in 0..QUEUE_CAPACITY {
            let pool = &pool;
            let node = fixture.0.node.clone();
            let canceled = canceled.clone();
            waiters.push(scope.spawn(move || {
                let mut backend = pool.backend_for(Uuid::new_v4(), canceled).unwrap();
                let handle = backend.open(&node).unwrap();
                assert!(handle.begin_exchange(ExchangeOptions::plain()).is_err());
            }));
        }
        let entry = pool.entry(&fixture.0.node).unwrap();
        let state = entry.state.lock().unwrap();
        let (state, _) = entry
            .changed
            .wait_timeout_while(state, Duration::from_secs(5), |state| {
                state.queued.len() < QUEUE_CAPACITY
            })
            .unwrap();
        assert_eq!(state.queued.len(), QUEUE_CAPACITY);
        drop(state);
        let full = a
            .begin_exchange(ExchangeOptions::plain())
            .err()
            .expect("queue accepted an extra exchange");
        assert!(full.to_string().contains("queue is full"));
        canceled.store(true, Ordering::Release);
        for waiter in waiters {
            waiter.join().unwrap();
        }
        assert!(entry.state.lock().unwrap().queued.is_empty());
    });
    assert!(fixture.observed().is_empty());
    lease.finish().unwrap();
    stop(&pool);
}

#[test]
fn uncertain_topology_can_resume_without_forgetting_a_blocked_old_path() {
    let pool = pool();
    let blocked = Fixture::new(190);
    let other = Fixture::new(191);
    let gate = blocked.gate(&blocked.0.opening);
    thread::scope(|scope| {
        let _release = ReleaseGate(gate.clone());
        let node = blocked.0.node.clone();
        let pool_ref = &pool;
        let opening = scope.spawn(move || {
            let mut backend = pool_ref
                .backend_for(Uuid::new_v4(), Arc::new(AtomicBool::new(false)))
                .unwrap();
            assert!(backend.open(&node).is_err());
        });
        gate.wait_entered();
        pool.invalidate_all().unwrap();
        let mut backend = pool
            .backend_for(Uuid::new_v4(), Arc::new(AtomicBool::new(false)))
            .unwrap();
        assert!(backend.open(&other.0.node).is_err());
        pool.resume_after_reconcile().unwrap();
        assert!(backend.open(&blocked.0.node).is_err());
        backend
            .open(&other.0.node)
            .unwrap()
            .send_feature(&[0x30])
            .unwrap();
        gate.release();
        opening.join().unwrap();
    });
    assert_eq!(other.observed(), [Observed::Send(vec![0x30])]);
    stop(&pool);
}
