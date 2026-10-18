//! Device-manager execution ownership from a9500, 5ed88, 5f086, 5de50,
//! 57090 and 5d890. Completion means the output worker has also finished its
//! cleanup; requesting cancellation never releases an execution identity.
use anyhow::{Result, ensure};
use std::collections::{BTreeMap, VecDeque};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Start,
    Cancel,
    Toggle,
    Queue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request<P> {
    pub caller: Uuid,
    pub guid: String,
    pub payload: P,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dispatch<P> {
    pub id: Uuid,
    pub request: Request<P>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Transition<P> {
    Start(Dispatch<P>),
    Cancel(Uuid),
    Queued,
    Unchanged,
}

pub struct Scheduler<P> {
    active: BTreeMap<(String, Uuid), Dispatch<P>>,
    queued: VecDeque<Request<P>>,
    queue_active: Option<Dispatch<P>>,
    draining: bool,
}

impl<P> Default for Scheduler<P> {
    fn default() -> Self {
        Self {
            active: BTreeMap::new(),
            queued: VecDeque::new(),
            queue_active: None,
            draining: false,
        }
    }
}

impl<P: Clone> Scheduler<P> {
    pub fn submit(&mut self, request: Request<P>, mode: Mode) -> Result<Transition<P>> {
        ensure!(!self.draining, "Mapping executions are draining");
        let key = (request.guid.clone(), request.caller);
        let running = self.active.get(&key).map(|run| run.id);
        match mode {
            Mode::Cancel => Ok(running.map_or(Transition::Unchanged, Transition::Cancel)),
            Mode::Toggle if running.is_some() => Ok(Transition::Cancel(running.unwrap())),
            Mode::Start if running.is_some() => Ok(Transition::Unchanged),
            Mode::Queue => {
                self.queued.push_back(request);
                Ok(self
                    .next_queue()
                    .map_or(Transition::Queued, Transition::Start))
            }
            Mode::Start | Mode::Toggle => {
                let dispatch = Dispatch {
                    id: Uuid::new_v4(),
                    request,
                };
                self.active.insert(key, dispatch.clone());
                Ok(Transition::Start(dispatch))
            }
        }
    }

    fn next_queue(&mut self) -> Option<Dispatch<P>> {
        if self.draining || self.queue_active.is_some() {
            return None;
        }
        let request = self.queued.pop_front()?;
        let dispatch = Dispatch {
            id: Uuid::new_v4(),
            request,
        };
        self.queue_active = Some(dispatch.clone());
        Some(dispatch)
    }

    /// Failed worker creation also completes its reserved identity. The source
    /// queue skips such failures and tries the following entry in FIFO order.
    pub fn complete(&mut self, id: Uuid) -> Result<Option<Dispatch<P>>> {
        if self.queue_active.as_ref().is_some_and(|run| run.id == id) {
            self.queue_active = None;
            return Ok(self.next_queue());
        }
        let key = self
            .active
            .iter()
            .find_map(|(key, run)| (run.id == id).then(|| key.clone()));
        let Some(key) = key else {
            anyhow::bail!("Mapping execution completion belongs to another owner");
        };
        self.active.remove(&key);
        Ok(None)
    }

    /// The caller supplies the mapped-key equality already established by the
    /// input owner. GUID equality alone is insufficient for StopExecution.
    pub fn stop_matching(&mut self, caller: Uuid, matches: impl Fn(&P) -> bool) -> Vec<Uuid> {
        self.queued
            .retain(|request| request.caller != caller || !matches(&request.payload));
        self.active
            .values()
            .chain(self.queue_active.iter())
            .filter(|run| run.request.caller == caller && matches(&run.request.payload))
            .map(|run| run.id)
            .collect()
    }

    /// Disable and shutdown discard queued work before waiting for actual
    /// completion of every active execution (manager continuation at +168).
    pub fn begin_drain(&mut self) -> Vec<Uuid> {
        self.draining = true;
        self.queued.clear();
        self.active
            .values()
            .chain(self.queue_active.iter())
            .map(|run| run.id)
            .collect()
    }

    pub fn is_idle(&self) -> bool {
        self.active.is_empty() && self.queue_active.is_none() && self.queued.is_empty()
    }

    pub fn finish_drain(&mut self) -> Result<()> {
        ensure!(
            self.draining && self.is_idle(),
            "Mapping execution cleanup is incomplete"
        );
        self.draining = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(caller: Uuid, key: u32) -> Request<u32> {
        Request {
            caller,
            guid: "same-macro".into(),
            payload: key,
        }
    }
    fn started(transition: Transition<u32>) -> Dispatch<u32> {
        let Transition::Start(run) = transition else {
            panic!("expected an execution")
        };
        run
    }

    #[test]
    fn caller_identity_scopes_toggle_until_actual_completion() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let mut state = Scheduler::default();
        let first = started(state.submit(request(a, 1), Mode::Start).unwrap());
        let second = started(state.submit(request(b, 1), Mode::Start).unwrap());
        assert_eq!(
            state.submit(request(a, 2), Mode::Start).unwrap(),
            Transition::Unchanged
        );
        assert_eq!(
            state.submit(request(a, 2), Mode::Toggle).unwrap(),
            Transition::Cancel(first.id)
        );
        assert_eq!(
            state.submit(request(a, 2), Mode::Toggle).unwrap(),
            Transition::Cancel(first.id)
        );
        state.complete(first.id).unwrap();
        let replacement = started(state.submit(request(a, 2), Mode::Start).unwrap());
        assert!(state.complete(first.id).is_err());
        assert_eq!(state.stop_matching(b, |_| true), [second.id]);
        assert_eq!(state.stop_matching(a, |key| *key == 2), [replacement.id]);
    }

    #[test]
    fn queue_is_global_fifo_and_independent_from_normal_lookup() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let mut state = Scheduler::default();
        let normal = started(state.submit(request(a, 1), Mode::Start).unwrap());
        let first = started(state.submit(request(a, 2), Mode::Queue).unwrap());
        assert_eq!(
            state.submit(request(b, 3), Mode::Queue).unwrap(),
            Transition::Queued
        );
        assert_eq!(
            state.submit(request(a, 4), Mode::Queue).unwrap(),
            Transition::Queued
        );
        assert_eq!(
            state.submit(request(a, 2), Mode::Cancel).unwrap(),
            Transition::Cancel(normal.id)
        );
        let second = state.complete(first.id).unwrap().unwrap();
        assert_eq!(second.request, request(b, 3));
        let third = state.complete(second.id).unwrap().unwrap();
        assert_eq!(third.request, request(a, 4));
        state.complete(normal.id).unwrap();
        assert!(!state.is_idle());
        state.complete(third.id).unwrap();
        assert!(state.is_idle());
    }

    #[test]
    fn stop_matching_removes_pending_and_cancels_only_matching_caller_and_key() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let mut state = Scheduler::default();
        let first = started(state.submit(request(a, 1), Mode::Queue).unwrap());
        state.submit(request(a, 1), Mode::Queue).unwrap();
        state.submit(request(a, 2), Mode::Queue).unwrap();
        state.submit(request(b, 1), Mode::Queue).unwrap();
        assert_eq!(state.stop_matching(a, |key| *key == 1), [first.id]);
        let next = state.complete(first.id).unwrap().unwrap();
        assert_eq!(next.request, request(a, 2));
        let next = state.complete(next.id).unwrap().unwrap();
        assert_eq!(next.request, request(b, 1));
    }

    #[test]
    fn draining_drops_queue_and_waits_for_confirmed_cleanup_before_resuming() {
        let a = Uuid::new_v4();
        let mut state = Scheduler::default();
        let first = started(state.submit(request(a, 1), Mode::Queue).unwrap());
        let normal = started(state.submit(request(a, 3), Mode::Start).unwrap());
        state.submit(request(a, 2), Mode::Queue).unwrap();
        let ids = state.begin_drain();
        assert!(ids.contains(&first.id) && ids.contains(&normal.id));
        assert!(state.submit(request(a, 4), Mode::Start).is_err());
        assert!(state.finish_drain().is_err());
        assert!(state.complete(first.id).unwrap().is_none());
        assert!(state.finish_drain().is_err());
        assert!(state.complete(normal.id).unwrap().is_none());
        state.finish_drain().unwrap();
        assert!(state.is_idle());
        started(state.submit(request(a, 4), Mode::Start).unwrap());
    }
}
