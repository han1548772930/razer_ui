use super::State;

#[test]
fn keyboard_inventory_runtime_tracks_accepted_work_before_background_polling() {
    let state = State::default();
    assert!(!state.pending());
    let first = state.accept().unwrap();
    let second = state.accept().unwrap();
    assert!(state.pending());
    drop(first);
    assert!(state.pending());
    drop(second);
    assert!(!state.pending());
}

#[test]
fn keyboard_inventory_runtime_shutdown_closes_admission_and_waits_before_resume() {
    let mut state = State::default();
    let accepted = state.accept().unwrap();
    assert!(state.stop().is_empty());
    assert!(state.accept().is_none());
    state.resume();
    assert!(state.accept().is_none());
    assert!(state.pending());
    drop(accepted);
    assert!(!state.pending());
    state.resume();
    assert!(state.accept().is_some());
}

#[test]
fn keyboard_inventory_runtime_canceled_unpolled_work_releases_accepted_owner() {
    let state = State::default();
    let accepted = state.accept().unwrap();
    let future = async move {
        let _accepted = accepted;
        panic!("canceled work was executed");
    };
    assert!(state.pending());
    drop(future);
    assert!(!state.pending());
}
