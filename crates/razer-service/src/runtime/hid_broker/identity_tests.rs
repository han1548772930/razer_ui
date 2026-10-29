use super::*;

fn node() -> HidNode {
    HidNode {
        path: br"\\?\hid#vid_1532&pid_00bf&mi_01#test".to_vec(),
        vendor_id: 0x1532,
        product_id: 0x00bf,
        usage_page: 1,
        usage: 2,
        interface_number: 1,
        release_number: 0x100,
        serial_number: None,
        manufacturer: None,
        product: None,
    }
}

fn observed() -> serde_json::Value {
    serde_json::json!({"complete":true,"failures":[],"interfaces":[{
        "path":String::from_utf8(node().path).unwrap(),"vendor_id":0x1532,"product_id":0xbf,
        "version_number":0x100,"device_container_id":"{12345678-1234-4234-8234-123456789abc}",
        "container_id_error":null,"device_instance_id":"HID\\VID_1532&PID_00BF&MI_01\\test",
        "device_instance_id_error":null,"parent_instance_id":"USB\\VID_1532&PID_00BF&MI_01\\parent",
        "parent_instance_id_error":null,"claim_interface":1,"claim_interface_error":null,
        "usage_page":1,"usage":2
    }]})
}

#[test]
fn windows_collection_keeps_physical_identity_and_raw_node() {
    let selected = node();
    let identity = parse(&selected, observed()).unwrap();
    assert_eq!(identity.node, selected);
    assert!(
        matches!(identity.platform, PlatformIdentity::Windows { container_id, hid_instance_id, parent_instance_id } if container_id=="{12345678-1234-4234-8234-123456789abc}" && hid_instance_id.starts_with("HID") && parent_instance_id.starts_with("USB"))
    );
}

#[test]
fn missing_nullable_fields_are_schema_failures_not_unobserved_defaults() {
    for field in [
        "device_container_id",
        "container_id_error",
        "device_instance_id",
        "device_instance_id_error",
        "parent_instance_id",
        "parent_instance_id_error",
        "claim_interface",
        "claim_interface_error",
        "usage_page",
        "usage",
    ] {
        let mut raw = observed();
        raw["interfaces"][0].as_object_mut().unwrap().remove(field);
        assert!(
            parse(&node(), raw)
                .unwrap_err()
                .to_string()
                .contains("schema"),
            "{field}"
        );
    }
}

#[test]
fn mismatched_unobserved_and_duplicate_collections_are_rejected() {
    for (field, value) in [
        ("path", serde_json::json!("other")),
        ("vendor_id", serde_json::json!(1)),
        ("product_id", serde_json::json!(0xbe)),
        ("version_number", serde_json::json!(0x101)),
        ("usage_page", serde_json::json!(null)),
        ("usage", serde_json::json!(1)),
        ("claim_interface", serde_json::json!(2)),
        ("device_container_id", serde_json::json!(null)),
        ("parent_instance_id", serde_json::json!("")),
        (
            "device_instance_id_error",
            serde_json::json!("native query failed"),
        ),
    ] {
        let mut raw = observed();
        raw["interfaces"][0][field] = value;
        assert!(parse(&node(), raw).is_err(), "{field}");
    }
    let mut raw = observed();
    raw["complete"] = serde_json::json!(false);
    assert!(parse(&node(), raw).is_err());
    let mut raw = observed();
    let extra = raw["interfaces"][0].clone();
    raw["interfaces"].as_array_mut().unwrap().push(extra);
    assert!(parse(&node(), raw).is_err());
}

#[test]
fn malformed_and_failed_native_inventory_cannot_establish_identity() {
    let mut raw = observed();
    raw["interfaces"][0]["product_id"] = serde_json::json!("191");
    assert!(parse(&node(), raw).is_err());
    let mut raw = observed();
    raw["failures"] = serde_json::json!([{"operation":"open_interface","error":"access denied"}]);
    assert!(format!("{:#}", parse(&node(), raw).unwrap_err()).contains("access denied"));
}
