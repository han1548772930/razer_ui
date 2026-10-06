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
//! 当前资源审计入口见 `docs/re/13-resource-usage.md`。

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
    rust_i18n::set_locale(locale);
}
