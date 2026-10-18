//! Retained local IPC worker. All socket and restoration work runs off the caller thread.
use anyhow::{Context as _, Result, ensure};
use razer_device::audio_surround::{Identity, NativeCall, Product, observed::Observations};
use serde::Serialize;
use std::{sync::{Arc, Mutex, mpsc}, thread::{self, JoinHandle}};

#[path = "worker.rs"]
mod worker;

#[derive(Debug, Clone, Default, Serialize)]
pub(super) struct Snapshot {
    pub connected: bool,
    pub version: Option<String>,
    pub observations: Observations,
    pub front_endpoint: Option<String>,
    pub signal_requested: Option<bool>,
    pub signal_delivered: Option<bool>,
    pub failure: Option<String>,
}

pub(super) enum Command {
    Call(NativeCall, Arc<std::sync::atomic::AtomicBool>, mpsc::SyncSender<Result<bool>>),
    Shutdown(mpsc::SyncSender<Result<()>>),
}

pub(super) struct Session {
    sender: tokio::sync::mpsc::UnboundedSender<Command>,
    shared: Arc<Mutex<Snapshot>>,
    thread: Option<JoinHandle<()>>,
}
impl Session {
    pub fn open(identity: &Identity) -> Result<Self> {
        let product = identity.product();
        let shared = Arc::new(Mutex::new(Snapshot::default()));
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        let (started, ready) = mpsc::sync_channel(1);
        let state = shared.clone();
        let thread = thread::Builder::new().name(format!("surround-{}", u32::from(product))).spawn(move || {
            let result = tokio::runtime::Builder::new_current_thread().enable_all().build();
            match result {
                Ok(runtime) => runtime.block_on(worker::run(product, receiver, state, started)),
                Err(error) => { let _ = started.send(Err(error.into())); }
            }
        })?;
        let mut session = Self { sender, shared, thread: Some(thread) };
        match ready.recv().context("Surround worker exited before initialization")? {
            Ok(()) => Ok(session),
            Err(error) => { if let Some(thread) = session.thread.take() { let _ = thread.join(); } Err(error) }
        }
    }
    pub fn snapshot(&self) -> Result<Snapshot> {
        Ok(self.shared.lock().map_err(|_| anyhow::anyhow!("Surround state lock poisoned"))?.clone())
    }
    pub fn call(&mut self, call: &NativeCall, validate: &dyn Fn() -> Result<()>) -> Result<bool> {
        validate()?;
        ensure!(self.snapshot()?.connected, "Surround service state is disconnected");
        let cancellation = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (reply, result) = mpsc::sync_channel(1);
        self.sender.send(Command::Call(call.clone(), cancellation.clone(), reply)).context("Surround worker stopped")?;
        let mut cancelled = None;
        loop {
            match result.recv_timeout(std::time::Duration::from_millis(25)) {
                Ok(result) => return if let Some(error) = cancelled { Err(error) } else { result },
                Err(mpsc::RecvTimeoutError::Disconnected) => anyhow::bail!("Surround worker stopped during operation"),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if cancelled.is_none() { if let Err(error) = validate() {
                        cancellation.store(true, std::sync::atomic::Ordering::Release);
                        cancelled = Some(error);
                    } }
                }
            }
        }
    }
    pub fn close(&mut self) -> Result<()> {
        let Some(thread) = self.thread.take() else { return Ok(()); };
        let (reply, result) = mpsc::sync_channel(1);
        let requested = self.sender.send(Command::Shutdown(reply));
        let cleanup = match requested {
            Ok(()) => result.recv().context("Surround worker stopped during cleanup")?,
            Err(_) => Err(anyhow::anyhow!("Surround worker stopped before cleanup")),
        };
        thread.join().map_err(|_| anyhow::anyhow!("Surround worker panicked"))?;
        cleanup
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if let Err(error) = self.close() { eprintln!("[Surround cleanup] {error:#}"); }
    }
}

pub(super) fn queue_name(product: Product) -> &'static str {
    match product { Product::Clio => "carol", Product::Phantom => "clio_x", Product::VirtualRoute => "soma_pro" }
}
