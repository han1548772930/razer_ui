//! Mounted PID1342 local MicSetup/preview actions from current MW5736 Ri/bi/Li/Pi/Ui/Wi/Yi.
//! RzNative_053E_v1.0.16.0 SetMicSetup RVA35870 uses exact two-byte control OUT.
use crate::{
    audio_mixer::{MixerChannel, MixerControl, MixerSession, MixerTarget, MixerValue},
    backend::FeatureTransport,
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[path = "audio_mixer_local/lighting.rs"]
pub mod lighting;
#[path = "audio_mixer_local/reports.rs"]
mod reports;
pub use reports::{ReportChannel, TransferFailure, TransferProgress};
#[cfg(test)]
#[path = "audio_mixer_local/source_feature_tests.rs"]
mod source_feature_tests;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Command {
    LightingSample {
        sources: [u8; 4],
    },
    Lighting {
        action: lighting::Action,
    },
    Endpoint {
        jack: u8,
        flow: u8,
        value: MixerValue,
    },
    Route {
        input: u8,
        output: u8,
        enabled: bool,
    },
    AnalogGain {
        key: String,
        value: f64,
    },
    AnalogMute {
        key: String,
        enabled: bool,
    },
    Bleep {
        enabled: bool,
    },
    Fader {
        region: u8,
        mode: u8,
    },
    MicSetup {
        mode: u8,
    },
    PhantomPower {
        enabled: bool,
    },
    InputMicGain {
        value: f64,
    },
    InputMicMute {
        enabled: bool,
    },
    ChannelMute {
        channel: u8,
        enabled: bool,
        #[serde(default)]
        profile_updates: Vec<(String, Value)>,
    },
    Property {
        target: MixerTarget,
        value: MixerValue,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Receipt {
    pub command: Command,
    pub transport_completed: bool,
    /// Actual input bytes or source property receipt, never a synthesized read.
    pub response: Value,
    /// Explicit Yi local-storage assignments after the native channel send.
    /// These are not source UI dispatches or device readbacks.
    pub profile_updates: Vec<(String, Value)>,
}

#[derive(Debug)]
pub struct CommandFailure {
    /// Completed source command retained as diagnostics after owner loss.
    pub completed: Receipt,
}

impl std::fmt::Display for CommandFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Mixer source command completed; final owner validation failed")
    }
}

impl std::error::Error for CommandFailure {}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CommandBatchFailure {
    pub receipts: Vec<Receipt>,
    pub failed_command: Command,
    pub transfer: Option<TransferProgress>,
    pub exchange: Option<crate::audio_mixer::MixerExchangeProgress>,
    pub completed: Option<Receipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query_data: Option<Vec<u8>>,
}

impl CommandBatchFailure {
    pub fn retain(
        error: anyhow::Error,
        receipts: Vec<Receipt>,
        failed_command: Command,
        query_data: Option<Vec<u8>>,
    ) -> anyhow::Error {
        let transfer = error
            .downcast_ref::<TransferFailure>()
            .map(|failure| failure.progress.clone());
        let exchange = error
            .downcast_ref::<crate::audio_mixer::MixerExchangeFailure>()
            .map(|failure| failure.progress.clone());
        let completed = error
            .downcast_ref::<CommandFailure>()
            .map(|failure| failure.completed.clone());
        let query_data = query_data.or_else(|| {
            error
                .downcast_ref::<Self>()
                .and_then(|failure| failure.query_data.clone())
        });
        error.context(Self {
            receipts,
            failed_command,
            transfer,
            exchange,
            completed,
            query_data,
        })
    }
}

impl std::fmt::Display for CommandBatchFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(
            "Mixer command group stopped; actual preceding receipts and failed exchange retained",
        )
    }
}
impl std::error::Error for CommandBatchFailure {}

pub fn execute_group(
    device: &dyn FeatureTransport,
    commands: &[Command],
    validate: &impl Fn() -> anyhow::Result<()>,
    control: &impl Fn(&[u8]) -> anyhow::Result<()>,
) -> anyhow::Result<Vec<Receipt>> {
    let mut receipts = Vec::new();
    for command in commands {
        match execute(device, command, validate, control) {
            Ok(receipt) => receipts.push(receipt),
            Err(error) => {
                return Err(CommandBatchFailure::retain(
                    error,
                    receipts,
                    command.clone(),
                    None,
                ));
            }
        }
    }
    Ok(receipts)
}

