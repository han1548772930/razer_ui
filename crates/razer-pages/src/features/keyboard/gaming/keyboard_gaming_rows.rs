//! Per-product current Gaming Mode row gates, statically derived from mounted UI.
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize)]
pub(super) struct Rows {
    product_id: u32,
    pub(super) menu: bool,
    pub(super) copilot: bool,
}

pub(super) fn for_product(product_id: u32) -> Option<&'static Rows> {
    static ROWS: OnceLock<Vec<Rows>> = OnceLock::new();
    ROWS.get_or_init(|| read_keyboard_gaming_review_catalog())
        .iter()
        .find(|rows| rows.product_id == product_id)
}

// Generated catalog reader: razer-pages/keyboard_gaming_review_data.
fn read_keyboard_gaming_review_catalog() -> Vec<Rows> {
    fn decode_0(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<Rows>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Rows> {
        let value = match r.tag()? {
            0 => Rows {
                product_id: r.child(decode_2)?,
                menu: r.child(decode_3)?,
                copilot: r.child(decode_3)?,
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
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_GAMING_REVIEW_DATA,
        0,
        "c7ac87f99fbdb96304a8326c21d0a8d69c69d6ae2b70652b07b2d5ca0fc955a8",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
#[test]
fn packaged_catalog_keyboard_gaming_review_data_decodes() {
    let _ = read_keyboard_gaming_review_catalog();
}
// End generated catalog reader: razer-pages/keyboard_gaming_review_data.
