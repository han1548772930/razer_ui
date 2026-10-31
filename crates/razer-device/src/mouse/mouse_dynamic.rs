//! Current dynamic-sensitivity state, mode and custom float32 curve tasks.
//! Device observations contain no local editor template or control points.
use crate::{
    backend::FeatureTransport,
    device_reads::DeviceReadCapability,
    mouse_current_dpi::{self, Capability},
    mouse_sensor::{
        self as sensor, AccelerationMode, SensorChange, SensorObservation, SensorQuery,
        SensorWriteReport,
    },
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const PRODUCTS: &[u32] = &[190, 192, 196, 222, 226, 229, 239];
pub const CURVE_POINTS: usize = 250;
#[path = "mouse_dynamic_input.rs"]
pub mod input;

pub fn capability(product_id: u32) -> Option<&'static Capability> {
    PRODUCTS
        .contains(&product_id)
        .then(|| mouse_current_dpi::capability(product_id))
        .flatten()
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[non_exhaustive]
pub struct Setting {
    pub enabled: bool,
    pub mode: AccelerationMode,
    pub custom_sensor_accelerations: Vec<f32>,
}
impl Setting {
    pub fn new(enabled: bool, mode: AccelerationMode, points: Vec<f32>) -> anyhow::Result<Self> {
        let setting = Self {
            enabled,
            mode,
            custom_sensor_accelerations: points,
        };
        setting.validate()?;
        Ok(setting)
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.custom_sensor_accelerations.len() <= CURVE_POINTS
                && self
                    .custom_sensor_accelerations
                    .iter()
                    .all(|value| value.is_finite()),
            "Dynamic sensitivity contains an invalid float32 curve"
        );
        ensure!(
            !self.enabled
                || self.mode != AccelerationMode::Custom
                || self.custom_sensor_accelerations.len() == CURVE_POINTS,
            "Custom sensitivity requires all 250 float32 points"
        );
        Ok(())
    }
    fn sensor_change(&self) -> SensorChange {
        SensorChange::DynamicSensitivity {
            enabled: self.enabled,
            mode: self.mode,
            custom_sensor_accelerations: self.custom_sensor_accelerations.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[non_exhaustive]
pub struct Reading {
    pub raw_state: u8,
    pub raw_mode: u8,
    pub custom_sensor_accelerations: Option<Vec<f32>>,
}
impl Reading {
    pub fn enabled(&self) -> Option<bool> {
        match self.raw_state {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }
    pub fn mode(&self) -> Option<AccelerationMode> {
        mode(self.raw_mode)
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.custom_sensor_accelerations
                .as_ref()
                .is_none_or(|points| self.raw_mode == AccelerationMode::Custom as u8
                    && points.len() == CURVE_POINTS
                    && points.iter().all(|point| point.is_finite())),
            "Dynamic device observation has an invalid curve"
        );
        ensure!(
            self.raw_mode != AccelerationMode::Custom as u8
                || self.custom_sensor_accelerations.is_some(),
            "Custom device observation omits its curve"
        );
        Ok(())
    }
    pub fn confirms(&self, setting: &Setting) -> bool {
        self.enabled() == Some(setting.enabled)
            && (!setting.enabled
                || self.mode() == Some(setting.mode)
                    && (setting.mode != AccelerationMode::Custom
                        || self
                            .custom_sensor_accelerations
                            .as_ref()
                            .is_some_and(|points| {
                                points.iter().map(|point| point.to_bits()).eq(setting
                                    .custom_sensor_accelerations
                                    .iter()
                                    .map(|point| point.to_bits()))
                            })))
    }
}
pub fn mode(raw: u8) -> Option<AccelerationMode> {
    Some(match raw {
        0 => AccelerationMode::Classic,
        1 => AccelerationMode::Natural,
        2 => AccelerationMode::Jump,
        3 => AccelerationMode::Custom,
        _ => return None,
    })
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[non_exhaustive]
pub struct Report {
    pub requested: Option<Setting>,
    pub writes: Option<SensorWriteReport>,
    pub reading: Option<Reading>,
    pub write_failure: Option<String>,
    pub read_failure: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WriteFailure {
    pub progress: Report,
}

impl std::fmt::Display for WriteFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Mouse dynamic sensitivity task did not complete")
    }
}

impl Report {
    pub fn source_acknowledged(&self) -> bool {
        self.requested.as_ref().is_some_and(|setting| {
            self.write_failure.is_none()
                && self.writes.as_ref().is_some_and(|writes| {
                    sensor::validate_setter_report(&setting.sensor_change(), writes, false).is_ok()
                })
        })
    }
    pub fn confirmed(&self) -> bool {
        self.write_failure.is_none()
            && self.read_failure.is_none()
            && self.reading.as_ref().is_some_and(|reading| {
                self.requested
                    .as_ref()
                    .is_none_or(|setting| reading.confirms(setting))
            })
    }
    pub fn validate_reply(&self, requested: Option<&Setting>) -> anyhow::Result<()> {
        ensure!(
            self.requested.as_ref() == requested,
            "Dynamic reply belongs to another requested setting"
        );
        if let Some(reading) = &self.reading {
            reading.validate()?;
        }
        ensure!(
            !(self.reading.is_some() && self.read_failure.is_some())
                && (requested.is_some() || self.reading.is_some() || self.read_failure.is_some()),
            "Dynamic reply omits or contradicts its real device observation"
        );
        match requested {
            None => ensure!(
                self.writes.is_none() && self.write_failure.is_none(),
                "Dynamic read reply contains an unrelated mutation"
            ),
            Some(setting) => {
                setting.validate()?;
                let writes = self
                    .writes
                    .as_ref()
                    .context("Dynamic write reply omits delivery status")?;
                sensor::validate_setter_report(
                    &setting.sensor_change(),
                    writes,
                    self.write_failure.is_some(),
                )?;
            }
        }
        Ok(())
    }
}

fn read_current(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    next: &mut dyn FnMut() -> anyhow::Result<u8>,
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Reading> {
    let SensorObservation::AccelerationState { raw_state } = sensor::read(
        device,
        cap,
        SensorQuery::AccelerationState,
        next()?,
        validate,
    )?
    else {
        anyhow::bail!("Dynamic state response kind differs")
    };
    let SensorObservation::AccelerationMode { raw_mode } = sensor::read(
        device,
        cap,
        SensorQuery::AccelerationMode,
        next()?,
        validate,
    )?
    else {
        anyhow::bail!("Dynamic mode response kind differs")
    };
    let custom_sensor_accelerations = if raw_mode == AccelerationMode::Custom as u8 {
        let mut points = Vec::with_capacity(CURVE_POINTS);
        for offset in (0..CURVE_POINTS).step_by(8) {
            let count = (CURVE_POINTS - offset).min(8);
            let SensorObservation::CurveChunk { points: chunk, .. } = sensor::read(
                device,
                cap,
                SensorQuery::CurveChunk {
                    point_offset: offset as u8,
                    point_count: count as u8,
                },
                next()?,
                validate,
            )?
            else {
                anyhow::bail!("Dynamic curve response kind differs")
            };
            points.extend(chunk);
        }
        Some(points)
    } else {
        None
    };
    let reading = Reading {
        raw_state,
        raw_mode,
        custom_sensor_accelerations,
    };
    reading.validate()?;
    Ok(reading)
}

/// The mounted source ends a setter task after its actual acknowledgements.
/// A read is an explicit independent observation; it cannot invent editor data.
pub fn execute(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    setting: Option<&Setting>,
    next: &mut dyn FnMut() -> anyhow::Result<u8>,
    validate: &dyn Fn() -> anyhow::Result<()>,
    wait: &mut dyn FnMut(Duration) -> anyhow::Result<()>,
) -> anyhow::Result<Report> {
    let validate = &|| {
        validate().map_err(|error| {
            crate::backend::interrupt_exchange(
                error,
                crate::backend::ExchangeInterruption::Validation,
            )
        })
    };
    let wait = &mut |duration| {
        wait(duration).map_err(|error| {
            crate::backend::interrupt_exchange(error, crate::backend::ExchangeInterruption::Wait)
        })
    };
    ensure!(
        capability(cap.product_id).is_some(),
        "Dynamic product has no current factory audit"
    );
    if let Some(setting) = setting {
        setting.validate()?;
    }
    validate()?;
    let mut report = Report {
        requested: setting.cloned(),
        writes: None,
        reading: None,
        write_failure: None,
        read_failure: None,
    };
    if let Some(setting) = setting {
        match sensor::apply(
            device,
            cap,
            &setting.sensor_change(),
            &mut *next,
            validate,
            wait,
        ) {
            Ok(writes) => report.writes = Some(writes),
            Err(failure) => {
                report.write_failure = Some(failure.to_string());
                report.writes = Some(failure.report);
                if crate::backend::exchange_interrupted(&failure.error) {
                    let message = failure.error.to_string();
                    return Err(failure
                        .error
                        .context(WriteFailure { progress: report })
                        .context(message));
                }
            }
        }
    }
    if setting.is_none() {
        match read_current(device, cap, next, validate) {
            Ok(reading) => report.reading = Some(reading),
            Err(error) => report.read_failure = Some(format!("{error:#}")),
        }
    }
    report.validate_reply(setting)?;
    Ok(report)
}

/// Optional transport diagnosis, separate from the mounted source task and its
/// UI completion. Getter results never supply local template/control points.
pub fn diagnose(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    setting: Option<&Setting>,
    next: &mut dyn FnMut() -> anyhow::Result<u8>,
    validate: &dyn Fn() -> anyhow::Result<()>,
    wait: &mut dyn FnMut(Duration) -> anyhow::Result<()>,
) -> anyhow::Result<Report> {
    let mut report = execute(device, cap, setting, next, validate, wait)?;
    if setting.is_some() {
        match read_current(device, cap, next, validate) {
            Ok(reading) => report.reading = Some(reading),
            Err(error) => report.read_failure = Some(format!("{error:#}")),
        }
    }
    report.validate_reply(setting)?;
    Ok(report)
}

#[cfg(test)]
#[path = "mouse_dynamic_tests.rs"]
mod tests;