#[derive(Clone, Default)]
pub struct State {
    pub channel_mutes: [Option<bool>; 4],
}

pub fn handles(path: &str) -> bool {
    lighting::handles(path)
        || path.starts_with("/device/micSetup/")
        || path == "/device/micSetup"
        || path == "/device/micPreview"
        || [
            "outputMixerReducer",
            "playbackMixReducer",
            "streamMixReducer",
            "lineOutReducer",
            "voiceChatReducer",
        ]
        .iter()
        .any(|key| {
            path.starts_with(&format!("/device/{key}/")) && !path.ends_with("/showPercentage")
        })
        || path == "/device/mixerBind/bleepButton"
        || path.starts_with("/device/mixerBind/inputSource/")
        || path == "/device/fader/useMode"
}

/// Native GetInputOutputChannelPeakValue truncates float*100 before 0x35a30
/// computes 20*log10(percent/100), then maps -60..0 dB to integer percent.
pub fn meter_percent(sample: f32) -> anyhow::Result<i32> {
    ensure!(
        sample.is_finite() && sample >= 0.,
        "Mixer peak sample invalid"
    );
    let percent = (sample * 100.).trunc() as i32;
    if percent == 0 {
        return Ok(0);
    }
    Ok(((20. * (f64::from(percent) / 100.).log10() + 60.) / 60. * 100.).trunc() as i32)
}

