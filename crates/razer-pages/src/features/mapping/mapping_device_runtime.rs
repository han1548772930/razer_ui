//! Retained mapping service worker shared by audited ordinary USB products.
//! Owners retain this handle independently of page/tab visibility. Dropping it
//! requests cancellation; the worker removes its device before service shutdown.
use razer_ipc::{ServiceClient, ServiceRequest};
use serde_json::Value;
/// Original unsupported-mapping callback, retaining input/output/timer tick.
#[derive(Clone)]
pub struct MappingCallbackRequested {
    raw: Value,
}
impl MappingCallbackRequested {
    pub(crate) fn new(raw: Value) -> Self {
        Self { raw }
    }
    pub fn raw(&self) -> &Value {
        &self.raw
    }
}
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
static ACTIVE_WORKERS: AtomicUsize = AtomicUsize::new(0);
pub(crate) fn active_workers() -> usize {
    ACTIVE_WORKERS.load(Ordering::Acquire)
}
struct WorkerGuard;
impl Drop for WorkerGuard {
    fn drop(&mut self) {
        ACTIVE_WORKERS.fetch_sub(1, Ordering::AcqRel);
    }
}

pub(crate) enum Reply {
    Ready,
    Published {
        revision: u64,
        result: Result<(), String>,
        indicator: Option<Result<Value, String>>,
    },
    Events(Vec<Value>),
    Failed(String),
    Stopped(Result<(), String>),
}
struct Publish {
    revision: u64,
    document: Value,
    indicator: Option<bool>,
    restore_on_stop: Option<Value>,
}
pub(crate) struct Owner {
    commands: mpsc::Sender<Publish>,
    canceled: Arc<AtomicBool>,
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.canceled.store(true, Ordering::Release);
    }
}
impl Owner {
    pub(crate) fn publish(
        &self,
        revision: u64,
        document: Value,
        indicator: Option<bool>,
    ) -> Result<(), String> {
        self.publish_with_restore(revision, document, indicator, None)
    }
    pub(crate) fn publish_with_restore(
        &self,
        revision: u64,
        document: Value,
        indicator: Option<bool>,
        restore_on_stop: Option<Value>,
    ) -> Result<(), String> {
        self.commands
            .send(Publish {
                revision,
                document,
                indicator,
                restore_on_stop,
            })
            .map_err(|_| "Mapping worker disconnected".into())
    }
    pub(crate) fn start(
        product_id: u32,
        container: String,
    ) -> anyhow::Result<(Self, mpsc::Receiver<Reply>)> {
        let (commands, receiver) = mpsc::channel::<Publish>();
        let (sender, replies) = mpsc::channel();
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        ACTIVE_WORKERS.fetch_add(1, Ordering::AcqRel);
        let guard = WorkerGuard;
        thread::Builder::new()
            .name(format!("mapping-device-{product_id}"))
            .spawn(move || {
                let _guard = guard;
                let owner_guid = uuid::Uuid::new_v4().to_string();
                let mut client = match ServiceClient::spawn() {
                    Ok(client) => client,
                    Err(error) => {
                        let message = format!("{error:#}");
                        let _ = sender.send(Reply::Failed(message.clone()));
                        let _ = sender.send(Reply::Stopped(Err(message)));
                        return;
                    }
                };
                let mut restore_on_stop = None;
                let result = (|| -> anyhow::Result<()> {
                    anyhow::ensure!(!signal.load(Ordering::Acquire), "Mapping start canceled");
                    let response = client.request(ServiceRequest::MappingDeviceStart {
                        product_id,
                        device_container_id: container,
                        owner_guid: owner_guid.clone(),
                    })?;
                    anyhow::ensure!(
                        response["registered"] == true
                            && response["driver_ready"] == true
                            && response["owner_guid"] == owner_guid,
                        "Mapping registration receipt does not match"
                    );
                    let _ = sender.send(Reply::Ready);
                    while !signal.load(Ordering::Acquire) {
                        let next = match receiver.recv_timeout(Duration::from_millis(100)) {
                            Ok(mut next) => {
                                while let Ok(newer) = receiver.try_recv() {
                                    next = newer;
                                }
                                Some(next)
                            }
                            Err(mpsc::RecvTimeoutError::Timeout) => None,
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        };
                        if signal.load(Ordering::Acquire) {
                            break;
                        }
                        if let Some(next) = next {
                            if next.restore_on_stop.is_some() {
                                // Retain the ordinary document before the first
                                // native write: publication can fail partway.
                                restore_on_stop = next.restore_on_stop.clone();
                            }
                            let result = client
                                .request(ServiceRequest::MappingDevicePublish {
                                    owner_guid: owner_guid.clone(),
                                    document: next.document,
                                })
                                .and_then(|value| {
                                    anyhow::ensure!(
                                        value["published"] == true
                                            && value["owner_guid"] == owner_guid,
                                        "Mapping publication receipt does not match"
                                    );
                                    Ok(())
                                })
                                .map_err(|error| format!("{error:#}"));
                            if result.is_ok() && next.restore_on_stop.is_none() {
                                restore_on_stop = None;
                            }
                            let indicator = if !signal.load(Ordering::Acquire) {
                                next.indicator.map(|enabled| {
                                    client
                                        .request(ServiceRequest::MappingKeyboardGamingIndicator {
                                            owner_guid: owner_guid.clone(),
                                            enabled,
                                        })
                                        .map_err(|error| format!("{error:#}"))
                                })
                            } else {
                                None
                            };
                            let _ = sender.send(Reply::Published {
                                revision: next.revision,
                                result,
                                indicator,
                            });
                            anyhow::ensure!(
                                !client.is_stopped(),
                                "Mapping service connection stopped"
                            );
                        }
                        if signal.load(Ordering::Acquire) {
                            break;
                        }
                        let value = client.request(ServiceRequest::MappingDeviceEvents {
                            owner_guid: owner_guid.clone(),
                        })?;
                        anyhow::ensure!(
                            value["owner_guid"] == owner_guid,
                            "Mapping event owner changed"
                        );
                        let events = value["events"]
                            .as_array()
                            .ok_or_else(|| anyhow::anyhow!("Mapping event list is missing"))?;
                        if !events.is_empty() {
                            let _ = sender.send(Reply::Events(events.clone()));
                        }
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    let _ = sender.send(Reply::Failed(format!("{error:#}")));
                }
                if let Some(document) = restore_on_stop {
                    let restored = client
                        .request(ServiceRequest::MappingDevicePublish {
                            owner_guid: owner_guid.clone(),
                            document,
                        })
                        .and_then(|value| {
                            anyhow::ensure!(
                                value["published"] == true && value["owner_guid"] == owner_guid,
                                "Temporary mapping restore receipt does not match"
                            );
                            Ok(())
                        });
                    if let Err(error) = restored {
                        eprintln!("[mapping-device] temporary mapping cleanup failed: {error:#}");
                        let _ = sender.send(Reply::Failed(format!(
                            "Temporary mapping cleanup: {error:#}"
                        )));
                    }
                }
                // Shutdown owns partial registrations too, including a failed
                // addUsbDevice callback or missing driver-ready event.
                let stopped = client
                    .request(ServiceRequest::Shutdown)
                    .map(|_| ())
                    .map_err(|e| format!("{e:#}"));
                let _ = sender.send(Reply::Stopped(stopped));
            })?;
        Ok((Self { commands, canceled }, replies))
    }
}
