use super::*;
use razer_device::keyboard_legacy_calibration::{self as calibration, Operation, Target};
impl PortableRuntime {
    pub(super) fn keyboard_legacy_calibration(
        &mut self,
        target: &Target,
        operation: &Operation,
    ) -> anyhow::Result<Value> {
        match target {
            Target::Collection { node, product_id } => {
                let cap = calibration::capability(*product_id)?;
                ensure!(
                    node.vendor_id == cap.vendor_id
                        && u32::from(node.product_id) == *product_id
                        && node.interface_number == 3,
                    "Calibration physical interface differs"
                );
                let started = Instant::now();
                let validate = || {
                    ensure!(
                        started.elapsed() < Duration::from_secs(20),
                        "Calibration operation expired"
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
                    "Calibration Feature descriptor differs"
                );
                let report = calibration::execute(
                    device.as_ref(),
                    &cap,
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
            Target::WindowsContainer(target) => {
                #[cfg(windows)]
                {
                    super::super::native::keyboard_legacy_calibration(target, operation)
                }
                #[cfg(not(windows))]
                {
                    let _ = target;
                    bail!("Windows calibration target requires Windows")
                }
            }
        }
    }
}
