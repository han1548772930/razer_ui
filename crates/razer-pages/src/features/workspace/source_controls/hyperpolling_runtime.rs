//! PID179's source UI state is independent of actual GET/SET diagnostics.
use super::*;
use razer_device::hyperpolling_receiver::Submission;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

static REQUESTS: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HyperpollingOrigin {
    User,
    Initialization,
}

#[derive(Clone)]
#[non_exhaustive]
pub struct HyperpollingIndicatorRequested {
    epoch: u64,
    sequence: u64,
    status: u8,
    origin: HyperpollingOrigin,
    valid: Arc<AtomicBool>,
}
impl HyperpollingIndicatorRequested {
    pub fn epoch(&self) -> u64 {
        self.epoch
    }
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn status(&self) -> u8 {
        self.status
    }
    pub fn origin(&self) -> HyperpollingOrigin {
        self.origin
    }
    pub fn guard(&self) -> Arc<AtomicBool> {
        self.valid.clone()
    }
    pub fn is_valid(&self) -> bool {
        self.valid.load(Ordering::Acquire)
    }
    fn cancel(&self) {
        self.valid.store(false, Ordering::Release);
    }
}

#[derive(Default)]
pub(super) struct State {
    epoch: Option<u64>,
    user_edited: bool,
    pending: Option<HyperpollingIndicatorRequested>,
    diagnostic: Option<Submission>,
}
impl State {
    pub(super) fn begin(&mut self, epoch: Option<u64>) {
        if self.epoch == epoch {
            return;
        }
        if let Some(pending) = self.pending.take() {
            pending.cancel();
        }
        self.epoch = epoch;
        self.user_edited = false;
        self.diagnostic = None;
    }
    fn request(
        &mut self,
        status: u8,
        origin: HyperpollingOrigin,
    ) -> Option<HyperpollingIndicatorRequested> {
        let epoch = self.epoch?;
        if origin == HyperpollingOrigin::Initialization && (status == 0 || self.user_edited) {
            return None;
        }
        if let Some(pending) = self.pending.take() {
            pending.cancel();
        }
        self.user_edited |= origin == HyperpollingOrigin::User;
        let request = HyperpollingIndicatorRequested {
            epoch,
            sequence: REQUESTS.fetch_add(1, Ordering::Relaxed),
            status,
            origin,
            valid: Arc::new(AtomicBool::new(true)),
        };
        self.pending = Some(request.clone());
        Some(request)
    }
    fn matches(&self, request: &HyperpollingIndicatorRequested) -> bool {
        self.epoch == Some(request.epoch)
            && request.is_valid()
            && self
                .pending
                .as_ref()
                .is_some_and(|pending| pending.sequence == request.sequence)
    }
    fn finish(
        &mut self,
        request: &HyperpollingIndicatorRequested,
        diagnostic: Option<Submission>,
    ) -> bool {
        if !self.matches(request) {
            return false;
        }
        self.pending = None;
        self.diagnostic = diagnostic;
        true
    }
}
impl Drop for State {
    fn drop(&mut self) {
        if let Some(pending) = self.pending.take() {
            pending.cancel();
        }
    }
}
impl EventEmitter<HyperpollingIndicatorRequested> for SourceControls {}
impl SourceControls {
    pub fn begin_hyperpolling_connection(&mut self, epoch: Option<u64>, cx: &mut Context<Self>) {
        if self.spec.product_id != 179 {
            return;
        }
        self.receiver.runtime.begin(epoch);
        cx.notify();
    }
    pub fn hyperpolling_request_matches(&self, request: &HyperpollingIndicatorRequested) -> bool {
        self.spec.product_id == 179 && self.receiver.runtime.matches(request)
    }
    pub fn initialize_hyperpolling_indicator(
        &mut self,
        epoch: u64,
        status: u8,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != 179 || self.receiver.runtime.epoch != Some(epoch) {
            return;
        }
        if let Some(request) = self
            .receiver
            .runtime
            .request(status, HyperpollingOrigin::Initialization)
        {
            cx.emit(request);
        }
    }
    pub fn publish_hyperpolling_indicator(
        &mut self,
        request: &HyperpollingIndicatorRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.hyperpolling_request_matches(request) {
            return false;
        }
        // Initialization reaches the UI after taskMaker's local save/enqueue;
        // user selection has already passed the source optimistic reducer.
        if request.origin == HyperpollingOrigin::Initialization {
            let value = serde_json::json!(request.status);
            if let Some(current) = self.draft.pointer_mut("/runtime/indicatorLedStatus") {
                *current = value;
                self.receiver.restart_indicator();
                self.sync(window, cx);
                cx.notify();
            }
        }
        true
    }
    pub fn finish_hyperpolling_indicator(
        &mut self,
        request: &HyperpollingIndicatorRequested,
        diagnostic: Option<Submission>,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id == 179 && self.receiver.runtime.finish(request, diagnostic) {
            cx.notify();
        }
    }
    pub(super) fn request_hyperpolling_indicator(&mut self, value: &Value, cx: &mut Context<Self>) {
        if self.spec.product_id != 179 {
            return;
        }
        if let Some(status) = value.as_u64().and_then(|value| u8::try_from(value).ok())
            && let Some(request) = self
                .receiver
                .runtime
                .request(status, HyperpollingOrigin::User)
        {
            cx.emit(request);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn hyperpolling_runtime_old_connection_and_replaced_request_cannot_complete() {
        let mut state = State::default();
        state.begin(Some(1));
        let first = state.request(2, HyperpollingOrigin::User).unwrap();
        let newer = state.request(3, HyperpollingOrigin::User).unwrap();
        assert!(!first.is_valid());
        assert!(!state.finish(&first, None));
        state.begin(Some(2));
        assert!(!newer.is_valid());
        assert!(!state.finish(&newer, None));
    }
    #[::core::prelude::v1::test]
    fn hyperpolling_runtime_truthy_initialization_never_overwrites_newer_user_selection() {
        let mut state = State::default();
        state.begin(Some(1));
        assert!(
            state
                .request(0, HyperpollingOrigin::Initialization)
                .is_none()
        );
        let request = state
            .request(254, HyperpollingOrigin::Initialization)
            .unwrap();
        assert_eq!(request.status(), 254);
        let user = state.request(2, HyperpollingOrigin::User).unwrap();
        assert!(!request.is_valid());
        assert!(
            state
                .request(3, HyperpollingOrigin::Initialization)
                .is_none()
        );
        assert!(state.finish(&user, None));
        assert!(state.diagnostic.is_none());
    }
    #[::core::prelude::v1::test]
    fn hyperpolling_runtime_dropping_the_page_cancels_its_actual_request_guard() {
        let mut state = State::default();
        state.begin(Some(1));
        let request = state.request(2, HyperpollingOrigin::User).unwrap();
        assert!(request.is_valid());
        drop(state);
        assert!(!request.is_valid());
    }
}
