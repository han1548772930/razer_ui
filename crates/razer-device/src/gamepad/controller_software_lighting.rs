//! Kitsune geometry and direct Feature frame submission from its current
//! product manifest and SagePC adapter. Shared engine data has one owner.
pub use crate::lighting_engine::{
    ColorStop, EffectParameters, EngineEffect, EngineFrame, LedConfiguration, LedMapping,
    QuickEffectLevel, SoftwareEffect, SoftwarePhase, SoftwareStatus, WaveDirection,
};
use anyhow::ensure;

// The lighting-engine manifest creates its own SagePC instance. The common
// adapter clamps its declared IN retry count to the Protocol25 limit of ten.
const SOFTWARE_COMMAND_INTERVAL: std::time::Duration = std::time::Duration::from_millis(5);

pub fn led_configuration(edition: u8) -> anyhow::Result<LedConfiguration> {
    ensure!(
        matches!(edition, 0 | 128..=133),
        "Kitsune edition has no current original lighting manifest"
    );
    let mut led_matrix = Vec::with_capacity(32);
    for row in 0..8 {
        for col in 0..10 {
            let index = match (row, col) {
                (0, col) => col,
                (7, col) => 25 - col,
                (row, 0) => 32 - row,
                (row, 9) => 9 + row,
                _ => continue,
            };
            led_matrix.push(LedMapping {
                matrix_pos: [row, col],
                device_pos: [0, index],
            });
        }
    }
    Ok(LedConfiguration {
        version: 2,
        vendor_id: 5426,
        product_id: 4115,
        edition,
        layout: 0,
        matrix_max_row: 8,
        matrix_max_col: 10,
        device_max_row: 1,
        device_max_col: 32,
        led_width: 40,
        led_height: 40,
        led_width_offset: 0,
        led_height_offset: 0,
        max_region: 0,
        region_max_row: 0,
        region_max_col: 0,
        led_matrix,
        led_regions: Vec::new(),
        led_input_map: Vec::new(),
    })
}

impl EngineFrame {
    pub fn validate_kitsune(&self, device_handle: u32) -> anyhow::Result<()> {
        ensure!(
            self.device_handle == device_handle,
            "Lighting frame belongs to another native device"
        );
        ensure!(
            self.region_id == 0
                && self.num_row == 1
                && self.num_col == 32
                && self.frame.len() == 96,
            "Lighting frame differs from the current Kitsune device geometry"
        );
        Ok(())
    }
}

/// Encode actual native device-order RGB bytes. Current SagePC splits its
/// single row into 25-LED packets and sends without a Feature IN transaction.
pub fn frame_reports(
    frame: &EngineFrame,
    device_handle: u32,
    transaction: &mut u8,
) -> anyhow::Result<Vec<[u8; 91]>> {
    frame.validate_kitsune(device_handle)?;
    let mut reports = Vec::with_capacity(2);
    for start in [0usize, 25] {
        reports.push(frame_report(frame, start, transaction)?);
    }
    Ok(reports)
}

fn frame_report(
    frame: &EngineFrame,
    start: usize,
    transaction: &mut u8,
) -> anyhow::Result<[u8; 91]> {
    let end = (start + 25).min(32);
    let mut data = vec![0, 0, 0, start as u8, (end - 1) as u8];
    data.extend_from_slice(&frame.frame[start * 3..end * 3]);
    let report =
        super::controller_feature::report(4, *transaction, [data.len() as u8, 15, 3], &data)?;
    *transaction = (*transaction + 1) % 31;
    Ok(report)
}

#[derive(Debug, Default)]
pub struct FrameSubmission {
    pub reports_submitted: u8,
    pub complete: bool,
    pub errors: Vec<String>,
    pub interruption: Option<anyhow::Error>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct FrameSubmissionFailure {
    pub reports_submitted: u8,
    pub errors: Vec<String>,
    pub interruption: Option<String>,
}

impl std::fmt::Display for FrameSubmissionFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Controller lighting exchange failed after submitting {} reports",
            self.reports_submitted
        )
    }
}

