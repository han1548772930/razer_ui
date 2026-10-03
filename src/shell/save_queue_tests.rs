use super::{SaveContinuation, save_continuation};

#[test]
fn exit_drains_requested_writes_and_a_follow_up_auxiliary_write() {
    let mut close_requested = true;
    // A requested save completed, but a separately clicked Save is queued.
    assert_eq!(
        save_continuation(&mut close_requested, true, true, true),
        SaveContinuation::Queued
    );
    assert!(close_requested);
    // That Save completes; a shortcut/tutorial edit made during I/O still
    // needs the auxiliary writer. Neither completion may consume the intent.
    assert_eq!(
        save_continuation(&mut close_requested, true, false, true),
        SaveContinuation::Auxiliary
    );
    assert!(close_requested);
    assert_eq!(
        save_continuation(&mut close_requested, true, false, false),
        SaveContinuation::Close
    );
    assert!(!close_requested);
}

#[test]
fn a_failed_write_cancels_close_without_dropping_the_next_scope() {
    let mut close_requested = true;
    assert_eq!(
        save_continuation(&mut close_requested, false, true, true),
        SaveContinuation::Queued
    );
    assert!(!close_requested);
    // A later successful scope can drain/retry pending auxiliary data, but it
    // cannot silently turn the failed close request into an application exit.
    assert_eq!(
        save_continuation(&mut close_requested, true, false, true),
        SaveContinuation::Auxiliary
    );
    assert_eq!(
        save_continuation(&mut close_requested, true, false, false),
        SaveContinuation::Idle
    );
    // No queued click means a failure waits for a new user action, rather than
    // starting an immediate, potentially endless auxiliary retry loop.
    assert_eq!(
        save_continuation(&mut close_requested, false, false, true),
        SaveContinuation::Idle
    );
}

#[test]
fn exit_does_not_require_submitting_editor_drafts() {
    let mut close_requested = true;
    assert_eq!(
        save_continuation(&mut close_requested, true, false, false),
        SaveContinuation::Close
    );
    assert!(!close_requested);
    assert_eq!(
        save_continuation(&mut close_requested, true, false, false),
        SaveContinuation::Idle
    );
}
