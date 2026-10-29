//! Current PID691 physical PID692 receiver indicator. Device-wide state is
//! separate from profile drafts; a setter ACK never invents a getter result.
use crate::{
    backend::{FeatureTransport, HidNode},
    device_reads::{DeviceReadCapability, DeviceReadTarget},
    feature_exchange,
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};

pub const PRODUCT_ID: u32 = 691;
pub const RECEIVER_ID: u32 = 692;
pub const GET: [u8; 3] = [1, 7, 144];
pub const SET: [u8; 3] = [1, 7, 16];

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Target {
    Collection { node: HidNode, product_id: u32 },
    WindowsContainer(DeviceReadTarget),
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Change {
    Initialize,
    Set(u8),
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Report {
    pub queried_status: Option<u8>,
    pub requested_status: Option<u8>,
    pub setter_acknowledgement: Option<Vec<u8>>,
}

#[derive(Debug)]
struct FailedReport(Report);

impl std::fmt::Display for FailedReport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Keyboard indicator operation failed with retained transport evidence")
    }
}

/// Partial transport evidence is diagnostic and never a successful change.
pub fn failed_report(error: &anyhow::Error) -> Option<&Report> {
    error
        .downcast_ref::<FailedReport>()
        .map(|failure| &failure.0)
}

pub fn capability(product_id: u32, physical: u32) -> anyhow::Result<DeviceReadCapability> {
    ensure!(
        product_id == PRODUCT_ID && physical == RECEIVER_ID,
        "Keyboard indicator requires its actual PID692 receiver"
    );
    // 691 connectRZDevice.rzDevice25Oled uses the base Protocol25 transaction
    // sequence and five times the three ordinary protocol delays.
    crate::keyboard_metadata::hid_capability(product_id, physical)
        .cloned()
        .context("Current keyboard indicator transport factory is absent")
}
pub fn execute(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    change: &Change,
    mut next: impl FnMut() -> anyhow::Result<u8>,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Report> {
    capability(cap.product_id, RECEIVER_ID)?;
    let mut report = Report {
        queried_status: None,
        requested_status: None,
        setter_acknowledgement: None,
    };
    let status = match change {
        Change::Set(status) => {
            ensure!(
                (1..=3).contains(status),
                "Keyboard indicator selection is outside current UI values"
            );
            Some(*status)
        }
        Change::Initialize => {
            let bytes = feature_exchange::exchange(device, cap, GET, &[0], next()?, validate)?;
            // Original parser sets jsonData only for a nonempty response;
            // initialization enqueues ON_SET only for a truthy status.
            report.queried_status = bytes.first().copied();
            report.queried_status.filter(|status| *status != 0)
        }
    };
    if let Some(status) = status {
        report.requested_status = Some(status);
        let result = feature_exchange::exchange(device, cap, SET, &[status], next()?, validate);
        report.setter_acknowledgement =
            feature_exchange::acknowledged_reply(&result).map(<[u8]>::to_vec);
        result.map_err(|error| error.context(FailedReport(report.clone())))?;
    }
    validate().map_err(|error| error.context(FailedReport(report.clone())))?;
    Ok(report)
}

#[cfg(test)]
#[path = "keyboard_indicator_tests.rs"]
mod tests;
