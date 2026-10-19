use super::State;

#[test]
fn legacy_dpi_admission_is_pending_before_background_worker_poll() {
    let mut state = State::default();
    let (first, first_guard) = state.accept().unwrap();
    let (second, second_guard) = state.accept().unwrap();
    assert_ne!(first, second);
    assert!(state.pending());
    drop(first_guard);
    assert!(state.pending());
    drop(second_guard);
    assert!(!state.pending());
}

#[test]
fn legacy_dpi_canceled_scheduling_releases_admission_without_starting_a_service() {
    let mut state = State::default();
    let (_, guard) = state.accept().unwrap();
    let future = async move {
        let _guard = guard;
        panic!("canceled setter must not execute");
    };
    assert!(state.pending());
    drop(future);
    assert!(!state.pending());
}

#[test]
fn legacy_dpi_shutdown_rejects_new_work_and_waits_for_accepted_worker_release() {
    let mut state = State::default();
    let (_, guard) = state.accept().unwrap();
    assert!(state.take_for_shutdown().is_empty());
    assert!(state.accept().is_none());
    assert!(state.pending());
    drop(guard);
    assert!(!state.pending());
}
