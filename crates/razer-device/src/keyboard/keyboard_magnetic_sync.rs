//! Current740/746 MW24592 ep/Xd and51765.Je. These are packet inputs;
//! matching onboard slots are selected from actual device metadata at execution.
use super::source;
use crate::keyboard_analog::{ActuationPoint, RapidTriggerKey};
use anyhow::{Context as _, ensure};
use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
enum InputKind {
    AnalogInput,
    KeyInput,
    DKMInput,
    MouseInput,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
enum OutputKind {
    #[serde(rename = "defaultGroup")]
    Default,
    #[serde(rename = "controllerGroup")]
    Controller,
    #[serde(rename = "joystickGroup")]
    Joystick,
    #[serde(rename = "disableGroup")]
    Disabled,
    #[serde(other)]
    Other,
}

#[derive(Clone, Copy, Debug)]
struct Point {
    make: u16,
    release: u16,
}
impl<'de> Deserialize<'de> for Point {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Array([f64; 2]),
            Object {
                #[serde(rename = "0")]
                make: f64,
                #[serde(rename = "1")]
                release: f64,
            },
        }
        let (make, release) = match Raw::deserialize(deserializer)? {
            Raw::Array([make, release]) | Raw::Object { make, release } => (make, release),
        };
        let checked = |value: f64| {
            if value.is_finite() && value.fract() == 0. && (0. ..=65535.).contains(&value) {
                Ok(value as u16)
            } else {
                Err(serde::de::Error::custom(
                    "Actuation threshold must be an integral u16",
                ))
            }
        };
        Ok(Self {
            make: checked(make)?,
            release: checked(release)?,
        })
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Rapid {
    make_sensitivity: f64,
    break_sensitivity: f64,
    #[serde(default)]
    is_enable_up_stroke: bool,
}
impl Rapid {
    fn sensitivities(&self) -> anyhow::Result<(u16, u16)> {
        let down = sensitivity(self.make_sensitivity)?;
        let up = if self.is_enable_up_stroke {
            sensitivity(self.break_sensitivity)?
        } else {
            down.saturating_sub(1638).max(1638)
        };
        Ok((down, up))
    }
}

fn sensitivity(value: f64) -> anyhow::Result<u16> {
    ensure!(value.is_finite(), "Rapid Trigger sensitivity is not finite");
    Ok((value * 1638.).floor().clamp(1638., 16380.) as u16)
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stage {
    output_type: OutputKind,
    actuation_point: Option<Point>,
    rapid_trigger: Option<Rapid>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Mapping {
    #[serde(rename = "inputID")]
    input_id: String,
    input_type: InputKind,
    #[serde(default)]
    is_hyper_shift: bool,
    mapping: Option<Vec<Stage>>,
    output_type: Option<OutputKind>,
}
impl Mapping {
    fn primary(&self) -> anyhow::Result<&Stage> {
        self.mapping
            .as_ref()
            .and_then(|stages| stages.first())
            .context("Analog primary stage is absent")
    }
    fn blocked(&self) -> bool {
        if let Some(stages) = &self.mapping {
            stages
                .get(1)
                .is_some_and(|stage| stage.output_type != OutputKind::Default)
                || stages.first().is_some_and(|stage| {
                    matches!(
                        stage.output_type,
                        OutputKind::Controller | OutputKind::Joystick | OutputKind::Disabled
                    )
                })
        } else {
            self.output_type.is_some_and(|kind| {
                matches!(
                    kind,
                    OutputKind::Controller | OutputKind::Joystick | OutputKind::Disabled
                )
            })
        }
    }
}

#[derive(Deserialize)]
struct Profile {
    guid: String,
    mappings: Vec<Mapping>,
}
fn profile(value: &Value) -> anyhow::Result<Profile> {
    let profile: Profile =
        serde_json::from_value(value.clone()).context("Invalid magnetic profile settings")?;
    ensure!(!profile.guid.is_empty(), "Magnetic profile GUID is absent");
    for row in &profile.mappings {
        ensure!(
            !row.input_id.is_empty(),
            "Magnetic mapping input identity is absent"
        );
        if row.input_type == InputKind::AnalogInput {
            row.primary()?;
        }
    }
    Ok(profile)
}

pub fn global_actuation(
    product: u32,
    layout: u32,
    settings: &Value,
    value: &Value,
) -> anyhow::Result<Vec<ActuationPoint>> {
    let profile = profile(settings)?;
    let point: Point =
        serde_json::from_value(value.clone()).context("Invalid global actuation thresholds")?;
    let _ = point.release;
    let global = ActuationPoint {
        key_id: 255,
        threshold_low: point.make,
        threshold_high: point.make.saturating_sub(1638).max(256),
    };
    let defaults = source::default_mappings(product, layout)?;
    let mut points = vec![global.clone()];
    for key in source::obm_keys(product, layout)? {
        let normal = profile.mappings.iter().find(|row| {
            row.input_type == InputKind::AnalogInput
                && !row.is_hyper_shift
                && row.input_id == key.input
        });
        let blocked = normal.is_some_and(Mapping::blocked)
            || defaults.iter().any(|row| {
                row.input == key.input
                    && matches!(row.output, source::DefaultOutput::Profile { .. })
            });
        let primary = profile
            .mappings
            .iter()
            .rfind(|row| row.input_type == InputKind::AnalogInput && row.input_id == key.input);
        let (low, high) = if blocked {
            let defaults = source::actuation(&profile.guid, Some(key.input));
            (defaults.low, defaults.high)
        } else if let Some(primary) = primary {
            let low = primary
                .primary()?
                .actuation_point
                .context("Mapped make actuation threshold is absent")?
                .make;
            (low, low.saturating_sub(1638).max(256))
        } else {
            (global.threshold_low, global.threshold_high)
        };
        points.push(ActuationPoint {
            key_id: key.id,
            threshold_low: low,
            threshold_high: high,
        });
    }
    Ok(points)
}

pub fn global_rapid_trigger(
    product: u32,
    layout: u32,
    settings: &Value,
    value: &Value,
) -> anyhow::Result<Vec<RapidTriggerKey>> {
    let profile = profile(settings)?;
    let value: Rapid =
        serde_json::from_value(value.clone()).context("Invalid global Rapid Trigger settings")?;
    let (down, up) = value.sensitivities()?;
    let mut keys = vec![RapidTriggerKey {
        key_id: 255,
        enabled: 1,
        down_sensitivity: down,
        up_sensitivity: up,
    }];
    let defaults = source::rapid(&profile.guid, None);
    for key in source::obm_keys(product, layout)? {
        let saved = profile
            .mappings
            .iter()
            .rev()
            .filter(|row| row.input_type == InputKind::AnalogInput && row.input_id == key.input)
            .find_map(|row| {
                row.mapping
                    .as_ref()
                    .and_then(|stages| stages.first())
                    .and_then(|stage| stage.rapid_trigger.as_ref())
            });
        let (enabled, down, up) = if let Some(saved) = saved {
            let (down, up) = saved.sensitivities()?;
            (1, down, up)
        } else {
            (u8::from(defaults.enabled), defaults.down, defaults.up)
        };
        keys.push(RapidTriggerKey {
            key_id: key.id,
            enabled,
            down_sensitivity: down,
            up_sensitivity: up,
        });
    }
    Ok(keys)
}

/// Source Yc collects removed normal analog keys first, followed by current
/// rows. Modified mappings occur in both passes; their reset precedes the new
/// thresholds exactly as the source array collector does.
pub fn per_key_changes(
    product: u32,
    settings: &Value,
    old: &Value,
) -> anyhow::Result<(Vec<ActuationPoint>, Vec<RapidTriggerKey>)> {
    let current = profile(settings)?;
    let mut removed = old.clone();
    removed["mappings"] = Value::Array(super::removed_mappings(settings, old)?);
    let old = profile(&removed)?;
    let mut points = Vec::new();
    let mut rapid_keys = Vec::new();
    for (rows, reset) in [(&old.mappings, true), (&current.mappings, false)] {
        for row in rows {
            if row.input_type != InputKind::AnalogInput || row.is_hyper_shift {
                continue;
            }
            let Ok(key) = source::firmware_key(product, &row.input_id) else {
                continue;
            };
            let primary = row.primary()?;
            let actuation = source::actuation(&current.guid, None);
            let rt = source::rapid(&current.guid, Some(&row.input_id));
            let point = if reset {
                Point {
                    make: actuation.low,
                    release: 256,
                }
            } else {
                primary
                    .actuation_point
                    .context("Analog mapping thresholds are absent")?
            };
            let requested = ActuationPoint {
                key_id: key,
                threshold_low: point.make,
                threshold_high: point.release,
            };
            points.push(requested);
            let (enabled, down, up) = if reset {
                (u8::from(rt.enabled), rt.down, rt.up)
            } else if let Some(rapid) = &primary.rapid_trigger {
                // Yc preserves the source's independent breakSensitivity; the
                // UI has already selected its continuous/upstroke semantics.
                (
                    1,
                    if rapid.make_sensitivity != 0. {
                        sensitivity(rapid.make_sensitivity)?
                    } else {
                        rt.down
                    },
                    if rapid.break_sensitivity != 0. {
                        sensitivity(rapid.break_sensitivity)?
                    } else {
                        rt.up
                    },
                )
            } else {
                (0, rt.down, rt.up)
            };
            let requested = RapidTriggerKey {
                key_id: key,
                enabled,
                down_sensitivity: down,
                up_sensitivity: up,
            };
            rapid_keys.push(requested);
        }
    }
    Ok((points, rapid_keys))
}
