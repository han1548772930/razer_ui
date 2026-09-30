//! 鼠标与键盘的功能模型。
//!
//! # 证据等级说明
//!
//! 这些设置**由设备模块在运行时下发**（`products/{id}/ui/{productId}_{edition}/`），
//! 本地快照里只有基础字段（DPI 范围、dkmKeys、电量等）。
//! 因此本文件的默认值属于 `docs/FEATURES.md` 证据分级里的 **[推断]**：
//! 依据雷云公开的产品行为与实测字段命名设定，接上设备模块后应以模块下发值为准。
//!
//! 已经 **[实测]** 的部分：`dpiStages` 结构、`minDPI/maxDPI/dpiStep`、
//! `dkmKeys`、`powerStatus`、`hasBattery`。
// 该模块的 API 面是**故意完整**的：逐条对应逆向雷云得到的功能层/模型定义，
// 即使界面暂未调用每个成员也保留，使模型与逆向结果一一对应。
// 这只用于领域模型模块；`src/pages/**` 里不存在这个豁免。
#![allow(dead_code)]


use serde::{Deserialize, Serialize};

use crate::model::{DeviceCategory, DkmKey};

// ---------------------------------------------------------------------------
// 性能
// ---------------------------------------------------------------------------

/// 轮询率。雷云实测字段名为 `PollingRate`（日志中出现 544 次）。
///
/// 2000 Hz 及以上需要 HyperPolling 无线接收器——本机正好有
/// `Razer HyperPolling Wireless Dongle`（productId 179）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PollingRate {
    Hz125,
    Hz250,
    Hz500,
    Hz1000,
    Hz2000,
    Hz4000,
    Hz8000,
}

impl PollingRate {
    pub const ALL: [PollingRate; 7] = [
        PollingRate::Hz125,
        PollingRate::Hz250,
        PollingRate::Hz500,
        PollingRate::Hz1000,
        PollingRate::Hz2000,
        PollingRate::Hz4000,
        PollingRate::Hz8000,
    ];

    /// 下拉（`.s3-dropdown`）的选项文案，**与 [`Self::ALL`] 同序**。
    ///
    /// 显示文案与 `self.hz()` 的 `format!("{} Hz", …)` 一致。
    pub const LABELS: [&'static str; 7] = [
        "125 Hz", "250 Hz", "500 Hz", "1000 Hz", "2000 Hz", "4000 Hz", "8000 Hz",
    ];

    pub fn hz(self) -> u32 {
        match self {
            Self::Hz125 => 125,
            Self::Hz250 => 250,
            Self::Hz500 => 500,
            Self::Hz1000 => 1000,
            Self::Hz2000 => 2000,
            Self::Hz4000 => 4000,
            Self::Hz8000 => 8000,
        }
    }

    pub fn label(self) -> String {
        format!("{} Hz", self.hz())
    }

    /// 是否需要 HyperPolling 无线接收器。
    pub fn needs_hyperpolling(self) -> bool {
        self.hz() >= 2000
    }
}

/// 抬升距离。雷云日志中 `Liftoff` 出现 18 次。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LiftOffDistance {
    Low,
    Medium,
    High,
}

impl LiftOffDistance {
    pub const ALL: [LiftOffDistance; 3] = [
        LiftOffDistance::Low,
        LiftOffDistance::Medium,
        LiftOffDistance::High,
    ];

    /// 下拉（`.s3-dropdown`）的选项文案，**与 [`Self::ALL`] 同序**。
    pub const LABELS: [&'static str; 3] = [
        "低", "中", "高",
    ];

    pub fn zh(self) -> &'static str {
        match self {
            Self::Low => "低",
            Self::Medium => "中",
            Self::High => "高",
        }
    }
}

/// 回弹模式（`DEBOUNCE_MODE`，微软称 Debounce）。
///
/// 两个方向雷云都给了原文：
/// - `DEBOUNCE_MODE_DESC_1`「提高游戏响应速度，是快速按下和释放按键的理想选择。**可能会出现重复输入。**」
/// - `DEBOUNCE_MODE_DESC_2`「通过防止意外重复输入，实现精确的按键敲击。」
/// - `DEBOUNCE_MODE_TOOLTIP`「改变键盘的响应速度，实现快速的按键敲击或精确的操作。」
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DebounceMode {
    /// 快响应、可能重复输入
    Fast,
    /// 防重复、精确敲击
    Precise,
}

impl DebounceMode {
    pub const ALL: [DebounceMode; 2] = [DebounceMode::Fast, DebounceMode::Precise];

    /// 下拉（`.s3-dropdown`）的选项文案，**与 [`Self::ALL`] 同序**。
    ///
    /// 单独列出来是因为 `select_row` 要的是 `&'static [&'static str]`，
    /// 而 `zh()` 是逐项方法，无法在常量位置调用。
    pub const LABELS: [&'static str; 2] = ["快速敲击", "精确敲击"];

    /// 说明文案 key，界面上直接取雷云原文。
    pub fn desc_key(self) -> &'static str {
        match self {
            Self::Fast => "DEBOUNCE_MODE_DESC_1",
            Self::Precise => "DEBOUNCE_MODE_DESC_2",
        }
    }

    pub fn zh(self) -> &'static str {
        match self {
            Self::Fast => "快速敲击",
            Self::Precise => "精确敲击",
        }
    }
}

/// 灵敏度滑块（`SENSITIVITY_CLUTCH`）。
///
/// 雷云有左/右/全局三个 key：
/// `LEFT_SENSITIVITY_CLUTCH` 左灵敏度滑块、`RIGHT_SENSITIVITY_CLUTCH` 右灵敏度滑块、
/// `GLOBAL_SENSITIVITY_CLUTCH` 左右灵敏度滑块。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensitivityClutch {
    /// 左滑块档位
    pub left: u16,
    /// 右滑块档位
    pub right: u16,
    /// 左右联动（对应 `GLOBAL_SENSITIVITY_CLUTCH`）
    pub linked: bool,
}

impl Default for SensitivityClutch {
    fn default() -> Self {
        Self {
            left: 400,
            right: 800,
            linked: false,
        }
    }
}

/// 灵敏度匹配的一条配置（`SENSITIVITY_MATCHER`）。
///
/// 雷云原文：`SENSITIVITY_MATCHER_TOOLTIP`「微调鼠标以模仿另一个鼠标的感觉。
/// **需要有第二个鼠标才能使用此功能。**」
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatcherProfile {
    /// 被模仿的目标鼠标
    pub target: String,
    /// 是否已完成校准（`SENSITIVITY_MATCHER_ACTION_DESC_3` 灵敏度匹配成功）
    pub matched: bool,
}

impl MatcherProfile {
    pub fn new(target: &str) -> Self {
        Self {
            target: target.to_string(),
            matched: false,
        }
    }
}

/// 灵敏度匹配（`SENSITIVITY_MATCHER`）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SensitivityMatcher {
    #[serde(default)]
    pub profiles: Vec<MatcherProfile>,
}

/// 性能设置。鼠标与键盘共用轮询率，其余项鼠标专有。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Performance {
    pub polling_rate: PollingRate,
    /// 抬升距离（鼠标）
    #[serde(default)]
    pub lift_off: Option<LiftOffDistance>,
    /// 鼠标加速度（关闭是竞技玩家的常见选择）
    #[serde(default)]
    pub acceleration: Option<bool>,
    /// 表面校准
    #[serde(default)]
    pub surface_calibration: Option<bool>,
    /// 传感器旋转角，0–180 度
    #[serde(default)]
    pub sensor_rotation: Option<u16>,
    /// 回弹模式（`DEBOUNCE_MODE`），键盘也有
    #[serde(default)]
    pub debounce_mode: Option<DebounceMode>,
    /// 玩游戏时自动切换轮询率（`AUTO_SWITCH_POLLING_RATE_WHEN_INGAME`，
    /// 该页标题是 `INGAME_POLLING_RATE_HEADER`「轮询率智能切换」）
    #[serde(default)]
    pub ingame_polling_switch: Option<bool>,
    /// 灵敏度滑块（`SENSITIVITY_CLUTCH`）
    #[serde(default)]
    pub sensitivity_clutch: Option<SensitivityClutch>,
    /// 灵敏度匹配（`SENSITIVITY_MATCHER`）
    #[serde(default)]
    pub sensitivity_matcher: Option<SensitivityMatcher>,
}

