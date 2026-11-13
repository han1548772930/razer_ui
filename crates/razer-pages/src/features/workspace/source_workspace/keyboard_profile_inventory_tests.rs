use super::*;
use razer_device::{
    backend::HidNode,
    keyboard_profile_inventory::{
        GuidStatus, MappingHashObservation, ProfileIdentity, ProfileMetadataObservation,
        ProfileSlot,
    },
};

fn target(pid: u32, path: &[u8]) -> inventory::Target {
    let cap = razer_device::keyboard_metadata::hid_capability(pid, pid).unwrap();
    inventory::Target::Collection {
        node: HidNode {
            path: path.to_vec(),
            vendor_id: cap.vendor_id,
            product_id: pid as u16,
            interface_number: i32::from(cap.claim_interface_for(pid).unwrap()),
            usage_page: 0xff00,
            usage: 1,
            release_number: 0,
            serial_number: None,
            manufacturer: None,
            product: None,
        },
        product_id: pid,
        peer_product_id: None,
    }
}

fn request(owner: &mut Owner, pid: u32, generation: u64) -> (uuid::Uuid, Arc<AtomicBool>) {
    let id = uuid::Uuid::new_v4();
    let canceled = Arc::new(AtomicBool::new(false));
    owner.begin(Request {
        id,
        generation,
        product_id: pid,
        options: None,
        canceled: canceled.clone(),
    });
    (id, canceled)
}

fn observation() -> inventory::Observation {
    inventory::Observation {
        target: target(550, b"physical-keyboard"),
        inventory: ProfileInventory {
            max_profiles_supported: 7,
            declared_profile_count: 2,
            slots: vec![
                ProfileSlot {
                    slot_id: 9,
                    metadata: ProfileMetadataObservation::Failed {
                        message: "first slot metadata not returned".into(),
                    },
                },
                ProfileSlot {
                    slot_id: 4,
                    metadata: ProfileMetadataObservation::Observed {
                        identity: ProfileIdentity {
                            guid: "12345678-1234-5678-1234-567812345678".into(),
                            guid_status: GuidStatus::Recognized,
                            name: "Actual device name".into(),
                            owner: "actual owner".into(),
                        },
                        mapping_hash: MappingHashObservation::Observed([1, 2, 3, 4, 5, 6]),
                    },
                },
            ],
        },
    }
}

#[::core::prelude::v1::test]
fn keyboard_inventory_owner_publishes_actual_device_values_and_returned_slot_order() {
    let mut owner = Owner::default();
    let (id, _) = request(&mut owner, 550, 2);
    assert!(matches!(owner.observation, Observation::Loading));
    assert!(owner.inventory().is_none());
    assert!(owner.finish(id, 2, Ok((observation(), None))));
    let observed = owner.inventory().unwrap();
    assert_eq!(observed.max_profiles_supported, 7);
    assert_eq!(
        observed
            .slots
            .iter()
            .map(|slot| slot.slot_id)
            .collect::<Vec<_>>(),
        [9, 4]
    );
    let ProfileMetadataObservation::Observed { identity, .. } = &observed.slots[1].metadata else {
        panic!("actual metadata lost")
    };
    assert_eq!(identity.name, "Actual device name");
    assert_eq!(identity.owner, "actual owner");
    assert_eq!(owner.error(), Some("first slot metadata not returned"));
    assert!(matches!(
        &observed.slots[0].metadata,
        ProfileMetadataObservation::Failed { .. }
    ));
}

#[::core::prelude::v1::test]
fn keyboard_inventory_failure_clears_previous_observation_and_rejects_old_completion() {
    let mut owner = Owner::default();
    let (old, old_guard) = request(&mut owner, 550, 1);
    assert!(owner.finish(old, 1, Ok((observation(), None))));
    let (next, _) = request(&mut owner, 550, 2);
    assert!(old_guard.load(Ordering::Acquire));
    assert!(owner.inventory().is_none());
    assert!(!owner.finish(old, 1, Ok((observation(), None))));
    assert!(owner.finish(next, 2, Err("actual read failed".into())));
    assert_eq!(owner.error(), Some("actual read failed"));
    assert!(owner.inventory().is_none());
    assert!(owner.slots().is_none());
}

