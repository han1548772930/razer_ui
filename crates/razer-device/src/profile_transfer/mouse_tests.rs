use super::*;

fn guids() -> impl Iterator<Item = String> {
    (1..200).map(|value| format!("{value:08x}-0000-4000-8000-000000000000"))
}

fn run(
    product_id: u32,
    rows: &[Value],
    profiles: &[Value],
    macros: &[Value],
    settings: &Value,
    selected: Option<&Value>,
) -> Result<PreparedMouseImport> {
    prepare_global_mouse_import(
        rows,
        MouseImportContext::new(product_id, profiles, macros, Some(settings))
            .with_selected_profile(selected),
        guids(),
    )
}

fn mapping_field() -> &'static str {
    MappingProduct::for_product_id(70).unwrap().storage_field()
}

fn settings(product_id: u32, profiles: &[Value]) -> Value {
    let mut settings = json!({"version":7,"activeProfile":"kept",
        "defaultTurbos":[{"guid":"default","name":"kept"}],"synapseTurbos":[],
        "extension":{"raw":true}});
    if product_id == 70 {
        let mut document = settings.clone();
        document["profiles"] = json!(profiles);
        document["defaultMappings"] = json!({"appEngine":{"mappings":[],
            "hash":"de2ccb2014607a84df08306567cd96f0"}});
        document.as_object_mut().unwrap().remove("activeProfile");
        if let Some(first) = profiles.first() {
            document["activeProfile"] = first["guid"].clone();
        }
        settings = json!({"activeProfile":"kept","extension":{"raw":true}});
        settings[mapping_field()] = document;
    }
    settings
}

#[test]
fn mouse_import_182_defaults_match_complete_current_product_info() {
    assert_eq!(
        super::super::mouse_defaults::default_profile_182(),
        json!({"guid":"","name":"","dpi":{"x":800,"y":800},
            "ledPowerSettings":{"idleState":1,"idleValue":60},"lowPowerMode":30,"mappings":[],
            "dpiStages":{"stages":[
                {"x":400,"y":400,"independent":false,"visible":true},
                {"x":800,"y":800,"independent":false,"visible":true},
                {"x":1600,"y":1600,"independent":false,"visible":true},
                {"x":3200,"y":3200,"independent":false,"visible":true},
                {"x":6400,"y":6400,"independent":false,"visible":true}],
                "active":3,"enable":true,"count":5},"pollingRateWireless":1000,"pollingRate":1000})
    );
    assert_eq!(
        super::super::mouse_defaults::default_device_182(),
        json!({"version":1,"vendorId":5426,"productId":182,"category":"MOUSE",
            "reportIDs":{"4":"razerKeyReportID","5":"hardwareEventReportID"},
            "calibration":{"calibrationTrackingMode":false,"liftOffRangeValue":3,
                "smartTracking":{"isAsymmetric":false,"trackingDistance":2,"liftOffDistance":13,"landingDistance":12},
                "presetTracking":{"isAsymmetric":false,"surfaceDistance":5,"liftOffDistance":13,"landingDistance":12},
                "profiles":[{"type":"default"}],"selectedProfile":{}},"lowPowerMode":30})
    );
}

#[test]
fn mouse_import_preserves_old_collection_shallow_extensions_and_source_name_order() {
    let profiles = [json!({"guid":"kept","name":"Game","mappings":[],"appEngine":{"raw":"old"}})];
    let rows = [
        json!({"profile":{"guid":"foreign","name":"Game","dpi":{"x":900},"_extension":7},"listItems":[]}),
        json!({"profile":{"name":"Game","dpi":null},"listItems":[]}),
        json!({"profile":{"name":"game"},"listItems":[]}),
    ];
    for pid in [70, 182] {
        let before_rows = rows.clone();
        let before_profiles = profiles.clone();
        let settings = settings(pid, &profiles);
        let output = run(pid, &rows, &profiles, &[], &settings, None).unwrap();
        assert_eq!(output.profiles[0], profiles[0]);
        assert_eq!(output.profiles[1]["name"], "Game (1)");
        assert_eq!(output.profiles[2]["name"], "Game (2)");
        assert_eq!(output.profiles[3]["name"], "game");
        assert_eq!(output.profiles[1]["dpi"], json!({"x":900}));
        assert_eq!(output.profiles[1]["_extension"], 7);
        assert_eq!(output.profiles[2]["dpi"], Value::Null);
        assert_eq!(
            output.profiles[1]["appEngine"],
            json!({"mappings":[],"hash":"de2ccb2014607a84df08306567cd96f0"})
        );
        assert!(
            output.profiles[1]
                .get("selectedSensitivityMatcher")
                .is_none()
        );
        assert_eq!(output.device_settings["activeProfile"], "kept");
        assert_eq!(output.device_settings["extension"], settings["extension"]);
        let persisted = if pid == 70 {
            &output.device_settings[mapping_field()]
        } else {
            &output.device_settings
        };
        assert_eq!(persisted["version"], 8);
        if pid == 70 {
            assert_eq!(persisted["profiles"][0], profiles[0]);
            assert_eq!(persisted["profiles"], json!(output.profiles));
            assert!(output.device_settings.get("version").is_none());
            assert!(output.device_settings.get("defaultTurbos").is_none());
            assert!(output.device_settings.get("synapseTurbos").is_none());
        }
        assert_eq!(rows, before_rows);
        assert_eq!(profiles, before_profiles);
    }
}

