use super::{NativeReturn, ReleaseReason, SourceClock};
use std::time::Duration;

// Expectations follow the current UsbRzDeviceAction send/get/batch/release
// branches and rzDevice25's nullish OUT interval override. Time is explicit;
// no native I/O, wall-clock sleep or production interval defaults are involved.
fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}

#[test]
fn first_out_and_an_idle_collection_do_not_wait() {
    let mut clock = SourceClock::default();
    assert_eq!(clock.out_wait(ms(100), ms(5)), None);
    clock.native_return(ms(100), NativeReturn::Send);
    assert_eq!(clock.out_wait(ms(105), ms(5)), None);
    assert_eq!(clock.out_wait(ms(200), ms(5)), None);
}

#[test]
fn insufficient_out_spacing_waits_one_complete_two_millisecond_step() {
    let mut clock = SourceClock::default();
    clock.native_return(ms(100), NativeReturn::Send);
    assert_eq!(clock.out_wait(ms(104), ms(5)), Some(ms(2)));
    assert_eq!(clock.out_wait(ms(106), ms(5)), None);
}

#[test]
fn polling_rechecks_elapsed_time_without_moving_the_last_command() {
    let mut clock = SourceClock::default();
    clock.native_return(ms(100), NativeReturn::Send);
    let waits = [100, 102, 104, 106].map(|now| clock.out_wait(ms(now), ms(5)));
    assert_eq!(waits, [Some(ms(2)), Some(ms(2)), Some(ms(2)), None]);
}

#[test]
fn separate_callers_observe_the_same_collection_clock() {
    fn caller_a(clock: &mut SourceClock) {
        clock.native_return(ms(100), NativeReturn::Send);
    }
    fn caller_b(clock: &SourceClock) -> Option<Duration> {
        clock.out_wait(ms(104), ms(5))
    }

    let mut clock = SourceClock::default();
    caller_a(&mut clock);
    assert_eq!(caller_b(&clock), Some(ms(2)));
    clock.release(ms(104), ReleaseReason::ExplicitMatched);
    assert_eq!(clock.out_wait(ms(108), ms(5)), Some(ms(2)));
    assert_eq!(clock.out_wait(ms(109), ms(5)), None);
}

#[test]
fn a_get_return_replaces_the_send_timestamp() {
    let mut clock = SourceClock::default();
    clock.native_return(ms(100), NativeReturn::Send);
    clock.native_return(ms(104), NativeReturn::Get);
    assert_eq!(clock.out_wait(ms(105), ms(5)), Some(ms(2)));
    assert_eq!(clock.out_wait(ms(109), ms(5)), None);
}

#[test]
fn an_explicit_zero_override_has_no_out_wait() {
    let mut clock = SourceClock::default();
    clock.native_return(ms(100), NativeReturn::Send);
    assert_eq!(clock.out_wait(ms(100), Duration::ZERO), None);
    assert_eq!(clock.out_wait(ms(104), Duration::ZERO), None);
    assert_eq!(clock.out_wait(ms(104), ms(5)), Some(ms(2)));
}

#[test]
fn a_batch_normal_return_updates_the_clock_without_an_automatic_release_update() {
    let mut clock = SourceClock::default();
    clock.native_return(ms(100), NativeReturn::Batch);
    assert_eq!(clock.out_wait(ms(101), ms(2)), Some(ms(2)));
    clock.release(ms(104), ReleaseReason::BatchFinally);
    assert_eq!(clock.out_wait(ms(105), ms(5)), None);
}

#[test]
fn a_matching_explicit_release_updates_even_without_a_native_return() {
    let mut clock = SourceClock::default();
    clock.release(ms(100), ReleaseReason::ExplicitMatched);
    assert_eq!(clock.out_wait(ms(104), ms(5)), Some(ms(2)));
    assert_eq!(clock.out_wait(ms(105), ms(5)), None);
}

#[test]
fn a_rejected_explicit_release_never_updates_the_clock() {
    let mut clock = SourceClock::default();
    clock.release(ms(100), ReleaseReason::ExplicitRejected);
    assert_eq!(clock.out_wait(ms(101), ms(5)), None);
    clock.native_return(ms(100), NativeReturn::Send);
    clock.release(ms(104), ReleaseReason::ExplicitRejected);
    assert_eq!(clock.out_wait(ms(105), ms(5)), None);
}

#[test]
fn forced_and_automatic_releases_do_not_change_the_last_command() {
    for reason in [
        ReleaseReason::TabDestroyed,
        ReleaseReason::HoldTimeout,
        ReleaseReason::Canceled,
        ReleaseReason::BatchFinally,
    ] {
        let mut clock = SourceClock::default();
        clock.release(ms(100), reason);
        assert_eq!(clock.out_wait(ms(101), ms(5)), None);
        clock.native_return(ms(100), NativeReturn::Send);
        clock.release(ms(104), reason);
        assert_eq!(clock.out_wait(ms(105), ms(5)), None);
    }
}

#[test]
fn a_native_throw_leaves_the_clock_unobserved_or_at_its_previous_return() {
    let mut clock = SourceClock::default();
    clock.native_return(ms(100), NativeReturn::Threw);
    assert_eq!(clock.out_wait(ms(101), ms(5)), None);
    clock.native_return(ms(100), NativeReturn::Send);
    clock.native_return(ms(104), NativeReturn::Threw);
    assert_eq!(clock.out_wait(ms(105), ms(5)), None);
}

#[test]
fn a_native_throw_can_still_be_followed_by_a_matching_explicit_release() {
    let mut clock = SourceClock::default();
    clock.native_return(ms(100), NativeReturn::Send);
    clock.native_return(ms(104), NativeReturn::Threw);
    clock.release(ms(104), ReleaseReason::ExplicitMatched);
    assert_eq!(clock.out_wait(ms(105), ms(5)), Some(ms(2)));
    assert_eq!(clock.out_wait(ms(109), ms(5)), None);
}

#[test]
fn a_real_return_at_the_monotonic_origin_is_observed() {
    let mut clock = SourceClock::default();
    clock.native_return(Duration::ZERO, NativeReturn::Send);
    assert_eq!(clock.out_wait(ms(1), ms(5)), Some(ms(2)));
    assert_eq!(clock.out_wait(ms(5), ms(5)), None);
}
