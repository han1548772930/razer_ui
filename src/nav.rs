//! Routes from GM (182), zh (653), Ov (777) and frontend HomePage.
//! Pairing is a separate display mode; HELP is a toolbar action.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Home,
    GamerRoom,
    Modules,
    Shortcuts,
    Customize,
    Performance,
    Power,
    Calibration,
    Lighting,
    Sound,
    Mic,
    Setting,
    Pairing,
    Help,
}

impl Tab {
    pub const MAIN: [Self; 4] = [Self::Home, Self::GamerRoom, Self::Modules, Self::Shortcuts];
    pub fn for_product(pid: u32) -> &'static [Self] {
        match pid {
            182 => &[
                Self::Customize,
                Self::Performance,
                Self::Power,
                Self::Calibration,
            ],
            653 | crate::demo::DEMO_PRODUCT_ID => &[Self::Customize, Self::Lighting],
            777 => &[Self::Sound, Self::Mic, Self::Lighting, Self::Power],
            _ => &[],
        }
    }
    pub fn is_main(self) -> bool {
        Self::MAIN.contains(&self) || self == Self::Setting
    }
    pub fn id(self) -> &'static str {
        match self {
            Self::Home => "dashboard",
            Self::GamerRoom => "gamer-room",
            Self::Modules => "modules",
            Self::Shortcuts => "shortcuts",
            Self::Customize => "customize",
            Self::Performance => "performance",
            Self::Power => "power",
            Self::Calibration => "calibration",
            Self::Lighting => "lighting",
            Self::Sound => "sound",
            Self::Mic => "mic",
            Self::Setting => "settings",
            Self::Pairing => "pairing",
            Self::Help => "help",
        }
    }
    pub fn label(self) -> String {
        let (key, fallback) = match self {
            Self::Home => ("DASHBOARD", "仪表盘"),
            Self::GamerRoom => ("GAMER_ROOM", "Gamer Room"),
            Self::Modules => ("DEVICES_AND_MODULES", "设备与模块"),
            Self::Shortcuts => ("GLOBAL_SHORTCUTS", "全局快捷键"),
            Self::Customize => ("TAB_CUSTOMIZE", "自定义"),
            Self::Performance => ("TAB_PERFORMANCE", "性能"),
            Self::Power => ("TAB_POWER", "电源"),
            Self::Calibration => ("TAB_CALIBRATION", "校准"),
            Self::Lighting => ("TAB_LIGHTING", "灯光"),
            Self::Sound => ("TAB_SOUND", "声音"),
            Self::Mic => ("TAB_MIC", "麦克风"),
            Self::Setting => ("TAB_SETTING", "设置"),
            Self::Pairing => ("TAB_PAIRING", "配对"),
            Self::Help => ("HELP", "帮助"),
        };
        crate::i18n::t_or(key, fallback)
    }
    pub fn from_arg(arg: &str) -> Option<Self> {
        let arg = arg.to_ascii_lowercase().replace("tab_", "");
        Self::MAIN
            .into_iter()
            .chain([
                Self::Customize,
                Self::Performance,
                Self::Power,
                Self::Calibration,
                Self::Lighting,
                Self::Sound,
                Self::Mic,
                Self::Setting,
                Self::Pairing,
                Self::Help,
            ])
            .find(|t| {
                t.id() == arg
                    || (*t == Self::Home && arg == "home")
                    || (*t == Self::Setting && arg == "setting")
            })
    }
}

#[cfg(test)]
mod tests {
    use super::Tab;
    #[test]
    fn product_routes_exclude_shared_but_unmounted_features() {
        assert_eq!(
            Tab::for_product(182),
            &[
                Tab::Customize,
                Tab::Performance,
                Tab::Power,
                Tab::Calibration
            ]
        );
        assert_eq!(Tab::for_product(653), &[Tab::Customize, Tab::Lighting]);
        assert_eq!(
            Tab::for_product(777),
            &[Tab::Sound, Tab::Mic, Tab::Lighting, Tab::Power]
        );
        assert!(Tab::for_product(999).is_empty());
        for pid in [182, 653, 777] {
            assert!(!Tab::for_product(pid).contains(&Tab::Pairing));
            assert!(!Tab::for_product(pid).iter().any(|t| t.is_main()));
        }
        assert_eq!(Tab::from_arg("scrolling"), None);
    }
}