fn num(v: &Value, key: &str) -> anyhow::Result<f64> {
    let value = v[key]
        .as_f64()
        .with_context(|| format!("Mic profile lacks numeric {key}"))?;
    ensure!(value.is_finite(), "Mic profile contains non-finite {key}");
    Ok(value)
}
fn yes(v: &Value, key: &str) -> anyhow::Result<bool> {
    v[key]
        .as_bool()
        .with_context(|| format!("Mic profile lacks boolean {key}"))
}
// JS Number(x.toFixed(0)), including negative ties away from zero.
fn n3(percent: bool, value: f64, min: f64, max: f64) -> anyhow::Result<f64> {
    ensure!(
        min.is_finite() && max.is_finite() && max >= min,
        "Mic range invalid"
    );
    if percent {
        ensure!(max > min, "Mic percent range empty");
        Ok(((value.clamp(min, max) - min) / (max - min) * 100.).round())
    } else {
        Ok(((max - min) * value / 100. + min).round())
    }
}
fn property(control: MixerControl, value: MixerValue) -> Command {
    Command::Property {
        target: MixerTarget {
            control,
            band: None,
            channel: MixerChannel::Both,
        },
        value,
    }
}
fn endpoint(key: &str, value: MixerValue) -> anyhow::Result<Command> {
    let (jack, flow) = match key {
        "chat" => (3, 0),
        "music" => (1, 0),
        "game" => (7, 0),
        "system" => (16, 0),
        "streamMixReducer" => (13, 1),
        "voiceChatReducer" => (10, 1),
        _ => anyhow::bail!("Unknown Mixer software endpoint {key}"),
    };
    Ok(Command::Endpoint { jack, flow, value })
}
fn gain_command(key: &str, value: f64) -> anyhow::Result<Command> {
    Ok(match key {
        "mic" | "xlr" => Command::InputMicGain { value },
        "optical" | "lineIn" | "playbackMixReducer" | "lineOutReducer" => Command::AnalogGain {
            key: key.into(),
            value,
        },
        _ => endpoint(
            key,
            MixerValue::Scalar {
                value: value as f32,
            },
        )?,
    })
}
fn mute_command(key: &str, enabled: bool) -> anyhow::Result<Command> {
    Ok(match key {
        "mic" | "xlr" => Command::InputMicMute { enabled },
        "optical" | "lineIn" | "playbackMixReducer" | "lineOutReducer" => Command::AnalogMute {
            key: key.into(),
            enabled,
        },
        _ => endpoint(key, MixerValue::Boolean { enabled })?,
    })
}
fn gain_values(v: &Value) -> anyhow::Result<(f64, f64, f64)> {
    let (value, min, max) = (num(v, "value")?, num(v, "min")?, num(v, "max")?);
    ensure!(
        max > min && (min..=max).contains(&value),
        "Mic gain outside source range"
    );
    Ok((value, min, max))
}
impl State {
    fn channel(&mut self, channel: u8, enabled: bool, out: &mut Vec<Command>) {
        if let Some(slot) = channel
            .checked_sub(1)
            .and_then(|i| self.channel_mutes.get_mut(i as usize))
        {
            if *slot != Some(enabled) {
                *slot = Some(enabled);
                out.push(Command::ChannelMute {
                    channel,
                    enabled,
                    profile_updates: Vec::new(),
                });
            }
        }
    }
    fn channel_routes(&mut self, device: &Value, out: &mut Vec<Command>) -> anyhow::Result<()> {
        let input = device["mixerBind"]["inputSource"]
            .as_object()
            .context("Mic profile lacks mixer input bindings")?;
        for (key, v) in input {
            let channel = key
                .strip_prefix("channel")
                .context("Mixer channel key invalid")?
                .parse::<u8>()?;
            let source = v.as_str().context("Mixer input binding invalid")?;
            let muted = |name: &str| device[name][source]["isMuted"].as_bool().unwrap_or(false);
            let mode = device["mixerBind"]["muteButton"][key]
                .as_str()
                .context("Mixer mute binding absent")?
                .to_ascii_lowercase();
            let enabled = match mode.as_str() {
                "muteall" => [
                    "playbackMixReducer",
                    "streamMixReducer",
                    "lineOutReducer",
                    "voiceChatReducer",
                ]
                .iter()
                .any(|x| muted(x)),
                "playbackmix" => muted("playbackMixReducer"),
                "streammix" => muted("streamMixReducer"),
                "lineout" => muted("lineOutReducer"),
                "voicechat" => muted("voiceChatReducer"),
                _ => false,
            };
            self.channel(channel, enabled, out);
        }
        Ok(())
    }
    fn output_gain(
        &mut self,
        device: &mut Value,
        key: &str,
        gain: &Value,
        out: &mut Vec<Command>,
    ) -> anyhow::Result<()> {
        let (value, min, max) = gain_values(gain)?;
        let muted = value == min;
        out.push(mute_command(key, muted)?);
        // Fi passes lowercase linein to Yi; retain the source spelling.
        let routing_key = match key {
            "xlr" => "mic",
            "lineIn" => "linein",
            _ => key,
        };
        let input = device["mixerBind"]["inputSource"]
            .as_object()
            .context("Mic profile lacks mixer input bindings")?
            .clone();
        for (channel, source) in input {
            if source != routing_key {
                continue;
            }
            let id = channel
                .strip_prefix("channel")
                .context("Mixer channel key invalid")?
                .parse::<u8>()?;
            let changed = id >= 1 && id <= 4 && self.channel_mutes[id as usize - 1] != Some(muted);
            self.channel(id, muted, out);
            if changed {
                let mode = device["mixerBind"]["muteButton"][&channel]
                    .as_str()
                    .context("Mixer mute binding absent")?
                    .to_ascii_lowercase();
                for (name, tag) in [
                    ("playbackMixReducer", "playbackmix"),
                    ("streamMixReducer", "streammix"),
                    ("lineOutReducer", "lineout"),
                    ("voiceChatReducer", "voicechat"),
                ] {
                    if (mode == "muteall" || mode == tag) && device[name][routing_key].is_object() {
                        device[name][routing_key]["isMuted"] = json!(muted);
                        if let Some(Command::ChannelMute {
                            profile_updates, ..
                        }) = out.last_mut()
                        {
                            profile_updates.push((
                                format!("/device/{name}/{routing_key}/isMuted"),
                                json!(muted),
                            ));
                        }
                    }
                }
            }
        }
        out.push(gain_command(
            key,
            if key == "xlr" {
                ((value - min) / (max - min) * 40. * 100.).round() / 100.
            } else {
                value
            },
        )?);
        Ok(())
    }
    pub fn compile(&mut self, path: &str, profile: &Value) -> anyhow::Result<Vec<Command>> {
        ensure!(
            handles(path),
            "Unrecognized local Mixer source action {path}"
        );
        if lighting::handles(path) {
            return lighting::compile(path, profile);
        }
        if path.starts_with("/device/mixerBind/inputSource/") {
            return Ok(Vec::new());
        }
        let mut d = profile
            .get("device")
            .context("Mic profile device absent")?
            .clone();
        let setup = d["micSetup"].clone();
        let mode = setup["useMode"].as_u64().context("Mic setup mode absent")?;
        ensure!(mode <= 1, "Mic setup mode outside source enum");
        let key = if mode == 0 { "xlr" } else { "mic" };
        let mut out = Vec::new();
        if path == "/device/micSetup" || path.starts_with("/device/micSetup/") {
            let (value, min, max) = gain_values(&setup[key])?;
            let percent = n3(true, value, min, max)?;
            if mode == 0 {
                let enabled = yes(&setup["xlr"], "isEnabled")?;
                out.push(Command::MicSetup {
                    mode: if enabled { 2 } else { 1 },
                });
                out.push(Command::PhantomPower { enabled });
            } else {
                out.push(Command::MicSetup { mode: 3 });
            }
            // UI taskMaker persists its gain copy before this call; restoration
            // does not copy. Always use the supplied retained profile for bi.
            let (output, output_min, output_max) = gain_values(&d["outputMixerReducer"][key])?;
            let output_percent = n3(true, output, output_min, output_max)?;
            let lower = if mode == 0 { -12. } else { 0. };
            let ceiling = n3(false, percent, lower, 39.75)?;
            out.push(Command::InputMicGain {
                value: n3(false, output_percent, lower, ceiling)?,
            });
            if d["micPreview"] == true {
                self.output_gain(&mut d, key, &setup[key], &mut out)?;
            }
        } else if path == "/device/micPreview" {
            let preview = yes(&d, "micPreview")?;
            let mut gain = d["outputMixerReducer"][key].clone();
            if preview {
                gain["value"] = setup[key]["value"].clone();
            }
            self.output_gain(&mut d, key, &gain, &mut out)?;
            let route = if preview {
                true
            } else {
                self.channel_routes(&d, &mut out)?;
                yes(&d["playbackMixReducer"]["mic"], "isEnabled")?
                    && !yes(&d["playbackMixReducer"]["mic"], "isMuted")?
            };
            out.push(property(
                MixerControl::MicrophoneToHeadphones,
                MixerValue::Boolean { enabled: route },
            ));
            let monitor = if preview {
                -9.
            } else {
                (num(&d["micMonitor"], "value")? * 45. / 100. - 45.).floor()
            };
            ensure!(
                (-45. ..=0.).contains(&monitor),
                "Mic monitor outside source range"
            );
            out.push(property(
                MixerControl::PageMicMonitorVolume,
                MixerValue::Scalar {
                    value: monitor as f32,
                },
            ));
        } else if path.starts_with("/device/outputMixerReducer/") {
            let selected = path.split('/').nth(3).context("Output gain key absent")?;
            let gain = d["outputMixerReducer"][selected].clone();
            self.output_gain(&mut d, selected, &gain, &mut out)?;
        } else if path == "/device/mixerBind/bleepButton" {
            let mode = d["mixerBind"]["bleepButton"]
                .as_str()
                .context("Mixer bleep binding absent")?;
            out.push(Command::Bleep {
                enabled: mode.eq_ignore_ascii_case("bleep"),
            });
        } else if path == "/device/fader/useMode" {
            let mode = d["fader"]["useMode"]
                .as_u64()
                .context("Fader mode absent")?;
            ensure!(mode <= 1, "Fader source mode outside enum");
            for effect in d["fader"]["quickEffects"]
                .as_array()
                .context("Fader effect regions absent")?
            {
                out.push(Command::Fader {
                    region: u8::try_from(
                        effect["regionId"].as_u64().context("Fader region absent")?,
                    )?,
                    mode: mode as u8 + 1,
                });
            }
        } else {
            let reducer = path
                .split('/')
                .nth(2)
                .context("Mixer reducer path absent")?;
            let selected = path.split('/').nth(3).context("Mixer route path absent")?;
            if selected == "volume" {
                let gain = &d[reducer]["volume"];
                let (value, min, _) = gain_values(gain)?;
                let muted = !yes(gain, "isEnabled")?;
                out.push(mute_command(reducer, muted)?);
                if !muted {
                    out.push(mute_command(reducer, value == min)?);
                }
                out.push(gain_command(reducer, value)?);
                return Ok(out);
            }
            self.channel_routes(&d, &mut out)?;
            let route = &d[reducer][selected];
            let input = match selected {
                "mic" => 0,
                "chat" => 1,
                "music" => 2,
                "game" => 3,
                "optical" => 4,
                "lineIn" => 5,
                "system" => 6,
                _ => anyhow::bail!("Unknown source routing input {selected}"),
            };
            let output = match reducer {
                "playbackMixReducer" => 0,
                "streamMixReducer" => 1,
                "lineOutReducer" => 2,
                "voiceChatReducer" => 3,
                _ => anyhow::bail!("Unknown source routing output {reducer}"),
            };
            out.push(Command::Route {
                input,
                output,
                enabled: yes(route, "isEnabled")? && !yes(route, "isMuted")?,
            });
        }
        Ok(out)
    }
}

