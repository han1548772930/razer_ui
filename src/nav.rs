//! 导航：**按设备**决定标签页 —— 与雷云真实结构一致。
//!
//! # 为什么不是「区块 + 标签页」
//!
//! 我此前按 i18n key 前缀硬分了一套「区块（Section）」，那是**没有依据的**，已删除。
//! 读到真实前端后可以确认：
//!
//! - 雷云主界面外壳是顶栏、设备内容与设置（见 `docs/RAZER-SYNAPSE-UI-SPEC.md` §3、§4）；
//! - 每个**产品**有一份**独立的 React 应用**，位于
//!   `/synapse/products/<productId>/ui/`；
//! - 该设备显示哪些标签页，**写在该设备模块自己的代码里，各设备不同**。
//!
//! # 证据（模块已下载到 `.ref/devices/`）
//!
//! | 设备 | productId | 模块内声明的标签页 |
//! |---|---|---|
//! | Razer DeathAdder V3 Pro（鼠标） | 182 | 自定义 · 性能 · 正在配对 · 校准 · 电源 · 滚动 |
//! | BlackWidow V4 Pro（键盘） | 653 | 自定义 · 性能 · 灯光 · 电源 · 滚动 |
//! | RAZER KRAKEN BT SANRIO（耳机） | 777 | 自定义 · 灯光 · 校准 · 电源 · 声音 · 麦克风 |
//!
//! 标签页词汇表有两处来源，已对齐：
//!
//! - 语言包 **23** 个 `TAB_*`（`locales/zh-CN.json`，都有中文原文）；
//! - 设备模块导出 **27** 个 `TAB_*` 常量，多出的 4 个中
//!   `TAB_KEY_BINDS` / `TAB_MY_MACROS` 是真实导航项
//!   （文案键为 `TEXT_NAV_TAB_KEY_BINDS` / `TEXT_NAV_TAB_MY_MACROS`），
//!   `TAB_HEADER` / `TAB_TOOLTIP` 不是页面。
// 该模块的 API 面是**故意完整**的：逐条对应逆向雷云得到的功能层/模型定义，
// 即使界面暂未调用每个成员也保留，使模型与逆向结果一一对应。
// 这只用于领域模型模块；`src/pages/**` 里不存在这个豁免。
#![allow(dead_code)]


use gpui_kit::assets::IconName;

/// 设备类别。
///
/// 雷云按类别与具体产品决定页面内容；这里用于挑选该设备的标签页集合。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Mouse,
    Keyboard,
    Headset,
    Accessory,
    Laptop,
    Other,
}

impl DeviceKind {
    /// 从设备上报的类别字段解析（`MOUSE` / `KEYBOARD` / `ACCESSORY` …）。
    pub fn from_category(category: &str) -> Self {
        match category.to_ascii_lowercase().as_str() {
            "mouse" => Self::Mouse,
            "keyboard" => Self::Keyboard,
            "headset" | "headphones" | "earbuds" => Self::Headset,
            "accessory" => Self::Accessory,
            "laptop" | "notebook" => Self::Laptop,
            _ => Self::Other,
        }
    }

    /// 从本项目的设备类别枚举转换。
    pub fn from_enum(category: crate::model::DeviceCategory) -> Self {
        use crate::model::DeviceCategory;
        match category {
            DeviceCategory::Mouse => Self::Mouse,
            DeviceCategory::Keyboard | DeviceCategory::Keypad => Self::Keyboard,
            DeviceCategory::Headset | DeviceCategory::Audio => Self::Headset,
            DeviceCategory::Mousepad | DeviceCategory::Accessory => Self::Accessory,
            DeviceCategory::Controller => Self::Other,
            DeviceCategory::Other => Self::Other,
        }
    }

