//! Current 179 Te: binding read, temporary name, validDevices and bounded retries.
use super::pairing_state::{
    ReceiverCategory, ReceiverPairingEvent, ReceiverPairingObservation, ReceiverPeer, Update,
};

/// Positive live device observations survive unrelated discovery errors.
/// Only a complete snapshot may use absence as evidence of disconnection.
/// Entries are real navigable workspace identities; no catalog-only devices.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ReceiverDevicesObservation {
    connected: Vec<(u32, u32)>,
    complete: bool,
}
impl ReceiverDevicesObservation {
    pub(crate) fn complete(connected: Vec<(u32, u32)>) -> Self {
        Self {
            connected,
            complete: true,
        }
    }
    pub(crate) fn partial(connected: Vec<(u32, u32)>) -> Self {
        Self {
            connected,
            complete: false,
        }
    }
    pub(crate) fn connected(&self) -> &[(u32, u32)] {
        &self.connected
    }
    pub(crate) fn is_complete(&self) -> bool {
        self.complete
    }
    fn connection(&self, product_id: u32) -> Option<bool> {
        if self.connected.iter().any(|(pid, _)| *pid == product_id) {
            Some(true)
        } else if self.complete {
            Some(false)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ReceiverDeviceRequested {
    product_id: u32,
    edition_id: u32,
    category: ReceiverCategory,
}

#[cfg(test)]
#[path = "receiver_page_state_tests.rs"]
mod tests;
impl ReceiverDeviceRequested {
    pub(crate) fn product_id(&self) -> u32 {
        self.product_id
    }
    pub(crate) fn edition_id(&self) -> u32 {
        self.edition_id
    }
    pub(crate) fn page(&self) -> crate::nav::Tab {
        match self.category {
            ReceiverCategory::Mouse => crate::nav::Tab::Performance,
            ReceiverCategory::Keyboard => crate::nav::Tab::Customize,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PageRetry(u64);

#[derive(Default)]
pub(super) struct ReceiverPageState {
    pub(super) active: bool,
    querying: Option<u64>,
    retry: Option<PageRetry>,
    attempts: u8,
    devices: ReceiverDevicesObservation,
    peers: Vec<ReceiverPeer>,
    resolved: bool,
    pub(super) loading: bool,
    pub(super) failed: bool,
}
impl ReceiverPageState {
    pub(super) fn peer(&self) -> Option<&ReceiverPeer> {
        self.peers.first()
    }
    pub(super) fn temporary(&self) -> bool {
        self.peer().is_some() && !self.resolved
    }
    pub(super) fn connected(&self) -> Option<bool> {
        let peer = self.peer()?;
        self.devices.connection(peer.product_id())
    }
    fn matched_edition(&self) -> bool {
        self.peer().is_some_and(|peer| {
            peer.edition_id().is_some_and(|edition| {
                self.devices
                    .connected()
                    .contains(&(peer.product_id(), edition))
            })
        })
    }
    pub(super) fn observe_devices(&mut self, devices: ReceiverDevicesObservation) -> bool {
        if self.devices == devices {
            return false;
        }
        self.devices = devices;
        if self.matched_edition() {
            self.resolved = true;
            self.loading = false;
            self.retry = None;
        }
        true
    }
    pub(super) fn navigation(&self) -> Option<ReceiverDeviceRequested> {
        if self.connected() != Some(true) {
            return None;
        }
        let peer = self.peer()?;
        let mut candidates = self.devices.connected().iter().filter(|(pid, edition)| {
            *pid == peer.product_id()
                && peer
                    .edition_id()
                    .is_none_or(|expected| expected == *edition)
        });
        let &(product_id, edition_id) = candidates.next()?;
        if candidates.next().is_some() {
            return None;
        }
        Some(ReceiverDeviceRequested {
            product_id,
            edition_id,
            category: peer.category()?,
        })
    }
    pub(super) fn begin(&mut self) -> Option<ReceiverPairingEvent> {
        if !self.active {
            return None;
        }
        self.attempts = 0;
        self.request()
    }
    fn request(&mut self) -> Option<ReceiverPairingEvent> {
        if !self.active {
            return None;
        }
        let event = ReceiverPairingEvent::query();
        self.querying = Some(event.session());
        self.retry = None;
        self.failed = false;
        self.loading = true;
        Some(event)
    }
    pub(super) fn suspend(&mut self) -> Option<ReceiverPairingEvent> {
        let had_request = self.querying.take().is_some() || self.retry.is_some();
        self.retry = None;
        self.loading = false;
        had_request.then(ReceiverPairingEvent::cancel)
    }
    pub(super) fn retry(&self) -> Option<PageRetry> {
        self.retry
    }
    pub(super) fn resume_retry(&mut self, ticket: PageRetry) -> Option<ReceiverPairingEvent> {
        if self.retry != Some(ticket) {
            return None;
        }
        self.request()
    }
    pub(super) fn observe(&mut self, observation: &ReceiverPairingObservation) -> bool {
        if self.querying != Some(observation.session) {
            return false;
        }
        match &observation.update {
            Update::Bindings(peers) => {
                self.querying = None;
                self.apply_bindings(peers.clone());
                if self.temporary() && self.connected() == Some(true) && self.attempts < 30 {
                    self.attempts += 1;
                    self.retry = Some(PageRetry(observation.session));
                    self.loading = true;
                }
            }
            Update::Failed(super::pairing_state::ReceiverOperation::Bindings) => {
                self.querying = None;
                self.retry = None;
                self.loading = false;
                self.failed = true;
            }
            _ => return false,
        }
        true
    }
    fn apply_bindings(&mut self, peers: Vec<ReceiverPeer>) {
        let same =
            self.peer().map(ReceiverPeer::identity) == peers.first().map(ReceiverPeer::identity);
        self.peers = peers;
        self.resolved = (same && self.resolved) || self.matched_edition();
        self.failed = false;
        self.loading = false;
        self.retry = None;
    }
    /// The caller has already validated the dialog's generation and operation.
    /// Scan candidates and unsent Bind/Unbind intents never reach this branch.
    pub(super) fn observe_dialog_result(&mut self, observation: &ReceiverPairingObservation) {
        match &observation.update {
            Update::Bindings(peers) => self.apply_bindings(peers.clone()),
            Update::Bound(peer) => {
                self.apply_bindings(vec![peer.clone()]);
                self.resolved = true;
            }
            Update::Unbound => self.apply_bindings(Vec::new()),
            _ => {}
        }
    }
}
