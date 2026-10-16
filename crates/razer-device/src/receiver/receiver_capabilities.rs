//! Exact receiver bindings derived from current DeviceInfo, feature, factory,
//! protocol inheritance and command ASTs. Catalog membership is not a capability.
#[cfg(test)]
use anyhow::Context as _;
use anyhow::ensure;
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::collections::BTreeSet;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum ReceiverProtocol {
    #[serde(rename = "razer_device25_wireless_status_v2")]
    RazerDevice25WirelessStatusV2,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReceiverStartupRetry {
    pub first_peer_status: u8,
    pub delays_ms: Vec<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
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
    pub(crate) fn validate_source(&self) -> anyhow::Result<()> {
        ensure!(
            self.source_product_id != 0
                && self.product_id != 0
                && self.vendor_id != 0
                && self.report_bytes == 91
                && self.command[0] <= 80
                && self.transaction_modulus > 0
                && self.max_retry_in > 0
                && self.max_retry_out > 0
                && !self.source_class.is_empty(),
            "Invalid receiver identity or report transport"
        );
        let counter_bits = (0..self.transaction_modulus).fold(0, |bits, counter| bits | counter);
        ensure!(
            self.transaction_prefix & counter_bits == 0,
            "Receiver transaction namespace overlaps its counter"
        );
        if let Some(retry) = &self.startup_retry {
            ensure!(
                !retry.delays_ms.is_empty(),
                "Receiver startup retry policy has no attempts"
            );
        }
        Ok(())
    }

    pub fn get_report_id(&self) -> u8 {
        if self.report_id == 0 {
            self.claim_interface
        } else {
            self.report_id
        }
    }
}

#[cfg(test)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    schema_version: u8,
    #[serde(rename = "scope")]
    _scope: String,
    capabilities: Vec<ReceiverCapability>,
}

#[cfg(test)]
fn decode_catalog(json: &str) -> anyhow::Result<Catalog> {
    let catalog: Catalog =
        serde_json::from_str(json).context("Invalid receiver capability catalog")?;
    ensure!(
        catalog.schema_version == 1 && !catalog.capabilities.is_empty(),
        "Invalid receiver capability schema"
    );
    let mut products = BTreeSet::new();
    for capability in &catalog.capabilities {
        ensure!(
            products.insert(capability.product_id),
            "Repeated receiver query product"
        );
        capability.validate_source()?;
    }
    Ok(catalog)
}

pub fn all() -> &'static [ReceiverCapability] {
    static CATALOG: OnceLock<Vec<ReceiverCapability>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let products = crate::receiver_query_capabilities::products();
        for product in &products {
            product
                .validate_source()
                .expect("fixed receiver query table");
        }
        products
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receiver_catalog_validates_namespace_retry_shape_and_product_identity() {
        let source = serde_json::json!({"schema_version":1,"scope":"receiver_queries","capabilities":[all()[0]]});
        assert!(decode_catalog(&source.to_string()).is_ok());
        for mutation in 0..4 {
            let mut value = source.clone();
            match mutation {
                0 => value["capabilities"][0]["transaction_modulus"] = 0.into(),
                1 => value["capabilities"][0]["report_bytes"] = 90.into(),
                2 => value["capabilities"][0]["transaction_prefix"] = 1.into(),
                3 => {
                    let repeated = value["capabilities"][0].clone();
                    value["capabilities"].as_array_mut().unwrap().push(repeated);
                }
                _ => unreachable!(),
            }
            assert!(
                decode_catalog(&value.to_string()).is_err(),
                "mutation {mutation}"
            );
        }
    }
}
