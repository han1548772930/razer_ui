//! Marie routing state and ordered writes, from native 4bbc0/4e6b0 and
//! 4d520–4faa0. Constructed levels are source mixer drafts, not observations.
use crate::seiren_driver::{
    self as driver, AudioControlWrite, Channel, ChannelCounts, ChannelKind, MeterReading, Mode,
    Route,
};
use anyhow::{Context, Result, ensure};
use std::collections::BTreeMap;

#[path = "seiren_mixer/observation.rs"]
mod observation;
pub use observation::{LiveState, Reading};

pub const HEADPHONES_INPUT: u32 = 65536;
pub const MICROPHONE_ALIAS: u32 = 131072;
pub const STREAM: u32 = 131073;
pub const PLAYBACK: u32 = 131074;
pub const MICROPHONE: u32 = 262145;
pub const SERVICE: u32 = 262146;
pub const HEADPHONES: u32 = 524289;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EndpointObservation {
    pub level: u32,
    pub muted: bool,
}

#[derive(Clone, Debug)]
struct Node {
    channels: Vec<Channel>,
    draft: EndpointObservation,
    observed: Option<EndpointObservation>,
}

#[cfg(test)]
#[path = "seiren_mixer/tests.rs"]
mod tests;
#[derive(Clone, Copy, Debug)]
struct RouteDraft {
    level: u32,
    enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Write {
    Dsp(Vec<u8>),
    Control(AudioControlWrite),
    EndpointVolume { device_id: u32, level: u32 },
    EndpointMute { device_id: u32, muted: bool },
}

pub struct Mixer {
    nodes: BTreeMap<u32, Node>,
    routes: BTreeMap<(u32, u32), RouteDraft>,
    mode: Mode,
    stream_monitor: bool,
    playback_external: bool,
    basic_monitor: RouteDraft,
}
impl Mixer {
    pub fn new(counts: ChannelCounts, mode: Mode) -> Result<Self> {
        ensure!(
            !matches!(mode, Mode::Unknown(_)),
            "Marie mixer mode has no known routing semantics"
        );
        driver::meter_channels(counts)?;
        let mut nodes = BTreeMap::new();
        let mut add = |id, kind, start: u32, count: u32, available: u32| -> Result<()> {
            ensure!(
                start + count <= available,
                "Marie channel group {id} is truncated"
            );
            nodes.insert(
                id,
                Node {
                    channels: (start..start + count)
                        .map(|index| Channel {
                            kind,
                            index: index as u8,
                        })
                        .collect(),
                    draft: EndpointObservation {
                        level: 100,
                        muted: false,
                    },
                    observed: None,
                },
            );
            Ok(())
        };
        if counts.device_output >= 2 {
            add(
                HEADPHONES,
                ChannelKind::DeviceOutput,
                0,
                2,
                counts.device_output,
            )?;
        }
        if counts.app_recording > 0 {
            add(
                MICROPHONE_ALIAS,
                ChannelKind::AppRecording,
                0,
                1,
                counts.app_recording,
            )?;
        }
        for (start, id) in [(0, STREAM), (2, PLAYBACK)] {
            if counts.app_recording_virtual > start {
                add(
                    id,
                    ChannelKind::AppRecordingVirtual,
                    start,
                    2,
                    counts.app_recording_virtual,
                )?;
            }
        }
        if counts.app_playback >= 2 {
            add(
                HEADPHONES_INPUT,
                ChannelKind::AppPlayback,
                0,
                2,
                counts.app_playback,
            )?;
        }
        if counts.device_input > 0 {
            add(
                MICROPHONE,
                ChannelKind::DeviceInput,
                0,
                1,
                counts.device_input,
            )?;
        }
        if counts.device_input >= 3 {
            add(SERVICE, ChannelKind::DeviceInput, 1, 1, counts.device_input)?;
            add(262147, ChannelKind::DeviceInput, 2, 1, counts.device_input)?;
        }
        for (index, id) in [
            65537, 65538, 65539, 65540, 65541, 65542, 65553, 65554, 65555,
        ]
        .into_iter()
        .enumerate()
        {
            let start = index as u32 * 2;
            if counts.app_playback_virtual > start {
                add(
                    id,
                    ChannelKind::AppPlaybackVirtual,
                    start,
                    2,
                    counts.app_playback_virtual,
                )?;
            }
        }
        let mut routes = BTreeMap::new();
        for (&output, node) in &nodes {
            if !node.channels[0].kind.output() {
                continue;
            }
            for (&input, node) in &nodes {
                // 4e6b0 creates dry/post-exp meter nodes without adding routes.
                if node.channels[0].kind.output() || matches!(input, SERVICE | 262147) {
                    continue;
                }
                routes.insert(
                    (output, input),
                    RouteDraft {
                        level: 100,
                        enabled: false,
                    },
                );
            }
        }
        Ok(Self {
            nodes,
            routes,
            mode,
            stream_monitor: false,
            playback_external: false,
            basic_monitor: RouteDraft {
                level: 0,
                enabled: false,
            },
        })
    }
    pub fn mode(&self) -> &Mode {
        &self.mode
    }
    pub fn observe_endpoint(&mut self, id: u32, value: EndpointObservation) -> Result<()> {
        ensure!(value.level <= 100, "Marie endpoint level exceeds range");
        let node = self
            .nodes
            .get_mut(&id)
            .context("Marie endpoint has no observed channel")?;
        node.observed = Some(value);
        node.draft = value;
        Ok(())
    }
    pub fn endpoint_observation(&self, id: u32) -> Option<EndpointObservation> {
        self.nodes.get(&id).and_then(|node| node.observed)
    }
    pub fn invalidate_endpoint(&mut self, id: u32) {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.observed = None;
        }
    }
    fn node(&self, id: u32) -> Result<&Node> {
        self.nodes
            .get(&id)
            .with_context(|| format!("Marie node {id} is absent"))
    }
    fn selected_mix(&self) -> u32 {
        if self.stream_monitor { STREAM } else { PLAYBACK }
    }
    fn route(&self, output: u32, input: u32, settings: RouteDraft) -> Result<Vec<Write>> {
        let source = self.node(input)?;
        let destination = self.node(output)?;
        let mut writes = Vec::new();
        let mut level = settings.level;
        let mut enabled = settings.enabled;
        if output == HEADPHONES && input == MICROPHONE {
            let mix = self.node(self.selected_mix())?;
            // Physical endpoint state must be observed; draft defaults are not
            // evidence that the microphone is unmuted.
            let microphone = source
                .observed
                .context("Marie microphone mute has not been observed")?;
            let monitor = if self.mode == Mode::Basic {
                self.basic_monitor
            } else {
                let level = ((mix.draft.level as f32 / 100.0) * settings.level as f32) as u32;
                RouteDraft {
                    level,
                    enabled: level != 0
                        && settings.enabled
                        && !microphone.muted
                        && !mix.draft.muted,
                }
            };
            writes.push(Write::Control(AudioControlWrite::monitor_enabled(
                monitor.enabled,
            )));
            writes.extend(
                AudioControlWrite::monitor_level(monitor.level)?
                    .into_iter()
                    .map(Write::Control),
            );
            enabled = false;
        }
        if self.mode == Mode::Basic && output == HEADPHONES && input == HEADPHONES_INPUT {
            level = 100;
            enabled = true;
        }
        let routes = destination
            .channels
            .iter()
            .enumerate()
            .map(|(index, output)| Route {
                input: source
                    .channels
                    .get(index)
                    .copied()
                    .unwrap_or(source.channels[0]),
                output: *output,
            })
            .collect::<Vec<_>>();
        writes.push(Write::Dsp(driver::route_gain(&routes, level, enabled)?));
        Ok(writes)
    }
    fn routing(&self, output: u32, input: u32) -> Result<Vec<Write>> {
        let settings = *self
            .routes
            .get(&(output, input))
            .context("Marie route is absent")?;
        let outputs: &[u32] = match output {
            STREAM if self.stream_monitor => &[STREAM, PLAYBACK, HEADPHONES],
            PLAYBACK if self.stream_monitor => &[],
            PLAYBACK => &[PLAYBACK, HEADPHONES],
            _ => std::slice::from_ref(&output),
        };
        let mut writes = Vec::new();
        for &output in outputs {
            writes.extend(self.route(output, input, settings)?);
        }
        Ok(writes)
    }
    pub fn refresh_monitor(&self) -> Result<Vec<Write>> {
        let settings = *self
            .routes
            .get(&(self.selected_mix(), MICROPHONE))
            .context("Marie monitor route is absent")?;
        self.route(HEADPHONES, MICROPHONE, settings)
    }
    fn output_gain(&self, id: u32) -> Result<Vec<Write>> {
        let node = self.node(id)?;
        if !matches!(id, STREAM | PLAYBACK) || id == PLAYBACK && self.stream_monitor {
            return Ok(Vec::new());
        }
        let mut writes = vec![Write::Dsp(driver::output_gain(
            &node.channels,
            node.draft.level,
            !node.draft.muted,
        )?)];
        if id == STREAM && !self.stream_monitor {
            return Ok(writes);
        }
        if id == STREAM {
            writes.push(Write::Dsp(driver::output_gain(
                &self.node(PLAYBACK)?.channels,
                node.draft.level,
                !node.draft.muted,
            )?));
        }
        let (level, enabled) = if self.mode == Mode::Basic {
            (100, true)
        } else {
            (
                node.draft.level,
                !node.draft.muted && !self.playback_external,
            )
        };
        writes.push(Write::Dsp(driver::output_gain(
            &self.node(HEADPHONES)?.channels,
            level,
            enabled,
        )?));
        writes.extend(self.refresh_monitor()?);
        Ok(writes)
    }
    pub fn set_route_enabled(
        &mut self,
        output: u32,
        input: u32,
        enabled: bool,
    ) -> Result<Vec<Write>> {
        let input = if input == MICROPHONE_ALIAS {
            MICROPHONE
        } else {
            input
        };
        self.routes
            .get_mut(&(output, input))
            .context("Marie route is absent")?
            .enabled = enabled;
        self.routing(output, input)
    }
    pub fn set_route_level(&mut self, output: u32, input: u32, level: u32) -> Result<Vec<Write>> {
        ensure!(level <= 100, "Marie route level exceeds range");
        let input = if input == MICROPHONE_ALIAS {
            MICROPHONE
        } else {
            input
        };
        // Wrapper 55c0 first enables the route using its previous level, then
        // publishes the new level. Preserve both writes and their ordering.
        let mut writes = self.set_route_enabled(output, input, true)?;
        self.routes
            .get_mut(&(output, input))
            .context("Marie route is absent")?
            .level = level;
        writes.extend(self.routing(output, input)?);
        Ok(writes)
    }
    pub fn set_volume(&mut self, id: u32, level: u32) -> Result<Vec<Write>> {
        ensure!(level <= 100, "Marie node level exceeds range");
        self.nodes
            .get_mut(&id)
            .context("Marie volume node is absent")?
            .draft
            .level = level;
        if matches!(id, MICROPHONE | SERVICE | HEADPHONES) {
            return Ok(vec![Write::EndpointVolume {
                device_id: id,
                level,
            }]);
        }
        self.output_gain(id)
    }
    pub fn set_muted(&mut self, id: u32, muted: bool) -> Result<Vec<Write>> {
        self.nodes
            .get_mut(&id)
            .context("Marie mute node is absent")?
            .draft
            .muted = muted;
        if matches!(id, MICROPHONE | SERVICE | HEADPHONES) {
            return Ok(vec![Write::EndpointMute {
                device_id: id,
                muted,
            }]);
        }
        self.output_gain(id)
    }
    pub fn set_monitor_enabled(&mut self, enabled: bool) -> Vec<Write> {
        if self.mode == Mode::Basic {
            self.basic_monitor.enabled = enabled;
        }
        vec![Write::Control(AudioControlWrite::monitor_enabled(enabled))]
    }
    pub fn set_monitor_level(&mut self, level: u32) -> Result<Vec<Write>> {
        let writes = AudioControlWrite::monitor_level(level)?
            .into_iter()
            .map(Write::Control)
            .collect();
        if self.mode == Mode::Basic {
            self.basic_monitor.level = level;
        }
        Ok(writes)
    }
    pub fn set_stream_monitor(&mut self, enabled: bool) -> Result<Vec<Write>> {
        self.node(PLAYBACK)?;
        self.node(STREAM)?;
        self.stream_monitor = enabled;
        let mut writes = self.output_gain(PLAYBACK)?;
        writes.extend(self.output_gain(STREAM)?);
        for &(output, input) in self.routes.keys() {
            if output == self.selected_mix() {
                writes.extend(self.routing(PLAYBACK, input)?);
                writes.extend(self.routing(STREAM, input)?);
            }
        }
        Ok(writes)
    }
    pub fn set_playback_external(&mut self, enabled: bool) -> Result<Vec<Write>> {
        self.node(PLAYBACK)?;
        self.playback_external = enabled;
        self.output_gain(PLAYBACK)
    }
    pub fn observe_mode(&mut self, mode: Mode) -> Result<Vec<Write>> {
        self.observe_mode_value(mode)?;
        self.restore_mode_routes()
    }
    pub fn observe_mode_value(&mut self, mode: Mode) -> Result<()> {
        ensure!(
            !matches!(mode, Mode::Unknown(_)),
            "Marie mode has unknown routing semantics"
        );
        self.mode = mode;
        Ok(())
    }
    pub fn restore_mode_routes(&self) -> Result<Vec<Write>> {
        let mut writes = self.output_gain(PLAYBACK)?;
        writes.extend(self.output_gain(STREAM)?);
        writes.extend(self.routing(PLAYBACK, HEADPHONES_INPUT)?);
        Ok(writes)
    }
    pub fn meter_levels(&self, readings: &[MeterReading]) -> Result<BTreeMap<u32, Vec<u32>>> {
        let mut levels = BTreeMap::new();
        for (&id, node) in &self.nodes {
            if self.mode == Mode::Basic && !matches!(id, MICROPHONE | HEADPHONES) {
                continue;
            }
            let values = node
                .channels
                .iter()
                .map(|channel| {
                    let reading = readings
                        .iter()
                        .find(|value| value.channel == *channel)
                        .context("Marie meter response lacks an observed channel")?;
                    Ok(if self.stream_monitor && id == PLAYBACK {
                        0
                    } else {
                        reading.percent()
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            levels.insert(id, values);
        }
        Ok(levels)
    }
}
