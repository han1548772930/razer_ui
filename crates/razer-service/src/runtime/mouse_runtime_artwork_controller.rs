//! Retained mouse artwork reader. IPC polling never waits on HID retries.
use anyhow::{Context as _, ensure};
use razer_device::mouse_runtime_artwork::{self as artwork, Target, Update};
use serde_json::{Value, json};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
};

struct Operation {
    id: String,
    target: Target,
    canceled: Arc<AtomicBool>,
    updates: Receiver<Update>,
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
                "mouse_runtime_artwork",
                active
                    .worker
                    .join()
                    .map_err(|_| anyhow::anyhow!("Mouse artwork cleanup worker panicked"))
                    .and_then(stopped),
            );
        }
    }

    pub(super) fn start(&mut self, id: String, target: Target) -> anyhow::Result<()> {
        ensure!(
            !id.is_empty() && id.len() <= 128 && !id.contains('\0'),
            "Mouse artwork owner ID is invalid"
        );
        ensure!(
            self.active.is_none(),
            "Mouse artwork owner is already active"
        );
        ensure!(
            artwork::source(target.product_id()).is_some(),
            "No current hardware-module capability for this product"
        );
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        let selected = target.clone();
        let (publish, updates) = mpsc::sync_channel(256);
        let worker = thread::Builder::new()
            .name("mouse-runtime-artwork".into())
            .spawn(move || {
                let mut publish = |update| emit(&publish, &signal, update);
                #[cfg(windows)]
                {
                    super::native::mouse_runtime_artwork(&selected, &signal, &mut publish)
                }
                #[cfg(not(windows))]
                {
                    collection_monitor(&selected, &signal, &mut publish)
                }
            })?;
        self.active = Some(Operation {
            id,
            target,
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
            .context("Mouse artwork owner is absent")?;
        ensure!(active.id == id, "Mouse artwork owner ID differs");
        let updates = active.updates.try_iter().collect::<Vec<_>>();
        if !active.worker.is_finished() {
            return Ok(
                json!({"operation_id":id,"target":active.target,"updates":updates,"terminal":false}),
            );
        }
        let active = self.active.take().unwrap();
        let result = active.worker.join();
        // A final publish can race the first channel drain while the thread
        // exits, so collect again after joining the already-finished worker.
        let mut updates = updates;
        updates.extend(active.updates.try_iter());
        let error = match result {
            Ok(Ok(())) => None,
            Ok(Err(error)) => Some(format!("{error:#}")),
            Err(_) => Some("Mouse artwork reader thread terminated".into()),
        };
        Ok(
            json!({"operation_id":id,"target":active.target,"updates":updates,"terminal":true,"canceled":active.canceled.load(Ordering::Acquire),"error":error}),
        )
    }

    pub(super) fn stop(&mut self, id: &str) -> anyhow::Result<Value> {
        let active = self
            .active
            .as_ref()
            .context("Mouse artwork owner is absent")?;
        ensure!(active.id == id, "Mouse artwork owner ID differs");
        let active = self.active.take().unwrap();
        active.canceled.store(true, Ordering::Release);
        let result = active.worker.join();
        let outcome = result.map_err(|_| {
            anyhow::anyhow!("Mouse artwork reader thread terminated during cleanup")
        })?;
        let cleanup_error = stopped(outcome).err().map(|error| format!("{error:#}"));
        Ok(
            json!({"operation_id":id,"target":active.target,"stopped":true,"cleanup_error":cleanup_error}),
        )
    }
}

fn stopped(outcome: anyhow::Result<()>) -> anyhow::Result<()> {
    match outcome {
        Err(error) if error.to_string() == "Mouse artwork owner canceled" => Ok(()),
        outcome => outcome,
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        if let Some(active) = self.active.take() {
            active.canceled.store(true, Ordering::Release);
            let _ = active.worker.join();
        }
    }
}

fn emit(sender: &SyncSender<Update>, canceled: &AtomicBool, update: Update) -> anyhow::Result<()> {
    ensure!(
        !canceled.load(Ordering::Acquire),
        "Mouse artwork owner canceled"
    );
    sender
        .try_send(update)
        .context("Mouse artwork update consumer is unavailable or its queue is full")
}

#[cfg(not(windows))]
fn collection_monitor(
    target: &Target,
    canceled: &AtomicBool,
    publish: &mut impl FnMut(Update) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    use razer_device::{
        backend::{HidBackend, InterruptTransport},
        mouse_runtime_artwork::{Query, Transport},
        receiver_pairing::HardwareEvents,
    };
    use razer_hid::with_backend;
    use std::time::Duration;
    let Target::Collection { node, product_id } = target else {
        anyhow::bail!("Windows ContainerId artwork target requires its Windows adapter");
    };
    let source = artwork::source(*product_id).context("Mouse artwork source is absent")?;
    let cap = source.transport(u32::from(node.product_id), false)?;
    let guard = || {
        ensure!(
            !canceled.load(Ordering::Acquire),
            "Mouse artwork owner canceled"
        );
        let nodes = with_backend(|backend| backend.enumerate())?;
        ensure!(
            nodes.iter().filter(|current| *current == node).count() == 1,
            "Mouse artwork collection changed"
        );
        Ok(())
    };
    struct Feature<'a> {
        node: &'a razer_device::backend::HidNode,
        cap: razer_device::device_reads::DeviceReadCapability,
        sequence: u8,
    }
    impl Transport for Feature<'_> {
        fn query(
            &mut self,
            query: Query,
            validate: &dyn Fn() -> anyhow::Result<()>,
        ) -> anyhow::Result<u8> {
            validate()?;
            let device = with_backend(|backend| {
                backend.open_validated(self.node, std::rc::Rc::new(validate))
            })?;
            ensure!(
                device.report_lengths()?.feature.get(&0) == Some(&91),
                "Mouse artwork Feature Report size differs"
            );
            let transaction = self.cap.transaction_prefix | self.sequence;
            self.sequence = (self.sequence + 1) % 31;
            artwork::query_hid(device.as_ref(), &self.cap, transaction, query, &|| {
                validate()
            })
        }
    }
    struct Events<'a> {
        node: &'a razer_device::backend::HidNode,
        reader: Option<Box<dyn InterruptTransport>>,
    }
    impl HardwareEvents for Events<'_> {
        fn begin(&mut self) -> anyhow::Result<()> {
            let reader = with_backend(|backend| backend.open_interrupt(self.node))?;
            ensure!(
                reader
                    .report_lengths()
                    .input
                    .values()
                    .any(|length| *length > 0),
                "Observed collection has no interrupt input; sibling selection requires physical identity from the platform adapter"
            );
            self.reader = Some(reader);
            Ok(())
        }
        fn receive(&mut self, timeout: Duration) -> anyhow::Result<Option<Vec<u8>>> {
            let mut bytes = [0; 100];
            let count = self
                .reader
                .as_ref()
                .context("Mouse artwork interrupt is absent")?
                .read(&mut bytes, timeout.as_millis().try_into()?)?;
            ensure!(
                count <= bytes.len(),
                "Mouse artwork interrupt exceeds its buffer"
            );
            Ok((count > 0).then(|| bytes[..count].to_vec()))
        }
        fn finish(&mut self) -> anyhow::Result<()> {
            self.reader = None;
            Ok(())
        }
    }
    guard()?;
    ensure!(
        node.vendor_id == cap.vendor_id
            && source.accepts(u32::from(node.product_id))
            && node.interface_number == i32::from(source.interface()),
        "Mouse artwork collection source differs"
    );
    artwork::monitor(
        source,
        u32::from(node.product_id),
        &mut Feature {
            node,
            cap,
            sequence: 0,
        },
        &mut Events { node, reader: None },
        &guard,
        publish,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use razer_device::{backend::HidNode, mouse_runtime_artwork::Origin};
    #[test]
    fn completion_drains_final_source_updates_and_drop_cancels_and_joins() {
        let target = Target::Collection {
            product_id: 103,
            node: HidNode {
                path: b"simulated".to_vec(),
                vendor_id: 0x1532,
                product_id: 103,
                usage_page: 1,
                usage: 2,
                interface_number: 0,
                release_number: 0,
                serial_number: None,
                manufacturer: None,
                product: None,
            },
        };
        let (sender, updates) = mpsc::sync_channel(4);
        let worker = thread::spawn(move || {
            sender
                .send(Update::Sidepad {
                    origin: Origin::InitialQuery,
                    status: Some(4),
                    side_panel_name: "7ButtonSide".into(),
                    panel_module_id: 4,
                })
                .unwrap();
            Ok(())
        });
        while !worker.is_finished() {
            thread::yield_now();
        }
        let mut controller = Controller {
            active: Some(Operation {
                id: "simulated".into(),
                target: target.clone(),
                canceled: Arc::new(AtomicBool::new(false)),
                updates,
                worker,
            }),
        };
        assert!(controller.poll("other").is_err());
        let reply = controller.poll("simulated").unwrap();
        assert_eq!(reply["terminal"], true);
        assert_eq!(reply["updates"][0]["status"], 4);
        assert!(controller.active.is_none());
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        let finished = Arc::new(AtomicBool::new(false));
        let complete = finished.clone();
        let (sender, updates) = mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            while !signal.load(Ordering::Acquire) {
                thread::yield_now();
            }
            drop(sender);
            complete.store(true, Ordering::Release);
            anyhow::bail!("Mouse artwork owner canceled")
        });
        let controller = Controller {
            active: Some(Operation {
                id: "simulated".into(),
                target,
                canceled,
                updates,
                worker,
            }),
        };
        drop(controller);
        assert!(finished.load(Ordering::Acquire));
    }
}
