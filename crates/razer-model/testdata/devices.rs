use crate::model::{Device, DkmKey, DpiStages, PowerStatus, SetupStatus, localized};
use crate::product_preview::registered_preview;

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
    device.profiles[0].id = device.active_profile.clone();
    device.profiles[0].guid = device.active_profile.clone();
    device.profiles[0].name = "HL-Default".into();
    device.profiles[0].dpi_stages = Some(DpiStages::mouse_default());
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
    device.fill_defaults();
    device
}
