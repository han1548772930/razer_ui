use super::*;

#[test]
fn cancellation_finishes_production_without_inventing_a_failure_or_hardware_ack() {
    let signal = AtomicBool::new(true);
    let state = Mutex::new(SoftwareStatus::starting(4));
    complete_status(&state, check_cancellation(&signal));
    let state = state.into_inner().unwrap();
    assert_eq!(state.phase, SoftwarePhase::Stopped);
    assert_eq!(state.generation, 4);
    assert_eq!(state.acknowledged_writes, 0);
    assert!(state.errors.is_empty());
    assert!(!state.restore_acknowledged);
}

#[test]
fn stop_does_not_hide_an_independent_identity_or_native_failure() {
    let state = Mutex::new(SoftwareStatus::starting(5));
    complete_status(&state, Err(anyhow::anyhow!("Retained device identity changed")));
    complete_status(&state, Err(LightingCanceled.into()));
    let state = state.into_inner().unwrap();
    assert_eq!(state.phase, SoftwarePhase::Failed);
    assert_eq!(state.errors, ["Retained device identity changed"]);
    assert_eq!(state.acknowledged_writes, 0);
}

fn stopping_job(restore_acknowledged: bool) -> Job {
    let canceled = Arc::new(AtomicBool::new(false));
    let status = Arc::new(Mutex::new(SoftwareStatus::starting(6)));
    let worker_signal = canceled.clone();
    let worker_status = status.clone();
    let (sender, _receiver) = mpsc::sync_channel(1);
    let worker = thread::spawn(move || {
        while !worker_signal.load(Ordering::Acquire) {
            thread::yield_now();
        }
        let mut status = worker_status.lock().unwrap();
        status.phase = SoftwarePhase::Stopped;
        status.restore_acknowledged = restore_acknowledged;
        if restore_acknowledged {
            status.acknowledged_writes = 3;
        } else {
            status.cleanup_errors.push("Simulated restore rejected".into());
        }
    });
    Job {
        node: HidNode {
            path: b"simulated-lighting-stop".to_vec(),
            vendor_id: 5426,
            product_id: 4115,
            usage_page: 1,
            usage: 0,
            interface_number: 1,
            release_number: 0,
            serial_number: None,
            manufacturer: None,
            product: None,
        },
        generation: 6,
        sender,
        canceled,
        status,
        thread: Some(worker),
    }
}

#[test]
fn stop_acknowledges_only_the_restore_completed_by_this_stop() {
    let mut job = stopping_job(true);
    let first = job.stop_result().unwrap();
    assert_eq!(first.acknowledged_writes, 1);
    assert!(first.software.unwrap().restore_acknowledged);
    let repeated = job.stop_result().unwrap();
    assert_eq!(repeated.acknowledged_writes, 0);
    assert_eq!(repeated.software.unwrap().acknowledged_writes, 3);
}

#[test]
fn failed_restore_keeps_cleanup_error_without_an_ack() {
    let result = stopping_job(false).stop_result().unwrap();
    assert_eq!(result.acknowledged_writes, 0);
    let software = result.software.unwrap();
    assert!(!software.restore_acknowledged);
    assert_eq!(software.cleanup_errors, ["Simulated restore rejected"]);
}
