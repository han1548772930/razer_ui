//! Retained mapping service worker shared by audited ordinary USB products.
//! Owners retain this handle independently of page/tab visibility. Dropping it
//! requests cancellation; the worker removes its device before service shutdown.
use super::Client;
use super::{Transport, WorkerGuard, guarded, shutdown};
use razer_ipc::ServiceRequest;
use razer_ipc::mapping_control::Registry;
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct LifecycleKey {
    product_id: u32,
    container: String,
}

type Lifecycle = Arc<Mutex<()>>;
static LIFECYCLES: Mutex<BTreeMap<LifecycleKey, Weak<Mutex<()>>>> = Mutex::new(BTreeMap::new());

fn lifecycle(product_id: u32, container: &str) -> anyhow::Result<Lifecycle> {
    let mut sessions = LIFECYCLES
        .lock()
        .map_err(|_| anyhow::anyhow!("Mapping lifecycle registry lock poisoned"))?;
    sessions.retain(|_, session| session.strong_count() > 0);
    let key = LifecycleKey {
        product_id,
        container: container.to_ascii_lowercase(),
    };
    if let Some(session) = sessions.get(&key).and_then(Weak::upgrade) {
        return Ok(session);
    }
    let session = Arc::new(Mutex::new(()));
    sessions.insert(key, Arc::downgrade(&session));
    Ok(session)
}
pub enum Reply {
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
enum Command {
    Publish(Publish),
    Finish,
}
enum Admission {
    Accepting,
    Draining,
}

fn registration(value: Value, owner: &str) -> anyhow::Result<()> {
    #[derive(Deserialize)]
    struct Receipt {
        registered: bool,
        driver_ready: bool,
        owner_guid: String,
    }

    let receipt: Receipt = serde_json::from_value(value)?;
    anyhow::ensure!(
        receipt.registered && receipt.driver_ready && receipt.owner_guid == owner,
        "Mapping registration receipt does not match"
    );
    Ok(())
}

fn publication(value: Value, owner: &str) -> anyhow::Result<()> {
    #[derive(Deserialize)]
    struct Receipt {
        published: bool,
        owner_guid: String,
    }

    let receipt: Receipt = serde_json::from_value(value)?;
    anyhow::ensure!(
        receipt.published && receipt.owner_guid == owner,
        "Mapping publication receipt does not match"
    );
    Ok(())
}

fn event_batch(value: Value, owner: &str) -> anyhow::Result<Vec<Value>> {
    #[derive(Deserialize)]
    struct Receipt {
        owner_guid: String,
        events: Vec<Value>,
    }

    let receipt: Receipt = serde_json::from_value(value)?;
    anyhow::ensure!(receipt.owner_guid == owner, "Mapping event owner changed");
    Ok(receipt.events)
}
pub struct Session {
    commands: mpsc::Sender<Command>,
    admission: Mutex<Admission>,
    canceled: Arc<AtomicBool>,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.canceled.store(true, Ordering::Release);
    }
}
impl Session {
    pub fn publish(
        &self,
        revision: u64,
        document: Value,
        indicator: Option<bool>,
    ) -> Result<(), String> {
        self.publish_with_restore(revision, document, indicator, None)
    }
    pub fn publish_with_restore(
        &self,
        revision: u64,
        document: Value,
        indicator: Option<bool>,
        restore_on_stop: Option<Value>,
    ) -> Result<(), String> {
        let admission = self
            .admission
            .lock()
            .map_err(|_| "Mapping admission lock poisoned")?;
        if !matches!(*admission, Admission::Accepting) {
            return Err("Mapping owner is draining".into());
        }
        self.commands
            .send(Command::Publish(Publish {
                revision,
                document,
                indicator,
                restore_on_stop,
            }))
            .map_err(|_| "Mapping worker disconnected".into())
    }

    /// Close admission and finish the latest admitted document before native
    /// restoration/removal, shutdown and transport release. This never waits.
    /// Dropping the handle still cancels any pending work immediately.
    pub fn request_stop_after_pending(&self) -> Result<(), String> {
        let mut admission = self
            .admission
            .lock()
            .map_err(|_| "Mapping admission lock poisoned")?;
        if matches!(*admission, Admission::Draining) {
            return Ok(());
        }
        *admission = Admission::Draining;
        self.commands
            .send(Command::Finish)
            .map_err(|_| "Mapping worker disconnected".into())
    }
    pub fn start(
        product_id: u32,
        container: String,
    ) -> anyhow::Result<(Self, mpsc::Receiver<Reply>)> {
        Self::start_with_registry(product_id, container, Client::spawn, Registry::global())
    }

