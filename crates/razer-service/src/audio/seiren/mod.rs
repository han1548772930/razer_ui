//! Local microphone workers own IPC, temporary routing and service release.
//! GUI owners retain cancellation and consume observations after identity checks.
pub mod capture;
pub mod playback;

use anyhow::ensure;
use razer_ipc::ServiceRequest;
use serde::Deserialize;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
};

static ACTIVE_WORKERS: AtomicUsize = AtomicUsize::new(0);

/// Counts accepted work until its native client and temporary resources release.
pub fn active_workers() -> usize {
    ACTIVE_WORKERS.load(Ordering::Acquire)
}

struct WorkerGuard;
impl WorkerGuard {
    fn new() -> Self {
        ACTIVE_WORKERS.fetch_add(1, Ordering::AcqRel);
        Self
    }
}
impl Drop for WorkerGuard {
    fn drop(&mut self) {
        ACTIVE_WORKERS.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Retains one worker through Stop, Shutdown, client release and file cleanup.
/// Dropping the handle cancels; process shutdown must still await active workers.
pub struct Session {
    canceled: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
impl Session {
    fn spawn(
        name: &str,
        canceled: Arc<AtomicBool>,
        run: impl FnOnce() + Send + 'static,
    ) -> anyhow::Result<Self> {
        let guard = WorkerGuard::new();
        let thread = thread::Builder::new().name(name.into()).spawn(move || {
            let _guard = guard;
            run();
        })?;
        Ok(Self {
            canceled,
            thread: Some(thread),
        })
    }

    pub fn cancel(&self) {
        self.canceled.store(true, Ordering::Release);
    }

    pub fn is_finished(&self) -> bool {
        self.thread.as_ref().is_none_or(JoinHandle::is_finished)
    }

    /// Run on an I/O executor, or after `is_finished()` confirms a completed join.
    pub fn join(mut self) -> anyhow::Result<()> {
        if let Some(thread) = self.thread.take() {
            ensure!(thread.join().is_ok(), "Seiren audio worker panicked");
        }
        Ok(())
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if self.thread.is_some() {
            self.cancel();
        }
    }
}

#[derive(Clone)]
struct Owner {
    session_id: u64,
    product_id: u32,
    canceled: Arc<AtomicBool>,
}
impl Owner {
    fn new(session_id: u64, product_id: u32, canceled: Arc<AtomicBool>) -> anyhow::Result<Self> {
        ensure!(session_id != 0, "Seiren audio session identity is zero");
        ensure!(
            razer_device::seiren_eq::capability(product_id).is_some(),
            "Seiren audio product has no verified local capability"
        );
        Ok(Self {
            session_id,
            product_id,
            canceled,
        })
    }
    fn is_canceled(&self) -> bool {
        self.canceled.load(Ordering::Acquire)
    }
}

fn guarded<T>(run: impl FnOnce() -> anyhow::Result<T>) -> anyhow::Result<T> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)) {
        Ok(result) => result,
        Err(payload) => {
            let message = payload
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| payload.downcast_ref::<&str>().copied())
                .unwrap_or("unknown panic payload");
            anyhow::bail!("Seiren audio worker panicked: {message}")
        }
    }
}

fn shutdown(
    mut dispatch: impl FnMut(ServiceRequest) -> anyhow::Result<serde_json::Value>,
) -> anyhow::Result<()> {
    #[derive(Deserialize)]
    struct Receipt {
        stopped: bool,
    }
    guarded(|| {
        let receipt: Receipt = serde_json::from_value(dispatch(ServiceRequest::Shutdown)?)?;
        ensure!(
            receipt.stopped,
            "Seiren audio service shutdown was not confirmed"
        );
        Ok(())
    })
}