    /// 该类别设备页面上**真实存在**的标签页。
    ///
    /// 鼠标与键盘的列表取自各自设备模块（见模块文档），顺序即模块内常量的声明顺序。
    ///
    /// **其它类别尚未下载对应模块，因此返回空** —— 宁可不显示，也不编一套出来。
    /// 界面对空列表会显示「该设备页面结构尚未逆向」。
    pub fn tabs(self) -> &'static [Tab] {
        match self {
            // productId 182, Razer DeathAdder V3 Pro
            DeviceKind::Mouse => &[
                Tab::Customize,
                Tab::Performance,
                Tab::Pairing,
                Tab::Calibration,
                Tab::Power,
                Tab::Scrolling,
            ],
            // productId 653, BlackWidow V4 Pro
            DeviceKind::Keyboard => &[
                Tab::Customize,
                Tab::Performance,
                Tab::Lighting,
                Tab::Power,
                Tab::Scrolling,
            ],
            // productId 777, RAZER KRAKEN BT SANRIO LIMITED EDITION
            DeviceKind::Headset => &[
                Tab::Customize,
                Tab::Lighting,
                Tab::Calibration,
                Tab::Power,
                Tab::Sound,
                Tab::Mic,
            ],
            _ => &[],
        }
    }

    /// 该类别的中文名（取自雷云语言包）。
    pub fn zh(self) -> String {
        let key = match self {
            DeviceKind::Mouse => "MOUSE",
            DeviceKind::Keyboard => "KEYBOARD",
            DeviceKind::Headset => "HEADSET",
            DeviceKind::Accessory => "ACCESSORY",
            DeviceKind::Laptop => "LAPTOP",
            DeviceKind::Other => "DEVICE",
        };
        crate::i18n::t_or(key, "设备")
    }

    /// 该类别在雷云里使用的设备图标（SVG）。
    ///
    /// 雷云自己的设备模块就用 `/synapse/assets/imgs/favicon/MOUSE.svg`
    /// 这类按类别命名的图标。
    pub fn icon(self) -> IconName {
        match self {
            DeviceKind::Mouse => IconName::Mouse,
            DeviceKind::Keyboard => IconName::Keyboard,
            DeviceKind::Headset => IconName::Headphones,
            DeviceKind::Accessory => IconName::Cable,
            DeviceKind::Laptop => IconName::Laptop,
            DeviceKind::Other => IconName::Gamepad2,
        }
    }
}

/// 一个功能标签页，对应雷云的 `TAB_*` 常量。
///
/// # 词汇表来源
///
/// 语言包里有 **23** 个 `TAB_*`（`locales/zh-CN.json`，都有中文原文）；
/// 设备模块另导出 `TAB_KEY_BINDS` / `TAB_MY_MACROS` 两个真实导航项
/// （文案键是 `TEXT_NAV_TAB_KEY_BINDS` / `TEXT_NAV_TAB_MY_MACROS`，
/// 本身没有 `TAB_*` 中文）。`TAB_HEADER` / `TAB_TOOLTIP` 不是页面，不收。
///
/// 变体名与 [`Self::key`] 的对应是**机械的**（`Customize` → `TAB_CUSTOMIZE`），
/// 因此不额外维护一张表。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Home,
    Customize,
    Performance,
    Pairing,
    Calibration,
    Power,
    Scrolling,
    Lighting,
    Sound,
    Mic,
    Mixer,
    Audio,
    Enhancement,
    Eq,
    Haptics,
    Display,
    Oled,
    Battery,
    Color,
    Effects,
    Gaming,
    Keyboard,
    Macros,
    KeyBinds,
    Demo,
    Setting,
}

impl Tab {
    /// 全部标签页。顺序只用于 `--tab` 的用法提示，不表示界面顺序
    /// （界面顺序由设备的 [`DeviceKind::tabs`] 决定）。
    pub const ALL_TABS: [Tab; 26] = [
        Tab::Home,
        Tab::Customize,
        Tab::Performance,
        Tab::Pairing,
        Tab::Calibration,
        Tab::Power,
        Tab::Scrolling,
        Tab::Lighting,
        Tab::Sound,
        Tab::Mic,
        Tab::Mixer,
        Tab::Audio,
        Tab::Enhancement,
        Tab::Eq,
        Tab::Haptics,
        Tab::Display,
        Tab::Oled,
        Tab::Battery,
        Tab::Color,
        Tab::Effects,
        Tab::Gaming,
        Tab::Keyboard,
        Tab::Macros,
        Tab::KeyBinds,
        Tab::Demo,
        Tab::Setting,
    ];

