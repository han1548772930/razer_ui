use super::*;
use crate::runtime::shutdown::CleanupProgress;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

#[test]
fn joined_submission_failure_is_retained_across_portable_shutdown_attempts() {
    let (mut runtime, exited) = failing_submission_runtime();
    for id in [101, 102] {
        let CleanupProgress::Released(response) = runtime.shutdown().progress(id) else {
            panic!("submission worker must be joined before its shutdown returns");
        };
        assert!(exited.load(Ordering::Acquire));
        assert!(response.data.is_none());
        assert!(
            response
                .error
                .unwrap()
                .contains("submission restore confirmation failed")
        );
    }
}

pub(in crate::runtime) fn failing_submission_runtime() -> (PortableRuntime, Arc<AtomicBool>) {
    let mut runtime = PortableRuntime::default();
    let exited = Arc::new(AtomicBool::new(false));
    let signal = exited.clone();
    let target = razer_device::mouse_tracking::Target::WindowsContainer(
        razer_device::device_reads::DeviceReadTarget {
            product_id: 170,
            physical_product_id: 170,
            peer_product_id: None,
            device_container_id: "simulated-container".into(),
            path: "simulated-path".into(),
        },
    );
    runtime
        .mouse_tracking
        .start_with(
            "joined-operation".into(),
            target,
            razer_device::mouse_tracking::TrackingSetting::Symmetric {
                tracking_distance: 2,
            },
            move |_, _, _| {
                signal.store(true, Ordering::Release);
                anyhow::bail!("submission restore confirmation failed");
            },
        )
        .unwrap();
    (runtime, exited)
}

struct RetryEvents {
    attempts: Arc<AtomicUsize>,
    dropped: Arc<AtomicBool>,
}
impl razer_device::receiver_pairing::HardwareEvents for RetryEvents {
    fn begin(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
    fn receive(&mut self, _: Duration) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(None)
    }
    fn finish(&mut self) -> anyhow::Result<()> {
        if self.attempts.fetch_add(1, Ordering::AcqRel) == 0 {
            anyhow::bail!("interrupt unregister failed; owner retained");
        }
        Ok(())
    }
}
impl Drop for RetryEvents {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Release);
    }
}

#[test]
fn retryable_event_shutdown_preserves_failure_after_real_finish() {
    let mut runtime = PortableRuntime::default();
    let attempts = Arc::new(AtomicUsize::new(0));
    let dropped = Arc::new(AtomicBool::new(false));
    runtime.simulated_pending_events(Box::new(RetryEvents {
        attempts: attempts.clone(),
        dropped: dropped.clone(),
    }));
    let CleanupProgress::Pending(response) = runtime.shutdown().progress(103) else {
        panic!("the failed unregister still owns the same event source");
    };
    assert!(
        response
            .error
            .unwrap()
            .contains("interrupt unregister failed")
    );
    assert!(!dropped.load(Ordering::Acquire));
    assert!(runtime.latest_keyboard_events.cleanup_pending());
    let CleanupProgress::Released(response) = runtime.shutdown().progress(104) else {
        panic!("the second finish released the event source");
    };
    assert!(
        response
            .error
            .unwrap()
            .contains("interrupt unregister failed")
    );
    assert!(dropped.load(Ordering::Acquire));
    assert!(!runtime.latest_keyboard_events.cleanup_pending());
    assert_eq!(attempts.load(Ordering::Acquire), 2);
    assert!(!runtime.shutdown().succeeded());
    assert_eq!(attempts.load(Ordering::Acquire), 2);
}
