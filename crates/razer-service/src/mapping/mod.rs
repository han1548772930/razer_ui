//! Service-backed mapping publication and focused key capture lifecycles.

mod control;
pub(crate) mod owner;
pub use owner::Client;
pub mod device;
pub mod key_capture;
pub mod mode_capture;
pub mod recorder;
mod recorder_owner;
pub mod shortcuts;

use razer_ipc::{ServiceClient, ServiceRequest};
use serde::Deserialize;
use serde_json::Value;
use std::sync::atomic::{AtomicUsize, Ordering};

static ACTIVE_WORKERS: AtomicUsize = AtomicUsize::new(0);

fn lock_lifecycle(
    canceled: &std::sync::atomic::AtomicBool,
) -> anyhow::Result<std::sync::MutexGuard<'static, ()>> {
    loop {
        anyhow::ensure!(
            !canceled.load(Ordering::Acquire),
            "Capture lifecycle canceled"
        );
        match razer_ipc::KEY_CAPTURE_LIFECYCLE.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(std::sync::TryLockError::WouldBlock) => {
                std::thread::sleep(std::time::Duration::from_millis(16))
            }
            Err(std::sync::TryLockError::Poisoned(_)) => {
                anyhow::bail!("Capture lifecycle lock poisoned")
            }
        }
    }
}

/// Includes workers completing restoration and child-process cleanup.
pub fn active_workers() -> usize {
    ACTIVE_WORKERS.load(Ordering::Acquire)
}

/// Retained mapping owners outside this crate share the same typed control
/// acknowledgement and cleanup contract. Call only from their I/O worker.
pub fn apply_execution_control(
    owner: &razer_ipc::mapping_control::Owner,
    request: impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<()> {
    control::apply_with(owner, request)
}

struct WorkerGuard;

impl WorkerGuard {
    fn start() -> Self {
        ACTIVE_WORKERS.fetch_add(1, Ordering::AcqRel);
        Self
    }
}

impl Drop for WorkerGuard {
    fn drop(&mut self) {
        ACTIVE_WORKERS.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Tests use an in-memory implementation; production retains the owned IPC client.
trait Transport {
    fn request(&mut self, request: ServiceRequest) -> anyhow::Result<Value>;
    fn is_stopped(&self) -> bool;
}

impl Transport for ServiceClient {
    fn request(&mut self, request: ServiceRequest) -> anyhow::Result<Value> {
        ServiceClient::request(self, request)
    }

    fn is_stopped(&self) -> bool {
        ServiceClient::is_stopped(self)
    }
}

/// Keep an unexpected Rust unwind inside the owner that can restore temporary
/// state. Process aborts and native faults remain outside this boundary.
pub(crate) fn guarded<T>(operation: impl FnOnce() -> anyhow::Result<T>) -> anyhow::Result<T> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation)) {
        Ok(result) => result,
        Err(payload) => {
            let message = payload
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| payload.downcast_ref::<&str>().copied())
                .unwrap_or("unknown panic payload");
            Err(anyhow::anyhow!(
                "Mapping service worker panicked: {message}"
            ))
        }
    }
}

fn shutdown(client: &mut impl Transport) -> Result<(), String> {
    #[derive(Deserialize)]
    struct Receipt {
        stopped: bool,
    }

    guarded(|| -> anyhow::Result<()> {
        let receipt: Receipt = serde_json::from_value(client.request(ServiceRequest::Shutdown)?)?;
        anyhow::ensure!(
            receipt.stopped,
            "Mapping service cleanup was not acknowledged"
        );
        Ok(())
    })
    .map_err(|error| format!("{error:#}"))
}

#[cfg(test)]
mod test_transport;
