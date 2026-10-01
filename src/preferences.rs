//! Local preferences; saving these does not change the Synapse host.
use serde::{Deserialize, Serialize};
pub(crate) const LANGUAGES: &[(&str, &str)] = &[
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
pub(crate) const RECOMMENDATION_CATEGORIES: &[(&str, &str)] = &[
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
pub(crate) struct AppPreferences {
    pub(crate) language: String,
    pub(crate) notifications: bool,
    pub(crate) recommendations: bool,
    pub(crate) ignored_categories: Vec<String>,
    pub(crate) ignored_products: Vec<String>,
    pub(crate) new_products: bool,
    pub(crate) partner_deals: bool,
    pub(crate) gamer_room_tutorial_seen: bool,
}
impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            language: "zh-CN".into(),
            notifications: true,
            recommendations: true,
            ignored_categories: vec![],
            ignored_products: vec![],
            new_products: true,
            partner_deals: true,
            gamer_room_tutorial_seen: false,
        }
    }
}
impl AppPreferences {
    pub(crate) fn validate(&self) -> Result<(), String> {
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
