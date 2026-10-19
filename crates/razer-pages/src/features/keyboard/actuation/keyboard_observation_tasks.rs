//! A subscription keeps its device identity until its worker cleanup is confirmed.
use gpui_kit::BackgroundExecutor;
use razer_ipc::mapping_observation::Target;
use razer_service::mapping::observation::Completion;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Identity {
    target: Target,
    generation: Uuid,
}
impl Identity {
    pub(super) fn matches_target(&self, target: Option<&Target>) -> bool {
        Some(&self.target) == target
    }
}

struct Active<S> {
    identity: Identity,
    stream: S,
}

enum Cleanup<C> {
    Pending { identity: Identity, _wait: C },
    Failed { _identity: Identity, _error: String },
}

pub(super) struct SubscriptionTasks<S, C> {
    active: Option<Active<S>>,
    cleanup: Option<Cleanup<C>>,
}

impl<S, C> Default for SubscriptionTasks<S, C> {
    fn default() -> Self {
        Self {
            active: None,
            cleanup: None,
        }
    }
}

impl<S, C> SubscriptionTasks<S, C> {
    pub(super) fn occupied(&self) -> bool {
        self.active.is_some() || self.cleanup.is_some()
    }

    pub(super) fn stream(&self) -> Option<&S> {
        self.active.as_ref().map(|active| &active.stream)
    }

    pub(super) fn release(&mut self, release: impl FnOnce(S)) {
        if let Some(active) = self.active.take() {
            release(active.stream);
        }
    }

    pub(super) fn accepts(&self, identity: &Identity) -> bool {
        self.active
            .as_ref()
            .is_some_and(|active| &active.identity == identity)
    }

    pub(super) fn start<E>(
        &mut self,
        target: Target,
        start: impl FnOnce() -> Result<S, E>,
    ) -> Result<Option<Identity>, E> {
        if self.occupied() {
            return Ok(None);
        }
        let stream = start()?;
        let identity = Identity {
            target,
            generation: Uuid::new_v4(),
        };
        self.active = Some(Active {
            identity: identity.clone(),
            stream,
        });
        Ok(Some(identity))
    }

    pub(super) fn begin_cleanup(&mut self, wait: impl FnOnce(Identity, S) -> C) {
        if self.cleanup.is_some() {
            return;
        }
        if let Some(active) = self.active.take() {
            let identity = active.identity;
            let wait = wait(identity.clone(), active.stream);
            self.cleanup = Some(Cleanup::Pending {
                identity,
                _wait: wait,
            });
        }
    }

    /// Presentation changes cannot release or replace a pending device cleanup.
    pub(super) fn finish_cleanup(
        &mut self,
        identity: &Identity,
        result: Result<(), String>,
    ) -> bool {
        if !matches!(&self.cleanup, Some(Cleanup::Pending { identity: current, .. }) if current == identity)
        {
            return false;
        }
        self.cleanup = result.err().map(|error| Cleanup::Failed {
            _identity: identity.clone(),
            _error: error,
        });
        true
    }
}

