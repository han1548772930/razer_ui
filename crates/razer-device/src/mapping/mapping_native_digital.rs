//! Digital joystick/gamepad state and shared analog contributions.
//!
//! The current native handlers use wrapping byte button counters and keyed
//! extrema contributions, not sums of every held axis. Digital and analog
//! mappings update the same contribution containers. This module reproduces
//! that shared state without invoking DLLs, hooks or a transport. Returned
//! requests are plans; they never constitute a driver acknowledgement.

use crate::{
    mapping_native_analog::{GamepadAxis, JoystickBank, NativeAnalogOutput},
    mapping_native_output::{
        DriverInput, DriverInputReport, MappingOutputState, changed_driver_inputs,
        encode_driver_input,
    },
};
use serde_json::Value;
use std::{collections::BTreeMap, fmt};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum JoystickGroup {
    Axes,
    RotationAxes,
    Pov,
}

impl JoystickGroup {
    fn index(self) -> usize {
        match self {
            Self::Axes => 0,
            Self::RotationAxes => 1,
            Self::Pov => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum NativeDigitalOutput {
    /// Native parser keeps a DWORD button number. The event handler ignores
    /// numbers outside 1..=24. Flag 0 increments, 1 decrements, others do nothing.
    JoystickButton { button: u32, flag: u8 },
    JoystickAxes {
        group: JoystickGroup,
        value: [i8; 3],
    },
    /// Bits 10 and 11 are ignored by the native event handler. Other bits have
    /// independent counters; flag 0 presses, 1 releases, others do nothing.
    GamepadButtons { mask: u16, flag: u8 },
    /// Trigger values must be within 0..=255, as required by the current JSON
    /// constructor. Thumb values retain their native signed WORD bit pattern.
    GamepadAxis { axis: GamepadAxis, value: i16 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeDigitalError {
    ExpectedObject,
    MissingString(&'static str),
    UnsupportedType(String),
    UnsupportedId(String),
    ExpectedInt32(&'static str),
    ExpectedXyz,
    InvalidJoystickFlag(u8),
    InvalidTriggerValue(i16),
}

impl fmt::Display for NativeDigitalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpectedObject => write!(f, "native digital mapping requires an object"),
            Self::MissingString(field) => {
                write!(f, "native digital mapping requires string {field}")
            }
            Self::UnsupportedType(value) => {
                write!(f, "unsupported native digital mapping type {value}")
            }
            Self::UnsupportedId(value) => {
                write!(f, "unsupported native digital mapping id {value}")
            }
            Self::ExpectedInt32(field) => {
                write!(f, "native digital mapping requires int32 {field}")
            }
            Self::ExpectedXyz => {
                write!(f, "native joystick xyz requires exactly three int32 values")
            }
            Self::InvalidJoystickFlag(flag) => {
                write!(f, "native joystick button flag {flag} is outside 0..=1")
            }
            Self::InvalidTriggerValue(value) => {
                write!(f, "native gamepad trigger value {value} is outside 0..=255")
            }
        }
    }
}

impl std::error::Error for NativeDigitalError {}

impl NativeDigitalOutput {
    /// Compatibility boundary only. The native integer field helpers narrow
    /// int32 to the requested byte/word/dword before branch validation; preserve
    /// that behavior rather than accepting JSON doubles or adding saturation.
    pub fn from_native_json(output: &Value) -> Result<Self, NativeDigitalError> {
        let object = output
            .as_object()
            .ok_or(NativeDigitalError::ExpectedObject)?;
        let string = |field| {
            object
                .get(field)
                .and_then(Value::as_str)
                .ok_or(NativeDigitalError::MissingString(field))
        };
        let integer = |field| {
            object
                .get(field)
                .and_then(Value::as_i64)
                .and_then(|value| i32::try_from(value).ok())
                .ok_or(NativeDigitalError::ExpectedInt32(field))
        };
        let native_type = string("type")?;
        let id = string("id")?;
        match native_type {
            "joystick" => match id {
                "button" => {
                    let button = integer("button")? as u32;
                    let flag = integer("flag")? as u8;
                    if flag >= 2 {
                        return Err(NativeDigitalError::InvalidJoystickFlag(flag));
                    }
                    Ok(Self::JoystickButton { button, flag })
                }
                "axes" | "raxes" | "pov" => {
                    let xyz = object
                        .get("xyz")
                        .and_then(Value::as_array)
                        .filter(|values| values.len() == 3)
                        .ok_or(NativeDigitalError::ExpectedXyz)?;
                    let mut value = [0; 3];
                    for (index, item) in xyz.iter().enumerate() {
                        value[index] = item
                            .as_i64()
                            .and_then(|number| i32::try_from(number).ok())
                            .ok_or(NativeDigitalError::ExpectedXyz)?
                            .clamp(-128, 127) as i8;
                    }
                    let group = match id {
                        "axes" => JoystickGroup::Axes,
                        "raxes" => JoystickGroup::RotationAxes,
                        _ => JoystickGroup::Pov,
                    };
                    Ok(Self::JoystickAxes { group, value })
                }
                _ => Err(NativeDigitalError::UnsupportedId(id.into())),
            },
            "xboxGamepad" => {
                let value = integer("value")? as i16;
                if id == "button" {
                    return Ok(Self::GamepadButtons {
                        mask: value as u16,
                        flag: integer("flag")? as u8,
                    });
                }
                let axis = match id {
                    "leftTrigger" => GamepadAxis::LeftTrigger,
                    "rightTrigger" => GamepadAxis::RightTrigger,
                    "leftThumbX" => GamepadAxis::LeftThumbX,
                    "leftThumbY" => GamepadAxis::LeftThumbY,
                    "rightThumbX" => GamepadAxis::RightThumbX,
                    "rightThumbY" => GamepadAxis::RightThumbY,
                    _ => return Err(NativeDigitalError::UnsupportedId(id.into())),
                };
                validate_gamepad_value(axis, value)?;
                Ok(Self::GamepadAxis { axis, value })
            }
            _ => Err(NativeDigitalError::UnsupportedType(native_type.into())),
        }
    }
}

fn gamepad_index(axis: GamepadAxis) -> usize {
    match axis {
        GamepadAxis::LeftTrigger => 0,
        GamepadAxis::RightTrigger => 1,
        GamepadAxis::LeftThumbX => 2,
        GamepadAxis::LeftThumbY => 3,
        GamepadAxis::RightThumbX => 4,
        GamepadAxis::RightThumbY => 5,
    }
}

fn validate_gamepad_value(axis: GamepadAxis, value: i16) -> Result<(), NativeDigitalError> {
    if gamepad_index(axis) < 2 && !(0..=255).contains(&value) {
        Err(NativeDigitalError::InvalidTriggerValue(value))
    } else {
        Ok(())
    }
}

pub const GAMEPAD_SUPPORTED_BUTTONS: u16 = 0xf3ff;
const GAMEPAD_BUTTON_BITS: [u16; 14] = [
    1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 0x1000, 0x2000, 0x4000, 0x8000,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeMappingDelta {
    pub previous: MappingOutputState,
    pub desired: MappingOutputState,
    pub requests: Vec<DriverInput>,
}

impl NativeMappingDelta {
    fn new(previous: MappingOutputState, desired: MappingOutputState) -> Self {
        Self {
            previous,
            desired,
            requests: changed_driver_inputs(previous, desired),
        }
    }

    pub fn encoded_reports(&self) -> Vec<DriverInputReport> {
        self.requests
            .iter()
            .copied()
            .map(encode_driver_input)
            .collect()
    }
}

/// One logical device's native contribution state. This is shared by both
/// digital and analog handlers, keeping input identities in each target bank.
/// Button counters deliberately do not deduplicate by input id: the native
/// callers are responsible for pairing every press with a release.
#[derive(Clone, Debug)]
pub struct NativeDigitalMappingState {
    output: MappingOutputState,
    joystick_button_counts: [u8; 24],
    gamepad_button_counts: [u8; 14],
    joystick: [BTreeMap<String, [i8; 3]>; 3],
    gamepad: [BTreeMap<String, i16>; 6],
}

impl Default for NativeDigitalMappingState {
    fn default() -> Self {
        Self {
            output: MappingOutputState::default(),
            joystick_button_counts: [0; 24],
            gamepad_button_counts: [0; 14],
            joystick: std::array::from_fn(|_| BTreeMap::new()),
            gamepad: std::array::from_fn(|_| BTreeMap::new()),
        }
    }
}

fn opposing_extrema(values: impl Iterator<Item = i32>, min: i32, max: i32) -> i32 {
    let (mut negative, mut positive) = (0, 0);
    for value in values {
        negative = negative.min(value);
        positive = positive.max(value);
    }
    negative + positive + i32::from(negative == min && positive == max)
}

impl NativeDigitalMappingState {
    pub fn output(&self) -> MappingOutputState {
        self.output
    }

    pub fn joystick_button_count(&self, button: u32) -> Option<u8> {
        button
            .checked_sub(1)
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|index| self.joystick_button_counts.get(index))
            .copied()
    }

    /// Query one supported bit. Unsupported bits and combined masks return None.
    pub fn gamepad_button_count(&self, bit: u16) -> Option<u8> {
        GAMEPAD_BUTTON_BITS
            .iter()
            .position(|&supported| supported == bit)
            .map(|index| self.gamepad_button_counts[index])
    }

    pub fn joystick_contribution_count(&self, group: JoystickGroup) -> usize {
        self.joystick[group.index()].len()
    }

    pub fn gamepad_contribution_count(&self, axis: GamepadAxis) -> usize {
        self.gamepad[gamepad_index(axis)].len()
    }

    pub fn apply_digital(
        &mut self,
        input_id: &str,
        output: NativeDigitalOutput,
    ) -> Result<NativeMappingDelta, NativeDigitalError> {
        let previous = self.output;
        match output {
            NativeDigitalOutput::JoystickButton { button, flag } => {
                if let Some(index) = button.checked_sub(1).filter(|&index| index < 24) {
                    let count = &mut self.joystick_button_counts[index as usize];
                    match flag {
                        0 => *count = count.wrapping_add(1),
                        1 => *count = count.wrapping_sub(1),
                        _ => {}
                    }
                    self.output.joystick.buttons[index as usize] = *count != 0;
                }
            }
            NativeDigitalOutput::JoystickAxes { group, value } => {
                self.update_joystick(input_id, group, value);
            }
            NativeDigitalOutput::GamepadButtons { mask, flag } => {
                for (index, bit) in GAMEPAD_BUTTON_BITS.into_iter().enumerate() {
                    if mask & bit == 0 {
                        continue;
                    }
                    let count = &mut self.gamepad_button_counts[index];
                    match flag {
                        0 => {
                            *count = count.wrapping_add(1);
                            self.output.gamepad.buttons |= bit;
                        }
                        1 => {
                            let old = *count;
                            *count = count.wrapping_sub(1);
                            if old == 1 {
                                self.output.gamepad.buttons &= !bit;
                            }
                        }
                        _ => {}
                    }
                }
            }
            NativeDigitalOutput::GamepadAxis { axis, value } => {
                validate_gamepad_value(axis, value)?;
                self.update_gamepad(input_id, axis, value);
            }
        }
        Ok(NativeMappingDelta::new(previous, self.output))
    }

    /// Evaluate the existing source-verified V1/V2 curve into the very same
    /// containers digital mappings use. An identical id in a bank replaces its
    /// prior value even when the new event comes from a different handler.
    pub fn apply_analog(
        &mut self,
        input_id: &str,
        output: &NativeAnalogOutput,
        raw_actuation: u16,
    ) -> NativeMappingDelta {
        let previous = self.output;
        match output {
            NativeAnalogOutput::Joystick(curve) => {
                let group = match curve.bank {
                    JoystickBank::Axes => JoystickGroup::Axes,
                    JoystickBank::RotationAxes => JoystickGroup::RotationAxes,
                };
                self.update_joystick(input_id, group, curve.evaluate(raw_actuation));
            }
            NativeAnalogOutput::Gamepad(curve) => {
                self.update_gamepad(input_id, curve.axis, curve.evaluate(raw_actuation));
            }
        }
        NativeMappingDelta::new(previous, self.output)
    }

    fn update_joystick(&mut self, input_id: &str, group: JoystickGroup, value: [i8; 3]) {
        let contributions = &mut self.joystick[group.index()];
        if value == [0; 3] {
            contributions.remove(input_id);
        } else {
            contributions.insert(input_id.into(), value);
        }
        self.refresh_joystick(group);
    }

    fn refresh_joystick(&mut self, group: JoystickGroup) {
        let contributions = &self.joystick[group.index()];
        let value = std::array::from_fn(|axis| {
            opposing_extrema(
                contributions.values().map(|value| value[axis] as i32),
                -128,
                127,
            ) as i8
        });
        match group {
            JoystickGroup::Axes => self.output.joystick.axes = value,
            JoystickGroup::RotationAxes => self.output.joystick.rotation_axes = value,
            JoystickGroup::Pov => self.output.joystick.pov = value,
        }
    }

    fn update_gamepad(&mut self, input_id: &str, axis: GamepadAxis, value: i16) {
        let index = gamepad_index(axis);
        let contributions = &mut self.gamepad[index];
        if value == 0 || (index < 2 && value < 0) {
            contributions.remove(input_id);
        } else {
            contributions.insert(input_id.into(), value);
        }
        self.refresh_gamepad(axis);
    }

    fn refresh_gamepad(&mut self, axis: GamepadAxis) {
        let index = gamepad_index(axis);
        let contributions = &self.gamepad[index];
        let value = if index < 2 {
            contributions.values().copied().max().unwrap_or(0)
        } else {
            opposing_extrema(
                contributions.values().map(|&value| value as i32),
                -32768,
                32767,
            ) as i16
        };
        match axis {
            GamepadAxis::LeftTrigger => self.output.gamepad.left_trigger = value as u8,
            GamepadAxis::RightTrigger => self.output.gamepad.right_trigger = value as u8,
            GamepadAxis::LeftThumbX => self.output.gamepad.left_thumb[0] = value,
            GamepadAxis::LeftThumbY => self.output.gamepad.left_thumb[1] = value,
            GamepadAxis::RightThumbX => self.output.gamepad.right_thumb[0] = value,
            GamepadAxis::RightThumbY => self.output.gamepad.right_thumb[1] = value,
        }
    }

    /// Explicit axis-map removal is separate from an analog release event.
    /// Native release evaluates raw zero through its curve, whose zero point
    /// can itself be nonzero. Button counters require explicit release commands.
    pub fn remove_axis_input(&mut self, input_id: &str) -> NativeMappingDelta {
        let previous = self.output;
        for group in [
            JoystickGroup::Axes,
            JoystickGroup::RotationAxes,
            JoystickGroup::Pov,
        ] {
            if self.joystick[group.index()].remove(input_id).is_some() {
                self.refresh_joystick(group);
            }
        }
        for axis in [
            GamepadAxis::LeftTrigger,
            GamepadAxis::RightTrigger,
            GamepadAxis::LeftThumbX,
            GamepadAxis::LeftThumbY,
            GamepadAxis::RightThumbX,
            GamepadAxis::RightThumbY,
        ] {
            if self.gamepad[gamepad_index(axis)].remove(input_id).is_some() {
                self.refresh_gamepad(axis);
            }
        }
        NativeMappingDelta::new(previous, self.output)
    }

    /// RVA 0x100DE0 clears joystick counters, output bytes and all three banks.
    pub fn native_pooled_reset_joystick(&mut self) -> NativeMappingDelta {
        let previous = self.output;
        self.output.joystick = Default::default();
        self.joystick_button_counts = [0; 24];
        for group in &mut self.joystick {
            group.clear();
        }
        NativeMappingDelta::new(previous, self.output)
    }

    /// RVA 0x1054F0 clears only the 12 output bytes; counters and axis maps
    /// remain. Future events recalculate their addressed field independently.
    pub fn native_pooled_reset_gamepad(&mut self) -> NativeMappingDelta {
        let previous = self.output;
        self.output.gamepad = Default::default();
        NativeMappingDelta::new(previous, self.output)
    }

    /// Caller-requested complete cleanup; it is intentionally distinct from
    /// the native Xbox pooled reset and from replaying raw-zero analog release.
    pub fn clear(&mut self) -> NativeMappingDelta {
        let previous = self.output;
        *self = Self::default();
        NativeMappingDelta::new(previous, self.output)
    }
}

#[cfg(test)]
#[path = "mapping_native_digital_tests.rs"]
mod tests;
