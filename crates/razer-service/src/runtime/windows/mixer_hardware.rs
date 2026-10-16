//! Current1342 hardware-event registration: actual container-bound HID input.
use super::{hid, mixer_driver};
use anyhow::{Context as _, ensure};
use razer_device::{
    audio_mixer_hardware::{self, Snapshot},
    audio_mixer_local::{Command, Receipt},
    backend::{HidBackend, HidNode, InterruptTransport},
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
        let initial = with_backend(|backend| {
            let device = backend.open(&node)?;
            audio_mixer_hardware::read_initial(device.as_ref())
        })?;
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
        let readers = with_backend(|backend| {
            let nodes = backend.enumerate()?;
            let mut readers = Vec::new();
            for current in nodes.into_iter().filter(|n| {
                n.product_id == 1342
                    && n.vendor_id == 0x1532
                    && paths
                        .iter()
                        .any(|p| n.path.eq_ignore_ascii_case(p.as_bytes()))
            }) {
                let reader = backend.open_interrupt(&current)?;
                if reader.report_lengths().input.contains_key(&9) {
                    readers.push((current, reader));
                }
            }
            ensure!(
                !readers.is_empty(),
                "Mixer has no actual report9 interrupt collection"
            );
            Ok(readers)
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
        let device = with_backend(|backend| backend.open(&self.node))?;
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
        let control = |report: &[u8]| hid::mixer_control_output(&self.node, report);
        let mut receipts = Vec::new();
        for command in commands {
            match razer_device::audio_mixer_local::execute(
                device.as_ref(),
                &command,
                &validate,
                &control,
            ) {
                Ok(receipt) => receipts.push(receipt),
                Err(error) => {
                    receipts.push(Receipt {
                        command,
                        transport_completed: false,
                        response: json!({"error":format!("{error:#}")}),
                        profile_updates: Vec::new(),
                    });
                    break;
                }
            }
        }
        Ok(receipts)
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
                if report[0] == 9 {
                    snapshots.push(Snapshot::decode(&report[..count], false)?);
                }
            }
        }
        let mut events = Vec::new();
        for snapshot in snapshots {
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
            for commands in groups {
                match self.native(commands) {
                    Ok(group) => {
                        for r in &group {
                            if !r.transport_completed {
                                errors.push(r.response.to_string());
                            }
                        }
                        receipts.extend(group);
                    }
                    Err(error) => errors.push(format!("{error:#}")),
                }
            }
            let error = if errors.is_empty() {
                None
            } else {
                Some(errors.join("; "))
            };
            events
                .push(json!({"snapshot":snapshot,"native_receipts":receipts,"native_error":error}));
        }
        Ok(json!({"node":self.node,"product_id":1342,"events":events}))
    }
}