/// Executes fixed source reports without introducing extra getters. `control`
/// must preserve exact report size, unlike hidapi's padded Windows backend.
pub fn execute(
    device: &dyn FeatureTransport,
    command: &Command,
    validate: &dyn Fn() -> anyhow::Result<()>,
    control: &dyn Fn(&[u8]) -> anyhow::Result<()>,
) -> anyhow::Result<Receipt> {
    validate()?;
    let mut response = Value::Null;
    match command {
        Command::LightingSample { .. } => {
            anyhow::bail!("Mixer lighting peak sampling requires the local audio endpoint adapter")
        }
        Command::Lighting { action } => {
            response = lighting::execute(device, action, validate)?;
        }
        Command::Endpoint { .. } | Command::Route { .. } => {
            anyhow::bail!("Mixer endpoint/matrix command requires its local OS adapter")
        }
        Command::AnalogGain { .. }
        | Command::AnalogMute { .. }
        | Command::Bleep { .. }
        | Command::Fader { .. } => {
            response = reports::execute(device, command, validate)?;
        }
        Command::MicSetup { mode } => {
            ensure!((1..=3).contains(mode), "Mic setup native enum invalid");
            control(&[10, u8::from(*mode == 2) | (2 * (mode & 3))])?;
        }
        Command::PhantomPower { enabled } => {
            let input = reports::output(
                device,
                &[10, if *enabled { 5 } else { 2 }],
                Some(18),
                validate,
            )?
            .context("48V source command did not return an Input report")?;
            response = json!({"input":input});
        }
        Command::InputMicGain { value } => {
            ensure!(
                value.is_finite() && (-12. ..=40.).contains(value),
                "Mic source gain outside range"
            );
            let raw = ((value + 12.) / 51.75 * 69.).trunc() as u8;
            reports::output(device, &[3, 24, 0, 192, 16, raw, raw], None, validate)?;
        }
        Command::InputMicMute { enabled } => {
            reports::output(
                device,
                &[
                    3,
                    24,
                    0,
                    192,
                    148,
                    if *enabled { 196 } else { 68 },
                    if *enabled { 192 } else { 64 },
                ],
                None,
                validate,
            )?;
        }
        Command::ChannelMute {
            channel, enabled, ..
        } => {
            ensure!(
                (1..=5).contains(channel),
                "Mixer channel outside source table"
            );
            let mut report = [0; 64];
            report[..12].copy_from_slice(&[
                7,
                0,
                31,
                0,
                0,
                0,
                3,
                8,
                16,
                0,
                *channel,
                u8::from(*enabled),
            ]);
            reports::feature(device, &report, None, validate)?;
        }
        Command::Property { target, value } => {
            response =
                serde_json::to_value(MixerSession::new(device, validate)?.apply(target, value)?)?;
        }
    }
    let receipt = Receipt {
        command: command.clone(),
        transport_completed: true,
        response,
        profile_updates: match command {
            Command::ChannelMute {
                profile_updates, ..
            } => profile_updates.clone(),
            _ => Vec::new(),
        },
    };
    validate().map_err(|error| {
        error.context(CommandFailure {
            completed: receipt.clone(),
        })
    })?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn setup_rounds_each_source_stage() {
        let profile = json!({"device":{"micSetup":{"useMode":0,"xlr":{"value":68,"min":0,"max":68,"isEnabled":true},"mic":{"value":0,"min":0,"max":40}},"micPreview":false,"outputMixerReducer":{"xlr":{"value":68,"min":0,"max":68}}}});
        let plan = State::default()
            .compile("/device/micSetup", &profile)
            .unwrap();
        assert!(matches!(plan[0], Command::MicSetup { mode: 2 }));
        assert!(matches!(plan[2], Command::InputMicGain { value: 40. }));
    }
    #[test]
    fn source_channel_cache_has_no_fabricated_initial_observation() {
        let mut state = State::default();
        let mut out = Vec::new();
        state.channel(1, false, &mut out);
        state.channel(1, false, &mut out);
        state.channel(1, true, &mut out);
        assert_eq!(out.len(), 2);
    }
}
