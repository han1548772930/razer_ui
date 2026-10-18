use super::*;

fn observed(value: Option<bool>) -> Observation {
    value
        .map(Observation::current)
        .unwrap_or_else(Observation::missing)
}

fn finish_read(core: &mut Core, work: &Work, result: Result<Observation, String>) {
    let Operation::Read {
        flag,
        revision,
        generation,
    } = work.operation
    else {
        panic!("expected queued read")
    };
    assert!(
        core.complete(Completed::Read {
            flag,
            revision,
            generation,
            result,
        })
        .is_none()
    );
}

fn finish_write(core: &mut Core, work: &Work, result: Result<(), String>) -> PersistenceFinished {
    let Operation::Write {
        flag,
        revision,
        value,
    } = work.operation
    else {
        panic!("expected queued write")
    };
    core.complete(Completed::Write {
        flag,
        revision,
        value,
        result,
    })
    .expect("a requested write produces its persistence receipt")
}

#[test]
fn a_late_load_observes_disk_without_overwriting_a_later_click() {
    let mut core = Core::default();
    core.load(Flag::ArmoryIntroduction);
    let read = core.queue.pop_front().unwrap();
    core.set(Flag::ArmoryIntroduction, false);
    finish_read(&mut core, &read, Ok(observed(Some(true))));
    let state = &core.flags[&Flag::ArmoryIntroduction];
    assert!(!state.value);
    assert_eq!(state.persisted_value, Some(true));
    assert_eq!(state.save, SaveStatus::Queued { revision: 1 });
    let write = core.queue.pop_front().unwrap();
    finish_write(&mut core, &write, Ok(()));
    assert_eq!(
        core.flags[&Flag::ArmoryIntroduction].persisted_value,
        Some(false)
    );
}

#[test]
fn source_defaults_do_not_claim_a_successful_observation() {
    let mut core = Core::default();
    assert!(core.flags[&Flag::HueTutorial].value);
    assert!(!core.flags[&Flag::HueTutorial].ready());
    assert_eq!(core.flags[&Flag::HueTutorial].persisted_value, None);
    core.load(Flag::HueTutorial);
    let read = core.queue.pop_front().unwrap();
    finish_read(&mut core, &read, Err("malformed local file".into()));
    let state = &core.flags[&Flag::HueTutorial];
    assert!(state.value);
    assert!(state.ready());
    assert_eq!(
        state.load,
        LoadStatus::Failed("malformed local file".into())
    );
    assert_eq!(state.persisted_value, None);
}

#[test]
fn an_observed_missing_flag_is_distinct_from_a_failed_or_pending_load() {
    let mut core = Core::default();
    core.load(Flag::MagneticCalibrationSkipPrompt);
    core.load(Flag::MagneticCalibrationSkipPrompt);
    assert_eq!(core.queue.len(), 1);
    let read = core.queue.pop_front().unwrap();
    assert_eq!(
        core.flags[&Flag::MagneticCalibrationSkipPrompt].load,
        LoadStatus::Loading
    );
    finish_read(&mut core, &read, Ok(observed(None)));
    let state = &core.flags[&Flag::MagneticCalibrationSkipPrompt];
    assert!(state.ready());
    assert!(!state.value);
    assert_eq!(state.load, LoadStatus::Observed(observed(None)));
}

#[test]
fn writes_keep_acceptance_order_across_shared_pages_and_preferences() {
    let mut core = Core::default();
    core.set(Flag::HueTutorial, false);
    core.set(Flag::ArmoryIntroduction, false);
    core.set(Flag::HueTutorial, true);
    let first = core.queue.pop_front().unwrap();
    assert!(matches!(
        first.operation,
        Operation::Write {
            flag: Flag::HueTutorial,
            value: false,
            revision: 1
        }
    ));
    finish_write(&mut core, &first, Ok(()));
    assert!(core.flags[&Flag::HueTutorial].value);
    assert_eq!(
        core.flags[&Flag::HueTutorial].save,
        SaveStatus::Queued { revision: 2 }
    );
    let second = core.queue.pop_front().unwrap();
    assert!(matches!(
        second.operation,
        Operation::Write {
            flag: Flag::ArmoryIntroduction,
            value: false,
            revision: 1
        }
    ));
    finish_write(&mut core, &second, Ok(()));
    let third = core.queue.pop_front().unwrap();
    finish_write(&mut core, &third, Ok(()));
    assert_eq!(core.flags[&Flag::HueTutorial].persisted_value, Some(true));
    assert_eq!(
        core.flags[&Flag::HueTutorial].save,
        SaveStatus::Saved { revision: 2 }
    );
}