#[test]
fn mouse_import_compiles_source_multi_mode_with_product_dkm_keys() {
    for (pid, input, key, hash) in [
        (70, "DKM_M_01", 32, "a319b1e39b549d687c96f61792d76b04"),
        (182, "DKM_SB_03", 82, "6a023306d8c38006f2f259b52e53b871"),
    ] {
        let rows = [json!({"profile":{"name":"Disabled","mappings":[{
            "inputType":"DKMInput","inputID":input,"outputType":"disableGroup"}]},"listItems":[]})];
        let output = run(pid, &rows, &[], &[], &settings(pid, &[]), None).unwrap();
        assert_eq!(
            output.profiles[0]["appEngine"],
            json!({"mappings":[{
            "input":{"type":"razerKey","key":key,"flag":1},
            "output":{"type":"multi","outputs":[{"type":"disabled"}]}}],"hash":hash})
        );
    }
}

#[test]
fn mouse_import_182_compiles_current_factory_defaults_dkm_and_otfs_projection() {
    let factory = MappingEngine::new(182).unwrap();
    assert_eq!(
        factory.source_key_metadata("KEY_A").unwrap(),
        json!({"name":"A","inputID":"KEY_A","type":"Alphanumeric","scancode":30,
            "analogKeyID":31,"virtualKey":"65","keyCode":"65","flag":0,
            "pageID":"7","HID":"4"})
    );
    let profiles = [json!({"guid":"kept","name":"Kept","mappings":[]})];
    let root = json!({"version":7,"activeProfile":"kept","extension":{"raw":true}});
    let rows = [json!({"profile":{"name":"Factory","mappings":[
        {"inputType":"DKMInput","inputID":"DKM_SB_03","outputType":"mouseGroup",
            "mouseGroup":{"mouseAssignment":"DoubleClick"}},
        {"inputType":"DKMInput","inputID":"DKM_M_02","outputType":"keyboardGroup",
            "keyboardGroup":{"key":"KEY_A","modifiers":[]}},
        {"inputType":"DKMInput","inputID":"DKM_M_01","outputType":"sensitivityGroup",
            "sensitivityGroup":{"sensitivityAssignment":"DPI_OnTheFly"}}]},"listItems":[]})];
    let output = run(182, &rows, &profiles, &[], &root, None).unwrap();
    let turbo_guid = "00000001-0000-4000-8000-000000000000";
    let mut mappings = vec![
        json!({"input":{"type":"razerKey","key":32,"flag":0},
            "output":{"type":"multi","outputs":[{"type":"otfs","flag":0}]}}),
        json!({"input":{"type":"razerKey","key":32,"flag":1},
            "output":{"type":"multi","outputs":[{"type":"otfs","flag":1}]}}),
        json!({"input":{"type":"razerKey","key":33,"flag":0},
            "output":{"type":"multi","outputs":[{"type":"keyboard","scancode":30,"flag":0}]}}),
        json!({"input":{"type":"razerKey","key":33,"flag":1},
            "output":{"type":"multi","outputs":[{"type":"keyboard","scancode":30,"flag":1}]}}),
        json!({"input":{"type":"razerKey","key":82,"flag":0},
            "output":{"type":"multi","outputs":[{"type":"turbo","guid":turbo_guid,"flag":0,"repeat":1}]}}),
        json!({"input":{"type":"razerKey","key":82,"flag":1},
            "output":{"type":"multi","outputs":[{"type":"turbo","guid":turbo_guid,"flag":1}]}}),
    ];
    for (scroll_data, assignment) in [
        (
            [120, 240, 360, 480, 600, 720, 840, 960, 1080, 1200],
            "DPI_Value_Up",
        ),
        (
            [
                65416, 65296, 65176, 65056, 64936, 64816, 64696, 64576, 64456, 64336,
            ],
            "DPI_Value_Down",
        ),
    ] {
        mappings.extend(scroll_data.map(|data| {
            json!({"input":{"type":"mouse","id":1024,"data":data,"otfs":true},
                "output":{"type":"sensitivity","sensitivityGroup":{"sensitivityAssignment":assignment},"flag":0}})
        }));
    }
    assert_eq!(
        output.profiles[1]["appEngine"],
        json!({"mappings":mappings,"hash":"1ba01825a9a3ef3ae587c7617fe5b066"})
    );
    assert_eq!(output.profiles[0], profiles[0]);
    assert_eq!(
        output.profiles[1]["guid"],
        "00000027-0000-4000-8000-000000000000"
    );
    assert_eq!(output.device_settings["version"], 8);
    assert_eq!(output.device_settings["activeProfile"], "kept");
    assert_eq!(output.device_settings["extension"], root["extension"]);
    assert_eq!(output.device_settings["synapseTurbos"], json!([]));
    assert_eq!(
        output.device_settings["defaultTurbos"][0],
        json!({"name":"DoubleClick","id":"DoubleClick","guid":turbo_guid,
            "appEngine":{"events":[{"type":"mouse","id":1,"data":0},
                {"type":"mouse","id":2,"data":0},{"type":"delay","ms":10},
                {"type":"mouse","id":1,"data":0},{"type":"mouse","id":2,"data":0}],
                "hash":"3faa20480beb21fbd2017f4e1eac992c"}})
    );
    assert_eq!(
        output.device_settings["defaultTurbos"][37]["name"],
        "TrackJoggingBackward"
    );
    assert!(output.device_settings.get(mapping_field()).is_none());
    assert_eq!(
        root,
        json!({"version":7,"activeProfile":"kept","extension":{"raw":true}})
    );
}

