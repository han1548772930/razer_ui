use super::*;

fn read(core: &mut Core, work: &Work, values: Result<Preferences, &str>) {
    let Operation::Read {
        revision,
        generation,
    } = &work.operation
    else {
        panic!("expected read");
    };
    assert!(
        core.complete(Completed::Read {
            revision: *revision,
            generation: *generation,
            result: values
                .map(|values| (values, Origin::Current))
                .map_err(str::to_owned),
        })
        .is_none()
    );
}
fn write(
    core: &mut Core,
    work: &Work,
    disk: &mut Preferences,
    error: Option<&str>,
) -> PersistenceFinished {
    let Operation::Write { revision, changes } = &work.operation else {
        panic!("expected write");
    };
    let result = match error {
        Some(error) => Err(error.to_owned()),
        None => {
            for change in changes {
                disk.apply(change);
            }
            Ok(disk.clone())
        }
    };
    core.complete(Completed::Write {
        revision: *revision,
        result,
    })
    .unwrap()
}

#[test]
fn a_late_initial_read_merges_saved_fields_without_overwriting_a_later_click() {
    let mut core = Core::default();
    core.load(false).unwrap();
    let load = core.queue.pop_front().unwrap();
    core.change(vec![Change::Notifications(false)]).unwrap();
    let mut disk = Preferences::default();
    disk.apply(&Change::AutoStart(false));
    disk.apply(&Change::Introduction(false));
    read(&mut core, &load, Ok(disk.clone()));
    assert!(!core.preferences.settings().auto_start_enabled());
    assert!(!core.preferences.settings().notifications_enabled());
    assert!(!core.preferences.dashboard().introduction_enabled());
    assert!(core.committed.settings().notifications_enabled());
    let save = core.queue.pop_front().unwrap();
    assert!(write(&mut core, &save, &mut disk, None).is_success());
    assert_eq!(core.preferences, disk);
    assert!(!core.settings_dirty());
}

#[test]
fn source_defaults_do_not_claim_an_observation_or_a_successful_save() {
    let mut core = Core::default();
    assert_eq!(core.load, LoadStatus::NotRequested);
    core.load(false).unwrap();
    let load = core.queue.pop_front().unwrap();
    read(&mut core, &load, Err("invalid known field"));
    assert_eq!(core.load, LoadStatus::Failed("invalid known field".into()));
    assert!(core.preferences.settings().notifications_enabled());
    assert_eq!(core.committed_revision, 0);
    assert_eq!(core.pending_writes, 0);
    assert!(core.save_error.is_none());
}

#[test]
fn shared_page_writes_keep_fifo_order_and_carry_unacknowledged_edits() {
    let mut core = Core::default();
    core.change(vec![Change::Notifications(false)]).unwrap();
    core.change(vec![Change::Onboard(false)]).unwrap();
    core.change(vec![Change::DynamicLighting(true)]).unwrap();
    let first = core.queue.pop_front().unwrap();
    let second = core.queue.pop_front().unwrap();
    let third = core.queue.pop_front().unwrap();
    let mut disk = Preferences::default();
    let event = write(
        &mut core,
        &first,
        &mut disk,
        Some("first atomic publication failed"),
    );
    assert!(!event.is_success());
    assert_eq!(event.error(), Some("first atomic publication failed"));
    assert!(core.preferences.settings().dynamic_lighting_enabled());
    assert!(write(&mut core, &second, &mut disk, None).is_success());
    assert!(!disk.settings().notifications_enabled());
    assert!(!disk.dashboard().onboard_enabled());
    assert!(!disk.settings().dynamic_lighting_enabled());
    assert!(core.preferences.settings().dynamic_lighting_enabled());
    assert!(core.save_error.is_none());
    assert!(write(&mut core, &third, &mut disk, None).is_success());
    assert_eq!(core.preferences, disk);
    assert_eq!(core.pending_writes, 0);
    assert!(core.uncommitted.is_empty());
}

#[test]
fn an_earlier_acknowledgement_never_rolls_back_a_later_optimistic_value() {
    let mut core = Core::default();
    core.change(vec![Change::DynamicLighting(true)]).unwrap();
    let first = core.queue.pop_front().unwrap();
    core.change(vec![Change::DynamicLighting(false)]).unwrap();
    let mut disk = Preferences::default();
    write(&mut core, &first, &mut disk, None);
    assert!(disk.settings().dynamic_lighting_enabled());
    assert!(!core.preferences.settings().dynamic_lighting_enabled());
    assert!(core.settings_dirty());
    let second = core.queue.pop_front().unwrap();
    write(&mut core, &second, &mut disk, None);
    assert!(!core.settings_dirty());
    assert!(!disk.settings().dynamic_lighting_enabled());
}

#[test]
fn a_failed_save_keeps_the_previous_receipt_and_retries_the_same_local_intent() {
    let mut core = Core::default();
    core.change(vec![
        Change::Notifications(false),
        Change::StartMinimized(false),
    ])
    .unwrap();
    let failed = core.queue.pop_front().unwrap();
    let mut disk = Preferences::default();
    write(&mut core, &failed, &mut disk, Some("disk full"));
    assert!(core.committed.settings().notifications_enabled());
    assert!(!core.preferences.settings().notifications_enabled());
    assert_eq!(core.save_error.as_deref(), Some("disk full"));
    assert!(core.settings_dirty());
    core.retry().unwrap();
    core.retry().unwrap();
    assert_eq!(core.queue.len(), 1);
    let retry = core.queue.pop_front().unwrap();
    write(&mut core, &retry, &mut disk, None);
    assert!(!disk.settings().notifications_enabled());
    assert!(!disk.settings().start_minimized_enabled());
    assert!(core.save_error.is_none());
}

