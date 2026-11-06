//! Current earbuds auto-pause task and its connection-read initialization gate.
use crate::earbuds_protocol40::{self as protocol, *};
use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum Operation {
    Read,
    Battery { levels: Option<[u8; 2]> },
    Restore { enabled: Option<bool> },
    Write { enabled: bool },
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Report {
    pub product_id: u32,
    pub operation: Operation,
    pub connected: Option<bool>,
    pub left_battery: Option<u8>,
    pub right_battery: Option<u8>,
    pub observation: AutoPauseObservation,
    pub write: WriteOutcome,
}

impl Report {
    pub fn has_disconnected_earbud(&self) -> bool {
        self.connected == Some(true)
            && [self.left_battery, self.right_battery]
                .into_iter()
                .any(|level| level.is_none_or(|level| level == 0 || level == 255))
    }
}

/// DeviceInfo.claimInterface is 3 on 2657 and 0 on the other four products.
pub fn transport(product_id: u32) -> Result<crate::device_reads::DeviceReadCapability> {
    ensure!(
        supports_auto_pause(product_id),
        "Product has no earbuds auto-pause factory"
    );
    Ok(crate::device_reads::DeviceReadCapability {
        product_id,
        source_class: "rzDevice40".into(),
        direct_pids: vec![product_id],
        vendor_id: 0x1532,
        claim_interface: if product_id == 2657 { 3 } else { 0 },
        claim_interfaces_by_pid: Default::default(),
        physical_transports_by_pid: Default::default(),
        report_id: REPORT_ID,
        report_bytes: REPORT_LENGTH,
        transaction_prefix: 0x60,
        transaction_modulus: 32,
        queries: vec![],
        polling_codes: Default::default(),
        polling_value_offset: None,
        polling_default_hz: None,
        polling_physical_product_id: None,
        charging_codes: Default::default(),
        max_retry_in: 10,
        max_retry_out: 20,
        sleep_between_out_ms: 50,
        sleep_between_out_in_ms: 100,
        sleep_between_in_ms: 50,
    })
}

pub fn execute<S: Sleeper>(
    session: &mut Protocol40Session<'_, S>,
    product_id: u32,
    operation: Operation,
    validate: &dyn Fn() -> Result<()>,
) -> Result<Report> {
    transport(product_id)?;
    // Source user writes and profile restore do not run the battery setup again.
    // Initialization owns that gate; each operation still validates its live route.
    if let Operation::Write { enabled } = operation {
        session.set_auto_pause(enabled, validate)?;
        return Ok(Report {
            product_id,
            operation,
            connected: None,
            left_battery: None,
            right_battery: None,
            observation: AutoPauseObservation::Observed(enabled),
            write: WriteOutcome::Confirmed,
        });
    }
    if let Operation::Restore { enabled } = operation {
        let restored = protocol::restore_auto_pause(
            session,
            AutoPauseProfileField {
                is_enabled: enabled,
            },
            validate,
        )?;
        let observation = if restored.write == WriteOutcome::Confirmed {
            AutoPauseObservation::Observed(restored.published_local)
        } else {
            restored.observation
        };
        return Ok(Report {
            product_id,
            operation,
            connected: None,
            left_battery: None,
            right_battery: None,
            observation,
            write: restored.write,
        });
    }
    // Earlier four helpers explicitly supply Mouse; 2689 uses the instance type.
    let device_type = if product_id == 2689 {
        TransactionDeviceType::AudioHeadsetSpeakerMic
    } else {
        TransactionDeviceType::Mouse
    };
    let connection = session.send_command_as(
        CommandHeader::new([1, 24, 154]),
        &[1],
        device_type,
        validate,
    )?;
    ensure!(
        connection.data.first() == Some(&1),
        "Connection reply belongs to another audio type"
    );
    let connected = *connection
        .data
        .get(1)
        .context("Earbud connection reply has no status")?
        != 0;
    let mut report = Report {
        product_id,
        operation,
        connected: Some(connected),
        left_battery: None,
        right_battery: None,
        observation: AutoPauseObservation::Unobserved,
        write: WriteOutcome::Skipped,
    };
    if !connected {
        return Ok(report);
    }
    let levels = if let Operation::Battery {
        levels: Some(levels),
    } = operation
    {
        levels
    } else {
        let battery =
            session.send_command_as(CommandHeader::new([0, 7, 128]), &[], device_type, validate)?;
        ensure!(battery.data.len() >= 2, "Earbud battery reply omits a side");
        [battery.data[0], battery.data[1]]
    };
    report.left_battery = Some(levels[0]);
    report.right_battery = Some(levels[1]);
    if operation == Operation::Read {
        report.observation =
            AutoPauseObservation::Observed(session.get_auto_pause(validate)?.enabled);
    }
    validate()?;
    Ok(report)
}

#[cfg(test)]
#[path = "earbuds_auto_pause_tests.rs"]
mod tests;