#[test]
fn failed_writes_keep_the_optimistic_ui_and_previous_persisted_value() {
    let mut core = Core::default();
    core.load(Flag::KeyboardCalibrationIntro);
    let read = core.queue.pop_front().unwrap();
    finish_read(&mut core, &read, Ok(observed(Some(true))));
    core.set(Flag::KeyboardCalibrationIntro, false);
    let write = core.queue.pop_front().unwrap();
    finish_write(&mut core, &write, Err("atomic publication failed".into()));
    let state = &core.flags[&Flag::KeyboardCalibrationIntro];
    assert!(!state.value);
    assert_eq!(state.persisted_value, Some(true));
    assert_eq!(
        state.save,
        SaveStatus::Failed {
            revision: 1,
            error: "atomic publication failed".into()
        }
    );
}

#[test]
fn earlier_write_failure_does_not_replace_a_newer_queued_revision() {
    let mut core = Core::default();
    core.set(Flag::HueTutorial, false);
    let first = core.queue.pop_front().unwrap();
    core.set(Flag::HueTutorial, true);
    finish_write(&mut core, &first, Err("first write failed".into()));
    let state = &core.flags[&Flag::HueTutorial];
    assert!(state.value);
    assert_eq!(state.save, SaveStatus::Queued { revision: 2 });
}

#[test]
fn exit_cancels_loads_rejects_new_work_and_drains_every_accepted_write() {
    let mut core = Core::default();
    core.load(Flag::HueTutorial);
    let in_flight = core.queue.pop_front().unwrap();
    core.load(Flag::ArmoryIntroduction);
    core.set(Flag::HueTutorial, false);
    core.set(Flag::ArmoryIntroduction, false);
    core.begin_exit();
    assert_eq!(core.queue.len(), 2);
    core.set(Flag::HueTutorial, true);
    core.load(Flag::MagneticCalibrationSkipPrompt);
    assert_eq!(core.queue.len(), 2);
    finish_read(&mut core, &in_flight, Ok(observed(Some(true))));
    assert_eq!(core.flags[&Flag::HueTutorial].load, LoadStatus::Cancelled);
    assert_eq!(
        core.flags[&Flag::ArmoryIntroduction].load,
        LoadStatus::Cancelled
    );
    while let Some(work) = core.queue.pop_front() {
        finish_write(&mut core, &work, Ok(()));
    }
    for flag in [Flag::HueTutorial, Flag::ArmoryIntroduction] {
        assert_eq!(core.flags[&flag].persisted_value, Some(false));
        assert_eq!(core.flags[&flag].save, SaveStatus::Saved { revision: 1 });
    }
}

#[test]
fn a_later_acquisition_retries_a_failed_read_and_retains_intervening_clicks() {
    let mut core = Core::default();
    core.load(Flag::ArmoryIntroduction);
    let first = core.queue.pop_front().unwrap();
    finish_read(&mut core, &first, Err("temporary read failure".into()));
    core.load(Flag::ArmoryIntroduction);
    let retry = core.queue.pop_front().unwrap();
    core.set(Flag::ArmoryIntroduction, false);
    finish_read(&mut core, &retry, Ok(observed(Some(true))));
    assert!(!core.flags[&Flag::ArmoryIntroduction].value);
    assert!(core.flags[&Flag::ArmoryIntroduction].ready());
}

