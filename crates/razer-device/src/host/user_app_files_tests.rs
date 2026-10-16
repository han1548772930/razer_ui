use super::*;

#[test]
fn write_rejection_order_matches_native_entrypoint() {
    for (folder, path, size, expected) in [
        ("", "", 0, "Error : App is empty."),
        ("Common", "", 0, "Error : File path is empty."),
        ("Common", "engine.dll", 0, "Error : File data is empty."),
    ] {
        let response = validate_write(folder, path, size).unwrap().unwrap();
        assert!(!response.result);
        assert_eq!(response.reason, expected);
    }
    assert!(validate_write(" ", " ", 1).unwrap().is_none());
    assert!(validate_write("Common", "engine.dll", u32::MAX as u64 + 1).is_err());
    assert!(validate_write("Common\0other", "engine.dll", 1).is_err());
}

#[test]
fn only_zip_uses_direct_write_and_icon_has_its_own_signature_parser() {
    for (extension, expected) in [
        (".ZIP", VerificationRoute::ZipDirect),
        (".msi", VerificationRoute::PackagePath),
        (".MsIx", VerificationRoute::PackagePath),
        (".ico", VerificationRoute::IconHandle),
        (".dll", VerificationRoute::AuthenticodeHandle),
        (".json", VerificationRoute::AuthenticodeHandle),
        ("", VerificationRoute::AuthenticodeHandle),
        (".zip.exe", VerificationRoute::AuthenticodeHandle),
    ] {
        assert_eq!(verification_route(extension), expected);
    }
}

#[test]
fn failed_source_callback_is_preserved_as_response_data() {
    let response: AppFileResponse = serde_json::from_str(
        r#"{"result":false,"reason":"Error: This PE file is not signed by allowed signer"}"#,
    )
    .unwrap();
    assert!(!response.result);
    assert_eq!(
        serde_json::to_value(response).unwrap()["reason"],
        "Error: This PE file is not signed by allowed signer"
    );
}

#[test]
fn event_payload_remains_opaque_and_unknown_events_are_forwarded() {
    let event = AppServiceEvent::from_native(0, r#"{"filePath":"a.dll","result":false}"#.into());
    assert_eq!(event.event_id, "onsimpleadduserappfile");
    assert_eq!(event.event_data, r#"{"filePath":"a.dll","result":false}"#);
    assert_eq!(
        AppServiceEvent::from_native(999, "raw".into()).event_id,
        "unknown"
    );
    assert!(!UserAppFilesOperation::DrainEvents.mutates_files());
}