impl Default for Performance {
    fn default() -> Self {
        Self {
            polling_rate: PollingRate::Hz1000,
            lift_off: Some(LiftOffDistance::Low),
            acceleration: Some(false),
            surface_calibration: Some(false),
            sensor_rotation: Some(0),
            debounce_mode: Some(DebounceMode::Precise),
            ingame_polling_switch: Some(false),
            sensitivity_clutch: Some(SensitivityClutch::default()),
            sensitivity_matcher: Some(SensitivityMatcher::default()),
        }
    }
}

impl Performance {
    /// 键盘没有抬升距离/滑块/匹配等项，但**有**轮询率与回弹模式。
    pub fn for_keyboard() -> Self {
        Self {
            polling_rate: PollingRate::Hz1000,
            lift_off: None,
            acceleration: None,
            surface_calibration: None,
            sensor_rotation: None,
            debounce_mode: Some(DebounceMode::Precise),
            ingame_polling_switch: None,
            sensitivity_clutch: None,
            sensitivity_matcher: None,
        }
    }
}

// ---------------------------------------------------------------------------
// 电源
// ---------------------------------------------------------------------------

/// 无线设备的电源管理。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerManagement {
    /// 闲置多少分钟后休眠；0 表示从不
    pub sleep_after_min: u16,
    /// 低功耗模式（降低轮询率/灯光以延长续航）
    pub low_power_mode: bool,
    /// 闲置多少分钟后降低亮度；0 表示从不
    pub dim_after_min: u16,
    /// 电池健康优化（`BATTERY_HEALTH_OPTIMIZER`）
    ///
    /// 雷云原文：`BATTERY_HEALTH_OPTIMIZER_MOUSEMAT_DESCRIPTION`
    /// 「当达到设定的百分比时，鼠标垫将停止为设备充电。」
    #[serde(default)]
    pub battery_health_optimizer: bool,
    /// 停止充电的电量百分比（0–100）
    #[serde(default = "default_health_threshold")]
    pub battery_health_threshold: u8,
    /// 启用时（接电）的亮度 0–100（`BRIGHTNESS_WHEN_INACTIVE`）
    #[serde(default = "default_full_brightness")]
    pub brightness_when_active: u8,
    /// 电池供电时是否开灯（`LIGHTING_ON_BATTERY`）
    #[serde(default = "default_true")]
    pub lighting_on_battery: bool,
}

fn default_health_threshold() -> u8 {
    80
}

fn default_full_brightness() -> u8 {
    100
}

fn default_true() -> bool {
    true
}

impl Default for PowerManagement {
    fn default() -> Self {
        Self {
            sleep_after_min: 15,
            low_power_mode: false,
            dim_after_min: 1,
            battery_health_optimizer: false,
            battery_health_threshold: default_health_threshold(),
            brightness_when_active: default_full_brightness(),
            lighting_on_battery: true,
        }
    }
}

// ---------------------------------------------------------------------------
// 键盘
// ---------------------------------------------------------------------------

/// 键盘专属设置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardSettings {
    /// 游戏模式：禁用 Windows 键，避免游戏中误触
    pub gaming_mode: bool,
    /// 游戏模式下同时锁定 Alt+Tab
    pub lock_alt_tab: bool,
    /// 游戏模式下同时锁定 Alt+F4
    pub lock_alt_f4: bool,
    /// 全键无冲
    pub n_key_rollover: bool,
    /// 键盘布局，例如 `zh-CN`
    pub layout: String,
    /// 背光自动关闭时间（秒）；0 表示常亮
    pub backlight_timeout_sec: u16,
    /// Snap Tap 快速敲击（`SNAP_TAP`）
    #[serde(default)]
    pub snap_tap: Option<SnapTap>,
    /// 动态按键敲击（`DYNAMIC_KEY_STROKE`）
    #[serde(default)]
    pub dynamic_key_stroke: Option<DynamicKeyStroke>,
    /// 可调触发点（`ACTUATION_POINT`，模拟光轴 / 霍尔磁性轴）
    #[serde(default)]
    pub actuation: Option<Actuation>,
    /// 电池供电、无活动后调暗灯光（`DIM_KEYBOARD_LIGHTING_DESC`）。
    ///
    /// 原文：「以电池供电时，在无活动（分钟）后，设备将会变暗。」
    /// `DIM_KEYBOARD_LIGHTING_TIPS`：「当设备使用无线连接且不处于充电状态时，
    /// 调暗灯光功能可起作用。」0 表示关闭该行为。
    #[serde(default)]
    pub dim_on_battery_after_min: u16,
}

/// 配置文件切换方式（`PROFILE_SWITCHING`）。
///
/// 两种方式**逐字取自雷云的说明文案**：
///
/// | 方式 | 原文 key | 原文 |
/// |---|---|---|
/// | 自动 | `PROFILE_SWITCHING_AUTO_DES` | 只要你使用相应的应用程序，应用程序配置文件就会自动应用。当所列应用程序都未处于活动状态时，将应用默认应用程序配置文件。 |
/// | 手动 | `PROFILE_SWITCHING_MANUAL_DES` | 所选的应用程序配置文件始终处于活动状态，即使相关应用程序没有运行。 |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileSwitchMode {
    /// 跟随前台应用自动切换
    Automatic,
    /// 手动选定，始终生效
    Manual,
}

impl ProfileSwitchMode {
    pub const ALL: [ProfileSwitchMode; 2] = [
        ProfileSwitchMode::Automatic,
        ProfileSwitchMode::Manual,
    ];

    /// 下拉（`.s3-dropdown`）的选项文案，**与 [`Self::ALL`] 同序**。
    pub const LABELS: [&'static str; 2] = [
        "自动", "手动",
    ];

    /// 说明文案 key，界面上直接引用雷云原文。
    pub fn desc_key(self) -> &'static str {
        match self {
            Self::Automatic => "PROFILE_SWITCHING_AUTO_DES",
            Self::Manual => "PROFILE_SWITCHING_MANUAL_DES",
        }
    }

    pub fn zh(self) -> &'static str {
        match self {
            Self::Automatic => "自动",
            Self::Manual => "手动",
        }
    }
}

/// 一条「已关联的游戏/程序 → 配置文件」关联。
///
/// 原文依据：`LINKED_GAMES_TOUR_HEADER`「为每个游戏或应用程序使用特定配置文件和
/// 灯光效果」、`LINKED_GAMES_TOUR_CONTENT_2`「启动游戏或应用程序时，该配置文件和
/// 灯光效果就会启用。」
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkedGame {
    /// 游戏或程序名
    pub game: String,
    /// 关联到的配置文件 guid；空串表示使用 `GAME_PROFILE_DEFAULT`「默认」
    pub profile_guid: String,
    /// 同时应用该配置里的 Chroma 幻彩效果
    pub chroma: bool,
}

impl LinkedGame {
    pub fn new(game: &str) -> Self {
        Self {
            game: game.to_string(),
            profile_guid: String::new(),
            chroma: true,
        }
    }
}

/// 全局亮度（`BRIGHTNESS_GLOBAL`）。
///
/// 原文：`BRIGHTNESS_GLOBAL_DESC`「**一次性调整 Razer Synapse 雷云中所有设备**的
/// 亮度。」`BRIGHTNESS_GLOBAL_DESC_AT_LEAST_ONE_LED_DEVICE`「此功能需要至少一个
/// 支持 Razer Synapse 雷云且配有 LED 的设备。」
///
/// 因为它作用于**所有设备**，所以存放在应用级状态（`AppShell`）而不是单台设备上。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GlobalBrightness {
    /// 是否由全局值接管各设备亮度
    pub enabled: bool,
    /// 0–100
    pub level: u8,
}

