//! One retained app-engine owner publishes shortcut dependencies and mappings.
//! Page navigation does not own this actor. Cancellation drains native cleanup.
use super::{Transport, WorkerGuard, guarded, shutdown};
use razer_ipc::mapping_control::Registry;
use razer_ipc::shortcuts::{
    EventBatch, PublicationReceipt, RegistrationReceipt, StopReceipt, UnsupportedMapping,
};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_model::{
    macro_library::MacroLibraryFile,
    shortcuts::{Compiler, Shortcut},
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

static LIFECYCLE: Mutex<()> = Mutex::new(());

pub enum Reply {
    Ready,
    /// Confirms native storage acknowledgements, not execution or device output.
    Published {
        revision: u64,
        result: Result<(), String>,
    },
    Unsupported(UnsupportedMapping),
    Failed(String),
    Stopped(Result<(), String>),
}

struct Publish {
    revision: u64,
    shortcuts: Vec<Shortcut>,
    library: MacroLibraryFile,
}

enum Command {
    Publish(Publish),
    Finish,
}

enum Admission {
    Accepting,
    Draining,
}

pub struct Session {
    commands: mpsc::Sender<Command>,
    admission: Mutex<Admission>,
    stop: Arc<AtomicBool>,
    finished: Arc<AtomicBool>,
}

impl Drop for Session {
    fn drop(&mut self) {
        self.request_stop();
    }
}

impl Session {
    pub fn start() -> anyhow::Result<(Self, mpsc::Receiver<Reply>)> {
        Self::start_with_registry(ServiceClient::spawn, Registry::global())
    }

    pub fn publish(
        &self,
        revision: u64,
        shortcuts: Vec<Shortcut>,
        library: MacroLibraryFile,
    ) -> Result<(), String> {
        let admission = self
            .admission
            .lock()
            .map_err(|_| "Global shortcut admission lock poisoned")?;
        if !matches!(*admission, Admission::Accepting)
            || self.stop.load(Ordering::Acquire)
            || self.finished.load(Ordering::Acquire)
        {
            return Err("Global shortcut owner is stopping".into());
        }
        self.commands
            .send(Command::Publish(Publish {
                revision,
                shortcuts,
                library,
            }))
            .map_err(|_| "Global shortcut owner disconnected".into())
    }

    pub fn request_stop(&self) {
        let mut admission = self
            .admission
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *admission = Admission::Draining;
        self.stop.store(true, Ordering::Release);
    }

    /// Finish admitted publications before removing the native registration.
    /// Retain this session until completion; dropping it requests cancellation.
    pub fn request_stop_after_pending(&self) -> Result<(), String> {
        let mut admission = self
            .admission
            .lock()
            .map_err(|_| "Global shortcut admission lock poisoned")?;
        if matches!(*admission, Admission::Draining) || self.finished.load(Ordering::Acquire) {
            return Ok(());
        }
        *admission = Admission::Draining;
        self.commands
            .send(Command::Finish)
            .map_err(|_| "Global shortcut owner disconnected".into())
    }
    pub fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }

    #[cfg(test)]
    fn start_with<T: Transport + 'static>(
        factory: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
    ) -> anyhow::Result<(Self, mpsc::Receiver<Reply>)> {
        Self::start_with_registry(factory, Registry::default())
    }

    fn start_with_registry<T: Transport + 'static>(
        factory: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
        registry: Registry,
    ) -> anyhow::Result<(Self, mpsc::Receiver<Reply>)> {
        let (commands, input) = mpsc::channel();
        let (output, replies) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));
        let cancellation = stop.clone();
        let completion = finished.clone();
        let worker = WorkerGuard::start();
        std::thread::Builder::new()
            .name("global-shortcut-owner".into())
            .spawn(move || {
                let _worker = worker;
                let result = match LIFECYCLE.lock() {
                    Ok(_lifecycle) => {
                        if cancellation.load(Ordering::Acquire) {
                            Ok(())
                        } else {
                            run(factory, &input, &output, &cancellation, &registry)
                        }
                    }
                    Err(_) => Err("Global shortcut lifecycle lock poisoned".into()),
                };
                completion.store(true, Ordering::Release);
                let _ = output.send(Reply::Stopped(result));
            })?;
        Ok((
            Self {
                commands,
                admission: Mutex::new(Admission::Accepting),
                stop,
                finished,
            },
            replies,
        ))
    }
}

