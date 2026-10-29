//! Cancellable analog task worker. Polling remains responsive while HID is busy.
use anyhow::{Context as _, ensure};
use razer_device::{
    backend::HidNode,
    device_identity::{self, IdentityLookup},
    keyboard_submission::{self, Intent, Target},
};
use razer_hid::with_backend;
use serde_json::{Value, json};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

struct Operation {
    id: String,
    canceled: Arc<AtomicBool>,
    worker: JoinHandle<anyhow::Result<Value>>,
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
                "keyboard_submission",
                active
                    .worker
                    .join()
                    .map_err(|_| anyhow::anyhow!("Keyboard submission cleanup worker panicked"))
                    .and_then(|result| result),
            );
        }
    }

    pub(super) fn start(
        &mut self,
        id: String,
        target: Target,
        guid: String,
        intent: Intent,
    ) -> anyhow::Result<()> {
        ensure!(
            !id.is_empty() && id.len() <= 128 && !id.contains('\0'),
            "Keyboard operation ID is invalid"
        );
        ensure!(
            self.active.is_none(),
            "Keyboard operation result has not been collected"
        );
        keyboard_submission::validate(target.product_id(), &intent)?;
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        let worker = thread::Builder::new()
            .name("keyboard-analog-submission".into())
            .spawn(move || match target {
                Target::Collection { node, product_id } => {
                    collection(&node, product_id, &guid, intent, &signal)
                }
                Target::WindowsContainer(target) => {
                    #[cfg(windows)]
                    {
                        super::native::keyboard_submission(&target, &guid, intent, &signal)
                    }
                    #[cfg(not(windows))]
                    {
                        let _ = (target, guid, intent);
                        anyhow::bail!(
                            "Windows HID collection adapter is unavailable on this platform"
                        )
                    }
                }
            })?;
        self.active = Some(Operation {
            id,
            canceled,
            worker,
        });
        Ok(())
    }
    pub(super) fn poll(&mut self, id: &str) -> anyhow::Result<Value> {
        let active = self
            .active
            .as_ref()
            .context("Keyboard operation is absent")?;
        ensure!(active.id == id, "Keyboard operation ID differs");
        if !active.worker.is_finished() {
            return Ok(
                json!({"operation_id":id,"terminal":false,"state":if active.canceled.load(Ordering::Acquire){"canceling"}else{"running"}}),
            );
        }
        let operation = self.active.take().unwrap();
        let result = operation.worker.join();
        Ok(match result {
            Ok(Ok(result)) => {
                json!({"operation_id":id,"terminal":true,"state":"completed","result":result})
            }
            Ok(Err(error)) => {
                json!({"operation_id":id,"terminal":true,"state":if operation.canceled.load(Ordering::Acquire){"canceled"}else{"failed"},"error":format!("{error:#}")})
            }
            Err(_) => {
                json!({"operation_id":id,"terminal":true,"state":"failed","error":"Keyboard task thread terminated"})
            }
        })
    }
    pub(super) fn cancel(&mut self, id: &str) -> anyhow::Result<()> {
        let operation = self
            .active
            .as_ref()
            .context("Keyboard operation is absent")?;
        ensure!(operation.id == id, "Keyboard operation ID differs");
        operation.canceled.store(true, Ordering::Release);
        Ok(())
    }
}
impl Drop for Controller {
    fn drop(&mut self) {
        if let Some(operation) = self.active.take() {
            operation.canceled.store(true, Ordering::Release);
            let _ = operation.worker.join();
        }
    }
}

fn collection(
    node: &HidNode,
    product_id: u32,
    guid: &str,
    intent: Intent,
    signal: &AtomicBool,
) -> anyhow::Result<Value> {
    let cap = keyboard_submission::validate(product_id, &intent)?.device_read_capability();
    ensure!(
        node.vendor_id == cap.vendor_id
            && cap.direct_pids.contains(&u32::from(node.product_id))
            && node.interface_number
                == i32::from(cap.claim_interface_for(u32::from(node.product_id))?),
        "Keyboard collection differs from the source interface"
    );
    let IdentityLookup::Unique(identity) = device_identity::lookup(u32::from(node.product_id))
    else {
        anyhow::bail!("Keyboard collection has no unique physical identity")
    };
    ensure!(
        identity.product_id == product_id && !identity.is_dongle && !identity.is_ble,
        "Keyboard collection is not a verified direct device"
    );
    let started = Instant::now();
    let guard = || -> anyhow::Result<()> {
        ensure!(!signal.load(Ordering::Acquire), "Keyboard task canceled");
        ensure!(
            started.elapsed() < Duration::from_secs(55),
            "Keyboard task deadline expired"
        );
        let nodes = with_backend(|backend| backend.enumerate())?;
        ensure!(
            nodes.iter().filter(|current| *current == node).count() == 1,
            "Keyboard collection changed during submission"
        );
        Ok(())
    };
    guard()?;
    let device = with_backend(|backend| backend.open_validated(node, std::rc::Rc::new(&guard)))?;
    let mut sequence = 0;
    let receipt = keyboard_submission::submit(
        device.as_ref(),
        product_id,
        guid,
        intent,
        || {
            let transaction = sequence;
            sequence = (sequence + 1) % 31;
            Ok(transaction)
        },
        &guard,
    )?;
    Ok(
        json!({"node":node,"product_id":product_id,"receipt":receipt,"transport_metadata":device.metadata()}),
    )
}