impl Default for GlobalBrightness {
    fn default() -> Self {
        Self {
            enabled: false,
            level: 100,
        }
    }
}

/// Snap Tap 快速敲击的录入模式。
///
/// 五种取值**逐条来自雷云的说明文案**，不是自拟：
///
/// | 模式 | 原文 key | 原文 |
/// |---|---|---|
/// | 最后输入 | `SNAP_TAP_TOOLTIP_LAST_INPUT_DESC` | 当你按下两个按键时，键盘会在你按下第二个按键的瞬间释放第一个按键。 |
/// | 优先左侧 | `SNAP_TAP_TOOLTIP_PRIORITIZE_LEFT_DESC` | 按下按键时，键盘将始终优先响应第一个按键（左侧）。 |
/// | 优先右侧 | `SNAP_TAP_TOOLTIP_PRIORITIZE_RIGHT_DESC` | 按下按键时，键盘将始终优先响应第二个按键（右侧）。 |
/// | 同时释放 | `SNAP_TAP_TOOLTIP_NEUTRAL_DESC` | 当按下两个按键时，键盘会在按下第二个按键的瞬间立即释放两个按键。 |
/// | 按深度优先 | `SNAP_TAP_TOOLTIP_COMPARE_LEVEL_DESC` | 当两个按键同时按下时，键盘将优先响应按压更深的按键。 |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapTapMode {
    /// 最后输入优先（默认）
    LastInput,
    /// 始终优先左侧按键
    PrioritizeLeft,
    /// 始终优先右侧按键
    PrioritizeRight,
    /// 两个都立即释放
    Neutral,
    /// 按压更深的按键优先
    CompareLevel,
}

impl SnapTapMode {
    /// 顺序即雷云下拉菜单的顺序。
    pub const ALL: [SnapTapMode; 5] = [
        SnapTapMode::LastInput,
        SnapTapMode::PrioritizeLeft,
        SnapTapMode::PrioritizeRight,
        SnapTapMode::Neutral,
        SnapTapMode::CompareLevel,
    ];

    /// 下拉（`.s3-dropdown`）的选项文案，**与 [`Self::ALL`] 同序**。
    pub const LABELS: [&'static str; 5] = [
        "最后输入优先", "优先左侧", "优先右侧", "同时释放", "按深度优先",
    ];

    /// 说明文案 key：界面直接引用雷云原文，不改写措辞。
    pub fn desc_key(self) -> &'static str {
        match self {
            Self::LastInput => "SNAP_TAP_TOOLTIP_LAST_INPUT_DESC",
            Self::PrioritizeLeft => "SNAP_TAP_TOOLTIP_PRIORITIZE_LEFT_DESC",
            Self::PrioritizeRight => "SNAP_TAP_TOOLTIP_PRIORITIZE_RIGHT_DESC",
            Self::Neutral => "SNAP_TAP_TOOLTIP_NEUTRAL_DESC",
            Self::CompareLevel => "SNAP_TAP_TOOLTIP_COMPARE_LEVEL_DESC",
        }
    }

    /// 下拉项短名（语义取自雷云自己的说明文案）。
    pub fn zh(self) -> &'static str {
        match self {
            Self::LastInput => "最后输入优先",
            Self::PrioritizeLeft => "优先左侧",
            Self::PrioritizeRight => "优先右侧",
            Self::Neutral => "同时释放",
            Self::CompareLevel => "按深度优先",
        }
    }
}

/// 一组 Snap Tap 按键对。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapTapPair {
    /// 左侧按键的动作名
    pub left: String,
    /// 右侧按键的动作名
    pub right: String,
    /// 该组的录入模式
    pub mode: SnapTapMode,
}

impl SnapTapPair {
    pub fn new(left: &str, right: &str) -> Self {
        Self {
            left: left.to_string(),
            right: right.to_string(),
            mode: SnapTapMode::LastInput,
        }
    }
}

/// Snap Tap 快速敲击（`SNAP_TAP`）。
///
/// 上限取自雷云原文：`SNAP_TAP_DESC_V3`「最多可自定义**四对**独特的按键」、
/// `SNAP_TAP_TOOLTIP_MENU_DESC_2`「最多可为 Snap Tap 快速敲击功能选择**四组**按键组合」。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SnapTap {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub pairs: Vec<SnapTapPair>,
}

/// 最大组数（`SNAP_TAP_DESC_V3`：四对）。
pub const SNAP_TAP_MAX_PAIRS: usize = 4;

impl SnapTap {
    /// 还能再加一组吗。
    pub fn can_add(&self) -> bool {
        self.pairs.len() < SNAP_TAP_MAX_PAIRS
    }
}

/// 动态按键敲击的四个阶段（`DYNAMIC_KEY_STROKE_DESC`）。
///
/// 原文：「最多可为按键敲击的四个阶段分别指定四个绑定：按下开始、按下结束、
/// 释放开始和释放结束。」
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyStrokePhase {
    /// `PRESS_START`
    PressStart,
    /// `PRESS_END`
    PressEnd,
    /// 释放开始
    ReleaseStart,
    /// 释放结束
    ReleaseEnd,
}

impl KeyStrokePhase {
    pub const ALL: [KeyStrokePhase; 4] = [
        KeyStrokePhase::PressStart,
        KeyStrokePhase::PressEnd,
        KeyStrokePhase::ReleaseStart,
        KeyStrokePhase::ReleaseEnd,
    ];

    /// 雷云原文 key。`PRESS_START` / `PRESS_END` 有独立 key；
    /// 释放两阶段在语言包里没有单独的 key。
    pub fn label_key(self) -> Option<&'static str> {
        match self {
            Self::PressStart => Some("PRESS_START"),
            Self::PressEnd => Some("PRESS_END"),
            Self::ReleaseStart | Self::ReleaseEnd => None,
        }
    }

    pub fn zh(self) -> &'static str {
        match self {
            Self::PressStart => "按下开始",
            Self::PressEnd => "按下结束",
            Self::ReleaseStart => "释放开始",
            Self::ReleaseEnd => "释放结束",
        }
    }
}

/// 动态按键敲击（`DYNAMIC_KEY_STROKE`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicKeyStroke {
    #[serde(default)]
    pub enabled: bool,
    /// 四个阶段各自的绑定动作名；空串表示该阶段未指定
    #[serde(default)]
    pub phases: Vec<(KeyStrokePhase, String)>,
    /// `PRESS_START_SENSITIVITY` 按下开始灵敏度（毫米）
    #[serde(default = "default_press_sensitivity")]
    pub press_start_sensitivity: f32,
    /// `PRESS_END_SENSITIVITY` 按下结束灵敏度（毫米）
    #[serde(default = "default_press_sensitivity")]
    pub press_end_sensitivity: f32,
}

fn default_press_sensitivity() -> f32 {
    1.5
}

impl Default for DynamicKeyStroke {
    fn default() -> Self {
        Self {
            enabled: false,
            phases: KeyStrokePhase::ALL
                .iter()
                .map(|phase| (*phase, String::new()))
                .collect(),
            press_start_sensitivity: default_press_sensitivity(),
            press_end_sensitivity: default_press_sensitivity(),
        }
    }
}

impl DynamicKeyStroke {
    /// 取某阶段当前的绑定。
    pub fn phase(&self, phase: KeyStrokePhase) -> &str {
        self.phases
            .iter()
            .find(|(p, _)| *p == phase)
            .map(|(_, value)| value.as_str())
            .unwrap_or("")
    }
}

