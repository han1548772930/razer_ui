use super::*;
use crate::mapping_native_input::MouseInput;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum InputKind {
    Keyboard { scancode: u16, flag: u16 },
    AnalogKey { scancode: u16, flag: u16 },
    Mouse { id: u16, data: u16 },
    AnalogStick { axis: u8, flag: u8 },
    External { key: u32, flag: u32 },
    RazerKey { key: u32, flag: u32 },
    StreamController { key: u32, flag: u32 },
    ControlPodButton { source: bool, data: u8 },
    ControlPodDial { clockwise: bool },
    CoolingPad { id: u8, byte1: u8, byte2: u8 },
    RazerEvent(Vec<i8>),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Input {
    pub kind: InputKind,
    pub modifiers: u32,
    pub unit_id: Option<u16>,
    pub report_id: Option<u16>,
    pub delay_ms: Option<u32>,
    pub release_after_ms: Option<u32>,
    pub repeat: bool,
    pub interrupts: Vec<Input>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct InputIdentity {
    kind: InputKind,
    unit_id: Option<u16>,
    report_id: Option<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Press,
    Release,
    Instant,
}

impl Input {
    /// ExternalInput keeps its serialized modifier field outside the base
    /// modifier slot used by the native global-shortcut key (0x100150).
    pub fn lookup_modifiers(&self) -> u32 {
        if matches!(self.kind, InputKind::External { .. }) {
            0
        } else {
            self.modifiers
        }
    }
    pub fn new(kind: InputKind) -> Self {
        Self {
            kind,
            modifiers: 0,
            unit_id: None,
            report_id: None,
            delay_ms: None,
            release_after_ms: None,
            repeat: false,
            interrupts: Vec::new(),
        }
    }

    pub fn phase(&self) -> Phase {
        let flag = match self.kind {
            InputKind::Keyboard { flag, .. } | InputKind::AnalogKey { flag, .. } => {
                Some(u32::from(flag & 1))
            }
            InputKind::External { flag, .. } => Some(flag & 1),
            InputKind::RazerKey { flag, .. } | InputKind::StreamController { flag, .. } => {
                Some(flag)
            }
            InputKind::AnalogStick { flag, .. } => Some(flag.into()),
            InputKind::Mouse { id, data } => {
                let input = MouseInput {
                    id,
                    data,
                    ..Default::default()
                };
                return if input.is_pressed() {
                    Phase::Press
                } else if input.is_released() {
                    Phase::Release
                } else {
                    Phase::Instant
                };
            }
            _ => None,
        };
        match flag {
            Some(0) => Phase::Press,
            Some(1) => Phase::Release,
            _ => Phase::Instant,
        }
    }

    pub fn released(&self) -> Self {
        self.with_phase(true)
    }
    pub fn pressed(&self) -> Self {
        self.with_phase(false)
    }

    fn with_phase(&self, release: bool) -> Self {
        let mut input = self.clone();
        match &mut input.kind {
            InputKind::Keyboard { flag, .. } | InputKind::AnalogKey { flag, .. } => {
                *flag = (*flag & !1) | u16::from(release)
            }
            InputKind::External { flag, .. }
            | InputKind::RazerKey { flag, .. }
            | InputKind::StreamController { flag, .. } => *flag = u32::from(release),
            InputKind::AnalogStick { flag, .. } => *flag = u8::from(release),
            InputKind::Mouse { id, data } => {
                let mouse = MouseInput {
                    id: *id,
                    data: *data,
                    ..Default::default()
                };
                let mouse = if release { mouse.released() } else { mouse.pressed() };
                *id = mouse.id;
                *data = mouse.data;
            }
            _ => {}
        }
        input
    }

    /// The native virtual +56 identity excludes modifiers and repeat/delay
    /// metadata. +64 additionally removes the edge and, for Razer keys, report.
    pub fn lookup_identity(&self) -> InputIdentity {
        let kind = match self.kind {
            InputKind::External { key, flag } => InputKind::External {
                key: 0,
                flag: key.wrapping_shl(16).wrapping_add(flag),
            },
            _ => self.kind.clone(),
        };
        InputIdentity {
            kind,
            unit_id: self.unit_id,
            report_id: self.report_id,
        }
    }

    pub fn identity(&self) -> InputIdentity {
        let mut input = self.pressed();
        if let InputKind::External { key, flag } = self.kind {
            input.kind = InputKind::External {
                key: 0,
                flag: key.wrapping_shl(16).wrapping_add(flag & !1),
            };
        }
        if matches!(input.kind, InputKind::RazerKey { .. }) {
            input.report_id = None;
        }
        InputIdentity {
            kind: input.kind,
            unit_id: input.unit_id,
            report_id: input.report_id,
        }
    }
}

pub(super) fn optional_int(value: &Value, key: &str) -> Result<Option<i32>> {
    value
        .get(key)
        .map(|value| {
            value
                .as_i64()
                .and_then(|n| n.try_into().ok())
                .with_context(|| format!("Invalid int32 input field {key}"))
        })
        .transpose()
}
fn integer(value: &Value, key: &str) -> Result<i32> {
    optional_int(value, key)?.with_context(|| format!("Missing input field {key}"))
}
pub(super) fn optional_bool(value: &Value, key: &str) -> Result<bool> {
    value
        .get(key)
        .map(|v| {
            v.as_bool()
                .with_context(|| format!("Invalid bool input field {key}"))
        })
        .transpose()
        .map(|v| v.unwrap_or(false))
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .with_context(|| format!("Missing input string {key}"))
}

/// JSON is confined to this current-source constructor boundary (0xee5a1).
pub fn parse_input(value: &Value) -> Result<Input> {
    let kind = match text(value, "type")? {
        "keyboard" => InputKind::Keyboard {
            scancode: integer(value, "scancode")? as u16,
            flag: integer(value, "flag")? as u16,
        },
        "analogKey" => InputKind::AnalogKey {
            scancode: integer(value, "scancode")? as u16,
            flag: integer(value, "flag")? as u16,
        },
        "mouse" => {
            let id = integer(value, "id")? as u16;
            let data = optional_int(value, "data")?.unwrap_or(0) as u16;
            ensure!(
                MouseInput {
                    id,
                    data,
                    ..Default::default()
                }
                .mapping_id_valid(),
                "Invalid mapping mouse id/data"
            );
            InputKind::Mouse { id, data }
        }
        "analogStick" => {
            let axis = match text(value, "axis")? {
                "left" => 0,
                "right" => 1,
                "up" => 2,
                "down" => 3,
                other => bail!("Invalid analog axis {other}"),
            };
            let flag = integer(value, "flag")? as u8;
            ensure!(flag < 2, "Invalid analog stick flag");
            InputKind::AnalogStick { axis, flag }
        }
        "external" => InputKind::External {
            key: integer(value, "key")? as u32,
            flag: integer(value, "flag")? as u32,
        },
        "razerKey" => InputKind::RazerKey {
            key: integer(value, "key")? as u32,
            flag: integer(value, "flag")? as u32,
        },
        "streamController" => InputKind::StreamController {
            key: integer(value, "key")? as u32,
            flag: integer(value, "flag")? as u32,
        },
        "controlPod" => match text(value, "id")? {
            "topButton" => InputKind::ControlPodButton {
                source: false,
                data: integer(value, "data")? as u8,
            },
            "sourceButton" => InputKind::ControlPodButton {
                source: true,
                data: integer(value, "data")? as u8,
            },
            "dial" => InputKind::ControlPodDial {
                clockwise: match text(value, "rotation")? {
                    "clockwise" => true,
                    "anticlockwise" => false,
                    other => bail!("Invalid dial rotation {other}"),
                },
            },
            other => bail!("Invalid control pod id {other}"),
        },
        "coolingPad" => InputKind::CoolingPad {
            id: integer(value, "id")? as u8,
            byte1: integer(value, "byte1")? as u8,
            byte2: integer(value, "byte2")? as u8,
        },
        "razerEvent" => {
            let data = value
                .get("data")
                .and_then(Value::as_array)
                .context("Missing Razer event data")?;
            let data = data
                .iter()
                .map(|n| {
                    let n: i32 = n
                        .as_i64()
                        .and_then(|v| v.try_into().ok())
                        .context("Invalid Razer event int32")?;
                    ensure!(n != 0, "Zero Razer event byte");
                    Ok(if n < -127 { -128 } else { n.min(127) as i8 })
                })
                .collect::<Result<Vec<_>>>()?;
            InputKind::RazerEvent(data)
        }
        other => bail!("Unknown mapping input type {other}"),
    };
    let mut input = Input::new(kind);
    if matches!(
        input.kind,
        InputKind::Keyboard { .. }
            | InputKind::Mouse { .. }
            | InputKind::AnalogKey { .. }
            | InputKind::RazerKey { .. }
            | InputKind::External { .. }
    ) {
        input.modifiers = optional_int(value, "modifiers")?.unwrap_or(0) as u32;
    }
    if matches!(
        input.kind,
        InputKind::Keyboard { .. } | InputKind::Mouse { .. }
    ) {
        input.unit_id = optional_int(value, "unitId")?.map(|v| v as u16);
    }
    if matches!(input.kind, InputKind::Keyboard { .. }) {
        input.delay_ms = optional_int(value, "delayInMs")?.map(|v| v as u32);
        input.release_after_ms = optional_int(value, "releaseAfterMs")?.map(|v| v as u32);
    }
    if matches!(input.kind, InputKind::RazerKey { .. }) {
        input.report_id = optional_int(value, "reportId")?.map(|v| v as u16);
    }
    if matches!(input.kind, InputKind::ControlPodDial { .. }) {
        input.repeat = optional_bool(value, "repeat")?;
    }
    if let Some(interrupts) = value.get("interrupts") {
        let mut seen = BTreeSet::new();
        for raw in interrupts.as_array().context("Invalid input interrupts")? {
            // 0xee0f1 parses basic inputs here, not recursively nested lists.
            ensure!(
                raw.get("interrupts").is_none(),
                "Nested input interrupts are invalid"
            );
            let interrupt = parse_input(raw)?;
            ensure!(
                interrupt.phase() != Phase::Release,
                "Break interrupt is invalid"
            );
            ensure!(
                seen.insert(interrupt.identity()),
                "Duplicate input interrupt"
            );
            input.interrupts.push(interrupt);
        }
    }
    Ok(input)
}
