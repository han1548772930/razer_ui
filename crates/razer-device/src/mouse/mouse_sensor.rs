//! Current direct sensor rotation and dynamic sensitivity commands.
//! Source setters resolve after their response; optional independent readback
//! is explicit below. This module neither chooses a HID node nor saves a profile.
use crate::{backend::FeatureTransport, device_reads::DeviceReadCapability, feature_exchange};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use std::{fmt, time::Duration};

pub const AUDITED_PRODUCTS: &[u32] = &[190, 192, 196, 222, 226, 229, 239];
const PROFILE: u8 = 1;
const POINTS: usize = 250;
const CHUNK: usize = 8;
const GET_STATE: [u8; 3] = [2, 11, 144];
const SET_STATE: [u8; 3] = [2, 11, 16];
const GET_MODE: [u8; 3] = [2, 11, 145];
const SET_MODE: [u8; 3] = [2, 11, 17];
const GET_CURVE_SETTING: [u8; 3] = [3, 11, 146];
const GET_CURVE: [u8; 3] = [37, 11, 147];
const SET_CURVE: [u8; 3] = [37, 11, 19];
const GET_ROTATION: [u8; 3] = [3, 11, 148];
const SET_ROTATION: [u8; 3] = [3, 11, 20];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[repr(u8)]
pub enum AccelerationMode {
    Classic = 0,
    Natural = 1,
    Jump = 2,
    Custom = 3,
}

