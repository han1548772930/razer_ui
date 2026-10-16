//! Current Seiren EQ calls use their exact observed wired HID collection.
use super::*;
use razer_device::seiren_eq::{self as eq, Change, Report};
pub fn resolve(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
) -> anyhow::Result<DirectRoute> {
    let cap =
        eq::capability(observed.product_id()).context("Current Seiren EQ source is absent")?;
    super::resolve(client, observed, cap)
}
impl DirectRoute {
    pub fn seiren_eq(&self, client: &mut ServiceClient, change: Change) -> anyhow::Result<Report> {
        let product_id = match self {
            Self::Collection { product_id, .. } => *product_id,
            #[cfg(windows)]
            Self::Container(target) => target.product_id,
        };
        eq::validate_change(product_id, &change)?;
        let request = match self {
            Self::Collection { node, product_id } => ServiceRequest::HidNodeSeirenEq {
                node: node.clone(),
                product_id: *product_id,
                change: change.clone(),
            },
            #[cfg(windows)]
            Self::Container(target) => ServiceRequest::DeviceSeirenEq {
                target: target.clone(),
                change: change.clone(),
            },
        };
        let reply = client.request(request)?;
        self.check_reply(&reply)?;
        ensure!(
            reply["setting_scope"] == "seiren_eq_profile",
            "Seiren EQ service setting scope differs"
        );
        let report: Report = serde_json::from_value(reply["report"].clone())?;
        ensure!(
            report.requested() == &change && report.has_stages(),
            "Seiren EQ service returned another/empty source task"
        );
        Ok(report)
    }
}
