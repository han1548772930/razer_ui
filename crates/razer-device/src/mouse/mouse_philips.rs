//! PID 70 Philips preset setters from the current calibration engine.
//! Source apply completes on sensor, calibration and threshold acknowledgements;
//! none of these acknowledgements is an independent effective-setting readback.
use crate::{backend::FeatureTransport, device_reads::DeviceReadCapability, feature_exchange};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
#[path = "mouse_philips/configuration.rs"]
mod configuration;
#[path = "mouse_philips/measurement.rs"]
pub mod measurement;
pub use crate::feature_submission::Target;
pub use configuration::capture_projection;
pub use configuration::{ConfigChange, LiftStage, ProfileId, RawCalibration, Surface};
pub use configuration::{
    MeasurementDestination, MeasurementMode, MeasurementSave, PreparedMeasurement,
};

pub const PRODUCT_ID: u32 = 70;
pub const CALIBRATION_FIELDS: [&str; 12] = [
    "xOffset",
    "xSwing",
    "xTIAGain",
    "xReferenceHeightA",
    "xReferenceHeightB",
    "temperatureVarA",
    "temperatureVarB",
    "yOffset",
    "ySwing",
    "yTIAGain",
    "yReferenceHeightA",
    "yReferenceHeightB",
];
const SET_STATE: [u8; 3] = [3, 11, 3];
const SET_CALIBRATION: [u8; 3] = [50, 11, 8];
const SET_THRESHOLD: [u8; 3] = [8, 11, 2];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SensorCalibration {
    pub x_offset: u32,
    pub x_swing: u32,
    #[serde(rename = "xTIAGain")]
    pub x_tia_gain: u32,
    pub x_reference_height_a: u32,
    pub x_reference_height_b: u32,
    pub temperature_var_a: u32,
    pub temperature_var_b: u32,
    pub y_offset: u32,
    pub y_swing: u32,
    #[serde(rename = "yTIAGain")]
    pub y_tia_gain: u32,
    pub y_reference_height_a: u32,
    pub y_reference_height_b: u32,
}
impl SensorCalibration {
    fn values(self) -> [u32; 12] {
        [
            self.x_offset,
            self.x_swing,
            self.x_tia_gain,
            self.x_reference_height_a,
            self.x_reference_height_b,
            self.temperature_var_a,
            self.temperature_var_b,
            self.y_offset,
            self.y_swing,
            self.y_tia_gain,
            self.y_reference_height_a,
            self.y_reference_height_b,
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum Setting {
    Disabled,
    Apply {
        calibration: SensorCalibration,
        lift_off_threshold: u16,
    },
    Mini {
        calibration: SensorCalibration,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum Step {
    SensorState,
    CalibrationData,
    Threshold,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub step: Step,
    pub header: [u8; 3],
    pub payload: Vec<u8>,
}

pub fn prepare(setting: Setting) -> Vec<Command> {
    let mut commands = vec![Command {
        step: Step::SensorState,
        header: SET_STATE,
        payload: vec![0, 5, u8::from(!matches!(setting, Setting::Disabled))],
    }];
    if let Setting::Apply { calibration, .. } | Setting::Mini { calibration } = setting {
        let mut payload = vec![0, 5];
        for value in calibration.values() {
            payload.extend(value.to_be_bytes());
        }
        commands.push(Command {
            step: Step::CalibrationData,
            header: SET_CALIBRATION,
            payload,
        });
        if let Setting::Apply {
            lift_off_threshold, ..
        } = setting
        {
            let [high, low] = lift_off_threshold.to_be_bytes();
            commands.push(Command {
                step: Step::Threshold,
                header: SET_THRESHOLD,
                payload: vec![0, 5, high, low, 0, 0, 0, 0],
            });
        }
    }
    commands
}

pub fn validate_transport(cap: &DeviceReadCapability) -> Result<()> {
    ensure!(
        cap.product_id == PRODUCT_ID
            && cap.source_class == "rzDevice25Legacy"
            && cap.vendor_id == 0x1532
            && cap.report_bytes == 91
            && cap.report_id == 0
            && cap.transaction_prefix == 0
            && cap.transaction_modulus == 31,
        "Philips calibration requires the current PID 70 legacy factory"
    );
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Acknowledgement {
    pub step: Step,
    pub payload: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct WriteResult {
    pub requested: Setting,
    pub acknowledgements: Vec<Acknowledgement>,
    pub source_chain_acknowledged: bool,
    pub verified: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum FailurePhase {
    Prepare,
    BeforeCommand,
    Command,
    AfterAcknowledgement,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct WriteFailure {
    pub requested: Setting,
    pub phase: FailurePhase,
    pub step: Option<Step>,
    pub acknowledgements: Vec<Acknowledgement>,
    pub command_may_have_reached_device: bool,
    pub error: String,
}
impl std::fmt::Display for WriteFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Philips {:?} at {:?}: {}",
            self.phase, self.step, self.error
        )
    }
}
impl std::error::Error for WriteFailure {}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct SubmissionReport {
    pub requested: Setting,
    pub transport_source_class: String,
    /// setupDevice catches a failed mode initialization; retain it separately.
    pub initialization_error: Option<String>,
    pub outcome: std::result::Result<WriteResult, WriteFailure>,
}
pub type Progress = crate::feature_submission::Progress<Setting, SubmissionReport>;

pub fn apply(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    setting: Setting,
    mut next_transaction: impl FnMut() -> Result<u8>,
    validate: impl Fn() -> Result<()>,
) -> std::result::Result<WriteResult, WriteFailure> {
    let failure =
        |phase, step, acknowledgements, command_may_have_reached_device, error: anyhow::Error| {
            WriteFailure {
                requested: setting,
                phase,
                step,
                acknowledgements,
                command_may_have_reached_device,
                error: format!("{error:#}"),
            }
        };
    validate_transport(cap)
        .map_err(|error| failure(FailurePhase::Prepare, None, vec![], false, error))?;
    let mut acknowledgements = Vec::new();
    for command in prepare(setting) {
        validate().map_err(|error| {
            failure(
                FailurePhase::BeforeCommand,
                Some(command.step),
                acknowledgements.clone(),
                false,
                error,
            )
        })?;
        let transaction = next_transaction().map_err(|error| {
            failure(
                FailurePhase::BeforeCommand,
                Some(command.step),
                acknowledgements.clone(),
                false,
                error,
            )
        })?;
        let sent = std::cell::Cell::new(false);
        let payload = feature_exchange::exchange_with_report_data_size_observing_send(
            device,
            cap,
            command.header,
            &command.payload,
            transaction,
            command.header[0],
            &validate,
            &mut || sent.set(true),
        )
        .map_err(|error| {
            failure(
                FailurePhase::Command,
                Some(command.step),
                acknowledgements.clone(),
                sent.get(),
                error,
            )
        })?;
        acknowledgements.push(Acknowledgement {
            step: command.step,
            payload,
        });
        validate().map_err(|error| {
            failure(
                FailurePhase::AfterAcknowledgement,
                Some(command.step),
                acknowledgements.clone(),
                false,
                error,
            )
        })?;
    }
    Ok(WriteResult {
        requested: setting,
        acknowledgements,
        source_chain_acknowledged: true,
        verified: false,
    })
}

pub fn submit(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    setting: Setting,
    mut next_transaction: impl FnMut() -> Result<u8>,
    validate: impl Fn() -> Result<()>,
) -> SubmissionReport {
    let initialization_error = crate::mouse_legacy_polling::initialize_driver_mode(
        device,
        cap,
        &mut next_transaction,
        &validate,
    )
    .err()
    .map(|error| format!("{error:#}"));
    SubmissionReport {
        requested: setting,
        transport_source_class: cap.source_class.clone(),
        initialization_error,
        outcome: apply(device, cap, setting, next_transaction, validate),
    }
}

#[cfg(test)]
#[path = "mouse_philips/tests.rs"]
mod tests;
