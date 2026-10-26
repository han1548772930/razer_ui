//! One retained UI preference owner serializes local I/O across page lifetimes.
use gpui_kit::{App, AppContext as _, Context, Entity, EventEmitter, Global, Task};
pub use razer_storage::ui_preferences::{Flag, Observation, Origin};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::atomic::{AtomicUsize, Ordering},
};

static ACTIVE_IO: AtomicUsize = AtomicUsize::new(0);

/// Includes accepted queued writes, not just a currently executing file call.
pub fn active_io() -> usize {
    ACTIVE_IO.load(Ordering::Acquire)
}

struct AcceptedIo;
impl AcceptedIo {
    fn new() -> Self {
        ACTIVE_IO.fetch_add(1, Ordering::AcqRel);
        Self
    }
}
impl Drop for AcceptedIo {
    fn drop(&mut self) {
        ACTIVE_IO.fetch_sub(1, Ordering::AcqRel);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadStatus {
    NotRequested,
    Loading,
    Observed(Observation),
    Failed(String),
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SaveStatus {
    Unchanged,
    Queued { revision: u64 },
    Saved { revision: u64 },
    Failed { revision: u64, error: String },
}

pub struct PersistenceFinished {
    flag: Flag,
    revision: u64,
    result: Result<(), String>,
}
impl PersistenceFinished {
    pub fn flag(&self) -> Flag {
        self.flag
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn is_success(&self) -> bool {
        self.result.is_ok()
    }
    pub fn error(&self) -> Option<&str> {
        self.result.as_ref().err().map(String::as_str)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    /// Source default is presentation while the stored value is unobserved.
    value: bool,
    revision: u64,
    load: LoadStatus,
    save: SaveStatus,
    persisted_value: Option<bool>,
    read_generation: u64,
}
impl Snapshot {
    fn new(flag: Flag) -> Self {
        Self {
            value: flag.initial_value(),
            revision: 0,
            load: LoadStatus::NotRequested,
            save: SaveStatus::Unchanged,
            persisted_value: None,
            read_generation: 0,
        }
    }
    pub fn ready(&self) -> bool {
        matches!(self.load, LoadStatus::Observed(_) | LoadStatus::Failed(_))
    }
    pub fn value(&self) -> bool {
        self.value
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn load_status(&self) -> &LoadStatus {
        &self.load
    }
    pub fn save_status(&self) -> &SaveStatus {
        &self.save
    }
    pub fn persisted_value(&self) -> Option<bool> {
        self.persisted_value
    }
}

#[derive(Clone, Copy)]
enum Operation {
    PollNotifications {
        generation: u64,
    },
    Read {
        flag: Flag,
        revision: u64,
        generation: u64,
    },
    Write {
        flag: Flag,
        revision: u64,
        value: bool,
    },
}
impl Operation {
    fn run(self) -> Completed {
        // A storage panic must release the accepted request and leave the
        // worker able to drain subsequent writes and report the failure.
        match self {
            Self::PollNotifications { generation } => Completed::PollNotifications {
                generation,
                result: guarded(|| {
                    razer_storage::notification_preferences::raw_value()
                        .map_err(|error| format!("{error:#}"))
                }),
            },
            Self::Read {
                flag,
                revision,
                generation,
            } => Completed::Read {
                flag,
                revision,
                generation,
                result: guarded(|| {
                    razer_storage::ui_preferences::load(flag).map_err(|error| format!("{error:#}"))
                }),
            },
            Self::Write {
                flag,
                revision,
                value,
            } => Completed::Write {
                flag,
                revision,
                value,
                result: guarded(|| {
                    razer_storage::ui_preferences::save(flag, value)
                        .map_err(|error| format!("{error:#}"))
                }),
            },
        }
    }
}
fn guarded<T>(
    operation: impl FnOnce() -> Result<T, String> + std::panic::UnwindSafe,
) -> Result<T, String> {
    std::panic::catch_unwind(operation)
        .unwrap_or_else(|_| Err("UI preference storage panicked".into()))
}
struct Work {
    operation: Operation,
    accepted: AcceptedIo,
}
enum Completed {
    PollNotifications {
        generation: u64,
        result: Result<Option<String>, String>,
    },
    Read {
        flag: Flag,
        revision: u64,
        generation: u64,
        result: Result<Observation, String>,
    },
    Write {
        flag: Flag,
        revision: u64,
        value: bool,
        result: Result<(), String>,
    },
}
impl Completed {
    fn flag(&self) -> Flag {
        match self {
            Self::PollNotifications { .. } => Flag::DisplayNotifications,
            Self::Read { flag, .. } | Self::Write { flag, .. } => *flag,
        }
    }
    fn error(&self) -> Option<&str> {
        match self {
            Self::PollNotifications { result, .. } => result.as_ref().err().map(String::as_str),
            Self::Read { result, .. } => result.as_ref().err().map(String::as_str),
            Self::Write { result, .. } => result.as_ref().err().map(String::as_str),
        }
    }
}

#[derive(Default)]
struct NotificationWatch {
    active: bool,
    generation: u64,
    baseline: Option<Option<String>>,
    pending: bool,
    failed: bool,
    initial_read: bool,
}

struct Core {
    flags: BTreeMap<Flag, Snapshot>,
    queue: VecDeque<Work>,
    write_errors: BTreeMap<Flag, String>,
    pending_writes: usize,
    closing: bool,
    notification_watch: NotificationWatch,
}
impl Default for Core {
    fn default() -> Self {
        Self {
            flags: Flag::ALL
                .into_iter()
                .map(|flag| (flag, Snapshot::new(flag)))
                .collect(),
            queue: VecDeque::new(),
            write_errors: BTreeMap::new(),
            pending_writes: 0,
            closing: false,
            notification_watch: NotificationWatch::default(),
        }
    }
}
impl Core {
    fn observe_notifications(&mut self) {
        if self.closing || self.notification_watch.active {
            return;
        }
        let watch = &mut self.notification_watch;
        watch.active = true;
        watch.generation = watch.generation.wrapping_add(1);
        watch.baseline = None;
        watch.failed = false;
        watch.initial_read = true;
        watch.pending = false;
        self.invalidate_reads(Flag::DisplayNotifications);
        self.flags
            .get_mut(&Flag::DisplayNotifications)
            .expect("known UI preference")
            .load = LoadStatus::NotRequested;
        self.poll_notifications();
    }
    fn poll_notifications(&mut self) {
        let watch = &mut self.notification_watch;
        if self.closing || !watch.active || watch.pending {
            return;
        }
        watch.pending = true;
        self.queue.push_back(Work {
            operation: Operation::PollNotifications {
                generation: watch.generation,
            },
            accepted: AcceptedIo::new(),
        });
    }
    fn load(&mut self, flag: Flag) {
        let state = self.flags.get_mut(&flag).expect("known UI preference");
        if self.closing || !matches!(state.load, LoadStatus::NotRequested | LoadStatus::Failed(_)) {
            return;
        }
        state.load = LoadStatus::Loading;
        self.queue.push_back(Work {
            operation: Operation::Read {
                flag,
                revision: state.revision,
                generation: state.read_generation,
            },
            accepted: AcceptedIo::new(),
        });
    }
    fn set(&mut self, flag: Flag, value: bool) {
        if self.closing {
            return;
        }
        let state = self.flags.get_mut(&flag).expect("known UI preference");
        state.value = value;
        state.revision = state.revision.wrapping_add(1);
        state.save = SaveStatus::Queued {
            revision: state.revision,
        };
        self.queue.push_back(Work {
            operation: Operation::Write {
                flag,
                revision: state.revision,
                value,
            },
            accepted: AcceptedIo::new(),
        });
        self.pending_writes += 1;
    }
    fn complete(&mut self, completion: Completed) -> Option<PersistenceFinished> {
        match completion {
            Completed::PollNotifications { generation, result } => {
                let watch = &mut self.notification_watch;
                if self.closing || !watch.active || generation != watch.generation {
                    return None;
                }
                watch.pending = false;
                match result {
                    Ok(raw) => {
                        let recovering = std::mem::take(&mut watch.failed);
                        let initial_read = std::mem::take(&mut watch.initial_read);
                        if watch.baseline.as_ref() != Some(&raw)
                            || recovering
                            || initial_read
                            || matches!(
                                self.flags[&Flag::DisplayNotifications].load,
                                LoadStatus::NotRequested
                            )
                        {
                            watch.baseline = Some(raw);
                            // A changed key invalidates older observations; accepted
                            // user writes keep their position ahead of this reread.
                            self.invalidate_reads(Flag::DisplayNotifications);
                            self.flags
                                .get_mut(&Flag::DisplayNotifications)
                                .expect("known UI preference")
                                .load = LoadStatus::NotRequested;
                            self.load(Flag::DisplayNotifications);
                        }
                    }
                    Err(error) => {
                        watch.failed = true;
                        self.flags
                            .get_mut(&Flag::DisplayNotifications)
                            .expect("known UI preference")
                            .load = LoadStatus::Failed(error);
                    }
                }
                None
            }
            Completed::Read {
                flag,
                revision,
                generation,
                result,
            } => {
                let state = self.flags.get_mut(&flag).expect("known UI preference");
                if self.closing || generation != state.read_generation {
                    return None;
                }
                match result {
                    Ok(observation) => {
                        state.persisted_value = observation.value();
                        let current = state.revision == revision;
                        if current {
                            state.value =
                                observation.value().unwrap_or_else(|| flag.default_value());
                        }
                        state.load = LoadStatus::Observed(observation);
                        if current && flag.publish_on_observe() {
                            let value = state.value;
                            self.set(flag, value);
                        }
                    }
                    Err(error) => state.load = LoadStatus::Failed(error),
                }
                None
            }
            Completed::Write {
                flag,
                revision,
                value,
                result,
            } => {
                self.pending_writes -= 1;
                let finished = PersistenceFinished {
                    flag,
                    revision,
                    result: result.clone(),
                };
                let state = self.flags.get_mut(&flag).expect("known UI preference");
                match &result {
                    Ok(()) => {
                        state.persisted_value = Some(value);
                        self.write_errors.remove(&flag);
                        if flag == Flag::DisplayNotifications && self.notification_watch.active {
                            // localStorage does not notify the document performing
                            // its own write. A later external value still differs.
                            self.notification_watch.baseline = Some(Some(value.to_string()));
                        }
                    }
                    Err(error) => {
                        self.write_errors.insert(flag, error.clone());
                    }
                }
                if state.revision == revision {
                    state.save = match result {
                        Ok(_) => SaveStatus::Saved { revision },
                        Err(error) => SaveStatus::Failed { revision, error },
                    };
                }
                Some(finished)
            }
        }
    }
    fn save_error(&self) -> Option<&str> {
        self.write_errors.values().next().map(String::as_str)
    }
    fn retry_failed(&mut self) -> usize {
        if self.closing {
            return 0;
        }
        let failed = self
            .flags
            .iter()
            .filter_map(|(&flag, state)| {
                matches!(state.save, SaveStatus::Failed { .. }).then_some((flag, state.value))
            })
            .collect::<Vec<_>>();
        let count = failed.len();
        for (flag, value) in failed {
            self.set(flag, value);
        }
        count
    }
    fn begin_exit(&mut self) {
        self.closing = true;
        self.notification_watch.generation = self.notification_watch.generation.wrapping_add(1);
        self.notification_watch.pending = false;
        self.queue
            .retain(|work| matches!(work.operation, Operation::Write { .. }));
        for state in self.flags.values_mut() {
            state.read_generation = state.read_generation.wrapping_add(1);
            if matches!(state.load, LoadStatus::Loading) {
                state.load = LoadStatus::Cancelled;
            }
        }
    }
    fn cancel_observation(&mut self, flag: Flag) {
        self.invalidate_reads(flag);
        if flag == Flag::DisplayNotifications {
            self.notification_watch.active = false;
            self.notification_watch.generation = self.notification_watch.generation.wrapping_add(1);
            self.notification_watch.pending = false;
            if matches!(self.flags[&flag].load, LoadStatus::Cancelled) {
                self.flags.get_mut(&flag).expect("known UI preference").load =
                    LoadStatus::NotRequested;
            }
            self.queue
                .retain(|work| !matches!(work.operation, Operation::PollNotifications { .. }));
        }
    }
    fn invalidate_reads(&mut self, flag: Flag) {
        let state = self.flags.get_mut(&flag).expect("known UI preference");
        state.read_generation = state.read_generation.wrapping_add(1);
        if matches!(state.load, LoadStatus::Loading) {
            state.load = LoadStatus::NotRequested;
        }
        self.queue.retain(|work| {
            !matches!(work.operation, Operation::Read { flag: pending, .. } if pending == flag)
        });
    }
    fn cancel_exit(&mut self) {
        if !self.closing {
            return;
        }
        self.closing = false;
        self.poll_notifications();
        for flag in Flag::ALL {
            if matches!(self.flags[&flag].load, LoadStatus::Cancelled) {
                self.flags.get_mut(&flag).expect("known UI preference").load =
                    LoadStatus::NotRequested;
                self.load(flag);
            }
        }
    }
}

struct Shared(Entity<UiPreferences>);
impl Global for Shared {}

pub struct UiPreferences {
    core: Core,
    worker: Option<Task<()>>,
    notification_watcher: Option<Task<()>>,
}
impl EventEmitter<PersistenceFinished> for UiPreferences {}
impl UiPreferences {
    pub fn shared(cx: &mut App) -> Entity<Self> {
        if !cx.has_global::<Shared>() {
            let owner = cx.new(|_| Self {
                core: Core::default(),
                worker: None,
                notification_watcher: None,
            });
            cx.set_global(Shared(owner));
        }
        cx.global::<Shared>().0.clone()
    }
    pub fn acquire(flag: Flag, cx: &mut App) -> Entity<Self> {
        let owner = Self::shared(cx);
        owner.update(cx, |owner, cx| owner.load(flag, cx));
        owner
    }
    pub fn snapshot(&self, flag: Flag) -> &Snapshot {
        &self.core.flags[&flag]
    }
    pub fn closing(&self) -> bool {
        self.core.closing
    }
    pub fn save_error(&self) -> Option<&str> {
        self.core.save_error()
    }
    pub fn is_pending(&self) -> bool {
        self.core.pending_writes > 0
    }
    pub fn retry_failed(&mut self, cx: &mut Context<Self>) -> usize {
        let count = self.core.retry_failed();
        self.start_worker(cx);
        cx.notify();
        count
    }
    pub fn load(&mut self, flag: Flag, cx: &mut Context<Self>) {
        self.core.load(flag);
        self.start_worker(cx);
        cx.notify();
    }
    /// A real page mount/storage event re-reads through the same serialized
    /// owner. Pending writes stay ahead of this observation in the queue.
    pub fn reload(&mut self, flag: Flag, cx: &mut Context<Self>) {
        let state = self.core.flags.get_mut(&flag).expect("known UI preference");
        if !matches!(state.load, LoadStatus::Loading) {
            state.load = LoadStatus::NotRequested;
        }
        self.load(flag, cx);
    }
    /// The settings component's same-key storage listener lives only while
    /// mounted. File polling is serialized with its actual reads and writes.
    pub fn observe_notifications(&mut self, cx: &mut Context<Self>) {
        self.core.observe_notifications();
        self.start_worker(cx);
        self.start_notification_watcher(cx);
        cx.notify();
    }
    /// Unmount cancels unpublished observations, while accepted writes remain
    /// owned by the shared worker through their actual persistence result.
    pub fn cancel_observation(&mut self, flag: Flag, cx: &mut Context<Self>) {
        self.core.cancel_observation(flag);
        if flag == Flag::DisplayNotifications {
            // This task owns only a timer. In-flight file calls remain owned
            // by worker/AcceptedIo until their actual completion.
            self.notification_watcher = None;
        }
        cx.notify();
    }
    pub fn set(&mut self, flag: Flag, value: bool, cx: &mut Context<Self>) {
        self.core.set(flag, value);
        self.start_worker(cx);
        cx.notify();
    }
    fn start_worker(&mut self, cx: &mut Context<Self>) {
        if self.worker.is_some() || self.core.queue.is_empty() {
            return;
        }
        self.worker = Some(cx.spawn(async move |this, cx| {
            loop {
                let work = this
                    .update(cx, |this, _| this.core.queue.pop_front())
                    .ok()
                    .flatten();
                let Some(Work {
                    operation,
                    accepted,
                }) = work
                else {
                    break;
                };
                let (completed, accepted) = cx
                    .background_executor()
                    .spawn(async move { (operation.run(), accepted) })
                    .await;
                let _ = this.update(cx, |this, cx| {
                    let previous = matches!(completed, Completed::PollNotifications { .. })
                        .then(|| this.core.flags[&Flag::DisplayNotifications].clone());
                    let failure = completed
                        .error()
                        .map(|error| (completed.flag(), error.to_owned()));
                    if let Some(finished) = this.core.complete(completed) {
                        cx.emit(finished);
                    }
                    if previous.as_ref() != Some(&this.core.flags[&Flag::DisplayNotifications]) {
                        if let Some((flag, error)) = failure {
                            eprintln!("UI preference {flag:?}: {error}");
                        }
                        cx.notify();
                    }
                    cx.defer(move |_| drop(accepted));
                });
            }
            let _ = this.update(cx, |this, _| this.worker = None);
        }));
    }
    fn start_notification_watcher(&mut self, cx: &mut Context<Self>) {
        if self.notification_watcher.is_some()
            || self.core.closing
            || !self.core.notification_watch.active
        {
            return;
        }
        let generation = self.core.notification_watch.generation;
        self.notification_watcher = Some(cx.spawn(async move |this, cx| {
            loop {
                // Notifications can be missed or coalesced by file replacement.
                // Rechecking the one mounted key also handles atomic publication.
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(250))
                    .await;
                let running = this.update(cx, |this, cx| {
                    if this.core.closing
                        || !this.core.notification_watch.active
                        || this.core.notification_watch.generation != generation
                    {
                        return false;
                    }
                    this.core.poll_notifications();
                    this.start_worker(cx);
                    true
                });
                if !matches!(running, Ok(true)) {
                    break;
                }
            }
        }));
    }
}

/// Stop admitting new requests, discard unread page observations, and retain
/// the worker until every accepted write has produced a receipt.
pub fn begin_exit(cx: &mut App) {
    let owner = UiPreferences::shared(cx);
    owner.update(cx, |owner, cx| {
        owner.core.begin_exit();
        owner.notification_watcher = None;
        cx.notify();
    });
}

/// A failed requested workspace save cancels the application's exit intent.
/// Restore preference admission while rejecting reads from that closed scope.
pub fn cancel_exit(cx: &mut App) {
    let owner = UiPreferences::shared(cx);
    owner.update(cx, |owner, cx| {
        owner.core.cancel_exit();
        owner.start_worker(cx);
        owner.start_notification_watcher(cx);
        cx.notify();
    });
}

#[cfg(test)]
#[path = "ui_preferences/tests.rs"]
mod tests;
