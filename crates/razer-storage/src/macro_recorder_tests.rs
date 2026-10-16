use super::*;

#[test]
fn otfm_pretrigger_object_is_distinct_from_background_exclusive_string() {
    let trigger = otfm_trigger_value(756, "container");
    assert_eq!(trigger["deviceId"], "usb_5426_756_container");
    assert_eq!(trigger["macro_recorder_mode"], "kOTFM");
    assert!(trigger.get("mode").is_none());
    let exclusive: Value = serde_json::from_str(&otfm_exclusive_value()).unwrap();
    assert_eq!(exclusive["payload"]["macro_recorder_mode"], "kOTFM");
    assert!(exclusive.get("macro_recorder_mode").is_none());
    let mut lock = RecorderLock::default();
    let notification = lock.event(&json!({"key":STORAGE_KEY,"newValue":otfm_exclusive_value()}), None).unwrap();
    assert_eq!(notification, Some(RecorderNotification::Started(RecorderMode::Otfm)));
    assert!(lock.is_exclusive());
}
use crate::host::{HostStorage, HostStorageCall, HostStorageView, HostStoreKind};

fn call(storage: &mut HostStorage, sender: u64, action: &str, value: Option<Value>) -> Value {
    let mut payload = json!({"key":STORAGE_KEY});
    if let Some(value) = value { payload["value"] = value; }
    storage.call(HostStorageCall { store: HostStoreKind::Window, sender_id: sender,
        action: action.into(), payload, target_url_array: vec![] }).unwrap().value
}
fn setup() -> HostStorage {
    let mut storage=HostStorage::default();
    for id in 1..=3 {
        storage.register_view(HostStorageView { id, url:format!("local-{id}"), remote:false,
            destroyed:false,crashed:false,disposed:false,tracked_url:true });
    }
    call(&mut storage,3,"registerWindowStorageEvent",None);
    storage
}

#[test]
fn removal_keeps_other_exclusive_owner_then_emits_actual_old_mode() {
    let mut storage=setup();let mut lock=RecorderLock::default();
    call(&mut storage,1,"setWindowStorageItem",Some(json!(software_exclusive_value())));
    call(&mut storage,2,"setWindowStorageItem",Some(json!(json!({"mode":"exclusive","payload":{"macro_recorder_mode":"kOTFM"}}).to_string())));
    let aggregate=call(&mut storage,3,"getWindowStorageItem",None);
    let events=storage.drain_events(3).unwrap();
    assert_eq!(lock.event(&events[0].payload,aggregate.as_str()).unwrap(),Some(RecorderNotification::Started(RecorderMode::Software)));
    assert_eq!(lock.event(&events[1].payload,aggregate.as_str()).unwrap(),Some(RecorderNotification::Started(RecorderMode::Otfm)));
    call(&mut storage,1,"removeWindowStorageItem",None);
    let aggregate=call(&mut storage,3,"getWindowStorageItem",None);
    let event=storage.drain_events(3).unwrap().remove(0);
    assert_eq!(lock.event(&event.payload,aggregate.as_str()).unwrap(),None);
    assert!(lock.is_exclusive());
    call(&mut storage,2,"removeWindowStorageItem",None);
    let aggregate=call(&mut storage,3,"getWindowStorageItem",None);
    let event=storage.drain_events(3).unwrap().remove(0);
    assert_eq!(lock.event(&event.payload,aggregate.as_str()).unwrap(),Some(RecorderNotification::Stopped(RecorderMode::Otfm)));
    assert!(!lock.is_exclusive());
}

#[test]
fn initial_mode_uses_source_top_level_quirk_and_short_circuits() {
    let mut lock=RecorderLock::default();
    let rows=json!([{"value":software_exclusive_value()},{"value":"broken"}]).to_string();
    assert_eq!(lock.initialize(Some(&rows)),None);
    assert!(lock.is_exclusive());
    let rows=json!([{"value":json!({"mode":"exclusive","macro_recorder_mode":"kSoftware"}).to_string()}]).to_string();
    assert_eq!(lock.initialize(Some(&rows)),Some(RecorderNotification::Started(RecorderMode::Software)));
    assert_eq!(lock.initialize(Some("broken")),None);
    assert!(!lock.is_exclusive());
}

#[test]
fn otfm_pretrigger_has_no_exclusive_mode_and_changed_does_not_clear_hold() {
    let mut lock=RecorderLock::default();
    let pretrigger=json!({"key":STORAGE_KEY,"keyState":"created","newValue":json!({"macro_recorder_mode":"kOTFM","deviceId":"usb_5426_756_container","containerId":"container"}).to_string()});
    assert_eq!(lock.event(&pretrigger,None).unwrap(),None);assert!(!lock.is_exclusive());
    let started=json!({"key":STORAGE_KEY,"newValue":software_exclusive_value()});
    lock.event(&started,None).unwrap();assert!(lock.is_exclusive());
    let changed=json!({"key":STORAGE_KEY,"keyState":"changed","oldValue":software_exclusive_value(),"newValue":"{}"});
    lock.event(&changed,None).unwrap();assert!(lock.is_exclusive());
    let removed=json!({"key":STORAGE_KEY,"keyState":"removed","oldValue":software_exclusive_value()});
    assert_eq!(lock.event(&removed,Some("broken")).unwrap(),Some(RecorderNotification::Stopped(RecorderMode::Software)));
    assert!(!lock.is_exclusive());
}
