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
    let rows = VERSIONS.get_or_init(|| read_source_help_versions_catalog());
    let row = rows.iter().find(|row| row.product_id == product_id)?;
    let build = match &row.build_version {
        serde_json::Value::String(value) => value.clone(),
        value => value.to_string(),
    };
    Some(format!("{}.{build}", row.version))
}

// Generated catalog reader: razer-pages/source_help_versions_data.
fn read_source_help_versions_catalog() -> Vec<UiVersion> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<UiVersion>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<UiVersion> {
        let value = match r.tag()? {
            0 => UiVersion {
                product_id: r.child(decode_2)?,
                version: r.child(decode_3)?,
                build_version: r.child(decode_4)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<serde_json::Value> {
        let value = match r.tag()? {
            0 => r.external_value()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_SOURCE_HELP_VERSIONS_DATA,
        0,
        "ac9137cf247a4414c179f36ccfeb8888b4b3aba974338b8faf065cbbd66458a8",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod source_help_versions_data_catalog_tests {
    #[test]
    fn packaged_catalog_source_help_versions_data_decodes() {
        let _ = super::read_source_help_versions_catalog();
    }
}
// End generated catalog reader: razer-pages/source_help_versions_data.
