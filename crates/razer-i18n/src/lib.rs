//! Installed source-derived words with retained in-memory language selection.
use razer_resource_db::localization::{self, DictionarySnapshot};

fn words() -> &'static DictionarySnapshot {
    localization::snapshot().expect("Cannot initialize the installed locale catalog")
}

fn lookup<'a>(words: &'a DictionarySnapshot, locale: &str, key: &str) -> Option<&'a str> {
    if let Some(value) = words.get(locale, key) {
        return Some(value);
    }
    // Preserve the locked i18n lookup rule, including RFC 4647 private-use tags.
    let mut parent = locale;
    while let Some(index) = parent.rfind('-') {
        parent = parent[..index].trim_end_matches("-x");
        if let Some(value) = words.get(parent, key) {
            return Some(value);
        }
    }
    words.get("en", key)
}

/// Source copy; a missing translation retains the exact source key or literal.
pub fn t(key: &str) -> String {
    lookup(words(), &rust_i18n::locale(), key)
        .unwrap_or(key)
        .to_owned()
}

/// Explicit OLED language selection, independent of the global UI language.
pub fn t_locale(key: &str, locale: &str) -> String {
    lookup(words(), &canonical_locale(locale), key)
        .unwrap_or(key)
        .to_owned()
}

/// Values equal to their key and explicit empty values are still present.
pub fn has(key: &str) -> bool {
    lookup(words(), &rust_i18n::locale(), key).is_some()
}

pub fn t_or(key: &str, fallback: &str) -> String {
    lookup(words(), &rust_i18n::locale(), key)
        .unwrap_or(fallback)
        .to_owned()
}

/// Current source MIN/SEC templates substitute only their `{{value}}` marker.
pub fn t_value(key: &str, value: i64) -> String {
    t(key).replace("{{value}}", &value.to_string())
}

pub fn locale() -> String {
    rust_i18n::locale().to_string()
}

/// Initialize the immutable catalog before opening views, then publish the
/// shared language used by both product words and the component framework.
pub fn set_locale(locale: &str) {
    let _ = words();
    rust_i18n::set_locale(&canonical_locale(locale));
}

fn canonical_locale(locale: &str) -> String {
    let normalized = locale.replace('_', "-").to_ascii_lowercase();
    for (base, canonical) in [("zh-cn", "zh-CN"), ("zh-tw", "zh-TW"), ("pt-br", "pt-BR")] {
        if let Some(suffix) = normalized.strip_prefix(base)
            && (suffix.is_empty() || suffix.starts_with('-'))
        {
            return format!("{canonical}{suffix}");
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_key_and_translation_are_present() {
        assert_eq!(lookup(words(), "en", "RESET"), Some("RESET"));
        assert!(words().get("en", "RESET").is_some());
    }

    #[test]
    fn missing_key_uses_the_requested_fallback() {
        const MISSING: &str = "__razer_i18n_missing_translation_probe__";
        assert!(lookup(words(), "en", MISSING).is_none());
        assert_eq!(t_locale(MISSING, "en"), MISSING);
        assert_eq!(t_locale("Show Guide", "zh-CN"), "Show Guide");
    }

    #[test]
    fn explicit_locale_aliases_select_the_actual_dictionary() {
        for locale in ["zh-CN", "zh-cn", "ZH_CN"] {
            assert_eq!(canonical_locale(locale), "zh-CN");
            assert_eq!(t_locale("CANCEL", locale), t_locale("CANCEL", "zh-CN"));
            assert_eq!(t_locale("CANCEL", locale), "取消");
            assert_ne!(t_locale("CANCEL", locale), t_locale("CANCEL", "en"));
        }
        assert_eq!(canonical_locale("PT_br"), "pt-BR");
        assert_eq!(canonical_locale("EN"), "en");
    }

    #[test]
    fn parent_tags_and_english_fallback_preserve_locked_lookup_semantics() {
        assert_eq!(lookup(words(), "zh-CN-private", "CANCEL"), Some("取消"));
        assert_eq!(lookup(words(), "zh-CN-x-private", "CANCEL"), Some("取消"));
        assert_eq!(lookup(words(), "xx-ZZ", "RESET"), Some("RESET"));
        assert_eq!(t_locale("CANCEL", "en-GB"), t_locale("CANCEL", "en"));
        assert_eq!(t_locale("CANCEL", "zh-CN-private"), "取消");
        assert_eq!(t_locale("CANCEL", "ZH_CN_x_private"), "取消");
        assert_eq!(
            t_locale("CANCEL", "pt-BR-private"),
            t_locale("CANCEL", "pt-BR")
        );
    }

    #[test]
    fn installed_export_copy_and_namespace_templates_survive_migration() {
        assert_eq!(t_locale("SHOW_GUIDE", "zh-CN"), "显示指南");
        assert_ne!(t_locale("LED_GUIDE", "zh-CN"), "LED_GUIDE");
        assert_ne!(t_locale("LED_CHECK_STATUS", "en"), "LED_CHECK_STATUS");
        assert_eq!(t_locale("FIRMWARE_SOURCE.UPDATE", "en"), "Update");
        assert_eq!(t_locale("MIN", "en"), "{{value}} min.");
    }

    #[test]
    fn retained_language_switch_updates_shared_selection_and_exact_template_values() {
        const MISSING: &str = "__razer_i18n_missing_translation_probe__";
        for (language, reset, minutes) in [("en", "RESET", "7 min."), ("zh-CN", "重置", "7 分钟")]
        {
            set_locale(language);
            assert_eq!(locale(), language);
            assert_eq!(rust_i18n::locale().to_string(), language);
            assert_eq!(t("RESET"), reset);
            assert!(has("RESET"));
            assert_eq!(t_or("RESET", "unexpected fallback"), reset);
            assert_eq!(t_value("MIN", 7), minutes);
            assert!(!has(MISSING));
            assert_eq!(t_or(MISSING, "fallback"), "fallback");
            assert_eq!(t(MISSING), MISSING);
        }
        set_locale("en");
    }
}