/// The source task can select a separate mapping-engine fallback according to
/// feature configuration and firmware. That route is not a Feature command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum RotationRoute {
    DirectFeature,
    NativeMappingEngine,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum SensorChange {
    Rotation {
        enabled: bool,
        angle_degrees: i8,
        route: RotationRoute,
    },
    DynamicSensitivity {
        enabled: bool,
        mode: AccelerationMode,
        custom_sensor_accelerations: Vec<f32>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SensorQuery {
    AccelerationState,
    AccelerationMode,
    CurveSetting { mode: AccelerationMode },
    CurveChunk { point_offset: u8, point_count: u8 },
    Rotation,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum SensorObservation {
    AccelerationState {
        raw_state: u8,
    },
    AccelerationMode {
        raw_mode: u8,
    },
    CurveSetting {
        raw_mode: u8,
        input_offset: u8,
    },
    CurveChunk {
        point_offset: u8,
        points: Vec<f32>,
        /// Preserve actual little-endian floats and any returned tail bytes.
        raw_point_data: Vec<u8>,
    },
    Rotation {
        raw_state: u8,
        raw_angle: u8,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct SensorAcknowledgement {
    pub command: [u8; 3],
    pub payload: Vec<u8>,
    /// Empty SUCCESS data is a valid ACK, never an invented observation.
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct SensorConfirmation {
    pub acknowledgement_index: usize,
    pub observation: SensorObservation,
    pub matches_request: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct SensorWriteReport {
    pub attempted_writes: usize,
    pub acknowledgements: Vec<SensorAcknowledgement>,
    pub confirmations: Vec<SensorConfirmation>,
}

impl SensorWriteReport {
    pub fn verified(&self) -> bool {
        self.attempted_writes > 0
            && self.attempted_writes == self.acknowledgements.len()
            && self.acknowledgements.len() == self.confirmations.len()
            && self.confirmations.iter().all(|row| row.matches_request)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SensorFailurePhase {
    Prepare,
    Write,
    Wait,
    Readback,
}

#[derive(Debug)]
pub struct SensorWriteFailure {
    pub phase: SensorFailurePhase,
    pub report: SensorWriteReport,
    pub error: anyhow::Error,
}

impl SensorWriteFailure {
    pub fn device_state_may_have_changed(&self) -> bool {
        self.report.attempted_writes > 0
    }
}

impl fmt::Display for SensorWriteFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Sensor {:?}: {:#}", self.phase, self.error)
    }
}

impl std::error::Error for SensorWriteFailure {}

fn validate_cap(cap: &DeviceReadCapability) -> anyhow::Result<()> {
    ensure!(
        AUDITED_PRODUCTS.contains(&cap.product_id)
            && matches!(
                cap.source_class.as_str(),
                "rzDevice25" | "rzDevice25DualLinkMouse"
            )
            && cap.transaction_prefix == 0
            && cap.transaction_modulus == 31,
        "Sensor product or direct transport branch is not current-source audited"
    );
    Ok(())
}

fn query_request(query: SensorQuery) -> anyhow::Result<([u8; 3], Vec<u8>)> {
    Ok(match query {
        SensorQuery::AccelerationState => (GET_STATE, vec![PROFILE]),
        SensorQuery::AccelerationMode => (GET_MODE, vec![PROFILE]),
        SensorQuery::CurveSetting { mode } => (GET_CURVE_SETTING, vec![PROFILE, mode as u8]),
        SensorQuery::Rotation => (GET_ROTATION, vec![PROFILE]),
        SensorQuery::CurveChunk {
            point_offset,
            point_count,
        } => {
            ensure!(
                (1..=CHUNK as u8).contains(&point_count)
                    && usize::from(point_offset) + usize::from(point_count) <= POINTS,
                "Custom sensor chunk is outside the 250-point current task"
            );
            (
                GET_CURVE,
                vec![
                    PROFILE,
                    AccelerationMode::Custom as u8,
                    POINTS as u8,
                    point_offset,
                    point_count,
                ],
            )
        }
    })
}

fn parse(query: SensorQuery, data: &[u8]) -> anyhow::Result<SensorObservation> {
    ensure!(
        data.first() == Some(&PROFILE),
        "Sensor response profile is missing or differs"
    );
    Ok(match query {
        SensorQuery::AccelerationState => {
            ensure!(data.len() >= 2, "Sensor state response is truncated");
            SensorObservation::AccelerationState { raw_state: data[1] }
        }
        SensorQuery::AccelerationMode => {
            ensure!(data.len() >= 2, "Sensor mode response is truncated");
            SensorObservation::AccelerationMode { raw_mode: data[1] }
        }
        SensorQuery::CurveSetting { mode } => {
            ensure!(
                data.len() >= 3 && data[1] == mode as u8,
                "Curve setting response is truncated or has a different mode"
            );
            SensorObservation::CurveSetting {
                raw_mode: data[1],
                input_offset: data[2],
            }
        }
        SensorQuery::Rotation => {
            ensure!(data.len() >= 3, "Rotation response is truncated");
            SensorObservation::Rotation {
                raw_state: data[1],
                raw_angle: data[2],
            }
        }
        SensorQuery::CurveChunk {
            point_offset,
            point_count,
        } => {
            let size = usize::from(point_count) * 4;
            ensure!(
                data.len() >= 5 + size
                    && data[1..5]
                        == [
                            AccelerationMode::Custom as u8,
                            POINTS as u8,
                            point_offset,
                            point_count
                        ],
                "Custom curve response is truncated or has different chunk metadata"
            );
            let points = data[5..5 + size]
                .chunks_exact(4)
                .map(|bytes| f32::from_le_bytes(bytes.try_into().expect("four-byte chunk")))
                .collect();
            SensorObservation::CurveChunk {
                point_offset,
                points,
                raw_point_data: data[5..].to_vec(),
            }
        }
    })
}

pub fn read(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    query: SensorQuery,
    transaction: u8,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<SensorObservation> {
    validate_cap(cap)?;
    ensure!(
        query != SensorQuery::Rotation || cap.product_id == 226,
        "Rotation protocol has no current product audit"
    );
    let (command, payload) = query_request(query)?;
    let data = feature_exchange::exchange(device, cap, command, &payload, transaction, &validate)?;
    parse(query, &data)
}

struct WriteRequest {
    command: [u8; 3],
    payload: Vec<u8>,
    query: SensorQuery,
    wait_after: Duration,
}

fn prepare(change: &SensorChange) -> anyhow::Result<Vec<WriteRequest>> {
    let step = |command, payload, query| WriteRequest {
        command,
        payload,
        query,
        wait_after: Duration::ZERO,
    };
    match change {
        SensorChange::Rotation {
            enabled,
            angle_degrees,
            route,
        } => {
            ensure!(
                *route == RotationRoute::DirectFeature,
                "Native mapping rotation requires its separately audited OS adapter"
            );
            ensure!(
                (-44..=44).contains(angle_degrees),
                "Rotation is outside the current editor -44..44 degree range"
            );
            Ok(vec![step(
                SET_ROTATION,
                vec![PROFILE, u8::from(*enabled), *angle_degrees as u8],
                SensorQuery::Rotation,
            )])
        }
        SensorChange::DynamicSensitivity {
            enabled,
            mode,
            custom_sensor_accelerations,
        } => {
            // The source silently skips malformed Custom arrays. Reject before
            // changing its state rather than claim a complete custom upload.
            if *enabled && *mode == AccelerationMode::Custom {
                ensure!(
                    custom_sensor_accelerations.len() == POINTS,
                    "Custom sensitivity requires exactly 250 float32 values"
                );
                ensure!(
                    custom_sensor_accelerations
                        .iter()
                        .all(|value| value.is_finite()),
                    "Custom sensitivity contains a nonfinite value"
                );
            }
            let mut requests = vec![step(
                SET_STATE,
                vec![PROFILE, u8::from(*enabled)],
                SensorQuery::AccelerationState,
            )];
            if !enabled {
                return Ok(requests);
            }
            requests.push(step(
                SET_MODE,
                vec![PROFILE, *mode as u8],
                SensorQuery::AccelerationMode,
            ));
            if *mode == AccelerationMode::Custom {
                for (index, points) in custom_sensor_accelerations.chunks(CHUNK).enumerate() {
                    let offset = (index * CHUNK) as u8;
                    let count = points.len() as u8;
                    let mut payload = vec![PROFILE, *mode as u8, POINTS as u8, offset, count];
                    for value in points {
                        payload.extend_from_slice(&value.to_le_bytes());
                    }
                    let mut request = step(
                        SET_CURVE,
                        payload,
                        SensorQuery::CurveChunk {
                            point_offset: offset,
                            point_count: count,
                        },
                    );
                    // Current runner waits 15 ms only after a full 32-byte chunk.
                    if points.len() == CHUNK {
                        request.wait_after = Duration::from_millis(15);
                    }
                    requests.push(request);
                }
            }
            Ok(requests)
        }
    }
}

fn matches_request(request: &WriteRequest, observation: &SensorObservation) -> bool {
    match observation {
        SensorObservation::AccelerationState { raw_state } => request.payload[1] == *raw_state,
        SensorObservation::AccelerationMode { raw_mode } => request.payload[1] == *raw_mode,
        SensorObservation::Rotation {
            raw_state,
            raw_angle,
        } => request.payload[1..3] == [*raw_state, *raw_angle],
        SensorObservation::CurveChunk {
            points,
            raw_point_data,
            ..
        } => {
            let requested = &request.payload[5..];
            points.len() * 4 == requested.len()
                && raw_point_data.get(..requested.len()) == Some(requested)
        }
        SensorObservation::CurveSetting { .. } => false,
    }
}

pub(crate) fn validate_setter_report(
    change: &SensorChange,
    report: &SensorWriteReport,
    failed: bool,
) -> anyhow::Result<()> {
    let requests = prepare(change)?;
    ensure!(
        report.confirmations.is_empty()
            && report.attempted_writes <= requests.len()
            && report.acknowledgements.len() <= report.attempted_writes
            && report.attempted_writes <= report.acknowledgements.len() + 1,
        "Sensor setter report contains impossible delivery status"
    );
    for (ack, expected) in report.acknowledgements.iter().zip(&requests) {
        ensure!(
            ack.command == expected.command
                && ack.payload == expected.payload
                && ack.data.len() <= 80,
            "Sensor setter acknowledgement belongs to another command or payload"
        );
    }
    ensure!(
        failed
            || report.attempted_writes == requests.len()
                && report.acknowledgements.len() == requests.len(),
        "Sensor setter report omits successful source tasks"
    );
    Ok(())
}

/// Keep the source mutation order. Independent confirmation is an explicit
/// optional extension: no request value or setter echo substitutes for a GET.
/// `wait` is supplied by the caller so tests simulate the source chunk delays.
#[allow(clippy::too_many_arguments)]
pub fn apply(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    change: &SensorChange,
    confirm_with_readback: bool,
    mut next_transaction: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
    mut wait: impl FnMut(Duration) -> anyhow::Result<()>,
) -> Result<SensorWriteReport, SensorWriteFailure> {
    let mut report = SensorWriteReport::default();
    let fail = |phase, report, error| SensorWriteFailure {
        phase,
        report,
        error,
    };
    let requests = validate_cap(cap)
        .and_then(|()| {
            ensure!(
                !matches!(change, SensorChange::Rotation { .. }) || cap.product_id == 226,
                "Rotation protocol has no current product audit"
            );
            Ok(())
        })
        .and_then(|()| prepare(change))
        .map_err(|error| fail(SensorFailurePhase::Prepare, report.clone(), error))?;
    for request in &requests {
        let transaction = validate()
            .and_then(|()| next_transaction())
            .map_err(|error| fail(SensorFailurePhase::Write, report.clone(), error))?;
        // Packet/namespace checks occur before any send, and are not counted as
        // a hardware mutation. The counter remains conservative once I/O starts.
        ensure_transaction(cap, transaction)
            .map_err(|error| fail(SensorFailurePhase::Prepare, report.clone(), error))?;
        report.attempted_writes += 1;
        let data = feature_exchange::exchange(
            device,
            cap,
            request.command,
            &request.payload,
            transaction,
            &validate,
        )
        .map_err(|error| fail(SensorFailurePhase::Write, report.clone(), error))?;
        report.acknowledgements.push(SensorAcknowledgement {
            command: request.command,
            payload: request.payload.clone(),
            data,
        });
        if !request.wait_after.is_zero() {
            validate()
                .and_then(|()| wait(request.wait_after))
                .and_then(|()| validate())
                .map_err(|error| fail(SensorFailurePhase::Wait, report.clone(), error))?;
        }
    }
    validate().map_err(|error| fail(SensorFailurePhase::Write, report.clone(), error))?;
    if confirm_with_readback {
        for (index, request) in requests.iter().enumerate() {
            let observation = (|| {
                validate()?;
                read(device, cap, request.query, next_transaction()?, &validate)
            })()
            .map_err(|error| fail(SensorFailurePhase::Readback, report.clone(), error))?;
            let matches = matches_request(request, &observation);
            report.confirmations.push(SensorConfirmation {
                acknowledgement_index: index,
                observation,
                matches_request: matches,
            });
            if !matches {
                return Err(fail(
                    SensorFailurePhase::Readback,
                    report,
                    anyhow::anyhow!(
                        "Sensor readback differs; acknowledged writes were not rolled back"
                    ),
                ));
            }
        }
        validate()
            .context("Sensor readback session is no longer valid")
            .map_err(|error| fail(SensorFailurePhase::Readback, report.clone(), error))?;
    }
    Ok(report)
}

fn ensure_transaction(cap: &DeviceReadCapability, transaction: u8) -> anyhow::Result<()> {
    ensure!(
        transaction < cap.transaction_modulus,
        "Sensor transaction is outside the current direct namespace"
    );
    ensure!(
        cap.report_bytes == 91
            && (cap.report_id == 0
                || cap.source_class == "rzDevice25DualLinkMouse"
                    && cap.report_id == cap.claim_interface)
            && cap.max_retry_in > 0
            && cap.max_retry_out > 0,
        "Sensor packet capability differs from current source"
    );
    Ok(())
}

#[cfg(test)]
#[path = "mouse_sensor_tests.rs"]
mod tests;