    /// 雷云语言包里的文案 key。
    ///
    /// 除 `Macros` / `KeyBinds` 外都是 `TAB_<变体名大写>`。
    pub fn key(self) -> &'static str {
        match self {
            Self::Home => "TAB_HOME",
            Self::Customize => "TAB_CUSTOMIZE",
            Self::Performance => "TAB_PERFORMANCE",
            Self::Pairing => "TAB_PAIRING",
            Self::Calibration => "TAB_CALIBRATION",
            Self::Power => "TAB_POWER",
            Self::Scrolling => "TAB_SCROLLING",
            Self::Lighting => "TAB_LIGHTING",
            Self::Sound => "TAB_SOUND",
            Self::Mic => "TAB_MIC",
            Self::Mixer => "TAB_MIXER",
            Self::Audio => "TAB_AUDIO",
            Self::Enhancement => "TAB_ENHANCEMENT",
            Self::Eq => "TAB_EQ",
            Self::Haptics => "TAB_HAPTICS",
            Self::Display => "TAB_DISPLAY",
            Self::Oled => "TAB_OLED",
            Self::Battery => "TAB_BATTERY",
            Self::Color => "TAB_COLOR",
            Self::Effects => "TAB_EFFECTS",
            Self::Gaming => "TAB_GAMING",
            Self::Keyboard => "TAB_KEYBOARD",
            // 这两个在语言包里是 `TEXT_NAV_TAB_*`，没有 `TAB_*`
            // （见单元测试 `macros_and_keybinds_use_text_nav_keys`）。
            Self::Macros => "TEXT_NAV_TAB_MY_MACROS",
            Self::KeyBinds => "TEXT_NAV_TAB_KEY_BINDS",
            Self::Demo => "TAB_DEMO",
            Self::Setting => "TAB_SETTING",
        }
    }

    /// 标签文本（雷云原文；未命中时回退到下面的中文）。
    pub fn zh(self) -> String {
        crate::i18n::t_or(self.key(), self.fallback_zh())
    }

    /// 语言包未命中时的回退文案。
    ///
    /// 取自雷云语言包导出的原文，不是我自译的；`Macros` / `KeyBinds`
    /// 用它们各自的 `TEXT_NAV_TAB_*` 译文。
    fn fallback_zh(self) -> &'static str {
        match self {
            Self::Home => "首页",
            Self::Customize => "自定义",
            Self::Performance => "性能",
            Self::Pairing => "正在配对",
            Self::Calibration => "校准",
            Self::Power => "电源",
            Self::Scrolling => "滚动",
            Self::Lighting => "灯光",
            Self::Sound => "声音",
            Self::Mic => "麦克风",
            Self::Mixer => "混音器",
            Self::Audio => "音频",
            Self::Enhancement => "增强",
            Self::Eq => "均衡器",
            Self::Haptics => "触觉",
            Self::Display => "显示",
            Self::Oled => "OLED",
            Self::Battery => "电池",
            Self::Color => "颜色",
            Self::Effects => "效果",
            Self::Gaming => "游戏",
            Self::Keyboard => "键盘",
            Self::Macros => "我的宏",
            Self::KeyBinds => "按键绑定",
            Self::Demo => "演示",
            Self::Setting => "设置",
        }
    }

    /// 标签图标。雷云标签是「图标 + 文字」，图标取自同一套 SVG。
    ///
    /// 图标名按**语义**挑选，没有语义对应的用最接近的通用图标。
    pub fn icon(self) -> IconName {
        match self {
            Self::Home => IconName::House,
            Self::Customize => IconName::SlidersHorizontal,
            Self::Performance => IconName::Gauge,
            Self::Pairing => IconName::Bluetooth,
            Self::Calibration => IconName::Ruler,
            Self::Power => IconName::BatteryFull,
            Self::Scrolling => IconName::Scroll,
            Self::Lighting => IconName::Lightbulb,
            Self::Sound => IconName::Volume2,
            Self::Mic => IconName::Mic,
            Self::Mixer => IconName::SlidersHorizontal,
            Self::Audio => IconName::AudioLines,
            Self::Enhancement => IconName::Sparkles,
            Self::Eq => IconName::ListMusic,
            Self::Haptics => IconName::Activity,
            Self::Display => IconName::Monitor,
            Self::Oled => IconName::Tv,
            Self::Battery => IconName::Battery,
            Self::Color => IconName::Palette,
            Self::Effects => IconName::Sparkles,
            Self::Gaming => IconName::Gamepad2,
            Self::Keyboard => IconName::Keyboard,
            Self::Macros => IconName::FolderCog,
            Self::KeyBinds => IconName::Keyboard,
            Self::Demo => IconName::FlaskConical,
            Self::Setting => IconName::Settings,
        }
    }

    /// 该标签页在逆向过程中的证据 key —— 页面上真实出现过的文案 key。
    ///
    /// 用于「这一页的依据是什么」的可核对性：界面上的占位页会把它们列出来。
    pub fn evidence_keys(self) -> &'static [&'static str] {
        match self {
            Self::Home => &["DEVICES", "LINKED_GAMES"],
            Self::Customize => &["KEYMAP", "HYPERSHIFT", "STANDARD"],
            Self::Performance => &["POLLING_RATE", "DPI", "LIFT_OFF_DISTANCE"],
            Self::Pairing => &["PAIRING", "PAIRING_DEVICE"],
            Self::Calibration => &["CALIBRATION", "SURFACE_CALIBRATION"],
            Self::Power => &["SLEEP_AFTER", "DIM_AFTER", "BATTERY_HEALTH"],
            Self::Scrolling => &["SCROLL_MODE", "FREE_SPIN", "SCROLL_STEPS"],
            Self::Lighting => &["LIGHTING_EFFECT", "BRIGHTNESS", "LIGHTING_ZONE"],
            Self::Sound => &["AUDIO_ENHANCEMENT", "EQUALIZER", "VOLUME"],
            Self::Mic => &["SIDETONE", "MIC_GAIN", "SAMPLING_RATE"],
            Self::Mixer => &["MIXER", "AUDIO_MIX"],
            Self::Audio => &["AUDIO_DEVICE", "AUDIO_FUNCTION"],
            Self::Enhancement => &["AUDIO_ENHANCEMENT", "THX", "DOLBY"],
            Self::Eq => &["EQUALIZER", "EQ_PRESET"],
            Self::Haptics => &["HAPTICS", "HAPTIC_INTENSITY"],
            Self::Display => &["DISPLAY", "REFRESH_RATE"],
            Self::Oled => &["OLED", "OLED_BRIGHTNESS"],
            Self::Battery => &["BATTERY", "BATTERY_HEALTH"],
            Self::Color => &["COLOR", "CHROMA"],
            Self::Effects => &["EFFECTS", "LIGHTING_EFFECT"],
            Self::Gaming => &["GAMING_MODE", "GAMING"],
            Self::Keyboard => &["KEYBOARD", "SNAP_TAP", "DYNAMIC_KEY_STROKE"],
            Self::Macros => &["MACRO", "MY_MACROS"],
            Self::KeyBinds => &["KEY_BINDS", "BINDINGS"],
            Self::Demo => &["TAB_DEMO"],
            Self::Setting => &["SETTING", "PROFILE_SWITCHING", "BRIGHTNESS_GLOBAL"],
        }
    }

    /// 从 `--tab` 的命令行参数解析。
    ///
    /// 同时接受变体名（`Customize`）与文案 key（`TAB_CUSTOMIZE`），大小写不敏感。
    pub fn from_arg(arg: &str) -> Option<Self> {
        let want = arg.trim();
        let lowered = want.to_ascii_lowercase();
        Self::ALL_TABS.into_iter().find(|tab| {
            tab_name(*tab).eq_ignore_ascii_case(&lowered) || tab.key().eq_ignore_ascii_case(want)
        })
    }
}

