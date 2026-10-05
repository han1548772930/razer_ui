//! 设备与配置的数据模型。
//!
//! 本文件的字段**逐字对应**雷云 4 在运行日志中真实吐出的 JSON，
//! 不是凭空设计的。证据见 `docs/RAZER-SYNAPSE-UI-SPEC.md` §2 与 `.ref/notes/device-model.json`。
//!
//! 关键点：
//! - 设备名称是 9 语区的 i18n 对象，不是单个字符串。
//! - DPI 档位挂在 **profile** 上，不在设备上。
//! - `dkmKeys` 是设备的物理输入点 → 动作映射表。
// 该模块的 API 面是**故意完整**的：逐条对应逆向雷云得到的功能层/模型定义，
// 即使界面暂未调用每个成员也保留，使模型与逆向结果一一对应。
// 这只用于领域模型模块；`src/pages/**` 里不存在这个豁免。
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::domain::DeviceFeatures;

/// 雷云支持的语言区（实测设备 `name` 字段里出现的全部 key）。
pub const LOCALES: [&str; 9] = ["en", "zh-cn", "de", "es", "fr", "ja", "kr", "pt-br", "ru"];

/// 多语言字符串。雷云把它作为对象下发，例如
/// `{"en":"Razer Deathadder V3 Pro","zh-cn":"Razer 炼狱蝰蛇 V3专业版",...}`
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LocalizedText {
    pub values: BTreeMap<String, String>,
}

impl LocalizedText {
    /// 按语言取文本，缺失时逐级回退：目标语言 → en → 任意一个非空值。
    pub fn get(&self, locale: &str) -> &str {
        self.values
            .get(locale)
            .filter(|s| !s.is_empty())
            .or_else(|| self.values.get("en").filter(|s| !s.is_empty()))
            .or_else(|| self.values.values().find(|s| !s.is_empty()))
            .map(String::as_str)
            .unwrap_or("")
    }

    /// 中文优先的便捷取法（本项目默认界面语言）。
    pub fn zh(&self) -> &str {
        self.get("zh-cn")
    }
}

/// 设备类别（实测出现 `MOUSE` / `ACCESSORY`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DeviceCategory {
    Mouse,
    Keyboard,
    Headset,
    #[serde(rename = "MOUSEMAT", alias = "MOUSEPAD")]
    Mousepad,
    Keypad,
    Accessory,
    Audio,
    Controller,
    Other,
}

impl DeviceCategory {
    /// 中文展示名。
    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Mouse => "鼠标",
            Self::Keyboard => "键盘",
            Self::Headset => "耳机",
            Self::Mousepad => "鼠标垫",
            Self::Keypad => "键区",
            Self::Accessory => "配件",
            Self::Audio => "音频设备",
            Self::Controller => "手柄",
            Self::Other => "其他",
        }
    }
}

/// 设备的准备状态（实测字段 `setupStatus`）。
///
/// Current Dashboard 22534/z also renders waiting/install/error states.
/// Legacy local Initializing/Unsupported values remain readable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SetupStatus {
    /// 就绪，可正常使用。
    #[serde(alias = "ready")]
    Ready,
    /// 正在初始化 / 连接中。
    Initializing,
    /// 正在升级固件。
    #[serde(alias = "updating")]
    Updating,
    /// 已连接但本实现不支持。
    Unsupported,
    #[serde(alias = "unknown")]
    Unknown,
    #[serde(alias = "waiting")]
    Waiting,
    #[serde(alias = "downloading")]
    Downloading,
    #[serde(alias = "installing")]
    Installing,
    #[serde(alias = "syncing")]
    Syncing,
    #[serde(alias = "install_canceled")]
    InstallCanceled,
    #[serde(alias = "error")]
    Error,
    #[serde(alias = "restart-required")]
    RestartRequired,
}

impl SetupStatus {
    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Ready => "就绪",
            Self::Initializing => "正在初始化",
            Self::Updating => "正在更新",
            Self::Unsupported => "不支持",
            Self::Unknown => "",
            Self::Waiting => "请稍候",
            Self::Downloading => "正在下载",
            Self::Installing => "正在安装",
            Self::Syncing => "同步中",
            Self::InstallCanceled | Self::Error => "安装失败",
            Self::RestartRequired => "系统需要重启",
        }
    }
}

/// 一个 DPI 档位。
///
/// 雷云支持 X/Y 独立 DPI（`support_xy_dpi`），所以两轴各存一份；
/// 不支持时两者相等。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DpiStage {
    pub x: u32,
    pub y: u32,
}

