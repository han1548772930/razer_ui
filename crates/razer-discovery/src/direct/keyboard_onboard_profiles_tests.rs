use super::*;
use razer_device::device_reads::DeviceReadTarget;
use serde_json::json;

fn target() -> Target {
    Target::WindowsContainer {
        target: DeviceReadTarget {
            product_id: 679,
            physical_product_id: 679,
            peer_product_id: None,
            device_container_id: "actual-container".into(),
            path: "actual-hid".into(),
        },
    }
}

fn options() -> ReadOptions {
    ReadOptions {
        layout_id: 1,
        locally_known_macro_guids: Default::default(),
    }
}

fn device_reply() -> serde_json::Value {
    let keys = razer_device::keyboard_onboard_inventory::current_679_inventory(1);
    let key_inventory = keys
        .iter()
        .map(|key| {
            json!({
                "input_id":key.input_id,"firmware_key_id":key.firmware_key_id,
                "ignore_normal":key.ignore_normal,"ignore_hypershift":key.ignore_hypershift,
            })
        })
        .collect::<Vec<_>>();
    let passes = [("Normal", false), ("Hypershift", true)].into_iter().map(|(mode, hyper)| {
        let ids: Vec<_> = keys.iter().filter(|key| if hyper { !key.ignore_hypershift } else { !key.ignore_normal })
            .map(|key| key.firmware_key_id).collect();
        let batches = |size| ids.chunks(size).map(|chunk| json!({"key_ids":chunk,
            "reading":{"Failed":{"message":"actual device command failed"}}})).collect::<Vec<_>>();
        let assignments = ids.chunks(8).map(|chunk| json!({"key_ids":chunk,
            "attempts":[{"Failed":{"message":"actual assignment read failed"}}]})).collect::<Vec<_>>();
        json!({"mode":mode,"assignments":assignments,"rapid_trigger":batches(13),"actuation":batches(15)})
    }).collect::<Vec<_>>();
    json!({"target":target(),"options":options(),"reading":{
        "snapshot":{"product_id":679,"layout_id":1,"key_inventory":key_inventory,
            "macros":{"inventory":{"storage":{"max_macro_id":32767,"max_memory":65536,
                "available_memory":43210,"recyclable_memory":17},"count":0,"ids":[],"pages":[]},"macros":[]},
            "inventory":{"max_profiles_supported":6,"declared_profile_count":1,"slots":[{
                "slot_id":3,"metadata":{"Observed":{"identity":{"guid":"12345678-1234-5678-1234-567812345678",
                    "guid_status":"Recognized","name":"Actual hardware profile","owner":"hardware-owner"},"mapping_hash":"Unobserved"}}
            }]},
            "profiles":[{"slot_id":3,"brightness":{"Observed":{"profile_id":3,"region_id":7,"raw_value":51,"percent":20}},
                "passes":passes,"kill_switch":{"Observed":{"raw_enabled":9,"key_id1":4,"key_id2":8}}}]
        },
        "import_details":{"macros":[],"continuous_rapid_trigger":[{"slot_id":3,"raw_enabled":{"Observed":7}}]}
    }})
}

#[test]
fn keyboard_onboard_profiles_discovery_keeps_actual_values_failed_batches_and_unknown_flags() {
    let result = read_with(&target(), &options(), |request| {
        let ServiceRequest::KeyboardOnboardProfilesRead {
            target: requested,
            options: filter,
        } = request
        else {
            panic!("wrong request")
        };
        assert_eq!(requested, target());
        assert_eq!(filter, options());
        Ok(device_reply())
    })
    .unwrap();
    assert_eq!(
        result.snapshot.profiles[0]
            .brightness
            .observed()
            .unwrap()
            .percent,
        20
    );
    assert_eq!(
        result.snapshot.profiles[0]
            .kill_switch
            .observed()
            .unwrap()
            .raw_enabled,
        9
    );
    assert!(result.snapshot.profiles[0].actuation(4).is_none());
    assert_eq!(
        result.import_details.continuous_rapid_trigger[0].raw_enabled,
        razer_device::keyboard_onboard_profiles::Reading::Observed(7)
    );
}

#[test]
fn keyboard_onboard_profiles_discovery_rejects_missing_failed_and_stale_observations() {
    assert!(read_with(&target(), &options(), |_| anyhow::bail!("unplugged")).is_err());
    for malformed in [json!(null), json!({"target":target(),"options":options()})] {
        assert!(read_with(&target(), &options(), |_| Ok(malformed.clone())).is_err());
    }
    let mut stale = device_reply();
    stale["target"]["target"]["path"] = json!("old-hid");
    assert!(read_with(&target(), &options(), |_| Ok(stale.clone())).is_err());
    let mut missing = device_reply();
    missing["reading"]["snapshot"]["profiles"][0]["passes"][1]["actuation"]
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(read_with(&target(), &options(), |_| Ok(missing.clone())).is_err());
    let mut wrong_layout = device_reply();
    wrong_layout["options"]["layout_id"] = json!(2);
    assert!(read_with(&target(), &options(), |_| Ok(wrong_layout.clone())).is_err());
}
