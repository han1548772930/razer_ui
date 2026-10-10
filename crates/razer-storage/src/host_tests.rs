//! Pure Map/event fixtures for current host keyStorage.js and
//! modules/{memory_storage,window_storage}/index.js; no IPC, files or OS calls.
use super::*;

fn view(id: u64, url: &str) -> HostStorageView {
    HostStorageView {
        id,
        url: url.into(),
        remote: false,
        destroyed: false,
        crashed: false,
        disposed: false,
        tracked_url: true,
    }
}

fn call(
    storage: &mut HostStorage,
    store: HostStoreKind,
    sender_id: u64,
    action: &str,
    payload: Value,
) -> HostStorageReply {
    storage
        .call(HostStorageCall {
            store,
            sender_id,
            action: action.into(),
            payload,
            target_url_array: Vec::new(),
        })
        .unwrap()
}

#[test]
fn key_map_retains_primitive_types_number_equality_and_subscription_tombstones() {
    let mut storage = HostStorage::default();
    storage.register_view(view(1, "source"));
    storage.register_view(view(2, "subscriber"));
    for key in [json!(1), json!("1"), json!(true)] {
        call(
            &mut storage,
            HostStoreKind::Key,
            2,
            "registerEvent",
            json!({"key":key}),
        );
        call(
            &mut storage,
            HostStoreKind::Key,
            1,
            "setItem",
            json!({"key":key,"value":key}),
        );
    }
    let replaced = call(
        &mut storage,
        HostStoreKind::Key,
        1,
        "setItem",
        json!({"key":1.0,"value":"replacement"}),
    );
    assert_eq!(replaced.value["result"], true);
    assert_eq!(
        call(&mut storage, HostStoreKind::Key, 1, "getKeys", json!({})).value,
        json!([1, "1", true])
    );
    assert_eq!(
        call(
            &mut storage,
            HostStoreKind::Key,
            1,
            "getItem",
            json!({"key":1})
        )
        .value,
        "replacement"
    );
    let events = storage.drain_events(2).unwrap();
    assert_eq!(events.len(), 4);
    assert_eq!(events[0].payload["key"], 1);
    // KeyStorage uses changed even on the first set; this is not URLStorage.
    assert_eq!(events[0].payload["keyState"], "changed");
    assert!(storage.drain_events(1).unwrap().is_empty());

    call(
        &mut storage,
        HostStoreKind::Key,
        1,
        "removeItem",
        json!({"key":1.0}),
    );
    assert!(
        !call(
            &mut storage,
            HostStoreKind::Key,
            1,
            "getItem",
            json!({"key":1})
        )
        .defined
    );
    assert_eq!(
        storage.drain_events(2).unwrap()[0].payload["keyState"],
        "removed"
    );
    call(
        &mut storage,
        HostStoreKind::Key,
        1,
        "setItem",
        json!({"key":1,"value":false}),
    );
    assert_eq!(storage.drain_events(2).unwrap().len(), 1);
    assert_eq!(
        call(
            &mut storage,
            HostStoreKind::Key,
            1,
            "getItem",
            json!({"key":1})
        )
        .value,
        false
    );
    assert_eq!(
        call(
            &mut storage,
            HostStoreKind::Key,
            1,
            "setItem",
            json!({"key":null,"value":1})
        )
        .value["reason"],
        "missing arg.payload key"
    );
}