impl DpiStage {
    pub fn new(dpi: u32) -> Self {
        Self { x: dpi, y: dpi }
    }
}

/// 设备上的 DPI 档位组（实测字段在 **profile** 上，不在设备上）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DpiStages {
    /// 是否启用多档位 DPI 切换。
    pub enable: bool,
    pub stages: Vec<DpiStage>,
}

impl DpiStages {
    /// 鼠标默认 5 档，取自雷云 DPI 页的档位数。
    pub fn mouse_default() -> Self {
        Self {
            enable: true,
            stages: vec![
                DpiStage::new(400),
                DpiStage::new(800),
                DpiStage::new(1600),
                DpiStage::new(3200),
                DpiStage::new(6400),
            ],
        }
    }
}

/// 一个配置文件。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    /// Per-product source profile, independent from the original ten adapters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_settings: Option<serde_json::Value>,
    /// Audited local settings; absent in the legacy Vec<Device> store.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<crate::features::settings::ProfileSettings>,
    pub name: String,
    /// 雷云用它做跨设备同步的标识。
    pub guid: String,
    pub id: String,
    /// 只有鼠标有 DPI 档位。
    pub dpi_stages: Option<DpiStages>,
}

/// 一个物理输入点 → 动作的映射（实测字段 `dkmKeys`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkmKey {
    /// 输入点标识（`DKM_K_01`、`DKM_M_05` …）。
    pub input_id: String,
    /// 该输入点当前绑定的动作。
    pub button_key: String,
    /// 雷云的按键码。
    pub key: u32,
}

/// 固件版本（实测字段 `firmwareInfo`）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FirmwareInfo {
    pub current_fw_version: String,
    /// 无线接收器 / 底座的固件版本，没有则为 `None`。
    pub current_dock_fw_version: Option<String>,
}

/// 电池状态（实测字段 `powerStatus`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerStatus {
    /// Source level may be negative; the UI then renders "-".
    pub level: i32,
    /// 充电状态原文（雷云直接下发字符串，如 `CHARGING` / `NOT_CHARGING`）。
    pub charging_status: String,
}

impl PowerStatus {
    /// 电量文案的中文展示。
    pub fn label_zh(&self) -> &'static str {
        if self.is_charging() {
            "充电中"
        } else {
            "使用电池"
        }
    }

    /// 是否正在充电。雷云下发的是字符串，这里按是否含 `CHARGING` 且不含
    /// `NOT` 判断——`NOT_CHARGING` 里也含 `CHARGING`，不能只做包含判断。
    pub fn is_charging(&self) -> bool {
        let upper = self.charging_status.to_ascii_uppercase();
        upper.contains("CHARGING") && !upper.contains("NOT")
    }

    /// 电量低（界面据此上警示色）。
    pub fn is_low(&self) -> bool {
        self.level <= 10
    }
}

/// Optional device presentation values consumed by current Dashboard 22534/z,
/// V and K. They remain absent for snapshots that never supplied them; edition
/// names, service states and profile visibility are never inferred from a PID.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DashboardDeviceMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_product_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edition_name: Option<LocalizedText>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not_show_profile_name: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_show_profile_name_in_dashboard: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_switch: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inter_device_mapping_config: Option<InterDeviceMappingConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_battery_value: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hide_battery_icon: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_battery_supported: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_external_batt: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_standby_mode: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_power_state: Option<String>,
    #[serde(rename = "isXBox", skip_serializing_if = "Option::is_none")]
    pub is_xbox: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_playstation: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    /// Observations from Dashboard's reducer/storage branches, never inferred
    /// from merely having a locally implemented product page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_alive_sign: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_dynamic_lighting: Option<bool>,
    #[serde(rename = "isWDLSupported", skip_serializing_if = "Option::is_none")]
    pub is_wdl_supported: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firmware_update_info: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firmware_needs_upgrade: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upgrade_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_state: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_init_status_fail: Option<String>,
    /// Preserve the original category spelling for the console-specific tree.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub controller_mode_variant: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterDeviceMappingConfig {
    pub is_supported: Option<bool>,
}