#[test]
fn discard_only_reverts_failed_settings_and_keeps_unacknowledged_dashboard_edits() {
    let mut core = Core::default();
    core.change(vec![Change::Notifications(false), Change::Onboard(false)])
        .unwrap();
    assert!(core.discard_settings().is_err());
    let failed = core.queue.pop_front().unwrap();
    write(
        &mut core,
        &failed,
        &mut Preferences::default(),
        Some("publish denied"),
    );
    core.discard_settings().unwrap();
    assert!(core.preferences.settings().notifications_enabled());
    assert!(!core.preferences.dashboard().onboard_enabled());
    assert!(!core.settings_dirty());
    assert_eq!(core.uncommitted.len(), 1);
    assert_eq!(core.save_error.as_deref(), Some("publish denied"));
    core.retry().unwrap();
    let retry = core.queue.pop_front().unwrap();
    let mut disk = Preferences::default();
    write(&mut core, &retry, &mut disk, None);
    assert!(disk.settings().notifications_enabled());
    assert!(!disk.dashboard().onboard_enabled());
}

#[test]
fn discard_of_the_last_failed_setting_clears_only_its_unsaved_diagnostic() {
    let mut core = Core::default();
    core.change(vec![Change::AutoStart(false)]).unwrap();
    let failed = core.queue.pop_front().unwrap();
    write(
        &mut core,
        &failed,
        &mut Preferences::default(),
        Some("failed"),
    );
    core.discard_settings().unwrap();
    assert!(core.uncommitted.is_empty());
    assert!(core.save_error.is_none());
    assert!(core.preferences.settings().auto_start_enabled());
}

#[test]
fn exit_cancels_unread_observations_rejects_admission_and_drains_accepted_writes() {
    let mut core = Core::default();
    core.load(false).unwrap();
    let in_flight = core.queue.pop_front().unwrap();
    core.change(vec![Change::Notifications(false)]).unwrap();
    core.change(vec![Change::Onboard(false)]).unwrap();
    core.begin_exit();
    assert_eq!(core.load, LoadStatus::Cancelled);
    assert!(core.change(vec![Change::Notifications(true)]).is_err());
    assert!(core.retry().is_err());
    assert!(core.load(true).is_err());
    assert_eq!(core.queue.len(), 2);
    read(&mut core, &in_flight, Ok(Preferences::default()));
    assert_eq!(core.load, LoadStatus::Cancelled);
    let mut disk = Preferences::default();
    while let Some(work) = core.queue.pop_front() {
        write(&mut core, &work, &mut disk, None);
    }
    assert_eq!(core.pending_writes, 0);
    assert!(!disk.settings().notifications_enabled());
    assert!(!disk.dashboard().onboard_enabled());
    assert!(core.closing);
}

#[test]
fn canceled_exit_rejects_the_old_read_generation_before_a_new_observation() {
    let mut core = Core::default();
    core.load(false).unwrap();
    let old = core.queue.pop_front().unwrap();
    core.begin_exit();
    core.cancel_exit();
    assert!(!core.closing);
    assert_eq!(core.queue.len(), 1);
    read(&mut core, &old, Err("old scope failure"));
    assert_eq!(core.load, LoadStatus::Loading);
    let current = core.queue.pop_front().unwrap();
    let mut observed = Preferences::default();
    observed.apply(&Change::Introduction(false));
    read(&mut core, &current, Ok(observed));
    assert!(!core.preferences.dashboard().introduction_enabled());
    assert_eq!(core.load, LoadStatus::Observed(Origin::Current));
}

#[test]
fn a_read_older_than_an_acknowledged_write_cannot_replace_the_committed_baseline() {
    let mut core = Core::default();
    core.load(false).unwrap();
    let old = core.queue.pop_front().unwrap();
    core.change(vec![Change::AutoStart(false)]).unwrap();
    let save = core.queue.pop_front().unwrap();
    let mut disk = Preferences::default();
    write(&mut core, &save, &mut disk, None);
    read(&mut core, &old, Ok(Preferences::default()));
    assert!(!core.committed.settings().auto_start_enabled());
    assert_eq!(core.preferences, disk);
}

#[test]
fn failed_reads_can_be_retried_and_refresh_refuses_uncommitted_values() {
    let mut core = Core::default();
    core.load(false).unwrap();
    let first = core.queue.pop_front().unwrap();
    read(&mut core, &first, Err("malformed disk value"));
    core.load(false).unwrap();
    core.load(false).unwrap();
    assert_eq!(core.queue.len(), 1);
    core.change(vec![Change::Notifications(false)]).unwrap();
    assert!(core.load(true).is_err());
    assert!(!core.preferences.settings().notifications_enabled());
}

#[test]
fn a_storage_panic_becomes_a_failure_receipt() {
    assert_eq!(
        guarded::<()>(|| panic!("simulated storage panic")),
        Err("Chroma settings storage panicked".into())
    );
}
