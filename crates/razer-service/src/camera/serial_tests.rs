use super::*;
use std::{cell::Cell, rc::Rc};

#[test]
fn source_two_level_initialization_waits_after_every_failure_including_the_last() {
    let attempts = Cell::new(0);
    let mut waits = Vec::new();
    let failure = initialize_with::<()>(
        || {
            attempts.set(attempts.get() + 1);
            anyhow::bail!("filter was not found")
        },
        &|| Ok(()),
        |delay| { waits.push(delay); Ok(()) },
    ).unwrap_err();
    assert_eq!(attempts.get(), 300);
    assert_eq!(failure.attempts(), 300);
    assert_eq!(failure.interruption(), None);
    let expected = [vec![Duration::from_millis(1000); 30], vec![Duration::from_millis(100)]].concat();
    assert_eq!(waits, expected.repeat(10));
    assert_eq!(failure.attempts.last().unwrap().wrapper_attempt, 10);
    assert_eq!(failure.attempts.last().unwrap().native_attempt, 30);
}

#[test]
fn successful_second_wrapper_does_not_wait_after_success() {
    let attempts = Cell::new(0);
    let mut waits = Vec::new();
    let session = initialize_with(
        || {
            attempts.set(attempts.get() + 1);
            if attempts.get() == 32 { Ok(17) } else { anyhow::bail!("bind failed") }
        },
        &|| Ok(()),
        |delay| { waits.push(delay); Ok(()) },
    ).unwrap();
    assert_eq!(session, 17);
    assert_eq!(attempts.get(), 32);
    assert_eq!(waits.len(), 32);
    assert_eq!(waits[30], Duration::from_millis(100));
    assert_eq!(waits[31], Duration::from_millis(1000));
}

#[test]
fn canceled_initialization_releases_a_just_opened_session() {
    struct Session(Rc<Cell<bool>>);
    impl Drop for Session { fn drop(&mut self) { self.0.set(true); } }
    let canceled = Cell::new(false);
    let released = Rc::new(Cell::new(false));
    let result = initialize_with(
        || { canceled.set(true); Ok(Session(released.clone())) },
        &|| { anyhow::ensure!(!canceled.get(), "connection canceled"); Ok(()) },
        |_| panic!("a canceled connection must not begin another wait"),
    );
    assert!(released.get());
    let Err(failure) = result else { panic!("canceled session was published") };
    assert_eq!(failure.interruption(), Some("connection canceled"));
}

#[test]
fn cancellation_during_wait_stops_the_remaining_native_and_wrapper_attempts() {
    let attempts = Cell::new(0);
    let failure = initialize_with::<()>(
        || { attempts.set(attempts.get() + 1); anyhow::bail!("device removed") },
        &|| Ok(()),
        |_| anyhow::bail!("connection canceled while waiting"),
    ).unwrap_err();
    assert_eq!(attempts.get(), 1);
    assert_eq!(failure.attempts[0].error, "device removed");
    assert_eq!(failure.interruption(), Some("connection canceled while waiting"));
}
