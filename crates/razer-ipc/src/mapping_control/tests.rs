use super::*;
const TEST_TIMEOUT: Duration = Duration::from_secs(3);

fn owner(registry: &Registry) -> Owner {
    registry.register(uuid::Uuid::new_v4().to_string()).unwrap()
}
fn command(owner: &Owner, mapping: bool, shortcuts: Option<bool>) -> Control {
    let command = owner.commands.recv_timeout(TEST_TIMEOUT).unwrap();
    assert_eq!(command.owner_guid(), owner.owner_guid);
    assert_eq!(command.target().mapping_enabled(), mapping);
    assert_eq!(command.target().global_shortcuts_enabled(), shortcuts);
    command
}

#[test]
fn every_registered_process_must_acknowledge_pause_and_source_restore() {
    let registry = Registry::default();
    let mut first = owner(&registry);
    let mut second = owner(&registry);
    let acquire = {
        let registry = registry.clone();
        std::thread::spawn(move || registry.suspend(Suppression::SoftwareRecording).unwrap())
    };
    let first_pause = command(&first, false, None);
    let second_pause = command(&second, false, None);
    assert_eq!(first_pause.revision(), second_pause.revision());
    assert!(!acquire.is_finished());
    first_pause.acknowledge(Ok(()));
    second_pause.acknowledge(Ok(()));
    let mut suspension = acquire.join().unwrap();
    let release = std::thread::spawn(move || suspension.release());
    let first_restore = command(&first, true, Some(true));
    let second_restore = command(&second, true, Some(true));
    assert_eq!(first_restore.revision(), second_restore.revision());
    assert!(!release.is_finished());
    first_restore.acknowledge(Ok(()));
    second_restore.acknowledge(Ok(()));
    release.join().unwrap().unwrap();
    first.finish(Ok(())).unwrap();
    second.finish(Ok(())).unwrap();
}

#[test]
fn nested_scopes_keep_each_source_switch_disabled_until_its_final_lease() {
    let registry = Registry::default();
    let mut software = registry.suspend(Suppression::SoftwareRecording).unwrap();
    let mut capture = registry.suspend(Suppression::Mapping).unwrap();
    let mut retained = owner(&registry);
    command(&retained, false, None).acknowledge(Ok(()));
    let release = std::thread::spawn(move || software.release());
    command(&retained, false, Some(true)).acknowledge(Ok(()));
    release.join().unwrap().unwrap();
    let all = {
        let registry = registry.clone();
        std::thread::spawn(move || registry.suspend(Suppression::MappingAndShortcuts).unwrap())
    };
    command(&retained, false, Some(false)).acknowledge(Ok(()));
    let mut all = all.join().unwrap();
    let release = std::thread::spawn(move || capture.release());
    command(&retained, false, Some(false)).acknowledge(Ok(()));
    release.join().unwrap().unwrap();
    let release = std::thread::spawn(move || all.release());
    command(&retained, true, Some(true)).acknowledge(Ok(()));
    release.join().unwrap().unwrap();
    retained.finish(Ok(())).unwrap();
}

#[test]
fn replacement_owner_inherits_live_scope_and_old_identity_cannot_resume_it() {
    let registry = Registry::default();
    let mut suspension = registry.suspend(Suppression::Mapping).unwrap();
    let mut first = owner(&registry);
    command(&first, false, None).acknowledge(Ok(()));
    first.finish(Ok(())).unwrap();
    let mut replacement = owner(&registry);
    command(&replacement, false, None).acknowledge(Ok(()));
    assert_ne!(first.owner_guid, replacement.owner_guid);
    let release = std::thread::spawn(move || suspension.release());
    command(&replacement, true, None).acknowledge(Ok(()));
    release.join().unwrap().unwrap();
    assert!(first.drain().is_empty());
    replacement.finish(Ok(())).unwrap();
}

#[test]
fn rejected_pause_rolls_back_all_partially_changed_processes_before_returning() {
    let registry = Registry::default();
    let mut retained = owner(&registry);
    let acquire = {
        let registry = registry.clone();
        std::thread::spawn(move || registry.suspend(Suppression::Mapping))
    };
    command(&retained, false, None).acknowledge(Err("native rejected pause".into()));
    let rollback = command(&retained, true, None);
    assert!(!acquire.is_finished());
    rollback.acknowledge(Err("native rejected restore".into()));
    let error = acquire.join().unwrap().err().unwrap();
    assert!(error.contains("native rejected pause"));
    assert!(error.contains("rollback failed") && error.contains("native rejected restore"));
    retained.finish(Ok(())).unwrap();
}

