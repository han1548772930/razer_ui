use super::*;
struct Temp(std::path::PathBuf);
impl Temp {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "razer-receiver-led-{}-{}",
            std::process::id(),
            uuid().unwrap()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        Self(directory)
    }
    fn path(&self) -> std::path::PathBuf {
        self.0.join("local.json")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn default_profile() -> Value {
    json!({"guid":"","name":"Test-Default","indicatorLedConfiguration":[
        {"index":0,"ledStatus":2},{"index":1,"ledStatus":1},{"index":2,"ledStatus":3}],
        "mappings":[{"inputID":"LeftClick","unknown":true}],"unrecognized":{"preserved":true}})
}
#[test]
fn local_cache_uses_protocol_serial_owner_and_preserves_other_profiles_and_keys() {
    let temp = Temp::new();
    let path = temp.path();
    crate::source_local::set(&path, "other", &json!({"unchanged":true})).unwrap();
    let first = ensure_cache(&path, 229, "serial A", &default_profile())
        .unwrap()
        .unwrap();
    let a = first.profile["guid"].as_str().unwrap().to_owned();
    let second = ensure_cache(&path, 229, "serial B", &default_profile())
        .unwrap()
        .unwrap();
    let b = second.profile["guid"].as_str().unwrap().to_owned();
    assert_ne!(a, b);
    assert_eq!(second.document["profiles"].as_array().unwrap().len(), 2);
    assert_eq!(second.profile["name"], "Test-Default 1");
    assert_eq!(second.document["powerSavingValue"], 5);
    let retained = ensure_cache(&path, 229, "serial A", &json!({"ignored":true}))
        .unwrap()
        .unwrap();
    assert_eq!(retained.profile, first.profile);
    assert_eq!(retained.document["activeProfile"], a);
    assert_eq!(
        crate::source_local::get(&path, "other").unwrap(),
        Some(json!({"unchanged":true}))
    );
    assert!(active_profile(&path, 229, "").unwrap().is_none());
    assert!(
        ensure_cache(&path, 229, "", &default_profile())
            .unwrap()
            .is_none()
    );
}
#[test]
fn current_active_owner_is_reread_and_unrelated_profile_data_survives() {
    let temp = Temp::new();
    let path = temp.path();
    let mut first = ensure_cache(&path, 190, "serial", &default_profile())
        .unwrap()
        .unwrap();
    let original = first.profile.clone();
    let mut second = default_profile();
    second["guid"] = json!("second-profile");
    first.document["profiles"]
        .as_array_mut()
        .unwrap()
        .push(second);
    first.document["deviceMetadatas"]["serial"]["activeProfileGuid"] = json!("second-profile");
    // document.activeProfile deliberately still points to the old projection.
    crate::source_local::set(&path, "synapse_190", &first.document).unwrap();
    let requested = json!([{"index":3,"ledStatus":255}]);
    let receipt = save_initialization(&path, 190, "serial", &requested)
        .unwrap()
        .unwrap();
    assert_eq!(receipt.profile_guid, "second-profile");
    assert_eq!(receipt.version, 2);
    assert_eq!(receipt.document["profiles"][0], original);
    assert_eq!(receipt.document["profiles"][1][FIELD], requested);
    assert_eq!(
        receipt.document["profiles"][1]["unrecognized"],
        json!({"preserved":true})
    );
}
#[test]
fn initialization_missing_field_and_cache_are_skipped_without_local_success_fabrication() {
    let temp = Temp::new();
    let path = temp.path();
    assert!(
        save_initialization(&path, 239, "serial", &json!([]))
            .unwrap()
            .is_none()
    );
    let mut profile = default_profile();
    profile.as_object_mut().unwrap().remove(FIELD);
    ensure_cache(&path, 239, "serial", &profile).unwrap();
    let before = std::fs::read(&path).unwrap();
    assert!(
        save_initialization(&path, 239, "serial", &json!([]))
            .unwrap()
            .is_none()
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let receipt = save_user_selection(&path, 239, "serial", &json!([]))
        .unwrap()
        .unwrap();
    assert_eq!(receipt.document["profiles"][0][FIELD], json!([]));
}
#[test]
fn distinct_source_version_branches_and_wrap_are_preserved() {
    for (initialization, value, expected) in [
        (false, None, 1),
        (true, None, 2),
        (false, Some(json!(null)), 1),
        (true, Some(json!(null)), 2),
        (false, Some(json!(0)), 1),
        (true, Some(json!(1)), 2),
        (false, Some(json!(MAX_VERSION)), 0),
    ] {
        assert_eq!(
            next_version(value.as_ref(), initialization).unwrap(),
            expected
        );
    }
    assert!(next_version(Some(&json!(MAX_VERSION + 1)), false).is_err());
}
#[test]
fn malformed_document_or_configuration_never_replaces_existing_bytes() {
    let temp = Temp::new();
    let path = temp.path();
    crate::source_local::set(
        &path,
        "synapse_190",
        &json!({"productId":229,"profiles":[]}),
    )
    .unwrap();
    let before = std::fs::read(&path).unwrap();
    assert!(ensure_cache(&path, 190, "serial", &default_profile()).is_err());
    assert!(save_user_selection(&path, 190, "serial", &json!([])).is_err());
    assert!(save_user_selection(&path, 190, "serial", &json!({})).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
#[test]
fn local_ui_guid_is_retained_and_profile_binding_updates_the_serial_projection() {
    let temp = Temp::new();
    let path = temp.path();
    let mut profile = default_profile();
    profile["guid"] = json!("existing-ui-profile");
    let original = ensure_cache(&path, 190, "serial", &profile)
        .unwrap()
        .unwrap();
    assert_eq!(original.profile["guid"], "existing-ui-profile");
    let bound = bind_local_profile(
        &path,
        190,
        "serial",
        &json!({"guid":"second-ui-profile","name":"Other local profile",FIELD:[]}),
    )
    .unwrap();
    assert_eq!(bound.profile["guid"], "second-ui-profile");
    assert_eq!(
        bound.document["deviceMetadatas"]["serial"]["activeProfileGuid"],
        "second-ui-profile"
    );
    assert_eq!(bound.document["activeProfile"], "second-ui-profile");
    assert_eq!(bound.document["profiles"][0], original.profile);
    let updated = bind_local_profile(
        &path,
        190,
        "serial",
        &json!({"guid":"existing-ui-profile","name":"Renamed"}),
    )
    .unwrap();
    assert_eq!(updated.profile["unrecognized"], json!({"preserved":true}));
    assert_eq!(updated.profile["mappings"], original.profile["mappings"]);
    let before = std::fs::read(&path).unwrap();
    bind_local_profile(
        &path,
        190,
        "serial",
        &json!({"guid":"existing-ui-profile","name":"Renamed"}),
    )
    .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
