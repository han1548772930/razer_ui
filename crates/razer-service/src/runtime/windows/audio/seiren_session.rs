//! Serial Marie session. All driver, COM and mixer resources live on its worker;
//! callers exchange owned commands and results, never native function pointers.
use super::{driver::Driver, endpoint::Endpoint};
use anyhow::{Context, Result, ensure};
use razer_device::{
    seiren_driver::{self as protocol, Product, Transport},
    seiren_eq::{NativeOperation, NativeReply, NativeTransport},
    seiren_mixer::{self as routing, EndpointObservation, Mixer, Write},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[path = "seiren_session_owner.rs"]
mod owner;
use owner::Owner;

enum Command {
    Call(NativeOperation, mpsc::SyncSender<Result<NativeReply>>),
    Events(mpsc::SyncSender<Result<EventBatch>>),
    Stop(mpsc::SyncSender<Result<()>>),
}
pub(super) struct EventBatch {
    pub(super) events: Vec<Value>,
    pub(super) diagnostics: Vec<String>,
    pub(super) observed: routing::LiveState,
}
struct Worker {
    sender: mpsc::Sender<Command>,
    join: JoinHandle<()>,
}
pub(super) struct Session {
    worker: Mutex<Option<Worker>>,
    stopping: Arc<AtomicBool>,
}
impl Session {
    pub(super) fn open(product_id: u32, container: &str) -> Result<Self> {
        let product = Product::from_id(product_id)?;
        ensure!(!container.is_empty(), "Marie observed container is absent");
        let container = container.to_owned();
        let stopping = Arc::new(AtomicBool::new(false));
        let worker_stopping = stopping.clone();
        let (sender, receiver) = mpsc::channel();
        let join = thread::Builder::new()
            .name(format!("marie-{product_id}"))
            .spawn(move || {
                let mut owner: Option<Owner> = None;
                let mut next_poll = Instant::now() + Duration::from_millis(180);
                loop {
                    let command =
                        receiver.recv_timeout(next_poll.saturating_duration_since(Instant::now()));
                    match command {
                        Ok(Command::Call(operation, reply)) => {
                            let result = match operation {
                                NativeOperation::DeviceInitialize => {
                                    if owner.is_some() {
                                        Ok(NativeReply::Boolean(true))
                                    } else {
                                        Owner::open(
                                            product_id,
                                            product,
                                            &container,
                                            worker_stopping.clone(),
                                        )
                                        .map(|opened| {
                                            owner = Some(opened);
                                            NativeReply::Boolean(true)
                                        })
                                    }
                                }
                                NativeOperation::DeviceTerminate => {
                                    let result = owner.as_mut().map(Owner::close).transpose();
                                    if result.is_ok() {
                                        owner = None;
                                    }
                                    result.map(|_| NativeReply::Boolean(true))
                                }
                                _ => owner
                                    .as_mut()
                                    .context("Marie session is not initialized")
                                    .and_then(|owner| owner.call(&operation)),
                            };
                            let _ = reply.send(result);
                        }
                        Ok(Command::Events(reply)) => {
                            let result = owner
                                .as_mut()
                                .context("Marie session is not initialized")
                                .and_then(Owner::drain);
                            let _ = reply.send(result);
                        }
                        Ok(Command::Stop(reply)) => {
                            let result = owner.as_mut().map(Owner::close).transpose().map(|_| ());
                            drop(owner.take());
                            let _ = reply.send(result);
                            break;
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                    if Instant::now() >= next_poll {
                        if let Some(owner) = owner.as_mut() {
                            owner.poll();
                        }
                        next_poll = Instant::now() + Duration::from_millis(180);
                    }
                }
            })
            .context("Start Marie session worker")?;
        Ok(Self {
            worker: Mutex::new(Some(Worker { sender, join })),
            stopping,
        })
    }
    // No vendor library is loaded, so no vendor DLL version is reported.
    pub(super) fn version(&self) -> Option<&str> {
        None
    }
    pub(super) fn drain(&self) -> Result<EventBatch> {
        let (reply, receiver) = mpsc::sync_channel(1);
        let guard = self
            .worker
            .lock()
            .map_err(|_| anyhow::anyhow!("Marie worker owner poisoned"))?;
        guard
            .as_ref()
            .context("Marie session stopped")?
            .sender
            .send(Command::Events(reply))?;
        receiver
            .recv()
            .context("Marie event worker stopped before responding")?
    }
    fn stop(&mut self) -> Result<()> {
        self.stopping.store(true, Ordering::Release);
        let worker = self
            .worker
            .get_mut()
            .map_err(|_| anyhow::anyhow!("Marie worker owner poisoned"))?
            .take();
        if let Some(worker) = worker {
            let (reply, receiver) = mpsc::sync_channel(1);
            let sent = worker.sender.send(Command::Stop(reply));
            let result = if sent.is_ok() {
                receiver
                    .recv()
                    .context("Marie cleanup worker stopped before responding")
                    .and_then(|result| result)
            } else {
                Err(anyhow::anyhow!("Marie worker already exited"))
            };
            worker
                .join
                .join()
                .map_err(|_| anyhow::anyhow!("Marie worker panicked during cleanup"))?;
            result?;
        }
        Ok(())
    }
}
impl NativeTransport for Session {
    fn call(&self, operation: &NativeOperation) -> Result<NativeReply> {
        let (reply, receiver) = mpsc::sync_channel(1);
        let guard = self
            .worker
            .lock()
            .map_err(|_| anyhow::anyhow!("Marie worker owner poisoned"))?;
        guard
            .as_ref()
            .context("Marie session stopped")?
            .sender
            .send(Command::Call(operation.clone(), reply))?;
        receiver
            .recv()
            .context("Marie worker stopped before responding")?
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            eprintln!("[Marie cleanup] {error:#}");
        }
    }
}
