//! 雷云真实文案（i18n）。
//!
//! # 做法
//!
//! 用 [`rust_i18n`] —— 也就是 **gpui-component 内部自己用的那一套**
//! （它同样是 `rust_i18n::i18n!("locales", fallback = "en")` + `t!`）。
//!
//! 之前我手搓了一个 `HashMap` 查表，那不是库的规范用法，也没有语言回退、
//! 运行时切换语言等能力，已替换为 rust-i18n。
//!
//! 本模块只做**薄封装**：`i18n!` 宏在 `main.rs`（crate 根）调用，
//! 因为 `t!` 会展开成 `crate::_rust_i18n_try_translate(...)`。
//!
//! # 语言文件
//!
//! 位于 `locales/`，按 rust-i18n 约定**以文件名作为语言代码**：
//! `locales/zh-CN.json`、`locales/en.json`。
//!
//! 内容是雷云前端语言包导出的**原文**，不是我自译的 ——
//! 取得方式见 `docs/RAZER-SYNAPSE-UI-SPEC.md` §8。

/// 取雷云真实文案。
///
/// `t!` 在 key 缺失时会**原样返回 key**，所以调用方据此可以判断是否命中。
pub fn t(key: &str) -> String {
    rust_i18n::t!(key).to_string()
}

/// 该 key 在语言包里是否有译文。
///
/// `t!` 未命中时原样返回 key，所以「结果等于入参」就等于未命中。
pub fn has(key: &str) -> bool {
    rust_i18n::t!(key).as_ref() != key
}

/// 取雷云真实文案；未命中时返回 `fallback`。
///
/// 命中判断复用 [`has`]，避免同一段逻辑写两遍。
pub fn t_or(key: &str, fallback: &str) -> String {
    if has(key) {
        t(key)
    } else {
        fallback.to_string()
    }
}

/// 当前语言代码。
pub fn locale() -> String {
    rust_i18n::locale().to_string()
}

/// 切换语言。
pub fn set_locale(locale: &str) {
    rust_i18n::set_locale(locale);
}
