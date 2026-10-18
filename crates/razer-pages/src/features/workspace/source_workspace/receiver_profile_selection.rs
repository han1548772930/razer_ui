//! Receiver selection intent and serial-backed profile publication lifecycle.
use super::*;
use razer_storage::receiver_reset::ReceiverProfileSnapshot;

#[derive(Clone, Copy, Debug)]
pub struct ReceiverProfileSelectionRequested {
    generation: u64,
    initialization: bool,
}
impl ReceiverProfileSelectionRequested {
    pub fn generation(self) -> u64 {
        self.generation
    }
    pub fn is_initialization(self) -> bool {
        self.initialization
    }
}

struct Pending {
    request: ReceiverProfileSelectionRequested,
    captured_profile: String,
    published_profile: Option<String>,
}

#[derive(Default)]
pub(super) struct State {
    generation: u64,
    closing: bool,
    pending: Option<Pending>,
    initialization_requested: bool,
    error: Option<String>,
}
impl State {
    fn request(
        &mut self,
        profile: &str,
        initialization: bool,
    ) -> Option<ReceiverProfileSelectionRequested> {
        if self.closing {
            return None;
        }
        self.generation = self.generation.wrapping_add(1);
        let request = ReceiverProfileSelectionRequested {
            generation: self.generation,
            initialization,
        };
        self.pending = Some(Pending {
            request,
            captured_profile: profile.into(),
            published_profile: None,
        });
        self.error = None;
        Some(request)
    }

    fn matches(&self, generation: u64, captured: &str, current: &str) -> bool {
        self.pending.as_ref().is_some_and(|pending| {
            pending.request.generation == generation
                && pending.captured_profile == captured
                && pending
                    .published_profile
                    .as_deref()
                    .unwrap_or(&pending.captured_profile)
                    == current
        })
    }

    fn accepts_publication(
        &self,
        request: ReceiverProfileSelectionRequested,
        active: &str,
        next: &str,
    ) -> bool {
        self.pending.as_ref().is_some_and(|pending| {
            pending.request.generation == request.generation
                && pending.request.initialization == request.initialization
                && self.matches(request.generation, &pending.captured_profile, active)
                && (request.initialization || next == pending.captured_profile)
        })
    }

    fn published(&mut self, request: ReceiverProfileSelectionRequested, active: String) {
        if let Some(pending) = &mut self.pending
            && pending.request.generation == request.generation
        {
            pending.published_profile = Some(active);
        }
    }

    pub(super) fn invalidate(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.pending = None;
        self.initialization_requested = false;
        self.error = None;
    }

    pub(super) fn prepare_exit(&mut self) {
        self.closing = true;
    }

    pub(super) fn cancel_exit(&mut self) {
        self.closing = false;
    }
}

impl SourceProductWorkspace {
    pub(super) fn request_receiver_profile_selection(&self, cx: &mut Context<Self>) {
        if self.exiting || !matches!(self.device.product_id, 164 | 241) {
            return;
        }
        let Some(request) = self
            .receiver_profile_selection
            .borrow_mut()
            .request(&self.device.active_profile, false)
        else {
            return;
        };
        cx.emit(WorkspaceEvent::ReceiverProfileSelectionRequested(request));
    }

    pub(super) fn sync_receiver_profile_initialization(&self, cx: &mut Context<Self>) {
        use razer_model::model::DeviceConnectionObservation;
        if self.exiting
            || !matches!(self.device.product_id, 164 | 241)
            || self.device.real_product_id != self.device.product_id
            || !matches!(
                self.device.dashboard.connection_observation,
                Some(
                    DeviceConnectionObservation::UsbPresent
                        | DeviceConnectionObservation::HidPresent
                )
            )
        {
            return;
        }
        let has_document = match ReceiverProfileSnapshot::has_document(&self.device) {
            Ok(present) => present,
            Err(error) => {
                eprintln!("[receiver profile initialization] {error:#}");
                return;
            }
        };
        if has_document {
            return;
        }
        let mut state = self.receiver_profile_selection.borrow_mut();
        if state.initialization_requested || state.pending.is_some() {
            return;
        }
        let Some(request) = state.request(&self.device.active_profile, true) else {
            return;
        };
        state.initialization_requested = true;
        drop(state);
        cx.emit(WorkspaceEvent::ReceiverProfileSelectionRequested(request));
    }

