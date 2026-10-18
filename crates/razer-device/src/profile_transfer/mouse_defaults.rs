//! Current PID182 MW3298 profile, device-cache and DKM defaults.
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Serialize)]
struct Dpi {
    x: u32,
    y: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LedPower {
    idle_state: u8,
    idle_value: u8,
}

#[derive(Serialize)]
struct Stage {
    x: u32,
    y: u32,
    independent: bool,
    visible: bool,
}

#[derive(Serialize)]
struct Stages {
    stages: [Stage; 5],
    active: u8,
    enable: bool,
    count: u8,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Mouse182Profile {
    guid: String,
    name: String,
    dpi: Dpi,
    led_power_settings: LedPower,
    low_power_mode: u8,
    mappings: Vec<Value>,
    dpi_stages: Stages,
    polling_rate_wireless: u32,
    polling_rate: u32,
}

impl Default for Mouse182Profile {
    fn default() -> Self {
        Self {
            guid: String::new(),
            name: String::new(),
            dpi: Dpi { x: 800, y: 800 },
            led_power_settings: LedPower {
                idle_state: 1,
                idle_value: 60,
            },
            low_power_mode: 30,
            mappings: Vec::new(),
            dpi_stages: Stages {
                stages: [400, 800, 1600, 3200, 6400].map(|dpi| Stage {
                    x: dpi,
                    y: dpi,
                    independent: false,
                    visible: true,
                }),
                active: 3,
                enable: true,
                count: 5,
            },
            polling_rate_wireless: 1000,
            polling_rate: 1000,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SmartTracking {
    is_asymmetric: bool,
    tracking_distance: u8,
    lift_off_distance: u8,
    landing_distance: u8,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PresetTracking {
    is_asymmetric: bool,
    surface_distance: u8,
    lift_off_distance: u8,
    landing_distance: u8,
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum CalibrationKind {
    Default,
}

#[derive(Serialize)]
struct CalibrationProfile {
    #[serde(rename = "type")]
    kind: CalibrationKind,
}

#[derive(Serialize)]
struct EmptyObject {}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Calibration {
    calibration_tracking_mode: bool,
    lift_off_range_value: u8,
    smart_tracking: SmartTracking,
    preset_tracking: PresetTracking,
    profiles: [CalibrationProfile; 1],
    selected_profile: EmptyObject,
}

#[derive(Serialize)]
enum DeviceCategory {
    #[serde(rename = "MOUSE")]
    Mouse,
}

#[derive(Serialize)]
enum ReportKind {
    #[serde(rename = "razerKeyReportID")]
    RazerKey,
    #[serde(rename = "hardwareEventReportID")]
    HardwareEvent,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Mouse182Device {
    version: u64,
    vendor_id: u16,
    product_id: u16,
    category: DeviceCategory,
    #[serde(rename = "reportIDs")]
    report_ids: BTreeMap<u8, ReportKind>,
    calibration: Calibration,
    low_power_mode: u8,
}

impl Default for Mouse182Device {
    fn default() -> Self {
        Self {
            version: 1,
            vendor_id: 5426,
            product_id: 182,
            category: DeviceCategory::Mouse,
            report_ids: BTreeMap::from([(4, ReportKind::RazerKey), (5, ReportKind::HardwareEvent)]),
            calibration: Calibration {
                calibration_tracking_mode: false,
                lift_off_range_value: 3,
                smart_tracking: SmartTracking {
                    is_asymmetric: false,
                    tracking_distance: 2,
                    lift_off_distance: 13,
                    landing_distance: 12,
                },
                preset_tracking: PresetTracking {
                    is_asymmetric: false,
                    surface_distance: 5,
                    lift_off_distance: 13,
                    landing_distance: 12,
                },
                profiles: [CalibrationProfile {
                    kind: CalibrationKind::Default,
                }],
                selected_profile: EmptyObject {},
            },
            low_power_mode: 30,
        }
    }
}

/// The fixed profile before the source's computer-name and UUID assignment.
pub fn default_profile_182() -> Value {
    serde_json::to_value(Mouse182Profile::default()).expect("typed mouse profile")
}

/// The fixed root before adding a profile and selecting its GUID.
pub fn default_device_182() -> Value {
    serde_json::to_value(Mouse182Device::default()).expect("typed mouse device")
}

#[derive(Serialize)]
struct DkmKey {
    #[serde(rename = "inputID")]
    input_id: &'static str,
    #[serde(rename = "buttonKey")]
    button_key: &'static str,
    key: u8,
}

pub(super) fn source_dkm_keys_182() -> BTreeMap<String, Value> {
    [
        DkmKey {
            input_id: "DKM_M_01",
            button_key: "SensitivityStageUp",
            key: 32,
        },
        DkmKey {
            input_id: "DKM_M_02",
            button_key: "SensitivityStageDown",
            key: 33,
        },
        DkmKey {
            input_id: "DKM_SB_03",
            button_key: "CycleUpSensitivityStages",
            key: 82,
        },
    ]
    .into_iter()
    .map(|key| {
        (
            key.input_id.into(),
            serde_json::to_value(key).expect("typed mouse DKM key"),
        )
    })
    .collect()
}