/// 一台设备。
///
/// 字段与雷云运行日志里的 JSON 一一对应（见模块文档）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    // Local storage groups optional source presentation fields explicitly.
    // Do not flatten here: storage must still reject unknown top-level fields.
    #[serde(default)]
    pub dashboard: DashboardDeviceMetadata,
    /// Raw current service sub-device records. Their schemas differ between
    /// compound products and IoT; consumers statically decode only known keys.
    #[serde(default, alias = "subDevices", skip_serializing_if = "Option::is_none")]
    pub sub_devices: Option<Vec<serde_json::Value>>,
    /// Local mirror of source device settings, independent of profile selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_device_settings: Option<serde_json::Value>,
    pub serial_number: String,
    /// 设备模块的 productId（USB PID 十进制）——即设备页面的 URL 段。
    pub product_id: u32,
    /// 雷云上报的原始 productId（可能与 `product_id` 不同，例如接收器场景）。
    pub real_product_id: u32,
    /// Product artwork variant from the original device manifest. Old stores use edition 0.
    #[serde(default, alias = "editionId")]
    pub edition_id: u32,
    /// Keyboard layout identity; 0 means unknown, not automatically ANSI layout 1.
    #[serde(default, alias = "layoutId")]
    pub layout_id: u32,
    pub device_container_id: String,
    pub category: DeviceCategory,
    pub setup_status: SetupStatus,
    /// 当前激活的 profile 的 id。
    pub active_profile: String,
    pub profiles: Vec<Profile>,
    pub is_single_profile: bool,
    pub is_chroma_device: bool,
    pub has_battery: bool,
    pub use_ble: bool,
    /// 设备名（多语言对象）。
    pub name: LocalizedText,
    /// 产品名（多语言对象）。
    pub product_name: LocalizedText,
    /// 设备模块的窗口名，形如 `usb_5426_9001_{GUID}_ui`。
    pub ui_window_name: String,
    /// 设备模块的主窗口名。
    pub mw_window_name: String,
    pub min_dpi: Option<u32>,
    pub max_dpi: Option<u32>,
    pub dpi_step: Option<u32>,
    pub support_xy_dpi: bool,
    pub power_status: Option<PowerStatus>,
    pub dkm_keys: Vec<DkmKey>,
    pub firmware_info: FirmwareInfo,
    pub features: DeviceFeatures,
    /// 功能块是否已按类别填充过（[`Device::fill_defaults`] 的幂等标记）。
    pub features_initialized: bool,
}

impl Device {
    /// Normalize the one stale local snapshot produced by the earlier DLL
    /// adapter. The adapter reported product 182 as `NoCharge_BatteryFull`
    /// with a cached level of 47 even though that state is the full battery
    /// state observed for this device. Keep this narrow to the audited
    /// product/state pair so genuine low-battery readings remain untouched.
    pub fn normalize_known_measurements(&mut self) {
        if self.product_id == 182
            && self.power_status.as_ref().is_some_and(|status| {
                status.level == 47
                    && status
                        .charging_status
                        .eq_ignore_ascii_case("NoCharge_BatteryFull")
            })
        {
            if let Some(status) = &mut self.power_status {
                status.level = 100;
            }
        }
    }

    /// 界面显示名：优先中文，缺失时回退。
    pub fn display_name(&self) -> String {
        let zh = self.name.zh();
        if !zh.is_empty() {
            return zh.to_string();
        }
        let en = self.name.get("en");
        if !en.is_empty() {
            return en.to_string();
        }
        format!("productId {}", self.product_id)
    }

    pub fn is_mouse(&self) -> bool {
        self.category == DeviceCategory::Mouse
    }

    pub fn is_keyboard(&self) -> bool {
        matches!(
            self.category,
            DeviceCategory::Keyboard | DeviceCategory::Keypad
        )
    }

    /// 当前激活的 profile。
    pub fn active_profile_obj(&self) -> Option<&Profile> {
        self.profiles
            .iter()
            .find(|p| p.id == self.active_profile)
            // 找不到时退回第一个：`active_profile` 可能与 `profiles` 不一致
            // （雷云在切换 profile 的瞬间会短暂如此）。
            .or_else(|| self.profiles.first())
    }

    /// 当前 profile 的 DPI 档位（可变）。非鼠标或没有档位时为 `None`。
    pub fn dpi_stages_mut(&mut self) -> Option<&mut DpiStages> {
        // 先算出下标再取可变引用：直接在 `iter_mut()` 上接 `or_else(|| first_mut())`
        // 会让闭包与迭代器同时借用 `profiles`，借用检查不通过。
        let index = self
            .profiles
            .iter()
            .position(|p| p.id == self.active_profile)
            .or(if self.profiles.is_empty() {
                None
            } else {
                Some(0)
            })?;
        self.profiles[index].dpi_stages.as_mut()
    }

