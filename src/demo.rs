//! 演示设备。
//!
//! 本机实测只有鼠标（DeathAdder V3 Pro）与无线接收器，**没有键盘**。
//! 为了让键盘功能可见、可测，`--demo-keyboard` 会注入一台**合成**的雷蛇键盘。
//!
//! ⚠️ 它不是真实设备：productId 用 `9xxx` 段避开雷蛇真实编号，
//! 名称里带「演示」字样，界面上也会显式提示。

use std::collections::BTreeMap;

use crate::features::{DeviceFeatures, LightingEffect, LightingZone};
use crate::model::{
    Device, DeviceCategory, DkmKey, FirmwareInfo, LocalizedText, Profile, SetupStatus,
};

/// 合成键盘的 productId（避开雷蛇真实编号区间）。
pub const DEMO_PRODUCT_ID: u32 = 9001;

/// 构造一台合成键盘。
pub fn demo_keyboard() -> Device {
    let name = localized(&[
        ("en", "Razer BlackWidow V4 Pro (demo)"),
        ("zh-cn", "Razer 黑寡妇蜘蛛 V4 专业版（演示）"),
    ]);

    let dkm_keys = vec![
        dkm("DKM_K_01", "A", 30),
        dkm("DKM_K_02", "W", 17),
        dkm("DKM_K_03", "S", 31),
        dkm("DKM_K_04", "D", 32),
        dkm("DKM_K_05", "F1", 59),
        dkm("DKM_K_06", "空格", 57),
        dkm("DKM_K_07", "Windows 键", 91),
    ];

    let mut features = DeviceFeatures::for_category(DeviceCategory::Keyboard, false, true);
    // 演示用的灯光区域（真实键盘由设备模块下发）。
    features.lighting = vec![LightingZone::new(
        "整块键盘",
        LightingEffect::Wave,
        [0x44, 0xD6, 0x2C],
    )];

    Device {
        serial_number: "DEMO-KEYBOARD-0001".to_string(),
        product_id: DEMO_PRODUCT_ID,
        real_product_id: DEMO_PRODUCT_ID,
        device_container_id: "{DEMO-0000-0000-0000-000000000001}".to_string(),
        category: DeviceCategory::Keyboard,
        setup_status: SetupStatus::Ready,
        active_profile: "demo-profile-kb".to_string(),
        profiles: vec![Profile {
            name: "HL-Default".to_string(),
            guid: "demo-profile-kb".to_string(),
            id: "demo-profile-kb".to_string(),
            dpi_stages: None,
        }],
        is_single_profile: false,
        is_chroma_device: true,
        has_battery: false,
        use_ble: false,
        name: name.clone(),
        product_name: name,
        ui_window_name: "usb_5426_9001_{DEMO}_ui".to_string(),
        mw_window_name: "usb_5426_9001_{DEMO}_mw".to_string(),
        min_dpi: None,
        max_dpi: None,
        dpi_step: None,
        support_xy_dpi: false,
        power_status: None,
        dkm_keys,
        firmware_info: FirmwareInfo {
            current_fw_version: "1.0.0.0".to_string(),
            current_dock_fw_version: None,
        },
        features,
        features_initialized: true,
    }
}

fn dkm(input_id: &str, _label: &str, key: u32) -> DkmKey {
    DkmKey {
        input_id: input_id.to_string(),
        button_key: "Default".to_string(),
        key,
    }
}

fn localized(pairs: &[(&str, &str)]) -> LocalizedText {
    LocalizedText {
        values: pairs
            .iter()
            .map(|(locale, text)| (locale.to_string(), text.to_string()))
            .collect::<BTreeMap<_, _>>(),
    }
}
