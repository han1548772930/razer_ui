//! Current Joro wired function-key swap. The Pro Type Ergo single-profile
//! caller skips this command and must use its separate mapping/storage chain.
//! See docs/re/keyboard-function-swap-current-source.json for current evidence.
use crate::{
    backend::FeatureTransport, feature_exchange, keyboard_settings,
    keyboard_settings::KeyboardSettingsCapability,
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const AUDITED_WIRED_PRODUCTS: &[u32] = &[717];
const GET: [u8; 3] = [1, 2, 168];
const SET: [u8; 3] = [1, 2, 40];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FunctionKeyPrimary {
    Multimedia,
    Function,
}

impl FunctionKeyPrimary {
    pub const fn swap_byte(self) -> u8 {
        match self {
            Self::Multimedia => 0,
            Self::Function => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionKeySwap {
    /// Preserve unknown device values; do not infer a primary function from them.
    pub raw_swap: u8,
}

impl FunctionKeySwap {
    pub const fn primary(self) -> Option<FunctionKeyPrimary> {
        match self.raw_swap {
            0 => Some(FunctionKeyPrimary::Multimedia),
            1 => Some(FunctionKeyPrimary::Function),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct FunctionKeySwapWriteReport {
    pub requested_primary: FunctionKeyPrimary,
    /// Set immediately before an actual transport send, including retries.
    pub attempted: bool,
    pub send_attempts: usize,
    pub acknowledgment_data: Option<Vec<u8>>,
    pub observed: Option<FunctionKeySwap>,
    verified: bool,
}

impl FunctionKeySwapWriteReport {
    pub fn verified(&self) -> bool {
        self.verified
    }
    fn new(requested_primary: FunctionKeyPrimary) -> Self {
        Self {
            requested_primary,
            attempted: false,
            send_attempts: 0,
            acknowledgment_data: None,
            observed: None,
            verified: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum SwapFailurePhase {
    Prepare,
    Write,
    Readback,
}

#[derive(Debug)]
pub struct FunctionKeySwapWriteFailure {
    pub phase: SwapFailurePhase,
    pub report: FunctionKeySwapWriteReport,
    pub error: anyhow::Error,
}

impl fmt::Display for FunctionKeySwapWriteFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Function-key swap {:?}: {:#}", self.phase, self.error)
    }
}
impl std::error::Error for FunctionKeySwapWriteFailure {}

fn capability(product_id: u32) -> anyhow::Result<&'static KeyboardSettingsCapability> {
    ensure!(
        AUDITED_WIRED_PRODUCTS.contains(&product_id),
        "Product has no audited active wired FunctionKeySwap branch; Pro Type Ergo uses its mapping chain"
    );
    keyboard_settings::capability(product_id).context("Missing audited keyboard transport")
}

/// Check the immutable source layout even when a private test reduces retries.
/// This helper is also used by the independently gated wired knob module.
pub(crate) fn validate_wired_capability(cap: &KeyboardSettingsCapability) -> anyhow::Result<()> {
    let audited = keyboard_settings::capability(cap.product_id)
        .context("Unknown current keyboard transport")?;
    let t = &cap.transport;
    let a = &audited.transport;
    ensure!(
        cap.source_class == "rzDevice25"
            && t.product_id == cap.product_id
            && t.source_class == "rzDevice25"
            && t.vendor_id == 0x1532
            && t.claim_interface == a.claim_interface
            && t.claim_interfaces_by_pid == a.claim_interfaces_by_pid
            && t.direct_pids.as_slice() == [cap.product_id]
            && t.report_id == 0
            && t.report_bytes == 91
            && t.transaction_prefix == 0
            && t.transaction_modulus == 31
            && (1..=a.max_retry_in).contains(&t.max_retry_in)
            && (1..=a.max_retry_out).contains(&t.max_retry_out),
        "Current wired keyboard transport identity, packet or transaction namespace differs"
    );
    Ok(())
}

fn parse(data: &[u8]) -> anyhow::Result<FunctionKeySwap> {
    ensure!(!data.is_empty(), "Function-key swap getter is truncated");
    Ok(FunctionKeySwap { raw_swap: data[0] })
}

fn read_with_cap(
    device: &dyn FeatureTransport,
    cap: &KeyboardSettingsCapability,
    sequence: u8,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<FunctionKeySwap> {
    capability(cap.product_id)?;
    validate_wired_capability(cap)?;
    let transaction = keyboard_settings::wire_transaction(sequence)?;
    validate()?;
    let data =
        feature_exchange::exchange(device, &cap.transport, GET, &[0], transaction, validate)?;
    let observed = parse(&data)?;
    validate()?;
    Ok(observed)
}

pub fn read(
    device: &dyn FeatureTransport,
    product_id: u32,
    sequence: u8,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<FunctionKeySwap> {
    read_with_cap(device, capability(product_id)?, sequence, &validate)
}

fn apply_with_cap(
    device: &dyn FeatureTransport,
    cap: &KeyboardSettingsCapability,
    primary: FunctionKeyPrimary,
    confirm_with_readback: bool,
    next_sequence: &mut impl FnMut() -> anyhow::Result<u8>,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> Result<FunctionKeySwapWriteReport, FunctionKeySwapWriteFailure> {
    let mut report = FunctionKeySwapWriteReport::new(primary);
    let fail = |phase, report, error| FunctionKeySwapWriteFailure {
        phase,
        report,
        error,
    };
    let transaction = (|| {
        capability(cap.product_id)?;
        validate_wired_capability(cap)?;
        validate()?;
        keyboard_settings::wire_transaction(next_sequence()?)
    })()
    .map_err(|error| fail(SwapFailurePhase::Prepare, report.clone(), error))?;
    let data = feature_exchange::exchange_with_report_data_size_observing_send(
        device,
        &cap.transport,
        SET,
        &[primary.swap_byte()],
        transaction,
        SET[0],
        validate,
        &mut || {
            report.attempted = true;
            report.send_attempts += 1;
        },
    )
    .map_err(|error| fail(SwapFailurePhase::Write, report.clone(), error))?;
    // Original setter constructs jsonData from the requested byte, not a GET.
    report.acknowledgment_data = Some(data);
    validate().map_err(|error| fail(SwapFailurePhase::Write, report.clone(), error))?;
    if confirm_with_readback {
        let observed = next_sequence()
            .and_then(|sequence| read_with_cap(device, cap, sequence, validate))
            .map_err(|error| fail(SwapFailurePhase::Readback, report.clone(), error))?;
        report.observed = Some(observed);
        if observed.primary() != Some(primary) {
            return Err(fail(
                SwapFailurePhase::Readback,
                report,
                anyhow::anyhow!("Independent FunctionKeySwap GET differs; SET was not rolled back"),
            ));
        }
        report.verified = true;
    }
    Ok(report)
}

/// Apply the real wired setter and optionally confirm with an independent GET.
/// This does not publish appEngine mappings or save the original profile cache.
pub fn apply(
    device: &dyn FeatureTransport,
    product_id: u32,
    primary: FunctionKeyPrimary,
    confirm_with_readback: bool,
    mut next_sequence: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> Result<FunctionKeySwapWriteReport, FunctionKeySwapWriteFailure> {
    let cap = capability(product_id).map_err(|error| FunctionKeySwapWriteFailure {
        phase: SwapFailurePhase::Prepare,
        report: FunctionKeySwapWriteReport::new(primary),
        error,
    })?;
    apply_with_cap(
        device,
        cap,
        primary,
        confirm_with_readback,
        &mut next_sequence,
        &validate,
    )
}

#[cfg(test)]
#[path = "keyboard_function_swap_tests.rs"]
mod tests;
