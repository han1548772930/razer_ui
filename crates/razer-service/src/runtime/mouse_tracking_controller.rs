//! Orca Plus source validation and its shared bounded worker owner.
use anyhow::Context as _;
use razer_device::mouse_tracking::{
    self as tracking, Progress, SubmissionReport, Target, TrackingSetting,
};
use std::sync::{Arc, atomic::AtomicBool};
#[cfg(test)]
use std::{sync::atomic::Ordering, thread};
pub(super) type Controller =
    super::feature_submission::Controller<TrackingSetting, SubmissionReport>;
impl super::feature_submission::WorkerReport for SubmissionReport {
    fn cleanup_error(&self) -> Option<String> {
        let mut errors: Vec<_> = self.observation_error.iter().cloned().collect();
        if let Some(Err(error)) = &self.outcome {
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
        setting: TrackingSetting,
    ) -> anyhow::Result<Progress> {
        tracking::prepare(
            tracking::capability(target.product_id()).context("Orca Plus source is absent")?,
            setting,
        )?;
        self.start_with(id, target, setting, execute)
    }
}
fn execute(
    target: Target,
    setting: TrackingSetting,
    canceled: Arc<AtomicBool>,
) -> anyhow::Result<SubmissionReport> {
    match target {
        Target::Collection {
            node,
            product_id,
            peer_product_id,
        } => super::portable::PortableRuntime::default().mouse_tracking(
            &node,
            product_id,
            peer_product_id,
            setting,
            &canceled,
        ),
        Target::WindowsContainer(target) => {
            #[cfg(windows)]
            {
                super::native::mouse_tracking(&target, setting, &canceled)
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
    use std::sync::mpsc;
    fn target() -> Target {
        Target::WindowsContainer(razer_device::device_reads::DeviceReadTarget {
            product_id: 170,
            physical_product_id: 170,
            peer_product_id: None,
            device_container_id: "simulated-container".into(),
            path: "simulated-hid".into(),
        })
    }
    const SETTING: TrackingSetting = TrackingSetting::Symmetric {
        tracking_distance: 2,
    };
    #[test]
    fn cancel_retains_owner_until_worker_releases_its_scope() {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (cleaned_tx, cleaned_rx) = mpsc::channel();
        let mut controller = Controller::default();
        controller
            .start_with("owner".into(), target(), SETTING, move |_, _, signal| {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                assert!(signal.load(Ordering::Acquire));
                cleaned_tx.send(()).unwrap();
                anyhow::bail!("Canceled before write")
            })
            .unwrap();
        entered_rx.recv().unwrap();
        assert!(controller.cancel("other-owner").is_err());
        let canceled = controller.cancel("owner").unwrap();
        assert!(canceled.canceled && !canceled.terminal);
        assert!(
            controller
                .start_with("next".into(), target(), SETTING, |_, _, _| unreachable!())
                .is_err()
        );
        release_tx.send(()).unwrap();
        cleaned_rx.recv().unwrap();
        drop(controller);
    }
    #[test]
    fn drop_cancels_and_joins_before_returning() {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (cleaned_tx, cleaned_rx) = mpsc::channel();
        let mut controller = Controller::default();
        controller
            .start_with("owner".into(), target(), SETTING, move |_, _, signal| {
                entered_tx.send(()).unwrap();
                while !signal.load(Ordering::Acquire) {
                    thread::yield_now();
                }
                cleaned_tx.send(()).unwrap();
                anyhow::bail!("Canceled before write")
            })
            .unwrap();
        entered_rx.recv().unwrap();
        drop(controller);
        cleaned_rx.try_recv().unwrap();
    }

    #[test]
    fn shutdown_retains_adapter_failure_and_reports_worker_panic() {
        let mut controller = Controller::default();
        controller
            .start_with("failed".into(), target(), SETTING, |_, _, _| {
                anyhow::bail!("Device open failed before any setter")
            })
            .unwrap();
        let cleanup = controller.shutdown().unwrap().unwrap();
        assert!(cleanup.terminal && cleanup.canceled);
        assert!(cleanup.report.is_none());
        assert!(cleanup.error.unwrap().contains("Device open failed"));
        controller
            .start_with("panicked".into(), target(), SETTING, |_, _, _| {
                panic!("Simulated adapter panic")
            })
            .unwrap();
        assert!(
            controller
                .shutdown()
                .unwrap_err()
                .to_string()
                .contains("hardware completion is unknown")
        );
    }
}
