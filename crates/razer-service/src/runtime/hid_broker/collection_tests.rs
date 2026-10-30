use super::*;
use razer_device::backend::{
    FeatureBatchFailure, FeatureBatchOperation, FeatureBatchProgress, FeatureBatchReportReturn,
    exchange_release_completed, execute_feature_batch, native_io_attempted, native_read_bytes,
    observe_feature_batch,
};
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
    CountedOutput(Vec<u8>),
    Batch(Vec<FeatureBatchOperation>),
    BatchSleep(u8),
}

#[derive(Debug)]
struct ScriptedNativeFailure;

impl std::fmt::Display for ScriptedNativeFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("scripted typed native terminal failure")
    }
}

impl std::error::Error for ScriptedNativeFailure {}

#[derive(Debug)]
struct CallerIdentityLost;

impl std::fmt::Display for CallerIdentityLost {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("caller identity changed while the native result was pending")
    }
}

impl std::error::Error for CallerIdentityLost {}

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
    lengths: ReportLengths,
    opened: AtomicUsize,
    closed: AtomicUsize,
    observed: Mutex<Vec<Observed>>,
    opening: Mutex<Option<Arc<Gate>>>,
    sending: Mutex<Option<Arc<Gate>>>,
    reading: Mutex<Option<Arc<Gate>>>,
    interrupt_reading: Mutex<Option<Arc<Gate>>>,
    batching: Mutex<Option<Arc<Gate>>>,
    resolving: Mutex<Option<Arc<Gate>>>,
    gates: Mutex<Vec<Arc<Gate>>>,
}

struct Fixture(Arc<MockState>);

impl Fixture {
    fn new(product_id: u16) -> Self {
        Self::with_lengths(product_id, lengths())
    }

    fn with_lengths(product_id: u16, lengths: ReportLengths) -> Self {
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
            lengths,
            opened: AtomicUsize::new(0),
            closed: AtomicUsize::new(0),
            observed: Mutex::new(Vec::new()),
            opening: Mutex::new(None),
            sending: Mutex::new(None),
            reading: Mutex::new(None),
            interrupt_reading: Mutex::new(None),
            batching: Mutex::new(None),
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
            state,
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

impl MockFeature {
    fn write_native_output(&self, observed: Observed) -> anyhow::Result<usize> {
        assert_eq!(self.thread, thread::current().id());
        self.state.observed.lock().unwrap().push(observed);
        let gate = self.state.sending.lock().unwrap().take();
        if let Some(gate) = gate {
            gate.enter();
        }
        Ok(2)
    }
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
        if report.first() == Some(&0xef) {
            return Err(ScriptedNativeFailure.into());
        }
        if report.first() == Some(&0xfd) {
            return Err(native_io_failure(
                NativeIoOutcome::Returned { byte_count: 2 },
                anyhow::anyhow!("scripted adapter rejects a normal native return"),
            ));
        }
        Ok(())
    }

    fn send_feature_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        self.send_feature(report)?;
        Ok(if report.first() == Some(&0xfc) {
            2
        } else {
            report.len()
        })
    }

    fn send_feature_at_collection_max_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        self.send_feature_counted(report)
    }

    fn send_feature_batch(&self, batch: &FeatureBatch) -> anyhow::Result<FeatureBatchReceipt> {
        assert_eq!(self.thread, thread::current().id());
        self.state
            .observed
            .lock()
            .unwrap()
            .push(Observed::Batch(batch.operations().to_vec()));
        let result = execute_feature_batch(
            batch,
            |report| self.send_feature_at_collection_max_counted(report),
            |milliseconds| {
                self.state
                    .observed
                    .lock()
                    .unwrap()
                    .push(Observed::BatchSleep(milliseconds));
                Ok(())
            },
        );
        let gate = self.state.batching.lock().unwrap().take();
        if let Some(gate) = gate {
            gate.enter();
        }
        result
    }

    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        assert_eq!(self.thread, thread::current().id());
        self.state.observed.lock().unwrap().push(Observed::Get);
        let gate = self.state.reading.lock().unwrap().take();
        if let Some(gate) = gate {
            gate.enter();
        }
        match report.first() {
            Some(0xef) => return Err(ScriptedNativeFailure.into()),
            Some(0xfb) => return Ok(report.len() + 1),
            _ => {}
        }
        report[..3].copy_from_slice(&[0x7f, 0x12, 0x34]);
        Ok(3)
    }

    fn write_output(&self, report: &[u8]) -> anyhow::Result<()> {
        require_complete_send(
            self.write_native_output(Observed::Output(report.to_vec()))?,
            report.len(),
        )
    }

    fn write_output_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        self.write_native_output(Observed::CountedOutput(report.to_vec()))
    }

    fn report_lengths(&self) -> anyhow::Result<ReportLengths> {
        Ok(self.state.lengths.clone())
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
    state: Arc<MockState>,
    thread: thread::ThreadId,
    _local: Rc<()>,
}

impl InterruptTransport for MockInterrupt {
    fn read(&self, report: &mut [u8], _: u32) -> anyhow::Result<usize> {
        assert_eq!(self.thread, thread::current().id());
        let gate = self.state.interrupt_reading.lock().unwrap().take();
        if let Some(gate) = gate {
            gate.enter();
        }
        match report.first() {
            Some(0xef) => return Err(ScriptedNativeFailure.into()),
            Some(0xfb) => return Ok(report.len() + 1),
            _ => {}
        }
        report[..2].copy_from_slice(&[0x81, 0x53]);
        Ok(2)
    }