#[::core::prelude::v1::test]
fn keyboard_inventory_unknown_guid_and_empty_device_list_do_not_create_local_profiles() {
    let mut owner = Owner::default();
    let (id, _) = request(&mut owner, 550, 1);
    let mut actual = observation();
    let ProfileMetadataObservation::Observed { identity, .. } =
        &mut actual.inventory.slots[1].metadata
    else {
        panic!("metadata missing")
    };
    identity.guid = "ffffffff-ffff-ffff-ffff-ffffffffffff".into();
    identity.guid_status = GuidStatus::Unrecognized;
    assert!(owner.finish(id, 1, Ok((actual, None))));
    assert_eq!(owner.slots().unwrap().len(), 2);
    let (id, _) = request(&mut owner, 550, 2);
    let mut actual = observation();
    actual.inventory.declared_profile_count = 0;
    actual.inventory.slots.clear();
    assert!(owner.finish(id, 2, Ok((actual, None))));
    assert_eq!(owner.slots(), Some(&[][..]));
    assert!(owner.error().is_none());
}

#[::core::prelude::v1::test]
fn keyboard_inventory_disconnect_cancel_and_generation_change_hide_confirmed_slots() {
    let mut owner = Owner::default();
    let (id, guard) = request(&mut owner, 550, 1);
    assert!(!owner.finish(id, 2, Ok((observation(), None))));
    assert!(owner.finish(id, 1, Ok((observation(), None))));
    guard.store(true, Ordering::Release);
    assert!(owner.inventory().is_none());
    assert!(!owner.finish(id, 1, Ok((observation(), None))));
    owner.invalidate(false);
    assert!(matches!(owner.observation, Observation::Disconnected));
    let (next, _) = request(&mut owner, 550, 3);
    assert!(!owner.finish(id, 3, Err("late canceled result".into())));
    assert!(owner.finish(next, 3, Ok((observation(), None))));
}

#[::core::prelude::v1::test]
fn keyboard_inventory_owner_rejects_foreign_product_and_malformed_observation() {
    let mut owner = Owner::default();
    let (id, _) = request(&mut owner, 551, 1);
    assert!(owner.finish(id, 1, Ok((observation(), None))));
    assert!(owner.inventory().is_none());
    assert!(owner.error().unwrap().contains("another product"));
    let (id, _) = request(&mut owner, 550, 2);
    let mut actual = observation();
    actual.inventory.declared_profile_count = 1;
    assert!(owner.finish(id, 2, Ok((actual, None))));
    assert!(owner.inventory().is_none());
    assert!(owner.error().unwrap().contains("count differs"));
}

#[::core::prelude::v1::test]
fn keyboard_inventory_partial_756_slots_never_fabricate_maximum_or_count() {
    let mut owner = Owner::default();
    let (id, _) = request(&mut owner, 756, 1);
    let slot = ProfileSlot {
        slot_id: 6,
        metadata: ProfileMetadataObservation::Observed {
            identity: ProfileIdentity {
                guid: "12345678-1234-5678-1234-567812345678".into(),
                guid_status: GuidStatus::Recognized,
                name: "Observed 756 profile".into(),
                owner: "other local owner".into(),
            },
            mapping_hash: MappingHashObservation::Observed([1, 2, 3, 4, 5, 6]),
        },
    };
    assert!(owner.finish_slots(
        id,
        1,
        Ok(inventory::SlotsObservation {
            target: target(756, b"current-756"),
            slots: vec![slot.clone()]
        })
    ));
    assert_eq!(owner.slots(), Some([slot].as_slice()));
    assert!(owner.inventory().is_none());
    let (new, _) = request(&mut owner, 756, 2);
    assert!(!owner.finish_slots(id, 1, Err("stale 756 failure".into())));
    assert!(owner.finish_slots(new, 2, Err("756 query failed".into())));
    assert!(owner.slots().is_none());
    assert_eq!(owner.error(), Some("756 query failed"));
}

