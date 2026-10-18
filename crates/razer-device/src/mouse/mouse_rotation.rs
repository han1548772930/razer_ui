//! Profile-one sensor rotation. A setter ACK and an independent GET are
//! distinct results; native driver routing belongs to the source task owner.
pub use crate::mouse_sensor::RotationRoute;
use crate::{
    backend::FeatureTransport,
    device_reads::DeviceReadCapability,
    mouse_current_dpi::{self, Capability},
    mouse_sensor::{
        self as sensor, SensorChange, SensorObservation, SensorQuery, SensorWriteReport,
    },
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};

#[path = "mouse_rotation_ble.rs"]
pub mod ble;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportKind {
    Feature,
    Ble,
}

pub const PRODUCTS: &[u32] = &[190, 192, 196, 222, 226, 229, 239];

/// PID196 configures `fwVersionForRotation: "1.10.1.0"`. Its source comparator
/// coerces that four-part string to a new three-part SemVer object. Equal
/// coerced triples pass; the source's object-identity fourth-part branch is
/// never taken for two independently coerced strings.
pub fn route(product_id: u32, observed_firmware: Option<&str>) -> anyhow::Result<RotationRoute> {
    ensure!(
        PRODUCTS.contains(&product_id),
        "Rotation product has no mounted source task"
    );
    if product_id != 196 {
        return Ok(RotationRoute::DirectFeature);
    }
    let current = observed_firmware.context("Rotation firmware has not been observed")?;
    Ok(
        if coerce_version(current).is_some_and(|version| version >= [1, 10, 1]) {
            RotationRoute::DirectFeature
        } else {
            RotationRoute::NativeMappingEngine
        },
    )
}

fn coerce_version(input: &str) -> Option<[u64; 3]> {
    static COERCE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    // JS \d is ASCII. Keep the source's first-match and optional-group
    // backtracking behavior, including rejection of the first invalid triple.
    let expression = COERCE.get_or_init(|| {
        regex::Regex::new(
            r"(^|[^0-9])([0-9]{1,16})(?:\.([0-9]{1,16}))?(?:\.([0-9]{1,16}))?(?:$|[^0-9])",
        )
        .expect("source SemVer coercion pattern")
    });
    let captures = expression.captures(input)?;
    let mut version = [0; 3];
    for (index, value) in version.iter_mut().enumerate() {
        let segment = captures.get(index + 2).map_or("0", |part| part.as_str());
        if segment.len() > 1 && segment.starts_with('0') {
            return None;
        }
        let number: u64 = segment.parse().ok()?;
        if number > 9_007_199_254_740_991 {
            return None;
        }
        *value = number;
    }
    Some(version)
}

