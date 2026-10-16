//! Current source-gated ordinary polling for mouse onboard profiles.
//! Source fallback display values are separate from raw firmware observations.
use crate::{
    backend::FeatureTransport, device_reads::DeviceReadCapability,
    feature_exchange::exchange_with_report_data_size_observing_send,
};
use anyhow::{Result, ensure};
use serde::Serialize;

include!("mouse_profile_polling_data_generated.rs");
const GET: [u8; 3] = [2, 0, 142];
const SET: [u8; 3] = [2, 0, 14];

pub fn capability(product_id: u32) -> bool {
    PROFILE_POLLING_PRODUCT_IDS.contains(&product_id)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProfilePollingObservation {
    pub profile_id: u8,
    pub raw_code: u8,
    /// None preserves an unknown code instead of inventing a known rate.
    pub known_hz: Option<u16>,
    /// Original parser's displayed Hz, including its unknown-code fallback.
    pub source_hz: u16,
    pub raw_payload: Vec<u8>,
}
fn parse(profile_id: u8, payload: &[u8]) -> Result<ProfilePollingObservation> {
    ensure!(payload.len() >= 2, "Profile polling response is truncated");
    ensure!(
        payload[0] == profile_id,
        "Profile polling response identity differs"
    );
    let known_hz = PROFILE_POLLING_RATES
        .iter()
        .find(|&&(_, code)| code == payload[1])
        .map(|&(hz, _)| hz);
    Ok(ProfilePollingObservation {
        profile_id,
        raw_code: payload[1],
        known_hz,
        source_hz: known_hz.unwrap_or(100),
        raw_payload: payload.to_vec(),
    })
}

#[derive(Clone, Debug, Serialize)]
pub struct ProfilePollingWriteReport {
    pub profile_id: u8,
    pub requested_code: u8,
    pub attempted: bool,
    pub acknowledgment_received: bool,
    pub acknowledgment_data: Vec<u8>,
    pub observed: Option<ProfilePollingObservation>,
    pub source_completed: bool,
    verified: bool,
}
impl ProfilePollingWriteReport {
    pub fn verified(&self) -> bool {
        self.verified
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ProfilePollingFailurePhase {
    Prepare,
    Write,
    Readback,
    Cancelled,
}
#[derive(Debug)]
pub struct ProfilePollingWriteFailure {
    pub phase: ProfilePollingFailurePhase,
    pub progress: ProfilePollingWriteReport,
    error: anyhow::Error,
}
impl ProfilePollingWriteFailure {
    pub fn device_state_may_have_changed(&self) -> bool {
        self.progress.attempted
    }
}
impl std::fmt::Display for ProfilePollingWriteFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Mouse profile polling {:?} failed: {:#}",
            self.phase, self.error
        )
    }
}
impl std::error::Error for ProfilePollingWriteFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.error.as_ref())
    }
}
fn initial_report(profile_id: u8, raw_code: u8) -> ProfilePollingWriteReport {
    ProfilePollingWriteReport {
        profile_id,
        requested_code: raw_code,
        attempted: false,
        acknowledgment_received: false,
        acknowledgment_data: Vec::new(),
        observed: None,
        source_completed: false,
        verified: false,
    }
}