#[::core::prelude::v1::test]
fn keyboard_inventory_complete_reader_preserves_product_and_reconnection_identity() {
    use razer_device::{
        keyboard_onboard_profiles::{ImportDetails, KeyIdentity, Snapshot},
        onboard_macro::{MacroCatalog, MacroInventory, MacroStorageInfo},
    };

    for product_id in [
        550, 551, 552, 577, 579, 590, 599, 600, 602, 617, 658, 661, 664, 678, 679, 691, 697, 717,
    ] {
        let options = ReadOptions {
            layout_id: 1,
            locally_known_macro_guids: Default::default(),
        };
        let mut keys = razer_device::keyboard_onboard_inventory::inventory(product_id, 1)
            .unwrap()
            .iter()
            .map(|key| KeyIdentity {
                input_id: key.input_id.into(),
                firmware_key_id: key.firmware_key_id,
                ignore_normal: key.ignore_normal,
                ignore_hypershift: key.ignore_hypershift,
            })
            .collect::<Vec<_>>();
        keys.sort_unstable_by_key(|key| key.firmware_key_id);
        let inventory = inventory::Observation {
            target: target(product_id, b"same-reconnected-path"),
            inventory: ProfileInventory {
                max_profiles_supported: 7,
                declared_profile_count: 0,
                slots: Vec::new(),
            },
        };
        let full = CompleteReading {
            snapshot: Snapshot {
                product_id,
                layout_id: 1,
                key_inventory: keys,
                macros: MacroCatalog {
                    inventory: MacroInventory {
                        storage: MacroStorageInfo {
                            max_macro_id: 32767,
                            max_memory: 65536,
                            available_memory: 45678,
                            recyclable_memory: 123,
                        },
                        count: 0,
                        ids: Vec::new(),
                        pages: Vec::new(),
                    },
                    macros: Vec::new(),
                },
                inventory: inventory.inventory.clone(),
                profiles: Vec::new(),
            },
            import_details: ImportDetails {
                macros: Vec::new(),
                continuous_rapid_trigger: Vec::new(),
            },
        };
        full.validate_for(&options).unwrap();
        let mut owner = Owner::default();
        let (first, guard) = request(&mut owner, product_id, 1);
        owner.request.as_mut().unwrap().options = Some(options.clone());
        assert!(owner.finish(first, 1, Ok((inventory.clone(), Some(full.clone())))));
        let Observation::Ready { complete, .. } = &owner.observation else {
            panic!("Complete reading was discarded for {product_id}")
        };
        assert_eq!(
            complete
                .as_ref()
                .unwrap()
                .snapshot
                .macros
                .inventory
                .storage
                .available_memory,
            45678
        );
        let (next, _) = request(&mut owner, product_id, 2);
        owner.request.as_mut().unwrap().options = Some(options.clone());
        assert!(guard.load(Ordering::Acquire));
        assert!(owner.inventory().is_none());
        assert!(!owner.finish(first, 1, Ok((inventory.clone(), Some(full.clone())))));
        assert!(owner.finish(next, 2, Err("reconnected device read failed".into())));
        assert!(owner.inventory().is_none());
        assert_eq!(owner.error(), Some("reconnected device read failed"));

        let (third, _) = request(&mut owner, product_id, 3);
        owner.request.as_mut().unwrap().options = Some(options.clone());
        let mut foreign = full;
        foreign.snapshot.product_id = if product_id == 679 { 678 } else { 679 };
        foreign.snapshot.key_inventory = razer_device::keyboard_onboard_inventory::inventory(
            foreign.snapshot.product_id,
            options.layout_id,
        )
        .unwrap()
        .iter()
        .map(|key| KeyIdentity {
            input_id: key.input_id.into(),
            firmware_key_id: key.firmware_key_id,
            ignore_normal: key.ignore_normal,
            ignore_hypershift: key.ignore_hypershift,
        })
        .collect();
        foreign
            .snapshot
            .key_inventory
            .sort_unstable_by_key(|key| key.firmware_key_id);
        foreign.validate_for(&options).unwrap();
        assert!(owner.finish(third, 3, Ok((inventory, Some(foreign)))));
        assert!(owner.inventory().is_none());
        assert_eq!(
            owner.error(),
            Some("Complete keyboard read and inventory differ")
        );
    }
}
