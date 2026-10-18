//! A page subscription borrows the existing mapping device registration.
use super::{Client, Transport, WorkerGuard, guarded, shutdown};
use razer_ipc::{
    ServiceRequest,
    mapping_observation::{Batch, Receipt, SnapTapEvent, Target},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub enum Reply<E = SnapTapEvent> {
    Ready,
    Event(E),
    Failed(String),
}
pub struct Session<E = SnapTapEvent> {
    canceled: Arc<AtomicBool>,
    replies: mpsc::Receiver<Reply<E>>,
    worker: Option<JoinHandle<Result<(), String>>>,
}
impl Session {
    pub fn start(target: Target) -> anyhow::Result<Self> {
        Self::start_with(target, Client::spawn)
    }
    fn start_with<T: Transport + 'static>(
        target: Target,
        factory: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
    ) -> anyhow::Result<Self> {
        Self::start_stream(SnapTapStream(target), factory)
    }
}

pub(super) trait Stream: Send + 'static {
    type Event: Send + 'static;
    fn name() -> &'static str;
    fn start(self, observer: String) -> ServiceRequest;
    fn poll(observer: String) -> ServiceRequest;
    fn stop(observer: String) -> ServiceRequest;
    fn decode(value: serde_json::Value, observer: &str) -> anyhow::Result<Vec<Self::Event>>;
}
struct SnapTapStream(Target);
impl Stream for SnapTapStream {
    type Event = SnapTapEvent;
    fn name() -> &'static str {
        "mapping-snaptap-observer"
    }
    fn start(self, observer_guid: String) -> ServiceRequest {
        ServiceRequest::MappingObservationStart {
            target: self.0,
            observer_guid,
        }
    }
    fn poll(observer_guid: String) -> ServiceRequest {
        ServiceRequest::MappingObservationPoll { observer_guid }
    }
    fn stop(observer_guid: String) -> ServiceRequest {
        ServiceRequest::MappingObservationStop { observer_guid }
    }
    fn decode(value: serde_json::Value, observer: &str) -> anyhow::Result<Vec<Self::Event>> {
        serde_json::from_value::<Batch>(value)?.into_events(observer)
    }
}
impl<E> Session<E> {
    pub(super) fn start_stream<S: Stream<Event = E>, T: Transport + 'static>(
        stream: S,
        factory: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
    ) -> anyhow::Result<Self>
    where
        E: Send + 'static,
    {
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        let (sender, replies) = mpsc::sync_channel(512);
        let guard = WorkerGuard::start();
        let worker = thread::Builder::new()
            .name(S::name().into())
            .spawn(move || {
                let _guard = guard;
                let result = guarded(|| -> anyhow::Result<()> {
                    if signal.load(Ordering::Acquire) {
                        return Ok(());
                    }
                    let mut client = factory()?;
                    let observer_guid = uuid::Uuid::new_v4().to_string();
                    let mut registered = false;
                    let run = guarded(|| -> anyhow::Result<()> {
                        // A failed response can follow accepted registration.
                        // Stop is idempotent for this unique observer identity.
                        registered = true;
                        let receipt: Receipt = serde_json::from_value(
                            client.request(stream.start(observer_guid.clone()))?,
                        )?;
                        anyhow::ensure!(
                            receipt.matches(&observer_guid, true),
                            "Mapping observation registration was not acknowledged"
                        );
                        sender.try_send(Reply::Ready).map_err(|_| {
                            anyhow::anyhow!("Mapping observation receiver is unavailable")
                        })?;
                        while !signal.load(Ordering::Acquire) {
                            let batch = client.request(S::poll(observer_guid.clone()))?;
                            for event in S::decode(batch, &observer_guid)? {
                                if signal.load(Ordering::Acquire) {
                                    break;
                                }
                                sender.try_send(Reply::Event(event)).map_err(|_| {
                                    anyhow::anyhow!("Mapping observation receiver fell behind")
                                })?;
                            }
                            thread::park_timeout(Duration::from_millis(16));
                        }
                        Ok(())
                    });
                    let mut errors = Vec::new();
                    if let Err(error) = run {
                        errors.push(format!("{error:#}"));
                    }
                    if registered {
                        let stopped = guarded(|| -> anyhow::Result<()> {
                            let receipt: Receipt = serde_json::from_value(
                                client.request(S::stop(observer_guid.clone()))?,
                            )?;
                            anyhow::ensure!(
                                receipt.matches(&observer_guid, false),
                                "Mapping observation cleanup was not acknowledged"
                            );
                            Ok(())
                        });
                        if let Err(error) = stopped {
                            errors.push(format!("Observation unsubscribe: {error:#}"));
                        }
                    }
                    if let Err(error) = shutdown(&mut client) {
                        errors.push(error);
                    }
                    anyhow::ensure!(errors.is_empty(), "{}", errors.join("; "));
                    Ok(())
                })
                .map_err(|error| format!("{error:#}"));
                if let Err(error) = &result {
                    let _ = sender.try_send(Reply::Failed(error.clone()));
                }
                result
            })?;
        Ok(Self {
            canceled,
            replies,
            worker: Some(worker),
        })
    }
    pub fn drain(&self) -> Vec<Reply<E>> {
        self.replies.try_iter().collect()
    }
    pub fn is_finished(&self) -> bool {
        self.worker.as_ref().is_none_or(JoinHandle::is_finished)
    }
    pub fn request_stop(&self) {
        self.canceled.store(true, Ordering::Release);
        if let Some(worker) = &self.worker {
            worker.thread().unpark();
        }
    }
    pub fn stop(mut self) -> Result<(), String> {
        self.request_stop();
        self.worker.take().map_or(Ok(()), |worker| {
            worker
                .join()
                .map_err(|_| "Mapping observation worker panicked".to_owned())?
        })
    }
}
impl<E> Drop for Session<E> {
    fn drop(&mut self) {
        self.request_stop();
    }
}

#[cfg(test)]
mod tests;
