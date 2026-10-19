use super::*;

#[test]
fn imported_names_respect_utf16_limits_and_case_insensitive_identity() {
    let mut device = device();
    let name = "界".repeat(32);
    device.profiles[0].name = name.clone();
    let resolved = import_name(&device.profiles, &name);
    assert!(resolved.encode_utf16().count() <= 32);
    assert!(resolved.ends_with(" (1)"));
    device.profiles[0].name = "Work".into();
    assert_eq!(import_name(&device.profiles, "work"), "work (1)");
}

#[test]
fn profile_context_rejects_changed_device_profile_layout_and_connection() {
    let device = device();
    let context = ProfileContext::from_device(&device);
    assert!(context.matches(&device));
    let mut changed = device.clone();
    changed.device_container_id.push_str("replacement");
    assert!(!context.matches(&changed));
    changed = device.clone();
    changed.serial_number.push_str("replacement");
    assert!(!context.matches(&changed));
    changed = device.clone();
    changed.active_profile.push_str("replacement");
    assert!(!context.matches(&changed));
    changed = device.clone();
    changed.layout_id += 1;
    assert!(!context.matches(&changed));
    changed = device.clone();
    changed.use_ble = !changed.use_ble;
    assert!(!context.matches(&changed));
    changed = device.clone();
    changed.real_product_id += 1;
    assert!(!context.matches(&changed));
    changed = device.clone();
    changed.dashboard.connection_observation =
        Some(crate::model::DeviceConnectionObservation::ReceiverPeer(9));
    assert!(!context.matches(&changed));
}

#[test]
fn source_payload_selection_preserves_order_and_extensions_without_mutating_the_owner() {
    let mut device = device();
    let mut second = device.profiles[0].clone();
    second.id = "second-id".into();
    second.guid = "second-guid".into();
    second.name = "Second".into();
    second.source_settings =
        Some(serde_json::json!({"name":"stale", "guid":"stale", "extension":{"keep":42}}));
    device.profiles[0].source_settings = Some(serde_json::json!({"mappings":[]}));
    let first = device.profiles[0].id.clone();
    device.profiles.push(second);
    let before = serde_json::to_value(&device).unwrap();
    let selected = source_profile_payloads(&device, &["second-id".into(), first]).unwrap();
    assert_eq!(selected[0]["guid"], "second-guid");
    assert_eq!(selected[0]["name"], "Second");
    assert_eq!(selected[0]["extension"]["keep"], 42);
    assert_eq!(selected[1]["name"], "Work");
    assert!(selected[1]["mappings"].is_array());
    assert_eq!(serde_json::to_value(&device).unwrap(), before);
}

#[test]
fn invalid_source_selection_never_fabricates_export_payloads() {
    let mut device = device();
    let id = device.profiles[0].id.clone();
    assert!(source_profile_payloads(&device, &[]).is_err());
    assert!(source_profile_payloads(&device, &["removed".into()]).is_err());
    assert!(source_profile_payloads(&device, &[id.clone(), id.clone()]).is_err());
    for source in [
        None,
        Some(Value::Null),
        Some(serde_json::json!([])),
        Some(serde_json::json!(false)),
    ] {
        device.profiles[0].source_settings = source;
        assert!(source_profile_payloads(&device, &[id.clone()]).is_err());
    }
}

#[test]
fn export_preserves_profile_order_and_the_union_of_each_rows_selected_macros() {
    let mut device = device();
    device.profiles[0].source_settings = Some(serde_json::json!({"mappings":[],"opaque":42}));
    let id = device.profiles[0].id.clone();
    let selection = [SourceExportSelection {
        profile_id: id,
        macro_guids: vec![
            "second".into(),
            "missing".into(),
            "second".into(),
            "first".into(),
        ],
    }];
    let macros = [
        serde_json::json!({"guid":"first","name":"A"}),
        serde_json::json!({"guid":"second","name":"B"}),
    ];
    let (profiles, exported) = source_export_payloads(
        &device,
        &selection,
        &macros,
        razer_device::profile_transfer::ProfileExportRule::ProfilesWithSelectedMacros,
    )
    .unwrap();
    assert_eq!(profiles[0]["opaque"], 42);
    assert_eq!(exported, [macros[1].clone(), macros[0].clone()]);
    assert_eq!(
        device.profiles[0].source_settings.as_ref().unwrap()["opaque"],
        42
    );
}

