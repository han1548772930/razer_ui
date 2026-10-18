//! 设备与配置的数据模型。
//!
//! 持久化配置、产品身份和真实连接观察分别保存，见 `docs/README.md`。
//!
//! 关键点：
//! - 设备名称是按语区索引的 i18n 对象。
//! - DPI 档位挂在 **profile** 上，不在设备上。
//! - `dkmKeys` 是设备的物理输入点 → 动作映射表。
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
    pub fn exact(&self, locale: &str) -> Option<&str> {
        let normalized = locale.replace('_', "-");
        self.values
            .get(locale)
            .or_else(|| {
                self.values.iter().find_map(|(key, value)| {
                    key.replace('_', "-")
                        .eq_ignore_ascii_case(&normalized)
                        .then_some(value)
                })
            })
            .map(String::as_str)
    }

    /// 按语言取文本，缺失时逐级回退：目标语言 → en → 任意一个非空值。
    pub fn get(&self, locale: &str) -> &str {
        self.exact(locale)
            .filter(|s| !s.is_empty())
            .or_else(|| self.exact("en").filter(|s| !s.is_empty()))
            .or_else(|| {
                self.values
                    .values()
                    .find(|s| !s.is_empty())
                    .map(String::as_str)
            })
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    /// Per-product source profile, independent from the original ten adapters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_settings: Option<serde_json::Value>,
    /// Audited local settings; absent in the legacy Vec<Device> store.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<crate::settings::ProfileSettings>,
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
    /// Session provenance, never persisted as a successful runtime observation.
    #[serde(skip)]
    pub local_snapshot: bool,
    /// The local product module has been constructed for this session. This
    /// reproduces setupOnDeviceReady's publication, not hardware I/O success.
    #[serde(skip)]
    pub local_runtime_mounted: bool,
    #[serde(skip)]
    pub connection_observation: Option<DeviceConnectionObservation>,
    /// Actual read results for this connection only, never local profile data.
    #[serde(skip)]
    pub readonly_values: Option<razer_device::device_reads::DeviceReadValues>,
    #[serde(skip)]
    pub readonly_keyboard_metadata:
        Option<razer_device::keyboard_metadata::KeyboardMetadataReading>,
    #[serde(skip)]
    pub readonly_power: Option<PowerStatus>,
    /// Current controller callback observation, independent of query snapshots.
    #[serde(skip)]
    pub controller_power_status: Option<PowerStatus>,
    /// Factory mode observed for this connection; never inferred from saved bindings.
    #[serde(skip)]
    pub readonly_pairing_is_dongle: Option<bool>,
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
    /// Current runtimeData/WindowStorage artwork fields. None is unobserved;
    /// product IDs and local profile settings cannot establish these states.
    #[serde(skip_serializing)]
    pub sidepad_layout: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wristrest_connected: Option<bool>,
    #[serde(skip_serializing)]
    pub is_mat_plugged: Option<bool>,
    #[serde(skip_serializing)]
    pub is_mouse_connected: Option<bool>,
}