/// Joining continues even if the page and its foreground completion task close.
pub(super) fn join_stream<S: Send + 'static>(
    executor: &BackgroundExecutor,
    stream: S,
    stop: impl FnOnce(S) -> Completion + Send + 'static,
) -> async_channel::Receiver<Completion> {
    let (sender, receiver) = async_channel::bounded(1);
    executor
        .spawn(async move {
            let result = stop(stream);
            if let Err(error) = result.result() {
                eprintln!("[Mapping observation cleanup] {error}");
            }
            let _ = sender.send(result).await;
        })
        .detach();
    receiver
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};

    struct Wait(Rc<Cell<usize>>);
    impl Drop for Wait {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    fn target(container: &str) -> Target {
        Target::new(679, container.into())
    }

    #[test]
    fn subscription_repeated_stop_retains_wait_and_defers_stream_construction() {
        let dropped = Rc::new(Cell::new(0));
        let starts = Cell::new(0);
        let mut tasks = SubscriptionTasks::default();
        let create = || {
            starts.set(starts.get() + 1);
            Ok::<_, ()>(())
        };
        let identity = tasks.start(target("first"), create).unwrap().unwrap();
        assert!(identity.matches_target(Some(&target("first"))));
        assert!(!identity.matches_target(Some(&target("replacement"))));
        assert!(!identity.matches_target(None));
        tasks.begin_cleanup(|_, ()| Wait(dropped.clone()));
        tasks.begin_cleanup(|_, ()| panic!("replaced an existing cleanup"));
        assert_eq!(dropped.get(), 0);
        assert_eq!(tasks.start(target("replacement"), create).unwrap(), None);
        assert_eq!(starts.get(), 1);
        assert!(tasks.finish_cleanup(&identity, Ok(())));
        assert_eq!(dropped.get(), 1);
        assert!(
            tasks
                .start(target("replacement"), create)
                .unwrap()
                .is_some()
        );
        assert_eq!(starts.get(), 2);
    }

    #[test]
    fn subscription_failed_cleanup_retains_old_identity_across_disconnect_and_reconnect() {
        let mut tasks = SubscriptionTasks::default();
        let identity = tasks
            .start(target("first"), || Ok::<_, ()>(()))
            .unwrap()
            .unwrap();
        tasks.begin_cleanup(|_, ()| ());
        assert!(tasks.finish_cleanup(&identity, Err("unsubscribe failed".into())));
        assert!(
            matches!(&tasks.cleanup, Some(Cleanup::Failed { _identity: retained, _error: error })
            if retained == &identity && retained.target == target("first") && error == "unsubscribe failed")
        );
        for next in ["replacement", "first", "replacement"] {
            tasks.begin_cleanup(|_, ()| panic!("lost the failed cleanup"));
            assert_eq!(tasks.start(target(next), || Ok::<_, ()>(())).unwrap(), None);
        }
        assert!(!tasks.finish_cleanup(&identity, Ok(())));
        assert!(tasks.occupied());
    }

    #[test]
    fn subscription_stale_cleanup_cannot_remove_successor_with_the_same_target() {
        let mut tasks = SubscriptionTasks::default();
        let old = tasks
            .start(target("same"), || Ok::<_, ()>(()))
            .unwrap()
            .unwrap();
        tasks.begin_cleanup(|_, ()| ());
        assert!(tasks.finish_cleanup(&old, Ok(())));
        let current = tasks
            .start(target("same"), || Ok::<_, ()>(()))
            .unwrap()
            .unwrap();
        assert_ne!(old, current);
        assert!(!tasks.accepts(&old));
        assert!(tasks.accepts(&current));
        assert!(!tasks.finish_cleanup(&old, Err("late failure".into())));
        assert!(tasks.accepts(&current));
        tasks.begin_cleanup(|_, ()| ());
        assert!(!tasks.finish_cleanup(&old, Ok(())));
        assert!(tasks.occupied());
        assert!(tasks.finish_cleanup(&current, Ok(())));
        assert!(!tasks.occupied());
    }

    #[test]
    fn subscription_owner_release_transfers_active_stream_and_preserves_pending_join() {
        let releases = Cell::new(0);
        let mut tasks = SubscriptionTasks::<(), ()>::default();
        tasks
            .start(target("active"), || Ok::<_, ()>(()))
            .unwrap()
            .unwrap();
        tasks.release(|()| releases.set(releases.get() + 1));
        tasks.release(|()| panic!("stream released twice"));
        assert_eq!(releases.get(), 1);
        let identity = tasks
            .start(target("pending"), || Ok::<_, ()>(()))
            .unwrap()
            .unwrap();
        tasks.begin_cleanup(|_, ()| ());
        tasks.release(|()| panic!("pending join lost its owner"));
        assert!(tasks.occupied());
        assert!(tasks.finish_cleanup(&identity, Ok(())));
    }
}
