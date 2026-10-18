//! Held inter-device mappings follow target identity and runtime readiness.

use crate::model::SetupStatus;

/// A runtime observation does not acknowledge a device mapping write.
#[derive(Clone, Debug)]
pub struct TargetObservation<Owner> {
    owner: Owner,
    setup_status: SetupStatus,
}

impl<Owner> TargetObservation<Owner> {
    pub fn new(owner: Owner, setup_status: SetupStatus) -> Self {
        Self {
            owner,
            setup_status,
        }
    }

    pub fn owner(&self) -> &Owner {
        &self.owner
    }
}

/// Dispatch work caused by a target lifecycle change, independent of UI refreshes.
#[derive(Debug)]
#[must_use]
pub struct TargetTransition<Owner> {
    released: Option<Owner>,
    replayed: Option<Owner>,
}

impl<Owner> TargetTransition<Owner> {
    pub fn released(&self) -> Option<&Owner> {
        self.released.as_ref()
    }

    pub fn replayed(&self) -> Option<&Owner> {
        self.replayed.as_ref()
    }
}

/// Retains the recipient of a held mapping for replay and eventual release.
/// `Owner` identifies the retained entity, physical workspace and connection.
#[derive(Debug)]
pub struct HeldInterdeviceTarget<Owner> {
    target: Option<TargetObservation<Owner>>,
}

impl<Owner: Clone + Eq> HeldInterdeviceTarget<Owner> {
    /// The initial recipient has already received the original press callback.
    pub fn new(target: Option<TargetObservation<Owner>>) -> Self {
        Self { target }
    }

    pub fn owner(&self) -> Option<&Owner> {
        self.target.as_ref().map(|target| &target.owner)
    }

    /// Current middleware replays held OTFS when setupStatus changes to ready.
    /// An owner replacement also releases the outgoing recipient before dispatch.
    pub fn observe(&mut self, target: Option<TargetObservation<Owner>>) -> TargetTransition<Owner> {
        let same_owner = match (&self.target, &target) {
            (Some(previous), Some(current)) => previous.owner == current.owner,
            (None, None) => true,
            _ => false,
        };
        let released = (!same_owner)
            .then(|| self.target.as_ref().map(|target| target.owner.clone()))
            .flatten();
        let replayed = target.as_ref().and_then(|current| {
            let became_ready = self.target.as_ref().is_some_and(|previous| {
                previous.setup_status != SetupStatus::Ready
                    && current.setup_status == SetupStatus::Ready
            });
            (!same_owner || became_ready).then(|| current.owner.clone())
        });
        self.target = target;
        TargetTransition { released, replayed }
    }