/// 可调触发点（模拟光轴 / 霍尔效应磁性轴）。
///
/// 量程取自雷云原文：
/// - `RAPID_TRIGGER_ADJUSTABLE_ACTUATION_CONTENT_1_BODY`
///   「触发行程可自定义（**范围 0.1–4.0 毫米**）」
/// - `ACTUATION_WARINING`「警告：将触发灵敏度设置为低于 **1.0 毫米**可能会导致
///   按键灵敏感过高，从而提高输入错误的几率。」
/// - `SECONDARY_ACTUATION_DESC`「第二触发距离必须**大于或等于**主触发距离。」
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Actuation {
    #[serde(default)]
    pub enabled: bool,
    /// `PRIMARY_ACTUATION` 主触发（毫米）
    pub primary_mm: f32,
    /// `SECONDARY_ACTUATION` 第二触发（毫米）
    pub secondary_mm: f32,
    /// 快速触发（`RAPID_TRIGGER`）：松开即重置，交换更快
    #[serde(default)]
    pub rapid_trigger: bool,
    /// `ACTUATION_FEEDBACK` 触发反馈（按下红色 / 释放绿色）
    #[serde(default)]
    pub feedback: bool,
}

/// 可调触发的量程下限（雷云原文 0.1 毫米）。
pub const ACTUATION_MIN_MM: f32 = 0.1;
/// 量程上限（雷云原文 4.0 毫米）。
pub const ACTUATION_MAX_MM: f32 = 4.0;
/// 低于此值会有误触风险（`ACTUATION_WARINING`）。
pub const ACTUATION_WARN_MM: f32 = 1.0;

impl Default for Actuation {
    fn default() -> Self {
        Self {
            enabled: false,
            primary_mm: 1.5,
            secondary_mm: 2.5,
            rapid_trigger: false,
            feedback: false,
        }
    }
}

impl Actuation {
    /// 主触发点是否落在雷云警告的区间内。
    pub fn primary_is_risky(&self) -> bool {
        self.primary_mm < ACTUATION_WARN_MM
    }

    /// 第二触发是否满足「必须大于或等于主触发」。
    pub fn secondary_is_valid(&self) -> bool {
        self.secondary_mm >= self.primary_mm
    }
}

/// 老板键（`BOSS_KEY`）。
///
/// 原文：`BOSS_KEY_CONFIGURATION_DESC`「设置按下老板键时执行的操作。」、
/// `BOSS_KEY_CONFIGURATION_TIP`「配置鼠标的老板键功能。」
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BossKey {
    #[serde(default)]
    pub enabled: bool,
    /// 按下老板键时执行的动作
    pub action: String,
}

impl Default for BossKey {
    fn default() -> Self {
        Self {
            enabled: false,
            action: "静音".to_string(),
        }
    }
}

/// 变调（`KEY_SHIFTER`）。
///
/// 原文：`KEY_SHIFTER_TOOLTIP`「启用即可使用滑块调整线路输入端口上任意音频输入的
/// 音高和速度。」
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyShifter {
    #[serde(default)]
    pub enabled: bool,
    /// 音高，半音为单位（负为降调）
    pub pitch: i8,
    /// 速度，百分比（100 为原速）
    pub speed: u16,
}

impl Default for KeyShifter {
    fn default() -> Self {
        Self {
            enabled: false,
            pitch: 0,
            speed: 100,
        }
    }
}

impl Default for KeyboardSettings {
    fn default() -> Self {
        Self {
            gaming_mode: false,
            lock_alt_tab: false,
            lock_alt_f4: false,
            n_key_rollover: true,
            layout: "zh-CN".to_string(),
            backlight_timeout_sec: 0,
            snap_tap: Some(SnapTap::default()),
            dynamic_key_stroke: Some(DynamicKeyStroke::default()),
            actuation: Some(Actuation::default()),
            dim_on_battery_after_min: 0,
        }
    }
}

/// 可选的键盘布局。
pub const KEYBOARD_LAYOUTS: [(&str, &str); 4] = [
    ("zh-CN", "简体中文"),
    ("en-US", "英语（美国）"),
    ("ja-JP", "日语"),
    ("ko-KR", "韩语"),
];

// ---------------------------------------------------------------------------
// 灯光
// ---------------------------------------------------------------------------

/// Chroma 灯光效果。
///
/// ⚠️ 效果名清单属于 **[推断]**：真正的效果名定义在远程前端里，
/// 本机无法访问 `apps.razer.com`（见 `docs/FEATURES.md` §5 未解项 1）。
/// 这里列出的是雷云公开的效果集合，日志中实测出现过 `static`（213 次）
/// 与 `Reactive`（60 次）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LightingEffect {
    Off,
    Static,
    SpectrumCycling,
    Wave,
    Breathing,
    Reactive,
    Starlight,
    AudioMeter,
    Ripple,
    Firefly,
}

impl LightingEffect {
    pub const ALL: [LightingEffect; 10] = [
        LightingEffect::Off,
        LightingEffect::Static,
        LightingEffect::SpectrumCycling,
        LightingEffect::Wave,
        LightingEffect::Breathing,
        LightingEffect::Reactive,
        LightingEffect::Starlight,
        LightingEffect::AudioMeter,
        LightingEffect::Ripple,
        LightingEffect::Firefly,
    ];

    /// 下拉（`.s3-dropdown`）的选项文案，**与 [`Self::ALL`] 同序**。
    pub const LABELS: [&'static str; 10] = [
        "关闭", "静态", "光谱循环", "波浪", "呼吸", "响应", "星光", "音频计", "涟漪", "萤火",
    ];

    pub fn zh(self) -> &'static str {
        match self {
            Self::Off => "关闭",
            Self::Static => "静态",
            Self::SpectrumCycling => "光谱循环",
            Self::Wave => "波浪",
            Self::Breathing => "呼吸",
            Self::Reactive => "响应",
            Self::Starlight => "星光",
            Self::AudioMeter => "音频计",
            Self::Ripple => "涟漪",
            Self::Firefly => "萤火",
        }
    }

    /// 该效果是否使用主色。
    pub fn uses_color(self) -> bool {
        matches!(
            self,
            Self::Static | Self::Breathing | Self::Reactive | Self::Starlight | Self::Ripple
        )
    }

    /// 该效果是否有速度参数。
    pub fn uses_speed(self) -> bool {
        matches!(
            self,
            Self::Wave
                | Self::Breathing
                | Self::Starlight
                | Self::Ripple
                | Self::Firefly
                | Self::SpectrumCycling
        )
    }
}

/// 一个可独立设置的灯光区域。
///
/// 鼠标通常有「标志 / 滚轮 / 底部灯带」，键盘通常整块或用 Chroma Studio 分区。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightingZone {
    /// 区域名，例如「标志」
    pub name: String,
    pub effect: LightingEffect,
    /// 主色 RGB
    pub color: [u8; 3],
    /// 亮度 0–100
    pub brightness: u8,
    /// 速度 0–100
    pub speed: u8,
}

impl LightingZone {
    pub fn new(name: &str, effect: LightingEffect, color: [u8; 3]) -> Self {
        Self {
            name: name.to_string(),
            effect,
            color,
            brightness: 100,
            speed: 50,
        }
    }

    /// 亮度的中文描述。
    pub fn brightness_label(&self) -> String {
        format!("{}%", self.brightness)
    }
}

// ---------------------------------------------------------------------------
// 宏
// ---------------------------------------------------------------------------

/// 宏的一步。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroStep {
    /// 该步之前延迟的毫秒数
    pub delay_ms: u32,
    /// 事件描述，例如 `key_down:A` / `key_up:A` / `mouse_down:1`
    pub action: String,
}

/// 宏。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Macro {
    pub name: String,
    pub steps: Vec<MacroStep>,
    /// 按住循环播放，松开停止
    pub loop_until_release: bool,
}

impl Macro {
    /// 宏的总时长（毫秒）。
    pub fn total_ms(&self) -> u32 {
        self.steps.iter().map(|step| step.delay_ms).sum()
    }
}

// ---------------------------------------------------------------------------
// 时间与档位的枚举值
// ---------------------------------------------------------------------------
//
// 这几组是**离散档位**，不是连续值：雷云的界面用 `.s3-dropdown` 或
// 「◀ 值 ▶」在固定档位间切换，不允许任意取值。因此它们是数组常量，
// `0` 一律表示「从不 / 已关闭」——这是雷云的约定（见 `SLEEP_AFTER_DESC`）。

/// 闲置休眠档位（分钟），`POWER` 页。
pub const SLEEP_AFTER_STEPS: [u16; 8] = [0, 1, 5, 10, 15, 30, 45, 60];

