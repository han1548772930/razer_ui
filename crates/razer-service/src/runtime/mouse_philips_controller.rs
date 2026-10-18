//! Philips source dispatch uses the shared cancellation/join owner.
use anyhow::ensure;
use razer_device::mouse_philips::{self as philips, Progress, Setting, SubmissionReport, Target};
use std::sync::{Arc, atomic::AtomicBool};
pub(super) type Controller = super::feature_submission::Controller<Setting, SubmissionReport>;
impl super::feature_submission::WorkerReport for SubmissionReport {
    fn cleanup_error(&self) -> Option<String> {
        let mut errors: Vec<_> = self.initialization_error.iter().cloned().collect();
        if let Err(error) = &self.outcome {
            errors.push(error.to_string());
        }
        (!errors.is_empty()).then(|| errors.join("; "))
    }
}
impl Controller {
    pub(super) fn start(
        &mut self,
        id: String,
        target: Target,
        setting: Setting,
    ) -> anyhow::Result<Progress> {
        ensure!(
            target.product_id() == philips::PRODUCT_ID,
            "Philips calibration product differs"
        );
        self.start_with(id, target, setting, execute)
    }
}
fn execute(
    target: Target,
    setting: Setting,
    canceled: Arc<AtomicBool>,
) -> anyhow::Result<SubmissionReport> {
    match target {
        Target::Collection {
            node,
            product_id,
            peer_product_id,
        } => {
            ensure!(
                peer_product_id.is_none(),
                "Philips calibration does not use a receiver peer"
            );
            super::portable::PortableRuntime::default()
                .mouse_philips(&node, product_id, setting, &canceled)
        }
        Target::WindowsContainer(target) => {
            #[cfg(windows)]
            {
                super::native::mouse_philips(&target, setting, &canceled)
            }
            #[cfg(not(windows))]
            {
                let _ = target;
                anyhow::bail!("Windows container transport is unavailable on this platform")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{atomic::Ordering, mpsc};
    fn target(product_id: u32) -> Target {
        Target::WindowsContainer(razer_device::device_reads::DeviceReadTarget {
            product_id,
            physical_product_id: product_id,
            peer_product_id: None,
            device_container_id: "simulated-container".into(),
            path: "simulated-hid".into(),
        })
    }
    #[test]
    fn philips_start_rejects_orca_and_cancel_keeps_owner_until_joined() {
        let mut controller = Controller::default();
        assert!(
            controller
                .start("wrong-feature".into(), target(170), Setting::Disabled)
                .is_err()
        );
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        controller
            .start_with(
                "philips".into(),
                target(70),
                Setting::Disabled,
                move |_, _, signal| {
                    entered_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    assert!(signal.load(Ordering::Acquire));
                    anyhow::bail!("Adapter released the owned handle")
                },
            )
            .unwrap();
        entered_rx.recv().unwrap();
        assert!(controller.cancel("orca").is_err());
        assert!(!controller.cancel("philips").unwrap().terminal);
        assert!(
            controller
                .start_with(
                    "replacement".into(),
                    target(70),
                    Setting::Disabled,
                    |_, _, _| unreachable!()
                )
                .is_err()
        );
        release_tx.send(()).unwrap();
        let progress = controller.shutdown().unwrap().unwrap();
        assert!(progress.terminal && progress.canceled);
        assert!(progress.error.unwrap().contains("owned handle"));
    }
}