    pub fn release(self) -> Option<Owner> {
        self.target.map(|target| target.owner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Owner = (u64, &'static str);

    fn observation(owner: Owner, setup_status: SetupStatus) -> Option<TargetObservation<Owner>> {
        Some(TargetObservation::new(owner, setup_status))
    }

    #[test]
    fn unrelated_navigation_and_draft_refreshes_do_not_replay_a_held_mapping() {
        let owner = (1, "target-a");
        let mut held = HeldInterdeviceTarget::new(observation(owner, SetupStatus::Ready));
        for _ in 0..4 {
            let change = held.observe(observation(owner, SetupStatus::Ready));
            assert_eq!(change.released(), None);
            assert_eq!(change.replayed(), None);
        }
        assert_eq!(held.release(), Some(owner));
    }

    #[test]
    fn only_a_real_transition_to_ready_replays_for_the_same_owner() {
        let owner = (1, "target-a");
        let mut held = HeldInterdeviceTarget::new(observation(owner, SetupStatus::Initializing));
        for status in [
            SetupStatus::Unknown,
            SetupStatus::Waiting,
            SetupStatus::Error,
        ] {
            let change = held.observe(observation(owner, status));
            assert_eq!(change.released(), None);
            assert_eq!(change.replayed(), None);
        }
        let ready = held.observe(observation(owner, SetupStatus::Ready));
        assert_eq!(ready.released(), None);
        assert_eq!(ready.replayed(), Some(&owner));
        assert_eq!(
            held.observe(observation(owner, SetupStatus::Ready))
                .replayed(),
            None
        );
        assert_eq!(
            held.observe(observation(owner, SetupStatus::Initializing))
                .replayed(),
            None
        );
        assert_eq!(
            held.observe(observation(owner, SetupStatus::Ready))
                .replayed(),
            Some(&owner)
        );
    }

    #[test]
    fn replacing_an_entity_with_the_same_device_releases_the_original_owner() {
        let original = (1, "target-a");
        let replacement = (2, "target-a");
        let mut held = HeldInterdeviceTarget::new(observation(original, SetupStatus::Ready));
        let change = held.observe(observation(replacement, SetupStatus::Ready));
        assert_eq!(change.released(), Some(&original));
        assert_eq!(change.replayed(), Some(&replacement));
        assert_eq!(held.release(), Some(replacement));
    }

    #[test]
    fn changing_the_physical_identity_of_an_entity_is_an_owner_replacement() {
        let original = (1, "target-a");
        let replacement = (1, "target-b");
        let mut held = HeldInterdeviceTarget::new(observation(original, SetupStatus::Ready));
        let change = held.observe(observation(replacement, SetupStatus::Ready));
        assert_eq!(change.released(), Some(&original));
        assert_eq!(change.replayed(), Some(&replacement));
    }

    #[test]
    fn a_batched_reconnect_with_unchanged_readiness_is_a_new_target_connection() {
        let original = (1, "target-a", 0);
        let replacement = (1, "target-a", 2);
        let mut held =
            HeldInterdeviceTarget::new(Some(TargetObservation::new(original, SetupStatus::Ready)));
        let change = held.observe(Some(TargetObservation::new(
            replacement,
            SetupStatus::Ready,
        )));
        assert_eq!(change.released(), Some(&original));
        assert_eq!(change.replayed(), Some(&replacement));
        assert_eq!(held.release(), Some(replacement));
    }

    #[test]
    fn disconnect_releases_once_and_reconnection_dispatches_once() {
        let owner = (1, "target-a");
        let mut held = HeldInterdeviceTarget::new(observation(owner, SetupStatus::Ready));
        let disconnected = held.observe(None);
        assert_eq!(disconnected.released(), Some(&owner));
        assert_eq!(disconnected.replayed(), None);
        let still_disconnected = held.observe(None);
        assert_eq!(still_disconnected.released(), None);
        assert_eq!(still_disconnected.replayed(), None);
        let connected = held.observe(observation(owner, SetupStatus::Ready));
        assert_eq!(connected.released(), None);
        assert_eq!(connected.replayed(), Some(&owner));
        assert_eq!(held.release(), Some(owner));
    }

    #[test]
    fn an_initially_absent_target_is_retained_without_fabricating_a_dispatch() {
        let owner = (1, "target-a");
        let mut held = HeldInterdeviceTarget::new(None);
        let absent = held.observe(None);
        assert_eq!(absent.released(), None);
        assert_eq!(absent.replayed(), None);
        let discovered = held.observe(observation(owner, SetupStatus::Initializing));
        assert_eq!(discovered.released(), None);
        assert_eq!(discovered.replayed(), Some(&owner));
        let ready = held.observe(observation(owner, SetupStatus::Ready));
        assert_eq!(ready.released(), None);
        assert_eq!(ready.replayed(), Some(&owner));
    }

    #[test]
    fn a_callback_failure_does_not_turn_an_unchanged_observation_into_a_retry() {
        let owner = (1, "target-a");
        let mut held = HeldInterdeviceTarget::new(None);
        assert_eq!(
            held.observe(observation(owner, SetupStatus::Ready))
                .replayed(),
            Some(&owner)
        );
        // A failed adapter dispatch is not a new runtime readiness observation.
        let unchanged = held.observe(observation(owner, SetupStatus::Ready));
        assert_eq!(unchanged.released(), None);
        assert_eq!(unchanged.replayed(), None);
    }

    #[test]
    fn releasing_while_the_target_is_absent_has_no_recipient() {
        let held: HeldInterdeviceTarget<Owner> = HeldInterdeviceTarget::new(None);
        assert_eq!(held.release(), None);
    }
}
