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
}
impl Snapshot {
    fn new(flag: Flag) -> Self {
        Self {
            value: flag.default_value(),
            revision: 0,
            load: LoadStatus::NotRequested,
            save: SaveStatus::Unchanged,
            persisted_value: None,
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
            Self::Read { flag, .. } | Self::Write { flag, .. } => *flag,
        }
    }
    fn error(&self) -> Option<&str> {
        match self {
            Self::Read { result, .. } => result.as_ref().err().map(String::as_str),
            Self::Write { result, .. } => result.as_ref().err().map(String::as_str),
        }
    }
}

struct Core {
    flags: BTreeMap<Flag, Snapshot>,
    queue: VecDeque<Work>,
    write_errors: BTreeMap<Flag, String>,
    pending_writes: usize,
    closing: bool,
    read_generation: u64,
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
            read_generation: 0,
        }
    }
}
impl Core {
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
                generation: self.read_generation,
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
            Completed::Read {
                flag,
                revision,
                generation,
                result,
            } => {
                let state = self.flags.get_mut(&flag).expect("known UI preference");
                if self.closing || generation != self.read_generation {
                    return None;
                }
                match result {
                    Ok(observation) => {
                        state.persisted_value = observation.value();
                        if state.revision == revision {
                            state.value =
                                observation.value().unwrap_or_else(|| flag.default_value());
                        }
                        state.load = LoadStatus::Observed(observation);
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
        self.read_generation = self.read_generation.wrapping_add(1);
        self.queue
            .retain(|work| matches!(work.operation, Operation::Write { .. }));
        for state in self.flags.values_mut() {
            if matches!(state.load, LoadStatus::Loading) {
                state.load = LoadStatus::Cancelled;
            }
        }
    }
    fn cancel_exit(&mut self) {
        if !self.closing {
            return;
        }
        self.closing = false;
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
}
impl EventEmitter<PersistenceFinished> for UiPreferences {}
impl UiPreferences {
    pub fn shared(cx: &mut App) -> Entity<Self> {
        if !cx.has_global::<Shared>() {
            let owner = cx.new(|_| Self {
                core: Core::default(),
                worker: None,
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
                if let Some(error) = completed.error() {
                    eprintln!("UI preference {:?}: {error}", completed.flag());
                }
                let _ = this.update(cx, |this, cx| {
                    if let Some(finished) = this.core.complete(completed) {
                        cx.emit(finished);
                    }
                    cx.notify();
                    cx.defer(move |_| drop(accepted));
                });
            }
            let _ = this.update(cx, |this, _| this.worker = None);
        }));
    }
}

/// Stop admitting new requests, discard unread page observations, and retain
/// the worker until every accepted write has produced a receipt.
pub fn begin_exit(cx: &mut App) {
    let owner = UiPreferences::shared(cx);
    owner.update(cx, |owner, cx| {
        owner.core.begin_exit();
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
        cx.notify();
    });
}

#[cfg(test)]
#[path = "ui_preferences/tests.rs"]
mod tests;
