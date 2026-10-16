//! Focused keyboard mapping capture owns an isolated native service worker.
//! Dropping the handle cancels polling and restores mapping/redirect state.
use razer_ipc::{KEY_CAPTURE_LIFECYCLE as LIFECYCLE, ServiceClient, ServiceRequest};
use serde_json::Value;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
static ACTIVE: AtomicUsize = AtomicUsize::new(0);
pub(crate) fn active_workers() -> usize {
    ACTIVE.load(Ordering::Acquire)
}
struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        ACTIVE.fetch_sub(1, Ordering::AcqRel);
    }
}
pub(crate) enum Reply {
    Ready(Value),
    Events(Vec<Value>),
    Failed(String),
}
pub(crate) struct Owner {
    canceled: Arc<AtomicBool>,
    events: mpsc::Receiver<Reply>,
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.canceled.store(true, Ordering::Release);
    }
}
impl Owner {
    pub(crate) fn start() -> anyhow::Result<Self> {
        let (sender, events) = mpsc::channel();
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        ACTIVE.fetch_add(1, Ordering::AcqRel);
        let guard = Guard;
        thread::Builder::new()
            .name("mapping-key-capture".into())
            .spawn(move || {
                let _guard = guard;
                // A quick blur/refocus must not let the previous cleanup enable
                // mappings while the replacement recorder owns suppression.
                let _lifecycle = match LIFECYCLE.lock() {
                    Ok(guard) => guard,
                    Err(_) => {
                        let _ =
                            sender.send(Reply::Failed("Capture lifecycle lock poisoned".into()));
                        return;
                    }
                };
                if signal.load(Ordering::Acquire) {
                    return;
                }
                let mut client = match ServiceClient::spawn() {
                    Ok(client) => client,
                    Err(error) => {
                        let _ = sender.send(Reply::Failed(format!("{error:#}")));
                        return;
                    }
                };
                let result = (|| -> anyhow::Result<()> {
                    anyhow::ensure!(!signal.load(Ordering::Acquire), "Key capture canceled");
                    let ready = client.request(ServiceRequest::StartMappingKeyCapture)?;
                    anyhow::ensure!(
                        ready["capturing"] == true,
                        "Key capture acknowledgement missing"
                    );
                    let _ = sender.send(Reply::Ready(ready));
                    while !signal.load(Ordering::Acquire) {
                        let observed = client.request(ServiceRequest::MappingKeyCaptureEvents)?;
                        let values = observed["events"]
                            .as_array()
                            .ok_or_else(|| anyhow::anyhow!("Key capture event array missing"))?;
                        if !values.is_empty() && sender.send(Reply::Events(values.clone())).is_err()
                        {
                            break;
                        }
                        thread::sleep(Duration::from_millis(16));
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    let _ = sender.send(Reply::Failed(format!("{error:#}")));
                }
                // Shutdown also restores partial registrations after startup failure.
                if let Err(error) = client.request(ServiceRequest::Shutdown) {
                    let message = format!("Key capture cleanup failed: {error:#}");
                    eprintln!("[mapping-key-capture] {message}");
                    let _ = sender.send(Reply::Failed(message));
                }
            })?;
        Ok(Self { canceled, events })
    }
    pub(crate) fn drain(&self) -> Vec<Reply> {
        self.events.try_iter().collect()
    }
}
