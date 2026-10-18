//! Retained hardware-module reader. IPC polling never waits on HID retries.
use anyhow::{Context as _, ensure};
use razer_device::keyboard_wristrest::{self as wristrest, Target, Update};
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
                "keyboard_wristrest",
                active
                    .worker
                    .join()
                    .map_err(|_| anyhow::anyhow!("Wristrest cleanup worker panicked"))
                    .and_then(stopped),
            );
        }
    }

    pub(super) fn start(&mut self, id: String, target: Target) -> anyhow::Result<()> {
        ensure!(
            !id.is_empty() && id.len() <= 128 && !id.contains('\0'),
            "Wristrest owner ID is invalid"
        );
        ensure!(self.active.is_none(), "Wristrest owner is already active");
        ensure!(
            wristrest::capability(target.product_id()).is_some(),
            "No current hardware-module capability for this product"
        );
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        let selected = target.clone();
        let (publish, updates) = mpsc::sync_channel(256);
        let worker = thread::Builder::new()
            .name("keyboard-wristrest".into())
            .spawn(move || {
                let mut publish = |update| emit(&publish, &signal, update);
                #[cfg(windows)]
                {
                    super::native::keyboard_wristrest(&selected, &signal, &mut publish)
                }
                #[cfg(not(windows))]
                {
                    collection_initial(&selected, &signal, &mut publish)
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
        let active = self.active.as_ref().context("Wristrest owner is absent")?;
        ensure!(active.id == id, "Wristrest owner ID differs");
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
            Err(_) => Some("Wristrest reader thread terminated".into()),
        };
        Ok(
            json!({"operation_id":id,"target":active.target,"updates":updates,"terminal":true,"canceled":active.canceled.load(Ordering::Acquire),"error":error}),
        )
    }

    pub(super) fn stop(&mut self, id: &str) -> anyhow::Result<Value> {
        let active = self.active.as_ref().context("Wristrest owner is absent")?;
        ensure!(active.id == id, "Wristrest owner ID differs");
        let active = self.active.take().unwrap();
        active.canceled.store(true, Ordering::Release);
        let result = active.worker.join();
        let outcome = result
            .map_err(|_| anyhow::anyhow!("Wristrest reader thread terminated during cleanup"))?;
        let cleanup_error = stopped(outcome).err().map(|error| format!("{error:#}"));
        Ok(
            json!({"operation_id":id,"target":active.target,"stopped":true,"cleanup_error":cleanup_error}),
        )
    }
}

fn stopped(outcome: anyhow::Result<()>) -> anyhow::Result<()> {
    match outcome {
        Err(error) if error.to_string() == "Wristrest owner canceled" => Ok(()),
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
        "Wristrest owner canceled"
    );
    sender
        .try_send(update)
        .context("Wristrest update consumer is unavailable or its queue is full")
}

#[cfg(test)]
mod tests {
    use super::*;
    use razer_device::{backend::HidNode, keyboard_wristrest::Observation};

    fn target() -> Target {
        Target::Collection {
            product_id: wristrest::PRODUCT_ID,
            node: HidNode {
                path: b"scripted-only".to_vec(),
                vendor_id: 0x1532,
                product_id: 653,
                usage_page: 1,
                usage: 6,
                interface_number: 3,
                release_number: 0,
                serial_number: None,
                manufacturer: None,
                product: None,
            },
        }
    }

    #[test]
    fn queued_notifications_retain_origin_and_are_consumed_once() {
        let (tx, updates) = mpsc::sync_channel(4);
        let worker = thread::spawn(move || {
            tx.send(Update::Observed {
                observation: Observation::initial(2),
            })?;
            tx.send(Update::Observed {
                observation: Observation::notification(2),
            })?;
            Ok(())
        });
        while !worker.is_finished() {
            thread::yield_now();
        }
        let mut controller = Controller {
            active: Some(Operation {
                id: "mock".into(),
                target: target(),
                canceled: Arc::new(AtomicBool::new(false)),
                updates,
                worker,
            }),
        };
        assert!(controller.poll("other-owner").is_err());
        assert!(controller.active.is_some());
        let reply = controller.poll("mock").unwrap();
        assert_eq!(reply["terminal"], true);
        let updates: Vec<Update> = serde_json::from_value(reply["updates"].clone()).unwrap();
        assert!(matches!(updates[0], Update::Observed { observation } if !observation.connected()));
        assert!(matches!(updates[1], Update::Observed { observation } if observation.connected()));
        assert!(controller.active.is_none());
        assert!(controller.poll("mock").is_err());
    }

    #[test]
    fn stop_signals_cancellation_and_joins_without_fabricating_an_observation() {
        let (tx, updates) = mpsc::sync_channel(4);
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        let worker = thread::spawn(move || {
            while !signal.load(Ordering::Acquire) {
                thread::yield_now();
            }
            drop(tx);
            anyhow::bail!("Wristrest owner canceled")
        });
        let mut controller = Controller {
            active: Some(Operation {
                id: "mock".into(),
                target: target(),
                canceled,
                updates,
                worker,
            }),
        };
        let reply = controller.stop("mock").unwrap();
        assert_eq!(reply["stopped"], true);
        assert!(reply["cleanup_error"].is_null());
        assert!(controller.active.is_none());
        let (tx, _) = mpsc::sync_channel(1);
        assert!(
            emit(
                &tx,
                &AtomicBool::new(true),
                Update::Observed {
                    observation: Observation::initial(1)
                }
            )
            .is_err()
        );
    }

    #[test]
    fn shutdown_reports_real_cleanup_failure_without_reusing_the_operation() {
        let (_tx, updates) = mpsc::sync_channel(1);
        let mut controller = Controller {
            active: Some(Operation {
                id: "mock".into(),
                target: target(),
                canceled: Arc::new(AtomicBool::new(false)),
                updates,
                worker: thread::spawn(|| anyhow::bail!("interrupt reader finish rejected")),
            }),
        };
        let mut cleanup = crate::runtime::shutdown::Cleanup::default();
        controller.shutdown(&mut cleanup);
        let mut reply = razer_ipc::ResponseEnvelope {
            id: 1,
            data: Some(json!({"stopped":true})),
            error: None,
            fatal: false,
        };
        cleanup.apply(&mut reply);
        assert!(reply.data.is_none());
        assert!(
            reply
                .error
                .unwrap()
                .contains("interrupt reader finish rejected")
        );
        assert!(controller.active.is_none());
        let mut cleanup = crate::runtime::shutdown::Cleanup::default();
        controller.shutdown(&mut cleanup);
        let mut reply = razer_ipc::ResponseEnvelope {
            id: 2,
            data: Some(json!({"stopped":true})),
            error: None,
            fatal: false,
        };
        cleanup.apply(&mut reply);
        assert!(reply.error.is_none());
    }
}

#[cfg(not(windows))]
fn collection_initial(
    target: &Target,
    canceled: &AtomicBool,
    publish: &mut impl FnMut(Update) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    use razer_device::{
        backend::HidBackend,
        device_identity::{self, IdentityLookup},
    };
    use razer_hid::with_backend;
    let Target::Collection { node, product_id } = target else {
        anyhow::bail!("Windows ContainerId adapter is unavailable on this platform");
    };
    let cap = wristrest::capability(*product_id).context("Wristrest capability is absent")?;
    ensure!(
        node.vendor_id == cap.vendor_id
            && cap.direct_pids.contains(&u32::from(node.product_id))
            && node.interface_number == i32::from(cap.claim_interface),
        "Wristrest collection differs from the source interface"
    );
    let IdentityLookup::Unique(identity) = device_identity::lookup(u32::from(node.product_id))
    else {
        anyhow::bail!("Wristrest collection has no unique physical identity");
    };
    ensure!(
        identity.product_id == *product_id && !identity.is_ble && !identity.is_dongle,
        "Wristrest collection is not a direct wired device"
    );
    let guard = || {
        ensure!(
            !canceled.load(Ordering::Acquire),
            "Wristrest owner canceled"
        );
        let nodes = with_backend(|backend| backend.enumerate())?;
        ensure!(
            nodes.iter().filter(|current| *current == node).count() == 1,
            "Wristrest collection changed"
        );
        Ok(())
    };
    guard()?;
    let device = with_backend(|backend| backend.open(node))?;
    ensure!(
        device.report_lengths()?.feature.get(&0) == Some(&91),
        "Wristrest Feature Report size differs"
    );
    let observation = wristrest::query(device.as_ref(), 0, &guard)?;
    guard()?;
    publish(Update::Observed { observation })?;
    anyhow::bail!(
        "The source ContainerId/PID multi-collection notification selector requires a platform adapter; initial Feature observation is available"
    )
}
