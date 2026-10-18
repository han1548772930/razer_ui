use super::{Command, Owner, PendingRecovery, Request};
use razer_device::keyboard_latest_restore::{Coordinator, ProfileCollection, Work};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

fn owner() -> (Owner, mpsc::Receiver<Command>) {
    let (commands, work) = mpsc::channel();
    let (_sender, replies) = mpsc::channel();
    (
        Owner {
            identity: "device-owner".into(),
            route: "container:756".into(),
            valid: Arc::new(AtomicBool::new(true)),
            automatic: Arc::new(AtomicBool::new(true)),
            recoveries: Arc::new(AtomicUsize::new(0)),
            stopping: false,
            commands,
            replies,
            coordinator: Coordinator::default(),
            ready: true,
            revision: 0,
            revision_signal: Arc::new(AtomicU64::new(0)),
            document: None,
            source: None,
            collection: None,
            running_games: None,
            brightness_repeats: BTreeMap::new(),
            ble: false,
            macro_exclusive: Arc::new(AtomicBool::new(false)),
            pending_intents: BTreeSet::new(),
            cancelled_intents: Vec::new(),
        },
        work,
    )
}

fn restore_work(work: &mpsc::Receiver<Command>) -> Work<Request> {
    match work.try_recv().unwrap() {
        Command::Restore(work) => work,
        Command::CollectionOnly { .. } => panic!("expected an admitted physical restore"),
    }
}

#[test]
fn prepared_exit_drains_debounced_restore_and_saves_admitted_during_preparation_in_order() {
    let (mut owner, work) = owner();
    owner.enqueue(json!({"activeProfile":"a","version":1}), Some(0), false);
    owner.enqueue(json!({"activeProfile":"a","version":2}), Some(0), false);
    owner.prepare_exit();
    assert!(!owner.automatic.load(Ordering::Acquire));
    owner.stop_if_drained();
    assert!(!owner.stopping);
    let after_debounce = Instant::now() + Duration::from_secs(1);
    owner.dispatch(after_debounce).unwrap();
    let full = restore_work(&work);
    assert!(full.value.full);
    assert_eq!(full.value.document["version"], 1);
    owner.enqueue(json!({"activeProfile":"a","version":3}), Some(0), false);
    owner.stop_if_drained();
    assert!(owner.valid.load(Ordering::Acquire));
    assert!(full.generation.is_current());
    for version in [2, 3] {
        owner.coordinator.finish();
        owner.stop_if_drained();
        assert!(!owner.stopping);
        owner.dispatch(after_debounce).unwrap();
        let saved = restore_work(&work);
        assert!(!saved.value.full);
        assert_eq!(saved.value.document["version"], version);
        assert!(saved.generation.is_current());
        assert!(owner.valid.load(Ordering::Acquire));
    }
    owner.coordinator.finish();
    owner.stop_if_drained();
    assert!(owner.stopping);
    assert!(!owner.valid.load(Ordering::Acquire));
    owner.enqueue(json!({"activeProfile":"b","version":4}), None, false);
    assert_eq!(owner.document.as_ref().unwrap()["version"], 3);
    assert!(
        owner
            .switch(
                razer_device::keyboard_latest_restore::BleSwitch::Slot(2),
                false,
                None
            )
            .is_err()
    );
    owner.dispatch(after_debounce).unwrap();
    assert!(work.try_recv().is_err());
}

#[test]
fn source_delayed_recovery_keeps_idle_owner_alive_until_the_recovery_guard_is_released() {
    let (mut owner, work) = owner();
    owner.enqueue(json!({"activeProfile":"a"}), None, false);
    owner
        .dispatch(Instant::now() + Duration::from_secs(1))
        .unwrap();
    let request = restore_work(&work);
    let (started, admitted) = mpsc::channel();
    let (finish, proceed) = mpsc::channel();
    let recoveries = owner.recoveries.clone();
    let worker = thread::spawn(move || {
        let _recovery = PendingRecovery::start(recoveries);
        started.send(()).unwrap();
        proceed.recv().unwrap();
        assert!(request.generation.is_current());
    });
    admitted.recv_timeout(Duration::from_secs(3)).unwrap();
    owner.prepare_exit();
    owner.coordinator.finish();
    owner.stop_if_drained();
    assert!(!owner.stopping);
    assert!(owner.valid.load(Ordering::Acquire));
    finish.send(()).unwrap();
    worker.join().unwrap();
    owner.stop_if_drained();
    assert!(owner.stopping);
    assert!(!owner.valid.load(Ordering::Acquire));
}