#[test]
fn mouse_import_repairs_existing_macro_references_and_nested_stages_in_order() {
    let macros = [
        json!({"guid":"kept-macro","name":"Same"}),
        json!({"guid":"other-macro","name":"Same"}),
    ];
    let rows = [json!({"profile":{"name":"Macros","mappings":[
        {"outputType":"macroGroup","inputID":"DKM_M_01","inputType":"DKMInput",
            "macroGroup":{"guid":"unchecked-missing","name":"Missing","macroPlaybackOption":"Once"}},
        {"outputType":"macroGroup","inputID":"DKM_M_01","inputType":"DKMInput",
            "macroGroup":{"guid":"kept-macro","name":"Old","macroPlaybackOption":"Once"}},
        {"mapping":[{"macroGroup":{"guid":"other-macro","name":"Old"}},
            {"macroGroup":{"guid":"kept-macro","name":"Other"}},
            {"outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_A"}}]},
        {"mapping":[{"macroGroup":{"guid":"unknown","name":"Old"}},
            {"macroGroup":{"guid":"other-macro","name":"Old"}}]}],
        "sidePanelMappings":{"left":[{"macroGroup":{"guid":"other-macro","name":"Old"}}]}},
        "listItems":[{"guid":"unchecked-missing","isMacroSelected":false},
            {"guid":"kept-macro","isMacroSelected":false}]})];
    let output = run(182, &rows, &[], &macros, &settings(182, &[]), None).unwrap();
    let mappings = output.profiles[0]["mappings"].as_array().unwrap();
    assert_eq!(mappings.len(), 3);
    assert_eq!(mappings[0]["macroGroup"]["guid"], "kept-macro");
    assert_eq!(mappings[0]["macroGroup"]["name"], "Same");
    assert_eq!(
        mappings[1]["mapping"][0]["macroGroup"]["guid"],
        "other-macro"
    );
    assert_eq!(
        mappings[1]["mapping"][1]["macroGroup"]["guid"],
        "other-macro"
    );
    assert_eq!(mappings[1]["mapping"][2]["outputType"], "defaultGroup");
    assert_eq!(mappings[2]["mapping"][0]["keyboardGroup"], json!({}));
    assert_eq!(mappings[2]["mapping"][0]["outputType"], "keyboardGroup");
    assert_eq!(mappings[2]["mapping"][1]["outputType"], "defaultGroup");
    assert!(mappings[2]["mapping"][1].get("macroGroup").is_none());
    assert_eq!(
        output.profiles[0]["sidePanelMappings"]["left"][0]["macroGroup"]["name"],
        "Same"
    );
    assert!(
        output.profiles[0]["appEngine"]["mappings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|mapping| {
                mapping["output"]["outputs"]
                    .as_array()
                    .is_some_and(|outputs| {
                        outputs.iter().any(|output| {
                            output["type"] == "macro" && output["guid"] == "kept-macro"
                        })
                    })
            })
    );
    assert_eq!(macros[0]["name"], "Same");
    assert_eq!(
        rows[0]["profile"]["mappings"][1]["macroGroup"]["name"],
        "Old"
    );
}

