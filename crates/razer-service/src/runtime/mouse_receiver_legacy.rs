use super::*;
use razer_device::mouse_receiver_legacy::{self as indicator, Change, Target};
impl PortableRuntime {
    pub(super) fn mouse_receiver_legacy(
        &mut self,
        target: &Target,
        change: &Change,
    ) -> anyhow::Result<Value> {
        match target {
            Target::Collection { node, product_id } => {
                let cap =
                    indicator::change_capability(*product_id, u32::from(node.product_id), change)
                        .context("Current mouse receiver source is absent")?;
                ensure!(
                    indicator::accepts_change(
                        *product_id,
                        u32::from(node.product_id),
                        None,
                        change
                    ) && node.vendor_id == cap.vendor_id
                        && node.interface_number == i32::from(cap.claim_interface),
                    "Single LED physical collection differs"
                );
                let started = Instant::now();
                let validate = || {
                    ensure!(
                        started.elapsed() < Duration::from_secs(20),
                        "Single LED request expired"
                    );
                    Self::revalidate(node)
                };
                validate()?;
                let device = with_backend(|backend| {
                    backend.open_validated(node, std::rc::Rc::new(&validate))
                })?;
                ensure!(
                    device.report_lengths()?.feature.values().max().copied()
                        == Some(cap.report_bytes),
                    "Single LED Feature descriptor differs"
                );
                let report = indicator::execute(
                    device.as_ref(),
                    cap,
                    change,
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
            Target::WindowsContainer(target) => {
                #[cfg(windows)]
                {
                    super::super::native::mouse_receiver_legacy(target, change)
                }
                #[cfg(not(windows))]
                {
                    let _ = target;
                    bail!("Windows receiver target requires Windows")
                }
            }
        }
    }
}