/// 闲置降低亮度档位（分钟），`POWER` 页。
pub const DIM_AFTER_STEPS: [u16; 6] = [0, 1, 5, 10, 15, 30];

/// 无活动后关闭背光的档位（秒），`LIGHTING` 页。
pub const DIM_ON_BATTERY_STEPS: [u16; 5] = [0, 15, 30, 60, 300];

/// OLED 闲置关闭档位（秒），`OLED` 页。
pub const OLED_TIMEOUT_STEPS: [u16; 5] = [0, 30, 60, 300, 600];

/// 滚轮触觉等级**最多可禁用**的数量。
///
/// 雷云原文（`SCROLL_DISABLE_MODES_DESC`）：最多禁用 2 个等级。
/// 这是产品规则，不是实现细节，所以单独具名而不是在界面里写 `2`。
pub const SCROLL_MAX_DISABLED_STAGES: usize = 2;


// ---------------------------------------------------------------------------
// 聚合
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// 滚动（TAB_SCROLLING）
// ---------------------------------------------------------------------------
// 字段与取值依据该界面真实文案，见 docs/screens/06-scrolling.md：
//   SCROLL_MODE_DESC        「若追求精度，请用触觉滚动模式；若追求速度，请用自由滚动模式。」
//   CONFIGURE_SCROLL_WHEEL_STAGES_SETTINGS  「配置滚轮触觉等级」
//   SCROLL_STEPS_TOOLTIP    「调整滚轮每转的级数，以获得你喜欢的触感。」
//   SCROLL_TENSION_TOOLTIP  「降低滚动阻力……或增加阻力以提升触感。」
//   SCROLL_DISABLE_MODES_DESC 「最多可禁用 2 个滚动模式等级。」

/// 滚轮滚动模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollingMode {
    /// 触觉滚动（有档位感，追求精度）——`SCROLL_MODE_DESC`
    TactileCycling,
    /// 自由滚动（无阻尼，追求速度）——`FREE_SPIN_SCROLLING_MODE`
    FreeSpin,
}

impl ScrollingMode {
    pub const ALL: [ScrollingMode; 2] = [ScrollingMode::TactileCycling, ScrollingMode::FreeSpin];

    /// 下拉（`.s3-dropdown`）的选项文案，**与 [`Self::ALL`] 同序**。
    ///
    /// 文案取自 `zh()` 里 `t_or` 的兜底值（`SCROLL_MODE` / `FREE_SPIN`）。
    pub const LABELS: [&'static str; 2] = [
        "滚动模式", "自由滚动",
    ];

    /// 雷云文案 key（`FREE_SPIN` / `SCROLL_MODE`）。
    pub fn key(self) -> &'static str {
        match self {
            Self::TactileCycling => "SCROLL_MODE",
            Self::FreeSpin => "FREE_SPIN",
        }
    }

    pub fn zh(self) -> String {
        crate::i18n::t_or(self.key(), "滚动模式")
    }
}

/// 滚轮触觉等级（`CONFIGURE_SCROLL_WHEEL_STAGES_SETTINGS`）。
///
/// 雷云允许「最多禁用 2 个等级」（`SCROLL_DISABLE_MODES_DESC`），因此这里
/// 每个等级带一个 `disabled` 标志，界面上应限制最多禁用 2 个。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrollStage {
    /// 等级序号（1 起）
    pub index: u8,
    /// 该等级的触觉强度，0–100
    pub haptics: u8,
    /// 是否被禁用（最多 2 个）
    pub disabled: bool,
}

impl ScrollStage {
    pub fn new(index: u8, haptics: u8) -> Self {
        Self {
            index,
            haptics,
            disabled: false,
        }
    }
}

/// 滚动设置（`TAB_SCROLLING`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scrolling {
    pub mode: ScrollingMode,
    /// 滚轮触觉等级表
    #[serde(default)]
    pub stages: Vec<ScrollStage>,
    /// 滚轮每转级数（`SCROLL_STEPS`）
    pub steps: u8,
    /// 滚动阻力（`SCROLL_TENSION`），0–100
    pub tension: u8,
    /// 滚动加速（`SCROLL_ACCELERATION`）
    pub acceleration: bool,
    /// 自由滚动模式下的加速等级（`SCROLL_ACCELERATION_WITH_LEVEL`）
    pub acceleration_level: u8,
    /// 高分辨率滚动（`HIGH_RESOLUTION_SCROLLING`）
    pub high_resolution: bool,
    /// 水平滚动（`HORIZONTAL_SCROLLING`）
    pub horizontal: bool,
    /// 滚轮触觉总开关（`ACTIVE_SCROLL_WHEEL_HAPTICS`）
    pub haptics_enabled: bool,
}

impl Default for Scrolling {
    fn default() -> Self {
        Self {
            mode: ScrollingMode::TactileCycling,
            stages: (1..=5).map(|i| ScrollStage::new(i, 50)).collect(),
            steps: 24,
            tension: 50,
            acceleration: false,
            acceleration_level: 1,
            high_resolution: true,
            horizontal: false,
            haptics_enabled: true,
        }
    }
}

impl Scrolling {
    /// 已禁用的等级数。雷云上限为 2（`SCROLL_DISABLE_MODES_DESC`）。
    pub fn disabled_count(&self) -> usize {
        self.stages.iter().filter(|s| s.disabled).count()
    }

    /// 是否还能再禁用一个等级。
    pub fn can_disable_more(&self) -> bool {
        self.disabled_count() < 2
    }
}

// ---------------------------------------------------------------------------
// 校准（TAB_CALIBRATION）
// ---------------------------------------------------------------------------
// 依据 docs/screens/04-calibration.md：
//   CALIBRATION_INFORMATION   「校准信息」
//   CREATE_OWN_SURFACE_PROFILE「创建自己的表面配置文件」
//   CALIBRATE_STEP1/2         「单击鼠标左键，并移动鼠标。」「以 Z 字形方式移动鼠标……」

/// 表面配置文件（鼠标垫）。
///
/// 雷云自带预校准的雷蛇鼠标垫数据（`CALIBRATE_MSG2`），也可自建
/// （`CREATE_OWN_SURFACE_PROFILE`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurfaceProfile {
    /// 展示名
    pub name: String,
    /// 是否为雷云预置（预置项不可删除）
    pub builtin: bool,
    /// 是否已校准
    pub calibrated: bool,
}

impl SurfaceProfile {
    pub fn new(name: &str, builtin: bool) -> Self {
        Self {
            name: name.to_string(),
            builtin,
            calibrated: false,
        }
    }
}

/// 校准状态机（`CALIBRATION_INFORMATION` → 步骤 → 完成/失败）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CalibrationState {
    /// 未开始
    Idle,
    /// 校准中（`CALIBRATING`）
    Running,
    /// 完成（`CALIBRATION_COMPLETED`）
    Completed,
    /// 失败（`CALIBRATION_FAILED`）
    Failed,
}

impl CalibrationState {
    pub fn key(self) -> &'static str {
        match self {
            Self::Idle => "CALIBRATION_INFORMATION",
            Self::Running => "CALIBRATING",
            Self::Completed => "CALIBRATION_COMPLETED",
            Self::Failed => "CALIBRATION_FAILED",
        }
    }

    pub fn zh(self) -> String {
        crate::i18n::t_or(self.key(), "校准")
    }
}

/// 校准设置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Calibration {
    /// 可选表面配置文件；首项通常是「默认」
    #[serde(default)]
    pub surfaces: Vec<SurfaceProfile>,
    /// 当前选中的表面索引
    pub selected: usize,
    /// 当前校准状态
    pub state: CalibrationState,
}

impl Default for Calibration {
    fn default() -> Self {
        Self {
            surfaces: vec![SurfaceProfile::new("默认", true)],
            selected: 0,
            state: CalibrationState::Idle,
        }
    }
}

impl Calibration {
    pub fn current(&self) -> Option<&SurfaceProfile> {
        self.surfaces.get(self.selected)
    }
}

