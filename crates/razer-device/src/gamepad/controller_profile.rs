//! Current 2636/2676 OBM assignment and polling readers. The caller retains
//! transport, transaction sequence, cancellation and connection ownership.
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Current 2636/2676 controllerDevice 10511.Fk converts a present source slot
/// zero to wire slot one. Callers must separately reject missing source slots.
pub fn wire_slot(observed_source_slot: u8) -> u8 {
    if observed_source_slot == 0 {
        1
    } else {
        observed_source_slot
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DockFirmwareVersion {
    pub major: u8,
    pub minor: u8,
    pub internal: u8,
    pub reserved: u8,
}

/// The 2636 factory identifies a dongle from this command's successful status.
/// Failed transfers do not construct an observation or establish a wired link.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ConnectionProbeObservation {
    pub raw_data: Vec<u8>,
}
impl ConnectionProbeObservation {
    pub fn validate_for(&self, product_id: u32) -> anyhow::Result<()> {
        ensure!(
            product_id == 2636 && self.raw_data.len() <= 80,
            "Controller connection probe belongs to another product or exceeds Protocol25"
        );
        Ok(())
    }

    /// Version bytes are optional. The factory's connection decision only
    /// checks SUCCESS; an empty or short successful reply still proves dongle.
    pub fn firmware_version(&self) -> Option<DockFirmwareVersion> {
        let data = self.raw_data.get(..4)?;
        Some(DockFirmwareVersion {
            major: data[0],
            minor: data[1],
            internal: data[2],
            reserved: data[3],
        })
    }
}

pub fn read_connection_probe(
    product_id: u32,
    exchange: &mut impl FnMut([u8; 3], &[u8]) -> anyhow::Result<Vec<u8>>,
) -> anyhow::Result<ConnectionProbeObservation> {
    ensure!(
        product_id == 2636,
        "No verified controller connection probe for {product_id}"
    );
    // Current 87969 -> 12844.Qp -> 14397.mf sends no request payload. The
    // caller's Feature channel must check transaction/class/id and status=2.
    let observation = ConnectionProbeObservation {
        raw_data: exchange([4, 0, 0x93], &[])?,
    };
    observation.validate_for(product_id)?;
    Ok(observation)
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PollingMethod {
    ProfilePeriod,
    UsbHighSpeed,
}

fn polling_method(product_id: u32) -> anyhow::Result<PollingMethod> {
    match product_id {
        2636 => Ok(PollingMethod::ProfilePeriod),
        2676 => Ok(PollingMethod::UsbHighSpeed),
        _ => anyhow::bail!("No verified controller profile polling reader for {product_id}"),
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PollingObservation {
    pub profile_slot: u8,
    pub method: PollingMethod,
    /// Retain unknown protocol values instead of the source decoder's fallback.
    pub raw_code: u8,
}
impl PollingObservation {
    pub fn hertz(&self) -> Option<u16> {
        match (self.method, self.raw_code) {
            (PollingMethod::ProfilePeriod, 1) => Some(1000),
            (PollingMethod::ProfilePeriod, 2) => Some(500),
            (PollingMethod::ProfilePeriod, 3) => Some(333),
            (PollingMethod::ProfilePeriod, 4) => Some(250),
            (PollingMethod::ProfilePeriod, 5) => Some(200),
            (PollingMethod::ProfilePeriod, 8) => Some(125),
            (PollingMethod::ProfilePeriod, 10) => Some(100),
            (PollingMethod::UsbHighSpeed, 1) => Some(8000),
            (PollingMethod::UsbHighSpeed, 2) => Some(4000),
            (PollingMethod::UsbHighSpeed, 4) => Some(2000),
            (PollingMethod::UsbHighSpeed, 8) => Some(1000),
            (PollingMethod::UsbHighSpeed, 16) => Some(500),
            (PollingMethod::UsbHighSpeed, 32) => Some(250),
            (PollingMethod::UsbHighSpeed, 64) => Some(125),
            _ => None,
        }
    }
    pub fn validate_for(&self, product_id: u32, profile_slot: u8) -> anyhow::Result<()> {
        ensure!(
            self.profile_slot == profile_slot && self.method == polling_method(product_id)?,
            "Controller polling observation belongs to another product branch or slot"
        );
        Ok(())
    }
}

pub fn read_polling(
    product_id: u32,
    profile_slot: u8,
    exchange: &mut impl FnMut([u8; 3], &[u8]) -> anyhow::Result<Vec<u8>>,
) -> anyhow::Result<PollingObservation> {
    // g2 is false for 2636, whose isObm.yes enables profile polling. The
    // 2676 mount explicitly enables isHyperpollingDevice for controllers.
    let method = polling_method(product_id)?;
    let command = match method {
        PollingMethod::ProfilePeriod => 0x8e,
        PollingMethod::UsbHighSpeed => 0xc0,
    };
    let payload = exchange([2, 0, command], &[profile_slot])?;
    ensure!(
        payload.len() == 2 && payload[0] == profile_slot,
        "Controller polling reply is incomplete or belongs to another slot"
    );
    Ok(PollingObservation {
        profile_slot,
        method,
        raw_code: payload[1],
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssignmentFunction {
    Disabled,
    MouseButton,
    KeyboardKey,
    MacroTypeI,
    MacroTypeII,
    MacroTypeIII,
    Dpi,
    Profile,
    Lighting,
    PowerKeys,
    MediaKeys,
    DoubleClick,
    ModeButton,
    TurboKey,
    TurboButton,
    MacroTypeIV,
    Controller,
    RazerKey,
    ScrollWheelMode,
    WindowsShortcut,
    Unknown(u8),
}
impl AssignmentFunction {
    pub fn from_raw(raw: u8) -> Self {
        match raw {
            0 => Self::Disabled,
            1 => Self::MouseButton,
            2 => Self::KeyboardKey,
            3 => Self::MacroTypeI,
            4 => Self::MacroTypeII,
            5 => Self::MacroTypeIII,
            6 => Self::Dpi,
            7 => Self::Profile,
            8 => Self::Lighting,
            9 => Self::PowerKeys,
            10 => Self::MediaKeys,
            11 => Self::DoubleClick,
            12 => Self::ModeButton,
            13 => Self::TurboKey,
            14 => Self::TurboButton,
            15 => Self::MacroTypeIV,
            16 => Self::Controller,
            17 => Self::RazerKey,
            18 => Self::ScrollWheelMode,
            21 => Self::WindowsShortcut,
            raw => Self::Unknown(raw),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ButtonAssignment {
    pub profile_slot: u8,
    pub button_id: u8,
    pub mode: u8,
    pub function_id: u8,
    pub data_size: u8,
    /// The current hn/An decoders retain exactly bytes [5, 10).
    pub data: [u8; 5],
}
impl ButtonAssignment {
    pub fn function(&self) -> AssignmentFunction {
        AssignmentFunction::from_raw(self.function_id)
    }
    /// This is the source engine's omission rule, not evidence of a missing
    /// read. The complete observed row remains retained for later consumers.
    pub fn source_omits_assignment(&self) -> bool {
        self.function() == AssignmentFunction::Controller
            && (self.data[0] == self.button_id || self.data[0] == 0)
    }
    pub fn function_data(&self) -> anyhow::Result<&[u8]> {
        self.data
            .get(..usize::from(self.data_size))
            .context("Controller assignment declares data beyond its five-byte source decoder")
    }
    pub fn validate_for(&self, profile_slot: u8, button_id: u8) -> anyhow::Result<()> {
        ensure!(
            self.profile_slot == profile_slot && self.button_id == button_id && self.mode == 0,
            "Controller assignment belongs to another slot, button or mapping mode"
        );
        Ok(())
    }
    fn parse(profile_slot: u8, button_id: u8, payload: &[u8]) -> anyhow::Result<Self> {
        ensure!(
            (10..=80).contains(&payload.len()),
            "Controller assignment reply is incomplete or oversized"
        );
        let assignment = Self {
            profile_slot: payload[0],
            button_id: payload[1],
            mode: payload[2],
            function_id: payload[3],
            data_size: payload[4],
            data: payload[5..10].try_into()?,
        };
        assignment.validate_for(profile_slot, button_id)?;
        Ok(assignment)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum AssignmentRead {
    Observed { assignment: ButtonAssignment },
    Failed { error: String },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ButtonRead {
    pub button_id: u8,
    pub result: AssignmentRead,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MappingObservation {
    pub profile_slot: u8,
    pub buttons: Vec<ButtonRead>,
    pub canceled: bool,
}
impl MappingObservation {
    pub fn validate_for(&self, profile_slot: u8, button_ids: &[u8]) -> anyhow::Result<()> {
        validate_buttons(button_ids)?;
        ensure!(
            self.profile_slot == profile_slot,
            "Controller mappings belong to another slot"
        );
        ensure!(
            self.buttons.len() <= button_ids.len()
                && self
                    .buttons
                    .iter()
                    .map(|button| button.button_id)
                    .eq(button_ids[..self.buttons.len()].iter().copied())
                && (self.canceled || self.buttons.len() == button_ids.len()),
            "Controller mapping reads differ from the real button order or omit an uncanceled read"
        );
        for button in &self.buttons {
            match &button.result {
                AssignmentRead::Observed { assignment } => {
                    assignment.validate_for(profile_slot, button.button_id)?
                }
                AssignmentRead::Failed { error } => ensure!(
                    !error.is_empty(),
                    "Controller mapping failure omits its error"
                ),
            }
        }
        Ok(())
    }
}
fn validate_buttons(button_ids: &[u8]) -> anyhow::Result<()> {
    ensure!(
        !button_ids.is_empty()
            && button_ids.len() <= 79
            && !button_ids.contains(&0)
            && button_ids.iter().copied().collect::<BTreeSet<_>>().len() == button_ids.len(),
        "Controller mapping directory is empty, oversized or has duplicate/zero button IDs"
    );
    Ok(())
}

pub fn read_mappings(
    product_id: u32,
    profile_slot: u8,
    button_ids: &[u8],
    exchange: &mut impl FnMut([u8; 3], &[u8]) -> anyhow::Result<Vec<u8>>,
    canceled: &mut impl FnMut() -> bool,
) -> anyhow::Result<MappingObservation> {
    ensure!(
        matches!(product_id, 2636 | 2676),
        "No verified controller mapping reader for {product_id}"
    );
    validate_buttons(button_ids)?;
    let mut observation = MappingObservation {
        profile_slot,
        buttons: Vec::new(),
        canceled: false,
    };
    for button_id in button_ids {
        if canceled() {
            observation.canceled = true;
            break;
        }
        let result = match exchange([80, 2, 0x8c], &[profile_slot, *button_id, 0])
            .and_then(|payload| ButtonAssignment::parse(profile_slot, *button_id, &payload))
        {
            Ok(assignment) => AssignmentRead::Observed { assignment },
            Err(error) if crate::controller_feature::terminal_io_failure(&error) => {
                return Err(error);
            }
            Err(error) => AssignmentRead::Failed {
                error: format!("{error:#}"),
            },
        };
        observation.buttons.push(ButtonRead {
            button_id: *button_id,
            result,
        });
    }
    observation.canceled |= canceled();
    observation.validate_for(profile_slot, button_ids)?;
    Ok(observation)
}

#[cfg(test)]
#[path = "controller_profile_tests.rs"]
mod tests;
