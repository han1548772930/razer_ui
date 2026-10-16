use super::*;
use razer_device::keyboard_indicator::{self as indicator, Change, Target};
impl PortableRuntime {
    pub(super) fn keyboard_indicator(
        &mut self,
        target: &Target,
        change: &Change,
    ) -> anyhow::Result<Value> {
        match target {
            Target::Collection { node, product_id } => {
                let cap = indicator::capability(*product_id, u32::from(node.product_id))?;
                ensure!(
                    node.vendor_id == cap.vendor_id && node.interface_number == 2,
                    "Keyboard indicator physical interface differs"
                );
                let started = Instant::now();
                let validate = || {
                    ensure!(
                        started.elapsed() < Duration::from_secs(20),
                        "Keyboard indicator task expired"
                    );
                    Self::revalidate(node)
                };
                validate()?;
                let device = with_backend(|backend| backend.open(node))?;
                ensure!(
                    device.report_lengths()?.feature.values().max().copied() == Some(cap.report_bytes),
                    "Keyboard indicator Feature descriptor differs"
                );
                let report = indicator::execute(
                    device.as_ref(),
                    &cap,
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
                    super::super::native::keyboard_indicator(target, change)
                }
                #[cfg(not(windows))]
                {
                    let _ = target;
                    bail!("Windows keyboard indicator target requires Windows")
                }
            }
        }
    }
}