#[test]
fn mouse_import_generates_default_turbos_and_prunes_only_source_direct_references() {
    let rows = [json!({"profile":{"name":"Turbo","appEngine":{"mappings":[
        {"output":{"type":"turbo","guid":"old"}},
        {"output":{"type":"multi","outputs":[{"type":"turbo","guid":"nested"}]}}]}},"listItems":[]})];
    let mut root = json!({"extension":"kept"});
    root[mapping_field()] = json!({"version":0,"profiles":[],
        "defaultMappings":{"appEngine":{"mappings":[],"hash":"de2ccb2014607a84df08306567cd96f0"}},
        "synapseTurbos":[{"guid":"old"},{"guid":"nested"},{"guid":"unrelated"}]});
    let output = run(70, &rows, &[], &[], &root, None).unwrap();
    assert_eq!(
        output.device_settings[mapping_field()]["defaultTurbos"]
            .as_array()
            .unwrap()
            .len(),
        38
    );
    assert_eq!(
        output.device_settings[mapping_field()]["synapseTurbos"],
        json!([{"guid":"nested"},{"guid":"unrelated"}])
    );
    assert_eq!(
        output.profiles[0]["guid"],
        "00000027-0000-4000-8000-000000000000"
    );
}

#[test]
fn mouse_import_preserves_falsy_mappings_and_merges_selected_eq_camera_extensions() {
    let profiles = [
        json!({"guid":"one","name":"One","camera":{"old":1,"priority":1}}),
        json!({"guid":"two","name":"Two","camera":{"new":2,"priority":2}}),
    ];
    let selected = json!({"equalizer":{"selected":2}});
    let rows = [
        json!({"profile":{"name":"Extra","mappings":null,"appEngine":{"raw":true},
        "equalizer":false,"camera":{"priority":9},"selectedSensitivityMatcher":null},"listItems":[]}),
    ];
    let root = json!({"version":9007199254740991_u64,"extension":2});
    let output = run(182, &rows, &profiles, &[], &root, Some(&selected)).unwrap();
    assert_eq!(output.profiles[2]["mappings"], Value::Null);
    assert_eq!(output.profiles[2]["appEngine"], json!({"raw":true}));
    assert_eq!(
        output.profiles[2]["camera"],
        json!({"old":1,"new":2,"priority":9})
    );
    assert_eq!(output.profiles[2]["equalizer"], selected["equalizer"]);
    assert_eq!(
        output.profiles[2]["selectedSensitivityMatcher"],
        Value::Null
    );
    assert_eq!(output.device_settings, json!({"version":0,"extension":2}));
}

