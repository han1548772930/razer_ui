use super::*;

#[test]
fn profile_collection_appends_in_source_order_without_replacing_retained_state() {
    let collection = ProfileCollection::from_document(&json!({
        "activeProfile":"imported-b","version":99,"brightness":1,
        "profiles":[{"guid":"a","name":"Captured A"},{"guid":"imported-b","name":"B"},{"guid":"imported-c","name":"C","extension":{"kept":true}}]
    })).unwrap();
    let mut retained = json!({
        "activeProfile":"a","version":7,"brightness":80,
        "deviceMetadatas":{"serial":{"activeProfileGuid":"a","name":"Actual owner"}},
        "profiles":[{"guid":"a","name":"Observed A","appEngine":{"actual":true}}]
    });
    assert!(collection.append_to(&mut retained).unwrap());
    assert_eq!(
        retained,
        json!({
            "activeProfile":"a","version":7,"brightness":80,
            "deviceMetadatas":{"serial":{"activeProfileGuid":"a","name":"Actual owner"}},
            "profiles":[{"guid":"a","name":"Observed A","appEngine":{"actual":true}},
                {"guid":"imported-b","name":"B"},{"guid":"imported-c","name":"C","extension":{"kept":true}}]
        })
    );
    let once = retained.clone();
    assert!(!collection.append_to(&mut retained).unwrap());
    assert_eq!(retained, once);
}

#[test]
fn profile_collection_rejects_missing_invalid_or_duplicate_guids_without_partial_mutation() {
    for invalid in [
        json!({}),
        json!({"profiles":{}}),
        json!({"profiles":[{}]}),
        json!({"profiles":[{"guid":""}]}),
        json!({"profiles":[{"guid":12}]}),
        json!({"profiles":[{"guid":"a"},{"guid":"a"}]}),
    ] {
        assert!(ProfileCollection::from_document(&invalid).is_err());
    }
    let collection =
        ProfileCollection::from_document(&json!({"profiles":[{"guid":"imported"}]})).unwrap();
    for mut invalid in [
        json!({"activeProfile":"a"}),
        json!({"profiles":[{"guid":"a"},{}]}),
        json!({"profiles":[{"guid":"a"},{"guid":"a"}]}),
    ] {
        let original = invalid.clone();
        assert!(collection.append_to(&mut invalid).is_err());
        assert_eq!(invalid, original);
    }
}

#[test]
fn profile_collection_keeps_ble_deferred_selection_and_persistence_owner() {
    let stored = json!({"activeProfile":"a","version":8,
        "profiles":[{"guid":"a"},{"guid":"b"}],
        "deviceMetadatas":{"serial":{"activeProfileGuid":"a"}}});
    let mut ble = BleProfiles::default();
    ble.hypershift(true);
    let slot = Slot {
        slot_id: 3,
        guid: "b".into(),
    };
    let deferred = ble
        .switched_for_serial(&stored, &slot, false, true, "serial", "Device")
        .unwrap()
        .unwrap();
    let collection =
        ProfileCollection::from_document(&json!({"activeProfile":"imported","version":999,
        "profiles":[{"guid":"a"},{"guid":"b"},{"guid":"imported","name":"Imported"}]}))
        .unwrap();
    ble.append_collection(&collection).unwrap();
    assert!(ble.persistence_deferred());
    assert!(ble.hypershift);
    let current = ble.document(&stored);
    assert_eq!(current["activeProfile"], "b");
    assert_eq!(current["version"], deferred["version"]);
    assert_eq!(current["deviceMetadatas"], deferred["deviceMetadatas"]);
    assert_eq!(current["profiles"][2]["guid"], "imported");
    assert!(ble.switched(&stored, &slot, true, true).unwrap().is_none());
    ble.hypershift(false);
    assert!(ble.persistence_deferred());
    ble.persisted();
    assert!(!ble.persistence_deferred());
}

#[test]
fn drain_waits_for_debounce_running_work_and_every_fifo_update() {
    let mut queue = Coordinator::default();
    let now = Instant::now();
    assert!(queue.is_idle());
    queue.request(1, "full", now, Duration::from_millis(50));
    queue.enqueue_update(2, "first save");
    queue.enqueue_update(3, "last save");
    assert!(!queue.is_idle());
    assert!(queue.take_ready(now).is_none());
    let at = now + Duration::from_millis(50);
    for (id, value) in [(1, "full"), (2, "first save"), (3, "last save")] {
        let work = queue.take_ready(at).unwrap();
        assert_eq!((work.id, work.value), (id, value));
        assert!(work.generation.is_current());
        assert!(!queue.is_idle());
        assert!(queue.take_ready(at).is_none());
        queue.finish();
        assert_eq!(queue.is_idle(), id == 3);
    }
}