// ---------------------------------------------------------------------------
// 配对（TAB_PAIRING）
// ---------------------------------------------------------------------------
// 依据 docs/screens/03-pairing.md：
//   HYPERPOLLING_WIRELESS_DONGLE_HEADER 「使用 Razer HyperPolling 无线接收器……」
//   DONGLE_IS_LATEST                    「……使用的已经是最新固件。」
//   MULTI_DEVICE_PAIRING                「多设备配对」
//   HYPERPOLLING_WIRELESS_*_UNPAIR_CONFIRM_TEXT 「你即将取消……的配对。确定要继续吗？」

/// 无线接收器类型（决定该页显示哪套文案）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DongleKind {
    /// HyperPolling 无线接收器（`HYPERPOLLING_WIRELESS`）
    HyperPolling,
    /// 鼠标底座专业版（`HYPERPOLLING_WIRELESS_HEADER_1`）
    MouseDockPro,
    /// 高效工作接收器 / 多设备（`MULTI_DEVICE_DONGLE`）
    Productivity,
}

impl DongleKind {
    pub fn key(self) -> &'static str {
        match self {
            Self::HyperPolling => "HYPERPOLLING_WIRELESS",
            Self::MouseDockPro => "HYPERPOLLING_WIRELESS_HEADER_1",
            Self::Productivity => "MULTI_DEVICE_DONGLE",
        }
    }

    pub fn zh(self) -> String {
        crate::i18n::t_or(self.key(), "接收器")
    }
}

/// 配对设置与状态。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pairing {
    /// 接收器类型
    pub dongle: DongleKind,
    /// 接收器固件是否已是最新（`DONGLE_IS_LATEST`）
    pub dongle_is_latest: bool,
    /// 已配对的设备名
    #[serde(default)]
    pub paired_devices: Vec<String>,
    /// 正在配对中（`PAIRING`）
    pub pairing: bool,
}

impl Default for Pairing {
    fn default() -> Self {
        Self {
            dongle: DongleKind::HyperPolling,
            dongle_is_latest: true,
            paired_devices: Vec::new(),
            pairing: false,
        }
    }
}

// ---------------------------------------------------------------------------
// 声音（TAB_SOUND）
// ---------------------------------------------------------------------------
// 依据 docs/screens/08-sound.md，耳机模块的布局分区是：
//   ① 音量与输出   volume / volume-item / volume-title / description-volume-map
//   ② 均衡器       switch-eq-item / switch-eq-title / description-eq-map
//   ③ 增强         thx-wrapper / thx-head / thx-main-title / thx-spatial / thx-reset
//   其它           audio-tutorial__video（教程视频）、launch-sound-app、
//                  text-sound-properties、audio-power-saving
//
// ⚠️ 更正：`audio-left` / `audio-right` **不是**左右两栏。
// 它们的真实 CSS 是
//   `.widget-prod img.audio-left, .widget-prod img.audio-right
//    { left:auto; position:static; top:auto }`
// 即**产品图片**的类名。真正的分栏是 `.widget-col { width:600px }`。
// 见 docs/screens/00-visual-system.md。

/// 音效增强模式（`AUDIO_ENHANCEMENT_HEADER` = 音效增强）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioEnhancement {
    /// 不处理（`AUDIO_MODE_DESC`「提供未经任何处理的原声音效。」）
    None,
    /// THX（`AUDIO_ENHANCEMENT_THX_TOOLTIP`「享受 THX 认证的沉浸式影音体验。」）
    Thx,
    /// 杜比（`AUDIO_ENHANCEMENT_DOLBY_TOOLTIP`「通过杜比虚拟音箱启用虚拟声音。」）
    Dolby,
}

impl AudioEnhancement {
    pub const ALL: [AudioEnhancement; 3] = [
        AudioEnhancement::None,
        AudioEnhancement::Thx,
        AudioEnhancement::Dolby,
    ];

    /// 下拉（`.s3-dropdown`）的选项文案，**与 [`Self::ALL`] 同序**。
    pub const LABELS: [&'static str; 3] = [
        "无", "THX 空间音效", "杜比虚拟音箱",
    ];

    pub fn zh(self) -> &'static str {
        match self {
            Self::None => "无",
            Self::Thx => "THX 空间音效",
            Self::Dolby => "杜比虚拟音箱",
        }
    }
}

/// 均衡器。雷云区分「电竞均衡器」与「标准均衡器」
/// （`AUDIO_EQ_TOOLTIP`：所有电竞均衡器调整都会保存在耳机上）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Equalizer {
    pub enabled: bool,
    /// 当前预设名
    pub preset: String,
    /// 10 段增益，单位 dB（-12..=12）
    #[serde(default)]
    pub bands: Vec<i8>,
    /// 是否为电竞均衡器（决定是否保存在耳机上）
    pub esports: bool,
}

impl Default for Equalizer {
    fn default() -> Self {
        Self {
            enabled: false,
            preset: "默认".to_string(),
            bands: vec![0; 10],
            esports: false,
        }
    }
}

/// 声音设置（`TAB_SOUND`），仅耳机/音频设备有。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sound {
    /// 主音量 0–100（`volume` / `volume-title`）
    pub volume: u8,
    /// 混音：游戏/聊天 0–100（Kraken 的第二个音量项）
    pub chat_mix: u8,
    /// 均衡器
    #[serde(default)]
    pub equalizer: Equalizer,
    /// 音效增强
    pub enhancement: AudioEnhancement,
    /// 音频计（`AUDIO_METER`），幻彩可视化用
    pub audio_meter: bool,
    /// 对立体声内容做音频镜像（`AUDIO_MONITORING_CHECK`）
    pub audio_mirroring: bool,
    /// 省电时降低音频（`audio-power-saving`）
    pub power_saving: bool,
}

impl Default for Sound {
    fn default() -> Self {
        Self {
            volume: 70,
            chat_mix: 50,
            equalizer: Equalizer::default(),
            enhancement: AudioEnhancement::None,
            audio_meter: false,
            audio_mirroring: false,
            power_saving: false,
        }
    }
}

// ---------------------------------------------------------------------------
// 麦克风（TAB_MIC）
// ---------------------------------------------------------------------------
// 依据 docs/screens/09-mic.md：
//   ① 麦克风音量 / 增益  mic-container / mic-enhancements / micboost / microphone
//   ② 监听与降噪         整个 MonitoringDashboard_* 面板 + MonitoringToggle_button
//
// 文案依据：
//   MIC_GAIN              麦克风增益
//   MIC_BOOST             麦克风增强
//   MIC_MONITORING_SIDETONE 麦克风监听（侧音）
//   MIC_AI_NOISE_CANCELLATION 麦克风 AI 降噪
//   MICROPHONE_V2_TOOLTIP 采样率 / 高通滤波器 / 模拟增益限制器

/// 麦克风采样率（`MICROPHONE_V2_TOOLTIP`「修改采样率以控制录音的解析度。」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SamplingRate {
    Hz44100,
    Hz48000,
    Hz96000,
}

impl SamplingRate {
    pub const ALL: [SamplingRate; 3] = [
        SamplingRate::Hz44100,
        SamplingRate::Hz48000,
        SamplingRate::Hz96000,
    ];

    /// 下拉（`.s3-dropdown`）的选项文案，**与 [`Self::ALL`] 同序**。
    ///
    /// 显示文案与 `self.label()` 的 `format!("{} Hz", …)` 一致。
    pub const LABELS: [&'static str; 3] = [
        "44100 Hz", "48000 Hz", "96000 Hz",
    ];

    pub fn hz(self) -> u32 {
        match self {
            Self::Hz44100 => 44_100,
            Self::Hz48000 => 48_000,
            Self::Hz96000 => 96_000,
        }
    }

    pub fn label(self) -> String {
        format!("{} Hz", self.hz())
    }
}

