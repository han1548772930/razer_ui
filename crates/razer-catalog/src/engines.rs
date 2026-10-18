//! Native adapters whose loading lifecycle remains unverified.
const BLOCKING_ENGINES: &[&str] = &["SysUtilsNative"];

/// 该引擎是否受到加载限制。
pub fn blocks_load(stem: &str) -> bool {
    BLOCKING_ENGINES
        .iter()
        .any(|name| name.eq_ignore_ascii_case(stem))
}