#[test]
fn export_rules_reject_required_missing_macros_and_ignore_them_for_profiles_only() {
    use razer_device::profile_transfer::ProfileExportRule;
    let mut device = device();
    device.profiles[0].source_settings = Some(serde_json::json!({"mappings":[]}));
    let mut selection = [SourceExportSelection {
        profile_id: device.profiles[0].id.clone(),
        macro_guids: vec!["existing".into(), "missing".into()],
    }];
    let macros = [serde_json::json!({"guid":"existing","name":"Selected"})];
    assert!(
        source_export_payloads(
            &device,
            &selection,
            &macros,
            ProfileExportRule::ProfilesWithRequiredSelectedMacros,
        )
        .is_err()
    );
    let (profiles, exported) = source_export_payloads(
        &device,
        &selection,
        &macros,
        ProfileExportRule::ProfilesOnly,
    )
    .unwrap();
    assert_eq!(profiles.len(), 1);
    assert!(exported.is_empty());
    selection[0].macro_guids = vec!["existing".into(), "existing".into()];
    let (_, exported) = source_export_payloads(
        &device,
        &selection,
        &macros,
        ProfileExportRule::ProfilesWithRequiredSelectedMacros,
    )
    .unwrap();
    assert_eq!(exported, macros);
}

fn import_transaction(original: &Device) -> SourceProfileImport {
    let mut candidate = original.clone();
    let mut appended = candidate.profiles[0].clone();
    appended.id = "fresh-profile".into();
    appended.guid = "fresh-profile".into();
    appended.name = "Imported".into();
    appended.source_settings = Some(serde_json::json!({"guid":"fresh-profile", "name":"Imported"}));
    candidate.profiles.push(appended);
    candidate.source_device_settings =
        Some(serde_json::json!({"version":2,"extension":{"keep":42}}));
    SourceProfileImport::new(original, candidate).unwrap()
}

#[test]
fn import_receipt_merges_only_its_captured_append_without_activating_a_profile() {
    let mut device = device();
    let original = device.clone();
    let request = import_transaction(&device);
    assert!(request.is_same(&request.clone()));
    assert!(!request.is_same(&import_transaction(&device)));
    device
        .product_name
        .values
        .insert("en".into(), "localized".into());
    device = request.merge_receipt(&device).unwrap();
    assert_eq!(device.active_profile, original.active_profile);
    assert_eq!(device.profiles.len(), original.profiles.len() + 1);
    assert!(device.profiles.starts_with(&original.profiles));
    assert_eq!(device.product_name.get("en"), "localized");
    assert!(request.merge_receipt(&device).is_err());
}

#[test]
fn import_admission_rejects_changed_owner_connection_and_unsaved_edits() {
    let original = device();
    let request = import_transaction(&original);
    for change in 0..7 {
        let mut current = original.clone();
        match change {
            0 => current.serial_number.push_str("replaced"),
            1 => current.device_container_id.push_str("replaced"),
            2 => current.use_ble = !current.use_ble,
            3 => current.layout_id += 1,
            4 => current.profiles[0].name.push_str("edited"),
            5 => current.active_profile = "switched".into(),
            6 => current.source_device_settings = Some(serde_json::json!({"edited":true})),
            _ => unreachable!(),
        }
        let before = serde_json::to_value(&current).unwrap();
        assert!(!request.matches(&current), "case {change}");
        assert_eq!(serde_json::to_value(&current).unwrap(), before);
    }
    let mut replacement = original.clone();
    replacement.profiles[0].name.push_str("changed");
    assert!(SourceProfileImport::new(&original, replacement).is_err());
}

fn macro_document(guid: &str, name: &str) -> Value {
    serde_json::json!({
        "guid":guid,
        "name":name,
        "macroEvents":[],
        "macroPlaybackOption":"standard",
        "appEngine":{"events":[],"hash":"source-engine","mouseMoveType":"none"},
        "localExtension":{"preserved":42}
    })
}

