//! Current1342 hardware-event registration: actual container-bound HID input.
use super::{hid, mixer_driver};
use anyhow::{Context as _, ensure};
use razer_device::{
    audio_mixer_hardware::{self, Snapshot},
    audio_mixer_local::{Command, Receipt},
    backend::{HidNode, InterruptTransport},
};
use razer_hid::with_backend;
use serde_json::{Value, json};

pub(super) struct Hardware {
    node: HidNode,
    container: String,
    readers: Vec<(HidNode, Box<dyn InterruptTransport>)>,
    previous: Snapshot,
    mic_muted: bool,
}
impl Hardware {
    pub(super) fn start(node: HidNode) -> anyhow::Result<(Self, Value)> {
        let physical = mixer_driver::observe_node(&node, 1342)?;
        let container = physical["device_container_id"]
            .as_str()
            .context("Mixer hardware container absent")?
            .to_owned();
        let validate = || {
            ensure!(
                mixer_driver::observe_node(&node, 1342)? == physical,
                "Mixer hardware instance changed during initialization"
            );
            Ok(())
        };
        let initial = with_backend(|backend| {
            let device = backend.open_validated(&node, std::rc::Rc::new(&validate))?;
            let lease = device.begin_exchange_validated(
                razer_device::backend::ExchangeOptions::plain(),
                std::rc::Rc::new(&validate),
            )?;
            let reading = audio_mixer_hardware::read_initial(lease.transport());
            let snapshot = reading.as_ref().ok().cloned();
            razer_device::backend::finish_exchange(lease, reading).map_err(|error| match snapshot {
                Some(snapshot) => {
                    error.context(audio_mixer_hardware::InitialReadFailure { snapshot })
                }
                None => error,
            })
        })?;
        let readers = (|| {
            let observation = hid::enumerate_for_product(0x1532, 1342)?;
            ensure!(
                observation["complete"] == true,
                "Mixer hardware enumeration incomplete"
            );
            let paths = observation["interfaces"]
                .as_array()
                .context("Mixer interfaces absent")?
                .iter()
                .filter(|item| {
                    item["device_container_id"]
                        .as_str()
                        .is_some_and(|id| id.eq_ignore_ascii_case(&container))
                })
                .filter_map(|item| item["path"].as_str().map(str::to_owned))
                .collect::<Vec<_>>();
            with_backend(|backend| {
                let nodes = backend.enumerate()?;
                let mut readers = Vec::new();
                for current in nodes.into_iter().filter(|n| {
                    n.product_id == 1342
                        && n.vendor_id == 0x1532
                        && paths
                            .iter()
                            .any(|p| n.path.eq_ignore_ascii_case(p.as_bytes()))
                }) {
                    let reader =
                        backend.open_interrupt_validated(&current, std::rc::Rc::new(&validate))?;
                    if reader.report_lengths().input.contains_key(&9) {
                        readers.push((current, reader));
                    }
                }
                ensure!(
                    !readers.is_empty(),
                    "Mixer has no actual report9 interrupt collection"
                );
                Ok(readers)
            })
        })()
        .map_err(|error| {
            error.context(audio_mixer_hardware::InitialReadFailure {
                snapshot: initial.clone(),
            })
        })?;
        let response = json!({"node":node,"product_id":1342,"started":true,"initial":initial,"device_container_id":container});
        let mic_muted = initial.mic_muted;
        Ok((
            Self {
                node,
                container,
                readers,
                previous: initial,
                mic_muted,
            },
            response,
        ))
    }
    fn native(&self, commands: Vec<Command>) -> anyhow::Result<Vec<Receipt>> {
        if commands.is_empty() {
            return Ok(Vec::new());
        }
        let validate = || {
            let physical = mixer_driver::observe_node(&self.node, 1342)?;
            ensure!(
                physical["device_container_id"]
                    .as_str()
                    .is_some_and(|c| c.eq_ignore_ascii_case(&self.container)),
                "Mixer native event owner changed"
            );
            Ok(())
        };
        let device = with_backend(|backend| {
            backend.open_validated(&self.node, std::rc::Rc::new(&validate))
        })?;
        let control = |report: &[u8]| hid::mixer_control_output(&self.node, report);
        razer_device::audio_mixer_local::execute_group(
            device.as_ref(),
            &commands,
            &validate,
            &control,
        )
    }
    pub(super) fn read(&mut self, node: &HidNode) -> anyhow::Result<Value> {
        ensure!(node == &self.node, "Mixer hardware request owner changed");
        let physical = mixer_driver::observe_node(node, 1342)?;
        ensure!(
            physical["device_container_id"]
                .as_str()
                .is_some_and(|c| c.eq_ignore_ascii_case(&self.container)),
            "Mixer hardware physical owner changed"
        );
        let current = with_backend(|backend| backend.enumerate())?;
        let mut snapshots = Vec::new();
        let mut event_receipts = Vec::new();
        let result: anyhow::Result<Vec<Value>> = (|| {
            for (retained, reader) in &self.readers {
                ensure!(
                    current.iter().filter(|n| *n == retained).count() == 1,
                    "Mixer interrupt collection disconnected or changed"
                );
                let lengths = reader.report_lengths();
                let size = lengths
                    .input
                    .values()
                    .max()
                    .copied()
                    .context("Mixer input descriptor absent")?;
                ensure!(size <= 4096, "Mixer interrupt descriptor exceeds capacity");
                let mut report = vec![0; size];
                for _ in 0..128 {
                    let count = reader.read(&mut report, 0)?;
                    if count == 0 {
                        break;
                    }
                    ensure!(
                        count <= report.len(),
                        "Mixer interrupt byte count exceeds buffer capacity"
                    );
                    if report[0] == 9 {
                        snapshots.push(Snapshot::decode(&report[..count], false)?);
                    }
                }
            }
            let mut events = Vec::new();
            let mut first_failure = None;
            for snapshot in &snapshots {
                let mut groups = Vec::new();
                let mut commands = Vec::new();
                if snapshot.keys[5] && !self.previous.keys[5] {
                    self.mic_muted = !self.mic_muted;
                    commands.push(Command::ChannelMute {
                        channel: 5,
                        enabled: self.mic_muted,
                        profile_updates: Vec::new(),
                    });
                    commands.push(Command::InputMicMute {
                        enabled: self.mic_muted,
                    });
                }
                if !commands.is_empty() {
                    groups.push(commands);
                }
                if snapshot.mic_plugged && !self.previous.mic_plugged {
                    groups.push(vec![Command::MicSetup { mode: 3 }]);
                }
                // Source updates retained event state before asynchronous actions.
                self.previous = snapshot.clone();
                let mut receipts = Vec::new();
                let mut errors = Vec::new();
                let mut command_failures = Vec::new();
                let mut terminal_failure = None;
                for commands in groups {
                    match self.native(commands) {
                        Ok(group) => {
                            for r in &group {
                                if !r.transport_completed {
                                    errors.push(r.response.to_string());
                                    if first_failure.is_none() {
                                        first_failure = Some(anyhow::anyhow!(
                                            "Mixer source command did not complete its transport"
                                        ));
                                    }
                                }
                            }
                            receipts.extend(group);
                        }
                        Err(error) => {
                            let terminal =
                                razer_device::backend::exchange_finalization_failed(&error);
                            if let Some(failure) = error.downcast_ref::<razer_device::audio_mixer_local::CommandBatchFailure>() {
                            receipts.extend(failure.receipts.clone());
                            command_failures.push(failure.clone());
                        }
                            errors.push(format!("{error:#}"));
                            if terminal {
                                terminal_failure = Some(error);
                                break;
                            } else if first_failure.is_none() {
                                first_failure = Some(error);
                            }
                        }
                    }
                }
                let error = if errors.is_empty() {
                    None
                } else {
                    Some(errors.join("; "))
                };
                event_receipts.push(audio_mixer_hardware::HardwareEventReceipt {
                    snapshot: snapshot.clone(),
                    receipts: receipts.clone(),
                    command_failures,
                    errors,
                });
                if let Some(error) = terminal_failure {
                    return Err(match first_failure {
                        Some(first) => error.context(format!(
                            "Earlier Mixer hardware action also failed: {first:#}"
                        )),
                        None => error,
                    });
                }
                events.push(
                    json!({"snapshot":snapshot,"native_receipts":receipts,"native_error":error}),
                );
            }
            if let Some(error) = first_failure {
                return Err(error);
            }
            Ok(events)
        })();
        let events = result.map_err(|error| {
            error.context(audio_mixer_hardware::HardwareEventsFailure {
                snapshots,
                events: event_receipts,
            })
        })?;
        Ok(json!({"node":self.node,"product_id":1342,"events":events}))
    }
}
