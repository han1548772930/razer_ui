//! Source local reset documents; no JSON parsing or remote resource is required.
//! Maintenance input SHA-256: 22d675e2911e19d055810deab6bff80e7bc390de3d1ada4e1db430ffcecfae10
use crate::receiver_defaults::{
    Brightness, GlobalDocument, ProfileDocument, QuickEffects, ReportIds, ResetDefaults,
    SwitchOffLighting,
};

#[rustfmt::skip]
pub(crate) static PRODUCTS: &[ResetDefaults] = &[
    ResetDefaults {
        product_id: 164, edition_id: 0,
        device_name: "Razer Mouse Dock Pro",
        schema_version: 13,
        default_profile: ProfileDocument {
            guid: "", name: " ",
            brightness: Brightness { is_enabled: true, value: 100 },
            switch_off_lighting: SwitchOffLighting { is_display_on: false, is_idle_enabled: false, idle_minutes: 1 },
            quick_effects: QuickEffects { selected_effect_id: 3 },
            mappings: &[],
            can_pair_two_devices: None,
        },
        default_global: GlobalDocument {
            version: 1, vendor_id: 5426, product_id: 164,
            category: "ACCESSORY",
            report_ids: ReportIds { key_report: "razerKeyReportID", hardware_event: "hardwareEventReportID" },
            can_pair_two_devices: None,
        },
    },
    ResetDefaults {
        product_id: 241, edition_id: 0,
        device_name: "Razer Mouse Dock V2 Pro",
        schema_version: 13,
        default_profile: ProfileDocument {
            guid: "", name: " ",
            brightness: Brightness { is_enabled: true, value: 100 },
            switch_off_lighting: SwitchOffLighting { is_display_on: false, is_idle_enabled: false, idle_minutes: 1 },
            quick_effects: QuickEffects { selected_effect_id: 3 },
            mappings: &[],
            can_pair_two_devices: Some(true),
        },
        default_global: GlobalDocument {
            version: 1, vendor_id: 5426, product_id: 241,
            category: "ACCESSORY",
            report_ids: ReportIds { key_report: "razerKeyReportID", hardware_event: "hardwareEventReportID" },
            can_pair_two_devices: Some(true),
        },
    },
];