    fn report_lengths(&self) -> ReportLengths {
        self.state.lengths.clone()
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
fn only_the_whole_batch_dispatch_uses_its_fixed_two_millisecond_out_interval() {
    let mut batch = FeatureBatch::new();
    batch.push_report(&[0x10, 1, 2, 3]).unwrap();
    batch.push_sleep(1);
    batch.push_report(&[0x20, 4, 5, 6]).unwrap();
    let packet = Packet::Batch(batch);
    let mut clock = SourceClock::default();
    clock.native_return(Duration::from_millis(10), NativeReturn::Get);
    let interval = packet.out_interval(ExchangeTiming::Batch).unwrap();
    assert_eq!(interval, Duration::from_millis(2));
    assert_eq!(
        clock.out_wait(Duration::from_millis(11), interval),
        Some(Duration::from_millis(2))
    );
    assert_eq!(clock.out_wait(Duration::from_millis(12), interval), None);
    let ordinary_send = Packet::Send {
        bytes: vec![0x10, 1, 2, 3],
        collection_max: true,
    };
    assert_eq!(ordinary_send.out_interval(ExchangeTiming::Batch), None);
    assert_eq!(ordinary_send.out_interval(ExchangeTiming::Plain), None);
    assert_eq!(
        ordinary_send.out_interval(ExchangeTiming::MutexOut {
            interval: Duration::from_millis(8)
        }),
        Some(Duration::from_millis(8))
    );
}

#[test]
fn only_a_normal_whole_batch_return_refreshes_the_clock_including_zero() {
    let fixture = Fixture::new(190);
    let mut backend = raw_factory().unwrap();
    let native = backend.open(&fixture.0.node).unwrap();
    let mut empty = FeatureBatch::new();
    let mut sleep_only = FeatureBatch::new();
    sleep_only.push_sleep(0);
    let mut partial_failure = FeatureBatch::new();
    partial_failure.push_report(&[0x10, 1, 2, 3]).unwrap();
    partial_failure.push_report(&[0xef, 4, 5, 6]).unwrap();
    for batch in [&empty, &sleep_only] {
        let mut clock = SourceClock::default();
        let reply = Packet::Batch(batch.clone())
            .execute(native.as_ref(), &mut clock, Instant::now())
            .unwrap();
        let PacketReply::Batch(receipt) = reply else {
            panic!("batch lost its typed receipt");
        };
        assert_eq!(receipt.raw_return(), 0);
        assert_eq!(receipt.byte_count(), 0);
        assert_eq!(
            clock.out_wait(Duration::ZERO, Duration::MAX),
            Some(Duration::from_millis(2))
        );
    }
    let mut clock = SourceClock::default();
    let error = Packet::Batch(partial_failure)
        .execute(native.as_ref(), &mut clock, Instant::now())
        .err()
        .expect("second report must reject the batch");
    assert_eq!(native_io_outcome(&error), Some(NativeIoOutcome::Rejected));
    assert!(error.downcast_ref::<ScriptedNativeFailure>().is_some());
    assert_eq!(clock.out_wait(Duration::ZERO, Duration::MAX), None);
    empty.push_report(&[0x10, 1, 2, 3]).unwrap();
    Packet::Batch(empty)
        .execute(native.as_ref(), &mut clock, Instant::now())
        .unwrap();
    assert_eq!(
        clock.out_wait(Duration::ZERO, Duration::MAX),
        Some(Duration::from_millis(2))
    );
}

#[test]
fn an_empty_batch_lease_finish_does_not_dispatch_or_refresh_the_clock() {
    let pool = pool();
    let fixture = Fixture::new(190);
    let entry = pool.entry(&fixture.0.node).unwrap();
    entry
        .wait_ready_validated(&AtomicBool::new(false), &no_validation)
        .unwrap();
    let mut backend = raw_factory().unwrap();
    let native = backend.open(&fixture.0.node).unwrap();
    let control = Arc::new(LeaseControl {
        signature: Signature {
            owner: Uuid::new_v4(),
            request: Uuid::new_v4(),
            epoch: entry.epoch,
        },
        canceled: Arc::new(AtomicBool::new(false)),
        options: ExchangeOptions::batch(),
        forced: AtomicBool::new(false),
    });
    let (sender, inbox) = mpsc::sync_channel(PACKET_CAPACITY);
    let (response, receipt) = mpsc::sync_channel(1);
    sender
        .send(ActorRequest {
            signature: control.signature,
            operation: ActorOperation::Finish,
            response,
        })
        .unwrap();
    let (granted, _admission) = mpsc::sync_channel(1);
    let pending = PendingLease {
        control,
        inbox,
        granted,
    };
    let mut clock = SourceClock::default();
    entry.run_lease(native.as_ref(), &mut clock, Instant::now(), &pending);
    expect_unit(receipt.recv().unwrap().unwrap()).unwrap();
    assert_eq!(clock.out_wait(Duration::ZERO, Duration::MAX), None);
    assert!(fixture.observed().is_empty());
    drop(native);
    stop(&pool);
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
fn ordinary_output_preserves_descriptor_admission_and_the_native_write_contract() {
    for leased in [false, true] {
        let pool = pool();
        let fixture = Fixture::with_lengths(
            190,
            ReportLengths {
                output: BTreeMap::from([(0, 2), (3, 4)]),
                ..lengths()
            },
        );
        let device = open(&pool, &fixture.0.node);
        let mut lease = leased.then(|| device.begin_exchange(ExchangeOptions::plain()).unwrap());
        let transport = lease
            .as_ref()
            .map_or(device.as_ref(), |lease| lease.transport());
        for report in [
            vec![],
            vec![4, 1, 2, 3],
            vec![0],
            vec![0, 1, 2],
            vec![0, 1, 2, 3, 4],
            vec![3, 1],
        ] {
            let error = transport.write_output(&report).unwrap_err();
            assert_eq!(native_io_outcome(&error), Some(NativeIoOutcome::NotCalled));
        }
        assert!(fixture.observed().is_empty());
        transport.write_output(&[0, 1]).unwrap();
        for report in [[0, 1, 2, 3], [3, 1, 2, 3]] {
            let error = transport.write_output(&report).unwrap_err();
            assert_eq!(
                native_io_outcome(&error),
                Some(NativeIoOutcome::Returned { byte_count: 2 })
            );
        }
        assert_eq!(
            fixture.observed(),
            [
                Observed::Output(vec![0, 1]),
                Observed::Output(vec![0, 1, 2, 3]),
                Observed::Output(vec![3, 1, 2, 3]),
            ]
        );
        if let Some(lease) = lease.take() {
            lease.finish().unwrap();
        }
        drop(lease);
        drop(device);
        stop(&pool);
    }
}

#[test]
fn counted_output_preserves_short_long_and_unobserved_id_buffers_for_its_native_owner() {
    for leased in [false, true] {
        let pool = pool();
        let fixture = Fixture::new(190);
        let device = open(&pool, &fixture.0.node);
        let mut lease = leased.then(|| device.begin_exchange(ExchangeOptions::plain()).unwrap());
        let transport = lease
            .as_ref()
            .map_or(device.as_ref(), |lease| lease.transport());
        for report in [vec![9], vec![0, 1, 2], vec![9, 1, 2, 3, 4]] {
            assert_eq!(transport.write_output_counted(&report).unwrap(), 2);
        }
        assert_eq!(
            fixture.observed(),
            [
                Observed::CountedOutput(vec![9]),
                Observed::CountedOutput(vec![0, 1, 2]),
                Observed::CountedOutput(vec![9, 1, 2, 3, 4]),
            ]
        );
        if let Some(lease) = lease.take() {
            lease.finish().unwrap();
        }
        drop(lease);
        drop(device);
        stop(&pool);
    }
}

#[test]
fn ordinary_output_preserves_the_count_when_its_owner_retires_during_native_io() {
    for report in [vec![0, 2], vec![3, 2, 3, 4]] {
        let pool = pool();
        let fixture = Fixture::with_lengths(
            190,
            ReportLengths {
                output: BTreeMap::from([(0, 2), (3, 4)]),
                ..lengths()
            },
        );
        let gate = fixture.gate(&fixture.0.sending);
        let canceled = Arc::new(AtomicBool::new(false));
        thread::scope(|scope| {
            let _release = ReleaseGate(gate.clone());
            let _cancel = CancelOnDrop(canceled.clone());
            let pool_ref = &pool;
            let node = fixture.0.node.clone();
            let signal = canceled.clone();
            let bytes = report.clone();
            let (sent, received) = mpsc::sync_channel(1);
            let worker = scope.spawn(move || {
                let transport = open(pool_ref, &node);
                let validate = Rc::new(move || {
                    ensure!(!signal.load(Ordering::Acquire), "output owner retired");
                    Ok(())
                });
                let lease = transport
                    .begin_exchange_validated(ExchangeOptions::plain(), validate)
                    .unwrap();
                let result = lease.transport().write_output(&bytes);
                let finish = lease.finish();
                sent.send((result, finish)).unwrap();
            });
            gate.wait_entered();
            canceled.store(true, Ordering::Release);
            assert!(matches!(
                received.try_recv(),
                Err(mpsc::TryRecvError::Empty)
            ));
            gate.release();
            let (result, finish) = received.recv_timeout(Duration::from_secs(3)).unwrap();
            let error = result.unwrap_err();
            assert_eq!(
                native_io_outcome(&error),
                Some(NativeIoOutcome::Returned { byte_count: 2 })
            );
            assert!(native_io_attempted(&Err(error.context("diagnostic owner"))));
            assert!(!exchange_release_completed(&finish.unwrap_err()));
            worker.join().unwrap();
        });
        let recovery = open(&pool, &fixture.0.node);
        recovery.send_feature(&[0x30]).unwrap();
        assert_eq!(
            fixture.observed(),
            [Observed::Output(report), Observed::Send(vec![0x30])]
        );
        assert_eq!(fixture.0.opened.load(Ordering::Acquire), 1);
        stop(&pool);
    }
}

#[test]
fn packet_actor_preserves_typed_terminal_errors_and_normal_return_facts() {
    let pool = pool();
    let fixture = Fixture::new(190);
    let device = open(&pool, &fixture.0.node);
    let terminal = device.send_feature(&[0xef]).unwrap_err();
    assert!(terminal.downcast_ref::<ScriptedNativeFailure>().is_some());
    assert_eq!(
        terminal.root_cause().to_string(),
        "scripted typed native terminal failure"
    );
    let returned = device.send_feature_counted(&[0xfd]).unwrap_err();
    assert_eq!(
        native_io_outcome(&returned),
        Some(NativeIoOutcome::Returned { byte_count: 2 })
    );
    assert_eq!(
        returned.root_cause().to_string(),
        "scripted adapter rejects a normal native return"
    );
    drop(device);
    stop(&pool);
}

#[test]
fn counted_feature_return_survives_the_actor_while_legacy_send_stays_strict() {
    let pool = pool();
    let fixture = Fixture::new(190);
    let device = open(&pool, &fixture.0.node);
    assert_eq!(device.send_feature_counted(&[0xfc, 1, 2, 3]).unwrap(), 2);
    assert_eq!(
        device
            .send_feature_at_collection_max_counted(&[0xfc, 1, 2, 3])
            .unwrap(),
        2
    );
    for error in [
        device.send_feature(&[0xfc, 1, 2, 3]).unwrap_err(),
        device
            .send_feature_at_collection_max(&[0xfc, 1, 2, 3])
            .unwrap_err(),
    ] {
        assert_eq!(
            native_io_outcome(&error),
            Some(NativeIoOutcome::Returned { byte_count: 2 })
        );
    }
    assert_eq!(fixture.observed().len(), 4);
    drop(device);
    stop(&pool);
}

#[test]
fn one_batch_packet_preserves_the_whole_receipt_and_native_operation_order() {
    let pool = pool();
    let fixture = Fixture::new(190);
    let device = open(&pool, &fixture.0.node);
    let mut batch = FeatureBatch::new();
    batch.push_report(&[0x10, 1, 2, 3]).unwrap();
    batch.push_sleep(0);
    batch.push_report(&[0x20, 4, 5, 6]).unwrap();
    let lease = device.begin_exchange(ExchangeOptions::batch()).unwrap();
    let receipt = lease.transport().send_feature_batch(&batch).unwrap();
    assert_eq!(receipt.byte_count(), 8);
    assert_eq!(receipt.raw_return(), 8);
    assert_eq!(
        receipt.progress(),
        &FeatureBatchProgress {
            report_attempts: vec![
                razer_device::backend::FeatureBatchReportAttempt {
                    operation_index: 0,
                    outcome: Some(NativeIoOutcome::Returned { byte_count: 4 }),
                },
                razer_device::backend::FeatureBatchReportAttempt {
                    operation_index: 2,
                    outcome: Some(NativeIoOutcome::Returned { byte_count: 4 }),
                },
            ],
            report_returns: vec![
                FeatureBatchReportReturn {
                    operation_index: 0,
                    requested_bytes: 4,
                    returned_bytes: 4,
                },
                FeatureBatchReportReturn {
                    operation_index: 2,
                    requested_bytes: 4,
                    returned_bytes: 4,
                },
            ],
            completed_operations: 3,
        }
    );
    lease.finish().unwrap();
    assert_eq!(
        fixture.observed(),
        [
            Observed::Batch(batch.operations().to_vec()),
            Observed::Send(vec![0x10, 1, 2, 3]),
            Observed::BatchSleep(0),
            Observed::Send(vec![0x20, 4, 5, 6]),
        ]
    );
    stop(&pool);
    assert_eq!(fixture.0.opened.load(Ordering::Acquire), 1);
    assert_eq!(fixture.0.closed.load(Ordering::Acquire), 1);
}

#[test]
fn zero_batch_returns_and_partial_failures_remain_distinct_through_the_actor() {
    let pool = pool();
    let fixture = Fixture::new(190);
    let device = open(&pool, &fixture.0.node);
    let empty = FeatureBatch::new();
    let mut sleep_only = FeatureBatch::new();
    sleep_only.push_sleep(0);
    for batch in [&empty, &sleep_only] {
        let receipt = device.send_feature_batch(batch).unwrap();
        assert_eq!(receipt.byte_count(), 0);
        assert_eq!(receipt.raw_return(), 0);
        assert!(receipt.progress().report_returns.is_empty());
        assert_eq!(
            receipt.progress().completed_operations,
            batch.operations().len()
        );
    }
    for second in [0xef, 0xfd] {
        let mut batch = FeatureBatch::new();
        batch.push_report(&[0x10, 1, 2, 3]).unwrap();
        batch.push_report(&[second, 4, 5, 6]).unwrap();
        batch.push_report(&[0x30, 7, 8, 9]).unwrap();
        let error = device.send_feature_batch(&batch).unwrap_err();
        assert_eq!(native_io_outcome(&error), Some(NativeIoOutcome::Rejected));
        let failure = FeatureBatchFailure::from_error(&error).unwrap();
        let mut returns = vec![FeatureBatchReportReturn {
            operation_index: 0,
            requested_bytes: 4,
            returned_bytes: 4,
        }];
        if second == 0xef {
            assert!(error.downcast_ref::<ScriptedNativeFailure>().is_some());
        } else {
            returns.push(FeatureBatchReportReturn {
                operation_index: 1,
                requested_bytes: 4,
                returned_bytes: 2,
            });
        }
        assert_eq!(failure.progress.report_returns, returns);
        assert_eq!(failure.progress.completed_operations, 1);
        assert_eq!(failure.raw_return, None);
    }
    assert!(
        !fixture
            .observed()
            .contains(&Observed::Send(vec![0x30, 7, 8, 9]))
    );
    stop(&pool);
}

#[test]
fn cancellation_waits_for_the_whole_batch_and_preserves_its_terminal_progress() {
    for second in [None, Some(0x20), Some(0xef), Some(0xfd)] {
        let pool = pool();
        let fixture = Fixture::new(190);
        let gate = fixture.gate(&fixture.0.batching);
        let canceled = Arc::new(AtomicBool::new(false));
        thread::scope(|scope| {
            let _cancel = CancelOnDrop(canceled.clone());
            let _release = ReleaseGate(gate.clone());
            let pool_ref = &pool;
            let node = fixture.0.node.clone();
            let signal = canceled.clone();
            let (sent, received) = mpsc::sync_channel(1);
            let worker = scope.spawn(move || {
                let mut backend = pool_ref.backend_for(Uuid::new_v4(), signal).unwrap();
                let device = backend.open(&node).unwrap();
                let mut batch = FeatureBatch::new();
                if let Some(second) = second {
                    batch.push_report(&[0x10, 1, 2, 3]).unwrap();
                    batch.push_report(&[second, 4, 5, 6]).unwrap();
                }
                sent.send(device.send_feature_batch(&batch).unwrap_err())
                    .unwrap();
            });
            gate.wait_entered();
            canceled.store(true, Ordering::Release);
            assert!(matches!(
                received.try_recv(),
                Err(mpsc::TryRecvError::Empty)
            ));
            gate.release();
            let error = received.recv_timeout(Duration::from_secs(3)).unwrap();
            let failure = FeatureBatchFailure::from_error(&error).unwrap();
            match second {
                None => {
                    assert_eq!(
                        native_io_outcome(&error),
                        Some(NativeIoOutcome::Returned { byte_count: 0 })
                    );
                    assert_eq!(failure.progress, FeatureBatchProgress::default());
                    assert_eq!(failure.raw_return, Some(0));
                }
                Some(0x20) => {
                    assert_eq!(
                        native_io_outcome(&error),
                        Some(NativeIoOutcome::Returned { byte_count: 8 })
                    );
                    assert_eq!(failure.progress.completed_operations, 2);
                    assert_eq!(failure.progress.report_returns.len(), 2);
                    assert_eq!(failure.raw_return, Some(8));
                }
                Some(second) => {
                    assert_eq!(native_io_outcome(&error), Some(NativeIoOutcome::Rejected));
                    assert_eq!(failure.progress.completed_operations, 1);
                    assert_eq!(failure.raw_return, None);
                    if second == 0xef {
                        assert!(error.downcast_ref::<ScriptedNativeFailure>().is_some());
                        assert_eq!(failure.progress.report_returns.len(), 1);
                    } else {
                        assert_eq!(failure.progress.report_returns.len(), 2);
                        assert_eq!(failure.progress.report_returns[1].returned_bytes, 2);
                    }
                }
            }
            assert!(format!("{error:#}").contains("cancel"));
            worker.join().unwrap();
        });
        stop(&pool);
        assert_eq!(fixture.0.closed.load(Ordering::Acquire), 1);
    }
}

#[test]
fn read_actor_errors_preserve_typed_causes_and_actual_oversized_counts() {
    let pool = pool();
    let fixture = Fixture::new(190);
    let device = open(&pool, &fixture.0.node);
    let mut backend = pool
        .backend_for(Uuid::new_v4(), Arc::new(AtomicBool::new(false)))
        .unwrap();
    let interrupt = backend.open_interrupt(&fixture.0.node).unwrap();
    for interrupt_read in [false, true] {
        for (first_byte, oversized) in [(0xef, false), (0xfb, true)] {
            let mut report = [first_byte; 4];
            let error = if interrupt_read {
                interrupt.read(&mut report, 1)
            } else {
                device.get_feature(&mut report)
            }
            .unwrap_err();
            if oversized {
                assert_eq!(
                    native_io_outcome(&error),
                    Some(NativeIoOutcome::Returned { byte_count: 5 })
                );
            } else {
                assert!(error.downcast_ref::<ScriptedNativeFailure>().is_some());
                assert_eq!(native_io_outcome(&error), None);
            }
            assert!(native_read_bytes(&error).is_none());
            assert_eq!(report, [first_byte; 4]);
        }
    }
    stop(&pool);
}

#[test]
fn cancellation_keeps_feature_terminal_counts_and_errors_without_publishing_the_read() {
    for (reading, first_byte, returned) in [
        (false, 0x10, Some(4)),
        (false, 0xef, None),
        (false, 0xfd, Some(2)),
        (true, 0x33, Some(3)),
        (true, 0xef, None),
        (true, 0xfb, Some(5)),
    ] {
        let pool = pool();
        let fixture = Fixture::new(190);
        let gate = fixture.gate(if reading {
            &fixture.0.reading
        } else {
            &fixture.0.sending
        });
        let canceled = Arc::new(AtomicBool::new(false));
        thread::scope(|scope| {
            let _cancel = CancelOnDrop(canceled.clone());
            let _release = ReleaseGate(gate.clone());
            let pool_ref = &pool;
            let node = fixture.0.node.clone();
            let signal = canceled.clone();
            let (sent, received) = mpsc::sync_channel(1);
            let worker = scope.spawn(move || {
                let mut backend = pool_ref.backend_for(Uuid::new_v4(), signal).unwrap();
                let device = backend.open(&node).unwrap();
                let mut report = [first_byte; 4];
                let result = if reading {
                    device.get_feature(&mut report)
                } else {
                    device.send_feature_counted(&report)
                };
                sent.send((result.unwrap_err(), report)).unwrap();
            });
            gate.wait_entered();
            canceled.store(true, Ordering::Release);
            assert!(matches!(
                received.try_recv(),
                Err(mpsc::TryRecvError::Empty)
            ));
            gate.release();
            let (error, report) = received.recv_timeout(Duration::from_secs(3)).unwrap();
            assert_eq!(
                native_io_outcome(&error),
                returned.map(|byte_count| NativeIoOutcome::Returned { byte_count })
            );
            if first_byte == 0xef {
                assert!(error.downcast_ref::<ScriptedNativeFailure>().is_some());
            }
            assert_eq!(
                native_read_bytes(&error),
                if reading && first_byte == 0x33 {
                    Some([0x7f, 0x12, 0x34].as_slice())
                } else {
                    None
                }
            );
            assert!(format!("{error:#}").contains("cancel"));
            assert_eq!(report, [first_byte; 4]);
            worker.join().unwrap();
        });
        assert_eq!(fixture.observed().len(), 1);
        stop(&pool);
        assert_eq!(fixture.0.closed.load(Ordering::Acquire), 1);
    }
}

#[test]
fn caller_validation_rejects_terminal_feature_results_on_its_owning_thread() {
    for (reading, first_byte, returned) in [
        (false, 0x10, Some(4)),
        (false, 0xef, None),
        (false, 0xfd, Some(2)),
        (true, 0x33, Some(3)),
        (true, 0xef, None),
        (true, 0xfb, Some(5)),
    ] {
        let pool = pool();
        let fixture = Fixture::new(190);
        let gate = fixture.gate(if reading {
            &fixture.0.reading
        } else {
            &fixture.0.sending
        });
        let stale = Arc::new(AtomicBool::new(false));
        let canceled = Arc::new(AtomicBool::new(false));
        thread::scope(|scope| {
            let _cancel = CancelOnDrop(canceled.clone());
            let _release = ReleaseGate(gate.clone());
            let pool_ref = &pool;
            let node = fixture.0.node.clone();
            let signal = canceled.clone();
            let identity = stale.clone();
            let (sent, received) = mpsc::sync_channel(1);
            let worker = scope.spawn(move || {
                let mut backend = pool_ref.backend_for(Uuid::new_v4(), signal).unwrap();
                let device = backend.open(&node).unwrap();
                let owner = thread::current().id();
                let validate = Rc::new(move || {
                    assert_eq!(thread::current().id(), owner);
                    if identity.load(Ordering::Acquire) {
                        Err(CallerIdentityLost.into())
                    } else {
                        Ok(())
                    }
                });
                let lease = device
                    .begin_exchange_validated(ExchangeOptions::plain(), validate)
                    .unwrap();
                let mut report = [first_byte; 4];
                let result = if reading {
                    lease.transport().get_feature(&mut report)
                } else {
                    lease.transport().send_feature_counted(&report)
                };
                let error = result.unwrap_err();
                let release = lease.finish().unwrap_err();
                assert!(release.downcast_ref::<CallerIdentityLost>().is_some());
                sent.send((error, report)).unwrap();
            });
            gate.wait_entered();
            stale.store(true, Ordering::Release);
            assert!(matches!(
                received.try_recv(),
                Err(mpsc::TryRecvError::Empty)
            ));
            gate.release();
            let (error, report) = received.recv_timeout(Duration::from_secs(3)).unwrap();
            assert_eq!(
                native_io_outcome(&error),
                returned.map(|byte_count| NativeIoOutcome::Returned { byte_count })
            );
            if first_byte == 0xef {
                assert!(error.downcast_ref::<ScriptedNativeFailure>().is_some());
            } else if matches!(first_byte, 0x10 | 0x33) {
                assert!(error.downcast_ref::<CallerIdentityLost>().is_some());
            }
            assert_eq!(
                native_read_bytes(&error),
                if reading && first_byte == 0x33 {
                    Some([0x7f, 0x12, 0x34].as_slice())
                } else {
                    None
                }
            );
            assert!(format!("{error:#}").contains("caller identity changed"));
            assert_eq!(report, [first_byte; 4]);
            worker.join().unwrap();
        });
        assert_eq!(fixture.observed().len(), 1);
        stop(&pool);
        assert_eq!(fixture.0.closed.load(Ordering::Acquire), 1);
    }
}

#[test]
fn caller_validation_waits_for_the_batch_terminal_receipt_before_rejecting_publication() {
    for second in [None, Some(0x20), Some(0xef), Some(0xfd)] {
        let pool = pool();
        let fixture = Fixture::new(190);
        let gate = fixture.gate(&fixture.0.batching);
        let stale = Arc::new(AtomicBool::new(false));
        let canceled = Arc::new(AtomicBool::new(false));
        thread::scope(|scope| {
            let _cancel = CancelOnDrop(canceled.clone());
            let _release = ReleaseGate(gate.clone());
            let pool_ref = &pool;
            let node = fixture.0.node.clone();
            let signal = canceled.clone();
            let identity = stale.clone();
            let (sent, received) = mpsc::sync_channel(1);
            let worker = scope.spawn(move || {
                let mut backend = pool_ref.backend_for(Uuid::new_v4(), signal).unwrap();
                let device = backend.open(&node).unwrap();
                let owner = thread::current().id();
                let validate = Rc::new(move || {
                    assert_eq!(thread::current().id(), owner);
                    if identity.load(Ordering::Acquire) {
                        Err(CallerIdentityLost.into())
                    } else {
                        Ok(())
                    }
                });
                let lease = device
                    .begin_exchange_validated(ExchangeOptions::batch(), validate)
                    .unwrap();
                let mut batch = FeatureBatch::new();
                if let Some(second) = second {
                    batch.push_report(&[0x10, 1, 2, 3]).unwrap();
                    batch.push_report(&[second, 4, 5, 6]).unwrap();
                }
                let error = lease.transport().send_feature_batch(&batch).unwrap_err();
                let release = lease.finish().unwrap_err();
                assert!(release.downcast_ref::<CallerIdentityLost>().is_some());
                sent.send(error).unwrap();
            });
            gate.wait_entered();
            stale.store(true, Ordering::Release);
            assert!(matches!(
                received.try_recv(),
                Err(mpsc::TryRecvError::Empty)
            ));
            gate.release();
            let error = received.recv_timeout(Duration::from_secs(3)).unwrap();
            let failure = FeatureBatchFailure::from_error(&error).unwrap();
            match second {
                None | Some(0x20) => {
                    let reports = if second.is_some() { 2 } else { 0 };
                    let bytes = reports * 4;
                    assert_eq!(
                        native_io_outcome(&error),
                        Some(NativeIoOutcome::Returned { byte_count: bytes })
                    );
                    assert_eq!(failure.progress.completed_operations, reports);
                    assert_eq!(failure.progress.reports_submitted(), reports);
                    assert_eq!(failure.raw_return, Some(bytes as i32));
                    assert!(error.downcast_ref::<CallerIdentityLost>().is_some());
                }
                Some(second) => {
                    assert_eq!(native_io_outcome(&error), Some(NativeIoOutcome::Rejected));
                    assert_eq!(failure.progress.completed_operations, 1);
                    assert_eq!(failure.raw_return, None);
                    if second == 0xef {
                        assert!(error.downcast_ref::<ScriptedNativeFailure>().is_some());
                        assert_eq!(failure.progress.report_returns.len(), 1);
                    } else {
                        assert_eq!(failure.progress.report_returns.len(), 2);
                        assert_eq!(failure.progress.report_returns[1].returned_bytes, 2);
                    }
                }
            }
            assert!(format!("{error:#}").contains("caller identity changed"));
            worker.join().unwrap();
        });
        assert_eq!(
            fixture.observed().len(),
            if second.is_some() { 3 } else { 1 }
        );
        stop(&pool);
        assert_eq!(fixture.0.closed.load(Ordering::Acquire), 1);
    }
}

#[test]
fn cancellation_keeps_interrupt_terminal_counts_and_errors_without_publishing_the_read() {
    for (first_byte, returned) in [(0x33, Some(2)), (0xef, None), (0xfb, Some(5))] {
        let pool = pool();
        let fixture = Fixture::new(190);
        let gate = fixture.gate(&fixture.0.interrupt_reading);
        let canceled = Arc::new(AtomicBool::new(false));
        thread::scope(|scope| {
            let _cancel = CancelOnDrop(canceled.clone());
            let _release = ReleaseGate(gate.clone());
            let pool_ref = &pool;
            let node = fixture.0.node.clone();
            let signal = canceled.clone();
            let (sent, received) = mpsc::sync_channel(1);
            let worker = scope.spawn(move || {
                let mut backend = pool_ref.backend_for(Uuid::new_v4(), signal).unwrap();
                let interrupt = backend.open_interrupt(&node).unwrap();
                let mut report = [first_byte; 4];
                let error = interrupt.read(&mut report, 1).unwrap_err();
                sent.send((error, report)).unwrap();
            });
            gate.wait_entered();
            canceled.store(true, Ordering::Release);
            assert!(matches!(
                received.try_recv(),
                Err(mpsc::TryRecvError::Empty)
            ));
            gate.release();
            let (error, report) = received.recv_timeout(Duration::from_secs(3)).unwrap();
            assert_eq!(
                native_io_outcome(&error),
                returned.map(|byte_count| NativeIoOutcome::Returned { byte_count })
            );
            if first_byte == 0xef {
                assert!(error.downcast_ref::<ScriptedNativeFailure>().is_some());
            }
            assert_eq!(
                native_read_bytes(&error),
                if first_byte == 0x33 {
                    Some([0x81, 0x53].as_slice())
                } else {
                    None
                }
            );
            assert!(format!("{error:#}").contains("cancel"));
            assert_eq!(report, [first_byte; 4]);
            worker.join().unwrap();
        });
        stop(&pool);
    }
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
fn packets_canceled_before_actor_dispatch_are_not_native_attempts() {
    for batched in [false, true] {
        let pool = pool();
        let fixture = Fixture::new(190);
        let entry = pool.entry(&fixture.0.node).unwrap();
        entry
            .wait_ready_validated(&AtomicBool::new(false), &no_validation)
            .unwrap();
        let canceled = Arc::new(AtomicBool::new(false));
        let _cancel = CancelOnDrop(canceled.clone());
        let lease = entry
            .acquire(
                Uuid::new_v4(),
                canceled.clone(),
                ExchangeOptions::batch(),
                &no_validation,
            )
            .unwrap();
        let mut batch = FeatureBatch::new();
        batch.push_report(&[0x10, 1, 2, 3]).unwrap();
        let (response, receipt) = mpsc::sync_channel(1);
        // Hold admission closed until cancellation is visible. The request is
        // already in the actor queue, but cannot reach the native boundary.
        let admission = entry.state.lock().unwrap();
        lease
            .sender
            .send(ActorRequest {
                signature: lease.control.signature,
                operation: ActorOperation::Packet(if batched {
                    Packet::Batch(batch.clone())
                } else {
                    Packet::Send {
                        bytes: vec![0x10, 1, 2, 3],
                        collection_max: false,
                    }
                }),
                response,
            })
            .unwrap();
        canceled.store(true, Ordering::Release);
        drop(admission);
        let error = receipt
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .err()
            .expect("canceled packet must not publish success");
        assert_eq!(native_io_outcome(&error), Some(NativeIoOutcome::NotCalled));
        if batched {
            let mut attempted = Vec::new();
            observe_feature_batch(&batch, &Err(error), |report| {
                attempted.push(report.to_vec())
            });
            assert!(attempted.is_empty());
        } else {
            assert!(!native_io_attempted(&Err(error)));
        }
        assert!(fixture.observed().is_empty());
        drop(lease);
        stop(&pool);
    }
}

#[test]
fn preadmission_cancellation_and_caller_validation_keep_not_called_and_typed_causes() {
    let pool = pool();
    let fixture = Fixture::new(190);
    let device = open(&pool, &fixture.0.node);
    let rejected = Rc::new(std::cell::Cell::new(false));
    let validation_state = rejected.clone();
    let validation = Rc::new(move || {
        if validation_state.get() {
            Err(ScriptedNativeFailure.into())
        } else {
            Ok(())
        }
    });
    let lease = device
        .begin_exchange_validated(ExchangeOptions::batch(), validation)
        .unwrap();
    rejected.set(true);
    let error = lease.transport().send_feature_counted(&[0x10]).unwrap_err();
    assert_eq!(native_io_outcome(&error), Some(NativeIoOutcome::NotCalled));
    assert!(error.downcast_ref::<ScriptedNativeFailure>().is_some());
    assert!(!native_io_attempted(&Err(error)));
    let mut batch = FeatureBatch::new();
    batch.push_report(&[0x10]).unwrap();
    let result = lease.transport().send_feature_batch(&batch);
    let error = result.as_ref().unwrap_err();
    assert_eq!(native_io_outcome(error), Some(NativeIoOutcome::NotCalled));
    assert!(error.downcast_ref::<ScriptedNativeFailure>().is_some());
    observe_feature_batch(&batch, &result, |_| {
        panic!("failed preflight is not a write attempt")
    });
    drop(lease);

    let entry = pool.entry(&fixture.0.node).unwrap();
    let canceled = Arc::new(AtomicBool::new(false));
    let lease = entry
        .acquire(
            Uuid::new_v4(),
            canceled.clone(),
            ExchangeOptions::plain(),
            &no_validation,
        )
        .unwrap();
    canceled.store(true, Ordering::Release);
    let error = lease
        .call(ActorOperation::Packet(Packet::Send {
            bytes: vec![0x10],
            collection_max: false,
        }))
        .err()
        .expect("canceled lease must not admit native work");
    assert_eq!(native_io_outcome(&error), Some(NativeIoOutcome::NotCalled));
    assert!(!native_io_attempted(&Err(error)));
    assert!(fixture.observed().is_empty());
    drop(lease);
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
fn queued_release_classifies_this_cycle_before_consuming_reused_guard_errors() {
    for retry in [false, true] {
        for after_release in [false, true] {
            let pool = pool();
            let fixture = Fixture::new(190);
            let device = open(&pool, &fixture.0.node);
            let calls = Arc::new(AtomicUsize::new(0));
            let signal = calls.clone();
            let validate = Rc::new(move || {
                let current = signal.load(Ordering::Acquire);
                if current != 0 {
                    signal.fetch_add(1, Ordering::AcqRel);
                    if current >= if after_release { 2 } else { 1 } {
                        return validate_exchange_release(Ok(()), Err(CallerIdentityLost.into()));
                    }
                }
                Ok(())
            });
            let lease = device
                .begin_exchange_validated(ExchangeOptions::plain(), validate)
                .unwrap();
            calls.store(1, Ordering::Release);
            let error = if retry {
                let error = lease.retry_release().unwrap_err();
                drop(lease);
                error
            } else {
                lease.finish().unwrap_err()
            };
            assert!(error.downcast_ref::<CallerIdentityLost>().is_some());
            assert_eq!(exchange_release_completed(&error), after_release);
            assert!(fixture.observed().is_empty());
            let recovery = open(&pool, &fixture.0.node);
            recovery
                .begin_exchange(ExchangeOptions::plain())
                .unwrap()
                .finish()
                .unwrap();
            drop((recovery, device));
            stop(&pool);
        }
    }
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
                packet_result,
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
        let packet_error = packet_result.unwrap_err();
        assert_eq!(
            native_io_outcome(&packet_error),
            Some(NativeIoOutcome::Returned { byte_count: 1 })
        );
        assert!(
            format!("{packet_error:#}").contains("domain operation canceled after native dispatch")
        );
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
fn rejected_packet_publication_retains_the_actors_actual_read_bytes() {
    use razer_device::backend::native_read_bytes;
    for count in [0, 2, 4, 5] {
        let error = combine_packet(
            Ok(PacketReply::Bytes {
                bytes: vec![11, 22, 33, 44],
                count,
            }),
            Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "old actor epoch").into()),
        )
        .err()
        .unwrap();
        assert_eq!(
            native_io_outcome(&error),
            Some(NativeIoOutcome::Returned { byte_count: count })
        );
        assert_eq!(native_read_bytes(&error), [11, 22, 33, 44].get(..count));
        assert!(error.downcast_ref::<std::io::Error>().is_some());
        let nested = combine_packet(Err(error), Err(anyhow::anyhow!("caller canceled")))
            .err()
            .unwrap();
        assert_eq!(native_read_bytes(&nested), [11, 22, 33, 44].get(..count));
        assert!(format!("{nested:#}").contains("caller canceled"));
    }
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
            let error = handle.send_feature(&[0x10]).unwrap_err();
            assert_eq!(
                native_io_outcome(&error),
                Some(NativeIoOutcome::Returned { byte_count: 1 })
            );
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
        let error = received.recv().unwrap().err().unwrap();
        assert_eq!(native_io_outcome(&error), Some(NativeIoOutcome::NotCalled));
        assert!(!native_io_attempted(&Err(error)));
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
