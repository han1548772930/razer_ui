use crate::domain::{
    BossKey, Calibration, DeviceFeatures, LightingEffect, LightingZone, Pairing, PowerManagement,
    Scrolling,
};
use crate::model::{
    Device, DkmKey, DpiStage, DpiStages, PowerStatus, Profile, SetupStatus, localized,
};
use crate::product_preview::registered_preview;

pub fn with_profile(pid: u32) -> Device {
    let mut device = registered_preview(pid).unwrap();
    device.active_profile = "test-profile".into();
    device.profiles = vec![Profile {
        id: device.active_profile.clone(),
        guid: device.active_profile.clone(),
        name: "Test profile".into(),
        settings: None,
        source_settings: None,
        dpi_stages: None,
    }];
    device
}

pub fn mouse() -> Device {
    let mut device = registered_preview(182).unwrap();
    device.serial_number = "TEST-MOUSE".into();
    device.device_container_id = "test-mouse".into();
    device.setup_status = SetupStatus::Ready;
    device.name = localized(&[
        ("en", "Razer DeathAdder V3 Pro"),
        ("zh-cn", "Razer 炼狱蝰蛇 V3 专业版"),
    ]);
    device.product_name = device.name.clone();
    device.active_profile = "profile-1".into();
    device.profiles = vec![Profile {
        id: device.active_profile.clone(),
        guid: device.active_profile.clone(),
        name: "HL-Default".into(),
        settings: None,
        source_settings: None,
        dpi_stages: Some(DpiStages {
            enable: true,
            stages: [400, 800, 1600, 3200, 6400]
                .into_iter()
                .map(DpiStage::new)
                .collect(),
        }),
    }];
    device.min_dpi = Some(100);
    device.max_dpi = Some(30_000);
    device.dpi_step = Some(50);
    device.has_battery = true;
    device.power_status = Some(PowerStatus {
        level: 100,
        charging_status: "NOT_CHARGING".into(),
    });
    device.firmware_info.current_fw_version = "2.0.3.0".into();
    device.dkm_keys = [
        ("DKM_M_01", "Left Click", 1),
        ("DKM_M_02", "Right Click", 2),
        ("DKM_M_03", "Middle Click", 3),
    ]
    .into_iter()
    .map(|(input_id, button_key, key)| DkmKey {
        input_id: input_id.into(),
        button_key: button_key.into(),
        key,
    })
    .collect();
    device.features = DeviceFeatures {
        lighting: vec![
            LightingZone::new("标志", LightingEffect::Static, [0x44, 0xd6, 0x2c]),
            LightingZone::new("滚轮", LightingEffect::SpectrumCycling, [0x44, 0xd6, 0x2c]),
        ],
        power: Some(PowerManagement::default()),
        scrolling: Some(Scrolling::default()),
        calibration: Some(Calibration::default()),
        pairing: Some(Pairing::default()),
        boss_key: Some(BossKey::default()),
        ..Default::default()
    };
    device.features_initialized = true;
    device
}
