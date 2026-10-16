//! Current local scalar analog tasks, including observed OBM GUID slot selection.
//! Evidence: 742 MW module-34737::{j8,TB}, module-38657::getProfileNameStructure,
//! module-74503::{getProfileIDList,getProfileName}, module-48967::PeF.
use crate::{
    backend::{FeatureTransport, HidNode},
    device_reads::DeviceReadTarget,
    feature_exchange,
    keyboard_analog::{self, KeyboardAnalogChange, KeyboardAnalogTransportCapability},
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum Intent {
    ContinuousRapidTrigger { enabled: bool },
    BottomDeadzone { value: String },
    CustomGamingCombinations { settings: Value },
    SnapTap { settings: Value },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Target {
    Collection { node: HidNode, product_id: u32 },
    WindowsContainer(DeviceReadTarget),
}

impl Target {
    pub fn product_id(&self) -> u32 {
        match self {
            Self::Collection { product_id, .. } => *product_id,
            Self::WindowsContainer(target) => target.product_id,
        }
    }
}

/// Exact caller payload parsing; profile matching requires actual device inventory.
pub fn intent(product_id: u32, message: &Value) -> anyhow::Result<Intent> {
    let payload = &message["payload"];
    match message["type"]
        .as_str()
        .context("Keyboard intent type is missing")?
    {
        "ON_TOGGLE_CONTINUOUS_RAPID_TRIGGER" => {
            let enabled = payload["value"]
                .as_bool()
                .context("Continuous rapid trigger value is missing")?;
            validate(product_id, &Intent::ContinuousRapidTrigger { enabled })?;
            Ok(Intent::ContinuousRapidTrigger { enabled })
        }
        "ON_SET_BOTTOM_DEADZONE" => {
            let value = payload["bottomDeadzone"]["value"]
                .as_str()
                .context("Bottom deadzone value is missing")?
                .to_owned();
            let intent = Intent::BottomDeadzone { value };
            validate(product_id, &intent)?;
            Ok(intent)
        }
        "ON_SET_GAME_MODE_COMBINATION_KEYS" => {
            let intent = Intent::CustomGamingCombinations {
                settings: payload.clone(),
            };
            validate(product_id, &intent)?;
            Ok(intent)
        }
        "ON_SET_SNAP_TAP" => {
            let intent = Intent::SnapTap {
                settings: snap::local_settings(payload)?,
            };
            validate(product_id, &intent)?;
            Ok(intent)
        }
        event => anyhow::bail!("Keyboard task {event} requires its mapping/OBM submission chain"),
    }
}

pub fn validate(
    product_id: u32,
    intent: &Intent,
) -> anyhow::Result<KeyboardAnalogTransportCapability> {
    let cap = keyboard_analog::transport_capability(product_id)
        .context("Keyboard analog transport has not been captured")?;
    match intent {
        Intent::ContinuousRapidTrigger { .. } => ensure!(
            cap.analog_v3,
            "Product has no continuous rapid trigger branch"
        ),
        Intent::BottomDeadzone { .. } => ensure!(
            [742, 756].contains(&product_id),
            "Product has no captured bottom deadzone caller"
        ),
        Intent::CustomGamingCombinations { settings } => {
            ensure!(
                [742, 756].contains(&product_id),
                "Product has no captured custom gaming combinations task"
            );
            gaming::custom_keys(product_id, settings)?;
        }
        Intent::SnapTap { settings } => {
            snap::validate(product_id, &cap, settings)?;
        }
    }
    Ok(cap)
}

#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
pub struct ProfileIdentity {
    pub profile_id: u8,
    pub guid: String,
    pub raw_metadata: Vec<u8>,
    pub metadata_attempts: u8,
}

#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
pub struct Submission {
    pub profile_id: u8,
    pub command: [u8; 3],
    pub payload: Vec<u8>,
    pub attempted: bool,
    pub acknowledgement: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
pub struct Receipt {
    pub intent: Intent,
    pub profile_guid: String,
    pub inventory: Vec<ProfileIdentity>,
    pub matched_slots: Vec<u8>,
    pub submissions: Vec<Submission>,
    pub completed: bool,
}

#[derive(Debug)]
#[non_exhaustive]
pub struct Failure {
    pub receipt: Receipt,
    pub error: anyhow::Error,
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Keyboard submission: {:#}; progress: {}",
            self.error,
            serde_json::to_string(&self.receipt).unwrap_or_default()
        )
    }
}
impl std::error::Error for Failure {}

fn guid_text(bytes: &[u8]) -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        u32::from_le_bytes(bytes[..4].try_into().unwrap()),
        u16::from_le_bytes(bytes[4..6].try_into().unwrap()),
        u16::from_le_bytes(bytes[6..8].try_into().unwrap()),
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

fn inventory(
    device: &dyn FeatureTransport,
    cap: &KeyboardAnalogTransportCapability,
    next: &mut impl FnMut() -> anyhow::Result<u8>,
    guard: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Vec<ProfileIdentity>> {
    let transport = cap.device_read_capability();
    let list = feature_exchange::exchange(device, &transport, [80, 5, 129], &[], next()?, guard)?;
    let count = usize::from(*list.first().context("Profile ID list is empty")?);
    ensure!(
        count <= usize::from(cap.hardware_profile_slots) && list.len() > count,
        "Profile ID list is truncated or exceeds the captured inventory"
    );
    let ids = &list[1..1 + count];
    ensure!(
        ids.iter().all(|id| *id > 0)
            && ids.iter().collect::<std::collections::BTreeSet<_>>().len() == ids.len(),
        "Profile IDs are invalid or repeated"
    );
    let mut metadata_transport = transport;
    metadata_transport.sleep_between_out_in_ms *= 2;
    metadata_transport.sleep_between_in_ms = metadata_transport
        .sleep_between_in_ms
        .max(metadata_transport.sleep_between_out_in_ms);
    let mut profiles = Vec::new();
    for &profile_id in ids {
        let mut metadata = Vec::new();
        let mut attempts = 0;
        let mut read_error = None;
        for _ in 0..10 {
            guard()?;
            attempts += 1;
            metadata.clear();
            read_error = None;
            for offset in (0u16..250).step_by(64) {
                let bytes = usize::from((250 - offset).min(64));
                let offset_bytes = offset.to_be_bytes();
                let chunk = (|| -> anyhow::Result<Vec<u8>> {
                    let data = feature_exchange::exchange_with_report_data_size(
                        device,
                        &metadata_transport,
                        [80, 5, 136],
                        &[profile_id, offset_bytes[0], offset_bytes[1], 0, 0],
                        next()?,
                        (bytes + 5) as u8,
                        guard,
                    )?;
                    ensure!(
                        data.len() >= bytes + 5
                            && data[0] == profile_id
                            && data[1..3] == offset_bytes,
                        "Profile metadata response is truncated or belongs to another slot/offset"
                    );
                    Ok(data[5..5 + bytes].to_vec())
                })();
                match chunk {
                    Ok(chunk) => metadata.extend_from_slice(&chunk),
                    Err(error) => {
                        guard()?;
                        read_error = Some(format!("{error:#}"));
                        metadata.clear();
                        break;
                    }
                }
            }
            if metadata.len() == 250 {
                let guid = guid_text(&metadata[..16]);
                if !guid.contains("ffff") && !guid.contains("0000") {
                    break;
                }
            }
        }
        // The source retains an invalid GUID after ten retries. Preserve those
        // actual bytes, so an absent match cannot be replaced by a local index.
        ensure!(
            metadata.len() == 250,
            "Profile metadata failed after {attempts} attempts: {}",
            read_error.unwrap_or_default()
        );
        profiles.push(ProfileIdentity {
            profile_id,
            guid: guid_text(&metadata[..16]),
            raw_metadata: metadata,
            metadata_attempts: attempts,
        });
    }
    Ok(profiles)
}

/// Setter ACKs satisfy these source tasks. No synthetic getter-to-UI update occurs.
pub fn submit(
    device: &dyn FeatureTransport,
    product_id: u32,
    profile_guid: &str,
    intent: Intent,
    mut next: impl FnMut() -> anyhow::Result<u8>,
    guard: impl Fn() -> anyhow::Result<()>,
) -> Result<Receipt, Failure> {
    let mut receipt = Receipt {
        intent,
        profile_guid: profile_guid.into(),
        inventory: Vec::new(),
        matched_slots: Vec::new(),
        submissions: Vec::new(),
        completed: false,
    };
    let result = (|| -> anyhow::Result<()> {
        let cap = validate(product_id, &receipt.intent)?;
        ensure!(
            profile_guid.len() == 36
                && profile_guid.bytes().enumerate().all(|(ix, byte)| {
                    if [8, 13, 18, 23].contains(&ix) {
                        byte == b'-'
                    } else {
                        byte.is_ascii_hexdigit()
                    }
                }),
            "Active profile GUID is invalid"
        );
        guard()?;
        receipt.inventory = inventory(device, &cap, &mut next, &guard)?;
        receipt.matched_slots = receipt
            .inventory
            .iter()
            .filter(|profile| profile.guid == profile_guid)
            .map(|profile| profile.profile_id)
            .collect();
        if let Intent::SnapTap { settings } = &receipt.intent {
            let settings = settings.clone();
            snap::submit(
                device,
                product_id,
                &cap,
                &settings,
                &mut receipt,
                &mut next,
                &guard,
            )?;
            guard()?;
            receipt.completed = true;
            return Ok(());
        }
        if let Intent::CustomGamingCombinations { settings } = &receipt.intent {
            let settings = settings.clone();
            gaming::submit(
                device,
                product_id,
                &cap,
                &settings,
                &mut receipt,
                &mut next,
                &guard,
            )?;
            guard()?;
            receipt.completed = true;
            return Ok(());
        }
        let slots = std::iter::once(1)
            .chain(receipt.matched_slots.iter().copied())
            .collect::<Vec<_>>();
        for profile_id in slots {
            guard()?;
            match &receipt.intent {
                Intent::ContinuousRapidTrigger { enabled } => {
                    receipt.submissions.push(Submission {
                        profile_id,
                        command: [2, 2, 32],
                        payload: vec![profile_id, u8::from(*enabled)],
                        attempted: false,
                        acknowledgement: None,
                    });
                    let outcome = keyboard_analog::write(
                        device,
                        &cap,
                        KeyboardAnalogChange::ContinuousRapidTrigger {
                            profile_id,
                            enabled: *enabled,
                        },
                        false,
                        &mut next,
                        &guard,
                    );
                    let step = receipt.submissions.last_mut().unwrap();
                    match outcome {
                        Ok(report) => {
                            step.attempted = report.attempted_writes > 0;
                            step.acknowledgement = report
                                .acknowledgements
                                .into_iter()
                                .next()
                                .map(|ack| ack.data);
                        }
                        Err(error) => {
                            step.attempted = error.report.attempted_writes > 0;
                            step.acknowledgement = error
                                .report
                                .acknowledgements
                                .first()
                                .map(|ack| ack.data.clone());
                            return Err(error.into());
                        }
                    }
                }
                Intent::BottomDeadzone { value } => {
                    let raw = match value.to_lowercase().as_str() {
                        "responsive" => 10,
                        "stable" => 14,
                        _ => 12,
                    };
                    receipt.submissions.push(Submission {
                        profile_id,
                        command: [2, 2, 40],
                        payload: vec![profile_id, raw],
                        attempted: false,
                        acknowledgement: None,
                    });
                    let step = receipt.submissions.last_mut().unwrap();
                    let transaction = next()?;
                    step.acknowledgement = Some(
                        feature_exchange::exchange_with_report_data_size_observing_send(
                            device,
                            &cap.device_read_capability(),
                            step.command,
                            &step.payload,
                            transaction,
                            2,
                            &guard,
                            &mut || step.attempted = true,
                        )?,
                    );
                }
                Intent::CustomGamingCombinations { .. } | Intent::SnapTap { .. } => {
                    unreachable!("keyboard task has an independent slot policy")
                }
            }
        }
        guard()?;
        receipt.completed = true;
        Ok(())
    })();
    match result {
        Ok(()) => Ok(receipt),
        Err(error) => Err(Failure { receipt, error }),
    }
}

/// Current profile reducer values are persisted before the hardware task.
pub fn persist_local(message: &Value, profile: &mut Value) -> anyhow::Result<()> {
    let payload = &message["payload"];
    match message["type"]
        .as_str()
        .context("Keyboard local intent type is missing")?
    {
        "ON_TOGGLE_CONTINUOUS_RAPID_TRIGGER" => {
            profile["continuousRapidTrigger"] = payload["value"].clone()
        }
        "ON_SET_BOTTOM_DEADZONE" => profile["bottomDeadzone"] = payload["bottomDeadzone"].clone(),
        "ON_SET_GAME_MODE_COMBINATION_KEYS" => {
            if !profile["gamingMode"].is_object() {
                profile["gamingMode"] = serde_json::json!({});
            }
            profile["gamingMode"]["customGameMode"] = payload.clone();
        }
        "ON_SET_SNAP_TAP" => profile["snapTap"] = snap::local_settings(payload)?,
        event => {
            anyhow::bail!("Keyboard local intent {event} requires its captured profile reducer")
        }
    }
    Ok(())
}

#[path = "keyboard_gaming_submission.rs"]
mod gaming;

#[path = "keyboard_snap_submission.rs"]
mod snap;

#[cfg(test)]
#[path = "keyboard_submission_tests.rs"]
mod tests;