fn macro_library() -> MacroLibraryFile {
    let mut library = MacroLibraryFile::default();
    library
        .append_source_documents(&[macro_document(
            "3067b9df-40a4-4bc3-a9ea-2380195fe948",
            "Existing",
        )])
        .unwrap();
    library.next_id = 42;
    library.current = Some(1);
    library.tutorial = crate::macro_library::Tutorial::Complete;
    library.entries[0].open = true;
    library
}

#[test]
fn profile_macro_import_retains_the_complete_typed_document_until_a_storage_receipt() {
    let original = device();
    let library = macro_library();
    let document = macro_document("b18aa9cc-ab4c-4fe4-8c70-3e46ee584b96", "Imported");
    let profile_request = import_transaction(&original);
    assert!(profile_request.prepare_macros(&library).unwrap().is_none());
    let request = profile_request
        .clone()
        .with_source_macros(&library.mapping_catalog(), vec![document.clone()]);
    assert!(!request.is_same(&profile_request));
    let before = library.clone();
    let macros = request.prepare_macros(&library).unwrap().unwrap();
    assert_eq!(library, before);
    assert!(request.matches(&original));
    assert!(macros.matches(&library));
    let candidate = macros.candidate();
    assert_eq!(candidate.next_id, 43);
    assert_eq!(candidate.current, library.current);
    assert_eq!(candidate.tutorial, library.tutorial);
    assert_eq!(candidate.entries[0], library.entries[0]);
    assert_eq!(candidate.entries[1].id, 42);
    assert_eq!(candidate.entries[1].source_document, Some(document.clone()));
    let loaded: MacroLibraryFile =
        serde_json::from_slice(&serde_json::to_vec(candidate).unwrap()).unwrap();
    assert!(loaded.same_document(candidate));
    assert_eq!(loaded.source_macro_documents().unwrap()[1], document);
    let published = macros.merge_receipt(&library).unwrap();
    assert_eq!(published, *candidate);
    assert!(macros.merge_receipt(&published).is_err());
    let retained = import_transaction(&original)
        .with_source_macros(&before.mapping_catalog(), Vec::new())
        .prepare_macros(&before)
        .unwrap()
        .unwrap();
    assert_eq!(retained.candidate(), &before);
    let mut changed = before;
    changed.entries[0].name.push_str(" edited");
    assert!(!retained.matches(&changed));
}

#[test]
fn profile_macro_preparation_rejects_catalog_changes_and_invalid_documents_without_partial_edits() {
    let original = device();
    let library = macro_library();
    let catalog = library.mapping_catalog();
    let document = macro_document("b18aa9cc-ab4c-4fe4-8c70-3e46ee584b96", "Imported");
    let request =
        import_transaction(&original).with_source_macros(&catalog, vec![document.clone()]);
    let mut changed = library.clone();
    changed.entries[0].name = "Edited while selecting the file".into();
    let changed_before = changed.clone();
    assert!(request.prepare_macros(&changed).is_err());
    assert_eq!(changed, changed_before);
    assert!(
        import_transaction(&original)
            .with_source_macros(&catalog, vec![])
            .prepare_macros(&changed)
            .is_err()
    );
    for documents in [
        vec![document.clone(), document.clone()],
        vec![document.clone(), macro_document("invalid-guid", "Invalid")],
        vec![library.mapping_catalog()[0].clone()],
    ] {
        let before = library.clone();
        let request = import_transaction(&original).with_source_macros(&catalog, documents);
        assert!(request.prepare_macros(&library).is_err());
        assert_eq!(library, before);
        assert!(request.matches(&original));
    }
}

