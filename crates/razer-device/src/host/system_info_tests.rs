use super::*;

#[test]
fn native_display_requires_its_enumeration_and_preserves_null() {
    for invalid in [
        br#"{}"#.as_slice(),
        br#"null"#,
        br#"{"Display":{}}"#,
        br#"{"Display":null,"CPU":42}"#,
        br#"{"Display":[],"CPU":null}"#,
    ] {
        assert!(Data::decode(Query::Display, invalid).is_err());
    }
    let Data::Display(details) = Data::decode(
        Query::Display,
        br#"{"CPU":"Observed processor","Display":null}"#,
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(details.cpu.as_deref(), Some("Observed processor"));
    assert!(matches!(details.display, NativeValue::Null(())));
    assert!(details.device_name.is_none());
    let value = serde_json::to_value(details).unwrap();
    assert!(value["Display"].is_null());
    assert!(value.get("DeviceName").is_none());
}

#[test]
fn full_report_exports_actual_sections_and_external_fields() {
    let bytes = br#"{"Display":{"DeviceName":"Measured laptop","Display":[{"Name":"Local display","Manufactuer":"Monitor vendor"}],"FutureField":{"version":2}},"Firmware":{"BiosFW":"01.02","ECFW":"03","FutureFirmware":"1"},"Driver":{"Device driver":"7.8"},"ExtraSection":{"sample":false}}"#;
    let Data::Full(report) = Data::decode(Query::Full, bytes).unwrap() else {
        panic!()
    };
    assert_eq!(
        report.display.as_ref().unwrap().monitors()[0].name,
        "Local display"
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&report.file_bytes().unwrap()).unwrap(),
        serde_json::from_slice::<serde_json::Value>(bytes).unwrap()
    );
    for invalid in [
        br#"{}"#.as_slice(),
        br#"{"Display":null}"#,
        br#"{"Driver":{"name":42}}"#,
    ] {
        assert!(Data::decode(Query::Full, invalid).is_err());
    }
    assert!(Data::decode(Query::Full, br#"{"Firmware":null,"Driver":null}"#).is_ok());
}

#[test]
fn response_rejects_wrong_product_and_query() {
    let response = Response {
        product_id: 737,
        data: Data::decode(Query::Display, br#"{"Display":[]}"#).unwrap(),
    };
    assert!(response.validate(737, Query::Display).is_ok());
    assert!(response.validate(694, Query::Display).is_err());
    assert!(response.validate(737, Query::Full).is_err());
    assert!(response.validate(190, Query::Display).is_err());
}
