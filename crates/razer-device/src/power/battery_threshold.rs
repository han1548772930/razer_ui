//! Current direct-mouse critical battery threshold (low-power mode setting).
//! The source sends setCriticalLevel, then confirms getCriticalLevel separately.
//! This changes the active device setting; profile/task-cache persistence is not
//! implemented here. See docs/re/battery-threshold-current.md for product gates.
use crate::{backend::FeatureTransport, device_reads::DeviceReadCapability, feature_exchange};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};

/// Each product was rechecked against its current official HTML/main manifest,
/// effective class, battery feature, command builders and setter/readback task.
/// Receivers, BLE and unknown classes must not inherit this list automatically.
pub const AUDITED_PRODUCTS: &[u32] = &[
    124, 143, 148, 156, 165, 167, 170, 175, 180, 182, 184, 185, 192, 196, 204, 211, 218, 222, 226,
    231, 235, 45066,
];
const GET: [u8; 3] = [1, 7, 129];
const SET: [u8; 3] = [1, 7, 1];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatteryThreshold {
    pub raw_value: u8,
    pub percent: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BatteryThresholdWriteResult {
    pub requested_percent: u8,
    /// The source setter parses its returned byte, but only the getter confirms
    /// the requested percentage. A successful send cannot populate this field.
    pub acknowledged: BatteryThreshold,
    pub observed: BatteryThreshold,
    pub verified: bool,
}

/// Current helper: Math.ceil(percent / 100 * 255). Bound valid UI percentages
/// before sending, rather than invoke the source's legacy >255 fallback.
pub fn encode_percent(percent: u8) -> anyhow::Result<u8> {
    ensure!(
        percent <= 100,
        "Battery threshold must be in 0..100 percent"
    );
    Ok((u16::from(percent) * 255).div_ceil(100) as u8)
}

pub fn decode(raw_value: u8) -> BatteryThreshold {
    BatteryThreshold {
        raw_value,
        percent: (u16::from(raw_value) * 100 / 255) as u8,
    }
}

fn validate_cap(cap: &DeviceReadCapability) -> anyhow::Result<()> {
    ensure!(
        AUDITED_PRODUCTS.contains(&cap.product_id)
            && cap.source_class == "rzDevice25DualLinkMouse"
            && cap.transaction_prefix == 0
            && cap.transaction_modulus == 31,
        "Current critical-battery product/transport branch has not been audited"
    );
    Ok(())
}

fn parse(data: &[u8]) -> anyhow::Result<BatteryThreshold> {
    ensure!(!data.is_empty(), "Critical-battery response is truncated");
    Ok(decode(data[0]))
}

pub fn read(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    transaction: u8,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<BatteryThreshold> {
    validate_cap(cap)?;
    let data = feature_exchange::exchange(device, cap, GET, &[], transaction, &validate)?;
    parse(&data)
}

/// Source order: setter/ACK -> cancellation guard -> getter -> percent equality.
/// Caller validation must check retained identity, cancellation and deadline.
/// Retries reuse the same transaction for one command; a fresh transaction is
/// allocated for the separate getter. No hardware rollback is claimed on error.
pub fn apply(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    percent: u8,
    mut next_transaction: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<BatteryThresholdWriteResult> {
    validate_cap(cap)?;
    let raw = encode_percent(percent)?;
    validate()?;
    let data = feature_exchange::exchange(device, cap, SET, &[raw], next_transaction()?, &validate)
        .context("Critical-battery write ACK is unconfirmed; reread before retrying")?;
    let acknowledged = parse(&data).context("Critical-battery ACK payload is truncated")?;
    validate().context("Critical-battery setting may have changed; confirmation was canceled")?;
    let observed = read(device, cap, next_transaction()?, &validate)
        .context("Critical-battery write was acknowledged, but readback is unconfirmed")?;
    ensure!(
        observed.percent == percent,
        "Critical-battery readback differs from the requested percentage; write is unconfirmed"
    );
    validate().context("Critical-battery readback session changed; write is unconfirmed")?;
    Ok(BatteryThresholdWriteResult {
        requested_percent: percent,
        acknowledged,
        observed,
        verified: true,
    })
}

#[cfg(test)]
#[path = "battery_threshold_tests.rs"]
mod tests;
