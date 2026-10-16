//! Current `noscript#version` data, parsed from each available product HTML.
//! See `docs/re/current-help-versions.json` for hashes and missing sources.
//! Runtime MW/app versions must come from storage, not the product UI build.
use super::*;

#[derive(Deserialize)]
struct UiVersion {
    product_id: u32,
    version: String,
    build_version: serde_json::Value,
}

pub(super) fn ui_version(product_id: u32) -> Option<String> {
    static VERSIONS: OnceLock<Vec<UiVersion>> = OnceLock::new();
    let rows = VERSIONS.get_or_init(|| crate::data::source_help_versions_data!());
    let row = rows.iter().find(|row| row.product_id == product_id)?;
    let build = match &row.build_version {
        serde_json::Value::String(value) => value.clone(),
        value => value.to_string(),
    };
    Some(format!("{}.{build}", row.version))
}