pub struct SoftwareHid<'a, T: crate::backend::FeatureTransport + ?Sized> {
    transport: &'a T,
    transaction: u8,
}
impl<'a, T: crate::backend::FeatureTransport + ?Sized> SoftwareHid<'a, T> {
    pub fn new(transport: &'a T) -> Self {
        Self {
            transport,
            transaction: 0,
        }
    }
    pub fn select_custom(
        &mut self,
        wait: &mut impl FnMut(std::time::Duration) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        self.effect(0, 8, wait)
    }
    pub fn restore_on_exit(
        &mut self,
        wait: &mut impl FnMut(std::time::Duration) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        self.effect(1, 3, wait)
    }
    fn effect(
        &mut self,
        profile: u8,
        effect: u8,
        wait: &mut impl FnMut(std::time::Duration) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let request = super::controller_feature::report(
            4,
            self.transaction,
            [6, 15, 2],
            &[profile, 0, effect, 0, 0, 0],
        )?;
        self.transaction = (self.transaction + 1) % 31;
        super::controller_feature::exchange(
            self.transport,
            &request,
            SOFTWARE_COMMAND_INTERVAL,
            wait,
        )?;
        Ok(())
    }
    pub fn send_frame(
        &mut self,
        frame: &EngineFrame,
        device_handle: u32,
        wait: &mut impl FnMut(std::time::Duration) -> anyhow::Result<()>,
    ) -> anyhow::Result<FrameSubmission> {
        frame.validate_kitsune(device_handle)?;
        let mut result = FrameSubmission::default();
        for start in [0, 25] {
            if let Err(error) = wait(std::time::Duration::ZERO) {
                result.interruption = Some(error);
                return Ok(result);
            }
            let report = frame_report(frame, start, &mut self.transaction)?;
            let options = crate::feature_exchange::with_protocol25_scope(
                crate::backend::ExchangeOptions::mutex_out(SOFTWARE_COMMAND_INTERVAL),
                "rzDevice25",
            );
            let submitted =
                crate::feature_exchange::with_exchange(self.transport, options, |transport, _| {
                    let mut last_error = String::from("No controller frame OUT return");
                    for _ in 0..20 {
                        wait(std::time::Duration::ZERO)?;
                        let sent = transport.send_feature_at_collection_max_counted(&report);
                        let complete = match &sent {
                            Ok(count) => *count == report.len(),
                            Err(error) => matches!(
                                crate::backend::native_io_outcome(error),
                                Some(crate::backend::NativeIoOutcome::Returned { byte_count })
                                    if byte_count == report.len()
                            ),
                        };
                        if complete {
                            result.reports_submitted += 1;
                            result.complete = result.reports_submitted == 2;
                        }
                        if let Err(validation) = wait(std::time::Duration::ZERO) {
                            return Err(crate::backend::complete_native_io(sent, validation));
                        }
                        match sent {
                            Ok(count) if count == report.len() => return Ok(true),
                            Ok(count) => {
                                last_error = format!(
                                    "Controller frame OUT returned {count} bytes; expected {}",
                                    report.len()
                                );
                            }
                            Err(error)
                                if super::controller_feature::terminal_io_failure(&error) =>
                            {
                                return Err(error);
                            }
                            Err(error) => last_error = format!("{error:#}"),
                        }
                    }
                    result.errors.push(last_error);
                    Ok(false)
                });
            let submitted = submitted.map_err(|error| {
                error.context(FrameSubmissionFailure {
                    reports_submitted: result.reports_submitted,
                    errors: result.errors.clone(),
                    interruption: result
                        .interruption
                        .as_ref()
                        .map(|error| format!("{error:#}")),
                })
            })?;
            // SagePC waits only after sendCommand has returned and released its
            // own HID cycle, including after the final successful chunk.
            if submitted {
                if let Err(error) = wait(std::time::Duration::from_millis(1)) {
                    result.interruption = Some(error);
                    return Ok(result);
                }
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
#[path = "controller_software_lighting_tests.rs"]
mod tests;
