//! Current Orca Plus tracking and asymmetric cutoff over direct Feature HID.
//! The actual engine ends at setter ACKs; it has no effective lift-setting
//! getter. A completed command chain therefore does not claim verified settings.
use crate::{backend::FeatureTransport, device_reads::DeviceReadCapability, feature_exchange};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

const CLASS: u8 = 0;
const SENSOR: u8 = 4;
const GET_STATE: [u8; 3] = [3, 11, 131];
const SET_STATE: [u8; 3] = [3, 11, 3];
const GET_CONFIG: [u8; 3] = [10, 11, 133];
const SET_CONFIG: [u8; 3] = [10, 11, 5];
const SET_LIFT: [u8; 3] = [4, 11, 11];

#[derive(Clone, Copy, Debug)]
pub struct TrackingCapability {
    product_id: u32,
    symmetric_sensor_enabled: bool,
}

const PRODUCTS: &[TrackingCapability] = &[
    TrackingCapability {
        product_id: 170,
        symmetric_sensor_enabled: true,
    },
    TrackingCapability {
        product_id: 204,
        symmetric_sensor_enabled: true,
    },
    TrackingCapability {
        product_id: 226,
        symmetric_sensor_enabled: true,
    },
    TrackingCapability {
        product_id: 45066,
        symmetric_sensor_enabled: true,
    },
];

pub fn capability(product_id: u32) -> Option<&'static TrackingCapability> {
    PRODUCTS.iter().find(|cap| cap.product_id == product_id)
}