#[test]
fn profile_macro_admission_ignores_expansion_and_receipts_preserve_later_edits() {
    let library = macro_library();
    let request = import_transaction(&device()).with_source_macros(
        &library.mapping_catalog(),
        vec![macro_document(
            "b18aa9cc-ab4c-4fe4-8c70-3e46ee584b96",
            "Imported",
        )],
    );
    let import = request.prepare_macros(&library).unwrap().unwrap();
    let mut expanded = library.clone();
    expanded.entries[0].open = !expanded.entries[0].open;
    assert!(import.matches(&expanded));
    for change in 0..6 {
        let mut current = library.clone();
        match change {
            0 => current.next_id += 1,
            1 => current.current = None,
            2 => current.tutorial = crate::macro_library::Tutorial::Initial,
            3 => current.entries[0].name.push_str(" edited"),
            4 => current.entries[0].record_delay = 1,
            5 => {
                current.entries[0].source_document.as_mut().unwrap()["localExtension"] =
                    serde_json::json!({"edited":true});
            }
            _ => unreachable!(),
        }
        let before = current.clone();
        assert!(!import.matches(&current), "case {change}");
        let merged = import.merge_receipt(&current).unwrap();
        assert!(
            merged.entries.starts_with(&current.entries),
            "case {change}"
        );
        assert_eq!(current, before, "case {change}");
    }
}

#[test]
fn import_receipt_merge_preserves_profile_edits_deletions_and_current_selection() {
    let mut original = device();
    original.source_device_settings = Some(serde_json::json!({
        "version":7,"calibration":{"value":1},"extension":{"keep":42}
    }));
    let mut second = original.profiles[0].clone();
    second.id = "second".into();
    second.guid = "second".into();
    second.name = "Second".into();
    original.profiles.push(second);
    let mut candidate = import_transaction(&original).candidate().clone();
    candidate.source_device_settings = Some(serde_json::json!({
        "version":8,"calibration":{"value":1},"extension":{"keep":42}
    }));
    let request = SourceProfileImport::new(&original, candidate).unwrap();
    let mut current = original.clone();
    current.profiles.remove(0);
    current.profiles[0].name = "Edited second".into();
    current.active_profile = "second".into();
    current.source_device_settings.as_mut().unwrap()["calibration"]["value"] = 9.into();
    let before = serde_json::to_value(&current).unwrap();
    let merged = request.merge_receipt(&current).unwrap();
    assert_eq!(merged.active_profile, "second");
    assert_eq!(merged.profiles.len(), 2);
    assert_eq!(merged.profiles[0], current.profiles[0]);
    assert_eq!(merged.profiles[1].guid, "fresh-profile");
    assert_eq!(
        merged.source_device_settings,
        Some(serde_json::json!({
            "version":8,"calibration":{"value":9},"extension":{"keep":42}
        }))
    );
    assert_eq!(serde_json::to_value(&current).unwrap(), before);
    assert_eq!(request.candidate().profiles.len(), 3);
    assert_eq!(request.candidate().active_profile, original.active_profile);
}

fn projected_import() -> (Device, SourceProfileImport) {
    let mut original = device();
    original.product_id = 70;
    original.profiles[0].id = "first".into();
    original.profiles[0].guid = "first".into();
    original.active_profile = "first".into();
    let mut second = original.profiles[0].clone();
    second.id = "second".into();
    second.guid = "second".into();
    second.name = "Second".into();
    original.profiles.push(second);
    original.source_device_settings = Some(serde_json::json!({
        "calibration":{"value":3},
        "_mouse70_mapping_document":{
            "version":7,"activeProfile":"first",
            "profiles":[
                {"guid":"first","name":"Work","appEngine":{"hash":"old-first"}},
                {"guid":"second","name":"Second","appEngine":{"hash":"old-second"}}
            ],
            "defaultMappings":{"appEngine":{"hash":"defaults"}},
            "defaultTurbos":[{"guid":"default-turbo","delay":1}],
            "synapseTurbos":[{"guid":"old-turbo","delay":2}],
            "macros":[{"guid":"published-macro","hash":"previous-publication"}],
            "extension":{"keep":42}
        }
    }));
    let mut candidate = import_transaction(&original).candidate().clone();
    candidate.source_device_settings = Some(serde_json::json!({
        "calibration":{"value":3},
        "_mouse70_mapping_document":{
            "version":8,"activeProfile":"first",
            "profiles":[
                {"guid":"first","name":"Work","appEngine":{"hash":"old-first"}},
                {"guid":"second","name":"Second","appEngine":{"hash":"old-second"}},
                {"guid":"fresh-profile","name":"Imported","appEngine":{"hash":"imported"}}
            ],
            "defaultMappings":{"appEngine":{"hash":"defaults"}},
            "defaultTurbos":[{"guid":"default-turbo","delay":1}],
            "synapseTurbos":[{"guid":"old-turbo","delay":2},{"guid":"new-turbo","delay":4}],
            "macros":[{"guid":"published-macro","hash":"previous-publication"}],
            "extension":{"keep":42}
        }
    }));
    let request = SourceProfileImport::new(&original, candidate).unwrap();
    (original, request)
}

