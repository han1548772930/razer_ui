//! Current six-product RollerMappingFeature (12211), not the legacy mapping
//! task runner. Device-global optimistic storage belongs to the caller. These
//! receipts distinguish the source bool, skipped mappings, and real HID ACKs.
use crate::thx_blackshark_firmware::{
    BlackSharkFirmwareCommand, BlackSharkFirmwareCommandProgress, BlackSharkFirmwareFailureKind,
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RollerMappingProgress {
    pub commands: Vec<BlackSharkFirmwareCommandProgress>,
    /// Present only after a current, successful getter. Unknown wire values
    /// remain visible here; the source presents those as its default empty list.
    pub observed_index: Option<u8>,
    pub observed_mappings: Option<Vec<Value>>,
    /// Original getMappings payload, including [] after a nonexceptional
    /// FAIL/unknown enum. Never an assertion that a default value was read.
    pub source_mappings: Option<Vec<Value>>,
    /// Source setMappings bool. True with zero commands does NOT prove a write.
    pub source_result: Option<bool>,
    pub ignored_mapping_indices: Vec<usize>,
    pub rejected_mappings: Vec<String>,
    pub error: Option<String>,
}

pub fn validate_mappings(mappings: &[Value]) -> Result<()> {
    ensure!(mappings.len() <= 256, "Roller mapping request too large");
    ensure!(
        serde_json::to_vec(mappings)?.len() <= 512 * 1024,
        "Roller mapping payload too large"
    );
    Ok(())
}

fn index(assignment: &str) -> Option<u8> {
    match assignment {
        "GameChatBalance" => Some(0),
        "SidetoneLevel" => Some(1),
        "FootstepScaling" => Some(2),
        "BluetoothVolume" => Some(3),
        "NoFunction" => Some(47),
        _ => None,
    }
}

pub fn mappings_from_index(index: u8) -> Vec<Value> {
    let assignment = match index {
        1 => "SidetoneLevel",
        2 => "FootstepScaling",
        3 => "BluetoothVolume",
        47 => "NoFunction",
        // Current getMappings returns [] for default or an unknown enum.
        _ => return vec![],
    };
    let mut mapping =
        json!({"inputID":"KEY_SWITCH_AUDIO_EQ", "inputType":"AudioInput", "isHyperShift":false});
    if index == 47 {
        mapping["outputType"] = json!("disableGroup");
    } else {
        mapping["outputType"] = json!("audioGroup");
        mapping["audioGroup"] = json!({"audioMode":assignment,"audioAssignment":assignment});
    }
    vec![mapping]
}

/// None is a getter; Some([]) is the source's explicit restore-default setter.
pub fn execute(
    mappings: Option<&[Value]>,
    mut exchange: impl FnMut(BlackSharkFirmwareCommand) -> Result<BlackSharkFirmwareCommandProgress>,
    mut validate: impl FnMut() -> Result<()>,
) -> RollerMappingProgress {
    let mut progress = RollerMappingProgress::default();
    let result = (|| -> Result<()> {
        validate()?;
        let Some(mappings) = mappings else {
            let command = exchange(BlackSharkFirmwareCommand::GetRollerFunction)?;
            let observed = command
                .succeeded()
                .then(|| {
                    command
                        .reply()
                        .and_then(|reply| reply.data.first().copied())
                })
                .flatten();
            let failure = command
                .failure
                .as_ref()
                .map(|failure| failure.message.clone());
            let exceptional = command.failure.as_ref().is_some_and(|failure| {
                matches!(
                    failure.kind,
                    BlackSharkFirmwareFailureKind::Validation
                        | BlackSharkFirmwareFailureKind::Scheduling
                        | BlackSharkFirmwareFailureKind::Transport
                )
            });
            progress.commands.push(command);
            validate()?;
            if let Some(observed) = observed {
                progress.observed_index = Some(observed);
                progress.observed_mappings = Some(mappings_from_index(observed));
                progress.source_mappings = progress.observed_mappings.clone();
            } else {
                if !exceptional {
                    progress.source_mappings = Some(vec![]);
                }
                anyhow::bail!(
                    "Roller getter did not return a successful value: {}",
                    failure.as_deref().unwrap_or("missing reply data")
                );
            }
            return Ok(());
        };
        validate_mappings(mappings)?;
        let mut requested = Vec::new();
        let mut succeeded = true;
        if mappings.is_empty() {
            requested.push(0);
        } else {
            for (position, mapping) in mappings.iter().enumerate() {
                match mapping["outputType"].as_str() {
                    Some("disableGroup") => requested.push(47),
                    Some("audioGroup") => {
                        let assignment = mapping["audioGroup"]["audioAssignment"]
                            .as_str()
                            .unwrap_or("");
                        if let Some(index) = index(assignment) {
                            requested.push(index);
                        } else {
                            succeeded = false;
                            progress.rejected_mappings.push(format!(
                                "mapping {position}: invalid assignment {assignment:?}"
                            ));
                        }
                    }
                    _ => progress.ignored_mapping_indices.push(position),
                }
            }
        }
        for index in requested {
            validate()?;
            let command = exchange(BlackSharkFirmwareCommand::SetRollerFunction(index))?;
            succeeded &= command.succeeded();
            let exceptional = command
                .failure
                .as_ref()
                .filter(|failure| {
                    matches!(
                        failure.kind,
                        BlackSharkFirmwareFailureKind::Validation
                            | BlackSharkFirmwareFailureKind::Scheduling
                            | BlackSharkFirmwareFailureKind::Transport
                    )
                })
                .map(|failure| failure.message.clone());
            progress.commands.push(command);
            if let Some(error) = exceptional {
                anyhow::bail!(error);
            }
            // Rejection returns false and the current source continues with the
            // next item. A canceled/expired physical scope stops further writes.
            validate()?;
        }
        progress.source_result = Some(succeeded);
        Ok(())
    })();
    if let Err(error) = result {
        progress.error = Some(format!("{error:#}"));
    }
    progress
}

#[cfg(test)]
#[path = "blackshark_roller_mapping_tests.rs"]
mod tests;