#[test]
fn mouse_import_rejects_invalid_rows_versions_and_profile_identities() {
    let valid = [json!({"profile":{"name":"Valid"},"listItems":[]})];
    for row in [
        json!({"profile":{},"listItems":[]}),
        json!({"profile":{"name":"A"}}),
        json!({"profile":{"name":"A"},"listItems":[null]}),
        json!({"profile":{"name":"A","mappings":{}},"listItems":[]}),
        json!({"profile":{"name":"A","mappings":[null]},"listItems":[]}),
    ] {
        assert!(run(70, &[row], &[], &[], &settings(70, &[]), None).is_err());
    }
    assert!(run(70, &[], &[], &[], &settings(70, &[]), None).is_err());
    assert!(run(190, &valid, &[], &[], &settings(190, &[]), None).is_err());
    for root in [
        Value::Null,
        json!({"version":null}),
        json!({"version":"1"}),
        json!({"version":1.5}),
        json!({"version":9007199254740992_u64}),
    ] {
        assert!(run(182, &valid, &[], &[], &root, None).is_err());
    }
    let duplicate = [
        json!({"guid":"duplicate","name":"A"}),
        json!({"guid":"duplicate","name":"B"}),
    ];
    assert!(
        run(
            182,
            &valid,
            &duplicate,
            &[],
            &settings(182, &duplicate),
            None
        )
        .is_err()
    );
    for fresh in [Vec::new(), vec!["invalid".to_owned()]] {
        assert!(
            prepare_global_mouse_import(
                &valid,
                MouseImportContext::new(70, &[], &[], Some(&settings(70, &[]))),
                fresh
            )
            .is_err()
        );
    }
    let duplicate = [json!({"guid":"00000001-0000-4000-8000-000000000000","name":"A"})];
    assert!(
        run(
            182,
            &valid,
            &duplicate,
            &[],
            &settings(182, &duplicate),
            None
        )
        .is_err()
    );
}

#[test]
fn mouse_import_product_ui_selects_compiles_and_remaps_macros_without_changing_global_stage() {
    let existing = [json!({"guid":"duplicate","name":"Macro"})];
    let macro_data = [
        json!({"guid":"duplicate","name":"Macro","macroEvents":[{"Type":0,"Number":0.01}],
            "macroPlaybackOption":"Once","extension":{"kept":true}}),
        json!({"guid":"unselected","name":"Ignored"}),
    ];
    let rows = [json!({"profile":{"name":"Product","mappings":[
        {"inputType":"DKMInput","inputID":"DKM_M_01","outputType":"macroGroup",
            "macroGroup":{"guid":"duplicate","name":"Old","macroPlaybackOption":"Once"}},
        {"mapping":[{"macroGroup":{"guid":"duplicate","name":"Old"}},
            {"macroGroup":{"guid":"untouched","name":"Second"}}]}]},
        "listItems":[{"guid":"duplicate","isMacroSelected":true},
            {"guid":"unselected","isMacroSelected":false}]})];
    for pid in [70, 182] {
        let root = settings(pid, &[]);
        let product = prepare_mouse_import(
            &rows,
            &macro_data,
            MouseImportContext::new(pid, &[], &existing, Some(&root)),
            guids(),
        )
        .unwrap();
        assert_eq!(product.imported_macros.len(), 1);
        assert_eq!(
            product.imported_macros[0]["guid"],
            "00000001-0000-4000-8000-000000000000"
        );
        assert_eq!(product.imported_macros[0]["name"], "Macro (1)");
        assert_eq!(
            product.imported_macros[0]["extension"],
            json!({"kept":true})
        );
        assert_eq!(
            product.imported_macros[0]["appEngine"],
            json!({
            "events":[{"type":"delay","ms":10.0}],"hash":"2ede0932f61c279240bddccbf0447ef8","mouseMoveType":"none"})
        );
        assert_eq!(
            product.profiles[0]["guid"],
            "00000002-0000-4000-8000-000000000000"
        );
        let mapping = &product.profiles[0]["mappings"];
        assert_eq!(
            mapping[0]["macroGroup"]["guid"],
            product.imported_macros[0]["guid"]
        );
        assert_eq!(mapping[0]["macroGroup"]["name"], "Macro (1)");
        assert_eq!(
            mapping[1]["mapping"][0]["macroGroup"]["guid"],
            product.imported_macros[0]["guid"]
        );
        assert_eq!(mapping[1]["mapping"][1]["macroGroup"]["guid"], "untouched");
        let global = run(pid, &rows, &[], &existing, &root, None).unwrap();
        assert!(global.imported_macros.is_empty());
        assert_eq!(
            global.profiles[0]["mappings"][0]["macroGroup"]["guid"],
            "duplicate"
        );
        assert_eq!(
            global.profiles[0]["mappings"][0]["macroGroup"]["name"],
            "Macro"
        );
    }
    assert_eq!(macro_data[0]["guid"], "duplicate");
    assert_eq!(macro_data[0]["name"], "Macro");
}

