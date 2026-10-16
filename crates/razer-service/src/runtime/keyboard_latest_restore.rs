use super::*;
use razer_device::keyboard_latest_protocol::{self as protocol, Operation, Target};
use razer_device::{backend::InterruptTransport, receiver_pairing::HardwareEvents};
#[derive(Default)]
pub(super) struct EventsState {
    target: Option<Value>,
    events: Option<Box<dyn HardwareEvents>>,
}
impl Drop for EventsState {
    fn drop(&mut self) {
        if let Some(events) = self.events.as_mut() {
            if let Err(error) = events.finish() {
                eprintln!("keyboard756 event cleanup: {error:#}");
            }
        }
    }
}
struct CollectionEvents {
    node: HidNode,
    reader: Option<Box<dyn InterruptTransport>>,
}
impl HardwareEvents for CollectionEvents {
    fn begin(&mut self) -> anyhow::Result<()> {
        PortableRuntime::revalidate(&self.node)?;
        ensure!(
            self.reader.is_none(),
            "756 interrupt collection already opened"
        );
        self.reader = Some(with_backend(|backend| backend.open_interrupt(&self.node))?);
        Ok(())
    }
    fn receive(&mut self, timeout: Duration) -> anyhow::Result<Option<Vec<u8>>> {
        PortableRuntime::revalidate(&self.node)?;
        let reader = self
            .reader
            .as_ref()
            .context("756 interrupt collection is not open")?;
        let mut data = [0u8; 100];
        let count = reader.read(&mut data, timeout.as_millis().min(u32::MAX as u128) as u32)?;
        ensure!(
            count <= data.len(),
            "756 input report exceeds native capacity"
        );
        Ok((count > 0).then(|| data[..count].to_vec()))
    }
    fn finish(&mut self) -> anyhow::Result<()> {
        self.reader = None;
        Ok(())
    }
}
impl PortableRuntime {
    pub(super) fn keyboard_latest_restore(
        &mut self,
        target: &Target,
        operation: &Operation,
    ) -> anyhow::Result<Value> {
        if let Operation::Events { enabled } = operation {
            let identity = serde_json::to_value(target)?;
            if *enabled == Some(true) {
                ensure!(
                    self.latest_keyboard_events.events.is_none(),
                    "756 input subscription already belongs to an owner"
                );
                let mut events: Box<dyn HardwareEvents> = match target {
                    Target::Collection(node) => Box::new(CollectionEvents {
                        node: node.clone(),
                        reader: None,
                    }),
                    Target::WindowsContainer(_) | Target::WindowsBle(_) => {
                        #[cfg(windows)]
                        {
                            super::super::native::keyboard_latest_event_source(target)?
                        }
                        #[cfg(not(windows))]
                        {
                            anyhow::bail!(
                                "Windows756 input events require their installed HID adapter"
                            )
                        }
                    }
                };
                if let Err(error) = events.begin() {
                    let _ = events.finish();
                    return Err(error);
                }
                self.latest_keyboard_events.events = Some(events);
                self.latest_keyboard_events.target = Some(identity.clone());
            } else {
                ensure!(
                    self.latest_keyboard_events.target.as_ref() == Some(&identity),
                    "756 input subscription actual owner changed"
                );
            }
            let mut packets = Vec::new();
            let events = self
                .latest_keyboard_events
                .events
                .as_mut()
                .context("756 input subscription is absent")?;
            if *enabled == Some(false) {
                events.finish()?;
                self.latest_keyboard_events.events = None;
                self.latest_keyboard_events.target = None;
            } else {
                for _ in 0..64 {
                    match events.receive(Duration::ZERO)? {
                        Some(data) => packets.push(data),
                        None => break,
                    }
                }
            }
            return Ok(json!({"target":target,"report":protocol::Reply::Events{packets}}));
        }
        match target {
            Target::Collection(node) => {
                let cap = protocol::capability(u32::from(node.product_id))?;
                ensure!(
                    node.vendor_id == cap.vendor_id
                        && node.interface_number
                            == i32::from(cap.claim_interface_for(u32::from(node.product_id))?),
                    "756 restore collection interface differs"
                );
                let started = Instant::now();
                let validate = || {
                    ensure!(
                        started.elapsed() < Duration::from_secs(120),
                        "756 restore operation expired"
                    );
                    Self::revalidate(node)
                };
                validate()?;
                let device = with_backend(|backend| backend.open(node))?;
                ensure!(
                    device.report_lengths()?.feature.values().max().copied() == Some(cap.report_bytes),
                    "756 restore Feature descriptor differs"
                );
                let report = protocol::execute_hid(
                    device.as_ref(),
                    cap,
                    operation,
                    || {
                        self.transaction(
                            node,
                            &cap.source_class,
                            cap.transaction_prefix,
                            cap.transaction_modulus,
                        )
                    },
                    &validate,
                )?;
                Ok(json!({"target":target,"report":report}))
            }
            Target::WindowsContainer(_) | Target::WindowsBle(_) => {
                #[cfg(windows)]
                {
                    super::super::native::keyboard_latest_restore(target, operation)
                }
                #[cfg(not(windows))]
                {
                    anyhow::bail!("Windows keyboard restore requires its platform adapter")
                }
            }
        }
    }
}