#[test]
fn generation_drops_pending_work_and_delayed_request_blocks_updates() {
    let mut queue = Coordinator::default();
    let now = Instant::now();
    assert!(queue.request(1, "a", now, Duration::ZERO).is_empty());
    let running = queue.take_ready(now).unwrap();
    queue.enqueue_update(2, "b");
    assert_eq!(queue.request(3, "c", now, Duration::from_millis(50)), [2]);
    assert!(!running.generation.is_current());
    assert!(queue.take_ready(now + Duration::from_millis(100)).is_none());
    queue.finish();
    queue.enqueue_update(4, "d");
    assert!(queue.take_ready(now).is_none());
    let current = queue.take_ready(now + Duration::from_millis(50)).unwrap();
    assert_eq!(current.id, 3);
    assert!(current.generation.is_current());
    queue.finish();
    assert_eq!(
        queue
            .take_ready(now + Duration::from_millis(50))
            .unwrap()
            .id,
        4
    );
}
#[test]
fn ble_slot_switch_preserves_deferred_cache_until_actual_persistence() {
    let slots = slots(&[Slot {
        slot_id: 1,
        guid: "custom".into(),
    }]);
    let doc = json!({"activeProfile":"custom","version":crate::mapping_submission::MAX_SOURCE_VERSION,
        "profiles":[{"guid":"custom"},{"guid":slots[0].guid}],"retained":true});
    let mut state = BleProfiles::default();
    state.hypershift(true);
    let target = state
        .target(&doc, &slots, &BleSwitch::Slot(2))
        .unwrap()
        .clone();
    let result = state.switched(&doc, &target, true, true).unwrap().unwrap();
    assert_eq!(result["version"], 0);
    assert_eq!(result["retained"], true);
    assert!(state.persistence_deferred());
    state.hypershift(false);
    assert_eq!(state.deferred(), Some(&result));
    assert!(state.switched(&doc, &target, true, true).unwrap().is_none());
    state.persisted();
    assert!(state.deferred().is_none());
    assert!(state.target(&doc, &slots, &BleSwitch::Slot(99)).is_err());
    assert_eq!(
        ble_profile_frame(30, Some(6)).unwrap(),
        [30, 1, 0, 0, 3, 2, 0, 0, 6]
    );
    assert_eq!(
        ble_profile_frame(0, None).unwrap(),
        [0, 0, 0, 0, 3, 130, 0, 0]
    );
    assert!(ble_profile_response(&[]).is_err());
    assert!(ble_profile_response(&[0]).is_err());
}

#[test]
fn real_slot_notification_persists_even_when_hypershift_and_keeps_serial_owners() {
    let mut state = BleProfiles::default();
    state.hypershift(true);
    let stored = json!({"activeProfile":"a","version":8,"profiles":[{"guid":"a"},{"guid":"b"}],"deviceMetadatas":{"other":{"name":"Other","activeProfileGuid":"a"},"serial":{"name":"User name","extra":true}}});
    let target = Slot {
        slot_id: 3,
        guid: "b".into(),
    };
    let deferred = state
        .switched_for_serial(&stored, &target, false, true, "serial", "Device")
        .unwrap()
        .unwrap();
    assert!(state.persistence_deferred());
    assert_eq!(deferred["deviceMetadatas"]["serial"]["name"], "User name");
    assert_eq!(
        deferred["deviceMetadatas"]["other"],
        stored["deviceMetadatas"]["other"]
    );
    let target = Slot {
        slot_id: 2,
        guid: "a".into(),
    };
    let actual = state
        .switched_for_serial(&stored, &target, true, false, "serial", "Device")
        .unwrap()
        .unwrap();
    assert!(!state.persistence_deferred());
    assert_eq!(
        actual["deviceMetadatas"]["serial"]["activeProfileGuid"],
        "a"
    );
    assert_eq!(actual["deviceMetadatas"]["serial"]["extra"], true);
}

#[test]
fn local_navigation_uses_actual_slot_order_then_names_and_source_cycle_skip() {
    let mut document = json!({"activeProfile":"onboard-last","profiles":[{"guid":"z","name":"zulu"},{"guid":"onboard-last","name":"A"},{"guid":"local-a","name":"alpha"},{"guid":"factory","name":"Factory"}]});
    let slots = [
        Slot {
            slot_id: 2,
            guid: "factory".into(),
        },
        Slot {
            slot_id: 6,
            guid: "onboard-last".into(),
        },
    ];
    assert_eq!(
        navigate_local(
            &document,
            &slots,
            &BleSwitch::Navigation("NextProfile".into()),
            true
        )
        .unwrap(),
        "local-a"
    );
    document["activeProfile"] = json!("z");
    assert_eq!(
        navigate_local(
            &document,
            &slots,
            &BleSwitch::Navigation("CycleUp".into()),
            true
        )
        .unwrap(),
        "onboard-last"
    );
    assert_eq!(
        navigate_local(
            &document,
            &slots,
            &BleSwitch::Navigation("CycleUp".into()),
            false
        )
        .unwrap(),
        "factory"
    );
    assert_eq!(
        navigate_local(
            &document,
            &slots,
            &BleSwitch::Navigation("NextProfile".into()),
            true
        )
        .unwrap(),
        "z"
    );
    assert!(navigate_local(&document, &slots, &BleSwitch::Guid("absent".into()), true).is_err());
}
