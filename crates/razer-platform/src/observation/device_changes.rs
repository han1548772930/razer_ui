//! Observe Windows USB/HID topology without loading the vendor Node addons.
//! The current host starts USB add/remove monitoring in UsbRzDeviceAction.initUSB.
//! Events only invalidate discovery; they never supply a device or a profile.
use async_channel::{Receiver, Sender};
use std::{
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread::JoinHandle,
};

#[derive(Debug, PartialEq, Eq)]
pub enum DeviceChange {
    Ready,
    Changed,
    Failed(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceInterfaceAction {
    Arrival,
    Removal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceNotificationError {
    MissingData,
    DataTooShort,
    DataTooLong,
    UnexpectedFilterType,
    OddPathByteCount,
    MissingPathTerminator,
    EmptyPath,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceChangeLoss {
    QueueOverflow,
    SequenceExhausted,
    RevisionExhausted,
    PublisherPoisoned,
    InvalidNotification(DeviceNotificationError),
}

/// A closed or lost stream cannot establish a device connection epoch. Start a
/// new monitor and wait for its Ready before enumerating; retain and replay
/// Interface events received during that enumeration, including identical paths.
#[derive(Debug, PartialEq, Eq)]
pub enum ExactDeviceChange {
    Ready {
        sequence: u64,
    },
    Interface {
        sequence: u64,
        action: DeviceInterfaceAction,
        /// The original Windows symbolic link, without its terminating NUL.
        path_utf16: Vec<u16>,
    },
    Lost {
        after_sequence: u64,
        reason: DeviceChangeLoss,
    },
    Failed(String),
}

const EXACT_QUEUE_CAPACITY: usize = 256;

/// Shared admission revision for the exact publisher and its consumer. A
/// revision exists only while every published event has been reconciled.
#[derive(Clone, Debug)]
pub struct ExactDeviceChangeGate {
    queue: Arc<EventQueue>,
    stop: mpsc::SyncSender<()>,
}

impl ExactDeviceChangeGate {
    fn queue(&self) -> &ExactQueue {
        match self.queue.as_ref() {
            EventQueue::Exact(queue) => queue,
            EventQueue::Coalesced(_) => unreachable!("exact gates require exact delivery"),
        }
    }

    /// Capture before opening or dispatching native I/O, then validate the
    /// same revision after the actual return. Zero is never an admitted epoch.
    pub fn current_revision(&self) -> Option<u64> {
        let queue = self.queue();
        let revision = queue.admitted_revision.load(Ordering::Acquire);
        (revision != 0 && !queue.events.is_closed() && !queue.state.is_poisoned())
            .then_some(revision)
    }

    pub fn is_current(&self, revision: u64) -> bool {
        self.current_revision() == Some(revision)
    }

    /// Close admission after reconciliation becomes uncertain. A newer
    /// callback leaves its event pending and prevents this prefix barrier.
    pub fn suspend_for_reconcile(&self, consumed_sequence: u64) -> bool {
        let queue = self.queue();
        let state = queue.state();
        queue.admitted_revision.store(0, Ordering::Release);
        !state.terminal
            && !state.revoked
            && state.registered
            && state.last_sequence == consumed_sequence
            && !queue.events.is_closed()
    }

    /// Reopen only if the consumed prefix still equals the complete published
    /// prefix. The callback must only commit short in-memory admission state;
    /// native I/O, enumeration and terminal joins belong before this call.
    /// An already admitted prefix is idempotent and does not invoke resume.
    pub fn try_resume(
        &self,
        consumed_sequence: u64,
        resume: impl FnOnce() -> Result<(), String>,
    ) -> Result<bool, String> {
        let queue = self.queue();
        let mut state = queue.state();
        if state.terminal || state.revoked || queue.events.is_closed() {
            let _ = self.stop.try_send(());
            return Ok(false);
        }
        if !state.registered || state.last_sequence != consumed_sequence {
            return Ok(false);
        }
        if queue.admitted_revision.load(Ordering::Acquire) != 0 {
            return Ok(true);
        }
        let Some(revision) = state.last_revision.checked_add(1) else {
            queue.lose(&mut state, DeviceChangeLoss::RevisionExhausted);
            let _ = self.stop.try_send(());
            return Err("Exact device-change admission revision exhausted".into());
        };
        resume()?;
        state.last_revision = revision;
        queue.admitted_revision.store(revision, Ordering::Release);
        Ok(true)
    }
}

/// Dropping the owner only signals the thread. Native unregistration may wait
/// for callbacks and therefore must never run in a view's destructor.
pub struct DeviceChangeMonitor {
    stop: Option<mpsc::SyncSender<()>>,
    worker: Option<JoinHandle<Result<(), String>>>,
    exact_gate: Option<ExactDeviceChangeGate>,
}

impl DeviceChangeMonitor {
    pub fn start() -> Result<(Self, Receiver<DeviceChange>), String> {
        let (events, changes) = async_channel::bounded(1);
        let monitor = Self::start_events(EventQueue::Coalesced(events))?;
        Ok((monitor, changes))
    }

    /// Exact notifications are bounded. Lost or Failed permanently ends the
    /// stream and requests native unregistration. The native worker retains
    /// failed handles and retries until callback quiescence is established.
    pub fn start_exact() -> Result<(Self, Receiver<ExactDeviceChange>), String> {
        let (events, changes) = EventQueue::exact(EXACT_QUEUE_CAPACITY);
        let monitor = Self::start_events(events)?;
        Ok((monitor, changes))
    }

    pub fn exact_gate(&self) -> Option<ExactDeviceChangeGate> {
        self.exact_gate.clone()
    }

    fn start_events(events: EventQueue) -> Result<Self, String> {
        Self::start_worker(events, |stopped, events| {
            if !matches!(stopped.try_recv(), Err(mpsc::TryRecvError::Empty)) {
                return Ok(());
            }
            let notifications = native::Notifications::register(events.clone())?;
            wait_for_stop(stopped, &events);
            notifications.shutdown()
        })
    }

    fn start_worker(
        events: EventQueue,
        run: impl FnOnce(mpsc::Receiver<()>, EventPublisher) -> Result<(), String> + Send + 'static,
    ) -> Result<Self, String> {
        let (stop, stopped) = mpsc::sync_channel(1);
        let publisher = EventPublisher::new(events, stop.clone());
        let exact_gate = publisher.exact_gate();
        let worker = std::thread::Builder::new()
            .name("razer-device-changes".into())
            .spawn(move || {
                let delivery = EventDelivery(publisher);
                let result = run(stopped, delivery.0.clone());
                if let Err(error) = &result {
                    delivery.0.failed(error.clone());
                }
                result
            })
            .map_err(|error| format!("无法创建设备变化监听线程：{error}"))?;
        Ok(Self {
            stop: Some(stop),
            worker: Some(worker),
            exact_gate,
        })
    }

    /// Consume on a background executor and wait until every native handle and
    /// callback context is released. Historical retry failures remain errors.
    pub fn shutdown(mut self) -> Result<(), String> {
        self.signal_stop();
        self.join_worker()
    }

    /// Stop observation without waiting on native unregistration.
    pub fn request_stop(&mut self) {
        self.signal_stop();
    }

    /// Native cleanup is terminal only after its retained worker finishes.
    pub fn is_finished(&self) -> bool {
        self.worker.as_ref().is_none_or(JoinHandle::is_finished)
    }

    /// Retain the worker while cleanup is pending. A terminal error reports
    /// historical failures after join; it never abandons an active worker.
    pub fn try_shutdown(&mut self) -> Result<bool, String> {
        self.signal_stop();
        if !self.is_finished() {
            return Ok(false);
        }
        self.join_worker()?;
        Ok(true)
    }

    fn join_worker(&mut self) -> Result<(), String> {
        match self.worker.take() {
            Some(worker) => worker
                .join()
                .map_err(|_| "设备变化监听线程异常结束。".to_string())?,
            None => Ok(()),
        }
    }

    fn signal_stop(&mut self) {
        if let Some(gate) = &self.exact_gate {
            gate.queue().revoke();
        }
        if let Some(stop) = self.stop.take() {
            let _ = stop.try_send(());
        }
    }
}

impl Drop for DeviceChangeMonitor {
    fn drop(&mut self) {
        // Native unregistration can wait for callbacks. Only shutdown joins;
        // an entity destructor merely signals the dedicated stop channel.
        self.signal_stop();
    }
}

struct EventDelivery(EventPublisher);

impl Drop for EventDelivery {
    fn drop(&mut self) {
        // Terminal delivery belongs to the observation worker, including its
        // unwind path; another publisher clone cannot keep the stream open.
        self.0.close();
    }
}

fn wait_for_stop(stopped: mpsc::Receiver<()>, events: &EventPublisher) {
    // Only successful registration reaches this barrier. Exact delivery keeps
    // callbacks that arrived during registration ahead of the Ready event.
    events.ready();
    let _ = stopped.recv();
}

#[derive(Debug)]
pub(crate) enum EventQueue {
    Coalesced(Sender<DeviceChange>),
    Exact(ExactQueue),
}

impl EventQueue {
    pub(crate) fn exact(capacity: usize) -> (Self, Receiver<ExactDeviceChange>) {
        // All ordinary sends share one lock. The additional slot is reserved
        // for Lost or Failed and never replaces an already queued event.
        let (events, changes) = async_channel::bounded(capacity + 1);
        (
            Self::Exact(ExactQueue {
                events,
                capacity,
                admitted_revision: AtomicU64::new(0),
                state: Mutex::new(ExactDeliveryState {
                    last_sequence: 0,
                    last_revision: 0,
                    registered: false,
                    revoked: false,
                    terminal: false,
                }),
            }),
            changes,
        )
    }
}

#[derive(Clone, Debug)]
pub(crate) struct EventPublisher {
    queue: Arc<EventQueue>,
    stop: mpsc::SyncSender<()>,
}

impl EventPublisher {
    pub(crate) fn new(queue: EventQueue, stop: mpsc::SyncSender<()>) -> Self {
        Self {
            queue: Arc::new(queue),
            stop,
        }
    }

    pub(crate) fn exact_gate(&self) -> Option<ExactDeviceChangeGate> {
        matches!(self.queue.as_ref(), EventQueue::Exact(_)).then(|| ExactDeviceChangeGate {
            queue: self.queue.clone(),
            stop: self.stop.clone(),
        })
    }

    pub(crate) fn interface(&self, action: DeviceInterfaceAction, path_utf16: Vec<u16>) {
        let live = match self.queue.as_ref() {
            EventQueue::Coalesced(events) => match events.try_send(DeviceChange::Changed) {
                Ok(()) | Err(async_channel::TrySendError::Full(_)) => true,
                Err(async_channel::TrySendError::Closed(_)) => false,
            },
            EventQueue::Exact(queue) => queue.publish(|sequence| ExactDeviceChange::Interface {
                sequence,
                action,
                path_utf16,
            }),
        };
        if !live {
            let _ = self.stop.try_send(());
        }
    }

    pub(crate) fn ready(&self) {
        let live = match self.queue.as_ref() {
            EventQueue::Coalesced(events) => events.force_send(DeviceChange::Ready).is_ok(),
            EventQueue::Exact(queue) => {
                queue.publish(|sequence| ExactDeviceChange::Ready { sequence })
            }
        };
        if !live {
            let _ = self.stop.try_send(());
        }
    }

    pub(crate) fn lost(&self, reason: DeviceChangeLoss) {
        match self.queue.as_ref() {
            EventQueue::Coalesced(events) => {
                let _ = events.force_send(DeviceChange::Failed(format!(
                    "设备变化通知已失效：{reason:?}"
                )));
                events.close();
            }
            EventQueue::Exact(queue) => {
                let mut state = queue.state();
                queue.lose(&mut state, reason);
            }
        }
        let _ = self.stop.try_send(());
    }

    pub(crate) fn failed(&self, error: String) {
        match self.queue.as_ref() {
            EventQueue::Coalesced(events) => {
                let _ = events.force_send(DeviceChange::Failed(error));
            }
            EventQueue::Exact(queue) => {
                let mut state = queue.state();
                queue.terminate(&mut state, ExactDeviceChange::Failed(error));
            }
        }
    }

    pub(crate) fn close(&self) {
        match self.queue.as_ref() {
            EventQueue::Coalesced(events) => {
                events.close();
            }
            EventQueue::Exact(queue) => {
                queue.close();
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct ExactQueue {
    events: Sender<ExactDeviceChange>,
    capacity: usize,
    admitted_revision: AtomicU64,
    state: Mutex<ExactDeliveryState>,
}

#[derive(Debug)]
struct ExactDeliveryState {
    last_sequence: u64,
    last_revision: u64,
    registered: bool,
    revoked: bool,
    terminal: bool,
}

impl ExactQueue {
    fn revoke(&self) {
        let mut state = self.state();
        self.admitted_revision.store(0, Ordering::Release);
        state.revoked = true;
    }

    fn close(&self) {
        let mut state = self.state();
        self.admitted_revision.store(0, Ordering::Release);
        state.revoked = true;
        state.terminal = true;
        self.events.close();
    }

    fn state(&self) -> MutexGuard<'_, ExactDeliveryState> {
        match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => {
                let mut state = poisoned.into_inner();
                self.lose(&mut state, DeviceChangeLoss::PublisherPoisoned);
                state
            }
        }
    }

    fn publish(&self, event: impl FnOnce(u64) -> ExactDeviceChange) -> bool {
        let mut state = self.state();
        // The callback closes admission before exposing its event. Resume
        // uses this same lock, so an empty-queue observation cannot overwrite
        // a concurrent invalidation with an admitted older revision.
        self.admitted_revision.store(0, Ordering::Release);
        if state.terminal {
            return false;
        }
        let Some(sequence) = state.last_sequence.checked_add(1) else {
            self.lose(&mut state, DeviceChangeLoss::SequenceExhausted);
            return false;
        };
        if self.events.len() >= self.capacity {
            self.lose(&mut state, DeviceChangeLoss::QueueOverflow);
            return false;
        }
        let event = event(sequence);
        let registered = matches!(event, ExactDeviceChange::Ready { .. });
        match self.events.try_send(event) {
            Ok(()) => {
                state.last_sequence = sequence;
                state.registered |= registered;
                true
            }
            Err(async_channel::TrySendError::Closed(_)) => {
                state.terminal = true;
                false
            }
            Err(async_channel::TrySendError::Full(_)) => {
                self.lose(&mut state, DeviceChangeLoss::QueueOverflow);
                false
            }
        }
    }

    fn lose(&self, state: &mut ExactDeliveryState, reason: DeviceChangeLoss) {
        let after_sequence = state.last_sequence;
        self.terminate(
            state,
            ExactDeviceChange::Lost {
                after_sequence,
                reason,
            },
        );
    }

    fn terminate(&self, state: &mut ExactDeliveryState, event: ExactDeviceChange) {
        self.admitted_revision.store(0, Ordering::Release);
        if !state.terminal {
            state.terminal = true;
            let _ = self.events.try_send(event);
            self.events.close();
        }
    }
}

#[cfg(windows)]
use crate::platform::windows::device_changes as native;

#[cfg(not(windows))]
mod native {
    use super::*;
    pub(super) struct Notifications;
    impl Notifications {
        pub(super) fn register(_: EventPublisher) -> Result<Self, String> {
            Err("此平台尚未接入设备插拔监听；可手动刷新设备信息。".into())
        }

        pub(super) fn shutdown(self) -> Result<(), String> {
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "device_changes/tests.rs"]
mod tests;
