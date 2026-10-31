use super::*;
use serde_json::json;

fn observed(version: &str) -> HelpRuntimeVersionStatus {
    HelpRuntimeVersionStatus::Observed(version.into())
}
fn middleware(product_id: u32, container: &str, rows: Value) -> HelpRuntimeVersionStatus {
    middleware_version(product_id, container, Ok(Some(rows.to_string())))
}
fn application(rows: Value) -> HelpRuntimeVersionStatus {
    application_version(Ok(Some(rows.to_string())))
}
fn scoped(rows: Value, container: &str) -> Result<Option<String>, String> {
    scoped_version(
        &serde_json::from_value::<Vec<WindowEntry>>(rows).unwrap(),
        container,
        Some((105, Some(106))),
    )
}

#[test]
fn every_current_help_page_selects_the_source_storage_branch() {
    let first_entry_products = [
        658, 708, 747, 752, 1442, 1451, 1453, 2660, 2664, 2668, 2689, 3594, 4144, 45066,
    ];
    let mut pages = 0;
    for help in super::super::super::records() {
        for page in help.pages.iter().filter(|page| page.view_more) {
            let selected = middleware(
                help.product_id,
                "{Mounted-Container}",
                json!([
                    {"windowName":"https://host.example/products/999/mw/", "value":"first-publisher"},
                    {"windowName":"https://host.example/products/888/mw/?containerId=mounted-container", "value":"mounted-publisher"}
                ]),
            );
            let expected = if first_entry_products.contains(&help.product_id) {
                "first-publisher"
            } else {
                "mounted-publisher"
            };
            assert_eq!(
                selected,
                observed(expected),
                "{}:{}",
                help.product_id,
                page.offset
            );
            pages += 1;
        }
    }
    assert_eq!(pages, 330);
}

#[test]
fn new_product_pages_keep_their_distinct_source_publishers() {
    let rows = json!([
        {"windowName":"https://host.example/products/999/mw/", "value":"first-publisher"},
        {"windowName":"https://host.example/products/888/mw/?containerId=mounted-container", "value":"mounted-publisher"}
    ]);
    for (product, expected) in [
        (1451, "first-publisher"),
        (45061, "mounted-publisher"),
        (45063, "mounted-publisher"),
    ] {
        assert_eq!(
            middleware(product, "{Mounted-Container}", rows.clone()),
            observed(expected),
            "{product}"
        );
    }
}

#[test]
fn container_precedes_product_and_dongle_without_requiring_the_product_to_match() {
    let rows = json!([
        {"windowName":"https://host.example/products/105/mw/", "value":"product"},
        {"windowName":"https://host.example/products/106/mw/", "value":"dongle"},
        {"windowName":"https://host.example/products/999/mw/?containerId=%7BAB-CD%7D", "value":"container"}
    ]);
    assert_eq!(
        scoped(rows.clone(), "{ab-cd}").unwrap().as_deref(),
        Some("container")
    );
    assert_eq!(
        scoped(rows.clone(), "unmatched").unwrap().as_deref(),
        Some("product")
    );
    assert_eq!(
        scoped(rows, " {ab-cd} ").unwrap().as_deref(),
        Some("product")
    );
    assert_eq!(
        scoped(
            json!([
                {"windowName":"https://host.example/products/106/mw/", "value":"dongle-first"},
                {"windowName":"https://host.example/products/105/mw/", "value":"product-second"}
            ]),
            ""
        )
        .unwrap()
        .as_deref(),
        Some("dongle-first")
    );
}

#[test]
fn selected_record_with_no_version_does_not_fall_through_to_another_publisher() {
    for value in [Value::Null, json!(""), json!(false), json!(0)] {
        let rows = json!([
            {"windowName":"https://host.example/products/999/mw/?containerId=owned", "value":value},
            {"windowName":"https://host.example/products/105/mw/", "value":"must-not-select"}
        ]);
        assert_eq!(scoped(rows.clone(), "owned").unwrap(), None);
        assert_eq!(
            middleware(3594, "owned", rows),
            HelpRuntimeVersionStatus::Missing
        );
    }
    let rows = json!([
        {"windowName":"https://host.example/products/999/mw/?containerId=owned", "value":true},
        {"windowName":"https://host.example/products/105/mw/", "value":"must-not-select"}
    ]);
    assert!(scoped(rows.clone(), "owned").is_err());
    assert!(matches!(
        middleware(3594, "owned", rows),
        HelpRuntimeVersionStatus::Failed(_)
    ));
}