#[test]
fn replacing_delayed_recovery_cannot_release_its_successor() {
    let (mut owner, _work) = owner();
    let previous = PendingRecovery::start(owner.recoveries.clone());
    let successor = PendingRecovery::start(owner.recoveries.clone());
    owner.prepare_exit();
    drop(previous);
    owner.stop_if_drained();
    assert!(!owner.stopping);
    drop(successor);
    owner.stop_if_drained();
    assert!(owner.stopping);
}

#[test]
fn dropping_a_prepared_owner_invalidates_running_generation_and_closes_admission() {
    let (mut owner, work) = owner();
    owner.enqueue(json!({"activeProfile":"a"}), None, false);
    owner
        .dispatch(Instant::now() + Duration::from_secs(1))
        .unwrap();
    let current = restore_work(&work);
    let valid = owner.valid.clone();
    owner.prepare_exit();
    assert!(current.generation.is_current());
    drop(owner);
    assert!(!valid.load(Ordering::Acquire));
    assert!(!current.generation.is_current());
    assert!(matches!(
        work.try_recv(),
        Err(mpsc::TryRecvError::Disconnected)
    ));
}

#[test]
fn disconnected_worker_invalidates_the_queue_and_closes_admission() {
    let (mut owner, work) = owner();
    owner.enqueue(json!({"activeProfile":"a"}), None, false);
    drop(work);
    assert!(
        owner
            .dispatch(Instant::now() + Duration::from_secs(1))
            .is_err()
    );
    assert!(owner.stopping);
    assert!(!owner.valid.load(Ordering::Acquire));
    assert!(owner.coordinator.is_idle());
}

#[test]
fn collection_only_import_keeps_running_restore_and_pending_fifo_payloads() {
    let (mut owner, work) = owner();
    let first = json!({"activeProfile":"a","version":1,"profiles":[{"guid":"a","name":"A"}],"brightness":80});
    let second = json!({"activeProfile":"a","version":2,"profiles":[{"guid":"a","name":"Edited A"}],"brightness":50});
    owner.enqueue(first.clone(), Some(1), false);
    owner.enqueue(second.clone(), Some(1), false);
    owner.pending_intents.insert(42);
    let at = Instant::now() + Duration::from_secs(1);
    owner.dispatch(at).unwrap();
    let running = restore_work(&work);
    let source = json!({"profiles":["a","imported"]});
    let imported = ProfileCollection::from_document(&json!({
        "activeProfile":"imported","version":999,
        "profiles":[{"guid":"a","name":"Captured A"},{"guid":"imported","name":"Imported"}],
        "brightness":1
    }))
    .unwrap();
    assert!(
        owner
            .acknowledge_collection(
                "device-owner",
                "container:756",
                Ok(imported),
                source.clone()
            )
            .unwrap()
    );
    assert_eq!(owner.revision, 2);
    assert_eq!(owner.revision_signal.load(Ordering::Acquire), 2);
    assert!(running.generation.is_current());
    assert!(owner.pending_intents.contains(&42));
    assert!(owner.cancelled_intents.is_empty());
    assert_eq!(owner.running_games, Some(1));
    let retained = owner.document.as_ref().unwrap();
    assert_eq!(retained["activeProfile"], "a");
    assert_eq!(retained["version"], 2);
    assert_eq!(retained["brightness"], 50);
    assert_eq!(retained["profiles"][0], second["profiles"][0]);
    assert_eq!(retained["profiles"][1]["guid"], "imported");
    assert_eq!(owner.source, Some(source));
    let command = work.try_recv().unwrap();
    assert!(matches!(command, Command::CollectionOnly { .. }));
    assert_eq!(owner.recoveries.load(Ordering::Acquire), 1);
    owner.coordinator.finish();
    owner.dispatch(at).unwrap();
    let pending = restore_work(&work);
    assert_eq!(running.value.document, first);
    assert_eq!(pending.value.document, second);
    assert!(pending.generation.is_current());
    drop(command);
    owner.coordinator.finish();
    owner.dispatch(at).unwrap();
    assert!(work.try_recv().is_err());
    assert!(owner.coordinator.is_idle());
}

