use super::*;

#[::core::prelude::v1::test]
fn mouse_dynamic_exit_drains_accepted_writes_and_rejects_new_work() {
    let mut state = State::default();
    // The UI event was admitted before its asynchronous local save started.
    assert!(state.admits(false, Admission::New));
    // Exit begins while that save is still in flight.
    assert!(!state.admits(true, Admission::New));
    // Successful persistence and already queued setters keep their write admission.
    assert!(state.admits(true, Admission::AcceptedWrite));
    assert!(state.take_for_shutdown().is_empty());
    assert!(!state.admits(true, Admission::AcceptedWrite));
    assert!(!state.admits(false, Admission::New));
}

#[::core::prelude::v1::test]
fn mouse_dynamic_cancelled_exit_reopens_admission_before_final_cleanup() {
    let state = State::default();
    assert!(!state.admits(true, Admission::New));
    assert!(state.admits(false, Admission::New));
    assert!(state.admits(false, Admission::AcceptedWrite));
}