#[test]
fn url_selection_preserves_source_path_and_query_semantics() {
    let rows = json!([
        {"windowName":"/relative/products/105/mw/?containerId=owned", "value":"relative"},
        {"windowName":"https://host.example/products/00105/mw/", "value":"leading-zero"},
        {"windowName":"https://host.example/products/105x/mw/", "value":"nondecimal"},
        {"windowName":"https://host.example/products/7/products/105/mw/", "value":"second-products"},
        {"windowName":"https://host.example/ProDuCtS//106/mw/", "value":"source-dongle"}
    ]);
    assert_eq!(
        scoped(rows, "owned").unwrap().as_deref(),
        Some("source-dongle")
    );
    let rows = json!([
        {"windowName":"https://host.example/?containerId=other&containerId=owned", "value":"second-query"},
        {"windowName":"https://host.example/?containerid=owned", "value":"wrong-key-case"},
        {"windowName":"https://host.example/?containerId=o%7BW%7DnEd", "value":"normalized"}
    ]);
    assert_eq!(
        scoped(rows, "owned").unwrap().as_deref(),
        Some("normalized")
    );
}

#[test]
fn missing_source_identity_never_guesses_a_physical_or_ble_product() {
    let rows = serde_json::from_value::<Vec<WindowEntry>>(json!([
        {"windowName":"https://host.example/products/106/mw/?containerId=owned", "value":"actual"}
    ]))
    .unwrap();
    assert_eq!(
        scoped_version(&rows, "owned", None).unwrap().as_deref(),
        Some("actual")
    );
    assert!(scoped_version(&rows, "other", None).is_err());
    assert_eq!(
        scoped_version(&rows, "other", Some((105, None))).unwrap(),
        None
    );
}

#[test]
fn app_lookup_keeps_the_first_registered_synapse_and_uses_its_actual_version() {
    assert_eq!(
        application(json!([
            {"name":"razer_ui", "version":"9.99.99"},
            {"name":"synapse", "version":"7.12.3.45"},
            {"name":"synapse", "version":"8.0.0"}
        ])),
        observed("4.12.3.45")
    );
    assert_eq!(
        application(json!([{"name":"synapse", "version":"local"}])),
        observed("4")
    );
    for version in [Value::Null, json!(""), json!(false), json!(0)] {
        assert_eq!(
            application(json!([
                {"name":"synapse", "version":version},
                {"name":"synapse", "version":"1.2.3"}
            ])),
            HelpRuntimeVersionStatus::Missing
        );
    }
    assert_eq!(
        application(json!([
            {"name":"synapse"}, {"name":"synapse", "version":"1.2.3"}
        ])),
        HelpRuntimeVersionStatus::Missing
    );
}

#[test]
fn malformed_app_records_keep_source_failure_and_short_circuit_order() {
    for version in [json!(true), json!(42), json!([]), json!({})] {
        assert!(matches!(
            application(json!([
                {"name":"synapse", "version":version},
                {"name":"synapse", "version":"1.2.3"}
            ])),
            HelpRuntimeVersionStatus::Failed(_)
        ));
    }
    assert!(matches!(
        application(json!([
            null, {"name":"synapse", "version":"1.2.3"}
        ])),
        HelpRuntimeVersionStatus::Failed(_)
    ));
    assert_eq!(
        application(json!([
            {"name":"synapse", "version":"1.2.3"}, null
        ])),
        observed("4.2.3")
    );
    assert_eq!(
        application(json!([
            ["synapse", "1.2.3"], 1, true, "synapse", {"name":"Synapse", "version":"1.2.3"}
        ])),
        HelpRuntimeVersionStatus::Missing
    );
}

#[test]
fn absent_failed_and_invalid_storage_never_become_default_versions() {
    for raw in [None, Some(String::new()), Some("[]".into())] {
        assert_eq!(
            application_version(Ok(raw.clone())),
            HelpRuntimeVersionStatus::Missing
        );
        assert_eq!(
            middleware_version(3594, "", Ok(raw)),
            HelpRuntimeVersionStatus::Missing
        );
    }
    for raw in ["{", "null", "{}", "true"] {
        assert!(matches!(
            application_version(Ok(Some(raw.into()))),
            HelpRuntimeVersionStatus::Failed(_)
        ));
        assert!(matches!(
            middleware_version(3594, "", Ok(Some(raw.into()))),
            HelpRuntimeVersionStatus::Failed(_)
        ));
    }
    assert_eq!(
        application_version(Err("read denied".into())),
        HelpRuntimeVersionStatus::Failed("read denied".into())
    );
}

#[test]
fn scoped_window_storage_non_array_documents_are_missing_without_a_parse_failure() {
    for raw in ["null", "{}", "true", "42", "\"version\""] {
        assert_eq!(
            middleware_version(3595, "", Ok(Some(raw.into()))),
            HelpRuntimeVersionStatus::Missing
        );
    }
    assert!(matches!(
        middleware_version(3595, "", Ok(Some("{".into()))),
        HelpRuntimeVersionStatus::Failed(_)
    ));
}