impl TrackingCapability {
    pub fn product_id(&self) -> u32 {
        self.product_id
    }
    fn validate_transport(&self, cap: &DeviceReadCapability) -> Result<()> {
        ensure!(
            cap.product_id == self.product_id
                && cap.source_class == "rzDevice25DualLinkMouse"
                && cap.transaction_prefix == 0
                && cap.transaction_modulus == 31,
            "Orca Plus tracking product/transport branch has not been audited"
        );
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum TrackingSetting {
    Symmetric {
        tracking_distance: u8,
    },
    Asymmetric {
        landing_distance: u8,
        lift_off_distance: u8,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum TrackingStep {
    SensorState,
    LiftSetting,
    CutoffConfiguration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackingCommand {
    pub step: TrackingStep,
    pub header: [u8; 3],
    pub payload: Vec<u8>,
}

/// Distances are the current source slider units, not millimetres inferred from
/// their numeric values. The source encodes each selected distance minus one.
pub fn prepare(cap: &TrackingCapability, setting: TrackingSetting) -> Result<Vec<TrackingCommand>> {
    let (enabled, lift_mode, lift_height, cutoffs) = match setting {
        TrackingSetting::Symmetric { tracking_distance } => {
            ensure!(
                (1..=3).contains(&tracking_distance),
                "Tracking distance must be a current source stage 1..3"
            );
            (cap.symmetric_sensor_enabled, 1, tracking_distance - 1, None)
        }
        TrackingSetting::Asymmetric {
            landing_distance,
            lift_off_distance,
        } => {
            ensure!(
                landing_distance > 0
                    && landing_distance < lift_off_distance
                    && lift_off_distance <= 26,
                "Asymmetric source cutoff distances must satisfy 1 <= landing < lift <= 26"
            );
            (
                true,
                4,
                0,
                Some([lift_off_distance - 1, landing_distance - 1]),
            )
        }
    };
    let mut commands = vec![
        TrackingCommand {
            step: TrackingStep::SensorState,
            header: SET_STATE,
            payload: vec![CLASS, SENSOR, u8::from(enabled)],
        },
        TrackingCommand {
            step: TrackingStep::LiftSetting,
            header: SET_LIFT,
            payload: vec![CLASS, SENSOR, lift_mode, lift_height],
        },
    ];
    if let Some(cutoffs) = cutoffs {
        // The helper allocates ten bytes and leaves its other six parameters 0.
        let mut payload = vec![0; 10];
        payload[..4].copy_from_slice(&[CLASS, SENSOR, cutoffs[0], cutoffs[1]]);
        commands.push(TrackingCommand {
            step: TrackingStep::CutoffConfiguration,
            header: SET_CONFIG,
            payload,
        });
    }
    Ok(commands)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct SensorState {
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SensorConfiguration {
    /// Actual returned parameters, including any additional source parser field.
    pub parameters: Vec<u8>,
}

fn check_subject(data: &[u8], minimum: usize) -> Result<()> {
    ensure!(
        data.len() >= minimum,
        "Proximity sensor response is truncated"
    );
    ensure!(
        data[0] == CLASS && data[1] == SENSOR,
        "Proximity sensor response refers to another subject"
    );
    Ok(())
}

pub fn read_state(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    transaction: u8,
    validate: impl Fn() -> Result<()>,
) -> Result<SensorState> {
    capability(cap.product_id)
        .ok_or_else(|| anyhow::anyhow!("Product has no audited Orca Plus tracking branch"))?
        .validate_transport(cap)?;
    let data = feature_exchange::exchange(
        device,
        cap,
        GET_STATE,
        &[CLASS, SENSOR],
        transaction,
        &validate,
    )?;
    check_subject(&data, 3)?;
    ensure!(
        data[2] <= 1,
        "Proximity sensor state is outside the source boolean range"
    );
    Ok(SensorState {
        enabled: data[2] == 1,
    })
}

pub fn read_configuration(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    transaction: u8,
    validate: impl Fn() -> Result<()>,
) -> Result<SensorConfiguration> {
    capability(cap.product_id)
        .ok_or_else(|| anyhow::anyhow!("Product has no audited Orca Plus tracking branch"))?
        .validate_transport(cap)?;
    let data = feature_exchange::exchange(
        device,
        cap,
        GET_CONFIG,
        &[CLASS, SENSOR],
        transaction,
        &validate,
    )?;
    // The source reads parm0..parm8; require all nine actual parameter bytes.
    check_subject(&data, 11)?;
    Ok(SensorConfiguration {
        parameters: data[2..].to_vec(),
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TrackingAcknowledgement {
    pub step: TrackingStep,
    /// Raw ACK bytes are not an independent observation of the current setting.
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TrackingWriteResult {
    pub requested: TrackingSetting,
    pub acknowledgements: Vec<TrackingAcknowledgement>,
    pub source_chain_acknowledged: bool,
    /// Always false: current source provides no effective lift-setting getter.
    pub verified: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum TrackingFailurePhase {
    Prepare,
    BeforeCommand,
    Command,
    AfterAcknowledgement,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TrackingWriteFailure {
    pub requested: TrackingSetting,
    pub phase: TrackingFailurePhase,
    pub step: Option<TrackingStep>,
    pub acknowledgements: Vec<TrackingAcknowledgement>,
    /// A failed/canceled exchange may already have reached the device. No retry
    /// of the whole source chain or successful rollback is inferred from this.
    pub command_may_have_reached_device: bool,
    pub error: String,
}

impl std::fmt::Display for TrackingWriteFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Tracking {:?} at {:?}: {}",
            self.phase, self.step, self.error
        )
    }
}
impl std::error::Error for TrackingWriteFailure {}

/// Preserve source order and actual ACKs; stop on the first failure/cancellation.
/// The original asymmetric engine fire-and-forgets its Promise. This executor
/// retains it to report completion/failure without claiming an observed setting.
pub fn apply(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    setting: TrackingSetting,
    mut next_transaction: impl FnMut() -> Result<u8>,
    validate: impl Fn() -> Result<()>,
) -> std::result::Result<TrackingWriteResult, TrackingWriteFailure> {
    let failure =
        |phase, step, acknowledgements, command_may_have_reached_device, error: anyhow::Error| {
            TrackingWriteFailure {
                requested: setting,
                phase,
                step,
                acknowledgements,
                command_may_have_reached_device,
                error: format!("{error:#}"),
            }
        };
    let commands = (|| {
        let product = capability(cap.product_id)
            .ok_or_else(|| anyhow::anyhow!("Product has no audited Orca Plus tracking branch"))?;
        product.validate_transport(cap)?;
        prepare(product, setting)
    })()
    .map_err(|e| failure(TrackingFailurePhase::Prepare, None, vec![], false, e))?;
    let mut acknowledgements = vec![];
    for command in commands {
        validate().map_err(|e| {
            failure(
                TrackingFailurePhase::BeforeCommand,
                Some(command.step),
                acknowledgements.clone(),
                false,
                e,
            )
        })?;
        let transaction = next_transaction().map_err(|e| {
            failure(
                TrackingFailurePhase::BeforeCommand,
                Some(command.step),
                acknowledgements.clone(),
                false,
                e,
            )
        })?;
        let payload = feature_exchange::exchange(
            device,
            cap,
            command.header,
            &command.payload,
            transaction,
            &validate,
        )
        .map_err(|e| {
            failure(
                TrackingFailurePhase::Command,
                Some(command.step),
                acknowledgements.clone(),
                true,
                e,
            )
        })?;
        acknowledgements.push(TrackingAcknowledgement {
            step: command.step,
            payload,
        });
        validate().map_err(|e| {
            failure(
                TrackingFailurePhase::AfterAcknowledgement,
                Some(command.step),
                acknowledgements.clone(),
                false,
                e,
            )
        })?;
    }
    Ok(TrackingWriteResult {
        requested: setting,
        acknowledgements,
        source_chain_acknowledged: true,
        verified: false,
    })
}

#[cfg(test)]
#[path = "mouse_tracking_tests.rs"]
mod tests;