#[test]
fn canceling_a_pending_pause_waits_for_its_ordered_rollback() {
    let registry = Registry::default();
    let mut retained = owner(&registry);
    let canceled = Arc::new(AtomicBool::new(false));
    let signal = canceled.clone();
    let acquire = {
        let registry = registry.clone();
        std::thread::spawn(move || registry.suspend_cancelable(Suppression::Mapping, &signal))
    };
    let pause = command(&retained, false, None);
    canceled.store(true, Ordering::Release);
    let rollback = command(&retained, true, None);
    assert!(rollback.revision() > pause.revision());
    assert!(!acquire.is_finished());
    pause.acknowledge(Ok(()));
    rollback.acknowledge(Ok(()));
    assert!(acquire.join().unwrap().err().unwrap().contains("canceled"));
    retained.finish(Ok(())).unwrap();
}

#[test]
fn owner_closing_acknowledges_pending_scope_only_after_confirmed_cleanup() {
    for cleanup in [Ok(()), Err("shutdown rejected".to_owned())] {
        let registry = Registry::default();
        let mut retained = owner(&registry);
        let (mut suspension, pending) = registry.begin(Suppression::Mapping).unwrap();
        assert!(matches!(
            pending[0].1.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        retained.finish(cleanup.clone()).unwrap();
        assert_eq!(wait(pending, None).is_ok(), cleanup.is_ok());
        suspension.release().unwrap();
    }
}

#[test]
fn lease_drop_enqueues_restore_and_never_blocks_its_destructor() {
    let registry = Registry::default();
    let suspension = registry.suspend(Suppression::Mapping).unwrap();
    let mut retained = owner(&registry);
    command(&retained, false, None).acknowledge(Ok(()));
    drop(suspension);
    command(&retained, true, None).acknowledge(Ok(()));
    retained.finish(Ok(())).unwrap();
}

#[test]
fn callback_receipts_require_complete_identity_revision_and_target() {
    let target = Target {
        mapping_enabled: false,
        global_shortcuts_enabled: None,
    };
    let receipt = Receipt::new("owner".into(), 12, target);
    assert_eq!(
        serde_json::to_value(&receipt).unwrap(),
        serde_json::json!({
            "owner_guid":"owner", "revision":12, "target":{
                "mapping_enabled":false, "global_shortcuts_enabled":null,
            }, "observations":[],
        })
    );
    assert!(receipt.confirms("owner", 12, target));
    assert!(!receipt.confirms("other", 12, target));
    assert!(!receipt.confirms("owner", 13, target));
    let observed = Receipt::with_observations(
        "owner".into(),
        12,
        target,
        vec![Observation::new(
            Operation::DisableMapping,
            false,
            "already disabled".into(),
        )],
    );
    let observed: Receipt =
        serde_json::from_value(serde_json::to_value(observed).unwrap()).unwrap();
    assert!(!observed.observations()[0].result());
    assert_eq!(observed.observations()[0].reason(), "already disabled");
    assert!(!receipt.confirms(
        "owner",
        12,
        Target {
            mapping_enabled: true,
            ..target
        }
    ));
    for invalid in [
        serde_json::json!({}),
        serde_json::json!({"owner_guid":"owner","revision":12,"target":{}}),
    ] {
        assert!(serde_json::from_value::<Receipt>(invalid).is_err());
    }
}

#[test]
fn invalid_or_duplicate_owner_and_preexisting_cancellation_do_not_change_registry() {
    let registry = Registry::default();
    assert!(registry.register("not-a-uuid".into()).is_err());
    let mut retained = owner(&registry);
    assert!(registry.register(retained.owner_guid.clone()).is_err());
    assert!(
        registry
            .suspend_cancelable(Suppression::Mapping, &AtomicBool::new(true))
            .is_err()
    );
    assert!(
        registry
            .suspend_while(Suppression::Mapping, || false)
            .is_err()
    );
    assert!(registry.0.lock().unwrap().scopes.is_empty());
    assert!(retained.drain().is_empty());
    retained.finish(Ok(())).unwrap();
}
