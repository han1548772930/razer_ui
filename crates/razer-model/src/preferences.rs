//! Local preferences; saving these does not change the Synapse host.
use serde::{Deserialize, Serialize};

mod custom_colors;
pub use custom_colors::{CustomColorEdit, ProfileCustomColors};

/// Source group-item-order and groupsCollapsed, kept outside Settings drafts.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DashboardPreferences {
    /// Current Pi.getIntroductionBannerStatus: missing/null means visible.
    pub is_banner_open: Option<bool>,
    pub items_order: std::collections::BTreeMap<String, Vec<String>>,
    pub groups_collapsed: std::collections::BTreeMap<String, bool>,
}

/// The source palette has sixteen slots shared by the connected color pickers.
/// These are saved independently of device/profile and Settings form drafts.
pub type CustomColorSlots = [Option<[u8; 3]>; 16];

/// Current Settings 9762's action available without the host app catalogue.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TrayDoubleClickAction {
    #[serde(rename = "showMenu")]
    ShowMenu,
}

/// Local startup intent only; neither field is an observed host/OS setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LocalStartupDraft {
    pub auto_start: bool,
    pub start_minimized: bool,
}
impl Default for LocalStartupDraft {
    fn default() -> Self {
        // Current Settings Ks initializes both editors to true before queries.
        Self {
            auto_start: true,
            start_minimized: true,
        }
    }
}

pub const LANGUAGES: &[(&str, &str)] = &[
    ("en", "English"),
    ("de", "Deutsch"),
    ("es", "Español"),
    ("fr", "Français"),
    ("ja", "日本語"),
    ("kr", "한국어"),
    ("pt-BR", "Português (Brasileiro)"),
    ("ru", "Русский"),
    ("zh-CN", "中文（简体）"),
    ("zh-TW", "中文（繁體）"),
];
pub const RECOMMENDATION_CATEGORIES: &[(&str, &str)] = &[
    ("accessory", "ACCESSORIES"),
    ("audio", "AUDIO"),
    ("chromaHdk", "CHROMA_HDK"),
    ("keyboard", "KEYBOARDS"),
    ("keypad", "KEYPADS"),
    ("laptop", "LAPTOPS"),
    ("mousemat", "MATS"),
    ("mouse", "MICE"),
    ("monitor", "MONITORS"),
    ("streaming", "STREAMING"),
];
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AppPreferences {
    /// None represents an older workspace: current source defaults to showMenu.
    pub systray_double_click: Option<TrayDoubleClickAction>,
    /// None means no local startup choice has been saved, not disabled in Windows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub startup_draft: Option<LocalStartupDraft>,
    /// Local intent for source `isDynamicLighting`, not a native ownership receipt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_lighting_draft: Option<bool>,
    /// Source global localStorage `isShowSensitivityTutorial`; missing shows it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mouse_dynamic_tutorial_visible: Option<bool>,
    /// Source global localStorage `showCalibrationMessage`; missing shows it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mouse_calibration_welcome_visible: Option<bool>,
    /// Global localStorage `windowsLoginWarningHide`; missing means false.
    /// This local preference is independent of any device profile or write.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub windows_login_warning_hide: Option<bool>,
    /// Actual observed localStorage synapse_<PID> JSON strings. An absent key
    /// stays absent; local profile drafts never synthesize this source object.
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub source_product_preferences: std::collections::BTreeMap<u32, String>,
    pub language: String,
    /// Retired workspace key, retained only until storage migrates it to the
    /// shared displayNotifications preference. It has no runtime consumer.
    #[serde(rename = "notifications", skip_serializing_if = "Option::is_none")]
    pub legacy_notifications: Option<bool>,
    pub recommendations: bool,
    pub ignored_categories: Vec<String>,
    pub ignored_products: Vec<String>,
    pub owned_products: Vec<String>,
    pub new_products: bool,
    pub partner_deals: bool,
    pub gamer_room_tutorial_seen: bool,
    pub dashboard_tutorial_seen: bool,
    /// Dashboard localStorage `showProfileMigrationIcon`; closing its host tab hides it.
    pub profile_migration_icon_visible: bool,
}
impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            systray_double_click: None,
            startup_draft: None,
            dynamic_lighting_draft: None,
            mouse_dynamic_tutorial_visible: None,
            mouse_calibration_welcome_visible: None,
            windows_login_warning_hide: None,
            source_product_preferences: Default::default(),
            language: "zh-CN".into(),
            legacy_notifications: None,
            recommendations: true,
            ignored_categories: vec![],
            ignored_products: vec![],
            owned_products: vec![],
            new_products: true,
            partner_deals: true,
            gamer_room_tutorial_seen: false,
            dashboard_tutorial_seen: false,
            profile_migration_icon_visible: true,
        }
    }
}
impl AppPreferences {
    pub fn validate(&self) -> Result<(), String> {
        if !LANGUAGES.iter().any(|(code, _)| *code == self.language) {
            return Err("不支持的界面语言。".into());
        }
        let mut seen = std::collections::BTreeSet::new();
        if self.ignored_categories.iter().any(|id| {
            !RECOMMENDATION_CATEGORIES.iter().any(|(key, _)| key == id) || !seen.insert(id)
        }) {
            return Err("推荐分类无效或重复。".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_icon_defaults_visible_and_preserves_dismissal_across_reload() {
        let original: AppPreferences = serde_json::from_str("{}").unwrap();
        assert!(original.profile_migration_icon_visible);
        let closed = AppPreferences {
            profile_migration_icon_visible: false,
            ..original
        };
        let reloaded: AppPreferences =
            serde_json::from_str(&serde_json::to_string(&closed).unwrap()).unwrap();
        assert!(!reloaded.profile_migration_icon_visible);
    }
}