/// Session-only evidence. No variant means profiles/firmware are loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceConnectionObservation {
    UsbPresent,
    HidPresent,
    ReceiverPeer(u8),
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
    /// 设备模块的窗口名，使用真实 VID、PID 与容器标识。
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
    /// A source product window is scoped by product and container, not by the
    /// serial number subsequently returned by its initialization queries.
    /// Keep synthetic previews distinct when no physical container is present.
    pub fn workspace_identity(&self) -> String {
        if self.device_container_id.is_empty() {
            format!("{}:{}:", self.product_id, self.serial_number)
        } else {
            format!("{}:{}", self.product_id, self.device_container_id)
        }
    }

    /// Persisted workspaces supply local drafts, not new hardware readings.
    /// Keep identity, capabilities and all local profile/device settings.
    /// Runtime status must be observed again, never inferred from cached enums.
    pub fn begin_local_session(&mut self) {
        crate::source_names::restore_localized_names(self);
        self.dashboard.local_snapshot = true;
        self.dashboard.local_runtime_mounted = false;
        self.dashboard.connection_observation = None;
        self.dashboard.readonly_pairing_is_dongle = None;
        self.dashboard.readonly_keyboard_metadata = None;
        self.observe_read_values(None);
        self.clear_controller_input_observations();
        self.setup_status = SetupStatus::Unknown;
        self.power_status = None;
        self.firmware_info = FirmwareInfo::default();
        self.dashboard.no_alive_sign = None;
        self.dashboard.device_power_state = None;
        self.dashboard.device_state = None;
        self.dashboard.device_init_status_fail = None;
        self.dashboard.firmware_update_info = None;
        self.dashboard.firmware_needs_upgrade = None;
        self.dashboard.upgrade_mode = None;
        self.dashboard.is_dynamic_lighting = None;
        self.dashboard.sidepad_layout = None;
        self.dashboard.wristrest_connected = None;
        self.dashboard.is_mat_plugged = None;
        self.dashboard.is_mouse_connected = None;
    }

    pub fn observe_connection(&mut self, observation: Option<DeviceConnectionObservation>) {
        if self.dashboard.connection_observation != observation {
            self.clear_controller_input_observations();
            self.dashboard.readonly_pairing_is_dongle = None;
            self.dashboard.readonly_keyboard_metadata = None;
        }
        self.dashboard.connection_observation = observation;
        self.refresh_runtime_setup();
        if observation.is_none() {
            self.clear_controller_input_observations();
            self.dashboard.readonly_keyboard_metadata = None;
            self.observe_read_values(None);
        }
    }

    /// Called by the retained product owner after constructing its page and
    /// subscriptions. The source publishes READY before profile/GET tasks;
    /// this must not manufacture a profile, power observation, or write ACK.
    pub fn mount_local_runtime(&mut self) {
        self.dashboard.local_runtime_mounted = true;
        self.refresh_runtime_setup();
    }

    fn refresh_runtime_setup(&mut self) {
        if self.dashboard.connection_observation.is_none() {
            self.setup_status = SetupStatus::Unknown;
            self.dashboard.local_snapshot = true;
        } else if let Some(initial) = crate::runtime_setup::initial_setup_status(self.product_id) {
            self.setup_status = if self.dashboard.local_runtime_mounted {
                SetupStatus::Ready
            } else {
                initial
            };
            self.dashboard.local_snapshot = false;
        }
    }

    pub fn observe_keyboard_metadata(
        &mut self,
        reading: Option<razer_device::keyboard_metadata::KeyboardMetadataReading>,
    ) {
        if let Some(reading) = &reading {
            let (edition, layout) = crate::source_identity::variant(
                self.product_id,
                u32::from(reading.edition_id),
                u32::from(reading.keyboard_layout),
                self.layout_id,
            );
            crate::source_names::apply_observed_variant(self, edition, layout);
        }
        self.dashboard.readonly_keyboard_metadata = reading;
    }

    pub fn observe_read_values(
        &mut self,
        values: Option<razer_device::device_reads::DeviceReadValues>,
    ) {
        // A battery level alone cannot supply a charging state. Keep the
        // original partial results, and expose the power UI only when both read.
        self.dashboard.readonly_power = values.as_ref().and_then(|values| {
            Some(PowerStatus {
                level: i32::from(values.battery_percent?),
                charging_status: values.charging_status.clone()?,
            })
        });
        self.dashboard.readonly_values = values;
    }

    /// Current host runtimeData owns artwork. These observations update neither
    /// profiles nor local draft settings; absent queries stay unobserved.
    pub fn observe_mouse_runtime_artwork(
        &mut self,
        update: Option<&razer_device::mouse_runtime_artwork::Update>,
    ) {
        use razer_device::mouse_runtime_artwork::{self as artwork, Update};
        if artwork::source(self.product_id).is_none() {
            return;
        }
        match update {
            Some(Update::Sidepad { status, .. }) => {
                self.dashboard.sidepad_layout = status.map(serde_json::Value::from);
            }
            Some(Update::Combo {
                mat_plugged,
                mouse_connected,
                ..
            }) => {
                if let Some(plugged) = mat_plugged {
                    self.dashboard.is_mat_plugged = Some(*plugged);
                }
                if let Some(connected) = mouse_connected {
                    self.dashboard.is_mouse_connected = Some(*connected);
                }
            }
            Some(Update::Diagnostic { .. }) => {}
            None => {
                self.dashboard.sidepad_layout = None;
                self.dashboard.is_mat_plugged = None;
                self.dashboard.is_mouse_connected = None;
            }
        }
    }

    pub fn current_power_status(&self) -> Option<&PowerStatus> {
        if let Some(power) = &self.dashboard.controller_power_status {
            Some(power)
        } else if self.dashboard.readonly_values.is_some() {
            self.dashboard.readonly_power.as_ref()
        } else {
            self.power_status.as_ref()
        }
    }

    pub fn current_firmware_version(&self) -> &str {
        match &self.dashboard.readonly_values {
            Some(values) => values.firmware.as_deref().unwrap_or(""),
            None => &self.firmware_info.current_fw_version,
        }
    }

    pub fn display_name(&self) -> String {
        self.display_name_for_locale(&razer_i18n::locale())
    }

    pub fn dashboard_name_for_locale(&self, locale: &str) -> &str {
        let locale = crate::source_names::product_locale(locale);
        self.name
            .exact(locale)
            .filter(|name| !name.is_empty())
            .or_else(|| crate::source_names::legacy_dashboard_name(self, locale))
            .or_else(|| {
                self.dashboard
                    .base_product_name
                    .as_deref()
                    .filter(|name| !name.is_empty())
            })
            .or_else(|| self.name.exact("en").filter(|name| !name.is_empty()))
            .unwrap_or("")
    }

    pub fn display_name_for_locale(&self, locale: &str) -> String {
        let name = self.dashboard_name_for_locale(locale);
        if !name.is_empty() {
            return name.to_owned();
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
            .or(if self.profiles.is_empty() { None } else { Some(0) })?;
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
    razer_i18n::t_or(key, action)
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
    fn display_name_uses_requested_source_locale() {
        let device = &crate::test_devices::mouse();
        assert_eq!(
            device.display_name_for_locale("zh-CN"),
            "Razer 炼狱蝰蛇 V3 专业版"
        );
        assert_eq!(device.display_name_for_locale("en"), device.name.get("en"));
        assert_eq!(device.display_name_for_locale("ja"), device.name.get("en"));
    }

    #[test]
    fn localized_text_matches_case_and_separator_aliases() {
        let text = localized(&[("zh-CN", "中文"), ("EN", "English")]);
        assert_eq!(text.exact("zh-cn"), Some("中文"));
        assert_eq!(text.get("ZH_cn"), "中文");
        assert_eq!(text.get("de"), "English");
        assert_eq!(text.exact("de"), None);
    }

    #[test]
    fn keyboard_metadata_is_an_ephemeral_read_with_persistable_variant_identity() {
        let mut device = crate::test_devices::mouse();
        let before_profiles = serde_json::to_value(&device.profiles).unwrap();
        device.observe_keyboard_metadata(Some(
            razer_device::keyboard_metadata::KeyboardMetadataReading {
                keyboard_layout: 3,
                edition_id: 128,
                firmware_id: Some(7),
            },
        ));
        assert_eq!((device.edition_id, device.layout_id), (128, 3));
        assert_eq!(
            device
                .dashboard
                .readonly_keyboard_metadata
                .as_ref()
                .unwrap()
                .firmware_id,
            Some(7)
        );
        assert_eq!(
            serde_json::to_value(&device.profiles).unwrap(),
            before_profiles
        );
        let mut restored: Device =
            serde_json::from_value(serde_json::to_value(&device).unwrap()).unwrap();
        assert!(restored.dashboard.readonly_keyboard_metadata.is_none());
        assert_eq!((restored.edition_id, restored.layout_id), (128, 3));
        restored.begin_local_session();
        assert!(restored.dashboard.readonly_keyboard_metadata.is_none());
        device.observe_connection(None);
        assert!(device.dashboard.readonly_keyboard_metadata.is_none());
        assert_eq!((device.edition_id, device.layout_id), (128, 3));
    }

    /// `clamp_dpi` 必须同时夹取范围**并**对齐步进。
    #[test]
    fn clamp_dpi_clamps_and_snaps_to_step() {
        let device = &crate::test_devices::mouse();
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
    fn stored_runtime_values_are_not_current_observations() {
        let mut device = crate::test_devices::mouse();
        device.power_status.as_mut().unwrap().level = 47;
        device.power_status.as_mut().unwrap().charging_status = "NoCharge_BatteryFull".into();
        let profile_id = device.active_profile.clone();
        device.begin_local_session();
        assert!(device.power_status.is_none());
        assert_eq!(device.setup_status, SetupStatus::Unknown);
        assert!(device.dashboard.local_snapshot);
        assert_eq!(device.active_profile, profile_id);
    }

    #[test]
    fn queried_values_never_change_saved_drafts_and_expire_with_connection() {
        let mut device = crate::test_devices::mouse();
        device.begin_local_session();
        let saved = serde_json::to_value(&device).unwrap();
        device.observe_read_values(Some(razer_device::device_reads::DeviceReadValues {
            firmware: Some("2.1.0.0".into()),
            battery_percent: Some(47),
            charging_status: Some("NOT_CHARGING".into()),
            polling_hz: Some(4000),
            dpi: Some((800, 1600)),
            ..Default::default()
        }));
        assert_eq!(device.current_firmware_version(), "2.1.0.0");
        assert_eq!(device.current_power_status().unwrap().level, 47);
        assert_eq!(serde_json::to_value(&device).unwrap(), saved);
        device.observe_connection(None);
        assert!(device.current_firmware_version().is_empty());
        assert!(device.current_power_status().is_none());
        assert!(device.dashboard.readonly_values.is_none());
        assert_eq!(serde_json::to_value(&device).unwrap(), saved);
    }

    #[test]
    fn fill_defaults_is_idempotent() {
        let mut device = crate::test_devices::mouse();
        device.features_initialized = false;
        device.fill_defaults();
        let first = device.features.performance.polling_rate;
        device.fill_defaults();
        assert_eq!(device.features.performance.polling_rate, first);
        assert!(device.features_initialized);
    }

    #[test]
    fn mouse_runtime_artwork_preserves_drafts_and_is_never_serialized() {
        use razer_device::mouse_runtime_artwork::{Origin, Update};
        let mut device = crate::test_devices::mouse();
        device.product_id = 143;
        device.begin_local_session();
        let saved = serde_json::to_value(&device).unwrap();
        let profiles = serde_json::to_value(&device.profiles).unwrap();
        device.observe_mouse_runtime_artwork(Some(&Update::Sidepad {
            origin: Origin::InitialQuery,
            status: Some(4),
            side_panel_name: "6ButtonSide".into(),
            panel_module_id: 4,
        }));
        assert_eq!(device.dashboard.sidepad_layout, Some(serde_json::json!(4)));
        assert_eq!(serde_json::to_value(&device).unwrap(), saved);
        device.product_id = 104;
        device.observe_mouse_runtime_artwork(Some(&Update::Combo {
            origin: Origin::InitialQuery,
            mat_plugged: Some(true),
            mouse_connected: Some(true),
        }));
        device.observe_mouse_runtime_artwork(Some(&Update::Combo {
            origin: Origin::HardwareNotification,
            mat_plugged: None,
            mouse_connected: Some(false),
        }));
        assert_eq!(device.dashboard.is_mat_plugged, Some(true));
        assert_eq!(device.dashboard.is_mouse_connected, Some(false));
        let document = serde_json::to_value(&device).unwrap();
        for key in ["sidepadLayout", "isMatPlugged", "isMouseConnected"] {
            assert!(document["dashboard"].get(key).is_none());
        }
        assert_eq!(serde_json::to_value(&device.profiles).unwrap(), profiles);
        device.observe_mouse_runtime_artwork(Some(&Update::Diagnostic {
            message: "read failed".into(),
        }));
        assert_eq!(device.dashboard.is_mouse_connected, Some(false));
        device.observe_mouse_runtime_artwork(None);
        assert!(device.dashboard.sidepad_layout.is_none());
        assert!(device.dashboard.is_mat_plugged.is_none());
        assert!(device.dashboard.is_mouse_connected.is_none());

        let mut legacy = document;
        legacy["dashboard"]["sidepadLayout"] = serde_json::json!(3);
        legacy["dashboard"]["isMatPlugged"] = serde_json::json!(false);
        legacy["dashboard"]["isMouseConnected"] = serde_json::json!(true);
        let mut restored: Device = serde_json::from_value(legacy).unwrap();
        assert_eq!(
            restored.dashboard.sidepad_layout,
            Some(serde_json::json!(3))
        );
        assert_eq!(restored.dashboard.is_mat_plugged, Some(false));
        assert_eq!(restored.dashboard.is_mouse_connected, Some(true));
        restored.begin_local_session();
        assert!(restored.dashboard.sidepad_layout.is_none());
        assert!(restored.dashboard.is_mat_plugged.is_none());
        assert!(restored.dashboard.is_mouse_connected.is_none());
        assert_eq!(serde_json::to_value(&restored.profiles).unwrap(), profiles);
    }

    #[test]
    fn chroma_capability_does_not_imply_a_battery() {
        let mut mat = crate::product_preview::mouse_mat_preview(3076).unwrap();
        mat.is_chroma_device = true;
        mat.features_initialized = false;
        mat.fill_defaults();
        assert!(!mat.features.lighting.is_empty());
        assert!(mat.features.power.is_none());
        assert!(mat.features.keyboard.is_none());

        let mut mouse = crate::test_devices::mouse();
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

    #[test]
    fn test_snapshot_is_explicitly_ready_without_a_connection_observation() {
        let device = crate::test_devices::mouse();
        assert_eq!(device.setup_status, SetupStatus::Ready);
        assert!(device.dashboard.connection_observation.is_none());
    }
}