    pub fn receiver_profile_selection_matches(&self, generation: u64, guid: &str, _: &App) -> bool {
        matches!(self.device.product_id, 164 | 241)
            && self.receiver_profile_selection.borrow().matches(
                generation,
                guid,
                &self.device.active_profile,
            )
    }

    pub fn publish_receiver_selection_profile(
        &mut self,
        request: ReceiverProfileSelectionRequested,
        snapshot: ReceiverProfileSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let accepted = self
            .receiver_profile_selection
            .borrow()
            .accepts_publication(
                request,
                &self.device.active_profile,
                snapshot.active_profile(),
            );
        if !accepted {
            return false;
        }
        let active_profile = snapshot.active_profile().to_owned();
        if !self.publish_receiver_snapshot(snapshot, true, window, cx) {
            return false;
        }
        self.receiver_profile_selection
            .borrow_mut()
            .published(request, active_profile);
        true
    }

    pub fn finish_receiver_profile_selection(
        &mut self,
        request: ReceiverProfileSelectionRequested,
        error: Option<String>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut state = self.receiver_profile_selection.borrow_mut();
        let accepted = state.pending.as_ref().is_some_and(|pending| {
            pending.request.generation == request.generation
                && pending.request.initialization == request.initialization
                && state.matches(
                    request.generation,
                    &pending.captured_profile,
                    &self.device.active_profile,
                )
        });
        if !accepted {
            return;
        }
        state.pending = None;
        if let Some(error) = &error {
            eprintln!("[receiver profile selection] {error}");
        }
        state.error = error;
        drop(state);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::State;

    #[test]
    fn newer_selection_and_connection_changes_reject_late_publication() {
        let mut state = State::default();
        let first = state.request("first", false).unwrap();
        assert!(state.matches(first.generation(), "first", "first"));
        let next = state.request("next", false).unwrap();
        assert!(!state.matches(first.generation(), "first", "first"));
        assert!(!state.matches(next.generation(), "first", "next"));
        assert!(!state.matches(next.generation(), "next", "different"));
        assert!(state.matches(next.generation(), "next", "next"));
        state.invalidate();
        assert!(!state.matches(next.generation(), "next", "next"));
    }

    #[test]
    fn initialization_keeps_captured_owner_after_publishing_the_real_profile() {
        let mut state = State::default();
        let request = state.request("preview", true).unwrap();
        assert!(request.is_initialization());
        assert!(state.accepts_publication(request, "preview", "serial-profile"));
        state.published(request, "serial-profile".into());
        assert!(state.matches(request.generation(), "preview", "serial-profile"));
        assert!(!state.matches(request.generation(), "preview", "preview"));
        assert!(!state.matches(request.generation(), "serial-profile", "serial-profile"));
        let selection = state.request("next-profile", false).unwrap();
        assert!(!state.accepts_publication(selection, "next-profile", "different"));
        assert!(state.accepts_publication(selection, "next-profile", "next-profile"));
    }

    #[test]
    fn exit_closes_selection_admission_and_preserves_the_accepted_publication() {
        let mut state = State::default();
        let request = state.request("preview", true).unwrap();
        state.prepare_exit();
        assert!(state.request("new-profile", false).is_none());
        assert!(state.matches(request.generation(), "preview", "preview"));
        assert!(state.accepts_publication(request, "preview", "serial-profile"));
        state.published(request, "serial-profile".into());
        assert!(state.matches(request.generation(), "preview", "serial-profile"));
        state.cancel_exit();
        assert!(state.matches(request.generation(), "preview", "serial-profile"));
        assert!(state.request("next-profile", false).is_some());
    }

    #[test]
    fn connection_changes_still_reject_accepted_selection_during_exit() {
        let mut state = State::default();
        let request = state.request("profile", false).unwrap();
        state.prepare_exit();
        state.invalidate();
        assert!(!state.matches(request.generation(), "profile", "profile"));
        assert!(!state.accepts_publication(request, "profile", "profile"));
        assert!(state.request("profile", false).is_none());
    }
}
