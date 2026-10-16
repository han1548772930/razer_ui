//! Source-derived translations with locale fallback and runtime language selection.
//! Locale files use language codes under `locales/`; resource provenance is
//! documented in `docs/re/resources-current.md`.

rust_i18n::i18n!("../../locales", fallback = "en");

/// 取雷云真实文案。

///

/// 是否命中语言包请使用 [`has`]；译文可能与 key 相同。

pub fn t(key: &str) -> String {
    rust_i18n::t!(key).to_string()
}

/// Explicit OLED language selection, independent of the global UI locale.
pub fn t_locale(key: &str, locale: &str) -> String {
    rust_i18n::t!(key, locale = &canonical_locale(locale)).to_string()
}

/// 该 key 在语言包里是否有译文。

///

/// 直接查询语言包及其回退语言，保留 `RESET` 等译文与 key 相同的有效条目。

pub fn has(key: &str) -> bool {
    _rust_i18n_try_translate(&rust_i18n::locale(), key).is_some()
}

/// 取雷云真实文案；未命中时返回 `fallback`。

///

/// 命中判断复用 [`has`]，避免同一段逻辑写两遍。

pub fn t_or(key: &str, fallback: &str) -> String {
    if has(key) { t(key) } else { fallback.to_string() }
}

/// 取带 `{{value}}` 占位符的雷云文案并替换数值。

///

/// 源码里这类模板键由 `getTextItem(key, {value})` 填充，例如功耗取值标签用的

/// `MIN`（"{{value}} min."）与 `SEC`（"{{value}} sec."）；语言包本身保留占位符，

/// 因此这里做一次字面替换即可。

pub fn t_value(key: &str, value: i64) -> String {
    t(key).replace("{{value}}", &value.to_string())
}

/// 当前语言代码。

pub fn locale() -> String {
    rust_i18n::locale().to_string()
}

/// 切换语言。

pub fn set_locale(locale: &str) {
    rust_i18n::set_locale(&canonical_locale(locale));
}

fn canonical_locale(locale: &str) -> String {
    match locale.replace('_', "-").to_ascii_lowercase().as_str() {
        "zh-cn" => "zh-CN".into(),
        "zh-tw" => "zh-TW".into(),
        "pt-br" => "pt-BR".into(),
        normalized => normalized.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_key_and_translation_are_present() {
        assert_eq!(
            _rust_i18n_try_translate("en", "RESET").as_deref(),
            Some("RESET")
        );
        assert!(has("RESET"));
        assert_eq!(t_or("RESET", "unexpected fallback"), t("RESET"));
    }

    #[test]
    fn missing_key_uses_the_requested_fallback() {
        const MISSING: &str = "__razer_i18n_missing_translation_probe__";
        assert!(_rust_i18n_try_translate("en", MISSING).is_none());
        assert!(!has(MISSING));
        assert_eq!(t_or(MISSING, "fallback"), "fallback");
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
}
