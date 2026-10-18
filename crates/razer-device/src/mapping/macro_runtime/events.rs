use anyhow::{Context, Result, bail, ensure};
use serde_json::{Map, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MovementMode {
    None,
    Absolute,
    Foreground,
    Relative,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keyboard {
    pub scancode: u16,
    pub flag: u16,
    pub modifiers: u32,
    pub unit: Option<u16>,
    pub delay_ms: Option<u32>,
    pub release_after_ms: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mouse {
    pub id: u16,
    pub data: u16,
    pub modifiers: u32,
    pub unit: Option<u16>,
}

impl Mouse {
    pub fn input(&self) -> crate::mapping_native_input::MouseInput {
        crate::mapping_native_input::MouseInput {
            unit_id: self.unit.unwrap_or(0),
            id: self.id,
            data: self.data,
            modifiers: self.modifiers,
            ..Default::default()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioTarget {
    All,
    Microphone,
    Speaker,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioAction {
    Set {
        target: AudioTarget,
        volume: Option<u8>,
        mute: Option<u8>,
    },
    Step {
        target: AudioTarget,
        increase: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Launch {
    pub path: String,
    pub path_to_check: String,
    pub hidden: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Keyboard(Keyboard),
    Mouse(Mouse),
    MouseMove { x: i32, y: i32 },
    Delay { milliseconds: u32 },
    RandomDelay { min_ms: u32, max_ms: u32 },
    Clipboard(String),
    Launch(Launch),
    Audio(AudioAction),
    LoopStart { repeat: u32 },
    LoopEnd,
    Macro { guid: String, repeat: u32 },
}

pub(super) fn movement_mode(value: Option<&Value>) -> Result<MovementMode> {
    // f7106's optional string read leaves its source enum at zero when absent
    // or non-string. A present unknown string is a publication failure.
    match value.and_then(Value::as_str) {
        None | Some("none") => Ok(MovementMode::None),
        Some("absolute") => Ok(MovementMode::Absolute),
        Some("foreground") => Ok(MovementMode::Foreground),
        Some("relative") => Ok(MovementMode::Relative),
        Some(other) => bail!("Unknown macro mouseMoveType {other}"),
    }
}

// Native DB930 reads the Chromium int32 variant; DB873 then truncates to WORD.
// Optional readers ignore a failed read and retain the source constructor value.
fn optional_integer(row: &Map<String, Value>, key: &str) -> Option<i32> {
    row.get(key)?.as_i64()?.try_into().ok()
}
fn integer(row: &Map<String, Value>, key: &str) -> Result<i32> {
    optional_integer(row, key).with_context(|| format!("Missing int32 macro field {key}"))
}
fn text<'a>(row: &'a Map<String, Value>, key: &str) -> Result<&'a str> {
    row.get(key)
        .and_then(Value::as_str)
        .with_context(|| format!("Missing macro string {key}"))
}

pub(super) fn parse(value: &Value) -> Result<Event> {
    let row = value.as_object().context("Macro event is not an object")?;
    Ok(match text(row, "type")? {
        "keyboard" => Event::Keyboard(Keyboard {
            scancode: integer(row, "scancode")? as u16,
            flag: integer(row, "flag")? as u16,
            modifiers: optional_integer(row, "modifiers").unwrap_or(0) as u32,
            unit: optional_integer(row, "unitId").map(|value| value as u16),
            delay_ms: optional_integer(row, "delayInMs").map(|value| value as u32),
            release_after_ms: optional_integer(row, "releaseAfterMs").map(|value| value as u32),
        }),
        "mouse" => {
            let mouse = Mouse {
                id: integer(row, "id")? as u16,
                data: optional_integer(row, "data").unwrap_or(0) as u16,
                modifiers: optional_integer(row, "modifiers").unwrap_or(0) as u32,
                unit: optional_integer(row, "unitId").map(|value| value as u16),
            };
            ensure!(
                mouse.input().mapping_id_valid(),
                "Invalid macro mouse id/data"
            );
            Event::Mouse(mouse)
        }
        "mouseMove" => Event::MouseMove {
            x: integer(row, "x")?,
            y: integer(row, "y")?,
        },
        "delay" => Event::Delay {
            milliseconds: integer(row, "ms")? as u32,
        },
        "randomDelay" => Event::RandomDelay {
            min_ms: integer(row, "minMs")? as u32,
            max_ms: integer(row, "maxMs")? as u32,
        },
        "clipboard" => {
            ensure!(text(row, "id")? == "text", "Invalid macro clipboard id");
            Event::Clipboard(text(row, "text")?.into())
        }
        "launch" => Event::Launch(Launch {
            path: text(row, "path")?.into(),
            path_to_check: row
                .get("pathToCheck")
                .and_then(Value::as_str)
                .unwrap_or("")
                .into(),
            hidden: row
                .get("startHidden")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        }),
        "macro" => {
            let guid = text(row, "guid")?;
            ensure!(!guid.is_empty(), "Empty nested macro GUID");
            ensure!(
                matches!(integer(row, "flag")?, 0 | 1),
                "Invalid nested macro flag"
            );
            Event::Macro {
                guid: guid.into(),
                repeat: optional_integer(row, "repeat").unwrap_or(1) as u32,
            }
        }
        "loop" => {
            let flag = integer(row, "flag")?;
            ensure!(matches!(flag, 0 | 1), "Invalid macro loop flag");
            let repeat = if flag == 0 {
                integer(row, "repeat")?
            } else {
                optional_integer(row, "repeat").unwrap_or(1)
            } as u32;
            ensure!(repeat != 0, "Macro loop repeat is zero");
            if flag == 0 {
                Event::LoopStart { repeat }
            } else {
                Event::LoopEnd
            }
        }
        "audio" => Event::Audio(audio(row)?),
        kind => bail!("Invalid macro event type {kind}"),
    })
}

fn audio(row: &Map<String, Value>) -> Result<AudioAction> {
    let (target, step) = match text(row, "id")? {
        "all" => (AudioTarget::All, None),
        "mic" => (AudioTarget::Microphone, None),
        "speaker" => (AudioTarget::Speaker, None),
        "micVolumeDown" => (AudioTarget::Microphone, Some(false)),
        "micVolumeUp" => (AudioTarget::Microphone, Some(true)),
        "speakerVolumeDown" => (AudioTarget::Speaker, Some(false)),
        "speakerVolumeUp" => (AudioTarget::Speaker, Some(true)),
        other => bail!("Invalid macro audio id {other}"),
    };
    if let Some(increase) = step {
        return Ok(AudioAction::Step { target, increase });
    }
    let volume = optional_integer(row, "volume").map(|value| value as u32);
    let mute = optional_integer(row, "mute").map(|value| value as u32);
    ensure!(
        volume.is_some() || mute.is_some(),
        "Macro audio has no volume or mute"
    );
    ensure!(
        volume.is_none_or(|value| value <= 100),
        "Invalid macro audio volume"
    );
    ensure!(
        mute.is_none_or(|value| value < 3),
        "Invalid macro audio mute"
    );
    Ok(AudioAction::Set {
        target,
        volume: volume.map(|value| value as u8),
        mute: mute.map(|value| value as u8),
    })
}
