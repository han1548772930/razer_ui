use super::*;

fn identity(serial: &str, container: &str, path: &[u8]) -> ThxConnectionIdentity {
    ThxConnectionIdentity::new(1398, serial, container, 1398, path).unwrap()
}

#[test]
fn initial_empty_and_unknown_scans_do_not_create_connections() {
    let mut tracker = ThxRenderDiscoveryTracker::default();
    let key = identity("serial", "{container}", b"collection");
    assert!(tracker.observe(None).is_empty());
    assert!(tracker.observe(Some(&[])).is_empty());
    assert!(!tracker.contains(&key));
    assert!(tracker.current_epoch(&key).is_none());
    assert_eq!(tracker.observe(Some(&[key]))[0].epoch, 1);
}

#[test]
fn unchanged_and_reordered_scans_preserve_epochs_without_setup_requests() {
    let mut tracker = ThxRenderDiscoveryTracker::default();
    let first = identity("first", "{one}", b"collection-one");
    let second = identity("second", "{two}", b"collection-two");
    let added = tracker.observe(Some(&[first.clone(), second.clone()]));
    assert_eq!(added.len(), 2);
    let first_epoch = tracker.current_epoch(&first).unwrap();
    let second_epoch = tracker.current_epoch(&second).unwrap();
    assert_ne!(first_epoch.epoch, second_epoch.epoch);
    assert!(
        tracker
            .observe(Some(&[second.clone(), first.clone()]))
            .is_empty()
    );
    assert_eq!(tracker.current_epoch(&first), Some(first_epoch));
    assert_eq!(tracker.current_epoch(&second), Some(second_epoch));
}

#[test]
fn confirmed_removal_and_reconnect_create_a_new_epoch_only_for_that_route() {
    let mut tracker = ThxRenderDiscoveryTracker::default();
    let first = identity("first", "{one}", b"collection-one");
    let second = identity("second", "{two}", b"collection-two");
    tracker.observe(Some(&[first.clone(), second.clone()]));
    let first_epoch = tracker.current_epoch(&first).unwrap();
    let second_epoch = tracker.current_epoch(&second).unwrap();
    assert!(tracker.observe(Some(&[second.clone()])).is_empty());
    assert!(!tracker.contains(&first));
    assert!(tracker.current_epoch(&first).is_none());
    assert_eq!(tracker.current_epoch(&second), Some(second_epoch.clone()));
    let reconnected = tracker.observe(Some(&[first.clone(), second.clone()]));
    assert_eq!(reconnected.len(), 1);
    assert_eq!(reconnected[0].identity, first);
    assert!(reconnected[0].epoch > first_epoch.epoch);
    assert!(reconnected[0].epoch > second_epoch.epoch);
    assert_eq!(tracker.current_epoch(&second), Some(second_epoch));
}

#[test]
fn unknown_does_not_manufacture_a_disconnect_or_retry_uncertain_setup() {
    let mut tracker = ThxRenderDiscoveryTracker::default();
    let key = identity("serial", "{container}", b"collection");
    let observed = tracker.observe(Some(&[key.clone()])).remove(0);
    // The caller may have cancelled partial setup during an unknown scan.
    // Connection tracking has no successful-ready or retry acknowledgement.
    for _ in 0..3 {
        assert!(tracker.observe(None).is_empty());
        assert!(tracker.contains(&key));
        assert_eq!(tracker.current_epoch(&key), Some(observed.clone()));
    }
    assert!(tracker.observe(Some(&[key.clone()])).is_empty());
    assert_eq!(tracker.current_epoch(&key), Some(observed));
}

#[test]
fn unknown_preserves_existing_keys_but_a_later_confirmed_addition_is_new() {
    let mut tracker = ThxRenderDiscoveryTracker::default();
    let first = identity("first", "{one}", b"collection-one");
    let second = identity("second", "{two}", b"collection-two");
    let first_epoch = tracker.observe(Some(&[first.clone()])).remove(0);
    tracker.observe(None);
    let additions = tracker.observe(Some(&[second.clone(), first.clone()]));
    assert_eq!(additions.len(), 1);
    assert_eq!(additions[0].identity, second);
    assert!(additions[0].epoch > first_epoch.epoch);
    assert_eq!(tracker.current_epoch(&first), Some(first_epoch));
}

#[test]
fn duplicates_and_container_case_aliases_are_one_connection() {
    let upper = identity("serial", "{AbC-DEF}", b"collection");
    let lower = identity("serial", "{abc-def}", b"collection");
    assert_eq!(upper.container_id, "{abc-def}");
    assert_eq!(upper, lower);
    let mut tracker = ThxRenderDiscoveryTracker::default();
    assert_eq!(
        tracker
            .observe(Some(&[upper.clone(), lower.clone(), upper.clone()]))
            .len(),
        1
    );
    assert!(tracker.contains(&upper));
    assert_eq!(tracker.current_epoch(&upper), tracker.current_epoch(&lower));
    assert!(tracker.observe(Some(&[lower])).is_empty());
}

