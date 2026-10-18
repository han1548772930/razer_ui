//! A retained product owner serializes explicit Rotation GET and SET requests.
use super::{Client, Transport, WorkerGuard, guarded, shutdown};
use razer_device::mapping_native_rotation::{RotationReply, RotationRequest};
use razer_ipc::{
    ServiceRequest,
    mouse_rotation::{NativeProgress, NativeRegistration, NativeTarget, NativeUpdate},
};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use uuid::Uuid;

type Lifecycle = Arc<Mutex<()>>;
static LIFECYCLES: Mutex<BTreeMap<(u32, Uuid), Weak<Mutex<()>>>> = Mutex::new(BTreeMap::new());

fn lifecycle(target: &NativeTarget) -> anyhow::Result<Lifecycle> {
    let key = (
        target.product_id(),
        Uuid::parse_str(target.container_id().trim_matches(['{', '}']))?,
    );
    let mut owners = LIFECYCLES
        .lock()
        .map_err(|_| anyhow::anyhow!("Rotation lifecycle registry poisoned"))?;
    owners.retain(|_, owner| owner.strong_count() > 0);
    if let Some(owner) = owners.get(&key).and_then(Weak::upgrade) {
        return Ok(owner);
    }
    let owner = Arc::new(Mutex::new(()));
    owners.insert(key, Arc::downgrade(&owner));
    Ok(owner)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequestId {
    owner: Uuid,
    sequence: u64,
}

#[derive(Debug)]
pub enum Reply {
    Ready,
    Completed {
        id: RequestId,
        result: Result<RotationReply, String>,
    },
    Failed(String),
    Stopped(Result<(), String>),
}

struct Command {
    id: RequestId,
    request: RotationRequest,
}
struct Admission {
    accepting: bool,
    sequence: u64,
}
struct Shared {
    admission: Mutex<Admission>,
    canceled: AtomicBool,
}
impl Shared {
    fn stop(&self) {
        let mut admission = self
            .admission
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        admission.accepting = false;
        self.canceled.store(true, Ordering::Release);
    }
    fn canceled(&self) -> bool {
        self.canceled.load(Ordering::Acquire)
    }
    fn publish(
        &self,
        sender: &mpsc::Sender<Reply>,
        id: RequestId,
        result: Result<RotationReply, String>,
    ) -> anyhow::Result<()> {
        let _admission = self
            .admission
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let result = if self.canceled() && result.is_ok() {
            Err("Rotation operation canceled".into())
        } else {
            result
        };
        sender.send(Reply::Completed { id, result })?;
        Ok(())
    }
}

pub struct Session {
    owner: Uuid,
    shared: Arc<Shared>,
    commands: mpsc::SyncSender<Command>,
    worker: Option<JoinHandle<Result<(), String>>>,
}
impl Session {
    pub fn start(target: NativeTarget) -> anyhow::Result<(Self, mpsc::Receiver<Reply>)> {
        Self::start_with(target, Client::spawn)
    }

    fn start_with<T: Transport + 'static>(
        target: NativeTarget,
        factory: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
    ) -> anyhow::Result<(Self, mpsc::Receiver<Reply>)> {
        target.validate()?;
        let lifecycle = lifecycle(&target)?;
        let owner = Uuid::new_v4();
        let shared = Arc::new(Shared {
            admission: Mutex::new(Admission {
                accepting: true,
                sequence: 0,
            }),
            canceled: AtomicBool::new(false),
        });
        let signal = shared.clone();
        let (commands, receiver) = mpsc::sync_channel::<Command>(128);
        let (sender, replies) = mpsc::channel();
        let guard = WorkerGuard::start();
        let worker = thread::Builder::new()
            .name("mapping-mouse-rotation".into())
            .spawn(move || {
                let _guard = guard;
                let result = guarded(|| -> anyhow::Result<()> {
                    let _lifecycle = lifecycle
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Rotation target lifecycle poisoned"))?;
                    if signal.canceled() {
                        return Ok(());
                    }
                    let mut client = factory()?;
                    let owner_guid = owner.to_string();
                    let mut registration_attempted = false;
                    // Registration can be accepted even if its response is lost.
                    let run = guarded(|| -> anyhow::Result<()> {
                        if signal.canceled() {
                            return Ok(());
                        }
                        registration_attempted = true;
                        let receipt: NativeRegistration = serde_json::from_value(client.request(
                            ServiceRequest::MappingRotationStart {
                                owner_guid: owner_guid.clone(),
                                target,
                            },
                        )?)?;
                        anyhow::ensure!(
                            receipt.matches(&owner_guid, true),
                            "Rotation registration was not acknowledged"
                        );
                        {
                            let _admission = signal
                                .admission
                                .lock()
                                .unwrap_or_else(|error| error.into_inner());
                            if !signal.canceled() {
                                sender.send(Reply::Ready)?;
                            }
                        }
                        while !signal.canceled() {
                            let command = match receiver.recv_timeout(Duration::from_millis(16)) {
                                Ok(command) => command,
                                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                            };
                            let result = guarded(|| {
                                execute(&mut client, &owner_guid, command.request, &signal)
                            })
                            .map_err(|error| format!("{error:#}"));
                            let failure = result.as_ref().err().cloned();
                            signal.publish(&sender, command.id, result.unwrap_or_else(Err))?;
                            if let Some(error) = failure {
                                if !signal.canceled() {
                                    anyhow::bail!(error);
                                }
                            }
                        }
                        Ok(())
                    });
                    signal.stop();
                    let mut errors = Vec::new();
                    if let Err(error) = run {
                        errors.push(format!("{error:#}"));
                    }
                    let stopped = guarded(|| -> anyhow::Result<()> {
                        if !registration_attempted {
                            return Ok(());
                        }
                        let receipt: NativeRegistration = serde_json::from_value(client.request(
                            ServiceRequest::MappingRotationStop {
                                owner_guid: owner_guid.clone(),
                            },
                        )?)?;
                        anyhow::ensure!(
                            receipt.matches(&owner_guid, false),
                            "Rotation stop was not acknowledged"
                        );
                        Ok(())
                    });
                    if let Err(error) = stopped {
                        errors.push(format!("Rotation unregister: {error:#}"));
                    }
                    if let Err(error) = shutdown(&mut client) {
                        errors.push(error);
                    }
                    drop(client);
                    anyhow::ensure!(errors.is_empty(), "{}", errors.join("; "));
                    Ok(())
                })
                .map_err(|error| format!("{error:#}"));
                signal.stop();
                for command in receiver.try_iter() {
                    let _ = signal.publish(
                        &sender,
                        command.id,
                        Err("Rotation owner stopped before submission".into()),
                    );
                }
                if let Err(error) = &result {
                    let _ = sender.send(Reply::Failed(error.clone()));
                }
                let _ = sender.send(Reply::Stopped(result.clone()));
                result
            })?;
        Ok((
            Self {
                owner,
                shared,
                commands,
                worker: Some(worker),
            },
            replies,
        ))
    }

    pub fn request(&self, request: RotationRequest) -> Result<RequestId, String> {
        let mut admission = self
            .shared
            .admission
            .lock()
            .map_err(|_| "Rotation admission poisoned")?;
        if !admission.accepting {
            return Err("Rotation owner is stopping".into());
        }
        let sequence = admission
            .sequence
            .checked_add(1)
            .ok_or("Rotation request identity exhausted")?;
        let id = RequestId {
            owner: self.owner,
            sequence,
        };
        self.commands
            .try_send(Command { id, request })
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => "Rotation request queue is full",
                mpsc::TrySendError::Disconnected(_) => "Rotation worker disconnected",
            })?;
        admission.sequence = sequence;
        Ok(id)
    }

    pub fn request_stop(&self) {
        self.shared.stop();
        if let Some(worker) = &self.worker {
            worker.thread().unpark();
        }
    }

    pub fn is_finished(&self) -> bool {
        self.worker.as_ref().is_none_or(JoinHandle::is_finished)
    }

    /// Wait for native cancellation, lease release and shared-client cleanup.
    /// The page owner calls this only from its background cleanup task.
    pub fn stop(mut self) -> Result<(), String> {
        self.request_stop();
        self.worker.take().map_or(Ok(()), |worker| {
            worker
                .join()
                .map_err(|_| "Rotation worker panicked".to_owned())?
        })
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.request_stop();
    }
}

