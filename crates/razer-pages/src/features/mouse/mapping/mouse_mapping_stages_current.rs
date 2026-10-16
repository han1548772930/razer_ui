//! Current mounted dpiStages reducer initial rows and local active-profile load.
//! Legacy uppercase DPIStages does not enter this lowercase producer branch.
use super::*;

#[derive(Deserialize)]
struct Policy {
    product_id: u32,
    initial_rows: Vec<Value>,
}
fn policy(product_id: u32) -> Option<&'static Policy> {
    static POLICIES: OnceLock<Vec<Policy>> = OnceLock::new();
    POLICIES
        .get_or_init(|| read_mouse_mapping_stages_current_catalog())
        .iter()
        .find(|p| p.product_id == product_id)
}
impl MouseProductWorkspace {
    pub(super) fn current_mouse_sensitivity_stages(&self) -> Option<&[Value]> {
        // Source loadActiveProfilesSettings only dispatches this lowercase
        // property when it is present. Without it, the reducer keeps its own
        // initial rows; do not normalize uppercase CONFIG data into that path.
        if self.draft.get("dpiStages").is_some() {
            return self
                .draft
                .pointer("/dpiStages/stages")
                .and_then(Value::as_array)
                .map(Vec::as_slice);
        }
        Some(&policy(self.spec.product_id)?.initial_rows)
    }
}

// Generated catalog reader: razer-pages/mouse_mapping_stages_current_data.
fn read_mouse_mapping_stages_current_catalog() -> Vec<Policy> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Policy>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Policy> {
        let value = match r.tag()? {
            0 => Policy {
                product_id: r.child(decode_2)?,
                initial_rows: r.child(decode_3)?,
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
    fn decode_3(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<serde_json::Value>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_4)?,
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
        razer_resource_db::catalogs::RAZER_PAGES_MOUSE_MAPPING_STAGES_CURRENT_DATA,
        0,
        "6bb47cf3873706493d770c5aa1bc8c9b0a084841f86b373c94b903e15582e4ec",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod mouse_mapping_stages_current_data_catalog_tests {
    #[test]
    fn packaged_catalog_mouse_mapping_stages_current_data_decodes() {
        let _ = super::read_mouse_mapping_stages_current_catalog();
    }
}
// End generated catalog reader: razer-pages/mouse_mapping_stages_current_data.
