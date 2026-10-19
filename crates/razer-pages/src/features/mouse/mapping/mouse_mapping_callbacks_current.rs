//! Current unsupported-mapping boundary and DPI decisions shared by mouse owners.
//! MW60168 dispatches callbacks, MW83789 owns held clutch/OTFS state, and
//! MW30525 ut/vt/ft/yt own stage, value and temporary DPI operations.
use anyhow::{Context as _, Result, ensure};
use razer_device::{mouse_current_dpi::DpiPair, mouse_dpi_stages::DpiStagesDraft};
use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Connection {
    pub(super) container: String,
    pub(super) generation: u64,
}

#[derive(Default)]
pub(super) struct TemporaryRestoration {
    connection: Option<Connection>,
    required: bool,
    ticket: u64,
}

impl TemporaryRestoration {
    pub(super) fn observe_connection(&mut self, connection: Option<Connection>) {
        if self.connection != connection {
            self.connection = connection;
            self.required = false;
            self.advance();
        }
    }

    pub(super) fn require(&mut self) {
        self.advance();
        self.required = true;
    }

    pub(super) fn required(&self) -> bool {
        self.required
    }

    pub(super) fn ticket(&self) -> u64 {
        self.ticket
    }

    pub(super) fn confirm(&mut self, ticket: u64) {
        if ticket == self.ticket {
            self.required = false;
        }
    }

