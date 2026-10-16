use crate::domain::DeviceFeatures;
use crate::model::{Device, DeviceCategory, FirmwareInfo, Profile, SetupStatus};

mod catalog;
pub use catalog::{
    apply_preview_names, preview_edition_label, preview_product_label, preview_search_text,
    product_name as source_product_name,
};

/// Presentation identity for the original, persisted native preview records.
/// Current MW identity normalization supplies the default keyboard layout, but
/// this compatibility path must never change stored identities or observations.
pub fn presentation_variant(device: &Device) -> (u32, u32) {
    let original = (device.edition_id, device.layout_id);
    if device.layout_id != 0
        || !device.dashboard.local_snapshot
        || device.dashboard.connection_observation.is_some()
        || device.dashboard.readonly_keyboard_metadata.is_some()
        || device.serial_number != format!("PREVIEW-{}", device.product_id)
        || device.device_container_id != format!("preview-{}", device.product_id)
    {
        return original;
    }
    let normalized = crate::source_identity::variant(
        device.product_id,
        device.edition_id,
        device.layout_id,
        device.layout_id,
    );
    if normalized.0 == device.edition_id {
        normalized
    } else {
        original
    }
}

pub fn registered_preview(pid: u32) -> Option<Device> {
    let product = razer_catalog::registered(pid)?;
    let categories = product.categories();
    let category = if categories
        .iter()
        .any(|category| matches!(*category, "MOUSE" | "MOUSEPLUSMAT"))
    {
        DeviceCategory::Mouse
    } else if categories.contains(&"MOUSEMAT") {
        DeviceCategory::Mousepad
    } else if categories.contains(&"SYSTEM") {
        DeviceCategory::Other
    } else if categories.contains(&"KEYBOARD") {
        DeviceCategory::Keyboard
    } else if categories.contains(&"KEYPAD") {
        DeviceCategory::Keypad
    } else if categories
        .iter()
        .any(|category| category.starts_with("AUDIO"))
    {
        DeviceCategory::Audio
    } else if categories
        .iter()
        .any(|category| category.starts_with("GAMEPAD"))
    {
        DeviceCategory::Controller
    } else {
        DeviceCategory::Other
    };
    let profile_id = format!("preview-profile-{pid}");
    let mut device = Device {
        dashboard: Default::default(),
        sub_devices: None,
        source_device_settings: None,
        serial_number: format!("PREVIEW-{pid}"),
        product_id: pid,
        real_product_id: pid,
        edition_id: product.edition_ids().first().copied().unwrap_or(0),
        layout_id: 0,
        device_container_id: format!("preview-{pid}"),
        category,
        setup_status: SetupStatus::Unknown,
        active_profile: profile_id.clone(),
        profiles: vec![Profile {
            id: profile_id.clone(),
            guid: profile_id,
            name: "Default".into(),
            settings: None,
            source_settings: None,
            dpi_stages: None,
        }],
        is_single_profile: false,
        is_chroma_device: false,
        has_battery: false,
        use_ble: false,
        name: Default::default(),
        product_name: Default::default(),
        ui_window_name: String::new(),
        mw_window_name: String::new(),
        min_dpi: None,
        max_dpi: None,
        dpi_step: None,
        support_xy_dpi: false,
        power_status: None,
        dkm_keys: Vec::new(),
        firmware_info: FirmwareInfo::default(),
        features: DeviceFeatures::default(),
        features_initialized: false,
    };
    device.dashboard.local_snapshot = true;
    apply_preview_names(&mut device);
    Some(device)
}

pub fn mouse_mat_preview(pid: u32) -> Option<Device> {
    razer_catalog::audited_mouse_mat(pid)?;
    let mut device = registered_preview(pid)?;
    device.category = DeviceCategory::Mousepad;
    device.profiles[0].settings = Some(crate::settings::ProfileSettings::for_product(pid));
    Some(device)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_registered_preview_preserves_product_identity_without_device_observations() {
        for product in razer_catalog::registry() {
            let device = mouse_mat_preview(product.id())
                .or_else(|| registered_preview(product.id()))
                .unwrap();
            assert_eq!(device.product_id, product.id());
            assert_eq!(device.real_product_id, product.id());
            assert_eq!(device.setup_status, SetupStatus::Unknown);
            assert!(device.dashboard.local_snapshot);
            assert!(device.dashboard.connection_observation.is_none());
            assert!(device.dashboard.readonly_values.is_none());
            assert!(device.dashboard.readonly_keyboard_metadata.is_none());
            assert!(device.firmware_info.current_fw_version.is_empty());
            assert!(device.power_status.is_none());
            assert!(device.dkm_keys.is_empty());
            assert!(!device.features_initialized);
            assert!(!device.is_chroma_device);
            assert!(!device.has_battery);
            assert!(device.ui_window_name.is_empty());
            assert!(device.mw_window_name.is_empty());
        }
        assert!(registered_preview(9001).is_none());
    }
}