#[test]
fn collection_only_import_cannot_replace_late_candidate_active_or_observed_settings() {
    let (mut owner, work) = owner();
    let document = json!({"activeProfile":"a","version":3,"profiles":[{"guid":"a","name":"A"}],"brightness":80});
    owner.enqueue(document.clone(), None, false);
    let collection = ProfileCollection::from_document(&json!({
        "activeProfile":"imported","version":999,
        "profiles":[{"guid":"a","name":"Captured A"},{"guid":"imported","name":"Imported"}]
    }))
    .unwrap();
    owner
        .acknowledge_collection(
            "device-owner",
            "container:756",
            Ok(collection),
            json!({"captured":true}),
        )
        .unwrap();
    drop(work.try_recv().unwrap());
    let observed = json!({"activeProfile":"a","version":4,"profiles":[{"guid":"a","name":"Observed A","appEngine":{"ack":true}}],"brightness":45});
    let candidate = owner
        .candidate_document(owner.revision, observed.clone())
        .unwrap();
    assert_eq!(candidate["activeProfile"], observed["activeProfile"]);
    assert_eq!(candidate["version"], observed["version"]);
    assert_eq!(candidate["brightness"], observed["brightness"]);
    assert_eq!(candidate["profiles"][0], observed["profiles"][0]);
    assert_eq!(candidate["profiles"][1]["guid"], "imported");
    owner
        .switch(
            razer_device::keyboard_latest_restore::BleSwitch::Guid("imported".into()),
            false,
            None,
        )
        .unwrap();
    owner
        .dispatch(Instant::now() + Duration::from_secs(1))
        .unwrap();
    let switched = restore_work(&work);
    assert!(switched.value.full);
    assert_eq!(switched.value.document["profiles"][1]["guid"], "imported");
    assert!(owner.collection.is_none());
    assert_eq!(
        owner
            .candidate_document(owner.revision, document.clone())
            .unwrap(),
        document
    );
}

#[test]
fn collection_only_import_is_admitted_local_work_during_exit() {
    let (mut owner, work) = owner();
    let document = json!({"activeProfile":"a","version":1,"profiles":[{"guid":"a"}]});
    owner.enqueue(document.clone(), None, false);
    owner
        .dispatch(Instant::now() + Duration::from_secs(1))
        .unwrap();
    let running = restore_work(&work);
    owner.coordinator.finish();
    let collection =
        ProfileCollection::from_document(&json!({"profiles":[{"guid":"a"},{"guid":"imported"}]}))
            .unwrap();
    owner
        .acknowledge_collection(
            "device-owner",
            "container:756",
            Ok(collection),
            json!({"import":true}),
        )
        .unwrap();
    owner.prepare_exit();
    owner.stop_if_drained();
    assert!(!owner.stopping);
    let command = work.try_recv().unwrap();
    assert!(matches!(command, Command::CollectionOnly { .. }));
    assert!(running.generation.is_current());
    assert_eq!(owner.revision, running.id);
    owner.dispatch(Instant::now()).unwrap();
    assert!(work.try_recv().is_err());
    drop(command);
    owner.stop_if_drained();
    assert!(owner.stopping);
    assert!(!owner.valid.load(Ordering::Acquire));
}

