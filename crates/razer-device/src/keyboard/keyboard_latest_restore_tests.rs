use super::*;
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
