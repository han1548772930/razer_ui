use super::*;
use std::cell::{Cell, RefCell};

#[test]
fn audio_cleanup_operation_error_and_release_failure_remain_independent() {
    let completion = completed(
        Err("device request failed".into()),
        vec!["unregister failed".into()],
    );
    assert_eq!(
        completion.result.as_ref().map_err(String::as_str),
        Err("device request failed")
    );
    assert_eq!(
        completion.cleanup_failure.as_deref(),
        Some("unregister failed")
    );
    let completion = completed(Ok(()), vec!["shutdown acknowledgement missing".into()]);
    assert!(completion.result.is_ok());
    assert_eq!(
        completion.cleanup_failure.as_deref(),
        Some("shutdown acknowledgement missing")
    );
}

#[test]
fn audio_cleanup_panics_and_errors_do_not_skip_later_release_steps() {
    let events = RefCell::new(Vec::new());
    let mut errors = Vec::new();
    let operation = guarded::<()>(|| panic!("operation panic"));
    step(&mut errors, "capture", || {
        events.borrow_mut().push("capture");
        anyhow::bail!("capture stop failed")
    });
    step(&mut errors, "shortcuts", || {
        events.borrow_mut().push("shortcuts");
        panic!("unregister panic")
    });
    step(&mut errors, "notifications", || {
        events.borrow_mut().push("notifications");
        Ok(())
    });
    step(&mut errors, "native", || {
        events.borrow_mut().push("native");
        Ok(())
    });
    step(&mut errors, "shutdown", || {
        events.borrow_mut().push("shutdown");
        anyhow::bail!("no stopped acknowledgement")
    });
    let completion = completed(operation, errors);
    assert!(completion.result.unwrap_err().contains("panicked"));
    assert_eq!(
        *events.borrow(),
        [
            "capture",
            "shortcuts",
            "notifications",
            "native",
            "shutdown"
        ]
    );
    let failure = completion.cleanup_failure.unwrap();
    assert!(failure.contains("capture stop failed"));
    assert!(failure.contains("shortcuts: Audio_StreamMixer owner panicked"));
    assert!(failure.contains("no stopped acknowledgement"));
}

#[test]
fn audio_cleanup_termination_waits_for_real_release_and_retains_the_first_failure() {
    let attempts = Cell::new(0);
    let waits = Cell::new(0);
    let mut errors = Vec::new();
    terminate(
        &mut errors,
        || {
            attempts.set(attempts.get() + 1);
            if attempts.get() < 3 {
                anyhow::bail!("pipe reader is still owned");
            }
            Ok(())
        },
        || waits.set(waits.get() + 1),
    );
    assert_eq!(attempts.get(), 3);
    assert_eq!(waits.get(), 2);
    assert_eq!(
        errors,
        ["Audio service termination: pipe reader is still owned"]
    );
    assert!(completed(Ok(()), errors).cleanup_failure.is_some());
}
