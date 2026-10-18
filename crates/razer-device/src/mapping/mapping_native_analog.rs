//! Native analog mapping curves and contribution state.
//!
//! JSON is confined to the original mapping-engine input boundary. Curve
//! compilation and event evaluation use typed points, tables and device state.
//! The tables are native mapping data, not HID reports or a driver write ACK.

use anyhow::{Context as _, bail, ensure};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalogResolution {
    V1,
    V2,
}

impl AnalogResolution {
    fn bucket(self, actuation: u16) -> anyhow::Result<u8> {
        match self {
            Self::V1 => Ok(u8::try_from(actuation).context("V1 actuation key exceeds 255")?),
            Self::V2 => Ok((actuation >> 8) as u8),
        }
    }

    /// The recovered handlers select the low byte for V1 and high byte for V2.
    fn event_index(self, raw_actuation: u16) -> usize {
        match self {
            Self::V1 => (raw_actuation & 0xff) as usize,
            Self::V2 => (raw_actuation >> 8) as usize,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoystickBank {
    Axes,
    RotationAxes,
}

impl JoystickBank {
    fn index(self) -> usize {
        match self {
            Self::Axes => 0,
            Self::RotationAxes => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GamepadAxis {
    LeftTrigger,
    RightTrigger,
    LeftThumbX,
    LeftThumbY,
    RightThumbX,
    RightThumbY,
}

impl GamepadAxis {
    fn index(self) -> usize {
        match self {
            Self::LeftTrigger => 0,
            Self::RightTrigger => 1,
            Self::LeftThumbX => 2,
            Self::LeftThumbY => 3,
            Self::RightThumbX => 4,
            Self::RightThumbY => 5,
        }
    }

    fn is_trigger(self) -> bool {
        self.index() < 2
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JoystickPoint {
    pub actuation: u16,
    pub axes: [i32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GamepadPoint {
    pub actuation: u16,
    pub value: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JoystickCurve {
    pub resolution: AnalogResolution,
    pub bank: JoystickBank,
    table: [[i8; 3]; 256],
}

impl JoystickCurve {
    /// Equal native buckets retain the first point, as the native tree emplace
    /// does. Callers providing typed points control their original order.
    pub fn compile(
        resolution: AnalogResolution,
        bank: JoystickBank,
        points: &[JoystickPoint],
    ) -> anyhow::Result<Self> {
        ensure!(!points.is_empty(), "Native joystick actuations are empty");
        let mut ordered = BTreeMap::<u8, [i8; 3]>::new();
        for point in points {
            let bucket = resolution.bucket(point.actuation)?;
            ordered
                .entry(bucket)
                .or_insert_with(|| point.axes.map(|value| value.clamp(-128, 127) as i8));
        }
        let mut table = [[0i8; 3]; 256];
        let mut previous_index = 0usize;
        let mut previous_value = [0i8; 3];
        for (&index, &value) in &ordered {
            let index = index as usize;
            if index > previous_index {
                let distance = index - previous_index;
                // Native uses signed byte subtraction, divss, mulss, then
                // cvttss2si before subtracting the integer from the right end.
                let slope: [f32; 3] = std::array::from_fn(|axis| {
                    (value[axis] as i32 - previous_value[axis] as i32) as f32 / distance as f32
                });
                for (step, output) in table[previous_index..=index].iter_mut().enumerate() {
                    let remaining = (distance - step) as f32;
                    *output = std::array::from_fn(|axis| {
                        let delta = (slope[axis] * remaining) as i32;
                        (value[axis] as i32 - delta) as i8
                    });
                }
            }
            previous_index = index;
            previous_value = value;
        }
        // Preserve the actual native tail loop: start at the final point and
        // stop at index 254. Without a point at 255 the last entry stays zero.
        // This is established by instructions 0xEFCDD..0xEFD0E, not inferred
        // from the Hex-Rays loop's misleading increment variables.
        if previous_index < 255 {
            table[previous_index..255].fill(previous_value);
        }
        Ok(Self {
            resolution,
            bank,
            table,
        })
    }

    pub fn evaluate(&self, raw_actuation: u16) -> [i8; 3] {
        self.table[self.resolution.event_index(raw_actuation)]
    }

    pub fn table(&self) -> &[[i8; 3]; 256] {
        &self.table
    }

    /// Exact 768-byte lookup data copied into the native mapping object.
    pub fn lookup_bytes(&self) -> [u8; 768] {
        std::array::from_fn(|index| self.table[index / 3][index % 3] as u8)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GamepadCurve {
    pub resolution: AnalogResolution,
    pub axis: GamepadAxis,
    table: [i16; 256],
}

impl GamepadCurve {
    pub fn compile(
        resolution: AnalogResolution,
        axis: GamepadAxis,
        points: &[GamepadPoint],
    ) -> anyhow::Result<Self> {
        ensure!(!points.is_empty(), "Native gamepad actuations are empty");
        let mut ordered = BTreeMap::<u8, i16>::new();
        for point in points {
            let bucket = resolution.bucket(point.actuation)?;
            // Native GetInt narrows to a signed word before the range check.
            // Preserve that conversion even for out-of-range int32 inputs.
            let value = point.value as i16;
            let value = if axis.is_trigger() {
                value.clamp(0, 255)
            } else {
                value
            };
            ordered.entry(bucket).or_insert(value);
        }
        let mut table = [0i16; 256];
        let mut previous_index = 0usize;
        let mut previous_value = 0i16;
        for (&index, &value) in &ordered {
            let index = index as usize;
            if index > previous_index {
                let distance = index - previous_index;
                let slope = (value as i32 - previous_value as i32) as f32 / distance as f32;
                for (step, output) in table[previous_index..=index].iter_mut().enumerate() {
                    // Native Xbox SIMD does mulps/addps before cvttps2dq.
                    // This differs from the joystick's integer subtraction.
                    let from_right = step as i32 - distance as i32;
                    let interpolated = from_right as f32 * slope + value as f32;
                    *output = (interpolated as i32).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
                }
            }
            previous_index = index;
            previous_value = value;
        }
        if previous_index < 255 {
            // Xbox fills from final+1 through 255. A lone point at zero leaves
            // table[0] at its initial zero; this is also native behavior.
            table[previous_index + 1..].fill(previous_value);
        }
        Ok(Self {
            resolution,
            axis,
            table,
        })
    }

    pub fn evaluate(&self, raw_actuation: u16) -> i16 {
        self.table[self.resolution.event_index(raw_actuation)]
    }

    pub fn table(&self) -> &[i16; 256] {
        &self.table
    }

    /// Exact 512-byte little-endian lookup data, not a USB report.
    pub fn lookup_bytes(&self) -> [u8; 512] {
        std::array::from_fn(|index| self.table[index / 2].to_le_bytes()[index % 2])
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeAnalogOutput {
    Joystick(JoystickCurve),
    Gamepad(GamepadCurve),
}

/// Native StringToUint accepts an optional `+`, ASCII digits and leading zeroes.
/// It rejects whitespace, minus signs, incomplete parses and overflow.
fn native_actuation_key(key: &str) -> anyhow::Result<u16> {
    let digits = key.strip_prefix('+').unwrap_or(key).as_bytes();
    ensure!(!digits.is_empty(), "Native actuation key is empty");
    let mut value = 0u32;
    for &digit in digits {
        ensure!(
            digit.is_ascii_digit(),
            "Invalid native actuation key {key:?}"
        );
        value = value
            .checked_mul(10)
            .and_then(|value| value.checked_add((digit - b'0') as u32))
            .context("Native actuation key overflows uint32")?;
    }
    u16::try_from(value).context("Native actuation key exceeds 65535")
}

fn native_integer(value: &Value) -> anyhow::Result<i32> {
    let integer = value
        .as_i64()
        .context("Native actuation requires an integer JSON value")?;
    i32::try_from(integer).context("Native actuation is outside signed int32")
}

impl NativeAnalogOutput {
    /// Parse one of the four actual native output objects and compile its table.
    /// This is the compatibility boundary for existing app-engine documents.
    pub fn from_native_json(output: &Value) -> anyhow::Result<Self> {
        let object = output
            .as_object()
            .context("Native analog output requires an object")?;
        let native_type = object
            .get("type")
            .and_then(Value::as_str)
            .context("Missing native analog type string")?;
        let (resolution, is_joystick) = match native_type {
            "analogJoystick" => (AnalogResolution::V1, true),
            "analogJoystickV2" => (AnalogResolution::V2, true),
            "analogXboxGamepad" => (AnalogResolution::V1, false),
            "analogXboxGamepadV2" => (AnalogResolution::V2, false),
            _ => bail!("Native analog output type is unsupported: {native_type}"),
        };
        let id = object
            .get("id")
            .and_then(Value::as_str)
            .context("Missing native analog id string")?;
        let actuations = object
            .get("actuations")
            .and_then(Value::as_object)
            .context("Missing native analog actuations object")?;
        // Chromium Dict is a sorted vector. Its native iterator visits string
        // keys lexicographically before the numeric bucket tree is built.
        let mut entries: Vec<_> = actuations.iter().collect();
        entries.sort_unstable_by(|(left, _), (right, _)| left.as_bytes().cmp(right.as_bytes()));
        if is_joystick {
            let bank = match id {
                "axes" => JoystickBank::Axes,
                "raxes" => JoystickBank::RotationAxes,
                _ => bail!("Unsupported native joystick id {id}"),
            };
            let points = entries
                .into_iter()
                .map(|(key, value)| {
                    let values = value
                        .as_array()
                        .context("Native joystick actuation requires an array")?;
                    ensure!(
                        values.len() == 3,
                        "Native joystick actuation requires exactly 3 axes"
                    );
                    Ok(JoystickPoint {
                        actuation: native_actuation_key(key)?,
                        axes: [
                            native_integer(&values[0])?,
                            native_integer(&values[1])?,
                            native_integer(&values[2])?,
                        ],
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            Ok(Self::Joystick(JoystickCurve::compile(
                resolution, bank, &points,
            )?))
        } else {
            let axis = match id {
                "leftTrigger" => GamepadAxis::LeftTrigger,
                "rightTrigger" => GamepadAxis::RightTrigger,
                "leftThumbX" => GamepadAxis::LeftThumbX,
                "leftThumbY" => GamepadAxis::LeftThumbY,
                "rightThumbX" => GamepadAxis::RightThumbX,
                "rightThumbY" => GamepadAxis::RightThumbY,
                _ => bail!("Unsupported native gamepad id {id}"),
            };
            let points = entries
                .into_iter()
                .map(|(key, value)| {
                    Ok(GamepadPoint {
                        actuation: native_actuation_key(key)?,
                        value: native_integer(value)?,
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            Ok(Self::Gamepad(GamepadCurve::compile(
                resolution, axis, &points,
            )?))
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnalogOutputState {
    pub axes: [i8; 3],
    pub rotation_axes: [i8; 3],
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub left_thumb: [i16; 2],
    pub right_thumb: [i16; 2],
}

/// Pure output contribution state for one logical device. It never submits a
/// report, creates a virtual device or reports a hardware acknowledgement.
#[derive(Clone, Debug)]
pub struct AnalogMappingState {
    joystick: [BTreeMap<String, [i8; 3]>; 2],
    gamepad: [BTreeMap<String, i16>; 6],
}

impl Default for AnalogMappingState {
    fn default() -> Self {
        Self {
            joystick: std::array::from_fn(|_| BTreeMap::new()),
            gamepad: std::array::from_fn(|_| BTreeMap::new()),
        }
    }
}

fn opposing_extrema(
    values: impl Iterator<Item = i32>,
    negative_limit: i32,
    positive_limit: i32,
) -> i32 {
    let (mut negative, mut positive) = (0i32, 0i32);
    for value in values {
        negative = negative.min(value);
        positive = positive.max(value);
    }
    negative + positive + i32::from(negative == negative_limit && positive == positive_limit)
}

impl AnalogMappingState {
    /// Native contributions are keyed by input identity and replaced on every
    /// event. Returning to zero removes the contribution instead of latching it.
    pub fn apply(
        &mut self,
        input_id: &str,
        output: &NativeAnalogOutput,
        raw_actuation: u16,
    ) -> AnalogOutputState {
        match output {
            NativeAnalogOutput::Joystick(curve) => {
                let value = curve.evaluate(raw_actuation);
                let contributions = &mut self.joystick[curve.bank.index()];
                if value == [0; 3] {
                    contributions.remove(input_id);
                } else {
                    contributions.insert(input_id.into(), value);
                }
            }
            NativeAnalogOutput::Gamepad(curve) => {
                let value = curve.evaluate(raw_actuation);
                let contributions = &mut self.gamepad[curve.axis.index()];
                if value == 0 {
                    contributions.remove(input_id);
                } else {
                    contributions.insert(input_id.into(), value);
                }
            }
        }
        self.output()
    }

    /// Explicit cleanup for removed mappings, released inputs or disconnects.
    /// A caller still has to submit the returned state through a real adapter.
    pub fn remove_input(&mut self, input_id: &str) -> AnalogOutputState {
        for bank in &mut self.joystick {
            bank.remove(input_id);
        }
        for axis in &mut self.gamepad {
            axis.remove(input_id);
        }
        self.output()
    }

    pub fn clear(&mut self) -> AnalogOutputState {
        for bank in &mut self.joystick {
            bank.clear();
        }
        for axis in &mut self.gamepad {
            axis.clear();
        }
        self.output()
    }

    pub fn output(&self) -> AnalogOutputState {
        let banks: [[i8; 3]; 2] = std::array::from_fn(|bank| {
            std::array::from_fn(|axis| {
                opposing_extrema(
                    self.joystick[bank].values().map(|value| value[axis] as i32),
                    -128,
                    127,
                ) as i8
            })
        });
        let gamepad: [i16; 6] = std::array::from_fn(|axis| {
            if axis < 2 {
                self.gamepad[axis].values().copied().max().unwrap_or(0)
            } else {
                opposing_extrema(
                    self.gamepad[axis].values().map(|&value| value as i32),
                    -32768,
                    32767,
                ) as i16
            }
        });
        AnalogOutputState {
            axes: banks[0],
            rotation_axes: banks[1],
            left_trigger: gamepad[0] as u8,
            right_trigger: gamepad[1] as u8,
            left_thumb: [gamepad[2], gamepad[3]],
            right_thumb: [gamepad[4], gamepad[5]],
        }
    }
}

#[cfg(test)]
#[path = "mapping_native_analog_tests.rs"]
mod tests;
