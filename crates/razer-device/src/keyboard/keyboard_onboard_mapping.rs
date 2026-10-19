//! Current679's createButtonMappingFromFnDataByte consumer. Source defaults
//! resolve HID identities; they never substitute for an unread assignment.
use super::Mode;
use crate::{
    onboard_assignment::ButtonAssignment, onboard_assignment::MacroPlayback,
    onboard_macro::MacroCatalog,
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[path = "keyboard_onboard_mapping_source.rs"]
mod source;
pub use source::{
    FunctionId, MediaAction, MouseAction, PowerAction, ProfileAction, RazerAction, ShortcutAction,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExtendedKey {
    Extended00,
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HidKey {
    pub hid: u16,
    pub flag: u8,
    pub page_id: u16,
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Turbo {
    pub period_ms: u16,
    /// A zero period produces Infinity in the source and null in JSON.
    pub keys_per_second: Option<u16>,
}

impl Turbo {
    fn from_period(period_ms: u16) -> Self {
        Self {
            period_ms,
            keys_per_second: (period_ms != 0).then(|| {
                let period = u32::from(period_ms);
                ((2_000 + period) / (2 * period)) as u16
            }),
        }
    }
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyboardOutput {
    pub key: HidKey,
    pub modifiers: u8,
    pub extended_modifier: bool,
    pub extended_key: Option<ExtendedKey>,
    pub turbo: Option<Turbo>,
}

impl KeyboardOutput {
    pub fn modifier_names(&self) -> impl Iterator<Item = &'static str> {
        let modifiers = self.modifiers;
        source::MODIFIER_NAMES
            .iter()
            .filter(move |(bit, _)| modifiers & bit != 0)
            .map(|(_, name)| *name)
    }
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MouseOutput {
    pub action: MouseAction,
    pub turbo: Option<Turbo>,
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroOutput {
    pub macro_id: u16,
    pub guid: Option<String>,
    pub playback: MacroPlayback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DpiAction {
    Up,
    Down,
    Clutch,
    CycleUp,
    CycleDown,
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DpiOutput {
    pub action: DpiAction,
    pub x: Option<u16>,
    pub y: Option<u16>,
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileNavigation {
    pub action: ProfileAction,
    pub slot_id: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionFamily {
    Dpi,
    Profile,
    Power,
    Media,
    Razer,
    WindowsShortcut,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnknownReason {
    Function,
    SourceHasNoDecoder,
    KeyboardHid { hid: u16, mode: Mode },
    KeyboardGroup { hid: u16, mode: Mode },
    MouseButton { id: u8, mode: Mode },
    Action { family: ActionFamily, code: u16 },
    ControllerCatalogAbsent,
    ScrollWheelModeDisabled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Output {
    Disabled,
    Keyboard(KeyboardOutput),
    Mouse(MouseOutput),
    Macro(MacroOutput),
    Dpi(DpiOutput),
    Profile(ProfileNavigation),
    Power(PowerAction),
    Media(MediaAction),
    Razer(RazerAction),
    WindowsShortcut(ShortcutAction),
    Hypershift,
    Unknown {
        function: Option<FunctionId>,
        reason: UnknownReason,
    },
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assignment {
    pub raw: ButtonAssignment,
    pub input_id: String,
    pub mode: Mode,
    pub output: Output,
}

struct DefaultMapping {
    input_id: &'static str,
    mode: Mode,
    firmware_key_id: Option<u8>,
    input_hid: Option<u16>,
    output: Output,
}

/// Explicit source defaults are for comparing or constructing local profiles.
/// Their presence says nothing about the connected device's current mapping.
pub fn default_output(input_id: &str, mode: Mode) -> Option<&'static Output> {
    source::KEYBOARD
        .iter()
        .rev()
        .find(|row| row.input_id == input_id && row.mode == mode)
        .map(|row| &row.output)
}

pub fn keyboard_input(hid: u16, mode: Mode) -> Option<&'static str> {
    hid_mapping(hid, mode).map(|row| row.input_id)
}

fn hid_mapping(hid: u16, mode: Mode) -> Option<&'static DefaultMapping> {
    // Object assignment in init retains the last row for each input HID.
    source::KEYBOARD
        .iter()
        .rev()
        .find(|row| row.mode == mode && row.input_hid == Some(hid))
}

fn keyboard_output(
    data: &[u8; 5],
    mode: Mode,
    turbo: bool,
) -> std::result::Result<Output, UnknownReason> {
    let modifier_only = !turbo && data[0] > 0 && data[1] == 0;
    let hid = if modifier_only {
        // The source switch deliberately has no RightGUI case.
        match data[0] {
            1 => 224,
            2 => 225,
            4 => 226,
            8 => 227,
            16 => 228,
            32 => 229,
            64 => 230,
            _ => 0,
        }
    } else {
        u16::from(data[1])
    };
    let row = hid_mapping(hid, mode).ok_or(UnknownReason::KeyboardHid { hid, mode })?;
    let group = match &row.output {
        Output::Keyboard(group) => Some(*group),
        _ if mode == Mode::Hypershift && !turbo => hid_mapping(hid, Mode::Normal).and_then(|row| {
            if let Output::Keyboard(group) = row.output {
                Some(group)
            } else {
                None
            }
        }),
        _ => None,
    };
    let mut group = group.ok_or(UnknownReason::KeyboardGroup { hid, mode })?;
    group.modifiers |= if modifier_only { 0 } else { data[0] };
    if turbo {
        group.turbo = Some(Turbo::from_period(u16::from_be_bytes([data[2], data[3]])));
    }
    Ok(Output::Keyboard(group))
}

fn mouse_output(
    id: u8,
    mode: Mode,
    turbo: Option<Turbo>,
) -> std::result::Result<Output, UnknownReason> {
    source::MOUSE
        .iter()
        .rev()
        .find(|(row_mode, row_id, _)| *row_mode == mode && *row_id == id)
        .map(|(_, _, action)| {
            Output::Mouse(MouseOutput {
                action: *action,
                turbo,
            })
        })
        .ok_or(UnknownReason::MouseButton { id, mode })
}

/// Decode one actual five-byte assignment, retaining its exact wire fields
/// even when the current source cannot resolve the output or macro identity.
pub fn decode(raw: &ButtonAssignment, macros: &MacroCatalog) -> Result<Assignment> {
    let mode = match raw.mode {
        0 => Mode::Normal,
        1 => Mode::Hypershift,
        _ => anyhow::bail!("OBM assignment mode is invalid"),
    };
    ensure!(
        raw.data_size <= 5,
        "OBM assignment data size exceeds its wire field"
    );
    let physical = source::KEYBOARD
        .iter()
        .rev()
        .find(|row| row.mode == Mode::Normal && row.firmware_key_id == Some(raw.key_id))
        .ok_or_else(|| anyhow::anyhow!("OBM assignment firmware key has no source identity"))?;
    let function = source::function_id(u16::from(raw.function_id));
    let data = &raw.data;
    let unknown_action = |family, code| UnknownReason::Action { family, code };
    let output = match function {
        None => Err(UnknownReason::Function),
        Some(FunctionId::Off) => Ok(Output::Disabled),
        Some(FunctionId::ButtonCode) => mouse_output(data[0], mode, None),
        Some(FunctionId::KeyCode) => keyboard_output(data, mode, false),
        Some(FunctionId::MacroTypeI | FunctionId::MacroTypeII | FunctionId::MacroTypeIII) => {
            let macro_id = u16::from_be_bytes([data[0], data[1]]);
            let guid = macros
                .macros
                .iter()
                .find(|row| row.metadata.macro_id == macro_id)
                .map(|row| row.metadata.guid.clone());
            let playback = match function {
                Some(FunctionId::MacroTypeI) if data[2] == 1 => MacroPlayback::Once,
                Some(FunctionId::MacroTypeI) => MacroPlayback::NTimes(data[2]),
                Some(FunctionId::MacroTypeII) => MacroPlayback::Held,
                Some(FunctionId::MacroTypeIII) => MacroPlayback::Toggle,
                _ => unreachable!(),
            };
            Ok(Output::Macro(MacroOutput {
                macro_id,
                guid,
                playback,
            }))
        }
        Some(FunctionId::DPI) => {
            let action = match data[0] {
                1 => Some(DpiAction::Up),
                2 => Some(DpiAction::Down),
                5 => Some(DpiAction::Clutch),
                6 => Some(DpiAction::CycleUp),
                7 => Some(DpiAction::CycleDown),
                _ => None,
            };
            action
                .map(|action| {
                    let (x, y) = if action == DpiAction::Clutch {
                        (
                            u16::from_be_bytes([data[1], data[2]]),
                            u16::from_be_bytes([data[3], data[4]]),
                        )
                    } else {
                        (0, 0)
                    };
                    Output::Dpi(DpiOutput {
                        action,
                        x: (x != 0).then_some(x),
                        y: (y != 0).then_some(y),
                    })
                })
                .ok_or_else(|| unknown_action(ActionFamily::Dpi, u16::from(data[0])))
        }
        Some(FunctionId::Profile) => {
            let action = match data[0] {
                1 => Some(ProfileAction::Up),
                2 => Some(ProfileAction::Down),
                3 => Some(ProfileAction::Specific),
                4 => Some(ProfileAction::CycleUp),
                5 => Some(ProfileAction::CycleDown),
                _ => None,
            };
            action
                .map(|action| {
                    Output::Profile(ProfileNavigation {
                        action,
                        slot_id: (action == ProfileAction::Specific && data[1] != 0)
                            .then_some(data[1]),
                    })
                })
                .ok_or_else(|| unknown_action(ActionFamily::Profile, u16::from(data[0])))
        }
        Some(FunctionId::PowerKeys) => source::power_action(u16::from(data[0]))
            .map(Output::Power)
            .ok_or_else(|| unknown_action(ActionFamily::Power, u16::from(data[0]))),
        Some(FunctionId::MediaKeys) => {
            let code = u16::from_be_bytes([data[0], data[1]]);
            source::media_action(code)
                .map(Output::Media)
                .ok_or_else(|| unknown_action(ActionFamily::Media, code))
        }
        Some(FunctionId::DoubleClick) => Ok(Output::Mouse(MouseOutput {
            action: MouseAction::DoubleClick,
            turbo: None,
        })),
        Some(FunctionId::ModeButtonkey) => Ok(Output::Hypershift),
        Some(FunctionId::TurboModeKey) => keyboard_output(data, mode, true),
        Some(FunctionId::TurboModeButton) => mouse_output(
            data[0],
            mode,
            Some(Turbo::from_period(u16::from_be_bytes([data[1], data[2]]))),
        ),
        Some(FunctionId::RazerKey) => source::razer_action(u16::from(data[0]))
            .map(Output::Razer)
            .ok_or_else(|| unknown_action(ActionFamily::Razer, u16::from(data[0]))),
        Some(FunctionId::Win8ShortcutsKey) => source::shortcut_action(u16::from(data[0]))
            .map(Output::WindowsShortcut)
            .ok_or_else(|| unknown_action(ActionFamily::WindowsShortcut, u16::from(data[0]))),
        Some(FunctionId::Controller) => Err(UnknownReason::ControllerCatalogAbsent),
        Some(FunctionId::ScrollWheelMode) => Err(UnknownReason::ScrollWheelModeDisabled),
        Some(FunctionId::Lighting | FunctionId::MacroTypeIV) => {
            Err(UnknownReason::SourceHasNoDecoder)
        }
    };
    Ok(Assignment {
        raw: *raw,
        input_id: physical.input_id.into(),
        mode,
        output: output.unwrap_or_else(|reason| Output::Unknown { function, reason }),
    })
}

#[cfg(test)]
#[path = "keyboard_onboard_mapping_tests.rs"]
mod tests;
