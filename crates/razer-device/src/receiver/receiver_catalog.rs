//! Current 34340/he descriptive catalog fallback, independent of live identity.
use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PeerDescription {
    dongle_id: u32,
    category: String,
    product_name: BTreeMap<String, String>,
}

impl PeerDescription {
    pub fn category(&self) -> &str {
        &self.category
    }
    pub fn product_name(&self) -> &BTreeMap<String, String> {
        &self.product_name
    }
}

pub fn description(dongle_id: u32) -> Option<&'static PeerDescription> {
    static CATALOG: OnceLock<Vec<PeerDescription>> = OnceLock::new();
    CATALOG
        .get_or_init(|| {
            serde_json::from_str(include_str!("receiver_peer_catalog.json"))
                .expect("statically validated current receiver descriptions")
        })
        .iter()
        .find(|entry| entry.dongle_id == dongle_id)
}