    #[cfg(test)]
    fn start_with<T: Transport + 'static>(
        product_id: u32,
        container: String,
        factory: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
    ) -> anyhow::Result<(Self, mpsc::Receiver<Reply>)> {
        Self::start_with_registry(product_id, container, factory, Registry::default())
    }

    pub(super) fn start_with_registry<T: Transport + 'static>(
        product_id: u32,
        container: String,
        factory: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
        registry: Registry,
    ) -> anyhow::Result<(Self, mpsc::Receiver<Reply>)> {
        let lifecycle = lifecycle(product_id, &container)?;
        let (commands, receiver) = mpsc::channel::<Command>();
        let (sender, replies) = mpsc::channel();
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        let guard = WorkerGuard::start();
        thread::Builder::new()
            .name(format!("mapping-device-{product_id}"))
            .spawn(move || {
                let _guard = guard;
                // An outgoing worker restores/removes its registration before
                // the same target may be registered by a replacement worker.
                let _lifecycle = match lifecycle.lock() {
                    Ok(guard) => guard,
                    Err(_) => {
                        let message = "Mapping target lifecycle lock poisoned".to_owned();
                        let _ = sender.send(Reply::Failed(message.clone()));
                        let _ = sender.send(Reply::Stopped(Err(message)));
                        return;
                    }
                };
                if signal.load(Ordering::Acquire) {
                    let _ = sender.send(Reply::Stopped(Ok(())));
                    return;
                }
                let owner_guid = uuid::Uuid::new_v4().to_string();
                let mut client = match guarded(factory) {
                    Ok(client) => client,
                    Err(error) => {
                        let message = format!("{error:#}");
                        let _ = sender.send(Reply::Failed(message.clone()));
                        let _ = sender.send(Reply::Stopped(Err(message)));
                        return;
                    }
                };
                let mut control = match registry.register(owner_guid.clone()) {
                    Ok(control) => control,
                    Err(error) => {
                        let _ = sender.send(Reply::Failed(error.clone()));
                        let stopped = shutdown(&mut client);
                        drop(client);
                        let _ = sender.send(Reply::Stopped(stopped.and(Err(error))));
                        return;
                    }
                };
                let mut restore_on_stop = None;
                let result = guarded(|| -> anyhow::Result<()> {
                    anyhow::ensure!(!signal.load(Ordering::Acquire), "Mapping start canceled");
                    let response = client.request(ServiceRequest::MappingDeviceStart {
                        product_id,
                        device_container_id: container,
                        owner_guid: owner_guid.clone(),
                    })?;
                    registration(response, &owner_guid)?;
                    super::control::apply(&mut client, &control)?;
                    let _ = sender.send(Reply::Ready);
                    let mut draining = false;
                    while !signal.load(Ordering::Acquire) {
                        super::control::apply(&mut client, &control)?;
                        let next = match receiver.recv_timeout(Duration::from_millis(100)) {
                            Ok(Command::Publish(mut next)) => {
                                while let Ok(command) = receiver.try_recv() {
                                    match command {
                                        Command::Publish(newer) => next = newer,
                                        Command::Finish => {
                                            draining = true;
                                            break;
                                        }
                                    }
                                }
                                Some(next)
                            }
                            Ok(Command::Finish) => {
                                draining = true;
                                None
                            }
                            Err(mpsc::RecvTimeoutError::Timeout) => None,
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        };
                        if signal.load(Ordering::Acquire) {
                            break;
                        }
                        super::control::apply(&mut client, &control)?;
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
                                .and_then(|value| publication(value, &owner_guid))
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
                        if draining || signal.load(Ordering::Acquire) {
                            break;
                        }
                        let value = client.request(ServiceRequest::MappingDeviceEvents {
                            owner_guid: owner_guid.clone(),
                        })?;
                        let events = event_batch(value, &owner_guid)?;
                        if !events.is_empty() {
                            let _ = sender.send(Reply::Events(events));
                        }
                    }
                    Ok(())
                });
                if let Err(error) = result {
                    let _ = sender.send(Reply::Failed(format!("{error:#}")));
                }
                let restored = if let Some(document) = restore_on_stop {
                    let result = guarded(|| {
                        client
                            .request(ServiceRequest::MappingDevicePublish {
                                owner_guid: owner_guid.clone(),
                                document,
                            })
                            .and_then(|value| publication(value, &owner_guid))
                    })
                    .map_err(|error| format!("Temporary mapping cleanup: {error:#}"));
                    if let Err(error) = &result {
                        eprintln!("[mapping-device] {error}");
                        let _ = sender.send(Reply::Failed(error.clone()));
                    }
                    result
                } else {
                    Ok(())
                };
                // Shutdown owns partial registrations too, including a failed
                // addUsbDevice callback or missing driver-ready event.
                let stopped = shutdown(&mut client);
                let stopped = match (restored, stopped) {
                    (Ok(()), stopped) => stopped,
                    (Err(error), Ok(())) => Err(error),
                    (Err(error), Err(shutdown)) => Err(format!("{error}; {shutdown}")),
                };
                drop(client);
                let finished = control.finish(stopped.clone());
                let stopped = match (stopped, finished) {
                    (Ok(()), finished) => finished,
                    (Err(error), Ok(())) => Err(error),
                    (Err(error), Err(finished)) => Err(format!("{error}; {finished}")),
                };
                let _ = sender.send(Reply::Stopped(stopped));
            })?;
        Ok((
            Self {
                commands,
                admission: Mutex::new(Admission::Accepting),
                canceled,
            },
            replies,
        ))
    }
}

#[cfg(test)]
mod tests;
