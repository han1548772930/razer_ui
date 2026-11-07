//! Current earbuds auto-pause task and its connection-read initialization gate.
use crate::earbuds_protocol40::{self as protocol, *};
use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum Operation {
    Equalizer { setting: crate::earbuds_equalizer::Setting },
    /// Source audioInputSource command; no persisted value or restore.
    SwitchInputSource,
    Anc {
        setting: AncSetting,
    },
    Read,
    Firmware,
    Gesture {
        enabled: bool,
    },
    RestoreGesture {
        enabled: Option<bool>,
    },
    TapSpeed {
        speed: crate::earbuds_gesture::TapSpeed,
    },
    RestoreTapSpeed {
        speed: Option<crate::earbuds_gesture::TapSpeed>,
    },
    Battery {
        levels: Option<[u8; 2]>,
    },
    Restore {
        enabled: Option<bool>,
    },
    Write {
        enabled: bool,
    },
    DoNotDisturb {
        enabled: bool,
    },
    VoicePrompt {
        enabled: bool,
    },
    RestoreVoicePrompt {
        enabled: Option<bool>,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub enum FirmwareObservation {
    #[default]
    Unobserved,
    Observed(crate::earbuds_firmware::Reading),
    ReadFailed(String),
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
    #[serde(default)]
    pub do_not_disturb: Option<crate::earbuds_do_not_disturb::Observation>,
    #[serde(default)]
    pub voice_prompt: Option<ToggleObservation>,
    #[serde(default)]
    pub firmware: FirmwareObservation,
    #[serde(default)]
    pub gesture: Option<ToggleObservation>,
    #[serde(default)]
    pub tap_speed: Option<crate::earbuds_gesture::TapObservation>,
    #[serde(default)]
    pub anc: Option<crate::earbuds_anc::Report>,
}

impl Report {
    pub fn has_disconnected_earbud(&self) -> bool {
        self.connected == Some(false)
            || (self.connected == Some(true)
                && [self.left_battery, self.right_battery]
                    .into_iter()
                    .any(|level| level.is_none_or(|level| level == 0 || level == 255)))
    }
}

/// Current rzDevice40 enum: Disconnected 0, LeftOnly 1, RightOnly 2, BothConnected 3.
pub fn connection_is_online(status: u8) -> Result<bool> {
    ensure!(status <= 3, "Unknown earbud connection status {status}");
    Ok(status != 0)
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
    anc_cache: &crate::earbuds_anc::Cache,
    validate: &dyn Fn() -> Result<()>,
    validate_connection: &dyn Fn() -> Result<()>,
) -> Result<Report> {
    transport(product_id)?;
    if operation == Operation::SwitchInputSource || matches!(operation, Operation::Equalizer { .. }) {
        if let Operation::Equalizer { setting } = operation {
            crate::earbuds_equalizer::apply(session, product_id, setting, validate)?;
        } else {
            session.send_command(CommandHeader::new([0, 24, 29]), &[], validate)?;
        }
        return Ok(Report {
            product_id, operation, connected: None, left_battery: None, right_battery: None,
            observation: AutoPauseObservation::Unobserved, write: WriteOutcome::Confirmed,
            do_not_disturb: None, voice_prompt: None, firmware: FirmwareObservation::Unobserved,
            gesture: None, tap_speed: None, anc: None,
        });
    }
    if let Operation::Anc { setting } = operation {
        let anc = anc_cache.execute(session, product_id, setting, validate, validate_connection)?;
        return Ok(Report {
            product_id,
            operation,
            connected: None,
            left_battery: None,
            right_battery: None,
            observation: AutoPauseObservation::Unobserved,
            write: WriteOutcome::Skipped,
            do_not_disturb: None,
            voice_prompt: None,
            firmware: FirmwareObservation::Unobserved,
            gesture: None,
            tap_speed: None,
            anc: Some(anc),
        });
    }
    if matches!(
        operation,
        Operation::Gesture { .. }
            | Operation::RestoreGesture { .. }
            | Operation::TapSpeed { .. }
            | Operation::RestoreTapSpeed { .. }
    ) {
        use crate::earbuds_gesture as gesture;
        let (observed_gesture, tap_speed, write) = match operation {
            Operation::Gesture { enabled } => {
                gesture::write(session, enabled, validate)?;
                (
                    Some(ToggleObservation::Observed(enabled)),
                    None,
                    WriteOutcome::Confirmed,
                )
            }
            Operation::RestoreGesture { enabled } => {
                let result = gesture::restore(session, enabled, validate)?;
                (Some(result.observation), None, result.write)
            }
            Operation::TapSpeed { speed } => {
                gesture::write_tap(session, speed, validate)?;
                (
                    None,
                    Some(gesture::TapObservation::Observed(speed.ticks())),
                    WriteOutcome::Confirmed,
                )
            }
            Operation::RestoreTapSpeed { speed } => {
                let result = gesture::restore_tap(session, speed, validate)?;
                (None, Some(result.observation), result.write)
            }
            _ => unreachable!(),
        };
        return Ok(Report {
            product_id,
            operation,
            connected: None,
            left_battery: None,
            right_battery: None,
            observation: AutoPauseObservation::Unobserved,
            write,
            do_not_disturb: None,
            voice_prompt: None,
            firmware: FirmwareObservation::Unobserved,
            gesture: observed_gesture,
            tap_speed,
            anc: None,
        });
    }
    if operation == Operation::Firmware {
        let firmware = match crate::earbuds_firmware::read(session, product_id, validate) {
            Ok(reading) => FirmwareObservation::Observed(reading),
            Err(error) => {
                validate()?;
                FirmwareObservation::ReadFailed(format!("{error:#}"))
            }
        };
        return Ok(Report {
            product_id,
            operation,
            connected: None,
            left_battery: None,
            right_battery: None,
            observation: AutoPauseObservation::Unobserved,
            write: WriteOutcome::Skipped,
            do_not_disturb: None,
            voice_prompt: None,
            firmware,
            gesture: None,
            tap_speed: None,
            anc: None,
        });
    }
    if let Operation::VoicePrompt { enabled } = operation {
        crate::earbuds_voice_prompt::write(session, enabled, validate)?;
        return Ok(Report {
            product_id,
            operation,
            connected: None,
            left_battery: None,
            right_battery: None,
            observation: AutoPauseObservation::Unobserved,
            write: WriteOutcome::Confirmed,
            do_not_disturb: None,
            voice_prompt: Some(ToggleObservation::Observed(enabled)),
            firmware: FirmwareObservation::Unobserved,
            gesture: None,
            tap_speed: None,
            anc: None,
        });
    }
    if let Operation::RestoreVoicePrompt { enabled } = operation {
        let result = crate::earbuds_voice_prompt::restore(session, enabled, validate)?;
        return Ok(Report {
            product_id,
            operation,
            connected: None,
            left_battery: None,
            right_battery: None,
            observation: AutoPauseObservation::Unobserved,
            write: result.write,
            do_not_disturb: None,
            voice_prompt: Some(result.observation),
            firmware: FirmwareObservation::Unobserved,
            gesture: None,
            tap_speed: None,
            anc: None,
        });
    }
    if let Operation::DoNotDisturb { enabled } = operation {
        let (observed, write) =
            match crate::earbuds_do_not_disturb::write(session, enabled, validate) {
                Ok(result) => (
                    ToggleObservation::Observed(result.observation),
                    result.write,
                ),
                Err(error) => {
                    validate()?;
                    let observed = ToggleObservation::from_read_error(&error);
                    if !matches!(observed, ToggleObservation::Unknown(_)) {
                        return Err(error);
                    }
                    (observed, WriteOutcome::Failed(format!("{error:#}")))
                }
            };
        return Ok(Report {
            product_id,
            operation,
            connected: None,
            left_battery: None,
            right_battery: None,
            observation: AutoPauseObservation::Unobserved,
            write,
            do_not_disturb: Some(observed),
            voice_prompt: None,
            firmware: FirmwareObservation::Unobserved,
            gesture: None,
            tap_speed: None,
            anc: None,
        });
    }
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
            do_not_disturb: None,
            voice_prompt: None,
            firmware: FirmwareObservation::Unobserved,
            gesture: None,
            tap_speed: None,
            anc: None,
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
            do_not_disturb: None,
            voice_prompt: None,
            firmware: FirmwareObservation::Unobserved,
            gesture: None,
            tap_speed: None,
            anc: None,
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
    let connected = connection_is_online(
        *connection
            .data
            .get(1)
            .context("Earbud connection reply has no status")?,
    )?;
    let mut report = Report {
        product_id,
        operation,
        connected: Some(connected),
        left_battery: None,
        right_battery: None,
        observation: AutoPauseObservation::Unobserved,
        write: WriteOutcome::Skipped,
        do_not_disturb: None,
        voice_prompt: None,
        firmware: FirmwareObservation::Unobserved,
        gesture: None,
        tap_speed: None,
        anc: None,
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
    validate()?;
    Ok(report)
}

#[cfg(test)]
#[path = "earbuds_auto_pause_tests.rs"]
mod tests;