pub fn capability(product_id: u32) -> Option<&'static Capability> {
    PRODUCTS
        .contains(&product_id)
        .then(|| mouse_current_dpi::capability(product_id))
        .flatten()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Setting {
    pub enabled: bool,
    pub angle_degrees: i8,
}
impl Setting {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            (-44..=44).contains(&self.angle_degrees),
            "Rotation angle is outside the current editor range"
        );
        Ok(())
    }
    fn change(&self) -> SensorChange {
        SensorChange::Rotation {
            enabled: self.enabled,
            angle_degrees: self.angle_degrees,
            route: RotationRoute::DirectFeature,
        }
    }
    pub fn native_request(
        &self,
    ) -> anyhow::Result<crate::mapping_native_rotation::RotationRequest> {
        self.validate()?;
        Ok(crate::mapping_native_rotation::RotationRequest::SetAngle(
            crate::mapping_native_rotation::native_angle_for_task(
                self.enabled,
                self.angle_degrees,
            )?,
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reading {
    pub raw_state: u8,
    pub raw_angle: u8,
}
impl Reading {
    pub fn enabled(&self) -> Option<bool> {
        match self.raw_state {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }
    pub fn angle_degrees(&self) -> Option<i8> {
        let angle = self.raw_angle as i8;
        (-44..=44).contains(&angle).then_some(angle)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum Report {
    Read {
        reading: Reading,
    },
    Write {
        requested: Setting,
        delivery: SensorWriteReport,
        failure: Option<String>,
    },
    BleWrite {
        requested: Setting,
        delivery: ble::WriteReport,
        failure: Option<String>,
    },
}
impl Report {
    pub fn validate_reply(&self, expected: Option<&Setting>) -> anyhow::Result<()> {
        match (self, expected) {
            (Self::Read { .. }, None) => Ok(()),
            (
                Self::Write {
                    requested,
                    delivery,
                    failure,
                },
                Some(expected),
            ) => {
                expected.validate()?;
                ensure!(
                    requested == expected,
                    "Rotation reply belongs to another setting"
                );
                sensor::validate_setter_report(&requested.change(), delivery, failure.is_some())
            }
            (
                Self::BleWrite {
                    requested,
                    delivery,
                    failure,
                },
                Some(expected),
            ) => {
                expected.validate()?;
                ensure!(
                    requested == expected,
                    "BLE Rotation reply belongs to another setting"
                );
                delivery.validate(requested, failure.is_some())
            }
            _ => anyhow::bail!("Rotation reply belongs to another operation"),
        }
    }
    pub fn source_acknowledged(&self) -> bool {
        matches!(self, Self::Write { requested, failure: None, .. } | Self::BleWrite { requested, failure: None, .. } if self.validate_reply(Some(requested)).is_ok())
    }
    pub fn failure_reason(&self) -> Option<&str> {
        match self {
            Self::Write { failure, .. } | Self::BleWrite { failure, .. } => failure.as_deref(),
            Self::Read { .. } => None,
        }
    }
    pub fn validate_transport(&self, expected: TransportKind) -> anyhow::Result<()> {
        ensure!(
            matches!(
                (self, expected),
                (Self::Read { .. }, _)
                    | (Self::Write { .. }, TransportKind::Feature)
                    | (Self::BleWrite { .. }, TransportKind::Ble)
            ),
            "Rotation reply belongs to another transport"
        );
        Ok(())
    }
}

pub fn execute(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    setting: Option<&Setting>,
    next: &mut dyn FnMut() -> anyhow::Result<u8>,
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Report> {
    capability(cap.product_id).context("Rotation current factory is absent")?;
    validate()?;
    let report = if let Some(setting) = setting {
        setting.validate()?;
        let (delivery, failure) =
            match sensor::apply(device, cap, &setting.change(), next, validate, |_| {
                anyhow::bail!("Rotation setter has no source inter-command delay")
            }) {
                Ok(report) => (report, None),
                Err(failure) => {
                    let error = failure.to_string();
                    (failure.report, Some(error))
                }
            };
        Report::Write {
            requested: *setting,
            delivery,
            failure,
        }
    } else {
        let SensorObservation::Rotation {
            raw_state,
            raw_angle,
        } = sensor::read(device, cap, SensorQuery::Rotation, next()?, validate)?
        else {
            anyhow::bail!("Rotation getter returned another observation kind");
        };
        Report::Read {
            reading: Reading {
                raw_state,
                raw_angle,
            },
        }
    };
    report.validate_reply(setting)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rotation_firmware_route_keeps_the_actual_coercion_and_product_branches() {
        for (firmware, expected) in [
            ("1.10.1.0", RotationRoute::DirectFeature),
            ("1.10.1.999", RotationRoute::DirectFeature),
            ("1.10.1", RotationRoute::DirectFeature),
            ("1.10.0.999", RotationRoute::NativeMappingEngine),
            ("1.9.99.255", RotationRoute::NativeMappingEngine),
            ("2.0.0.0", RotationRoute::DirectFeature),
            ("v1.10.1-beta", RotationRoute::DirectFeature),
            ("-1.10.1", RotationRoute::DirectFeature),
            ("01.10.1 2.0.0", RotationRoute::NativeMappingEngine),
            ("firmware unknown", RotationRoute::NativeMappingEngine),
            ("9007199254740992.0.0", RotationRoute::NativeMappingEngine),
        ] {
            assert_eq!(route(196, Some(firmware)).unwrap(), expected, "{firmware}");
        }
        assert!(route(196, None).is_err());
        assert!(route(70, Some("2.0.0")).is_err());
        for product in [190, 192, 222, 226, 229, 239] {
            assert_eq!(route(product, None).unwrap(), RotationRoute::DirectFeature);
        }
        assert_eq!(coerce_version("12345678901234567.2.3"), Some([2, 3, 0]));
        assert_eq!(coerce_version("2.12345678901234567.3"), Some([2, 0, 0]));
        assert_eq!(coerce_version("2.01.0"), None);
    }
}