#[test]
fn import_receipt_merge_retains_nested_projection_edits_turbos_and_published_macros() {
    let (original, request) = projected_import();
    let mut current = original;
    current.active_profile = "second".into();
    current.profiles[0].name = "Edited work".into();
    let root = current.source_device_settings.as_mut().unwrap();
    root["calibration"]["value"] = 9.into();
    let document = &mut root["_mouse70_mapping_document"];
    document["activeProfile"] = "second".into();
    document["profiles"][0]["appEngine"]["hash"] = "edited-first".into();
    document["synapseTurbos"][0]["delay"] = 11.into();
    document["extension"]["new"] = true.into();
    let merged = request.merge_receipt(&current).unwrap();
    assert_eq!(merged.active_profile, "second");
    assert_eq!(merged.profiles[0].name, "Edited work");
    assert_eq!(merged.profiles[2].guid, "fresh-profile");
    let root = merged.source_device_settings.unwrap();
    let document = &root["_mouse70_mapping_document"];
    assert_eq!(root["calibration"]["value"], 9);
    assert_eq!(document["version"], 8);
    assert_eq!(document["activeProfile"], "second");
    assert_eq!(
        document["profiles"],
        serde_json::json!([
            {"guid":"first","name":"Work","appEngine":{"hash":"edited-first"}},
            {"guid":"second","name":"Second","appEngine":{"hash":"old-second"}},
            {"guid":"fresh-profile","name":"Imported","appEngine":{"hash":"imported"}}
        ])
    );
    assert_eq!(
        document["synapseTurbos"],
        serde_json::json!([
            {"guid":"old-turbo","delay":11},{"guid":"new-turbo","delay":4}
        ])
    );
    assert_eq!(
        document["defaultTurbos"],
        serde_json::json!([{"guid":"default-turbo","delay":1}])
    );
    assert_eq!(
        document["macros"],
        serde_json::json!([
            {"guid":"published-macro","hash":"previous-publication"}
        ])
    );
    assert_eq!(
        document["extension"],
        serde_json::json!({"keep":42,"new":true})
    );
    assert_eq!(
        request.candidate().source_device_settings.as_ref().unwrap()["_mouse70_mapping_document"]["activeProfile"],
        "first"
    );
}

