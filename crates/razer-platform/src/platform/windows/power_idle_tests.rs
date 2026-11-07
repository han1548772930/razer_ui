use super::*;

#[test]
fn power_idle_signed_duration_and_tick_wrap_are_distinct() {
    assert_eq!(idle_event(6000, 1000), Event::Idle { milliseconds: 5000 });
    assert_eq!(
        idle_event(0x1_0000_0010, 0xffff_fff0),
        Event::Idle { milliseconds: 32 }
    );
    assert_eq!(
        idle_event(i32::MAX as u64, 0),
        Event::Idle {
            milliseconds: i32::MAX as u32
        }
    );
    assert!(matches!(idle_event(0x8000_0000, 0), Event::Failed(_)));
    assert!(matches!(idle_event(1000, 1001), Event::Failed(_)));
}

#[test]
fn power_idle_counts_active_child_displays_and_keeps_query_failure() {
    let mut count = MonitorCount::default();
    count.observe(Ok(0));
    count.observe(Ok(2));
    count.observe(Ok(1));
    count.observe(Ok(5));
    assert_eq!(count.finish(), Ok(2));
    let mut count = MonitorCount::default();
    count.observe(Err("GetMonitorInfoW failed".into()));
    count.observe(Ok(1));
    assert_eq!(count.finish(), Err("GetMonitorInfoW failed".into()));
}

#[test]
fn power_idle_lid_failure_or_unobserved_display_never_becomes_off() {
    let (events, received) = async_channel::unbounded();
    let stopped = AtomicBool::new(false);
    let mut context = Context {
        events: &events,
        stopped: &stopped,
        display: None,
        lid_closed: true,
        lid_timer_active: false,
    };
    context.lid_monitors(Ok(0));
    assert!(received.try_recv().is_err());
    context.display(1);
    assert_eq!(received.try_recv().unwrap(), Event::DisplayOn);
    context.lid_monitors(Err("Enumeration failed".into()));
    assert_eq!(
        received.try_recv().unwrap(),
        Event::Failed("Enumeration failed".into())
    );
    assert_eq!(context.display, Some(1));
    context.lid_monitors(Ok(1));
    assert!(received.try_recv().is_err());
    context.lid_monitors(Ok(0));
    assert_eq!(received.try_recv().unwrap(), Event::DisplayOff);
    context.lid_monitors(Ok(0));
    assert!(received.try_recv().is_err());
}

#[test]
fn power_idle_resume_and_unlock_emit_only_the_source_events() {
    let (events, received) = async_channel::unbounded();
    let stopped = AtomicBool::new(false);
    let mut context = Context {
        events: &events,
        stopped: &stopped,
        display: Some(0),
        lid_closed: false,
        lid_timer_active: false,
    };
    context.lifecycle(WM_POWERBROADCAST, 18);
    context.lifecycle(WM_WTSSESSION_CHANGE, 7);
    assert!(received.try_recv().is_err());
    context.lifecycle(WM_POWERBROADCAST, 7);
    assert_eq!(received.try_recv().unwrap(), Event::Resume);
    assert_eq!(context.display, Some(1));
    assert!(received.try_recv().is_err());
    context.lifecycle(WM_WTSSESSION_CHANGE, 8);
    assert_eq!(received.try_recv().unwrap(), Event::ScreenUnlocked);
}

#[test]
fn power_idle_startup_failure_waits_for_release_and_retains_error() {
    let released = Arc::new(AtomicBool::new(false));
    let cleaned = released.clone();
    let (events, received) = async_channel::unbounded();
    let result = Owner::start_worker(events, move |_, _, _| {
        cleaned.store(true, Ordering::Release);
        Err("Registration failed; release failed".into())
    });
    assert!(matches!(result, Err(error) if error == "Registration failed; release failed"));
    assert!(released.load(Ordering::Acquire));
    assert_eq!(
        received.try_recv().unwrap(),
        Event::Failed("Registration failed; release failed".into())
    );
}

fn fake_owner(released: Arc<AtomicBool>) -> Owner {
    let (events, _) = async_channel::unbounded();
    Owner::start_worker(events, move |shared, _, ready| {
        ready.send(Ok(())).unwrap();
        pump_until_stopped(&shared.stopped, |_| {
            std::thread::yield_now();
            Ok(super::super::session_changes::MessageCycle::Continue)
        })?;
        released.store(true, Ordering::Release);
        Ok(())
    })
    .unwrap()
}

#[test]
fn power_idle_failed_wakeup_still_joins_and_reports_the_failure() {
    let released = Arc::new(AtomicBool::new(false));
    let mut owner = fake_owner(released.clone());
    assert_eq!(
        owner.finish(|_| Err("PostMessageW failed".into())),
        Err("PostMessageW failed".into())
    );
    assert!(released.load(Ordering::Acquire));
    assert!(owner.worker.is_none());
}

#[test]
fn power_idle_drop_waits_for_worker_release() {
    let released = Arc::new(AtomicBool::new(false));
    drop(fake_owner(released.clone()));
    assert!(released.load(Ordering::Acquire));
}
