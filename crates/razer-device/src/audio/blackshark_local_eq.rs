//! Current CloudBasedEqualizer's local slots, edits and firmware completion.
use crate::thx_blackshark_firmware::{
    BlackSharkFirmwareCommand as Command, BlackSharkFirmwareCommandProgress,
    BlackSharkLocalEqPreset as Preset, decode_blackshark_eq_bands, decode_blackshark_eq_order,
};
use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};

pub const SLOT_COUNT: u8 = 9;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Edit {
    pub index: u8,
    pub band_values: Option<[i16; 10]>,
    pub cloud_equalizer_id: Option<u8>,
    pub custom: Option<bool>,
    pub footstep_values: Option<[u8; 10]>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Request {
    Read,
    Hardware(Notification),
    Edit(Edit),
    Reorder { presets: Vec<Preset>, spatial: bool },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Job {
    Read,
    Observe {
        presets: Vec<Preset>,
        disable_thx: bool,
    },
    ReadSlots {
        indices: Vec<u8>,
    },
    Write {
        presets: Vec<Preset>,
        selected: Option<u8>,
        spatial: bool,
        footsteps: Option<[u8; 10]>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Notification {
    Selected { index: u8 },
    Bands { index: u8, values: [i16; 10] },
}

pub fn parse_notification(raw: &[u8]) -> Result<Option<Notification>> {
    if raw.first() != Some(&2) || raw.get(11) != Some(&2) {
        return Ok(None);
    }
    let Some(&record) = raw.get(10) else {
        return Ok(None);
    };
    if !matches!(record, 21 | 96) {
        return Ok(None);
    }
    let index = *raw.get(13).context("EQ notification omits slot")?;
    ensure!(index < SLOT_COUNT, "EQ notification slot is out of range");
    if record == 96 {
        return Ok(Some(Notification::Selected { index }));
    }
    let data = raw
        .get(14..24)
        .context("EQ notification bands are truncated")?;
    let mut values = [0; 10];
    for (value, byte) in values.iter_mut().zip(data) {
        *value = if *byte > 128 {
            128 - i16::from(*byte)
        } else {
            i16::from(*byte)
        };
    }
    Ok(Some(Notification::Bands { index, values }))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Progress {
    pub observed: Option<Vec<Preset>>,
    pub commands: Vec<BlackSharkFirmwareCommandProgress>,
    pub completed: bool,
    pub error: Option<String>,
}

pub fn validate(presets: &[Preset]) -> Result<()> {
    ensure!(
        !presets.is_empty() && presets.len() <= usize::from(SLOT_COUNT),
        "Local EQ requires one to nine ordered slots"
    );
    for (index, preset) in presets.iter().enumerate() {
        ensure!(
            usize::from(preset.index) == index,
            "Local EQ slot ordering differs"
        );
        preset.validate()?;
    }
    Ok(())
}

pub fn prepare(request: &Request, current: &[Preset]) -> Result<Job> {
    if matches!(request, Request::Read) {
        return Ok(Job::Read);
    }
    validate(current)?;
    match request {
        Request::Read => unreachable!(),
        Request::Hardware(event) => {
            let (index, bands) = match event {
                Notification::Selected { index } => (*index, None),
                Notification::Bands { index, values } => (*index, Some(*values)),
            };
            ensure!(
                current.iter().any(|preset| preset.index == index),
                "Hardware EQ slot is absent"
            );
            let mut presets = current.to_vec();
            for preset in &mut presets {
                preset.current_active = preset.index == index;
                if preset.current_active {
                    if let Some(bands) = bands {
                        preset.band_values = bands;
                    }
                }
            }
            validate(&presets)?;
            let disable_thx = matches!(event, Notification::Selected { .. })
                && presets
                    .iter()
                    .any(|preset| preset.current_active && preset.cloud_equalizer_id > 3);
            Ok(Job::Observe {
                presets,
                disable_thx,
            })
        }
        Request::Edit(edit) => {
            ensure!(
                current.iter().any(|preset| preset.index == edit.index),
                "Local EQ selected slot is absent"
            );
            let mut presets = current.to_vec();
            for preset in &mut presets {
                preset.current_active = preset.index == edit.index;
                if preset.current_active {
                    if let Some(bands) = edit.band_values {
                        preset.band_values = bands;
                    }
                    if let Some(id) = edit.cloud_equalizer_id {
                        preset.cloud_equalizer_id = id;
                    }
                    if let Some(custom) = edit.custom {
                        preset.custom = custom;
                    }
                }
            }
            validate(&presets)?;
            Ok(Job::Write {
                presets,
                selected: Some(edit.index),
                spatial: false,
                footsteps: edit.footstep_values,
            })
        }
        Request::Reorder { presets, spatial } => {
            validate(presets)?;
            ensure!(
                presets.iter().any(|preset| preset.current_active),
                "Local EQ active slot is absent"
            );
            Ok(Job::Write {
                presets: presets.clone(),
                selected: None,
                spatial: *spatial,
                footsteps: None,
            })
        }
    }
}

pub fn footsteps(id: u8) -> [u8; 10] {
    match id {
        10 | 13 | 14 => [0, 0, 1, 1, 1, 0, 0, 0, 0, 0],
        11 => [0, 0, 0, 1, 1, 1, 0, 0, 0, 0],
        12 => [0, 1, 1, 1, 0, 0, 0, 0, 0, 0],
        _ => [0, 0, 1, 1, 0, 0, 0, 0, 0, 0],
    }
}

pub fn execute(
    job: &Job,
    mut exchange: impl FnMut(Command) -> Result<BlackSharkFirmwareCommandProgress>,
    mut validate_scope: impl FnMut() -> Result<()>,
) -> Progress {
    let mut progress = Progress {
        observed: None,
        commands: vec![],
        completed: false,
        error: None,
    };
    let result = (|| -> Result<()> {
        let mut send = |command| -> Result<Vec<u8>> {
            validate_scope()?;
            let response = exchange(command)?;
            let data = response.reply().map(|reply| reply.data.clone());
            let success = response.succeeded();
            let failure = response.failure.clone();
            progress.commands.push(response);
            validate_scope()?;
            ensure!(success, "Local EQ firmware command failed: {failure:?}");
            data.context("Local EQ command has no response")
        };
        match job {
            Job::Observe { presets, .. } => {
                validate(presets)?;
                progress.observed = Some(presets.clone());
            }
            Job::Read | Job::ReadSlots { .. } => {
                let indices = match job {
                    Job::ReadSlots { indices } => indices.clone(),
                    _ => (0..SLOT_COUNT).collect(),
                };
                ensure!(
                    !indices.is_empty() && indices.len() <= usize::from(SLOT_COUNT),
                    "Invalid EQ read scope"
                );
                let mut presets = Vec::with_capacity(indices.len());
                for index in indices {
                    ensure!(index < SLOT_COUNT, "Invalid EQ slot index");
                    let raw = send(Command::GetOrder { index })?;
                    ensure!(
                        raw.first() != Some(&255),
                        "Local EQ slot {index} is not initialized"
                    );
                    let order = decode_blackshark_eq_order(&raw, index)?;
                    let bands =
                        decode_blackshark_eq_bands(&send(Command::GetBands { index })?, index)?;
                    presets.push(Preset {
                        index,
                        voice_prompt_index: order.voice_prompt_index,
                        band_values: bands,
                        enable_state: order.enabled,
                        current_active: order.active,
                        cloud_equalizer_id: order.cloud_equalizer_id,
                        custom: order.custom,
                    });
                }
                progress.observed = Some(presets);
            }
            Job::Write {
                presets,
                selected,
                spatial,
                footsteps: configured_footsteps,
            } => {
                validate(presets)?;
                if *spatial {
                    let active = presets
                        .iter()
                        .find(|preset| preset.current_active)
                        .context("Local EQ active slot is absent")?;
                    send(Command::SetOrder(active.order()))?;
                } else {
                    send(Command::BatchStart)?;
                    for preset in presets
                        .iter()
                        .filter(|preset| selected.is_none_or(|index| preset.index == index))
                    {
                        send(Command::SetBands {
                            index: preset.index,
                            bands: preset.band_values,
                        })?;
                        send(Command::SetOrder(preset.order()))?;
                    }
                    send(Command::BatchStop)?;
                    if selected.is_some() {
                        let active = presets
                            .iter()
                            .find(|preset| preset.current_active)
                            .context("Local EQ active slot is absent")?;
                        send(Command::SetFootsteps {
                            index: active.index,
                            bands: configured_footsteps
                                .unwrap_or_else(|| footsteps(active.cloud_equalizer_id)),
                        })?;
                    }
                }
            }
        }
        Ok(())
    })();
    match result {
        Ok(()) => progress.completed = true,
        Err(error) => progress.error = Some(format!("{error:#}")),
    }
    progress
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hardware_reports_update_slots_without_firmware_writes() {
        for pid in [1398,1401,2638,2641,4124,4126] {
            let current=crate::thx_blackshark_firmware::default_blackshark_local_eq_presets(pid).unwrap();
            let mut raw=[0u8;64];raw[0]=2;raw[10]=96;raw[11]=2;raw[13]=4;
            let event=parse_notification(&raw).unwrap().unwrap();
            let job=prepare(&Request::Hardware(event),&current).unwrap();
            let Job::Observe{presets,disable_thx}=&job else {panic!()};
            assert!(presets[4].current_active);
            assert_eq!(*disable_thx,presets[4].cloud_equalizer_id>3);
            let progress=execute(&job,|_|panic!("hardware notification must not write firmware"),||Ok(()));
            assert!(progress.completed && progress.commands.is_empty());
            raw[10]=21;raw[13]=2;raw[14..24].copy_from_slice(&[134,133,132,131,130,129,0,1,2,6]);
            let event=parse_notification(&raw).unwrap().unwrap();
            let Job::Observe{presets,disable_thx}=prepare(&Request::Hardware(event),&current).unwrap() else {panic!()};
            assert_eq!(presets[2].band_values,[-6,-5,-4,-3,-2,-1,0,1,2,6]);
            assert!(!disable_thx);
            assert_eq!(presets[0].band_values,current[0].band_values);
            assert!(parse_notification(&raw[..20]).is_err());
            raw[11]=1;assert!(parse_notification(&raw).unwrap().is_none());
        }
    }
    use crate::backend::HidNode;
    use crate::thx_blackshark_firmware::{
        BlackSharkFirmwareAttempt, BlackSharkFirmwareIdentity, BlackSharkFirmwareReply,
        default_blackshark_local_eq_presets,
    };

    fn response(pid: u32, command: Command, data: Vec<u8>) -> BlackSharkFirmwareCommandProgress {
        BlackSharkFirmwareCommandProgress {
            command,
            identity: BlackSharkFirmwareIdentity::new(
                pid,
                HidNode {
                    path: b"scripted-eq".to_vec(),
                    vendor_id: 0x1532,
                    product_id: pid as u16,
                    usage_page: 0xff00,
                    usage: 1,
                    interface_number: 5,
                    release_number: 1,
                    serial_number: None,
                    manufacturer: None,
                    product: None,
                },
                "scripted-container",
                1,
            )
            .unwrap(),
            attempts: vec![BlackSharkFirmwareAttempt {
                reply: Some(BlackSharkFirmwareReply {
                    status: 2,
                    resend_requested: false,
                    header: vec![],
                    data,
                }),
                ..Default::default()
            }],
            completed: true,
            failure: None,
        }
    }

    #[test]
    fn local_eq_reads_each_actual_order_and_band_without_defaults() {
        for pid in [1398, 1401, 2638, 2641, 4124, 4126] {
            let mut calls = vec![];
            let progress = execute(
                &Job::Read,
                |command| {
                    calls.push(command.clone());
                    let data = match command {
                        Command::GetOrder { index } => {
                            vec![index, index, 1, u8::from(index == 2), index, 0]
                        }
                        Command::GetBands { index } => vec![index, 129, 2, 3, 4, 5, 6, 7, 8, 9, 10],
                        _ => panic!("read must not write"),
                    };
                    Ok(response(pid, command, data))
                },
                || Ok(()),
            );
            assert!(progress.completed, "{:?}", progress.error);
            let observed = progress.observed.unwrap();
            assert_eq!(observed.len(), 9);
            assert_eq!(observed[2].band_values, [-1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
            assert!(observed[2].current_active);
            assert_eq!(calls.len(), 18);
            for (index, pair) in calls.chunks_exact(2).enumerate() {
                assert_eq!(
                    pair,
                    &[
                        Command::GetOrder { index: index as u8 },
                        Command::GetBands { index: index as u8 }
                    ]
                );
            }
        }
        let missing = execute(
            &Job::Read,
            |command| Ok(response(1398, command, vec![255, 0, 0, 0, 0, 0])),
            || Ok(()),
        );
        assert!(!missing.completed);
        assert!(missing.observed.is_none());
        assert_eq!(missing.commands.len(), 1);
    }

    #[test]
    fn local_eq_edits_reorder_and_failure_keep_source_command_order() {
        for pid in [1398, 1401, 2638, 2641, 4124, 4126] {
            let current = default_blackshark_local_eq_presets(pid).unwrap();
            let request = Request::Edit(Edit {
                index: 2,
                band_values: Some([-2; 10]),
                cloud_equalizer_id: None,
                custom: Some(true),
                footstep_values: None,
            });
            let job = prepare(&request, &current).unwrap();
            let Job::Write { presets, .. } = &job else { panic!() };
            assert!(!presets[0].current_active);
            assert!(presets[2].current_active && presets[2].custom);
            assert_eq!(presets[1].band_values, current[1].band_values);
            let mut calls = vec![];
            let done = execute(
                &job,
                |command| {
                    calls.push(command.clone());
                    Ok(response(pid, command, vec![0]))
                },
                || Ok(()),
            );
            assert!(done.completed);
            assert_eq!(
                calls,
                [
                    Command::BatchStart,
                    Command::SetBands {
                        index: 2,
                        bands: [-2; 10]
                    },
                    Command::SetOrder(presets[2].order()),
                    Command::BatchStop,
                    Command::SetFootsteps {
                        index: 2,
                        bands: [0, 0, 1, 1, 0, 0, 0, 0, 0, 0]
                    }
                ]
            );
            let failed = execute(
                &job,
                |command| {
                    if matches!(command, Command::SetBands { .. }) {
                        anyhow::bail!("write failed")
                    }
                    Ok(response(pid, command, vec![0]))
                },
                || Ok(()),
            );
            assert!(!failed.completed);
            assert_eq!(failed.commands.len(), 1);
            let spatial = Job::Write {
                presets: presets.clone(),
                selected: None,
                spatial: true,
                footsteps: None,
            };
            let done = execute(
                &spatial,
                |command| {
                    assert_eq!(command, Command::SetOrder(presets[2].order()));
                    Ok(response(pid, command, vec![0]))
                },
                || Ok(()),
            );
            assert!(done.completed);
            assert_eq!(done.commands.len(), 1);
            let override_job = prepare(
                &Request::Edit(Edit {
                    index: 1,
                    band_values: None,
                    cloud_equalizer_id: None,
                    custom: None,
                    footstep_values: Some([1; 10]),
                }),
                &current,
            )
            .unwrap();
            let override_result = execute(
                &override_job,
                |command| Ok(response(pid, command, vec![0])),
                || Ok(()),
            );
            assert!(override_result.completed);
            assert_eq!(
                override_result.commands.last().unwrap().command,
                Command::SetFootsteps {
                    index: 1,
                    bands: [1; 10]
                }
            );
        }
    }

    #[test]
    fn local_eq_cancelled_read_never_publishes_partial_or_writes() {
        let checks = std::cell::Cell::new(0);
        let progress = execute(
            &Job::Read,
            |command| Ok(response(1398, command, vec![0, 0, 1, 1, 0, 0])),
            || {
                let next = checks.get() + 1;
                checks.set(next);
                ensure!(next < 2, "cancelled");
                Ok(())
            },
        );
        assert!(progress.observed.is_none());
        assert!(!progress.completed);
        assert_eq!(progress.commands.len(), 1);
        let cmd = Command::SetFootsteps {
            index: 3,
            bands: [0, 0, 1, 1, 0, 0, 0, 0, 0, 0],
        };
        assert_eq!(cmd.command_header(), [235, 0, 11]);
        assert_eq!(cmd.expected_response(), [235, 1, 1]);
        assert_eq!(cmd.payload().unwrap(), [3, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0]);
    }
}