#[test]
fn import_receipt_merge_does_not_resurrect_deleted_nested_profiles() {
    let (original, request) = projected_import();
    let mut current = original;
    current.profiles.remove(0);
    current.active_profile = "second".into();
    let document =
        &mut current.source_device_settings.as_mut().unwrap()["_mouse70_mapping_document"];
    document["profiles"].as_array_mut().unwrap().remove(0);
    document["activeProfile"] = "second".into();
    let merged = request.merge_receipt(&current).unwrap();
    assert_eq!(
        merged
            .profiles
            .iter()
            .map(|profile| profile.guid.as_str())
            .collect::<Vec<_>>(),
        ["second", "fresh-profile"]
    );
    let document = &merged.source_device_settings.as_ref().unwrap()["_mouse70_mapping_document"];
    assert_eq!(
        document["profiles"]
            .as_array()
            .unwrap()
            .iter()
            .map(|profile| profile["guid"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["second", "fresh-profile"]
    );
}

#[test]
fn import_receipt_merge_uses_current_selection_when_creating_a_projection() {
    let (mut original, request) = projected_import();
    original
        .source_device_settings
        .as_mut()
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("_mouse70_mapping_document");
    let mut candidate = request.candidate().clone();
    candidate.source_device_settings.as_mut().unwrap()["_mouse70_mapping_document"]["version"] =
        1.into();
    let request = SourceProfileImport::new(&original, candidate).unwrap();
    let mut current = original;
    current.active_profile = "second".into();
    let merged = request.merge_receipt(&current).unwrap();
    assert_eq!(merged.active_profile, "second");
    assert_eq!(
        merged.source_device_settings.as_ref().unwrap()["_mouse70_mapping_document"]["activeProfile"],
        "second"
    );
}

#[test]
fn import_receipt_merge_requires_the_exact_active_profile_identity() {
    let (original, request) = projected_import();
    let mut current = original;
    current.profiles.remove(0);
    current.active_profile = "second".into();
    let merged = request.merge_receipt(&current).unwrap();
    assert_eq!(merged.active_profile, "second");
    assert_eq!(
        merged.source_device_settings.as_ref().unwrap()["_mouse70_mapping_document"]["activeProfile"],
        "second"
    );
    current.active_profile = "first".into();
    let before = serde_json::to_value(&current).unwrap();
    assert!(
        request
            .merge_receipt(&current)
            .unwrap_err()
            .contains("selection is absent")
    );
    assert_eq!(serde_json::to_value(&current).unwrap(), before);
}

#[test]
fn import_receipt_merge_rejects_true_field_guid_and_owner_conflicts_atomically() {
    let (original, request) = projected_import();
    for change in 0..6 {
        let mut current = original.clone();
        match change {
            0 => current.profiles.push(request.candidate().profiles.last().unwrap().clone()),
            1 => current.serial_number.push_str("replacement"),
            2 => current.dashboard.connection_observation = Some(crate::model::DeviceConnectionObservation::ReceiverPeer(9)),
            3 => current.source_device_settings.as_mut().unwrap()["_mouse70_mapping_document"]["version"] = 19.into(),
            4 => current.source_device_settings.as_mut().unwrap()["_mouse70_mapping_document"]["synapseTurbos"].as_array_mut().unwrap().push(serde_json::json!({"guid":"new-turbo","delay":99})),
            5 => current.source_device_settings = None,
            _ => unreachable!(),
        }
        let before = serde_json::to_value(&current).unwrap();
        assert!(request.merge_receipt(&current).is_err(), "case {change}");
        assert_eq!(
            serde_json::to_value(&current).unwrap(),
            before,
            "case {change}"
        );
    }
}

#[test]
fn profile_macro_receipt_merge_retains_io_edits_and_allocates_import_after_new_entries() {
    let baseline = macro_library();
    let mut document = macro_document("b18aa9cc-ab4c-4fe4-8c70-3e46ee584b96", "Imported X");
    document["macroEvents"] = serde_json::json!([
        {"Type":7,"guid":"3067b9df-40a4-4bc3-a9ea-2380195fe948"}
    ]);
    let request = import_transaction(&device())
        .with_source_macros(&baseline.mapping_catalog(), vec![document.clone()]);
    let import = request.prepare_macros(&baseline).unwrap().unwrap();
    let mut current = baseline.clone();
    current.entries[0].name = "Edited Y".into();
    current.entries[0].record_delay = 2;
    current.entries[0].open = false;
    current
        .append_source_documents(&[macro_document(
            "581af019-c9c7-492d-9fb5-19127e5a0d6f",
            "New Z",
        )])
        .unwrap();
    current.current = Some(42);
    let before = current.clone();
    let merged = import.merge_receipt(&current).unwrap();
    assert_eq!(current, before);
    assert_eq!(merged.entries.len(), 3);
    assert_eq!(merged.entries[0], current.entries[0]);
    assert_eq!(merged.entries[1], current.entries[1]);
    assert_eq!(merged.entries[2].id, 43);
    assert_eq!(merged.entries[2].source_document.as_ref(), Some(&document));
    assert_eq!(merged.entries[2].actions[0].macro_id, None);
    assert_eq!(
        merged.entries[2].actions[0].xml_macro_guid.as_deref(),
        Some("3067b9df-40a4-4bc3-a9ea-2380195fe948")
    );
    assert_eq!(
        merged.source_macro_documents().unwrap()[2]["macroEvents"][0]["guid"],
        "3067b9df-40a4-4bc3-a9ea-2380195fe948"
    );
    assert_eq!(merged.next_id, 44);
    assert_eq!(merged.current, Some(42));
    assert_eq!(merged.tutorial, baseline.tutorial);
    assert_eq!(import.candidate().entries[1].id, 42);
    assert_eq!(import.candidate().entries[0].name, "Existing");
    assert_eq!(
        merged
            .mapping_catalog()
            .iter()
            .map(|document| document["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["Edited Y", "New Z", "Imported X"]
    );
    assert!(!merged.same_document(import.candidate()));
}

#[test]
fn profile_macro_receipt_merge_keeps_clean_import_clean_and_rejects_guid_collision() {
    let baseline = macro_library();
    let document = macro_document("b18aa9cc-ab4c-4fe4-8c70-3e46ee584b96", "Imported X");
    let request = import_transaction(&device())
        .with_source_macros(&baseline.mapping_catalog(), vec![document.clone()]);
    let import = request.prepare_macros(&baseline).unwrap().unwrap();
    let mut current = baseline;
    current.entries[0].open = false;
    let merged = import.merge_receipt(&current).unwrap();
    assert!(merged.same_document(import.candidate()));
    assert!(!merged.entries[0].open);
    current.append_source_documents(&[document]).unwrap();
    current.entries.last_mut().unwrap().name = "Concurrent conflicting macro".into();
    let before = current.clone();
    assert!(import.merge_receipt(&current).is_err());
    assert_eq!(current, before);
}

#[test]
fn profile_macro_receipt_merge_preserves_existing_macro_deletion() {
    let baseline = macro_library();
    let request = import_transaction(&device()).with_source_macros(
        &baseline.mapping_catalog(),
        vec![macro_document(
            "b18aa9cc-ab4c-4fe4-8c70-3e46ee584b96",
            "Imported X",
        )],
    );
    let import = request.prepare_macros(&baseline).unwrap().unwrap();
    let mut current = baseline;
    current.entries.clear();
    current.current = None;
    let merged = import.merge_receipt(&current).unwrap();
    assert_eq!(merged.entries.len(), 1);
    assert_eq!(merged.entries[0].name, "Imported X");
    assert_eq!(merged.entries[0].id, 42);
    assert_eq!(merged.next_id, 43);
    assert_eq!(merged.current, None);
    assert!(!merged.same_document(import.candidate()));
}

fn device() -> crate::model::Device {
    let mut device = crate::test_devices::with_profile(653);
    device.layout_id = 1;
    device.profiles[0].name = "Work".into();
    let mut settings = ProfileSettings::default();
    settings.normalize(device.product_id);
    settings
        .bindings
        .insert("KEY_A".into(), "keyboard:Ctrl+C".into());
    settings
        .hypershift_bindings
        .insert("KEY_B".into(), "future:opaque".into());
    settings.linked_games.push(LinkedGame {
        name: "Game".into(),
        executable: "C:\\Games\\Game.exe".into(),
    });
    settings
        .keyboard
        .set_snap_pair(1, ["KEY_Q".into(), "KEY_E".into()]);
    let custom = settings.keyboard.add_dial().unwrap();
    settings.keyboard.rename_dial(&custom, "自定义拨轮");
    settings.keyboard.select_dial(&custom);
    settings
        .keyboard
        .dial_modes
        .iter_mut()
        .find(|mode| mode.uid == custom)
        .unwrap()
        .mappings
        .insert("ScrollRight".into(), "keyboard:Ctrl+C".into());
    device.profiles[0].settings = Some(settings);
    device
}

#[::core::prelude::v1::test]
fn local_profile_roundtrip_keeps_all_settings_and_regenerates_identity() {
    let device = device();
    let original = &device.profiles[0];
    let bytes = encode_profile(&device, original).unwrap();
    let imported = decode_profile(&bytes, device.product_id, device.layout_id).unwrap();
    assert_eq!(imported.name, original.name);
    assert_eq!(imported.settings, original.settings);
    assert!(imported.id.is_empty());
    assert!(imported.guid.is_empty());
    assert!(imported.dpi_stages.is_none());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["format"],
        "razer-ui-profile"
    );
}

#[::core::prelude::v1::test]
fn profile_transfer_keeps_a_dial_disabled_by_alt_tab_without_changing_its_identity() {
    let mut device = device();
    {
        let keyboard = &mut device.profiles[0].settings.as_mut().unwrap().keyboard;
        keyboard.select_dial("SWITCH_APPLICATIONS");
        for uid in ["KEYBOARD_BRIGHTNESS", "WINDOWS_ZOOM", "TRACK_JOGGING"] {
            keyboard.enable_dial(uid, false);
        }
    }
    for disabled in [true, false] {
        device.profiles[0]
            .settings
            .as_mut()
            .unwrap()
            .keyboard
            .set_disable_alt_tab(disabled);
        let bytes = encode_profile(&device, &device.profiles[0]).unwrap();
        let imported = decode_profile(&bytes, device.product_id, device.layout_id).unwrap();
        assert_eq!(imported.settings, device.profiles[0].settings);
        let keyboard = &imported.settings.as_ref().unwrap().keyboard;
        assert_eq!(keyboard.dial_active, "SWITCH_APPLICATIONS");
        assert!(
            keyboard
                .dial_modes
                .iter()
                .filter(|mode| !mode.is_custom)
                .all(|mode| !mode.enabled)
        );

        let mut invalid: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        invalid["profile"]["settings"]["keyboard"]["dial_active"] = "missing-mode".into();
        assert!(
            decode_profile(
                &serde_json::to_vec(&invalid).unwrap(),
                device.product_id,
                device.layout_id
            )
            .is_err()
        );
    }
}

#[::core::prelude::v1::test]
fn imports_reject_incompatible_schema_bad_values_duplicate_keys_and_large_files() {
    let device = device();
    let bytes = encode_profile(&device, &device.profiles[0]).unwrap();
    let valid: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let rejects = |value: serde_json::Value| {
        assert!(
            decode_profile(
                &serde_json::to_vec(&value).unwrap(),
                device.product_id,
                device.layout_id
            )
            .is_err()
        );
    };
    for (field, value) in [
        ("version", serde_json::json!(999)),
        ("format", serde_json::json!("synapse4")),
        ("product_id", serde_json::json!(182)),
        ("layout_id", serde_json::json!(999)),
    ] {
        let mut changed = valid.clone();
        changed[field] = value;
        rejects(changed);
    }
    let mut changed = valid.clone();
    changed["profile"]["settings"]["volume"] = 101.into();
    rejects(changed);
    let mut changed = valid.clone();
    changed["profile"]["settings"]["keyboard"]["future_option"] = true.into();
    rejects(changed);
    let mut changed = valid.clone();
    changed["profile"]["settings"]
        .as_object_mut()
        .unwrap()
        .remove("lighting");
    rejects(changed);
    let mut changed = valid.clone();
    changed["profile"]["settings"]["linked_games"][0]["executable"] = "relative.exe".into();
    rejects(changed);
    let mut changed = valid.clone();
    changed["profile"]["name"] = " ".into();
    rejects(changed);
    let duplicate = String::from_utf8(bytes)
        .unwrap()
        .replace("\"volume\": 70", "\"volume\": 20, \"volume\": 70");
    assert!(decode_profile(duplicate.as_bytes(), device.product_id, device.layout_id).is_err());
    assert!(
        decode_profile(
            &vec![b' '; MAX_BYTES + 1],
            device.product_id,
            device.layout_id
        )
        .is_err()
    );
    assert!(
        decode_profile(
            br#"{"profiles":[],"productId":653}"#,
            device.product_id,
            device.layout_id
        )
        .is_err()
    );
}
