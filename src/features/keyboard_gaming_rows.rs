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
    ROWS.get_or_init(|| {
        serde_json::from_str(include_str!("keyboard_gaming_review_data.json"))
            .expect("current per-product Gaming Mode rows")
    })
    .iter()
    .find(|rows| rows.product_id == product_id)
}
