//! The mounted profile queue pauses automatic source effects at exit while
//! retaining its accepted local writes and the source's optimistic selection.
use razer_storage::linked_profiles::{Effect, ProfileStack};
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Input {
    Base(String),
    Started(String),
    Stopped(String),
    Foreground(String),
}

#[derive(Default)]
pub(super) struct ProfileQueue {
    pub(super) inputs: VecDeque<Input>,
    pub(super) effects: VecDeque<Effect>,
    pub(super) saving: bool,
    frozen: bool,
    loaded_base: Option<String>,
}
impl ProfileQueue {
    pub(super) fn automatic(&self) -> bool {
        !self.frozen
    }

    pub(super) fn prepare_exit(&mut self) -> Vec<String> {
        self.frozen = true;
        let mut bases = Vec::new();
        self.inputs.retain(|input| {
            if let Input::Base(guid) = input {
                bases.push(guid.clone());
                false
            } else {
                true
            }
        });
        bases
    }

    pub(super) fn cancel_exit(&mut self) {
        self.frozen = false;
    }

    /// A manual base selection is already requested local persistence. During
    /// exit it uses the worker's serial SelectBase path without resuming an
    /// automatic runtime-profile switch.
    pub(super) fn select_base(&mut self, guid: String) -> Option<String> {
        if self.frozen {
            Some(guid)
        } else {
            self.inputs.push_back(Input::Base(guid));
            None
        }
    }

    pub(super) fn rebase_pending_stacks(&mut self, guid: &str) {
        self.loaded_base = Some(guid.into());
        for effect in &mut self.effects {
            if let Effect::SaveStack(stack) = effect {
                stack.select_base(guid);
            }
        }
    }

    pub(super) fn reconcile_loaded_stack(&mut self, mut stack: ProfileStack) -> ProfileStack {
        if let Some(base) = self.loaded_base.take() {
            stack.select_base(base);
        }
        stack
    }

    pub(super) fn next_effect(&mut self) -> Option<Effect> {
        (!self.frozen && !self.saving)
            .then(|| self.effects.pop_front())
            .flatten()
    }

    pub(super) fn acknowledge_save(&mut self, result: &Result<(), String>) {
        self.saving = false;
        if result.is_err() {
            // Source local-commit failure prevents its following system task
            // without rolling back the already published runtime selection.
            self.effects.clear();
        }
    }
}

#[derive(Clone, Default)]
pub(super) struct PendingIo(Arc<AtomicUsize>);
impl PendingIo {
    pub(super) fn admit<T>(&self, command: T) -> Queued<T> {
        self.0.fetch_add(1, Ordering::AcqRel);
        Queued {
            command,
            _pending: IoGuard(self.clone()),
        }
    }

    pub(super) fn is_pending(&self) -> bool {
        self.0.load(Ordering::Acquire) != 0
    }
}
pub(super) struct Queued<T> {
    pub(super) command: T,
    pub(super) _pending: IoGuard,
}
pub(super) struct IoGuard(PendingIo);
impl Drop for IoGuard {
    fn drop(&mut self) {
        self.0.0.fetch_sub(1, Ordering::AcqRel);
    }
}

#[cfg(test)]
#[path = "game_monitor_lifecycle_tests.rs"]
mod tests;
