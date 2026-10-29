//! Collection actors retain native handles, command spacing and queued leases.
use super::clock::{NativeReturn, ReleaseReason, SourceClock};
use anyhow::{Context as _, ensure};
use razer_device::backend::{
    ExchangeLease, ExchangeOptions, ExchangeTiming, ExchangeValidation, FeatureTransport,
    HidBackend, HidNode, InterruptTransport, Protocol25Topology, ReportLengths, SourceCommandScope,
};
use razer_platform::device_changes::ExactDeviceChangeGate;
use serde_json::Value;
use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    rc::Rc,
    sync::{
        Arc, Condvar, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use uuid::Uuid;

const QUEUE_CAPACITY: usize = 64;
const PACKET_CAPACITY: usize = 8;
const CONTROL_POLL: Duration = Duration::from_millis(10);
const MAX_BUFFER_BYTES: usize = 65_536;

pub(super) type RawFactory = fn() -> anyhow::Result<Box<dyn HidBackend>>;
pub(super) type IdentityResolver = fn(&HidNode) -> anyhow::Result<CollectionIdentity>;

#[derive(Clone)]
enum TopologyGate {
    Exact(ExactDeviceChangeGate),
    #[cfg(test)]
    Scripted(Arc<std::sync::atomic::AtomicU64>),
}

impl TopologyGate {
    fn current_revision(&self) -> Option<u64> {
        match self {
            Self::Exact(gate) => gate.current_revision(),
            #[cfg(test)]
            Self::Scripted(revision) => {
                let revision = revision.load(Ordering::Acquire);
                (revision != 0).then_some(revision)
            }
        }
    }

    fn capture(&self) -> anyhow::Result<TopologyRevision> {
        Ok(TopologyRevision {
            gate: self.clone(),
            revision: self
                .current_revision()
                .context("Exact HID topology is not reconciled")?,
        })
    }
}

#[derive(Clone)]
struct TopologyRevision {
    gate: TopologyGate,
    revision: u64,
}

impl TopologyRevision {
    fn is_current(&self) -> bool {
        match &self.gate {
            TopologyGate::Exact(gate) => gate.is_current(self.revision),
            #[cfg(test)]
            TopologyGate::Scripted(_) => self.gate.current_revision() == Some(self.revision),
        }
    }

    fn validate(&self) -> anyhow::Result<()> {
        ensure!(self.is_current(), "Exact HID topology revision changed");
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum PlatformIdentity {
    Portable,
    Windows {
        container_id: String,
        hid_instance_id: String,
        parent_instance_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CollectionIdentity {
    pub(super) node: HidNode,
    pub(super) platform: PlatformIdentity,
}

impl CollectionIdentity {
    fn validate(&self, selected: &HidNode) -> anyhow::Result<()> {
        ensure!(
            self.node == *selected,
            "Resolved collection differs from the selected node"
        );
        ensure!(
            !self.node.path.is_empty() && !self.node.path.contains(&0),
            "Collection path is empty or contains NUL"
        );
        if let PlatformIdentity::Windows {
            container_id,
            hid_instance_id,
            parent_instance_id,
        } = &self.platform
        {
            ensure!(
                [container_id, hid_instance_id, parent_instance_id]
                    .iter()
                    .all(|value| !value.is_empty() && !value.contains('\0')),
                "Windows collection identity is incomplete"
            );
        }
        Ok(())
    }

    fn path_key(&self) -> Vec<u8> {
        match self.platform {
            PlatformIdentity::Portable => self.node.path.clone(),
            PlatformIdentity::Windows { .. } => self.node.path.to_ascii_lowercase(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum CommandEndpoint {
    WindowsContainer(String),
    PortablePath(Vec<u8>),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum CommandKey {
    Protocol25Normal {
        endpoint: CommandEndpoint,
        product_id: u16,
    },
    Protocol25DualLink {
        endpoint: CommandEndpoint,
    },
}

#[derive(Default)]
struct CommandRegistry(Mutex<BTreeMap<CommandKey, Weak<CommandGate>>>);

#[derive(Default)]
struct CommandGate {
    state: Mutex<CommandState>,
    changed: Condvar,
}

#[derive(Default)]
struct CommandState {
    holder: Option<Uuid>,
    queued: VecDeque<Uuid>,
}

impl CommandRegistry {
    fn acquire(
        &self,
        entry: &Arc<Entry>,
        canceled: Arc<AtomicBool>,
        options: &ExchangeOptions,
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Option<CommandLease>> {
        validate()?;
        let Some(scope) = options.source_scope else {
            return Ok(None);
        };
        let endpoint = match &entry.identity.platform {
            PlatformIdentity::Windows { container_id, .. } => {
                CommandEndpoint::WindowsContainer(container_id.to_ascii_lowercase())
            }
            PlatformIdentity::Portable => {
                CommandEndpoint::PortablePath(entry.identity.node.path.clone())
            }
        };
        let key = match scope {
            SourceCommandScope::Protocol25 {
                topology: Protocol25Topology::Normal,
            } => CommandKey::Protocol25Normal {
                endpoint,
                product_id: entry.identity.node.product_id,
            },
            SourceCommandScope::Protocol25 {
                topology: Protocol25Topology::DualLink,
            } => CommandKey::Protocol25DualLink { endpoint },
        };
        let gate = {
            let mut gates = self.0.lock().map_err(poison)?;
            gates.retain(|_, gate| gate.strong_count() > 0);
            match gates.get(&key).and_then(Weak::upgrade) {
                Some(gate) => gate,
                None => {
                    let gate = Arc::new(CommandGate::default());
                    gates.insert(key, Arc::downgrade(&gate));
                    gate
                }
            }
        };
        let id = Uuid::new_v4();
        {
            let mut state = gate.state.lock().map_err(poison)?;
            ensure!(
                state.queued.len() < QUEUE_CAPACITY,
                "Source command queue is full"
            );
            state.queued.push_back(id);
            gate.changed.notify_all();
        }
        let lease = CommandLease { gate, id };
        loop {
            validate()?;
            ensure!(
                !canceled.load(Ordering::Acquire) && !options.is_canceled() && entry.admitting()?,
                "Source command admission was canceled or invalidated"
            );
            let mut state = lease.gate.state.lock().map_err(poison)?;
            if state.holder.is_none() && state.queued.front() == Some(&id) {
                state.queued.pop_front();
                state.holder = Some(id);
                drop(state);
                return Ok(Some(lease));
            }
            let waited = lease
                .gate
                .changed
                .wait_timeout(state, CONTROL_POLL)
                .map_err(poison)?;
            drop(waited);
        }
    }
}

struct CommandLease {
    gate: Arc<CommandGate>,
    id: Uuid,
}

impl Drop for CommandLease {
    fn drop(&mut self) {
        if let Ok(mut state) = self.gate.state.lock() {
            state.queued.retain(|queued| *queued != self.id);
            if state.holder == Some(self.id) {
                state.holder = None;
            }
            self.gate.changed.notify_all();
        }
    }
}

#[derive(Clone)]
pub(super) struct Pool {
    inner: Arc<PoolInner>,
}

struct PoolInner {
    raw_factory: RawFactory,
    resolver: IdentityResolver,
    state: Mutex<PoolState>,
    commands: Arc<CommandRegistry>,
}

#[derive(Default)]
struct PoolState {
    entries: BTreeMap<Vec<u8>, Arc<Entry>>,
    topology_gate: Option<TopologyGate>,
    generation: Uuid,
    uncertain: bool,
    stopped: bool,
}

impl Pool {
    pub(super) fn new(raw_factory: RawFactory, resolver: IdentityResolver) -> Self {
        Self {
            inner: Arc::new(PoolInner {
                raw_factory,
                resolver,
                state: Mutex::new(PoolState {
                    generation: Uuid::new_v4(),
                    uncertain: true,
                    ..PoolState::default()
                }),
                commands: Arc::new(CommandRegistry::default()),
            }),
        }
    }

    pub(super) fn backend_for(
        &self,
        owner: Uuid,
        canceled: Arc<AtomicBool>,
    ) -> anyhow::Result<Box<dyn HidBackend>> {
        ensure!(!owner.is_nil(), "Collection owner UUID is nil");
        Ok(Box::new(PooledBackend {
            pool: self.clone(),
            owner,
            canceled,
            inventory: None,
        }))
    }

    pub(super) fn set_topology_gate(&self, gate: ExactDeviceChangeGate) -> anyhow::Result<()> {
        self.install_topology_gate(TopologyGate::Exact(gate))
    }

    fn install_topology_gate(&self, gate: TopologyGate) -> anyhow::Result<()> {
        let mut state = self.inner.state.lock().map_err(poison)?;
        ensure!(!state.stopped, "Collection pool is stopping");
        state.uncertain = true;
        state.generation = Uuid::new_v4();
        for entry in state.entries.values() {
            entry.invalidate()?;
        }
        state.topology_gate = Some(gate);
        Ok(())
    }

    fn capture_topology(&self) -> anyhow::Result<TopologyRevision> {
        let state = self.inner.state.lock().map_err(poison)?;
        ensure!(
            !state.stopped && !state.uncertain,
            "Collection topology is not admitting work"
        );
        state
            .topology_gate
            .as_ref()
            .context("Exact HID topology owner is not installed")?
            .capture()
    }

    #[cfg(test)]
    fn entry(&self, node: &HidNode) -> anyhow::Result<Arc<Entry>> {
        self.entry_validated(node, &no_validation)
    }

    fn entry_validated(
        &self,
        node: &HidNode,
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Arc<Entry>> {
        validate()?;
        let (generation, topology) = {
            let state = self.inner.state.lock().map_err(poison)?;
            ensure!(
                !state.stopped && !state.uncertain,
                "Collection topology is not admitting work"
            );
            (
                state.generation,
                state
                    .topology_gate
                    .as_ref()
                    .context("Exact HID topology owner is not installed")?
                    .capture()?,
            )
        };
        let identity = (self.inner.resolver)(node)?;
        validate()?;
        identity.validate(node)?;
        let key = identity.path_key();
        let mut state = self.inner.state.lock().map_err(poison)?;
        ensure!(!state.stopped, "Collection pool is stopping");
        ensure!(
            !state.uncertain,
            "Collection topology requires reconciliation"
        );
        ensure!(
            state.generation == generation,
            "Collection topology changed during identity resolution"
        );
        topology.validate()?;
        if let Some(entry) = state.entries.get(&key) {
            if entry.identity != identity {
                entry.invalidate()?;
                anyhow::bail!(
                    "Collection identity changed; the previous actor must terminate first"
                );
            }
            ensure!(
                entry.admitting()?,
                "Collection actor is invalidated or quarantined"
            );
            return Ok(entry.clone());
        }
        let entry = Entry::start(
            identity,
            topology,
            self.inner.raw_factory,
            self.inner.resolver,
            self.inner.commands.clone(),
        )?;
        state.entries.insert(key, entry.clone());
        Ok(entry)
    }

    pub(super) fn invalidate_path(&self, path: &[u8]) -> anyhow::Result<()> {
        let mut state = self.inner.state.lock().map_err(poison)?;
        state.generation = Uuid::new_v4();
        for entry in state.entries.values() {
            let matches = match entry.identity.platform {
                PlatformIdentity::Portable => entry.identity.node.path == path,
                PlatformIdentity::Windows { .. } => {
                    entry.identity.node.path.eq_ignore_ascii_case(path)
                }
            };
            if matches {
                entry.invalidate()?;
            }
        }
        Ok(())
    }

    pub(super) fn invalidate_all(&self) -> anyhow::Result<()> {
        let mut state = self.inner.state.lock().map_err(poison)?;
        state.uncertain = true;
        state.generation = Uuid::new_v4();
        for entry in state.entries.values() {
            entry.invalidate()?;
        }
        Ok(())
    }

    /// Only terminal actors are joined and removed. A blocked native call keeps
    /// its entry and its OS/file leases, preventing a same-path successor.
    pub(super) fn reconcile(&self) -> anyhow::Result<usize> {
        let entries: Vec<_> = self
            .inner
            .state
            .lock()
            .map_err(poison)?
            .entries
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        let mut removed = 0;
        let mut errors = Vec::new();
        for (key, entry) in entries {
            if let Some(error) = entry.join_finished()? {
                let mut state = self.inner.state.lock().map_err(poison)?;
                if state
                    .entries
                    .get(&key)
                    .is_some_and(|current| Arc::ptr_eq(current, &entry))
                {
                    state.entries.remove(&key);
                    removed += 1;
                }
                if !error.is_empty() {
                    errors.push(error);
                }
            }
        }
        ensure!(
            errors.is_empty(),
            "Collection actor cleanup failed: {}",
            errors.join("; ")
        );
        Ok(removed)
    }

    /// The topology owner calls this only after its exact-event barrier and
    /// current identity scan succeeded. Reaping alone does not prove topology.
    pub(super) fn resume_after_reconcile(&self) -> anyhow::Result<()> {
        let mut state = self.inner.state.lock().map_err(poison)?;
        ensure!(!state.stopped, "Collection pool is stopping");
        ensure!(
            state.topology_gate.is_some(),
            "Exact HID topology owner is not installed"
        );
        // Invalidated entries remain in the map until terminal join. They keep
        // blocking their paths while independently verified paths can reopen.
        state.uncertain = false;
        state.generation = Uuid::new_v4();
        Ok(())
    }

    pub(super) fn begin_shutdown(&self) -> anyhow::Result<()> {
        {
            let mut state = self.inner.state.lock().map_err(poison)?;
            state.stopped = true;
            for entry in state.entries.values() {
                entry.invalidate()?;
            }
        }
        Ok(())
    }

    /// Empty means every previously retained actor was actually joined by
    /// reconcile; merely closing admission never removes an entry.
    pub(super) fn pending_count(&self) -> anyhow::Result<usize> {
        Ok(self.inner.state.lock().map_err(poison)?.entries.len())
    }

    pub(super) fn is_drained(&self) -> anyhow::Result<bool> {
        Ok(self.pending_count()? == 0)
    }
}

impl Drop for PoolInner {
    fn drop(&mut self) {
        // This owner belongs to the broker's background lifetime. Never detach
        // a native actor merely because a client or the final pool was dropped.
        if let Ok(state) = self.state.get_mut() {
            for entry in state.entries.values() {
                let _ = entry.invalidate();
            }
            for entry in state.entries.values() {
                entry.join_terminal();
            }
        }
    }
}

struct PooledBackend {
    pool: Pool,
    owner: Uuid,
    canceled: Arc<AtomicBool>,
    inventory: Option<Box<dyn HidBackend>>,
}

impl HidBackend for PooledBackend {
    fn enumerate(&mut self) -> anyhow::Result<Vec<HidNode>> {
        ensure!(
            !self.canceled.load(Ordering::Acquire),
            "Collection owner was canceled"
        );
        let topology = self.pool.capture_topology()?;
        if self.inventory.is_none() {
            self.inventory = Some((self.pool.inner.raw_factory)()?);
        }
        topology.validate()?;
        let result = self
            .inventory
            .as_mut()
            .expect("retained inventory backend")
            .enumerate();
        combine(result, topology.validate())
    }

    fn open(&mut self, node: &HidNode) -> anyhow::Result<Box<dyn FeatureTransport>> {
        self.open_validated(node, Rc::new(no_validation))
    }

    fn open_validated(
        &mut self,
        node: &HidNode,
        validate: ExchangeValidation<'_>,
    ) -> anyhow::Result<Box<dyn FeatureTransport>> {
        ensure!(
            !self.canceled.load(Ordering::Acquire),
            "Collection owner was canceled"
        );
        let entry = self.pool.entry_validated(node, validate.as_ref())?;
        let info = entry.wait_ready_validated(&self.canceled, validate.as_ref())?;
        entry.validate_current()?;
        validate()?;
        Ok(Box::new(FeatureProxy {
            entry,
            info,
            owner: self.owner,
            canceled: self.canceled.clone(),
        }))
    }

    fn open_interrupt(&mut self, node: &HidNode) -> anyhow::Result<Box<dyn InterruptTransport>> {
        self.open_interrupt_validated(node, Rc::new(no_validation))
    }

    fn open_interrupt_validated(
        &mut self,
        node: &HidNode,
        validate: ExchangeValidation<'_>,
    ) -> anyhow::Result<Box<dyn InterruptTransport>> {
        ensure!(
            !self.canceled.load(Ordering::Acquire),
            "Collection owner was canceled"
        );
        let entry = self.pool.entry_validated(node, validate.as_ref())?;
        entry.wait_ready_validated(&self.canceled, validate.as_ref())?;
        let interrupt = entry.interrupt()?;
        let lengths = interrupt.wait_ready(&self.canceled, validate.as_ref())?;
        entry.validate_current()?;
        validate()?;
        Ok(Box::new(InterruptProxy {
            entry,
            interrupt,
            lengths,
            owner: self.owner,
            canceled: self.canceled.clone(),
        }))
    }
}

#[derive(Clone)]
struct OpenInfo {
    node: HidNode,
    lengths: ReportLengths,
    metadata: Value,
}

struct Entry {
    identity: CollectionIdentity,
    topology: TopologyRevision,
    epoch: Uuid,
    raw_factory: RawFactory,
    resolver: IdentityResolver,
    commands: Arc<CommandRegistry>,
    state: Mutex<EntryState>,
    changed: Condvar,
    worker: Mutex<Option<JoinHandle<()>>>,
    interrupt: Mutex<Option<Arc<InterruptOwner>>>,
}

struct EntryState {
    admitting: bool,
    ready: Option<Result<OpenInfo, String>>,
    queued: VecDeque<PendingLease>,
    active: Option<Arc<LeaseControl>>,
    terminal: bool,
}

impl Entry {
    fn start(
        identity: CollectionIdentity,
        topology: TopologyRevision,
        raw_factory: RawFactory,
        resolver: IdentityResolver,
        commands: Arc<CommandRegistry>,
    ) -> anyhow::Result<Arc<Self>> {
        let entry = Arc::new(Self {
            identity,
            topology,
            epoch: Uuid::new_v4(),
            raw_factory,
            resolver,
            commands,
            state: Mutex::new(EntryState {
                admitting: true,
                ready: None,
                queued: VecDeque::new(),
                active: None,
                terminal: false,
            }),
            changed: Condvar::new(),
            worker: Mutex::new(None),
            interrupt: Mutex::new(None),
        });
        let actor = entry.clone();
        let worker = thread::Builder::new()
            .name("hid-collection".into())
            .spawn(move || actor.run())
            .context("Create collection actor")?;
        *entry.worker.lock().map_err(poison)? = Some(worker);
        Ok(entry)
    }

    fn admitting(&self) -> anyhow::Result<bool> {
        Ok(self.topology.is_current() && self.state.lock().map_err(poison)?.admitting)
    }

    fn validate_current(&self) -> anyhow::Result<()> {
        ensure!(
            self.admitting()?,
            "Collection epoch or exact topology revision is invalid"
        );
        Ok(())
    }

    fn invalidate(&self) -> anyhow::Result<()> {
        let mut state = self.state.lock().map_err(poison)?;
        state.admitting = false;
        for pending in state.queued.drain(..) {
            pending.control.forced.store(true, Ordering::Release);
            let _ = pending
                .granted
                .send(Err("Collection epoch was invalidated while queued".into()));
        }
        if let Some(active) = &state.active {
            active.forced.store(true, Ordering::Release);
        }
        self.changed.notify_all();
        drop(state);
        if let Some(interrupt) = self.interrupt.lock().map_err(poison)?.as_ref() {
            interrupt.stopped.store(true, Ordering::Release);
        }
        Ok(())
    }

    fn wait_ready_validated(
        &self,
        canceled: &AtomicBool,
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<OpenInfo> {
        loop {
            validate()?;
            let state = self.state.lock().map_err(poison)?;
            if let Some(Err(error)) = &state.ready {
                return Err(anyhow::Error::msg(error.clone()));
            }
            ensure!(
                state.admitting && self.topology.is_current(),
                "Collection epoch was invalidated during open"
            );
            ensure!(
                !canceled.load(Ordering::Acquire),
                "Collection open was canceled"
            );
            if let Some(result) = &state.ready {
                return result.clone().map_err(anyhow::Error::msg);
            }
            let waited = self
                .changed
                .wait_timeout(state, CONTROL_POLL)
                .map_err(poison)?;
            drop(waited);
        }
    }

    fn acquire(
        self: &Arc<Self>,
        owner: Uuid,
        canceled: Arc<AtomicBool>,
        options: ExchangeOptions,
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<ActiveLease> {
        validate()?;
        let (sender, inbox) = mpsc::sync_channel(PACKET_CAPACITY);
        let (granted, receipt) = mpsc::sync_channel(1);
        let control = Arc::new(LeaseControl {
            signature: Signature {
                owner,
                request: Uuid::new_v4(),
                epoch: self.epoch,
            },
            canceled,
            options,
            forced: AtomicBool::new(false),
        });
        ensure!(
            !control.canceled(),
            "HID exchange was canceled before admission"
        );
        {
            let mut state = self.state.lock().map_err(poison)?;
            ensure!(
                state.admitting && self.topology.is_current(),
                "Collection epoch is invalidated"
            );
            ensure!(
                state.queued.len() < QUEUE_CAPACITY,
                "Collection exchange queue is full"
            );
            state.queued.push_back(PendingLease {
                control: control.clone(),
                inbox,
                granted,
            });
            self.changed.notify_one();
        }
        let mut admission = PendingAdmission {
            entry: self.clone(),
            control: control.clone(),
            granted: false,
        };
        loop {
            validate()?;
            ensure!(
                !control.canceled() && self.admitting()?,
                "Queued HID exchange was canceled"
            );
            match receipt.recv_timeout(CONTROL_POLL) {
                Ok(result) => {
                    result.map_err(anyhow::Error::msg)?;
                    validate()?;
                    ensure!(
                        !control.canceled() && self.admitting()?,
                        "HID exchange was canceled during admission"
                    );
                    admission.granted = true;
                    return Ok(ActiveLease {
                        entry: self.clone(),
                        control,
                        sender,
                        finished: false,
                    });
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    anyhow::bail!("Collection actor closed during admission")
                }
            }
        }
    }

    fn run(self: Arc<Self>) {
        let _terminal = Terminal(self.clone());
        let opened = (|| {
            self.validate_current()?;
            let mut backend = (self.raw_factory)()?;
            self.validate_current()?;
            let native = backend.open(&self.identity.node)?;
            self.validate_current()?;
            ensure!(
                native.hid_node() == Some(&self.identity.node),
                "Native handle differs from its collection identity"
            );
            ensure!(
                (self.resolver)(&self.identity.node)? == self.identity,
                "OS collection identity changed during native open"
            );
            self.validate_current()?;
            let info = OpenInfo {
                node: self.identity.node.clone(),
                lengths: native.report_lengths()?,
                metadata: native.metadata(),
            };
            self.validate_current()?;
            Ok::<_, anyhow::Error>((backend, native, info))
        })();
        let (_backend, native, info) = match opened {
            Ok(opened) => opened,
            Err(error) => {
                if let Ok(mut state) = self.state.lock() {
                    state.ready = Some(Err(format!("{error:#}")));
                    state.admitting = false;
                    self.changed.notify_all();
                }
                return;
            }
        };
        if let Ok(mut state) = self.state.lock() {
            state.ready = Some(Ok(info));
            self.changed.notify_all();
        } else {
            return;
        }
        let origin = Instant::now();
        let mut clock = SourceClock::default();
        loop {
            let pending = {
                let Ok(mut state) = self.state.lock() else { return };
                loop {
                    if !state.admitting || !self.topology.is_current() {
                        return;
                    }
                    if let Some(pending) = state.queued.pop_front() {
                        state.active = Some(pending.control.clone());
                        break pending;
                    }
                    state = match self.changed.wait_timeout(state, CONTROL_POLL) {
                        Ok((state, _)) => state,
                        Err(_) => return,
                    };
                }
            };
            if pending.control.canceled() || pending.granted.send(Ok(())).is_err() {
                pending.control.forced.store(true, Ordering::Release);
            } else {
                self.run_lease(native.as_ref(), &mut clock, origin, &pending);
            }
            if let Ok(mut state) = self.state.lock() {
                state.active = None;
                self.changed.notify_all();
            }
        }
    }

    fn run_lease(
        &self,
        native: &dyn FeatureTransport,
        clock: &mut SourceClock,
        origin: Instant,
        pending: &PendingLease,
    ) {
        let mut progress = LeaseProgress::default();
        loop {
            if pending.control.canceled() || !self.admitting().unwrap_or(false) {
                return;
            }
            let request = match pending.inbox.recv_timeout(CONTROL_POLL) {
                Ok(request) => request,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return,
            };
            if request.signature != pending.control.signature
                || request.signature.epoch != self.epoch
                || request.signature.owner.is_nil()
                || request.signature.request.is_nil()
            {
                request.reply(Err(
                    "Collection operation has a stale or foreign identity".into()
                ));
                continue;
            }
            match request.operation {
                ActorOperation::Finish => {
                    let result = if pending.control.canceled() || !self.admitting().unwrap_or(false)
                    {
                        Err("HID exchange cannot confirm release after cancellation or epoch invalidation".into())
                    } else {
                        progress.finish_batch(
                            clock,
                            origin.elapsed(),
                            pending.control.options.timing,
                        );
                        let reason = match pending.control.options.timing {
                            ExchangeTiming::MutexOut { .. } => ReleaseReason::ExplicitMatched,
                            ExchangeTiming::Plain | ExchangeTiming::Batch => {
                                ReleaseReason::BatchFinally
                            }
                        };
                        clock.release(origin.elapsed(), reason);
                        Ok(PacketReply::Unit)
                    };
                    let _ = request.response.send(result);
                    return;
                }
                ActorOperation::Packet(operation) => {
                    let result = self
                        .execute(
                            native,
                            clock,
                            origin,
                            &pending.control,
                            &mut progress,
                            operation,
                        )
                        .map_err(|error| format!("{error:#}"));
                    let _ = request.response.send(result);
                }
            }
        }
    }

    fn execute(
        &self,
        native: &dyn FeatureTransport,
        clock: &mut SourceClock,
        origin: Instant,
        control: &LeaseControl,
        progress: &mut LeaseProgress,
        operation: Packet,
    ) -> anyhow::Result<PacketReply> {
        ensure!(
            !control.canceled() && self.admitting()?,
            "Collection operation was canceled before native dispatch"
        );
        let sending_feature = matches!(operation, Packet::Send { .. });
        if sending_feature {
            let interval = progress.out_interval(control.options.timing);
            if let Some(interval) = interval {
                while let Some(wait) = clock.out_wait(origin.elapsed(), interval) {
                    ensure!(
                        !control.canceled() && self.admitting()?,
                        "Collection OUT wait was canceled"
                    );
                    thread::sleep(wait);
                }
            }
        }
        ensure!(
            !control.canceled() && self.admitting()?,
            "Collection operation was canceled before native dispatch"
        );
        progress.dispatched(control.options.timing, sending_feature);
        let result = operation.execute(
            native,
            clock,
            origin,
            control.options.timing != ExchangeTiming::Batch,
        );
        progress.returned(control.options.timing, result.is_ok());
        // Cancellation is observed after the actual native terminal return.
        // The actor, handle and owned buffer survive any blocked native call.
        let current = (|| {
            ensure!(
                !control.canceled() && self.admitting()?,
                "Collection operation ended after cancellation or epoch invalidation"
            );
            Ok(())
        })();
        combine(result, current)
    }

    fn interrupt(self: &Arc<Self>) -> anyhow::Result<Arc<InterruptOwner>> {
        let mut interrupt = self.interrupt.lock().map_err(poison)?;
        ensure!(self.admitting()?, "Collection epoch is invalidated");
        if interrupt.is_none() {
            *interrupt = Some(InterruptOwner::start(self.clone())?);
        }
        Ok(interrupt
            .as_ref()
            .expect("retained interrupt actor")
            .clone())
    }

    fn join_finished(&self) -> anyhow::Result<Option<String>> {
        let mut worker = self.worker.lock().map_err(poison)?;
        if worker.as_ref().is_some_and(|worker| !worker.is_finished()) {
            return Ok(None);
        }
        let interrupt = self.interrupt.lock().map_err(poison)?;
        if let Some(interrupt) = interrupt.as_ref() {
            if !interrupt.finished()? {
                return Ok(None);
            }
        }
        let mut errors = Vec::new();
        if let Some(worker) = worker.take() {
            if worker.join().is_err() {
                errors.push("Feature collection actor panicked");
            }
        }
        if let Some(interrupt) = interrupt.as_ref() {
            if interrupt.join().is_err() {
                errors.push("Interrupt collection actor panicked");
            }
        }
        Ok(Some(errors.join("; ")))
    }

    fn join_terminal(&self) {
        if let Ok(mut worker) = self.worker.lock() {
            if let Some(worker) = worker.take() {
                let _ = worker.join();
            }
        }
        if let Ok(interrupt) = self.interrupt.lock() {
            if let Some(interrupt) = interrupt.as_ref() {
                let _ = interrupt.join();
            }
        }
    }
}

struct Terminal(Arc<Entry>);

impl Drop for Terminal {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0.state.lock() {
            state.admitting = false;
            state.terminal = true;
            if state.ready.is_none() {
                state.ready = Some(Err(
                    "Collection actor terminated before native open completed".into(),
                ));
            }
            for pending in state.queued.drain(..) {
                let _ = pending
                    .granted
                    .send(Err("Collection actor terminated while queued".into()));
            }
            self.0.changed.notify_all();
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Signature {
    owner: Uuid,
    request: Uuid,
    epoch: Uuid,
}

struct LeaseControl {
    signature: Signature,
    canceled: Arc<AtomicBool>,
    options: ExchangeOptions,
    forced: AtomicBool,
}

impl LeaseControl {
    fn canceled(&self) -> bool {
        self.forced.load(Ordering::Acquire)
            || self.canceled.load(Ordering::Acquire)
            || self.options.is_canceled()
    }
}

struct PendingLease {
    control: Arc<LeaseControl>,
    inbox: Receiver<ActorRequest>,
    granted: SyncSender<Result<(), String>>,
}

struct PendingAdmission {
    entry: Arc<Entry>,
    control: Arc<LeaseControl>,
    granted: bool,
}

impl Drop for PendingAdmission {
    fn drop(&mut self) {
        if self.granted {
            return;
        }
        self.control.forced.store(true, Ordering::Release);
        if let Ok(mut state) = self.entry.state.lock() {
            state
                .queued
                .retain(|pending| pending.control.signature != self.control.signature);
            self.entry.changed.notify_all();
        }
    }
}

struct ActorRequest {
    signature: Signature,
    operation: ActorOperation,
    response: SyncSender<Result<PacketReply, String>>,
}

impl ActorRequest {
    fn reply(self, result: Result<PacketReply, String>) {
        let _ = self.response.send(result);
    }
}

enum ActorOperation {
    Packet(Packet),
    Finish,
}

#[derive(Default)]
struct LeaseProgress {
    batch_started: bool,
    batch_failed: bool,
}

impl LeaseProgress {
    fn out_interval(&self, timing: ExchangeTiming) -> Option<Duration> {
        match timing {
            ExchangeTiming::Plain => None,
            ExchangeTiming::MutexOut { interval } => Some(interval),
            ExchangeTiming::Batch if !self.batch_started => Some(Duration::from_millis(2)),
            ExchangeTiming::Batch => None,
        }
    }

    fn dispatched(&mut self, timing: ExchangeTiming, sending_feature: bool) {
        if timing == ExchangeTiming::Batch && sending_feature {
            self.batch_started = true;
        }
    }

    fn returned(&mut self, timing: ExchangeTiming, succeeded: bool) {
        if timing == ExchangeTiming::Batch && !succeeded {
            self.batch_failed = true;
        }
    }

    fn finish_batch(&self, clock: &mut SourceClock, now: Duration, timing: ExchangeTiming) {
        if timing == ExchangeTiming::Batch && self.batch_started && !self.batch_failed {
            clock.native_return(now, NativeReturn::Batch);
        }
    }
}

enum Packet {
    Send {
        bytes: Vec<u8>,
        collection_max: bool,
    },
    Get {
        bytes: Vec<u8>,
        collection_max: bool,
    },
    Output(Vec<u8>),
    CountedOutput(Vec<u8>),
    Input(Vec<u8>),
    Interrupt {
        bytes: Vec<u8>,
        timeout_ms: u32,
    },
}

enum PacketReply {
    Unit,
    Count(usize),
    Bytes { bytes: Vec<u8>, count: usize },
}

impl Packet {
    fn execute(
        self,
        native: &dyn FeatureTransport,
        clock: &mut SourceClock,
        origin: Instant,
        update_clock: bool,
    ) -> anyhow::Result<PacketReply> {
        match self {
            Self::Send {
                bytes,
                collection_max,
            } => {
                let result = if collection_max {
                    native.send_feature_at_collection_max(&bytes)
                } else {
                    native.send_feature(&bytes)
                };
                if update_clock {
                    clock.native_return(
                        origin.elapsed(),
                        if result.is_ok() {
                            NativeReturn::Send
                        } else {
                            NativeReturn::Threw
                        },
                    );
                }
                result?;
                Ok(PacketReply::Unit)
            }
            Self::Get {
                mut bytes,
                collection_max,
            } => {
                let result = if collection_max {
                    native.get_feature_at_collection_max(&mut bytes)
                } else {
                    native.get_feature(&mut bytes)
                };
                if update_clock {
                    clock.native_return(
                        origin.elapsed(),
                        if result.is_ok() {
                            NativeReturn::Get
                        } else {
                            NativeReturn::Threw
                        },
                    );
                }
                let count = result?;
                ensure!(
                    count <= bytes.len(),
                    "Native Feature byte count exceeds the owned buffer"
                );
                Ok(PacketReply::Bytes { bytes, count })
            }
            Self::Output(bytes) => {
                native.write_output(&bytes)?;
                Ok(PacketReply::Unit)
            }
            Self::CountedOutput(bytes) => {
                native.write_output_counted(&bytes).map(PacketReply::Count)
            }
            Self::Input(mut bytes) => {
                let count = native.get_input(&mut bytes)?;
                ensure!(
                    count <= bytes.len(),
                    "Native Input byte count exceeds the owned buffer"
                );
                Ok(PacketReply::Bytes { bytes, count })
            }
            Self::Interrupt {
                mut bytes,
                timeout_ms,
            } => {
                let count = native.read_interrupt(&mut bytes, timeout_ms)?;
                ensure!(
                    count <= bytes.len(),
                    "Native interrupt byte count exceeds the owned buffer"
                );
                Ok(PacketReply::Bytes { bytes, count })
            }
        }
    }
}

struct ActiveLease {
    entry: Arc<Entry>,
    control: Arc<LeaseControl>,
    sender: SyncSender<ActorRequest>,
    finished: bool,
}

impl ActiveLease {
    fn call(&self, operation: ActorOperation) -> anyhow::Result<PacketReply> {
        ensure!(
            !self.control.canceled() && self.entry.admitting()?,
            "HID lease is canceled or stale"
        );
        let (response, receipt) = mpsc::sync_channel(1);
        let mut request = ActorRequest {
            signature: self.control.signature,
            operation,
            response,
        };
        loop {
            match self.sender.try_send(request) {
                Ok(()) => break,
                Err(TrySendError::Full(pending)) => {
                    request = pending;
                    ensure!(
                        !self.control.canceled() && self.entry.admitting()?,
                        "HID packet admission was canceled"
                    );
                    thread::sleep(CONTROL_POLL);
                }
                Err(TrySendError::Disconnected(_)) => {
                    anyhow::bail!("Native collection actor closed before packet admission")
                }
            }
        }
        // Once dispatched, cancellation cannot discard the native operation.
        // Wait for its terminal result; its buffers belong to the actor.
        let result = receipt
            .recv()
            .context("Native collection actor closed before terminal packet result")?
            .map_err(anyhow::Error::msg);
        let current = (|| {
            ensure!(
                !self.control.canceled(),
                "HID lease was canceled before consuming its terminal result"
            );
            self.entry.validate_current()
        })();
        combine(result, current)
    }

    fn finish(mut self) -> anyhow::Result<()> {
        let result = self.call(ActorOperation::Finish).and_then(expect_unit);
        self.finished = true;
        if result.is_err() {
            self.control.forced.store(true, Ordering::Release);
        }
        result
    }
}

impl Drop for ActiveLease {
    fn drop(&mut self) {
        if !self.finished {
            self.control.forced.store(true, Ordering::Release);
        }
    }
}

struct FeatureProxy {
    entry: Arc<Entry>,
    info: OpenInfo,
    owner: Uuid,
    canceled: Arc<AtomicBool>,
}

struct InterruptOwner {
    sender: SyncSender<InterruptRequest>,
    state: Mutex<Option<Result<ReportLengths, String>>>,
    changed: Condvar,
    stopped: AtomicBool,
    worker: Mutex<Option<JoinHandle<()>>>,
    topology: TopologyRevision,
}

struct InterruptRequest {
    signature: Signature,
    canceled: Arc<AtomicBool>,
    bytes: Vec<u8>,
    timeout_ms: u32,
    response: SyncSender<Result<PacketReply, String>>,
}

impl InterruptOwner {
    fn start(entry: Arc<Entry>) -> anyhow::Result<Arc<Self>> {
        let (sender, inbox) = mpsc::sync_channel(PACKET_CAPACITY);
        let owner = Arc::new(Self {
            sender,
            state: Mutex::new(None),
            changed: Condvar::new(),
            stopped: AtomicBool::new(false),
            worker: Mutex::new(None),
            topology: entry.topology.clone(),
        });
        let actor = owner.clone();
        let worker = thread::Builder::new()
            .name("hid-interrupt".into())
            .spawn(move || actor.run(entry, inbox))
            .context("Create interrupt collection actor")?;
        *owner.worker.lock().map_err(poison)? = Some(worker);
        Ok(owner)
    }

    fn run(self: Arc<Self>, entry: Arc<Entry>, inbox: Receiver<InterruptRequest>) {
        let opened = (|| {
            entry.validate_current()?;
            let mut backend = (entry.raw_factory)()?;
            entry.validate_current()?;
            let native = backend.open_interrupt(&entry.identity.node)?;
            entry.validate_current()?;
            ensure!(
                (entry.resolver)(&entry.identity.node)? == entry.identity,
                "OS collection identity changed during interrupt open"
            );
            let lengths = native.report_lengths();
            entry.validate_current()?;
            Ok::<_, anyhow::Error>((backend, native, lengths))
        })();
        let (_backend, native, lengths) = match opened {
            Ok(opened) => opened,
            Err(error) => {
                if let Ok(mut state) = self.state.lock() {
                    *state = Some(Err(format!("{error:#}")));
                    self.changed.notify_all();
                }
                self.stopped.store(true, Ordering::Release);
                return;
            }
        };
        if let Ok(mut state) = self.state.lock() {
            *state = Some(Ok(lengths));
            self.changed.notify_all();
        } else {
            return;
        }
        while !self.stopped.load(Ordering::Acquire) && entry.admitting().unwrap_or(false) {
            let mut request = match inbox.recv_timeout(CONTROL_POLL) {
                Ok(request) => request,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return,
            };
            let result = (|| {
                ensure!(
                    request.signature.epoch == entry.epoch
                        && !request.signature.owner.is_nil()
                        && !request.signature.request.is_nil(),
                    "Interrupt operation has a stale or foreign identity"
                );
                ensure!(
                    !request.canceled.load(Ordering::Acquire) && entry.admitting()?,
                    "Interrupt read was canceled before native dispatch"
                );
                let count = native.read(&mut request.bytes, request.timeout_ms)?;
                ensure!(
                    count <= request.bytes.len(),
                    "Native interrupt byte count exceeds the owned buffer"
                );
                ensure!(
                    !request.canceled.load(Ordering::Acquire) && entry.admitting()?,
                    "Interrupt read ended after cancellation or epoch invalidation"
                );
                Ok(PacketReply::Bytes {
                    bytes: request.bytes,
                    count,
                })
            })()
            .map_err(|error: anyhow::Error| format!("{error:#}"));
            let _ = request.response.send(result);
        }
    }

    fn wait_ready(
        &self,
        canceled: &AtomicBool,
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<ReportLengths> {
        loop {
            validate()?;
            self.topology.validate()?;
            let state = self.state.lock().map_err(poison)?;
            if let Some(result) = &*state {
                return result.clone().map_err(anyhow::Error::msg);
            }
            ensure!(
                !canceled.load(Ordering::Acquire),
                "Interrupt open was canceled"
            );
            ensure!(
                !self.stopped.load(Ordering::Acquire),
                "Interrupt actor terminated during open"
            );
            if self.finished()? {
                anyhow::bail!("Interrupt actor terminated without an open result");
            }
            let waited = self
                .changed
                .wait_timeout(state, CONTROL_POLL)
                .map_err(poison)?;
            drop(waited);
        }
    }

    fn finished(&self) -> anyhow::Result<bool> {
        Ok(self
            .worker
            .lock()
            .map_err(poison)?
            .as_ref()
            .is_none_or(|worker| worker.is_finished()))
    }

    fn join(&self) -> anyhow::Result<()> {
        if let Some(worker) = self.worker.lock().map_err(poison)?.take() {
            worker
                .join()
                .map_err(|_| anyhow::anyhow!("Interrupt actor panicked"))?;
        }
        Ok(())
    }
}

struct InterruptProxy {
    entry: Arc<Entry>,
    interrupt: Arc<InterruptOwner>,
    lengths: ReportLengths,
    owner: Uuid,
    canceled: Arc<AtomicBool>,
}

impl InterruptTransport for InterruptProxy {
    fn read(&self, report: &mut [u8], timeout_ms: u32) -> anyhow::Result<usize> {
        validate_buffer(report)?;
        ensure!(
            !self.canceled.load(Ordering::Acquire) && self.entry.admitting()?,
            "Interrupt owner is canceled or stale"
        );
        let (response, receipt) = mpsc::sync_channel(1);
        let request = InterruptRequest {
            signature: Signature {
                owner: self.owner,
                request: Uuid::new_v4(),
                epoch: self.entry.epoch,
            },
            canceled: self.canceled.clone(),
            bytes: report.to_vec(),
            timeout_ms,
            response,
        };
        match self.interrupt.sender.try_send(request) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => anyhow::bail!("Interrupt operation queue is full"),
            Err(TrySendError::Disconnected(_)) => {
                anyhow::bail!("Interrupt collection actor has closed")
            }
        }
        let reply = receipt
            .recv()
            .context("Interrupt actor closed before terminal read result")?
            .map_err(anyhow::Error::msg)?;
        ensure!(
            !self.canceled.load(Ordering::Acquire),
            "Interrupt owner was canceled before consuming its terminal result"
        );
        self.entry.validate_current()?;
        copy_reply(reply, report)
    }

    fn report_lengths(&self) -> ReportLengths {
        self.lengths.clone()
    }
}

impl FeatureProxy {
    fn lease(&self, options: ExchangeOptions) -> anyhow::Result<QueuedExchange<'_>> {
        self.lease_validated(options, Rc::new(no_validation))
    }

    fn lease_validated<'a>(
        &'a self,
        options: ExchangeOptions,
        validate: ExchangeValidation<'a>,
    ) -> anyhow::Result<QueuedExchange<'a>> {
        validate()?;
        let command = self.entry.commands.acquire(
            &self.entry,
            self.canceled.clone(),
            &options,
            validate.as_ref(),
        )?;
        let active = self.entry.acquire(
            self.owner,
            self.canceled.clone(),
            options.clone(),
            validate.as_ref(),
        )?;
        validate()?;
        Ok(QueuedExchange {
            proxy: self,
            options,
            active: RefCell::new(Some(active)),
            _command: command,
            validate,
        })
    }

    fn packet(&self, packet: Packet) -> anyhow::Result<PacketReply> {
        let lease = self.lease(ExchangeOptions::plain())?;
        let result = lease.packet(packet);
        let finish = Box::new(lease).finish();
        combine(result, finish)
    }
}

struct QueuedExchange<'a> {
    proxy: &'a FeatureProxy,
    options: ExchangeOptions,
    active: RefCell<Option<ActiveLease>>,
    _command: Option<CommandLease>,
    validate: ExchangeValidation<'a>,
}

impl QueuedExchange<'_> {
    fn packet(&self, packet: Packet) -> anyhow::Result<PacketReply> {
        (self.validate)()?;
        self.active
            .borrow()
            .as_ref()
            .context("HID exchange has no current cycle")?
            .call(ActorOperation::Packet(packet))
    }
}

impl ExchangeLease for QueuedExchange<'_> {
    fn transport(&self) -> &dyn FeatureTransport {
        self
    }

    fn retry_release(&self) -> anyhow::Result<()> {
        (self.validate)()?;
        let active = self
            .active
            .borrow_mut()
            .take()
            .context("HID retry has no current cycle")?;
        active.finish()?;
        let next = self.proxy.entry.acquire(
            self.proxy.owner,
            self.proxy.canceled.clone(),
            self.options.clone(),
            self.validate.as_ref(),
        )?;
        *self.active.borrow_mut() = Some(next);
        (self.validate)()
    }

    fn finish(self: Box<Self>) -> anyhow::Result<()> {
        (self.validate)()?;
        let active = self
            .active
            .borrow_mut()
            .take()
            .context("HID exchange has already released its cycle")?;
        active.finish()
    }
}

macro_rules! proxy_transport {
    () => {
        fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
            validate_buffer(report)?;
            expect_unit(self.packet(Packet::Send {
                bytes: report.to_vec(),
                collection_max: false,
            })?)
        }
        fn send_feature_at_collection_max(&self, report: &[u8]) -> anyhow::Result<()> {
            validate_buffer(report)?;
            expect_unit(self.packet(Packet::Send {
                bytes: report.to_vec(),
                collection_max: true,
            })?)
        }
        fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
            validate_buffer(report)?;
            copy_reply(
                self.packet(Packet::Get {
                    bytes: report.to_vec(),
                    collection_max: false,
                })?,
                report,
            )
        }
        fn get_feature_at_collection_max(&self, report: &mut [u8]) -> anyhow::Result<usize> {
            validate_buffer(report)?;
            copy_reply(
                self.packet(Packet::Get {
                    bytes: report.to_vec(),
                    collection_max: true,
                })?,
                report,
            )
        }
        fn write_output(&self, report: &[u8]) -> anyhow::Result<()> {
            validate_buffer(report)?;
            expect_unit(self.packet(Packet::Output(report.to_vec()))?)
        }
        fn write_output_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
            validate_buffer(report)?;
            match self.packet(Packet::CountedOutput(report.to_vec()))? {
                PacketReply::Count(count) => Ok(count),
                _ => anyhow::bail!("Counted Output returned the wrong typed result"),
            }
        }
        fn get_input(&self, report: &mut [u8]) -> anyhow::Result<usize> {
            validate_buffer(report)?;
            copy_reply(self.packet(Packet::Input(report.to_vec()))?, report)
        }
        fn read_interrupt(&self, report: &mut [u8], timeout_ms: u32) -> anyhow::Result<usize> {
            validate_buffer(report)?;
            copy_reply(
                self.packet(Packet::Interrupt {
                    bytes: report.to_vec(),
                    timeout_ms,
                })?,
                report,
            )
        }
    };
}

impl FeatureTransport for FeatureProxy {
    fn begin_exchange(
        &self,
        options: ExchangeOptions,
    ) -> anyhow::Result<Box<dyn ExchangeLease + '_>> {
        self.lease(options)
            .map(|lease| Box::new(lease) as Box<dyn ExchangeLease>)
    }

    fn begin_exchange_validated<'a>(
        &'a self,
        options: ExchangeOptions,
        validate: ExchangeValidation<'a>,
    ) -> anyhow::Result<Box<dyn ExchangeLease + 'a>> {
        self.lease_validated(options, validate)
            .map(|lease| Box::new(lease) as Box<dyn ExchangeLease>)
    }

    fn hid_node(&self) -> Option<&HidNode> {
        Some(&self.info.node)
    }

    fn report_lengths(&self) -> anyhow::Result<ReportLengths> {
        Ok(self.info.lengths.clone())
    }

    fn metadata(&self) -> Value {
        self.info.metadata.clone()
    }

    proxy_transport!();
}

impl FeatureTransport for QueuedExchange<'_> {
    fn hid_node(&self) -> Option<&HidNode> {
        Some(&self.proxy.info.node)
    }

    fn report_lengths(&self) -> anyhow::Result<ReportLengths> {
        Ok(self.proxy.info.lengths.clone())
    }

    fn metadata(&self) -> Value {
        self.proxy.info.metadata.clone()
    }

    proxy_transport!();
}

fn validate_buffer(buffer: &[u8]) -> anyhow::Result<()> {
    ensure!(
        !buffer.is_empty() && buffer.len() <= MAX_BUFFER_BYTES,
        "HID buffer is empty or exceeds the bounded operation size"
    );
    Ok(())
}

fn no_validation() -> anyhow::Result<()> {
    Ok(())
}

fn expect_unit(reply: PacketReply) -> anyhow::Result<()> {
    ensure!(
        matches!(reply, PacketReply::Unit),
        "HID operation returned the wrong typed result"
    );
    Ok(())
}

fn copy_reply(reply: PacketReply, buffer: &mut [u8]) -> anyhow::Result<usize> {
    let PacketReply::Bytes { bytes, count } = reply else {
        anyhow::bail!("HID read returned the wrong typed result");
    };
    ensure!(
        bytes.len() == buffer.len() && count <= buffer.len(),
        "HID read result differs from its owned buffer"
    );
    buffer.copy_from_slice(&bytes);
    Ok(count)
}

fn combine<T>(result: anyhow::Result<T>, cleanup: anyhow::Result<()>) -> anyhow::Result<T> {
    match (result, cleanup) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Err(error), Err(cleanup)) => {
            Err(error.context(format!("HID cycle release also failed: {cleanup:#}")))
        }
    }
}

fn poison<T>(_: std::sync::PoisonError<T>) -> anyhow::Error {
    anyhow::anyhow!("Collection owner state is poisoned")
}

#[cfg(test)]
#[path = "collection_tests.rs"]
mod tests;
