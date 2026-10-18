//! Audio shortcut fields share the native mode callback and mapping lifecycle.
//! The worker retains suppression until native shutdown and transport release.
use super::Client;
use super::{Transport, WorkerGuard, guarded, lock_lifecycle, shutdown};
use razer_ipc::{
    ServiceRequest,
    mapping_control::{Registry, Suppression},
};
use serde::Deserialize;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub enum Reply {
    Ready,
    Mode(bool),
    Failed(String),
}

#[derive(Deserialize)]
struct CaptureReceipt {
    capturing: bool,
    mappings_suspended: bool,
    mode_events_registered: bool,
}

#[derive(Deserialize)]
struct EventBatch {
    events: Vec<ModeEvent>,
}

#[derive(Deserialize)]
struct ModeEvent {
    event: String,
}

#[derive(Deserialize)]
struct ModePayload {
    #[serde(rename = "isEnabled")]
    enabled: bool,
}

pub struct Session {
    canceled: Arc<AtomicBool>,
    replies: mpsc::Receiver<Reply>,
    worker: Option<JoinHandle<Result<(), String>>>,
}

impl Session {
    pub fn start() -> anyhow::Result<Self> {
        Self::start_with(Client::spawn, Registry::global())
    }

    fn start_with<T: Transport + 'static>(
        factory: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
        registry: Registry,
    ) -> anyhow::Result<Self> {
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        let (sender, replies) = mpsc::channel();
        let guard = WorkerGuard::start();
        let worker = thread::Builder::new()
            .name("mapping-mode-capture".into())
            .spawn(move || {
                let _guard = guard;
                let result = guarded(|| -> anyhow::Result<()> {
                    let _lifecycle = lock_lifecycle(&signal)?;
                    if signal.load(Ordering::Acquire) {
                        return Ok(());
                    }
                    let mut client = guarded(factory)?;
                    let mut suppression = None;
                    let run = guarded(|| -> anyhow::Result<()> {
                        suppression = Some(
                            registry
                                .suspend_cancelable(Suppression::Mapping, &signal)
                                .map_err(anyhow::Error::msg)?,
                        );
                        let receipt: CaptureReceipt = serde_json::from_value(
                            client.request(ServiceRequest::StartShortcutCapture)?,
                        )?;
                        anyhow::ensure!(
                            receipt.capturing
                                && receipt.mappings_suspended
                                && receipt.mode_events_registered,
                            "Shortcut mode capture was not acknowledged",
                        );
                        if sender.send(Reply::Ready).is_err() {
                            return Ok(());
                        }
                        while !signal.load(Ordering::Acquire) {
                            let batch: EventBatch = serde_json::from_value(
                                client.request(ServiceRequest::ShortcutCaptureEvents)?,
                            )?;
                            for event in batch.events {
                                let payload: ModePayload = serde_json::from_str(&event.event)?;
                                if signal.load(Ordering::Acquire)
                                    || sender.send(Reply::Mode(payload.enabled)).is_err()
                                {
                                    return Ok(());
                                }
                            }
                            thread::park_timeout(Duration::from_millis(16));
                        }
                        Ok(())
                    });
                    let cleanup = shutdown(&mut client);
                    drop(client);
                    let restored = suppression.as_mut().map_or(Ok(()), |scope| scope.release());
                    let mut errors = Vec::new();
                    if let Err(error) = run {
                        errors.push(format!("{error:#}"));
                    }
                    if let Err(error) = cleanup {
                        errors.push(format!("Shortcut mode capture cleanup: {error}"));
                    }
                    if let Err(error) = restored {
                        errors.push(format!("Shortcut mode mapping restoration: {error}"));
                    }
                    anyhow::ensure!(errors.is_empty(), "{}", errors.join("; "));
                    Ok(())
                })
                .map_err(|error| format!("{error:#}"));
                if let Err(error) = &result {
                    let _ = sender.send(Reply::Failed(error.clone()));
                }
                result
            })?;
        Ok(Self {
            canceled,
            replies,
            worker: Some(worker),
        })
    }

    pub fn drain(&self) -> Vec<Reply> {
        self.replies.try_iter().collect()
    }

    pub fn is_finished(&self) -> bool {
        self.worker.as_ref().is_none_or(JoinHandle::is_finished)
    }

    /// Wait for callback cleanup and mapping restoration on a background owner.
    pub fn stop(mut self) -> Result<(), String> {
        self.request_stop();
        match self.worker.take() {
            Some(worker) => worker
                .join()
                .map_err(|_| "Shortcut mode capture worker panicked".to_owned())?,
            None => Ok(()),
        }
    }

    pub fn request_stop(&self) {
        self.canceled.store(true, Ordering::Release);
        if let Some(worker) = &self.worker {
            worker.thread().unpark();
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.request_stop();
    }
}

#[cfg(test)]
mod tests;
