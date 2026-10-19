use razer_model::domain::{
    BossKey, Calibration, DeviceFeatures, KeyboardSettings, LightingEffect, LightingZone, Pairing,
    Performance, PowerManagement, Scrolling,
};
use razer_model::model::{
    Device, DkmKey, DpiStage, DpiStages, PowerStatus, Profile, SetupStatus, localized,
};
use razer_model::product_preview::registered_preview;

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

pub fn keyboard() -> Device {
    let mut device = registered_preview(653).unwrap();
    device.active_profile = "keyboard-profile".into();
    device.profiles = vec![Profile {
        id: device.active_profile.clone(),
        guid: device.active_profile.clone(),
        name: "Keyboard test profile".into(),
        settings: Some(razer_model::settings::ProfileSettings::for_product(653)),
        source_settings: None,
        dpi_stages: None,
    }];
    device.layout_id = 1;
    device.dkm_keys = [
        ("DKM_K_01", 30),
        ("DKM_K_02", 17),
        ("DKM_K_03", 31),
        ("DKM_K_04", 32),
        ("DKM_K_05", 59),
        ("DKM_K_06", 57),
        ("DKM_K_07", 91),
    ]
    .into_iter()
    .map(|(input_id, key)| DkmKey {
        input_id: input_id.into(),
        button_key: "Default".into(),
        key,
    })
    .collect();
    device.is_chroma_device = true;
    device.features = DeviceFeatures {
        performance: Performance::for_keyboard(),
        lighting: vec![LightingZone::new(
            "整块键盘",
            LightingEffect::Wave,
            [0x44, 0xd6, 0x2c],
        )],
        keyboard: Some(KeyboardSettings::default()),
        scrolling: Some(Scrolling::default()),
        ..Default::default()
    };
    device.features_initialized = true;
    device
}
