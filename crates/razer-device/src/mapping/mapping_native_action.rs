//! Typed current mapping outputs (native 0xef470 and its nested parsers).
//! Output selection and transport execution are separate from this boundary.
use crate::{
    macro_runtime::{Event, parse_event},
    mapping_native_analog::NativeAnalogOutput,
    mapping_native_digital::NativeDigitalOutput,
    mapping_native_lookup::{Input, parse_input},
};
use anyhow::{Context as _, Result, bail, ensure};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModeScope {
    Global,
    Interdevice,
    Local,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemAction {
    Hibernate,
    Sleep,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayAction {
    Brightness(u8),
    BrightnessDown(u8),
    BrightnessUp(u8),
    DriverBrightnessDown,
    DriverBrightnessUp,
    DriverBrightnessStop,
}

/// Native mode/copy-text/unrecognized-output objects retain their JSON for
/// the verified notification boundary. Consumers must decode their own domain
/// contract; this payload does not claim an action was performed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotificationPayload(Box<str>);
impl NotificationPayload {
    fn new(value: &Value) -> Result<Self> {
        Ok(Self(serde_json::to_string(value)?.into()))
    }
    pub fn as_json(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Branch {
    pub input: Option<Input>,
    pub output: Output,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trigger {
    pub actuation: u16,
    pub output: Output,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Output {
    Event(Event),
    Digital(NativeDigitalOutput),
    Analog(NativeAnalogOutput),
    Macro {
        guid: String,
        flag: u32,
        repeat: u32,
    },
    PhasedMacro {
        guid: String,
        flag: u32,
        delay_ms: u32,
    },
    Turbo {
        guid: String,
        flag: u32,
        repeat: u32,
        delay_ms: u32,
    },
    Disabled,
    Display(DisplayAction),
    Hypershift {
        flag: u32,
    },
    Otfs {
        flag: u32,
    },
    Mode {
        scope: ModeScope,
        flag: u16,
        payload: NotificationPayload,
    },
    System(SystemAction),
    CopyText(NotificationPayload),
    Multi(Vec<Output>),
    Cascade(Vec<Branch>),
    Dpad(Vec<Branch>),
    DoubleMapping {
        analog: NativeAnalogOutput,
        digital: Box<Output>,
    },
    RapidTrigger {
        actuation: u16,
        make_sensitivity: u16,
        break_sensitivity: u16,
        continuous: Option<bool>,
        mapping: Box<Output>,
    },
    Triggers(BTreeMap<u16, Trigger>),
    ModTap {
        milliseconds: u32,
        tap: Box<Output>,
        hold: Box<Output>,
    },
    Forwarded {
        kind: String,
        payload: NotificationPayload,
    },
}

#[derive(Clone, Copy)]
enum Context {
    Direct,
    Multi,
    Cascade,
    Dpad,
    ModTap,
    Digital,
    Rapid,
    Trigger,
}

impl Context {
    fn permits(self, kind: &str) -> bool {
        let common = matches!(
            kind,
            "audio"
                | "clipboard"
                | "display"
                | "hypershift"
                | "joystick"
                | "keyboard"
                | "launch"
                | "macro"
                | "mode"
                | "mouse"
                | "otfs"
                | "turbo"
        );
        if common {
            return true;
        }
        match self {
            Self::Direct => matches!(
                kind,
                "analogJoystick"
                    | "analogJoystickV2"
                    | "analogXboxGamepad"
                    | "analogXboxGamepadV2"
                    | "cascade"
                    | "copyText"
                    | "disabled"
                    | "doubleMapping"
                    | "dpad"
                    | "multi"
                    | "modtap"
                    | "phasedMacro"
                    | "rapidTrigger"
                    | "system"
                    | "triggers"
                    | "xboxGamepad"
            ),
            Self::Multi => matches!(kind, "copyText" | "delay" | "randomDelay" | "system"),
            Self::Cascade | Self::Rapid => matches!(
                kind,
                "cascade" | "disabled" | "multi" | "system" | "xboxGamepad"
            ),
            Self::Dpad | Self::ModTap => {
                matches!(kind, "disabled" | "multi" | "system" | "xboxGamepad")
            }
            Self::Digital => matches!(kind, "disabled" | "multi" | "triggers" | "xboxGamepad"),
            Self::Trigger => matches!(kind, "multi" | "system" | "xboxGamepad"),
        }
    }
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .with_context(|| format!("Missing mapping output string {key}"))
}
fn optional_int(value: &Value, key: &str) -> Option<i32> {
    value.get(key)?.as_i64()?.try_into().ok()
}
fn integer(value: &Value, key: &str) -> Result<i32> {
    optional_int(value, key).with_context(|| format!("Missing mapping output int32 {key}"))
}
fn flag(value: &Value) -> Result<u32> {
    let flag = integer(value, "flag")? as u32;
    ensure!(flag < 2, "Invalid output flag");
    Ok(flag)
}
fn object<'a>(value: &'a Value, key: &str) -> Result<&'a Value> {
    value
        .get(key)
        .filter(|v| v.is_object())
        .with_context(|| format!("Missing mapping output object {key}"))
}

pub fn parse_output(value: &Value) -> Result<Output> {
    parse(value, Context::Direct, 0)
}

/// Compile default and active mappings independently before their source merge.
/// Native 0xe8888 removes identity outputs except dynamic-key/Snap Tap entries.
pub fn device_tables(
    document: &Value,
    global_engine: Option<&Value>,
) -> Result<crate::mapping_native_lookup::Tables<Output>> {
    crate::mapping_native_lookup::from_device_document_filtered(
        document,
        global_engine,
        parse_output,
        |mapping| {
            if let Output::PhasedMacro { flag, .. } = &mapping.output {
                use crate::mapping_native_lookup::Phase;
                ensure!(
                    matches!(
                        (mapping.input.phase(), *flag),
                        (Phase::Press, 0) | (Phase::Release, 1)
                    ),
                    "Phased macro flag differs from input edge"
                );
            }
            Ok(mapping.selectors.dynamic_key.is_some()
                || mapping.selectors.snap_tap.is_some()
                || !mapping.output.equals_input(&mapping.input))
        },
    )
}

impl Output {
    fn equals_input(&self, input: &Input) -> bool {
        use crate::mapping_native_lookup::InputKind;
        match (self, &input.kind) {
            (Self::Event(Event::Keyboard(output)), InputKind::Keyboard { scancode, flag }) => {
                output.scancode == *scancode
                    && output.flag == *flag
                    && output.modifiers == input.modifiers
                    && output.unit == input.unit_id
                    && output.delay_ms == input.delay_ms
                    && output.release_after_ms == input.release_after_ms
            }
            (Self::Event(Event::Mouse(output)), InputKind::Mouse { id, data }) => {
                output.id == *id
                    && output.data == *data
                    && output.modifiers == input.modifiers
                    && output.unit == input.unit_id
            }
            _ => false,
        }
    }
}

fn parse(value: &Value, context: Context, depth: usize) -> Result<Output> {
    ensure!(
        depth < 200,
        "Mapping output exceeds source JSON nesting limit"
    );
    let kind = text(value, "type")?;
    if !context.permits(kind) {
        return Ok(Output::Forwarded {
            kind: kind.into(),
            payload: NotificationPayload::new(value)?,
        });
    }
    Ok(match kind {
        "keyboard" | "mouse" | "audio" | "launch" | "clipboard" | "delay" | "randomDelay" => {
            Output::Event(parse_event(value)?)
        }
        "joystick" | "xboxGamepad" => {
            Output::Digital(NativeDigitalOutput::from_native_json(value)?)
        }
        "analogJoystick" | "analogJoystickV2" | "analogXboxGamepad" | "analogXboxGamepadV2" => {
            Output::Analog(NativeAnalogOutput::from_native_json(value)?)
        }
        "macro" => {
            let guid = text(value, "guid")?;
            ensure!(!guid.is_empty(), "Empty macro GUID");
            let flag = integer(value, "flag")? as u32;
            ensure!(flag <= 4, "Invalid macro flag");
            let repeat = if matches!(flag, 2 | 3) {
                0
            } else {
                optional_int(value, "repeat")
                    .map(|v| v as u32)
                    .unwrap_or(u32::from(flag == 4))
            };
            Output::Macro {
                guid: guid.into(),
                flag,
                repeat,
            }
        }
        "phasedMacro" => Output::PhasedMacro {
            guid: text(value, "guid")?.into(),
            // 0xf36f2 logs an invalid flag but still constructs the object.
            flag: integer(value, "flag")? as u32,
            delay_ms: optional_int(value, "delay").unwrap_or(30) as u32,
        },
        "turbo" => Output::Turbo {
            guid: text(value, "guid")?.into(),
            flag: flag(value)?,
            repeat: optional_int(value, "repeat").unwrap_or(0) as u32,
            delay_ms: optional_int(value, "delay").unwrap_or(1000) as u32,
        },
        "disabled" => Output::Disabled,
        "hypershift" => Output::Hypershift { flag: flag(value)? },
        "otfs" => Output::Otfs { flag: flag(value)? },
        "mode" => {
            let scope = match text(value, "scope")? {
                "global" => ModeScope::Global,
                "interdevice" => ModeScope::Interdevice,
                "local" => ModeScope::Local,
                other => bail!("Invalid mode scope {other}"),
            };
            let flag = integer(value, "flag")? as u16;
            ensure!(flag < 2, "Invalid mode flag");
            Output::Mode {
                scope,
                flag,
                payload: NotificationPayload::new(value)?,
            }
        }
        "system" => Output::System(match text(value, "id")? {
            "hibernate" => SystemAction::Hibernate,
            "sleep" => SystemAction::Sleep,
            other => bail!("Invalid system action {other}"),
        }),
        "display" => Output::Display(match text(value, "id")? {
            "brightness" => DisplayAction::Brightness(integer(value, "value")? as u8),
            "brightnessDown" => DisplayAction::BrightnessDown(integer(value, "value")? as u8),
            "brightnessUp" => DisplayAction::BrightnessUp(integer(value, "value")? as u8),
            "driverBrightnessDown" => DisplayAction::DriverBrightnessDown,
            "driverBrightnessUp" => DisplayAction::DriverBrightnessUp,
            "driverBrightnessStop" => DisplayAction::DriverBrightnessStop,
            other => bail!("Invalid display action {other}"),
        }),
        "copyText" => Output::CopyText(NotificationPayload::new(object(value, "data")?)?),
        "multi" => {
            let rows = value
                .get("outputs")
                .and_then(Value::as_array)
                .context("Missing multi outputs")?;
            ensure!(!rows.is_empty(), "Empty multi outputs");
            Output::Multi(
                rows.iter()
                    .map(|row| parse(row, Context::Multi, depth + 1))
                    .collect::<Result<_>>()?,
            )
        }
        "cascade" | "dpad" => {
            let rows = value
                .get(kind)
                .and_then(Value::as_array)
                .context("Missing branching mappings")?;
            ensure!(!rows.is_empty(), "Empty branching mappings");
            let context = if kind == "cascade" {
                Context::Cascade
            } else {
                Context::Dpad
            };
            let branches = rows
                .iter()
                .map(|row| {
                    Ok(Branch {
                        input: row
                            .get("input")
                            .filter(|v| v.is_object())
                            .map(parse_input)
                            .transpose()?,
                        output: parse(object(row, "output")?, context, depth + 1)?,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            if kind == "cascade" {
                Output::Cascade(branches)
            } else {
                Output::Dpad(branches)
            }
        }
        "doubleMapping" => Output::DoubleMapping {
            analog: NativeAnalogOutput::from_native_json(object(value, "analog")?)?,
            digital: Box::new(parse(
                object(value, "digital")?,
                Context::Digital,
                depth + 1,
            )?),
        },
        "modtap" => Output::ModTap {
            milliseconds: integer(value, "ms")? as u32,
            tap: Box::new(parse(object(value, "tap")?, Context::ModTap, depth + 1)?),
            hold: Box::new(parse(object(value, "hold")?, Context::ModTap, depth + 1)?),
        },
        "rapidTrigger" => {
            let actuation = integer(value, "actuation")? as u16;
            let make_sensitivity = integer(value, "makeSensitivity")? as u16;
            let break_sensitivity = integer(value, "breakSensitivity")? as u16;
            ensure!(
                make_sensitivity != 0 && break_sensitivity != 0,
                "Zero rapid-trigger sensitivity"
            );
            Output::RapidTrigger {
                actuation,
                make_sensitivity,
                break_sensitivity,
                continuous: value.get("continuous").and_then(Value::as_bool),
                mapping: Box::new(parse(object(value, "mapping")?, Context::Rapid, depth + 1)?),
            }
        }
        "triggers" => {
            let rows = object(value, "outputs")?.as_object().unwrap();
            ensure!(!rows.is_empty(), "Empty triggers");
            let mut triggers = BTreeMap::new();
            for (key, row) in rows {
                let digits = key.strip_prefix('+').unwrap_or(key);
                ensure!(
                    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()),
                    "Invalid trigger ID"
                );
                let id = digits.parse::<u16>().context("Trigger ID exceeds WORD")?;
                let trigger = Trigger {
                    actuation: integer(row, "actuation")? as u16,
                    output: parse(object(row, "mapping")?, Context::Trigger, depth + 1)?,
                };
                ensure!(
                    triggers.insert(id, trigger).is_none(),
                    "Duplicate numeric trigger ID"
                );
            }
            Output::Triggers(triggers)
        }
        _ => unreachable!("The source context admitted an unknown output"),
    })
}

#[cfg(test)]
#[path = "mapping_native_action/tests.rs"]
mod tests;