pub struct MouseProfilePolling<'a> {
    device: &'a dyn FeatureTransport,
    transport: &'a DeviceReadCapability,
}
impl<'a> MouseProfilePolling<'a> {
    pub fn new(
        device: &'a dyn FeatureTransport,
        transport: &'a DeviceReadCapability,
    ) -> Result<Self> {
        ensure!(
            capability(transport.product_id),
            "No current mouse profile polling capability"
        );
        ensure!(
            transport.source_class == "rzDevice25DualLinkMouse"
                && transport.vendor_id == 0x1532
                && transport.claim_interface == if transport.product_id == 226 { 3 } else { 0 }
                && transport.claim_interfaces_by_pid.is_empty()
                && transport.report_id == 0
                && transport.report_bytes == 91
                && transport.transaction_prefix == 0
                && transport.transaction_modulus == 31,
            "Profile polling requires the current direct mouse Feature transport"
        );
        Ok(Self { device, transport })
    }
    pub fn read(
        &self,
        profile_id: u8,
        mut next_transaction: impl FnMut() -> Result<u8>,
        validate: impl Fn() -> Result<()>,
    ) -> Result<ProfilePollingObservation> {
        ensure!(profile_id > 0, "Mouse profile ID cannot be zero");
        validate()?;
        let transaction = next_transaction()?;
        let payload = exchange_with_report_data_size_observing_send(
            self.device,
            self.transport,
            GET,
            &[profile_id],
            transaction,
            2,
            &validate,
            &mut || {},
        )?;
        parse(profile_id, &payload)
    }
    /// Write one of the source's seven supported ordinary rates.
    pub fn apply(
        &self,
        profile_id: u8,
        hz: u16,
        confirm_with_readback: bool,
        next_transaction: impl FnMut() -> Result<u8>,
        validate: impl Fn() -> Result<()>,
    ) -> std::result::Result<ProfilePollingWriteReport, ProfilePollingWriteFailure> {
        let raw_code = PROFILE_POLLING_RATES
            .iter()
            .find(|&&(rate, _)| rate == hz)
            .map(|&(_, code)| code);
        let Some(raw_code) = raw_code else {
            return Err(ProfilePollingWriteFailure {
                phase: ProfilePollingFailurePhase::Prepare,
                progress: initial_report(profile_id, 0),
                error: anyhow::anyhow!("Unsupported ordinary profile polling rate {hz}"),
            });
        };
        self.write_code(
            profile_id,
            raw_code,
            confirm_with_readback,
            next_transaction,
            validate,
        )
    }
    /// Restore exactly the raw observed profile code, including an unknown one.
    /// This is a Rust preservation policy, not the source's lossy Hz reencoding.
    pub fn restore(
        &self,
        observed: &ProfilePollingObservation,
        confirm_with_readback: bool,
        next_transaction: impl FnMut() -> Result<u8>,
        validate: impl Fn() -> Result<()>,
    ) -> std::result::Result<ProfilePollingWriteReport, ProfilePollingWriteFailure> {
        let checked = parse(observed.profile_id, &observed.raw_payload).and_then(|parsed| {
            ensure!(
                parsed == *observed,
                "Raw profile polling observation disagrees"
            );
            Ok(())
        });
        checked.map_err(|error| ProfilePollingWriteFailure {
            phase: ProfilePollingFailurePhase::Prepare,
            progress: initial_report(observed.profile_id, observed.raw_code),
            error,
        })?;
        self.write_code(
            observed.profile_id,
            observed.raw_code,
            confirm_with_readback,
            next_transaction,
            validate,
        )
    }
    fn write_code(
        &self,
        profile_id: u8,
        raw_code: u8,
        confirm_with_readback: bool,
        mut next_transaction: impl FnMut() -> Result<u8>,
        validate: impl Fn() -> Result<()>,
    ) -> std::result::Result<ProfilePollingWriteReport, ProfilePollingWriteFailure> {
        let mut report = initial_report(profile_id, raw_code);
        if profile_id == 0 {
            return Err(ProfilePollingWriteFailure {
                phase: ProfilePollingFailurePhase::Prepare,
                progress: report,
                error: anyhow::anyhow!("Mouse profile ID cannot be zero"),
            });
        }
        if let Err(error) = validate() {
            return Err(ProfilePollingWriteFailure {
                phase: ProfilePollingFailurePhase::Cancelled,
                progress: report,
                error,
            });
        }
        let transaction = next_transaction().map_err(|error| ProfilePollingWriteFailure {
            phase: ProfilePollingFailurePhase::Write,
            progress: report.clone(),
            error,
        })?;
        let cancelled = std::cell::Cell::new(false);
        let attempted = std::cell::Cell::new(false);
        let guarded = || validate().inspect_err(|_| cancelled.set(true));
        let response = exchange_with_report_data_size_observing_send(
            self.device,
            self.transport,
            SET,
            &[profile_id, raw_code],
            transaction,
            2,
            &guarded,
            &mut || attempted.set(true),
        );
        report.attempted = attempted.get();
        report.acknowledgment_data = response.map_err(|error| ProfilePollingWriteFailure {
            phase: if cancelled.get() {
                ProfilePollingFailurePhase::Cancelled
            } else {
                ProfilePollingFailurePhase::Write
            },
            progress: report.clone(),
            error,
        })?;
        report.acknowledgment_received = true;
        if let Err(error) = validate() {
            return Err(ProfilePollingWriteFailure {
                phase: ProfilePollingFailurePhase::Cancelled,
                progress: report,
                error,
            });
        }
        report.source_completed = true;
        if !confirm_with_readback {
            return Ok(report);
        }
        let observed = self
            .read(profile_id, &mut next_transaction, &guarded)
            .map_err(|error| ProfilePollingWriteFailure {
                phase: if cancelled.get() {
                    ProfilePollingFailurePhase::Cancelled
                } else {
                    ProfilePollingFailurePhase::Readback
                },
                progress: report.clone(),
                error,
            })?;
        report.verified = observed.raw_code == raw_code;
        report.observed = Some(observed);
        if !report.verified {
            return Err(ProfilePollingWriteFailure {
                phase: ProfilePollingFailurePhase::Readback,
                progress: report,
                error: anyhow::anyhow!("Independent profile polling GET code differs"),
            });
        }
        Ok(report)
    }
}

#[cfg(test)]
#[path = "mouse_profile_polling_tests.rs"]
mod tests;