#[test]
fn separate_containers_do_not_collapse_even_with_the_same_product_and_serial() {
    let first = identity("same-serial", "{one}", b"collection");
    let second = identity("same-serial", "{two}", b"collection");
    let mut tracker = ThxRenderDiscoveryTracker::default();
    let additions = tracker.observe(Some(&[first.clone(), second.clone()]));
    assert_eq!(additions.len(), 2);
    assert!(tracker.contains(&first));
    assert!(tracker.contains(&second));
    assert_ne!(
        tracker.current_epoch(&first),
        tracker.current_epoch(&second)
    );
    let second_epoch = tracker.current_epoch(&second).unwrap();
    assert!(tracker.observe(Some(&[second.clone()])).is_empty());
    assert!(!tracker.contains(&first));
    assert_eq!(tracker.current_epoch(&second), Some(second_epoch));
}

#[test]
fn changed_serial_or_path_is_a_different_physical_connection() {
    let original = identity("SERIAL", "{container}", b"collection");
    let other_serial = identity("serial", "{container}", b"collection");
    let other_path = identity("serial", "{container}", b"Collection");
    let mut tracker = ThxRenderDiscoveryTracker::default();
    let original_epoch = tracker.observe(Some(&[original.clone()])).remove(0);
    let serial_epoch = tracker.observe(Some(&[other_serial.clone()])).remove(0);
    assert!(!tracker.contains(&original));
    assert!(serial_epoch.epoch > original_epoch.epoch);
    let path_epoch = tracker.observe(Some(&[other_path.clone()])).remove(0);
    assert!(!tracker.contains(&other_serial));
    assert_eq!(path_epoch.identity, other_path);
    assert!(path_epoch.epoch > serial_epoch.epoch);
}

#[test]
fn changed_owner_or_wired_dongle_pid_is_a_new_connection() {
    let wired = identity("serial", "{container}", b"collection");
    let dongle =
        ThxConnectionIdentity::new(1398, "serial", "{container}", 1399, b"collection").unwrap();
    let other_owner =
        ThxConnectionIdentity::new(4124, "serial", "{container}", 4124, b"collection").unwrap();
    let mut tracker = ThxRenderDiscoveryTracker::default();
    let wired_epoch = tracker.observe(Some(&[wired.clone()])).remove(0);
    let dongle_epoch = tracker.observe(Some(&[dongle.clone()])).remove(0);
    assert!(!tracker.contains(&wired));
    assert!(dongle_epoch.epoch > wired_epoch.epoch);
    let owner_epoch = tracker.observe(Some(&[other_owner.clone()])).remove(0);
    assert!(!tracker.contains(&dongle));
    assert_eq!(owner_epoch.identity, other_owner);
    assert!(owner_epoch.epoch > dongle_epoch.epoch);
}

#[test]
fn clearing_the_confirmed_set_does_not_reset_epoch_allocation() {
    let key = identity("serial", "{container}", b"collection");
    let mut tracker = ThxRenderDiscoveryTracker::default();
    let first = tracker.observe(Some(&[key.clone()])).remove(0);
    assert!(tracker.observe(Some(&[])).is_empty());
    assert!(!tracker.contains(&key));
    tracker.observe(None);
    let second = tracker.observe(Some(&[key])).remove(0);
    assert!(second.epoch > first.epoch);
}

#[test]
fn only_audited_owner_and_physical_pid_pairs_are_accepted() {
    for (owner, physical) in [
        (1398, 1398),
        (1398, 1399),
        (1401, 1401),
        (1401, 1402),
        (2638, 2638),
        (2638, 2645),
        (2641, 2641),
        (2641, 2648),
        (4124, 4124),
        (4124, 4123),
        (4126, 4126),
    ] {
        ThxConnectionIdentity::new(owner, "serial", "{container}", physical, b"collection")
            .unwrap();
    }
    assert!(ThxConnectionIdentity::new(0, "serial", "{container}", 1398, b"collection").is_err());
    assert!(
        ThxConnectionIdentity::new(1398, "serial", "{container}", 1402, b"collection").is_err()
    );
}

#[test]
fn text_and_collection_boundaries_reject_unsafe_or_pseudo_identities() {
    for container in ["", "{nul\0container}", "HID-COLLECTION:cached"] {
        assert!(
            ThxConnectionIdentity::new(1398, "serial", container, 1398, b"collection").is_err()
        );
    }
    assert!(
        ThxConnectionIdentity::new(1398, "nul\0serial", "{container}", 1398, b"collection")
            .is_err()
    );
    assert!(
        ThxConnectionIdentity::new(1398, "serial", "x".repeat(4097), 1398, b"collection").is_err()
    );
    assert!(
        ThxConnectionIdentity::new(1398, "x".repeat(4097), "{container}", 1398, b"collection")
            .is_err()
    );
    for path in [vec![], b"nul\0path".to_vec(), vec![b'x'; 4097]] {
        assert!(ThxConnectionIdentity::new(1398, "serial", "{container}", 1398, path).is_err());
    }
    // A unique observed route may lack a reported serial. Do not fabricate one.
    let valid = ThxConnectionIdentity::new(1398, "", "{container}", 1398, vec![b'x'; 4096]);
    assert!(valid.is_ok());
}