fn execute(
    client: &mut impl Transport,
    owner_guid: &str,
    request: RotationRequest,
    signal: &Shared,
) -> anyhow::Result<Result<RotationReply, String>> {
    anyhow::ensure!(!signal.canceled(), "Rotation operation canceled");
    let mut update: NativeUpdate =
        serde_json::from_value(client.request(ServiceRequest::MappingRotationRequest {
            owner_guid: owner_guid.into(),
            request,
        })?)?;
    loop {
        anyhow::ensure!(!signal.canceled(), "Rotation operation canceled");
        match update.into_progress(owner_guid)? {
            NativeProgress::Failed { reason } => return Ok(Err(reason)),
            NativeProgress::Pending => {
                thread::park_timeout(Duration::from_millis(16));
                anyhow::ensure!(!signal.canceled(), "Rotation operation canceled");
                update = serde_json::from_value(client.request(
                    ServiceRequest::MappingRotationPoll {
                        owner_guid: owner_guid.into(),
                    },
                )?)?;
            }
            NativeProgress::Completed(reply) => {
                anyhow::ensure!(
                    match (request, reply) {
                        (RotationRequest::SetAngle(_), RotationReply::SetComplete) => true,
                        (RotationRequest::GetAngle, RotationReply::Angle { raw, bytes }) =>
                            i32::from_le_bytes(bytes) == raw,
                        _ => false,
                    },
                    "Rotation completion does not match its request"
                );
                return Ok(Ok(reply));
            }
        }
    }
}

#[cfg(test)]
mod tests;
