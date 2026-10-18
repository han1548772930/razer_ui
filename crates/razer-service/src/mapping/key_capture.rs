//! Focused keyboard mapping capture owns an isolated native service worker.
//! Dropping the handle cancels polling and restores mapping/redirect state.
use super::{Transport, WorkerGuard, guarded, shutdown};
#[cfg(test)]
use razer_ipc::KEY_CAPTURE_LIFECYCLE as LIFECYCLE;
use razer_ipc::global_shortcut_capture::{Batch as GlobalBatch, Event as GlobalEvent};
use razer_ipc::mapping_control::{Registry, Suppression};
use razer_ipc::{ServiceClient, ServiceRequest};
use serde::Deserialize;
use serde_json::Value;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
pub enum Reply {
    Ready(CaptureReady),
    Events(Vec<Value>),
    Mode(bool),
    Failed(String),
}

#[derive(Debug, Deserialize)]
pub struct CaptureReady {
    capturing: bool,
    redirected_devices: usize,
    devices: Vec<Value>,
    unresolved_devices: Vec<Value>,
}

impl CaptureReady {
    fn parse(value: Value) -> anyhow::Result<Self> {
        let receipt: Self = serde_json::from_value(value)?;
        anyhow::ensure!(receipt.capturing, "Key capture acknowledgement missing");
        anyhow::ensure!(
            receipt.redirected_devices == receipt.devices.len(),
            "Key capture registration count does not match"
        );
        Ok(receipt)
    }

    pub fn unresolved_devices(&self) -> &[Value] {
        &self.unresolved_devices
    }
}

#[derive(Deserialize)]
struct EventBatch {
    events: Vec<Value>,
}
#[derive(Clone, Copy)]
enum Kind {
    Mapping,
    GlobalShortcut,
}

pub struct Session {
    canceled: Arc<AtomicBool>,
    events: mpsc::Receiver<Reply>,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.canceled.store(true, Ordering::Release);
    }
}
impl Session {
    pub fn start() -> anyhow::Result<Self> {
        Self::start_with_registry(ServiceClient::spawn, Registry::global(), Kind::Mapping)
    }
    pub fn start_global_shortcut() -> anyhow::Result<Self> {
        Self::start_with_registry(
            ServiceClient::spawn,
            Registry::global(),
            Kind::GlobalShortcut,
        )
    }

    #[cfg(test)]
    pub(super) fn start_with<T: Transport + 'static>(
        factory: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
    ) -> anyhow::Result<Self> {
        Self::start_with_registry(factory, Registry::default(), Kind::Mapping)
    }

    fn start_with_registry<T: Transport + 'static>(
        factory: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
        registry: Registry,
        kind: Kind,
    ) -> anyhow::Result<Self> {
        let (sender, events) = mpsc::channel();
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        let guard = WorkerGuard::start();
        thread::Builder::new()
            .name("mapping-key-capture".into())
            .spawn(move || {
                let _guard = guard;
                // A quick blur/refocus must not let the previous cleanup enable
                // mappings while the replacement recorder owns suppression.
                let _lifecycle = match super::lock_lifecycle(&signal) {
                    Ok(guard) => guard,
                    Err(error) => {
                        if !signal.load(Ordering::Acquire) {
                            let _ = sender.send(Reply::Failed(format!("{error:#}")));
                        }
                        return;
                    }
                };
                if signal.load(Ordering::Acquire) {
                    return;
                }
                let mut client = match guarded(factory) {
                    Ok(client) => client,
                    Err(error) => {
                        let _ = sender.send(Reply::Failed(format!("{error:#}")));
                        return;
                    }
                };
                let mut suppression = None;
                let result = guarded(|| -> anyhow::Result<()> {
                    anyhow::ensure!(!signal.load(Ordering::Acquire), "Key capture canceled");
                    suppression = Some(
                        registry
                            .suspend_cancelable(Suppression::Mapping, &signal)
                            .map_err(anyhow::Error::msg)?,
                    );
                    let ready = client.request(match kind {
                        Kind::Mapping => ServiceRequest::StartMappingKeyCapture,
                        Kind::GlobalShortcut => ServiceRequest::StartGlobalShortcutCapture,
                    })?;
                    if matches!(kind, Kind::GlobalShortcut) {
                        anyhow::ensure!(
                            ready.get("mode_events_registered").and_then(Value::as_bool)
                                == Some(true),
                            "Global shortcut mode callback was not acknowledged"
                        );
                    }
                    let ready = CaptureReady::parse(ready)?;
                    let _ = sender.send(Reply::Ready(ready));
                    while !signal.load(Ordering::Acquire) {
                        match kind {
                            Kind::Mapping => {
                                let observed: EventBatch = serde_json::from_value(
                                    client.request(ServiceRequest::MappingKeyCaptureEvents)?,
                                )?;
                                if !observed.events.is_empty()
                                    && sender.send(Reply::Events(observed.events)).is_err()
                                {
                                    break;
                                }
                            }
                            Kind::GlobalShortcut => {
                                let batch: GlobalBatch = serde_json::from_value(
                                    client.request(ServiceRequest::GlobalShortcutCaptureEvents)?,
                                )?;
                                for event in batch.into_events() {
                                    let reply = match event {
                                        GlobalEvent::Input(value) => Reply::Events(vec![value]),
                                        GlobalEvent::Mode(enabled) => Reply::Mode(enabled),
                                    };
                                    if signal.load(Ordering::Acquire) || sender.send(reply).is_err()
                                    {
                                        return Ok(());
                                    }
                                }
                            }
                        }
                        thread::sleep(Duration::from_millis(16));
                    }
                    Ok(())
                });
                if let Err(error) = result {
                    let _ = sender.send(Reply::Failed(format!("{error:#}")));
                }
                // Shutdown also restores partial registrations after startup failure.
                if let Err(error) = shutdown(&mut client) {
                    let message = format!("Key capture cleanup failed: {error:#}");
                    eprintln!("[mapping-key-capture] {message}");
                    let _ = sender.send(Reply::Failed(message));
                }
                drop(client);
                if let Some(mut suppression) = suppression {
                    if let Err(error) = suppression.release() {
                        let _ = sender.send(Reply::Failed(format!(
                            "Key capture mapping restoration failed: {error}"
                        )));
                    }
                }
            })?;
        Ok(Self { canceled, events })
    }
    pub fn drain(&self) -> Vec<Reply> {
        self.events.try_iter().collect()
    }
}

#[cfg(test)]
mod tests;
