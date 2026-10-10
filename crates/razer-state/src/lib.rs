//! Local preferences; saving these does not change the Synapse host.
use gpui_kit::{App, BorrowAppContext, Global};
use serde::{Deserialize, Serialize};

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

#[derive(Default)]
pub struct CustomColors {
    colors: CustomColorSlots,
    saved: CustomColorSlots,
}
impl Global for CustomColors {}
impl CustomColors {
    pub fn new(colors: CustomColorSlots) -> Self {
        Self {
            colors,
            saved: colors,
        }
    }
    pub fn ensure(cx: &mut App) {
        if !cx.has_global::<Self>() {
            cx.set_global(Self::default());
        }
    }
    pub fn colors(&self) -> CustomColorSlots {
        self.colors
    }
    pub fn dirty(&self) -> bool {
        self.colors != self.saved
    }
    pub fn replace(colors: CustomColorSlots, cx: &mut App) {
        Self::ensure(cx);
        if cx.global::<Self>().colors != colors {
            cx.update_global::<Self, _>(|palette, _| palette.colors = colors);
        }
    }
    pub fn mark_saved(colors: CustomColorSlots, cx: &mut App) {
        // A completed write acknowledges its captured revision, not later edits.
        cx.update_global::<Self, _>(|palette, _| palette.saved = colors);
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
    pub language: String,
    pub notifications: bool,
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
            language: "zh-CN".into(),
            notifications: true,
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
        // Existing workspaces predate the Dashboard showProfileMigrationIcon key.
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

    #[gpui_kit::test]
    fn palette_save_completion_keeps_later_edits_pending(cx: &mut gpui_kit::TestAppContext) {
        cx.update(|cx| {
            CustomColors::ensure(cx);
            let mut first = [None; 16];
            first[0] = Some([1, 2, 3]);
            CustomColors::replace(first, cx);
            let captured = cx.global::<CustomColors>().colors();
            let mut second = first;
            second[1] = Some([4, 5, 6]);
            CustomColors::replace(second, cx);
            CustomColors::mark_saved(captured, cx);
            assert!(cx.global::<CustomColors>().dirty());
            assert_eq!(cx.global::<CustomColors>().colors(), second);
            CustomColors::mark_saved(second, cx);
            assert!(!cx.global::<CustomColors>().dirty());
        });
    }
}