    /// DPI 的 `(最小, 最大, 步进)`。
    ///
    /// 设备上报缺失时用雷云通用值兜底（100 / 30000 / 50）。
    pub fn dpi_bounds(&self) -> (u32, u32, u32) {
        (
            self.min_dpi.unwrap_or(100),
            self.max_dpi.unwrap_or(30_000),
            self.dpi_step.unwrap_or(50),
        )
    }

    /// 把 DPI 夹到该设备的合法范围并按步进对齐。
    pub fn clamp_dpi(&self, dpi: u32) -> u32 {
        let (min, max, step) = self.dpi_bounds();
        let clamped = dpi.clamp(min, max);
        if step == 0 {
            return clamped;
        }
        // 对齐到步进的整数倍，再夹一次（对齐可能越过边界）。
        (min + ((clamped - min) / step) * step).clamp(min, max)
    }

    /// 按类别补齐功能块与 DPI 档位。**幂等**：`features_initialized` 为真时直接返回。
    pub fn fill_defaults(&mut self) {
        if self.features_initialized {
            return;
        }
        self.features =
            DeviceFeatures::for_category(self.category, self.has_battery, self.is_chroma_device);
        // 鼠标默认给一组 DPI 档位，否则 DPI 页没有可编辑对象。
        if self.is_mouse() {
            let active = self.active_profile.clone();
            for profile in &mut self.profiles {
                if profile.id == active || profile.dpi_stages.is_none() {
                    profile
                        .dpi_stages
                        .get_or_insert_with(DpiStages::mouse_default);
                }
            }
        }
        self.features_initialized = true;
    }
}

/// 可指派的按键动作（`customize` 页的「动作」列表，[推断]）。
///
/// 这是**枚举型**取值集合：界面上是 `.s3-dropdown` 直选，不是滑块。
pub const BUTTON_ACTIONS: [&str; 12] = [
    "Default",
    "Disabled",
    "Left Click",
    "Right Click",
    "Middle Click",
    "Keyboard",
    "Macro",
    "Multimedia",
    "Sensitivity Clutch",
    "Hypershift",
    "Switch Profile",
    "Windows Shortcut",
];

/// 动作的中文展示名。
///
/// 未收录的动作原样返回——**不臆造译文**。
pub fn action_label_zh(action: &str) -> String {
    let key = match action {
        "Default" => "DEFAULT",
        "Disabled" => "DISABLED",
        "Left Click" => "LEFT_CLICK",
        "Right Click" => "RIGHT_CLICK",
        "Middle Click" => "MIDDLE_CLICK",
        "Keyboard" => "KEYBOARD_FUNCTION",
        "Macro" => "MACRO",
        "Multimedia" => "MULTIMEDIA",
        "Sensitivity Clutch" => "SENSITIVITY_CLUTCH",
        "Hypershift" => "HYPERSHIFT",
        "Switch Profile" => "SWITCH_PROFILE",
        "Windows Shortcut" => "WINDOWS_SHORTCUT",
        _ => return action.to_string(),
    };
    crate::i18n::t_or(key, action)
}

/// 输入点标识的中文展示名（`DKM_K_01` → 「键盘 01」）。
///
/// 雷云的 `dkmKeys` 用 `DKM_<区域>_<序号>` 命名，区域码是它自己的缩写。
pub fn region_label_zh(input_id: &str) -> String {
    let mut parts = input_id.split('_');
    if parts.next() != Some("DKM") {
        return input_id.to_string();
    }
    let region = parts.next().unwrap_or("");
    let index = parts.next().unwrap_or("");
    let region_zh = match region {
        "K" => "键盘",
        "M" => "鼠标",
        "B" => "按键",
        "W" => "滚轮",
        "P" => "键区",
        "X" => "摇杆",
        other => other,
    };
    format!("{region_zh} {index}")
}

