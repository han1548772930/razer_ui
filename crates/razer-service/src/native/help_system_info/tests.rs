use super::semantics::*;

#[test]
fn help_system_info_package_order_and_owners_are_distinct() {
    let names = [
        "AppUp.IntelGraphicsExperience_2.4.6.8_x86__publisher",
        "NVIDIACorp.NVIDIAControlPanel_8.1.9.0_x64__publisher",
        "AppUp.IntelGraphicsExperience_1.2.3.4_x64__publisher",
        "AppUp.IntelGraphicsExperience_9.9.9.9_x64__publisher",
    ];
    assert_eq!(
        package_version(names, "AppUp.IntelGraphicsExperience"),
        ("1.2.3.4".into(), "x64".into())
    );
    assert_eq!(
        package_version(names, "NVIDIACorp.NVIDIAControlPanel"),
        ("8.1.9.0".into(), "x64".into())
    );
    assert_eq!(
        package_version([], "Absent"),
        (String::new(), String::new())
    );
}

#[test]
fn help_system_info_memory_matches_six_significant_digits() {
    assert_eq!(memory_size(16 * 1024 * 1024 * 1024), "16 GB");
    assert_eq!(memory_size(17_008_742_400), "15.8406 GB");
    assert_eq!(memory_size(0), "0 GB");
    assert_eq!(memory_size(1024), "9.53674e-07 GB");
}

#[test]
fn help_system_info_edid_keeps_source_spacing_and_identity() {
    let mut bytes = [0u8; 128];
    bytes[8] = 0x10;
    bytes[9] = 0xac;
    bytes[57] = 0xfc;
    bytes[59..72].copy_from_slice(b"Panel 42  \n  ");
    let (name, manufacturer) = monitor_edid(&bytes).unwrap();
    assert_eq!(name.as_deref(), Some("Panel 42  "));
    assert_eq!(manufacturer, "DEL");
    assert_eq!(monitor_fallback(r"MONITOR\DEL42\{class}\0000"), "DEL42");
    assert!(monitor_edid(&bytes[..125]).is_err());
}

#[test]
fn help_system_info_firmware_and_driver_values_are_not_guessed() {
    assert_eq!(
        camera_revision("USB\\VID_1234&PID_5678&REV_0123&MI_00")
            .unwrap()
            .as_deref(),
        Some("0123")
    );
    assert_eq!(
        camera_revision("USB\\VID_1234&PID_5678&REV_0123").unwrap(),
        None
    );
    assert!(camera_revision("REV_12345&MI_00").is_err());
    assert_eq!(driver_version(0x001f_0000_000f_1770), "31.0.15.6000");
    assert!(driver_vendor("Intel(R) device"));
    assert!(!driver_vendor("intel device"));
    assert_eq!(manufacturer(0xffff), "Unknown Manufacturer");
}