#[test]
fn mouse_import_product_ui_unchecked_filter_uses_original_macro_store() {
    let macro_data =
        [json!({"guid":"new-macro","name":"New","macroEvents":[],"macroPlaybackOption":"Once"})];
    let rows = [false, true].map(|selected| {
        json!({"profile":{"name":"Profile","mappings":[{
        "inputType":"DKMInput","inputID":"DKM_M_01","outputType":"macroGroup",
        "macroGroup":{"guid":"new-macro","name":"Old","macroPlaybackOption":"Once"}}]},
        "listItems":[{"guid":"new-macro","isMacroSelected":selected}]})
    });
    let root = settings(182, &[]);
    let output = prepare_mouse_import(
        &rows,
        &macro_data,
        MouseImportContext::new(182, &[], &[], Some(&root)),
        guids(),
    )
    .unwrap();
    assert_eq!(output.imported_macros.len(), 1);
    assert_eq!(output.imported_macros[0]["guid"], "new-macro");
    assert_eq!(output.profiles[0]["mappings"], json!([]));
    assert_eq!(
        output.profiles[1]["mappings"][0]["macroGroup"]["name"],
        "New"
    );
    assert_eq!(
        output.profiles[0]["guid"],
        "00000001-0000-4000-8000-000000000000"
    );
}

#[test]
fn mouse_import_70_reuses_nested_turbo_owner_and_preserves_existing_native_projection() {
    let mapping = json!({"inputType":"DKMInput","inputID":"DKM_M_01",
        "outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_A","modifiers":[],
            "turboMode":{"keysPerSecond":10}}});
    let profiles = [
        json!({"guid":"kept","name":"Kept","mappings":[mapping.clone()],
        "_source_extension":{"retained":true}}),
    ];
    let mut document =
        mouse_legacy_mapping::prepare_document(&profiles, "kept", None, guids()).unwrap();
    document["profiles"][0]["projectionExtension"] = json!({"native":"retained"});
    document["macros"] = json!([{"guid":"old-publication"}]);
    document["phasedMacros"] = document["macros"].clone();
    let prior_document = document.clone();
    let mut root = json!({"version":999,"extension":{"root":true},
        "defaultTurbos":[{"guid":"unrelated-root"}]});
    root[mapping_field()] = document;
    let rows = [json!({"profile":{"name":"Imported","mappings":[mapping]},"listItems":[]})];

    let output = run(70, &rows, &profiles, &[], &root, Some(&profiles[0])).unwrap();
    let document = &output.device_settings[mapping_field()];
    assert_eq!(output.profiles[0], profiles[0]);
    assert_eq!(document["profiles"][0], prior_document["profiles"][0]);
    assert_eq!(document["profiles"][1], output.profiles[1]);
    assert_eq!(document["activeProfile"], "kept");
    assert_eq!(document["version"], 2);
    assert_eq!(document["defaultTurbos"], prior_document["defaultTurbos"]);
    assert_eq!(document["synapseTurbos"], prior_document["synapseTurbos"]);
    assert_eq!(
        document["defaultMappings"],
        prior_document["defaultMappings"]
    );
    assert_eq!(document["macros"], prior_document["macros"]);
    assert_eq!(document["phasedMacros"], prior_document["phasedMacros"]);
    assert_eq!(output.device_settings["version"], 999);
    assert_eq!(
        output.device_settings["defaultTurbos"],
        root["defaultTurbos"]
    );
    assert!(output.device_settings.get("synapseTurbos").is_none());
    assert_eq!(output.device_settings["extension"], root["extension"]);
    assert_eq!(root[mapping_field()], prior_document);

    // A later explicit save uses the ordinary compiler with no new IDs.
    let next = mouse_legacy_mapping::prepare_document(
        &output.profiles,
        "kept",
        Some(document),
        Vec::new(),
    )
    .unwrap();
    assert_eq!(next["defaultTurbos"], prior_document["defaultTurbos"]);
    assert_eq!(next["synapseTurbos"], prior_document["synapseTurbos"]);
}

