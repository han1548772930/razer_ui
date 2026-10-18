use super::*;
use razer_storage::host::HostStorageView;
use serde_json::Value;

#[::core::prelude::v1::test]
fn product_chroma_failed_profile_read_still_observes_installation_and_preserves_both_errors() {
    let mut calls = Vec::new();
    let readback = readback_with(
        || chroma_profiles::Observation {
            snapshot: Err("profile read failed".into()),
            dynamic_lighting: Ok(Some(true)),
            module_installed: Err("module record invalid".into()),
        },
        InstallationRead::Required,
        || {
            calls.push("installation");
            Err(anyhow::anyhow!("installation read failed"))
        },
    );
    assert_eq!(calls, ["installation"]);
    let error = readback.result().unwrap_err();
    assert!(error.contains("profile read failed"));
    assert!(error.contains("module record invalid"));
    assert!(error.contains("installation read failed"));
    assert_eq!(readback.snapshot.unwrap_err(), "profile read failed");
    assert_eq!(readback.dynamic_lighting, Ok(Some(true)));
    assert_eq!(
        readback.module_installed.unwrap_err(),
        "module record invalid"
    );
    assert_eq!(
        readback.installation.unwrap_err(),
        "installation read failed"
    );
}

#[::core::prelude::v1::test]
fn product_chroma_local_projection_does_not_invoke_the_installation_probe() {
    let readback = readback_with(
        || chroma_profiles::Observation {
            snapshot: Err("profile decode failed".into()),
            dynamic_lighting: Ok(Some(true)),
            module_installed: Ok(Some(false)),
        },
        InstallationRead::NotRequested,
        || panic!("Babel has no checkIsFullyInstall caller"),
    );
    assert!(matches!(readback.installation, Ok(None)));
    assert_eq!(readback.dynamic_lighting, Ok(Some(true)));
    assert_eq!(readback.module_installed, Ok(Some(false)));
    assert!(
        readback
            .result()
            .unwrap_err()
            .contains("profile decode failed")
    );
}

fn register(storage: &mut HostStorage, id: u64, url: &str) {
    storage.register_view(HostStorageView {
        id,
        url: url.into(),
        remote: false,
        destroyed: false,
        crashed: false,
        disposed: false,
        tracked_url: true,
    });
}

fn publish(storage: &mut HostStorage, sender_id: u64, devices: Value) {
    storage
        .call(HostStorageCall {
            store: HostStoreKind::Memory,
            sender_id,
            action: "setMemoryStorageItem".into(),
            payload: json!({"key": CONNECTED_DEVICES_KEY, "value": devices.to_string()}),
            target_url_array: Vec::new(),
        })
        .unwrap();
}

#[::core::prelude::v1::test]
fn product_chroma_memory_uses_the_first_publisher_and_preserves_source_order() {
    let mut storage = HostStorage::default();
    register(&mut storage, 1, "/lighting-engine/");
    register(&mut storage, 2, "/second-lighting-engine/");
    register(&mut storage, 3, "/synapse/products/4115/ui/");
    publish(
        &mut storage,
        1,
        json!({"payload":{"devices":[
            {"productId":679,"extra":"keep producer metadata outside the projection"},
            {"productId":190}, {"productId":679}
        ]}}),
    );
    publish(
        &mut storage,
        2,
        json!({"payload":{"devices":[{"productId":241}]}}),
    );
    assert_eq!(
        connected_products(&mut storage, 3).unwrap(),
        Some(vec![679, 190, 679])
    );
}

#[::core::prelude::v1::test]
fn product_chroma_memory_distinguishes_no_observation_from_an_observed_empty_list() {
    let mut storage = HostStorage::default();
    register(&mut storage, 1, "/lighting-engine/");
    register(&mut storage, 2, "/synapse/products/190/ui/");
    assert_eq!(connected_products(&mut storage, 2).unwrap(), None);
    publish(&mut storage, 1, json!({"payload":{"devices":[]}}));
    assert_eq!(connected_products(&mut storage, 2).unwrap(), Some(vec![]));
}

#[::core::prelude::v1::test]
fn product_chroma_memory_follows_updates_without_reopening_the_consumer() {
    let mut storage = HostStorage::default();
    register(&mut storage, 1, "/lighting-engine/");
    register(&mut storage, 2, "/synapse/products/4115/ui/");
    publish(
        &mut storage,
        1,
        json!({"payload":{"devices":[{"productId":4115}]}}),
    );
    assert_eq!(
        connected_products(&mut storage, 2).unwrap(),
        Some(vec![4115])
    );
    publish(
        &mut storage,
        1,
        json!({"payload":{"devices":[{"productId":190},{"productId":4115}]}}),
    );
    assert_eq!(
        connected_products(&mut storage, 2).unwrap(),
        Some(vec![190, 4115])
    );
    publish(&mut storage, 1, json!({"payload":{"devices":[]}}));
    assert_eq!(connected_products(&mut storage, 2).unwrap(), Some(vec![]));
}

#[::core::prelude::v1::test]
fn product_chroma_memory_rejects_malformed_or_undefined_transport_values() {
    for response in [
        HostStorageReply {
            defined: false,
            value: Value::Null,
        },
        HostStorageReply {
            defined: true,
            value: json!([]),
        },
        HostStorageReply {
            defined: true,
            value: json!("not JSON"),
        },
        HostStorageReply {
            defined: true,
            value: json!("[{}]"),
        },
    ] {
        assert!(decode_connected_products(response).is_err());
    }
}

#[::core::prelude::v1::test]
fn product_chroma_memory_rejects_missing_or_invalid_product_identity() {
    for devices in [
        json!({}),
        json!({"payload":{}}),
        json!({"payload":{"devices":null}}),
        json!({"payload":{"devices":[{}]}}),
        json!({"payload":{"devices":[{"productId":"679"}]}}),
        json!({"payload":{"devices":[{"productId":-1}]}}),
        json!({"payload":{"devices":[{"productId":4294967296u64}]}}),
    ] {
        let response = HostStorageReply {
            defined: true,
            value: json!(json!([{"value":devices.to_string()}]).to_string()),
        };
        assert!(decode_connected_products(response).is_err());
    }
}
