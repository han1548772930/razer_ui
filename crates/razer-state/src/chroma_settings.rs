//! One retained Chroma preference owner serializes I/O across all page/windows.
use gpui_kit::{App, AppContext as _, Context, Entity, EventEmitter, Global, Task};
pub use razer_model::chroma_settings::{AppFilter, Change, DashboardGroup, Preferences};
pub use razer_storage::chroma_settings::Origin;
use std::{
    collections::VecDeque,
    sync::atomic::{AtomicUsize, Ordering},
};

static ACTIVE_IO: AtomicUsize = AtomicUsize::new(0);

/// Counts work from admission until its foreground completion notification.
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
    Observed(Origin),
    Failed(String),
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersistenceFinished {
    error: Option<String>,
}
impl PersistenceFinished {
    pub fn is_success(&self) -> bool {
        self.error.is_none()
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

struct Edit {
    revision: u64,
    change: Change,
}
enum Operation {
    Read { revision: u64, generation: u64 },
    Write { revision: u64, changes: Vec<Change> },
}
enum Completed {
    Read {
        revision: u64,
        generation: u64,
        result: Result<(Preferences, Origin), String>,
    },
    Write {
        revision: u64,
        result: Result<Preferences, String>,
    },
}
impl Operation {
    fn run(self) -> Completed {
        match self {
            Self::Read {
                revision,
                generation,
            } => Completed::Read {
                revision,
                generation,
                result: guarded(|| {
                    razer_storage::chroma_settings::load()
                        .map(|observed| (observed.preferences().clone(), observed.origin()))
                        .map_err(|error| format!("{error:#}"))
                }),
            },
            Self::Write { revision, changes } => Completed::Write {
                revision,
                result: guarded(|| {
                    razer_storage::chroma_settings::update(&changes)
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
        .unwrap_or_else(|_| Err("Chroma settings storage panicked".into()))
}
struct Work {
    operation: Operation,
    accepted: AcceptedIo,
}

struct Core {
    preferences: Preferences,
    committed: Preferences,
    load: LoadStatus,
    revision: u64,
    committed_revision: u64,
    generation: u64,
    uncommitted: VecDeque<Edit>,
    queue: VecDeque<Work>,
    pending_writes: usize,
    save_error: Option<String>,
    closing: bool,
}
impl Default for Core {
    fn default() -> Self {
        Self {
            preferences: Preferences::default(),
            committed: Preferences::default(),
            load: LoadStatus::NotRequested,
            revision: 0,
            committed_revision: 0,
            generation: 0,
            uncommitted: VecDeque::new(),
            queue: VecDeque::new(),
            pending_writes: 0,
            save_error: None,
            closing: false,
        }
    }
}
fn is_settings_change(change: &Change) -> bool {
    matches!(
        change,
        Change::AutoStart(_)
            | Change::StartMinimized(_)
            | Change::Notifications(_)
            | Change::DynamicLighting(_)
            | Change::ResetTutorials(_)
    )
}
impl Core {
    fn load(&mut self, refresh: bool) -> Result<(), String> {
        if self.closing {
            return Err("Chroma settings are closing".into());
        }
        if refresh && !self.uncommitted.is_empty() {
            return Err("Local Chroma changes are unsaved; retry saving before refreshing".into());
        }
        if matches!(self.load, LoadStatus::Loading)
            || (!refresh && matches!(self.load, LoadStatus::Observed(_)))
        {
            return Ok(());
        }
        self.generation = self.generation.wrapping_add(1);
        self.load = LoadStatus::Loading;
        self.queue.push_back(Work {
            operation: Operation::Read {
                revision: self.revision,
                generation: self.generation,
            },
            accepted: AcceptedIo::new(),
        });
        Ok(())
    }
    fn change(&mut self, changes: Vec<Change>) -> Result<(), String> {
        if self.closing {
            return Err("Chroma settings are closing".into());
        }
        if changes.is_empty() {
            return Ok(());
        }
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or("Chroma settings revision exhausted")?;
        for change in changes {
            self.preferences.apply(&change);
            self.uncommitted.push_back(Edit {
                revision: self.revision,
                change,
            });
        }
        self.enqueue_write();
        Ok(())
    }
    fn enqueue_write(&mut self) {
        self.queue.push_back(Work {
            operation: Operation::Write {
                revision: self.revision,
                changes: self
                    .uncommitted
                    .iter()
                    .map(|edit| edit.change.clone())
                    .collect(),
            },
            accepted: AcceptedIo::new(),
        });
        self.pending_writes += 1;
    }
    fn retry(&mut self) -> Result<(), String> {
        if self.closing {
            return Err("Chroma settings are closing".into());
        }
        if self.uncommitted.is_empty() {
            return self.load(true);
        }
        if self.pending_writes == 0 {
            self.enqueue_write();
        }
        Ok(())
    }
    fn republish(&mut self) {
        self.preferences = self.committed.clone();
        for edit in &self.uncommitted {
            self.preferences.apply(&edit.change);
        }
    }
    fn complete(&mut self, completed: Completed) -> Option<PersistenceFinished> {
        match completed {
            Completed::Read {
                revision,
                generation,
                result,
            } => {
                if self.closing
                    || generation != self.generation
                    || revision < self.committed_revision
                {
                    return None;
                }
                match result {
                    Ok((preferences, origin)) => {
                        self.committed = preferences;
                        self.republish();
                        self.load = LoadStatus::Observed(origin);
                    }
                    Err(error) => self.load = LoadStatus::Failed(error),
                }
                None
            }
            Completed::Write { revision, result } => {
                self.pending_writes -= 1;
                let error = match result {
                    Ok(preferences) => {
                        self.committed = preferences;
                        self.committed_revision = revision;
                        self.uncommitted.retain(|edit| edit.revision > revision);
                        self.save_error = None;
                        self.republish();
                        self.load = LoadStatus::Observed(Origin::Current);
                        None
                    }
                    Err(error) => {
                        self.save_error = Some(error.clone());
                        Some(error)
                    }
                };
                Some(PersistenceFinished { error })
            }
        }
    }
    fn settings_dirty(&self) -> bool {
        self.uncommitted
            .iter()
            .any(|edit| is_settings_change(&edit.change))
    }
    fn discard_settings(&mut self) -> Result<(), String> {
        if self.closing {
            return Err("Chroma settings are closing".into());
        }
        if self.pending_writes > 0 {
            return Err("Chroma settings are still saving".into());
        }
        self.uncommitted
            .retain(|edit| !is_settings_change(&edit.change));
        if self.uncommitted.is_empty() {
            self.save_error = None;
        }
        self.republish();
        Ok(())
    }
    fn begin_exit(&mut self) {
        self.closing = true;
        self.generation = self.generation.wrapping_add(1);
        self.queue
            .retain(|work| matches!(work.operation, Operation::Write { .. }));
        if matches!(self.load, LoadStatus::Loading) {
            self.load = LoadStatus::Cancelled;
        }
    }
    fn cancel_exit(&mut self) {
        if !self.closing {
            return;
        }
        self.closing = false;
        if matches!(self.load, LoadStatus::Cancelled) {
            self.load = LoadStatus::NotRequested;
            let _ = self.load(false);
        }
    }
}

struct Shared(Entity<ChromaSettings>);
impl Global for Shared {}

pub struct ChromaSettings {
    core: Core,
    worker: Option<Task<()>>,
}
impl EventEmitter<PersistenceFinished> for ChromaSettings {}
impl ChromaSettings {
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
    pub fn acquire(cx: &mut App) -> Entity<Self> {
        let owner = Self::shared(cx);
        owner.update(cx, |owner, cx| {
            let _ = owner.core.load(false);
            owner.start_worker(cx);
            cx.notify();
        });
        owner
    }
    pub fn preferences(&self) -> &Preferences {
        &self.core.preferences
    }
    pub fn load_status(&self) -> &LoadStatus {
        &self.core.load
    }
    pub fn save_error(&self) -> Option<&str> {
        self.core.save_error.as_deref()
    }
    pub fn error(&self) -> Option<&str> {
        self.save_error().or_else(|| match &self.core.load {
            LoadStatus::Failed(error) => Some(error.as_str()),
            _ => None,
        })
    }
    pub fn is_pending(&self) -> bool {
        self.core.pending_writes > 0
    }
    pub fn is_closing(&self) -> bool {
        self.core.closing
    }
    pub fn settings_dirty(&self) -> bool {
        self.core.settings_dirty()
    }
    pub fn change(&mut self, change: Change, cx: &mut Context<Self>) -> Result<(), String> {
        self.change_many(vec![change], cx)
    }
    pub fn change_many(
        &mut self,
        changes: Vec<Change>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.core.change(changes)?;
        self.start_worker(cx);
        cx.notify();
        Ok(())
    }
    pub fn retry(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        self.core.retry()?;
        self.start_worker(cx);
        cx.notify();
        Ok(())
    }
    pub fn reload(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        self.core.load(true)?;
        self.start_worker(cx);
        cx.notify();
        Ok(())
    }
    pub fn discard_settings(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        self.core.discard_settings()?;
        cx.notify();
        Ok(())
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
                    if let Some(event) = this.core.complete(completed) {
                        cx.emit(event);
                    }
                    cx.notify();
                    // Emit effects run before this deferred release, so exit
                    // observes both the storage result and its subscribers.
                    cx.defer(move |_| drop(accepted));
                });
            }
            let _ = this.update(cx, |this, _| this.worker = None);
        }));
    }
}

pub fn begin_exit(cx: &mut App) {
    ChromaSettings::shared(cx).update(cx, |owner, cx| {
        owner.core.begin_exit();
        owner.start_worker(cx);
        cx.notify();
    });
}
pub fn cancel_exit(cx: &mut App) {
    ChromaSettings::shared(cx).update(cx, |owner, cx| {
        owner.core.cancel_exit();
        owner.start_worker(cx);
        cx.notify();
    });
}

#[cfg(test)]
#[path = "chroma_settings/tests.rs"]
mod tests;