fn run<T: Transport>(
    factory: impl FnOnce() -> anyhow::Result<T>,
    input: &mpsc::Receiver<Command>,
    output: &mpsc::Sender<Reply>,
    stop: &AtomicBool,
    registry: &Registry,
) -> Result<(), String> {
    let mut client = guarded(factory).map_err(|error| format!("{error:#}"))?;
    let owner = uuid::Uuid::new_v4().to_string();
    let mut control = match registry.register(owner.clone()) {
        Ok(control) => control,
        Err(error) => {
            let stopped = shutdown(&mut client);
            drop(client);
            return Err(match stopped {
                Ok(()) => error,
                Err(cleanup) => format!("{error}; {cleanup}"),
            });
        }
    };
    let mut start_attempted = false;
    let result = guarded(|| -> anyhow::Result<()> {
        if stop.load(Ordering::Acquire) {
            return Ok(());
        }
        start_attempted = true;
        let ready: RegistrationReceipt =
            serde_json::from_value(client.request(ServiceRequest::GlobalShortcutStart {
                owner_guid: owner.clone(),
            })?)?;
        anyhow::ensure!(
            ready.registered() && ready.owner_guid() == owner,
            "Global shortcut registration was not confirmed for this owner"
        );
        super::control::apply(&mut client, &control)?;
        output.send(Reply::Ready)?;
        let mut revision = None;
        let mut compiler = Compiler::new();
        while !stop.load(Ordering::Acquire) {
            super::control::apply(&mut client, &control)?;
            let command = match input.recv_timeout(Duration::from_millis(50)) {
                Ok(Command::Publish(command)) => Some(command),
                Ok(Command::Finish) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            };
            if stop.load(Ordering::Acquire) {
                break;
            }
            super::control::apply(&mut client, &control)?;
            if let Some(command) = command {
                let next = command.revision;
                let result = (|| -> anyhow::Result<()> {
                    anyhow::ensure!(
                        revision.is_none_or(|previous| next > previous),
                        "Global shortcut publication revision is stale"
                    );
                    // Failed native publications may already have changed a dependency.
                    // Retire their revision too; an explicit later revision may retry.
                    revision = Some(next);
                    let document = compiler
                        .compile(&command.shortcuts, &command.library)
                        .map_err(anyhow::Error::msg)?;
                    if stop.load(Ordering::Acquire) {
                        anyhow::bail!("Global shortcut publication canceled");
                    }
                    let receipt: PublicationReceipt = serde_json::from_value(client.request(
                        ServiceRequest::GlobalShortcutPublish {
                            owner_guid: owner.clone(),
                            revision: next,
                            document,
                        },
                    )?)?;
                    anyhow::ensure!(
                        receipt.published()
                            && receipt.owner_guid() == owner
                            && receipt.revision() == next,
                        "Global shortcut publication receipt does not match owner and revision"
                    );
                    Ok(())
                })()
                .map_err(|error| format!("{error:#}"));
                if output
                    .send(Reply::Published {
                        revision: next,
                        result,
                    })
                    .is_err()
                {
                    break;
                }
                if client.is_stopped() {
                    anyhow::bail!("Global shortcut transport stopped during publication");
                }
            }
            if stop.load(Ordering::Acquire) {
                break;
            }
            let batch: EventBatch =
                serde_json::from_value(client.request(ServiceRequest::GlobalShortcutPoll {
                    owner_guid: owner.clone(),
                })?)?;
            anyhow::ensure!(
                batch.owner_guid() == owner,
                "Global shortcut event owner changed"
            );
            for event in batch.into_events() {
                anyhow::ensure!(
                    event.event_type() == 3,
                    "Unexpected global shortcut event type"
                );
                output.send(Reply::Unsupported(event))?;
            }
        }
        Ok(())
    })
    .map_err(|error| format!("{error:#}"));
    if let Err(error) = &result {
        let _ = output.send(Reply::Failed(error.clone()));
    }
    let stop_result = if start_attempted {
        guarded(|| -> anyhow::Result<()> {
            let receipt: StopReceipt =
                serde_json::from_value(client.request(ServiceRequest::GlobalShortcutStop {
                    owner_guid: owner.clone(),
                })?)?;
            anyhow::ensure!(
                receipt.stopped() && receipt.owner_guid() == owner,
                "Global shortcut stop was not confirmed for this owner"
            );
            Ok(())
        })
        .map_err(|error| format!("{error:#}"))
    } else {
        Ok(())
    };
    let shutdown_result = shutdown(&mut client);
    drop(client);
    let cleanup_errors: Vec<_> = [stop_result, shutdown_result]
        .into_iter()
        .filter_map(Result::err)
        .collect();
    let cleanup = if cleanup_errors.is_empty() {
        Ok(())
    } else {
        Err(cleanup_errors.join("; "))
    };
    let finished = control.finish(cleanup.clone());
    let errors: Vec<_> = [result, cleanup, finished]
        .into_iter()
        .filter_map(Result::err)
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

#[cfg(test)]
mod tests;