#[test]
fn collection_only_import_refuses_stopped_invalid_or_replaced_owner() {
    for rejected in 0..4 {
        let (mut owner, work) = owner();
        let identity = if rejected == 0 {
            "replacement"
        } else {
            "device-owner"
        };
        let route = if rejected == 1 {
            "container:758"
        } else {
            "container:756"
        };
        if rejected == 2 {
            owner.stopping = true;
        }
        if rejected == 3 {
            owner.valid.store(false, Ordering::Release);
        }
        let collection =
            ProfileCollection::from_document(&json!({"profiles":[{"guid":"imported"}]})).unwrap();
        assert!(
            !owner
                .acknowledge_collection(identity, route, Ok(collection), json!({"import":true}))
                .unwrap()
        );
        assert!(owner.document.is_none());
        assert!(owner.source.is_none());
        assert!(owner.collection.is_none());
        assert_eq!(owner.revision, 0);
        assert_eq!(owner.recoveries.load(Ordering::Acquire), 0);
        assert!(work.try_recv().is_err());
    }
}

#[test]
fn collection_only_import_does_not_invent_an_unobserved_document_or_restore() {
    let (mut owner, work) = owner();
    let collection =
        ProfileCollection::from_document(&json!({"profiles":[{"guid":"imported"}]})).unwrap();
    let source = json!({"import":true});
    owner
        .acknowledge_collection(
            "device-owner",
            "container:756",
            Ok(collection),
            source.clone(),
        )
        .unwrap();
    assert!(owner.document.is_none());
    assert_eq!(owner.source, Some(source));
    assert_eq!(owner.revision_signal.load(Ordering::Acquire), 0);
    assert!(owner.coordinator.is_idle());
    assert!(matches!(
        work.try_recv().unwrap(),
        Command::CollectionOnly { .. }
    ));
    owner
        .dispatch(Instant::now() + Duration::from_secs(1))
        .unwrap();
    assert!(work.try_recv().is_err());
}

#[test]
fn disconnected_collection_worker_invalidates_admitted_restore() {
    let (mut owner, work) = owner();
    let document = json!({"activeProfile":"a","profiles":[{"guid":"a"}]});
    owner.enqueue(document.clone(), None, false);
    owner
        .dispatch(Instant::now() + Duration::from_secs(1))
        .unwrap();
    let running = restore_work(&work);
    drop(work);
    let collection =
        ProfileCollection::from_document(&json!({"profiles":[{"guid":"a"},{"guid":"imported"}]}))
            .unwrap();
    assert!(
        owner
            .acknowledge_collection(
                "device-owner",
                "container:756",
                Ok(collection),
                json!({"import":true})
            )
            .is_err()
    );
    assert!(!running.generation.is_current());
    assert!(owner.stopping);
    assert!(!owner.valid.load(Ordering::Acquire));
    assert_eq!(owner.document, Some(document));
    assert_eq!(owner.source, Some(json!({"import":true})));
    assert!(owner.collection.is_none());
    assert_eq!(owner.recoveries.load(Ordering::Acquire), 0);
}

#[test]
fn failed_collection_validation_acknowledges_local_source_without_admitting_hardware_work() {
    let (mut owner, work) = owner();
    let document = json!({"activeProfile":"a","profiles":[{"guid":"a"}]});
    owner.enqueue(document.clone(), None, false);
    let source = json!({"profiles":["a","invalid-import"]});
    assert!(
        owner
            .acknowledge_collection(
                "device-owner",
                "container:756",
                ProfileCollection::from_document(&json!({"profiles":[{"guid":""}]})),
                source.clone()
            )
            .is_err()
    );
    assert_eq!(owner.document, Some(document.clone()));
    assert_eq!(owner.source, Some(source));
    assert!(owner.collection.is_none());
    assert_eq!(owner.revision_signal.load(Ordering::Acquire), 1);
    assert_eq!(owner.recoveries.load(Ordering::Acquire), 0);
    assert!(work.try_recv().is_err());
    owner
        .dispatch(Instant::now() + Duration::from_secs(1))
        .unwrap();
    assert_eq!(restore_work(&work).value.document, document);
    owner.coordinator.finish();
    owner.dispatch(Instant::now()).unwrap();
    assert!(work.try_recv().is_err());
}
