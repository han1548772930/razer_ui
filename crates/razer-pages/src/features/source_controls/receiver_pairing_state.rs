//! 179/9473: J, W, re and se. Service observations and local intents are separate.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) fn next_generation() -> u64 {
    static GENERATION: AtomicU64 = AtomicU64::new(1);
    GENERATION.fetch_add(1, Ordering::Relaxed)
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum ReceiverCategory {
    Keyboard,
    Mouse,
}
impl ReceiverCategory {
    pub(super) fn key(self) -> &'static str {
        match self {
            Self::Keyboard => "KEYBOARD",
            Self::Mouse => "MOUSE",
        }
    }
}

/// Metadata is optional: the hardware connection query only supplies PID/status.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReceiverPeer {
    product_id: u32,
    #[serde(default)]
    status: Option<u8>,
    #[serde(default)]
    dongle_id: Option<u32>,
    #[serde(default)]
    edition_id: Option<u32>,
    #[serde(default)]
    layout_id: Option<u32>,
    #[serde(default)]
    category: Option<ReceiverCategory>,
    #[serde(default)]
    product_name: BTreeMap<String, String>,
    #[serde(default)]
    name: BTreeMap<String, String>,
    #[serde(default)]
    serial_number: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}
impl ReceiverPeer {
    pub fn queried(product_id: u32, status: u8) -> Self {
        Self {
            product_id,
            status: Some(status),
            dongle_id: None,
            edition_id: None,
            layout_id: None,
            category: None,
            product_name: BTreeMap::new(),
            name: BTreeMap::new(),
            serial_number: None,
            extra: BTreeMap::new(),
        }
    }
    pub fn product_id(&self) -> u32 {
        self.product_id
    }
    pub fn status(&self) -> Option<u8> {
        self.status
    }
    pub(super) fn category(&self) -> Option<ReceiverCategory> {
        self.category
    }
    pub(super) fn edition_id(&self) -> Option<u32> {
        self.edition_id
    }
    pub(super) fn label(&self, locale: &str) -> String {
        let localized = |names: &BTreeMap<String, String>| {
            names
                .iter()
                .find(|(language, name)| language.eq_ignore_ascii_case(locale) && !name.is_empty())
                .map(|(_, name)| name.clone())
        };
        localized(&self.product_name)
            .or_else(|| localized(&self.name))
            .or_else(|| {
                self.product_name
                    .get("en")
                    .filter(|name| !name.is_empty())
                    .cloned()
            })
            .or_else(|| self.name.get("en").filter(|name| !name.is_empty()).cloned())
            .unwrap_or_else(|| format!("PID {}", self.product_id))
    }
    pub(super) fn identity(&self) -> String {
        format!(
            "{}:{:?}:{:?}:{}",
            self.product_id,
            self.edition_id,
            self.layout_id,
            self.serial_number.as_deref().unwrap_or("")
        )
    }
    pub(super) fn image_identity(&self) -> Option<(u32, u32, u32)> {
        Some((self.product_id, self.edition_id?, self.layout_id?))
    }
    fn unbind_product_id(&self) -> u32 {
        self.dongle_id
            .filter(|id| *id != 0)
            .unwrap_or(self.product_id)
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub enum ReceiverPairingIntent {
    QueryBindings,
    Scan(ReceiverCategory),
    Bind(ReceiverPeer),
    Unbind(u32),
    Cancel,
}
#[derive(Clone, Debug)]
pub struct ReceiverPairingEvent {
    // Operation generation, including local cancellation; not just modal lifetime.
    session: u64,
    intent: ReceiverPairingIntent,
}
impl ReceiverPairingEvent {
    pub(super) fn query() -> Self {
        Self {
            session: next_generation(),
            intent: ReceiverPairingIntent::QueryBindings,
        }
    }
    pub(super) fn cancel() -> Self {
        Self {
            session: next_generation(),
            intent: ReceiverPairingIntent::Cancel,
        }
    }
    pub fn session(&self) -> u64 {
        self.session
    }
    pub fn intent(&self) -> &ReceiverPairingIntent {
        &self.intent
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReceiverOperation {
    Bindings,
    Scan,
    Bind,
    Unbind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReceiverProgress {
    Scanning(ReceiverCategory),
    Pairing,
    Unbinding,
    Upgrading(ReceiverCategory),
}
#[derive(Clone, Debug)]
pub(super) enum Update {
    Bindings(Vec<ReceiverPeer>),
    Scanned(Vec<ReceiverPeer>),
    Bound(ReceiverPeer),
    Unbound,
    FirmwareVersion(Option<String>),
    Failed(ReceiverOperation),
    Progress(ReceiverProgress),
}
/// Only a publisher with a real result may create these observations.
#[derive(Clone, Debug)]
pub struct ReceiverPairingObservation {
    pub(super) session: u64,
    pub(super) update: Update,
}
impl ReceiverPairingObservation {
    pub fn bindings(session: u64, peers: Vec<ReceiverPeer>) -> Self {
        Self {
            session,
            update: Update::Bindings(peers),
        }
    }
    pub fn scanned(session: u64, peers: Vec<ReceiverPeer>) -> Self {
        Self {
            session,
            update: Update::Scanned(peers),
        }
    }
    pub fn bound(session: u64, peer: ReceiverPeer) -> Self {
        Self {
            session,
            update: Update::Bound(peer),
        }
    }
    pub fn unbound(session: u64) -> Self {
        Self {
            session,
            update: Update::Unbound,
        }
    }
    pub fn firmware_version(session: u64, version: Option<String>) -> Self {
        Self {
            session,
            update: Update::FirmwareVersion(version),
        }
    }
    pub fn failed(session: u64, operation: ReceiverOperation) -> Self {
        Self {
            session,
            update: Update::Failed(operation),
        }
    }
    pub fn progress(session: u64, progress: ReceiverProgress) -> Self {
        Self {
            session,
            update: Update::Progress(progress),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Status {
    #[default]
    Loading,
    Ready,
    Upgrading,
    Scanning,
    Scanned,
    Pairing,
    Paired,
    PairFailed,
    ConfirmUnpair,
    Unpairing,
    Unpaired,
    UnpairFailed,
}
/// The current se failure callbacks restore presentation after four seconds.
/// A ticket belongs to one accepted result, never an unsent UI intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FailureRecovery {
    session: u64,
    status: Status,
}
impl FailureRecovery {
    pub(super) fn delay(self) -> std::time::Duration {
        std::time::Duration::from_secs(4)
    }
}
/// Te closes the utility one second after an accepted Bind/Unbind result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SuccessClose {
    session: u64,
    status: Status,
}
impl SuccessClose {
    pub(super) fn delay(self) -> std::time::Duration {
        std::time::Duration::from_secs(1)
    }
}
#[derive(Default)]
pub(super) struct PairingState {
    session: u64,
    open: bool,
    pub(super) status: Status,
    pub(super) category: Option<ReceiverCategory>,
    pub(super) bound: Vec<ReceiverPeer>,
    pub(super) candidates: Vec<ReceiverPeer>,
    pub(super) selected: usize,
    pub(super) firmware_version: Option<String>,
    pub(super) pending: Option<ReceiverPairingIntent>,
    // Retained after a publisher acknowledges progress, until its final result.
    active: Option<ReceiverPairingIntent>,
    recovery: Option<FailureRecovery>,
    success_close: Option<SuccessClose>,
    pub(super) failure: Option<ReceiverOperation>,
}
impl PairingState {
    pub(super) fn success_close(&self) -> Option<SuccessClose> {
        self.success_close
    }
    pub(super) fn can_close_success(&self, ticket: SuccessClose) -> bool {
        self.open
            && self.success_close == Some(ticket)
            && self.session == ticket.session
            && self.status == ticket.status
            && self.active.is_none()
    }
    pub(super) fn recovery(&self) -> Option<FailureRecovery> {
        self.recovery
    }
    pub(super) fn recover_failure(&mut self, ticket: FailureRecovery) -> bool {
        if !self.open
            || self.recovery != Some(ticket)
            || self.session != ticket.session
            || self.status != ticket.status
            || self.active.is_some()
        {
            return false;
        }
        self.recovery = None;
        self.failure = None;
        match ticket.status {
            Status::PairFailed => {
                self.status = Status::Ready;
                self.bound.clear();
                self.candidates.clear();
                self.selected = 0;
            }
            Status::UnpairFailed => self.status = Status::Paired,
            _ => return false,
        }
        true
    }
    pub(super) fn is_open(&self) -> bool {
        self.open
    }
    pub(super) fn open(&mut self) -> ReceiverPairingEvent {
        let session = self.session;
        *self = Self {
            session,
            open: true,
            ..Self::default()
        };
        self.request(ReceiverPairingIntent::QueryBindings)
    }
    pub(super) fn close(&mut self) -> ReceiverPairingEvent {
        self.open = false;
        self.invalidate()
    }
    fn invalidate(&mut self) -> ReceiverPairingEvent {
        self.session = next_generation();
        self.pending = None;
        self.active = None;
        self.recovery = None;
        self.success_close = None;
        ReceiverPairingEvent {
            session: self.session,
            intent: ReceiverPairingIntent::Cancel,
        }
    }
    fn request(&mut self, intent: ReceiverPairingIntent) -> ReceiverPairingEvent {
        self.session = next_generation();
        self.failure = None;
        self.recovery = None;
        self.success_close = None;
        self.pending = Some(intent.clone());
        self.active = Some(intent.clone());
        ReceiverPairingEvent {
            session: self.session,
            intent,
        }
    }
    pub(super) fn refresh(&mut self) -> Option<ReceiverPairingEvent> {
        if !self.open {
            return None;
        }
        Some(self.request(ReceiverPairingIntent::QueryBindings))
    }
    pub(super) fn select(&mut self, index: usize) {
        if self.open
            && self.status == Status::Scanned
            && index < self.candidates.len()
            && self.pending.is_none()
        {
            self.selected = index;
        }
    }
    pub(super) fn scan(&mut self, category: ReceiverCategory) -> Option<ReceiverPairingEvent> {
        if !self.open
            || self.pending.is_some()
            || !self.bound.is_empty()
            || !matches!(
                self.status,
                Status::Ready
                    | Status::Scanned
                    | Status::PairFailed
                    | Status::UnpairFailed
                    | Status::Unpaired
            )
            || (self.status == Status::Ready
                && self.firmware_version.as_deref().is_none_or(str::is_empty))
        {
            return None;
        }
        Some(self.request(ReceiverPairingIntent::Scan(category)))
    }
    pub(super) fn bind(&mut self) -> Option<ReceiverPairingEvent> {
        if !self.open || self.status != Status::Scanned || self.pending.is_some() {
            return None;
        }
        let peer = self.candidates.get(self.selected)?.clone();
        Some(self.request(ReceiverPairingIntent::Bind(peer)))
    }
    pub(super) fn cancel_selection(&mut self) -> Option<ReceiverPairingEvent> {
        if !self.open || self.status != Status::Scanned {
            return None;
        }
        // re.cancelSelect asks for fresh binding info; it does not invent LOADED.
        Some(self.request(ReceiverPairingIntent::QueryBindings))
    }
    pub(super) fn confirm_unpair(&mut self) {
        if self.open
            && self.status == Status::Paired
            && !self.bound.is_empty()
            && self.pending.is_none()
        {
            self.status = Status::ConfirmUnpair;
            self.success_close = None;
        }
    }
    pub(super) fn cancel_unpair(&mut self) -> Option<ReceiverPairingEvent> {
        if self.open && self.status == Status::ConfirmUnpair {
            self.status = Status::Paired;
            return Some(self.invalidate());
        }
        None
    }
    pub(super) fn unbind(&mut self) -> Option<ReceiverPairingEvent> {
        if !self.open || self.status != Status::ConfirmUnpair || self.pending.is_some() {
            return None;
        }
        let id = self.bound.first()?.unbind_product_id();
        Some(self.request(ReceiverPairingIntent::Unbind(id)))
    }
    pub(super) fn cancel_pending(&mut self) -> Option<ReceiverPairingEvent> {
        // Only the local, unacknowledged intent is removed. No device response.
        (self.open && self.pending.is_some()).then(|| self.invalidate())
    }
    pub(super) fn observe(
        &mut self,
        observation: ReceiverPairingObservation,
    ) -> (bool, Option<ReceiverPairingEvent>) {
        if !self.open || observation.session != self.session {
            return (false, None);
        }
        // A generation also has an operation type. A delayed or duplicate result
        // cannot overwrite a newer request or trigger automatic binding again.
        let expected = match &observation.update {
            Update::Bindings(_) => Some(ReceiverOperation::Bindings),
            Update::Scanned(_) => Some(ReceiverOperation::Scan),
            Update::Bound(_) => Some(ReceiverOperation::Bind),
            Update::Unbound => Some(ReceiverOperation::Unbind),
            Update::Failed(operation) => Some(*operation),
            Update::FirmwareVersion(_) | Update::Progress(_) => None,
        };
        let active = match self.active.as_ref() {
            Some(ReceiverPairingIntent::QueryBindings) => Some(ReceiverOperation::Bindings),
            Some(ReceiverPairingIntent::Scan(_)) => Some(ReceiverOperation::Scan),
            Some(ReceiverPairingIntent::Bind(_)) => Some(ReceiverOperation::Bind),
            Some(ReceiverPairingIntent::Unbind(_)) => Some(ReceiverOperation::Unbind),
            _ => None,
        };
        if expected.is_some() && expected != active {
            return (false, None);
        }
        match observation.update {
            Update::FirmwareVersion(version) => self.firmware_version = version,
            Update::Bindings(peers) => {
                self.failure = None;
                self.pending = None;
                self.active = None;
                self.candidates.clear();
                self.selected = 0;
                self.category = peers.first().and_then(ReceiverPeer::category);
                self.status = if peers.is_empty() {
                    Status::Ready
                } else {
                    Status::Paired
                };
                self.bound = peers;
            }
            Update::Scanned(peers) => {
                self.failure = None;
                if let Some(ReceiverPairingIntent::Scan(category)) = self.active.as_ref() {
                    self.category = Some(*category);
                }
                self.pending = None;
                self.active = None;
                self.status = Status::Scanned;
                self.candidates = peers;
                self.selected = self.selected.min(self.candidates.len().saturating_sub(1));
                // se automatically requests binding for exactly one real candidate.
                // Keep the request local until the publisher acknowledges it.
                if self.candidates.len() == 1 {
                    let peer = self.candidates[0].clone();
                    return (true, Some(self.request(ReceiverPairingIntent::Bind(peer))));
                }
            }
            Update::Bound(peer) => {
                self.failure = None;
                self.pending = None;
                self.active = None;
                self.category = peer.category;
                self.bound = vec![peer];
                self.selected = 0;
                self.status = Status::Paired;
                self.success_close = Some(SuccessClose {
                    session: self.session,
                    status: self.status,
                });
            }
            Update::Unbound => {
                self.failure = None;
                self.pending = None;
                self.active = None;
                self.bound.clear();
                self.status = Status::Unpaired;
                self.success_close = Some(SuccessClose {
                    session: self.session,
                    status: self.status,
                });
            }
            Update::Failed(operation) => {
                self.failure = Some(operation);
                self.pending = None;
                self.active = None;
                self.status = match operation {
                    ReceiverOperation::Bindings => {
                        self.bound.clear();
                        self.candidates.clear();
                        Status::Ready
                    }
                    ReceiverOperation::Scan => {
                        self.bound.clear();
                        self.candidates.clear();
                        Status::Scanned
                    }
                    ReceiverOperation::Bind => Status::PairFailed,
                    ReceiverOperation::Unbind => Status::UnpairFailed,
                };
                self.recovery = matches!(
                    operation,
                    ReceiverOperation::Bind | ReceiverOperation::Unbind
                )
                .then_some(FailureRecovery {
                    session: self.session,
                    status: self.status,
                });
            }
            Update::Progress(progress) => {
                let accepted = match (progress, self.active.as_ref()) {
                    (
                        ReceiverProgress::Scanning(category),
                        Some(ReceiverPairingIntent::Scan(expected)),
                    ) if category == *expected => {
                        self.category = Some(category);
                        self.status = Status::Scanning;
                        true
                    }
                    (ReceiverProgress::Pairing, Some(ReceiverPairingIntent::Bind(_))) => {
                        self.status = Status::Pairing;
                        true
                    }
                    (ReceiverProgress::Unbinding, Some(ReceiverPairingIntent::Unbind(_))) => {
                        self.status = Status::Unpairing;
                        true
                    }
                    (
                        ReceiverProgress::Upgrading(category),
                        Some(ReceiverPairingIntent::Scan(expected)),
                    ) if category == *expected => {
                        self.category = Some(category);
                        self.status = Status::Upgrading;
                        true
                    }
                    _ => false,
                };
                if !accepted {
                    return (false, None);
                }
                self.pending = None;
            }
        }
        (true, None)
    }
}

#[cfg(test)]
#[path = "receiver_pairing_state_tests.rs"]
mod tests;