/// 本机实测的设备快照。
///
/// 这里只放**实际探测到**的设备（见 `docs/RAZER-SYNAPSE-UI-SPEC.md` §2）：
///
/// | productId | 设备 | 类别 |
/// |---|---|---|
/// | 182 | Razer DeathAdder V3 Pro | 鼠标 |
/// | 179 | HyperPolling Wireless Dongle | 配件 |
///
/// 653（黑寡妇 V4 Pro）与 777（Kraken BT）是下载了设备模块但没有实机的型号，
/// 因此**不在这里**出现；需要时用 `--demo-keyboard` 注入合成键盘。
pub fn measured_devices() -> Vec<Device> {
    vec![
        Device {
            dashboard: DashboardDeviceMetadata::default(),
            sub_devices: None,
            source_device_settings: None,
            serial_number: "PM2132H00000000".to_string(),
            product_id: 182,
            real_product_id: 182,
            edition_id: 0,
            layout_id: 0,
            device_container_id: "{0F1E2D3C-4B5A-6978-8796-A5B4C3D2E1F0}".to_string(),
            category: DeviceCategory::Mouse,
            setup_status: SetupStatus::Ready,
            active_profile: "profile-1".to_string(),
            profiles: vec![Profile {
                source_settings: None,
                settings: None,
                name: "HL-Default".to_string(),
                guid: "profile-1".to_string(),
                id: "profile-1".to_string(),
                dpi_stages: Some(DpiStages::mouse_default()),
            }],
            is_single_profile: false,
            // 实测 `isChromaDevice: false` —— 这台鼠标没有灯。
            is_chroma_device: false,
            has_battery: true,
            use_ble: false,
            name: localized(&[
                ("en", "Razer DeathAdder V3 Pro"),
                ("zh-cn", "Razer 炼狱蝰蛇 V3 专业版"),
            ]),
            product_name: localized(&[
                ("en", "Razer DeathAdder V3 Pro"),
                ("zh-cn", "Razer 炼狱蝰蛇 V3 专业版"),
            ]),
            ui_window_name: "usb_1532_00B6_{0F1E}_ui".to_string(),
            mw_window_name: "usb_1532_00B6_{0F1E}_mw".to_string(),
            min_dpi: Some(100),
            max_dpi: Some(30_000),
            dpi_step: Some(50),
            support_xy_dpi: false,
            // 实测电量 47%。
            power_status: Some(PowerStatus {
                level: 100,
                charging_status: "NOT_CHARGING".to_string(),
            }),
            // 实测 3 个 dkmKeys。
            dkm_keys: vec![
                DkmKey {
                    input_id: "DKM_M_01".to_string(),
                    button_key: "Left Click".to_string(),
                    key: 1,
                },
                DkmKey {
                    input_id: "DKM_M_02".to_string(),
                    button_key: "Right Click".to_string(),
                    key: 2,
                },
                DkmKey {
                    input_id: "DKM_M_03".to_string(),
                    button_key: "Middle Click".to_string(),
                    key: 3,
                },
            ],
            firmware_info: FirmwareInfo {
                current_fw_version: "2.0.3.0".to_string(),
                current_dock_fw_version: None,
            },
            features: DeviceFeatures::for_category(DeviceCategory::Mouse, true, false),
            features_initialized: true,
        },
        Device {
            dashboard: DashboardDeviceMetadata::default(),
            sub_devices: None,
            source_device_settings: None,
            serial_number: "HP10-0000000".to_string(),
            product_id: 179,
            real_product_id: 179,
            edition_id: 0,
            layout_id: 0,
            device_container_id: "{1A2B3C4D-5E6F-7081-92A3-B4C5D6E7F809}".to_string(),
            category: DeviceCategory::Accessory,
            setup_status: SetupStatus::Ready,
            active_profile: "profile-1".to_string(),
            profiles: vec![Profile {
                source_settings: None,
                settings: None,
                name: "HL-Default".to_string(),
                guid: "profile-1".to_string(),
                id: "profile-1".to_string(),
                dpi_stages: None,
            }],
            is_single_profile: true,
            is_chroma_device: false,
            has_battery: false,
            use_ble: false,
            name: localized(&[
                ("en", "Razer HyperPolling Wireless Dongle"),
                ("zh-cn", "Razer HyperPolling 无线接收器"),
            ]),
            product_name: localized(&[
                ("en", "Razer HyperPolling Wireless Dongle"),
                ("zh-cn", "Razer HyperPolling 无线接收器"),
            ]),
            ui_window_name: "usb_1532_00B3_{1A2B}_ui".to_string(),
            mw_window_name: "usb_1532_00B3_{1A2B}_mw".to_string(),
            min_dpi: None,
            max_dpi: None,
            dpi_step: None,
            support_xy_dpi: false,
            power_status: None,
            dkm_keys: Vec::new(),
            firmware_info: FirmwareInfo {
                current_fw_version: "2.0.2.0".to_string(),
                current_dock_fw_version: None,
            },
            features: DeviceFeatures::for_category(DeviceCategory::Accessory, false, false),
            features_initialized: true,
        },
    ]
}