/// 麦克风设置（`TAB_MIC`），仅耳机/音频设备有。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mic {
    /// 麦克风音量 / 增益 0–100（`MICROPHONE_VOLUME` / `MIC_GAIN`）
    pub gain: u8,
    /// 麦克风增强档位 0–2（`MIC_BOOST`）
    pub boost: u8,
    /// 侧音电平 0–100（`MIC_MONITORING_SIDETONE`）
    pub sidetone: u8,
    /// 麦克风监听开关
    pub monitoring: bool,
    /// AI 降噪（`MIC_AI_NOISE_CANCELLATION`）
    pub ai_noise_cancellation: bool,
    /// 高通滤波器（滤掉低频隆隆声）
    pub high_pass_filter: bool,
    /// 模拟增益限制器（防削波）
    pub analogue_gain_limiter: bool,
    /// 采样率
    pub sampling_rate: SamplingRate,
    /// 是否静音（`MICROPHONE_MUTE`）
    pub muted: bool,
}

impl Default for Mic {
    fn default() -> Self {
        Self {
            gain: 70,
            boost: 0,
            sidetone: 0,
            monitoring: false,
            ai_noise_cancellation: false,
            high_pass_filter: false,
            analogue_gain_limiter: false,
            sampling_rate: SamplingRate::Hz48000,
            muted: false,
        }
    }
}

// ---------------------------------------------------------------------------
// 显示 / 触觉 / OLED
// ---------------------------------------------------------------------------

/// 显示屏设置（`TAB_DISPLAY`），仅笔记本。
///
/// 文案依据：`PERFORMANCE_MODE_SCREEN_REFRESH_RATE_HEADER`、
/// `PERFORMANCE_MODE_SCREEN_COLOR_PROFILE_HEADER`、`PERFORMANCE_LAPTOP_SCREEN`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Display {
    /// 屏幕刷新率（Hz）
    pub refresh_rate: u32,
    /// 可选刷新率
    #[serde(default)]
    pub refresh_rates: Vec<u32>,
    /// 色彩配置文件
    pub color_profile: String,
    /// 是否启用独显直连 / 性能模式联动
    pub performance_mode: bool,
}

impl Default for Display {
    fn default() -> Self {
        Self {
            refresh_rate: 240,
            refresh_rates: vec![60, 120, 144, 240, 300, 360],
            color_profile: "默认".to_string(),
            performance_mode: false,
        }
    }
}

/// 触觉设置（`TAB_HAPTICS`）。
///
/// 文案依据：`AUDIO_TO_HAPTICS_TITLE`、`AUDIO_TO_HAPTICS_GAIN_LEVEL`、
/// `AUDIO_DRIVEN_HAPTICS`「由音频转化而成的触觉反馈」。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Haptics {
    /// 音频转触觉是否启用
    pub audio_to_haptics: bool,
    /// 增益等级 0–100（`AUDIO_TO_HAPTICS_GAIN_LEVEL`）
    pub gain: u8,
    /// 最低频率（Hz）
    pub min_frequency: u32,
    /// 最高频率（Hz）
    pub max_frequency: u32,
    /// 强度 0–100
    pub intensity: u8,
}

impl Default for Haptics {
    fn default() -> Self {
        Self {
            audio_to_haptics: false,
            gain: 50,
            min_frequency: 20,
            max_frequency: 200,
            intensity: 50,
        }
    }
}

/// OLED 屏设置（`TAB_OLED`），带 OLED 显示屏的键盘。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Oled {
    /// 屏幕亮度 0–100
    pub brightness: u8,
    /// 是否显示自定义动画
    pub custom_animation: bool,
    /// 系统信息显示项（如日期格式）
    pub date_format: String,
    /// 关闭屏幕的闲置秒数；0 表示常亮
    pub timeout_sec: u16,
}

impl Default for Oled {
    fn default() -> Self {
        Self {
            brightness: 70,
            custom_animation: false,
            date_format: "YYYY-MM-DD".to_string(),
            timeout_sec: 0,
        }
    }
}

/// 设备的功能设置集合。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceFeatures {
    pub performance: Performance,
    /// 灯光区域；非 Chroma 设备为空
    #[serde(default)]
    pub lighting: Vec<LightingZone>,
    /// 电源管理；仅无线/带电池设备
    #[serde(default)]
    pub power: Option<PowerManagement>,
    /// 键盘专属设置；仅键盘
    #[serde(default)]
    pub keyboard: Option<KeyboardSettings>,
    /// Hypershift 第二套映射层是否启用
    #[serde(default)]
    pub hypershift_enabled: bool,
    /// Hypershift 第二层绑定：与主层相同的输入点，但动作独立。
    ///
    /// 雷云实测 `Razer Hypershift` 在日志中出现 24 次（`docs/FEATURES.md` C2）。
    #[serde(default)]
    pub hypershift_bindings: Vec<DkmKey>,
    #[serde(default)]
    pub macros: Vec<Macro>,
    /// 滚动设置；鼠标（带滚轮）与键盘有
    #[serde(default)]
    pub scrolling: Option<Scrolling>,
    /// 表面校准；鼠标与耳机有
    #[serde(default)]
    pub calibration: Option<Calibration>,
    /// 无线配对；仅无线设备/接收器有
    #[serde(default)]
    pub pairing: Option<Pairing>,
    /// 声音；仅耳机/音频设备有
    #[serde(default)]
    pub sound: Option<Sound>,
    /// 麦克风；仅耳机/音频设备有
    #[serde(default)]
    pub mic: Option<Mic>,
    /// 显示屏；仅笔记本有
    #[serde(default)]
    pub display: Option<Display>,
    /// 触觉；支持触觉反馈的设备有
    #[serde(default)]
    pub haptics: Option<Haptics>,
    /// OLED 屏；带 OLED 的键盘有
    #[serde(default)]
    pub oled: Option<Oled>,
    /// 老板键（`BOSS_KEY`）。
    ///
    /// 归属依据：`BOSS_KEY_CONFIGURATION_TIP`「配置**鼠标**的老板键功能。」
    /// 因此它是鼠标功能，放在这里而不是 `KeyboardSettings` 里。
    #[serde(default)]
    pub boss_key: Option<BossKey>,
    /// 变调（`KEY_SHIFTER`）。
    ///
    /// 归属依据：`KEY_SHIFTER_TOOLTIP`「启用即可使用滑块调整**线路输入端口**上
    /// 任意音频输入的音高和速度。」因此它是音频设备功能。
    #[serde(default)]
    pub key_shifter: Option<KeyShifter>,
}

impl Default for DeviceFeatures {
    fn default() -> Self {
        Self {
            performance: Performance::default(),
            lighting: Vec::new(),
            power: None,
            keyboard: None,
            hypershift_enabled: false,
            hypershift_bindings: Vec::new(),
            macros: Vec::new(),
            scrolling: None,
            calibration: None,
            pairing: None,
            sound: None,
            mic: None,
            display: None,
            haptics: None,
            oled: None,
            boss_key: None,
            key_shifter: None,
        }
    }
}

