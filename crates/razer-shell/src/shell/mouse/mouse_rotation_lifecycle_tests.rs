use super::*;

#[::core::prelude::v1::test]
fn rotation_exit_drains_accepted_saves_before_final_owner_cleanup() {
    let mut state = State::default();
    assert!(state.admits(false, Admission::New));
    assert!(!state.admits(true, Admission::New));
    assert!(state.admits(true, Admission::AcceptedWrite));
    assert!(!state.pending());
    assert!(state.take_for_shutdown().is_empty());
    assert!(!state.admits(true, Admission::AcceptedWrite));
    assert!(!state.admits(false, Admission::New));
    state.resume_after_exit();
    assert!(state.admits(false, Admission::New));
    assert!(!state.admits(true, Admission::New));
}

#[::core::prelude::v1::test]
fn rotation_cancelled_exit_preserves_new_request_admission_before_cleanup() {
    let state = State::default();
    assert!(!state.admits(true, Admission::New));
    assert!(state.admits(false, Admission::New));
}