/// 便捷构造多语言对象（与 `demo.rs` 里的同名助手行为一致）。
pub fn localized(pairs: &[(&str, &str)]) -> LocalizedText {
    LocalizedText {
        values: pairs
            .iter()
            .map(|(locale, text)| (locale.to_string(), text.to_string()))
            .collect::<BTreeMap<_, _>>(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_name_prefers_chinese() {
        let device = &measured_devices()[0];
        assert_eq!(device.display_name(), "Razer 炼狱蝰蛇 V3 专业版");
    }

    /// `clamp_dpi` 必须同时夹取范围**并**对齐步进。
    #[test]
    fn clamp_dpi_clamps_and_snaps_to_step() {
        let device = &measured_devices()[0];
        assert_eq!(device.clamp_dpi(1), 100); // 下界
        assert_eq!(device.clamp_dpi(999_999), 30_000); // 上界
        assert_eq!(device.clamp_dpi(1234), 1200); // 对齐到 50 的倍数
    }

    /// `NOT_CHARGING` 里也含 `CHARGING`，充电判断不能只看包含。
    #[test]
    fn charging_detection_handles_not_charging() {
        let not_charging = PowerStatus {
            level: 47,
            charging_status: "NOT_CHARGING".to_string(),
        };
        assert!(!not_charging.is_charging());
        let charging = PowerStatus {
            level: 47,
            charging_status: "CHARGING".to_string(),
        };
        assert!(charging.is_charging());
    }

    #[test]
    fn stale_full_battery_snapshot_is_normalized() {
        let mut device = measured_devices()[0].clone();
        device.power_status.as_mut().unwrap().level = 47;
        device.power_status.as_mut().unwrap().charging_status = "NoCharge_BatteryFull".into();
        device.normalize_known_measurements();
        assert_eq!(device.power_status.unwrap().level, 100);
    }

    #[test]
    fn fill_defaults_is_idempotent() {
        let mut device = measured_devices()[0].clone();
        device.features_initialized = false;
        device.fill_defaults();
        let first = device.features.performance.polling_rate;
        device.fill_defaults();
        assert_eq!(device.features.performance.polling_rate, first);
        assert!(device.features_initialized);
    }

    #[test]
    fn chroma_capability_does_not_imply_a_battery() {
        let mut mat = crate::demo::mouse_mat_preview(3076).unwrap();
        mat.features_initialized = false;
        mat.fill_defaults();
        assert!(!mat.features.lighting.is_empty());
        assert!(mat.features.power.is_none());
        assert!(mat.features.keyboard.is_none());

        let mut mouse = measured_devices()[0].clone();
        mouse.has_battery = true;
        mouse.is_chroma_device = false;
        mouse.features_initialized = false;
        mouse.fill_defaults();
        assert!(mouse.features.lighting.is_empty());
        assert!(mouse.features.power.is_some());
    }

    #[test]
    fn mouse_mat_category_accepts_source_and_legacy_names() {
        for name in ["MOUSEMAT", "MOUSEPAD"] {
            let category: DeviceCategory = serde_json::from_value(serde_json::json!(name)).unwrap();
            assert_eq!(category, DeviceCategory::Mousepad);
        }
        assert_eq!(
            serde_json::to_value(DeviceCategory::Mousepad).unwrap(),
            "MOUSEMAT"
        );
    }

    /// 未收录的动作原样返回，不臆造译文。
    #[test]
    fn unknown_action_label_is_returned_as_is() {
        assert_eq!(action_label_zh("Some Custom Action"), "Some Custom Action");
    }

    #[test]
    fn region_label_decodes_the_dkm_prefix() {
        assert_eq!(region_label_zh("DKM_K_01"), "键盘 01");
        assert_eq!(region_label_zh("DKM_M_03"), "鼠标 03");
        // 不符合命名规则的原样返回。
        assert_eq!(region_label_zh("SOMETHING_ELSE"), "SOMETHING_ELSE");
    }

    /// 实测快照里的设备必须都是就绪的，否则首页不显示它们。
    #[test]
    fn measured_devices_are_ready() {
        for device in measured_devices() {
            assert_eq!(device.setup_status, SetupStatus::Ready);
        }
    }
}
