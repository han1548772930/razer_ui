//! One retained receiver notification owner per worker; poll never blocks on HID.
use anyhow::{Context as _, ensure};
use razer_device::mouse_indicator_led::{self as indicator, EventBatch, Notification, Target};
use serde_json::{Value, json};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread::{self, JoinHandle},
};
struct Operation {
    id: String,
    canceled: Arc<AtomicBool>,
    updates: Receiver<Notification>,
    worker: JoinHandle<anyhow::Result<()>>,
}
#[derive(Default)]
pub(super) struct Controller {
    active: Option<Operation>,
}
impl Controller {
    pub(super) fn shutdown(&mut self, cleanup: &mut super::shutdown::Cleanup) {
        if let Some(active) = self.active.take() {
            active.canceled.store(true, Ordering::Release);
            cleanup.record(
                "mouse_indicator_led",
                active
                    .worker
                    .join()
                    .map_err(|_| anyhow::anyhow!("Receiver quality cleanup worker panicked"))
                    .and_then(stopped),
            );
        }
    }

    pub(super) fn start(&mut self, id: String, target: Target) -> anyhow::Result<()> {
        ensure!(
            !id.is_empty() && id.len() <= 128 && !id.contains('\0'),
            "Receiver quality owner ID is invalid"
        );
        ensure!(
            self.active.is_none(),
            "Receiver quality owner is already active"
        );
        ensure!(
            indicator::capability(target.product_id()).is_some(),
            "Three-LED source is absent"
        );
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        let (sender, updates) = mpsc::sync_channel(256);
        let worker = thread::Builder::new()
            .name("mouse-receiver-quality".into())
            .spawn(move || {
                let mut publish = |quality| {
                    ensure!(
                        !signal.load(Ordering::Acquire),
                        "Receiver quality owner canceled"
                    );
                    sender
                        .try_send(quality)
                        .context("Receiver quality consumer is absent or full")
                };
                #[cfg(windows)]
                {
                    super::native::mouse_indicator_led_monitor(&target, &signal, &mut publish)
                }
                #[cfg(not(windows))]
                {
                    collection_monitor(&target, &signal, &mut publish)
                }
            })?;
        self.active = Some(Operation {
            id,
            canceled,
            updates,
            worker,
        });
        Ok(())
    }
    pub(super) fn poll(&mut self, id: &str) -> anyhow::Result<Value> {
        let active = self
            .active
            .as_ref()
            .context("Receiver quality owner is absent")?;
        ensure!(active.id == id, "Receiver quality owner ID differs");
        let mut notifications = active.updates.try_iter().collect::<Vec<_>>();
        if !active.worker.is_finished() {
            let updates = quality_updates(&notifications);
            return Ok(serde_json::to_value(EventBatch {
                operation_id: id.into(),
                updates,
                notifications,
                terminal: false,
                canceled: false,
                error: None,
            })?);
        }
        let active = self.active.take().unwrap();
        let result = active.worker.join();
        notifications.extend(active.updates.try_iter());
        let error = match result {
            Ok(Ok(())) => None,
            Ok(Err(error)) => Some(format!("{error:#}")),
            Err(_) => Some("Receiver quality reader terminated".into()),
        };
        let updates = quality_updates(&notifications);
        Ok(serde_json::to_value(EventBatch {
            operation_id: id.into(),
            updates,
            notifications,
            terminal: true,
            canceled: active.canceled.load(Ordering::Acquire),
            error,
        })?)
    }
    pub(super) fn stop(&mut self, id: &str) -> anyhow::Result<Value> {
        let active = self
            .active
            .as_ref()
            .context("Receiver quality owner is absent")?;
        ensure!(active.id == id, "Receiver quality owner ID differs");
        let active = self.active.take().unwrap();
        active.canceled.store(true, Ordering::Release);
        let result = active
            .worker
            .join()
            .map_err(|_| anyhow::anyhow!("Receiver quality reader terminated during cleanup"))?;
        let cleanup_error = stopped(result).err().map(|error| format!("{error:#}"));
        Ok(json!({"operation_id":id,"stopped":true,"cleanup_error":cleanup_error}))
    }
}
fn stopped(outcome: anyhow::Result<()>) -> anyhow::Result<()> {
    match outcome {
        Err(error) if error.to_string() == "Receiver quality owner canceled" => Ok(()),
        outcome => outcome,
    }
}
fn quality_updates(notifications: &[Notification]) -> Vec<indicator::Quality> {
    notifications
        .iter()
        .filter_map(|notification| match notification {
            Notification::ConnectionQuality { quality } => Some(*quality),
            _ => None,
        })
        .collect()
}
impl Drop for Controller {
    fn drop(&mut self) {
        if let Some(active) = self.active.take() {
            active.canceled.store(true, Ordering::Release);
            let _ = active.worker.join();
        }
    }
}
#[cfg(not(windows))]
fn collection_monitor(
    target: &Target,
    canceled: &AtomicBool,
    publish: &mut impl FnMut(Notification) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    use razer_device::{
        backend::{HidBackend, InterruptTransport},
        receiver_pairing::HardwareEvents,
    };
    use razer_hid::with_backend;
    use std::time::Duration;
    let Target::Collection { node, product_id } = target else {
        anyhow::bail!("Receiver ContainerId needs its platform event adapter");
    };
    let source = indicator::capability(*product_id).context("Three-LED source is absent")?;
    let cap = source.transport(u32::from(node.product_id))?;
    ensure!(
        node.vendor_id == cap.vendor_id && node.interface_number == i32::from(cap.claim_interface),
        "Receiver quality source interface differs"
    );
    let validate = || {
        ensure!(
            !canceled.load(Ordering::Acquire),
            "Receiver quality owner canceled"
        );
        let nodes = with_backend(|backend| backend.enumerate())?;
        ensure!(
            nodes.iter().filter(|current| *current == node).count() == 1,
            "Receiver quality collection changed"
        );
        Ok(())
    };
    struct Events<'a> {
        node: &'a razer_device::backend::HidNode,
        reader: Option<Box<dyn InterruptTransport>>,
    }
    impl HardwareEvents for Events<'_> {
        fn begin(&mut self) -> anyhow::Result<()> {
            let reader = with_backend(|backend| backend.open_interrupt(self.node))?;
            let lengths = reader.report_lengths();
            ensure!(
                [5, 9].iter().any(|id| lengths
                    .input
                    .get(id)
                    .is_some_and(|length| (3..=100).contains(length))),
                "Observed receiver collection lacks its notification Input Report; physical sibling selection requires its platform identity adapter"
            );
            self.reader = Some(reader);
            Ok(())
        }
        fn receive(&mut self, timeout: Duration) -> anyhow::Result<Option<Vec<u8>>> {
            let mut buffer = [0u8; 100];
            let count = self
                .reader
                .as_ref()
                .context("Receiver quality interrupt is absent")?
                .read(&mut buffer, timeout.as_millis() as u32)?;
            ensure!(
                count <= buffer.len(),
                "Receiver quality interrupt exceeds its buffer"
            );
            Ok((count > 0).then(|| buffer[..count].to_vec()))
        }
        fn finish(&mut self) -> anyhow::Result<()> {
            self.reader = None;
            Ok(())
        }
    }
    indicator::monitor_notifications(&mut Events { node, reader: None }, &validate, publish)
}