#[test]
fn cancelling_exit_reopens_admission_and_rejects_the_cancelled_read_generation() {
    let mut core = Core::default();
    core.load(Flag::HueTutorial);
    let original = core.queue.pop_front().unwrap();
    core.set(Flag::HueTutorial, false);
    core.begin_exit();
    core.cancel_exit();
    assert!(!core.closing);
    assert_eq!(core.queue.len(), 2);
    finish_read(&mut core, &original, Ok(observed(Some(true))));
    assert_eq!(core.flags[&Flag::HueTutorial].load, LoadStatus::Loading);
    assert!(!core.flags[&Flag::HueTutorial].value);
    let accepted_write = core.queue.pop_front().unwrap();
    finish_write(&mut core, &accepted_write, Ok(()));
    let reread = core.queue.pop_front().unwrap();
    finish_read(&mut core, &reread, Ok(observed(Some(false))));
    assert_eq!(
        core.flags[&Flag::HueTutorial].load,
        LoadStatus::Observed(observed(Some(false)))
    );
    core.set(Flag::HueTutorial, true);
    assert_eq!(core.queue.len(), 1);
    assert_eq!(
        core.flags[&Flag::HueTutorial].save,
        SaveStatus::Queued { revision: 2 }
    );
}

#[test]
fn storage_panics_are_errors_and_a_subsequent_write_can_still_complete() {
    assert_eq!(
        guarded::<()>(|| panic!("simulated storage panic")),
        Err("UI preference storage panicked".into())
    );
    assert_eq!(guarded(|| Ok(42)), Ok(42));
}

#[test]
fn exit_write_failure_is_reported_until_an_explicit_retry_succeeds() {
    let mut core = Core::default();
    core.set(Flag::HueTutorial, false);
    let write = core.queue.pop_front().unwrap();
    core.begin_exit();
    let failed = finish_write(&mut core, &write, Err("disk is read-only".into()));
    assert_eq!(failed.flag(), Flag::HueTutorial);
    assert_eq!(failed.revision(), 1);
    assert!(!failed.is_success());
    assert_eq!(failed.error(), Some("disk is read-only"));
    assert_eq!(core.save_error(), Some("disk is read-only"));
    assert!(!core.flags[&Flag::HueTutorial].value);
    assert_eq!(core.flags[&Flag::HueTutorial].persisted_value, None);
    assert!(core.queue.is_empty());
    assert_eq!(core.retry_failed(), 0);

    core.cancel_exit();
    assert_eq!(core.retry_failed(), 1);
    assert_eq!(core.save_error(), Some("disk is read-only"));
    assert_eq!(core.pending_writes, 1);
    assert_eq!(core.retry_failed(), 0);
    let retry = core.queue.pop_front().unwrap();
    let saved = finish_write(&mut core, &retry, Ok(()));
    assert!(saved.is_success());
    assert_eq!(saved.revision(), 2);
    assert_eq!(core.save_error(), None);
    assert_eq!(core.flags[&Flag::HueTutorial].persisted_value, Some(false));
    assert_eq!(core.retry_failed(), 0);
}

#[test]
fn another_preference_success_cannot_confirm_a_pending_failed_write_retry() {
    let mut core = Core::default();
    core.set(Flag::HueTutorial, false);
    core.set(Flag::ArmoryIntroduction, false);
    let failed = core.queue.pop_front().unwrap();
    finish_write(&mut core, &failed, Err("Hue flag write failed".into()));
    assert_eq!(core.pending_writes, 1);
    assert_eq!(core.retry_failed(), 1);
    assert_eq!(core.pending_writes, 2);
    assert_eq!(core.save_error(), Some("Hue flag write failed"));

    let other = core.queue.pop_front().unwrap();
    let saved = finish_write(&mut core, &other, Ok(()));
    assert_eq!(saved.flag(), Flag::ArmoryIntroduction);
    assert!(saved.is_success());
    assert_eq!(core.pending_writes, 1);
    assert_eq!(core.save_error(), Some("Hue flag write failed"));
    assert_eq!(core.flags[&Flag::HueTutorial].persisted_value, None);

    let retry = core.queue.pop_front().unwrap();
    let saved = finish_write(&mut core, &retry, Ok(()));
    assert_eq!(saved.flag(), Flag::HueTutorial);
    assert!(saved.is_success());
    assert_eq!(core.pending_writes, 0);
    assert_eq!(core.save_error(), None);
    assert_eq!(core.flags[&Flag::HueTutorial].persisted_value, Some(false));
}