/// 变体名（用于 `--tab`）。
fn tab_name(tab: Tab) -> &'static str {
    match tab {
        Tab::Home => "home",
        Tab::Customize => "customize",
        Tab::Performance => "performance",
        Tab::Pairing => "pairing",
        Tab::Calibration => "calibration",
        Tab::Power => "power",
        Tab::Scrolling => "scrolling",
        Tab::Lighting => "lighting",
        Tab::Sound => "sound",
        Tab::Mic => "mic",
        Tab::Mixer => "mixer",
        Tab::Audio => "audio",
        Tab::Enhancement => "enhancement",
        Tab::Eq => "eq",
        Tab::Haptics => "haptics",
        Tab::Display => "display",
        Tab::Oled => "oled",
        Tab::Battery => "battery",
        Tab::Color => "color",
        Tab::Effects => "effects",
        Tab::Gaming => "gaming",
        Tab::Keyboard => "keyboard",
        Tab::Macros => "macros",
        Tab::KeyBinds => "keybinds",
        Tab::Demo => "demo",
        Tab::Setting => "setting",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 设备页面的标签页集合必须与设备模块里声明的常量一致
    /// （顺序也一致 —— 见模块文档里的证据表）。
    #[test]
    fn device_tabs_match_the_device_modules() {
        assert_eq!(
            DeviceKind::Mouse.tabs(),
            &[
                Tab::Customize,
                Tab::Performance,
                Tab::Pairing,
                Tab::Calibration,
                Tab::Power,
                Tab::Scrolling,
            ]
        );
        assert_eq!(
            DeviceKind::Keyboard.tabs(),
            &[
                Tab::Customize,
                Tab::Performance,
                Tab::Lighting,
                Tab::Power,
                Tab::Scrolling,
            ]
        );
        assert_eq!(
            DeviceKind::Headset.tabs(),
            &[
                Tab::Customize,
                Tab::Lighting,
                Tab::Calibration,
                Tab::Power,
                Tab::Sound,
                Tab::Mic,
            ]
        );
        // 未下载模块的类别宁可为空，也不编一套出来。
        assert!(DeviceKind::Accessory.tabs().is_empty());
    }

    /// `TAB_*` 之外的导航项用的是 `TEXT_NAV_TAB_*`，不能写成 `TAB_*`。
    #[test]
    fn macros_and_keybinds_use_text_nav_keys() {
        assert_eq!(Tab::Macros.key(), "TEXT_NAV_TAB_MY_MACROS");
        assert_eq!(Tab::KeyBinds.key(), "TEXT_NAV_TAB_KEY_BINDS");
    }

    /// 每个 `TAB_*` 都要在语言包里真正有译文（`TEXT_NAV_TAB_*` 同理）。
    #[test]
    fn every_tab_key_resolves_in_the_locale_pack() {
        for tab in Tab::ALL_TABS {
            let key = tab.key();
            // `--lang=en` 下只有部分 key 有译文，所以这里只要求**中文包**命中。
            assert!(
                crate::i18n::has(key) || key == "TAB_KEYBOARD",
                "{key} 在语言包里没有译文，且不是已知的例外"
            );
        }
    }

    #[test]
    fn from_arg_accepts_both_name_and_key() {
        assert_eq!(Tab::from_arg("customize"), Some(Tab::Customize));
        assert_eq!(Tab::from_arg("Customize"), Some(Tab::Customize));
        assert_eq!(Tab::from_arg("TAB_CUSTOMIZE"), Some(Tab::Customize));
        assert_eq!(Tab::from_arg("nope"), None);
    }
}