impl DeviceFeatures {
    /// 按设备类别补齐合理默认值。
    ///
    /// 接上设备模块后，应以模块下发的值为准而不再调用本方法。
    pub fn for_category(category: DeviceCategory, has_battery: bool, is_chroma: bool) -> Self {
        let mut features = Self {
            performance: match category {
                DeviceCategory::Keyboard | DeviceCategory::Keypad => Performance::for_keyboard(),
                _ => Performance::default(),
            },
            ..Default::default()
        };

        if is_chroma {
            features.lighting = default_lighting_zones(category);
        }
        if has_battery {
            features.power = Some(PowerManagement::default());
        }
        if matches!(category, DeviceCategory::Keyboard | DeviceCategory::Keypad) {
            features.keyboard = Some(KeyboardSettings::default());
        }

        // 下面三项按**实测的每设备标签页**决定，不是猜的：
        //   滚动  → 鼠标(182) 有、键盘(653) 有、耳机(777) 无
        //   校准  → 鼠标(182) 有、键盘(653) 无、耳机(777) 有
        //   配对  → 鼠标(182) 有、键盘(653) 无、耳机(777) 无
        // 见 docs/screens/README.md「按设备看标签页」。
        match category {
            DeviceCategory::Mouse => {
                features.scrolling = Some(Scrolling::default());
                features.calibration = Some(Calibration::default());
                features.pairing = Some(Pairing::default());
                // `BOSS_KEY_CONFIGURATION_TIP`「配置鼠标的老板键功能。」
                features.boss_key = Some(BossKey::default());
            }
            DeviceCategory::Keyboard | DeviceCategory::Keypad => {
                features.scrolling = Some(Scrolling::default());
            }
            DeviceCategory::Headset | DeviceCategory::Audio => {
                features.calibration = Some(Calibration::default());
                features.sound = Some(Sound::default());
                features.mic = Some(Mic::default());
                // `KEY_SHIFTER_TOOLTIP`「…调整线路输入端口上任意音频输入的音高和速度。」
                features.key_shifter = Some(KeyShifter::default());
            }
            _ => {}
        }
        // 注意：`display` / `haptics` / `oled` **故意不在这里填**。
        // 这三页虽在标签页词汇表里存在，但我尚未下载到显示它们的设备模块
        // （笔记本 / Sensa HD / OLED 键盘），因此没有依据说哪台设备有，
        // 宁可为空让界面显示「该设备没有此设置」。
        features
    }
}

/// 按类别给出默认灯光区域（[推断]）。
fn default_lighting_zones(category: DeviceCategory) -> Vec<LightingZone> {
    // 雷蛇绿 #44D62C
    const RAZER_GREEN: [u8; 3] = [0x44, 0xD6, 0x2C];
    match category {
        DeviceCategory::Mouse => vec![
            LightingZone::new("标志", LightingEffect::Static, RAZER_GREEN),
            LightingZone::new("滚轮", LightingEffect::SpectrumCycling, RAZER_GREEN),
        ],
        DeviceCategory::Keyboard => vec![LightingZone::new(
            "整块键盘",
            LightingEffect::Wave,
            RAZER_GREEN,
        )],
        _ => vec![LightingZone::new("整机", LightingEffect::Static, RAZER_GREEN)],
    }
}

/// 预设色板。首项是雷蛇绿 #44D62C，与界面主题色一致。
pub const COLOR_PALETTE: [[u8; 3]; 8] = [
    [0x44, 0xD6, 0x2C], // 雷蛇绿
    [0x00, 0xB4, 0xFF], // 冰蓝
    [0xFF, 0x1F, 0x1F], // 红
    [0xFF, 0xA5, 0x00], // 橙
    [0xFF, 0xE6, 0x00], // 黄
    [0x9B, 0x30, 0xFF], // 紫
    [0xFF, 0x00, 0xC8], // 品红
    [0xFF, 0xFF, 0xFF], // 白
];

/// 宏步骤可选的事件子集（[推断]，完整集合由设备模块下发）。
pub const MACRO_ACTIONS: [&str; 8] = [
    "key_down:A",
    "key_up:A",
    "key_down:D",
    "key_up:D",
    "mouse_down:1",
    "mouse_up:1",
    "mouse_down:2",
    "mouse_up:2",
];

#[cfg(test)]
mod tests {
    use super::*;

    /// `SNAP_TAP_DESC_V3`「最多可自定义四对独特的按键」——
    /// 上限写错会让界面允许雷云不允许的组数。
    #[test]
    fn snap_tap_caps_at_four_pairs() {
        assert_eq!(SNAP_TAP_MAX_PAIRS, 4);

        let mut snap = SnapTap::default();
        for _ in 0..SNAP_TAP_MAX_PAIRS {
            assert!(snap.can_add(), "第 {} 组之前都应该还能加", snap.pairs.len() + 1);
            snap.pairs.push(SnapTapPair::new("A", "D"));
        }
        assert!(!snap.can_add(), "到 4 组后不能再加");
    }

    /// 五种录入模式各有独立的说明 key，不能撞车。
    #[test]
    fn snap_tap_modes_are_distinct_and_documented() {
        let modes = SnapTapMode::ALL;
        assert_eq!(modes.len(), 5);
        let mut keys: Vec<&str> = modes.iter().map(|mode| mode.desc_key()).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), 5, "每种模式必须对应不同的雷云原文 key");
    }

    /// `DYNAMIC_KEY_STROKE_DESC`：四个阶段。
    #[test]
    fn key_stroke_has_four_phases_in_order() {
        assert_eq!(KeyStrokePhase::ALL.len(), 4);
        assert_eq!(KeyStrokePhase::ALL[0].zh(), "按下开始");
        assert_eq!(KeyStrokePhase::ALL[1].zh(), "按下结束");
        // 默认值把四个阶段都建出来，界面不必再判空
        let stroke = DynamicKeyStroke::default();
        for phase in KeyStrokePhase::ALL {
            assert_eq!(stroke.phase(phase), "");
        }
    }

    /// `ACTUATION_WARINING`：低于 1.0 毫米有误触风险。
    #[test]
    fn actuation_flags_risky_primary_point() {
        let mut act = Actuation::default();
        act.primary_mm = 1.5;
        assert!(!act.primary_is_risky());
        act.primary_mm = 0.8;
        assert!(act.primary_is_risky(), "低于 1.0mm 应被标记");
        act.primary_mm = ACTUATION_WARN_MM;
        assert!(!act.primary_is_risky(), "恰好 1.0mm 不算风险");
    }

    /// `SECONDARY_ACTUATION_DESC`：第二触发必须 ≥ 主触发。
    #[test]
    fn actuation_requires_secondary_not_below_primary() {
        let mut act = Actuation::default();
        act.primary_mm = 1.5;
        act.secondary_mm = 2.5;
        assert!(act.secondary_is_valid());
        act.secondary_mm = 1.5;
        assert!(act.secondary_is_valid(), "相等是允许的");
        act.secondary_mm = 1.0;
        assert!(!act.secondary_is_valid(), "低于主触发应判为非法");
    }

    /// 量程必须与雷云原文一致（0.1–4.0 毫米）。
    #[test]
    fn actuation_range_matches_the_copy() {
        assert!((ACTUATION_MIN_MM - 0.1).abs() < f32::EPSILON);
        assert!((ACTUATION_MAX_MM - 4.0).abs() < f32::EPSILON);
        assert!((ACTUATION_WARN_MM - 1.0).abs() < f32::EPSILON);
        // 默认值必须落在量程内
        let act = Actuation::default();
        assert!(act.primary_mm >= ACTUATION_MIN_MM && act.primary_mm <= ACTUATION_MAX_MM);
        assert!(act.secondary_mm >= ACTUATION_MIN_MM && act.secondary_mm <= ACTUATION_MAX_MM);
    }

    /// 归属修正：老板键是鼠标的、变调是音频设备的，不是键盘的。
    #[test]
    fn boss_key_and_key_shifter_are_not_keyboard_features() {
        use crate::model::DeviceCategory;

        let keyboard = DeviceFeatures::for_category(DeviceCategory::Keyboard, false, false);
        assert!(keyboard.boss_key.is_none(), "键盘不该有老板键");
        assert!(keyboard.key_shifter.is_none(), "键盘不该有变调");

        let mouse = DeviceFeatures::for_category(DeviceCategory::Mouse, false, false);
        assert!(mouse.boss_key.is_some(), "BOSS_KEY_CONFIGURATION_TIP 说的是鼠标");

        let headset = DeviceFeatures::for_category(DeviceCategory::Headset, false, false);
        assert!(headset.key_shifter.is_some(), "KEY_SHIFTER_TOOLTIP 说的是线路输入端口");
    }

    /// 键盘必备的三项进阶功能。
    #[test]
    fn keyboard_gets_snap_tap_stroke_and_actuation() {
        use crate::model::DeviceCategory;

        let keyboard = DeviceFeatures::for_category(DeviceCategory::Keyboard, false, false);
        let settings = keyboard.keyboard.expect("键盘应有键盘设置");
        assert!(settings.snap_tap.is_some());
        assert!(settings.dynamic_key_stroke.is_some());
        assert!(settings.actuation.is_some());
    }
}