    fn advance(&mut self) {
        self.ticket = self
            .ticket
            .checked_add(1)
            .expect("temporary DPI restoration identity exhausted");
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub(super) enum ScanCode {
    Number(u32),
    Text(String),
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub(super) enum StageAction {
    #[serde(rename = "DPI_Up")]
    Up,
    #[serde(rename = "DPI_Down")]
    Down,
    #[serde(rename = "DPI_CycleUp")]
    CycleUp,
    #[serde(rename = "DPI_CycleDown")]
    CycleDown,
}

impl StageAction {
    pub(super) fn assignment(self) -> &'static str {
        match self {
            Self::Up => "DPI_Up",
            Self::Down => "DPI_Down",
            Self::CycleUp => "DPI_CycleUp",
            Self::CycleDown => "DPI_CycleDown",
        }
    }
}

#[derive(Debug)]
pub(super) enum Callback {
    Mode(bool),
    Stage(StageAction),
    ClutchDown {
        key: ScanCode,
        dpi: DpiPair,
    },
    ClutchUp(ScanCode),
    Otfs {
        enabled: bool,
        interdevice: bool,
    },
    Value {
        up: bool,
    },
    Navigate {
        output: Value,
        lightpac: Option<Value>,
    },
    Forward(Value),
    Ignored,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Envelope {
    source: Option<String>,
    event_type: Option<u32>,
    event: Option<Encoded<Mode>>,
    json_event: Option<Value>,
    time_tick: Option<u64>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Encoded<T> {
    Text(String),
    Object(T),
}

impl<T: serde::de::DeserializeOwned> Encoded<T> {
    fn decode(self) -> Result<T> {
        match self {
            Self::Text(text) => Ok(serde_json::from_str(&text)?),
            Self::Object(value) => Ok(value),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Mode {
    is_enabled: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawCallback {
    input: String,
    output: String,
    #[serde(default)]
    event_name: Option<String>,
}

#[derive(Deserialize)]
struct OutputHeader {
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Sensitivity {
    sensitivity_group: SensitivityGroup,
    flag: u8,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SensitivityGroup {
    sensitivity_assignment: String,
    x: Option<u16>,
    y: Option<u16>,
}

#[derive(Deserialize)]
struct Input {
    scancode: ScanCode,
}

pub(super) fn parse(event: Value, min_dpi: u16, max_dpi: u16) -> Result<Callback> {
    let envelope: Envelope =
        serde_json::from_value(event).context("Invalid mapping callback envelope")?;
    if envelope.source.as_deref() == Some("devicemodechanged") {
        return if envelope.event_type == Some(4) {
            Ok(Callback::Mode(
                envelope
                    .event
                    .context("Missing mode callback value")?
                    .decode()?
                    .is_enabled,
            ))
        } else {
            Ok(Callback::Ignored)
        };
    }
    let Some(mut raw) = envelope.json_event else {
        return Ok(Callback::Ignored);
    };
    let header: OutputHeader = serde_json::from_value(raw.clone())?;
    if header.kind != "unsupportedmapping" {
        return Ok(Callback::Ignored);
    }
    if let Some(tick) = envelope.time_tick {
        raw.as_object_mut()
            .context("Mapping callback is not an object")?
            .insert("timerTick".into(), Value::from(tick));
    }
    parse_raw(raw, min_dpi, max_dpi)
}

pub(super) fn parse_raw(raw: Value, min_dpi: u16, max_dpi: u16) -> Result<Callback> {
    let callback: RawCallback =
        serde_json::from_value(raw.clone()).context("Invalid unsupported mapping callback")?;
    let input: Value =
        serde_json::from_str(&callback.input).context("Invalid mapping callback input")?;
    let output: Value =
        serde_json::from_str(&callback.output).context("Invalid mapping callback output")?;
    let header: OutputHeader = serde_json::from_value(output.clone())?;
    match header.kind.as_str() {
        "interdevice" | "lightpac" => Ok(Callback::Forward(raw)),
        "switchProfile" | "navigateProfile" => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Navigation {
                #[serde(rename = "lightpacId", default, deserialize_with = "linked_chroma")]
                lightpac_present: bool,
            }
            fn linked_chroma<'de, D: serde::Deserializer<'de>>(
                deserializer: D,
            ) -> std::result::Result<bool, D::Error> {
                Option::<String>::deserialize(deserializer)?;
                Ok(true)
            }
            let navigation: Navigation = serde_json::from_value(output.clone())?;
            Ok(Callback::Navigate {
                output,
                lightpac: (callback.event_name.as_deref() != Some("interDevice")
                    && header.kind == "switchProfile"
                    && navigation.lightpac_present)
                    .then_some(raw),
            })
        }
        "sensitivity" => {
            let sensitivity: Sensitivity = serde_json::from_value(output)?;
            ensure!(sensitivity.flag <= 1, "Unknown sensitivity callback flag");
            let group = sensitivity.sensitivity_group;
            let enabled = sensitivity.flag == 0;
            match group.sensitivity_assignment.as_str() {
                "DPI_Up" | "DPI_Down" | "DPI_CycleUp" | "DPI_CycleDown" => Ok(Callback::Stage(
                    serde_json::from_value(Value::String(group.sensitivity_assignment))?,
                )),
                "DPI_Clutch" => {
                    let input: Input = serde_json::from_value(input)
                        .context("Clutch callback has no valid input identity")?;
                    if !enabled {
                        return Ok(Callback::ClutchUp(input.scancode));
                    }
                    let dpi = DpiPair {
                        x: group.x.context("Clutch callback has no X DPI")?,
                        y: group.y.context("Clutch callback has no Y DPI")?,
                    };
                    ensure!(
                        (min_dpi..=max_dpi).contains(&dpi.x)
                            && (min_dpi..=max_dpi).contains(&dpi.y),
                        "Clutch DPI is outside the product range"
                    );
                    Ok(Callback::ClutchDown {
                        key: input.scancode,
                        dpi,
                    })
                }
                "DPI_OnTheFly" => Ok(Callback::Otfs {
                    enabled,
                    interdevice: callback.event_name.as_deref() == Some("interDevice"),
                }),
                "DPI_Value_Up" | "DPI_Value_Down" => Ok(Callback::Value {
                    up: group.sensitivity_assignment == "DPI_Value_Up",
                }),
                _ => Ok(Callback::Ignored),
            }
        }
        _ => Ok(Callback::Ignored),
    }
}

pub(super) fn next_stage(profile: &DpiStagesDraft, action: StageAction) -> Option<usize> {
    profile.stages.get(profile.active_stage.checked_sub(1)?)?;
    if !profile.enabled {
        return Some(profile.active_stage);
    }
    let visible: Vec<_> = profile
        .stages
        .iter()
        .enumerate()
        .filter(|(_, stage)| stage.visible)
        .map(|(index, _)| index + 1)
        .collect();
    let active = profile.active_stage;
    Some(
        match action {
            StageAction::Up => visible.iter().copied().find(|index| *index > active),
            StageAction::Down => visible.iter().rev().copied().find(|index| *index < active),
            StageAction::CycleUp => visible
                .iter()
                .copied()
                .find(|index| *index > active)
                .or_else(|| visible.first().copied()),
            StageAction::CycleDown => visible
                .iter()
                .rev()
                .copied()
                .find(|index| *index < active)
                .or_else(|| visible.last().copied()),
        }
        .unwrap_or(active),
    )
}

pub(super) fn otfs_value(value: u16, up: bool, step: u16, min: u16, max: u16) -> u16 {
    if up {
        value.saturating_add(step)
    } else {
        value.saturating_sub(step)
    }
    .clamp(min, max)
}

#[derive(Default)]
pub(super) struct DpiRuntime {
    pub(super) native_otfs: Option<bool>,
    pub(super) inter_otfs_requested: bool,
    pub(super) inter_otfs_applied: bool,
    clutches: Vec<(ScanCode, DpiPair)>,
}

pub(super) enum ClutchRelease {
    Unchanged,
    Temporary(DpiPair),
    Profile,
}

impl DpiRuntime {
    pub(super) fn otfs(&self) -> bool {
        self.native_otfs == Some(true) || self.inter_otfs_applied
    }

    pub(super) fn clutched(&self) -> bool {
        !self.clutches.is_empty()
    }

    pub(super) fn push_clutch(&mut self, key: ScanCode, dpi: DpiPair) {
        self.clutches.push((key, dpi));
    }

    pub(super) fn release_clutch(&mut self, key: &ScanCode) -> ClutchRelease {
        if self.clutches.is_empty() {
            return ClutchRelease::Unchanged;
        }
        let index = self.clutches.iter().position(|(held, _)| held == key);
        let restore = self.clutches.len() == 1 || index == Some(self.clutches.len() - 1);
        // Current MW83789 uses splice(-1, 1) for an unmatched nonfinal release.
        self.clutches
            .remove(index.unwrap_or(self.clutches.len() - 1));
        if !restore {
            return ClutchRelease::Unchanged;
        }
        self.clutches
            .last()
            .map_or(ClutchRelease::Profile, |(_, dpi)| {
                ClutchRelease::Temporary(*dpi)
            })
    }

    pub(super) fn clear_clutches(&mut self) {
        self.clutches.clear();
    }
}
