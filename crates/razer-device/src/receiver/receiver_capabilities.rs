//! Exact receiver bindings derived from current DeviceInfo, feature, factory,
//! protocol inheritance and command ASTs. Catalog membership is not a capability.
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum ReceiverProtocol {
    #[serde(rename = "razer_device25_wireless_status_v2")]
    RazerDevice25WirelessStatusV2,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ReceiverStartupRetry {
    pub first_peer_status: u8,
    pub delays_ms: Vec<u64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ReceiverCapability {
    pub source_product_id: u16,
    pub vendor_id: u16,
    pub product_id: u16,
    pub claim_interface: u8,
    pub report_bytes: usize,
    pub report_id: u8,
    pub protocol: ReceiverProtocol,
    pub command: [u8; 3],
    pub transaction_prefix: u8,
    pub transaction_modulus: u8,
    pub max_retry_in: u8,
    pub max_retry_out: u8,
    pub sleep_between_out_ms: u64,
    pub sleep_between_out_in_ms: u64,
    pub sleep_between_in_ms: u64,
    /// Keyboard middleware accepts productId as well as scalar dongleId;
    /// mouse/linker middleware uses scalar dongleId only.
    pub peer_match_product_id: bool,
    /// Independently audited discovery caller policy, separate from command
    /// transport retries. Missing policies must not be inferred from a class.
    #[serde(default)]
    pub startup_retry: Option<ReceiverStartupRetry>,
    pub source_class: String,
    pub evidence_path: String,
}

impl ReceiverCapability {
    pub fn get_report_id(&self) -> u8 {
        if self.report_id == 0 {
            self.claim_interface
        } else {
            self.report_id
        }
    }
}

pub fn all() -> &'static [ReceiverCapability] {
    #[derive(Deserialize)]
    struct Catalog {
        schema_version: u8,
        capabilities: Vec<ReceiverCapability>,
    }
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    &CATALOG
        .get_or_init(|| {
            let catalog: Catalog = serde_json::from_str(include_str!(
                "../../../../assets/data/receiver-query-capabilities.json"
            ))
            .expect("statically generated receiver capability asset must be valid");
            assert_eq!(catalog.schema_version, 1);
            catalog
        })
        .capabilities
}

pub fn capability(product_id: u16) -> Option<&'static ReceiverCapability> {
    all().iter().find(|entry| entry.product_id == product_id)
}

/// The current756 factory keeps its DualLinkKeyboard namespace when it opens
/// a cached master's physical PID. A241 owner's ordinary linker namespace
/// must not be substituted for that caller's prefix128 query.
pub fn for_source(
    product_id: u16,
    source_product_id: Option<u16>,
) -> anyhow::Result<ReceiverCapability> {
    use anyhow::{Context as _, ensure};
    let physical = capability(product_id).context("接收器没有当前查询能力")?;
    match source_product_id {
        None => Ok(physical.clone()),
        Some(source) if source == physical.source_product_id => Ok(physical.clone()),
        Some(756) => {
            ensure!(
                matches!(product_id, 164 | 179 | 241 | 757),
                "756 DualLink factory 没有该物理主设备的当前证据"
            );
            let mut selected = capability(757)
                .context("缺少756 DualLink factory 能力")?
                .clone();
            selected.product_id = physical.product_id;
            selected.claim_interface = physical.claim_interface;
            selected.report_id = physical.claim_interface;
            Ok(selected)
        }
        Some(_) => anyhow::bail!("接收器查询 caller 与当前 factory 不匹配"),
    }
}
