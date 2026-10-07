//! Exact receiver bindings derived from current DeviceInfo, feature, factory,
//! protocol inheritance and command ASTs. Catalog membership is not a capability.
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub(crate) enum ReceiverProtocol {
    #[serde(rename = "razer_device25_wireless_status_v2")]
    RazerDevice25WirelessStatusV2,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ReceiverCapability {
    pub(crate) source_product_id: u16,
    pub(crate) vendor_id: u16,
    pub(crate) product_id: u16,
    pub(crate) claim_interface: u8,
    pub(crate) report_bytes: usize,
    pub(crate) report_id: u8,
    pub(crate) protocol: ReceiverProtocol,
    pub(crate) command: [u8; 3],
    pub(crate) transaction_prefix: u8,
    pub(crate) transaction_modulus: u8,
    pub(crate) max_retry_in: u8,
    pub(crate) max_retry_out: u8,
    pub(crate) sleep_between_out_ms: u64,
    pub(crate) sleep_between_out_in_ms: u64,
    pub(crate) sleep_between_in_ms: u64,
    /// Keyboard middleware accepts productId as well as scalar dongleId;
    /// mouse/linker middleware uses scalar dongleId only.
    pub(crate) peer_match_product_id: bool,
    pub(crate) source_class: String,
    pub(crate) evidence_path: String,
}

pub(crate) fn all() -> &'static [ReceiverCapability] {
    #[derive(Deserialize)]
    struct Catalog {
        schema_version: u8,
        capabilities: Vec<ReceiverCapability>,
    }
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    &CATALOG
        .get_or_init(|| {
            let catalog: Catalog = serde_json::from_str(include_str!(
                "../../assets/data/receiver-query-capabilities.json"
            ))
            .expect("statically generated receiver capability asset must be valid");
            assert_eq!(catalog.schema_version, 1);
            catalog
        })
        .capabilities
}

pub(crate) fn capability(product_id: u16) -> Option<&'static ReceiverCapability> {
    all().iter().find(|entry| entry.product_id == product_id)
}