#[test]
fn url_maps_preserve_null_keys_truthiness_old_values_and_clear_scope() {
    for (store, noun) in [
        (HostStoreKind::Memory, "Memory"),
        (HostStoreKind::Window, "Window"),
    ] {
        let mut storage = HostStorage::default();
        storage.register_view(view(1, "first"));
        storage.register_view(view(2, "second"));
        call(
            &mut storage,
            store,
            2,
            &format!("register{noun}StorageEvent"),
            json!({}),
        );
        let set = format!("set{noun}StorageItem");
        assert!(
            !call(
                &mut storage,
                store,
                1,
                &set,
                json!({"key":null,"value":false})
            )
            .defined
        );
        let event = storage.drain_events(2).unwrap().remove(0);
        assert_eq!(event.payload["key"], Value::Null);
        assert_eq!(event.payload["keyState"], "created");
        assert!(event.payload.get("oldValue").is_none());
        assert_eq!(
            call(
                &mut storage,
                store,
                1,
                &set,
                json!({"key":null,"value":null})
            )
            .value,
            false
        );
        assert_eq!(
            storage.drain_events(2).unwrap()[0].payload["oldValue"],
            false
        );
        let get = format!("get{noun}StorageItem");
        assert_eq!(
            call(&mut storage, store, 1, &get, json!({"key":null})).value,
            "[]"
        );
        call(&mut storage, store, 2, &set, json!({"key":null,"value":[]}));
        let rows: Value = serde_json::from_str(
            call(&mut storage, store, 1, &get, json!({"key":null}))
                .value
                .as_str()
                .unwrap(),
        )
        .unwrap();
        // JS [] is truthy, unlike false and null; sender URLs stay distinct.
        assert_eq!(rows, json!([{"value":[],"windowName":"second"}]));
        assert_eq!(
            call(
                &mut storage,
                store,
                1,
                &format!("clear{noun}Storage"),
                json!({})
            )
            .value,
            true
        );
        assert_eq!(
            call(
                &mut storage,
                store,
                1,
                &format!("get{noun}StorageKeys"),
                json!({})
            )
            .value,
            json!([null])
        );
        call(
            &mut storage,
            store,
            1,
            &format!("reset{noun}Storage"),
            json!({}),
        );
        assert_eq!(
            call(&mut storage, store, 1, &get, json!({"key":null})).value,
            "[]"
        );
    }
}

#[test]
fn memory_targets_keep_duplicate_matches_and_filter_view_lifecycle() {
    let mut storage = HostStorage::default();
    storage.register_view(view(1, "sender"));
    let mut live = view(2, "synapse/device");
    live.tracked_url = false;
    storage.register_view(live.clone());
    for id in 3..=6 {
        let mut inactive = live.clone();
        inactive.id = id;
        match id {
            3 => inactive.remote = true,
            4 => inactive.destroyed = true,
            5 => inactive.crashed = true,
            _ => inactive.disposed = true,
        }
        storage.register_view(inactive);
    }
    storage
        .call(HostStorageCall {
            store: HostStoreKind::Memory,
            sender_id: 1,
            action: "setMemoryStorageItem".into(),
            payload: json!({"key":"level","value":42,"url":"stored-under"}),
            target_url_array: vec!["synapse".into(), "device".into()],
        })
        .unwrap();
    // Original loops over matching targets, sending once per matching entry.
    assert_eq!(storage.drain_events(2).unwrap().len(), 2);
    for id in [1, 3, 4, 5, 6] {
        assert!(storage.drain_events(id).unwrap().is_empty());
    }
    call(
        &mut storage,
        HostStoreKind::Memory,
        1,
        "setMemoryStorageItemNoEvent",
        json!({"key":"silent","value":3,"url":"ignored"}),
    );
    assert!(storage.drain_events(2).unwrap().is_empty());
    let snapshot = call(
        &mut storage,
        HostStoreKind::Memory,
        1,
        "getMemoryStorage",
        json!({}),
    );
    assert_eq!(
        snapshot.value,
        json!([
            ["stored-under", [["level", 42]]],
            ["sender", [["silent", 3]]]
        ])
    );
}

#[test]
fn same_url_subscription_survives_close_without_retaining_closed_event_queue() {
    let mut storage = HostStorage::default();
    storage.register_view(view(1, "sender"));
    storage.register_view(view(2, "subscriber"));
    call(
        &mut storage,
        HostStoreKind::Key,
        2,
        "registerEvent",
        json!({"key":"state"}),
    );
    storage.close_view(2);
    assert!(storage.drain_events(2).is_err());
    storage.register_view(view(3, "subscriber"));
    call(
        &mut storage,
        HostStoreKind::Key,
        1,
        "setItem",
        json!({"key":"state","value":7}),
    );
    assert_eq!(storage.drain_events(3).unwrap().len(), 1);
    assert!(
        storage
            .call(HostStorageCall {
                store: HostStoreKind::Key,
                sender_id: 2,
                action: "getKeys".into(),
                payload: json!({}),
                target_url_array: vec![],
            })
            .is_err()
    );
}