#[test]
fn mouse_import_70_initializes_missing_nested_projection_through_its_existing_owner() {
    let profiles = [json!({"guid":"kept","name":"Kept","mappings":[],
        "DPI":{"X":1200,"Y":1300},"PollingRate":500,"_extension":"source"})];
    let rows = [json!({"profile":{"name":"Imported"},"listItems":[]})];
    let root = json!({"extension":{"retained":true}});
    let output = run(70, &rows, &profiles, &[], &root, Some(&profiles[0])).unwrap();
    assert_eq!(output.profiles[0], profiles[0]);
    let document = &output.device_settings[mapping_field()];
    assert_eq!(document["version"], 1);
    assert_eq!(document["productId"], 70);
    assert_eq!(document["vendorId"], 5426);
    assert_eq!(document["category"], "MOUSE");
    assert_eq!(
        document["reportIDs"],
        json!({"4":"razerKeyReportID","5":"hardwareEventReportID"})
    );
    assert_eq!(document["activeProfile"], "kept");
    assert_eq!(document["profiles"][0]["dpi"], json!({"x":1200,"y":1300}));
    assert_eq!(document["profiles"][0]["pollingRate"], 500);
    assert!(document["profiles"][0].get("DPI").is_none());
    assert_eq!(document["profiles"][1], output.profiles[1]);
    assert_eq!(document["defaultTurbos"].as_array().unwrap().len(), 38);
    assert!(document["defaultMappings"]["appEngine"]["mappings"].is_array());
    assert_eq!(output.device_settings["extension"], root["extension"]);
    for field in ["version", "defaultTurbos", "synapseTurbos"] {
        assert!(output.device_settings.get(field).is_none());
    }
    let next = mouse_legacy_mapping::prepare_document(
        &output.profiles,
        "kept",
        Some(document),
        Vec::new(),
    )
    .unwrap();
    assert_eq!(next["defaultTurbos"], document["defaultTurbos"]);
    assert_eq!(next["synapseTurbos"], document["synapseTurbos"]);
}

#[test]
fn mouse_import_70_rejects_stale_nested_collections_selection_and_versions() {
    let profiles = [json!({"guid":"kept","name":"Kept","mappings":[]})];
    let rows = [json!({"profile":{"name":"Imported"},"listItems":[]})];
    let valid = settings(70, &profiles);
    let mut invalid_documents = Vec::new();
    for version in [
        Value::Null,
        json!("1"),
        json!(1.5),
        json!(9007199254740992_u64),
    ] {
        let mut root = valid.clone();
        root[mapping_field()]["version"] = version;
        invalid_documents.push(root);
    }
    for field in ["profiles", "activeProfile", "defaultMappings"] {
        let mut root = valid.clone();
        root[mapping_field()].as_object_mut().unwrap().remove(field);
        invalid_documents.push(root);
    }
    for (field, value) in [("guid", "other"), ("name", "Stale")] {
        let mut root = valid.clone();
        root[mapping_field()]["profiles"][0][field] = value.into();
        invalid_documents.push(root);
    }
    for collection in [json!([]), json!([profiles[0].clone(), profiles[0].clone()])] {
        let mut root = valid.clone();
        root[mapping_field()]["profiles"] = collection;
        invalid_documents.push(root);
    }
    for selection in [Value::Null, json!(4), json!("other")] {
        let mut root = valid.clone();
        root[mapping_field()]["activeProfile"] = selection;
        invalid_documents.push(root);
    }
    for root in invalid_documents {
        let before = root.clone();
        assert!(run(70, &rows, &profiles, &[], &root, Some(&profiles[0])).is_err());
        assert_eq!(root, before);
    }
    assert!(
        run(
            70,
            &rows,
            &profiles,
            &[],
            &valid,
            Some(&json!({"guid":"other"}))
        )
        .is_err()
    );
}

#[test]
fn mouse_import_70_normalizes_nested_turbo_alias_and_wraps_its_own_source_version() {
    let mut root = settings(70, &[]);
    root["version"] = 91.into();
    let document = root[mapping_field()].as_object_mut().unwrap();
    document.insert("version".into(), 9007199254740991_u64.into());
    let defaults = document.remove("defaultTurbos").unwrap();
    document.insert("synapseDefaultTurbos".into(), defaults.clone());
    let rows = [json!({"profile":{"name":"Imported"},"listItems":[]})];
    let output = run(70, &rows, &[], &[], &root, None).unwrap();
    let document = &output.device_settings[mapping_field()];
    assert_eq!(document["version"], 0);
    assert_eq!(document["defaultTurbos"], defaults);
    assert!(document.get("synapseDefaultTurbos").is_none());
    assert!(document.get("activeProfile").is_none());
    assert_eq!(
        output.profiles[0]["guid"],
        "00000001-0000-4000-8000-000000000000"
    );
    assert_eq!(output.device_settings["version"], 91);
    assert!(output.device_settings.get("defaultTurbos").is_none());
    assert!(output.device_settings.get("synapseTurbos").is_none());
}
