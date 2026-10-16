//! Current Seiren 1422/1446 ParametricEQFeature and Marie_FW Protocol25 tasks.
//! Storage/optimistic reducers are caller-owned; acknowledgements are retained
//! separately from the source feature's sometimes unchecked return value.
use crate::{backend::FeatureTransport, device_reads::DeviceReadCapability, feature_exchange};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[path = "seiren_microphone_dsp.rs"]
mod microphone_dsp;
#[path = "seiren_eq_native.rs"]
mod native;
pub use native::{
    Level, NativeLevels, NativeOperation, NativeReply, NativeStage, NativeTransport,
    execute_native, execute_native_in_session,
};

pub fn capability(product_id: u32) -> Option<&'static DeviceReadCapability> {
    static CAPS: OnceLock<Vec<DeviceReadCapability>> = OnceLock::new();
    CAPS.get_or_init(|| crate::data::seiren_eq_data::seiren_eq_data!())
        .iter()
        .find(|cap| cap.product_id == product_id)
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Band {
    #[serde(default)]
    direction: u8,
    band: u8,
    #[serde(default = "enabled_band")]
    band_status: u8,
    freq: f64,
    gain: f64,
    q_factor: f64,
    filter_type: u8,
}
fn enabled_band() -> u8 {
    1
}
impl Band {
    pub fn band(&self) -> u8 {
        self.band
    }
    pub fn band_status(&self) -> u8 {
        self.band_status
    }
    fn wire(&self, profile: u8) -> anyhow::Result<[u8; 9]> {
        self.validate()?;
        let frequency = (self.freq as u16).to_be_bytes();
        Ok([
            profile,
            0,
            self.band,
            self.band_status,
            frequency[0],
            frequency[1],
            (self.gain * 10.).round() as i8 as u8,
            (self.q_factor * 10.) as u8,
            self.filter_type,
        ])
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.direction == 0 && self.band <= 7 && self.band_status <= 1,
            "Seiren EQ band identity/direction/status differs from the mounted UI"
        );
        ensure!(
            self.freq.is_finite()
                && (20. ..=20000.).contains(&self.freq)
                && (1..=5).contains(&self.filter_type),
            "Seiren EQ frequency/filter differs from the mounted choices"
        );
        ensure!(
            self.gain.is_finite()
                && (-12. ..=12.).contains(&self.gain)
                && (self.gain * 10. - (self.gain * 10.).round()).abs() <= 0.0001,
            "Seiren EQ gain must be -12..12 in tenths"
        );
        ensure!(
            self.q_factor.is_finite()
                && (0.3..=15.).contains(&self.q_factor)
                && (self.q_factor * 10. - (self.q_factor * 10.).round()).abs() <= 0.0001,
            "Seiren EQ Q factor must be 0.3..15 in tenths"
        );
        Ok(())
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Effect {
    #[serde(alias = "enabled")]
    is_enabled: bool,
    #[serde(deserialize_with = "source_byte")]
    value: u8,
}
fn source_byte<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u8, D::Error> {
    let value = f64::deserialize(deserializer)?;
    if value.is_finite() && (0. ..=255.).contains(&value) && value.fract() == 0. {
        Ok(value as u8)
    } else {
        Err(serde::de::Error::custom(
            "Source numeric choice must be a whole byte",
        ))
    }
}
impl Effect {
    pub fn new(is_enabled: bool, value: u8) -> Self {
        Self { is_enabled, value }
    }
    pub fn is_enabled(&self) -> bool {
        self.is_enabled
    }
    pub fn value(&self) -> u8 {
        self.value
    }
    fn effective(&self) -> u8 {
        if self.is_enabled { self.value } else { 0 }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    eq_name: String,
    parametric_eq_settings: Vec<Band>,
    high_pass_filter: Effect,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    vocal_bass: Option<Effect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    vocal_exciter: Option<Effect>,
}
impl Preset {
    pub fn eq_name(&self) -> &str {
        &self.eq_name
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "operation", content = "parameters", rename_all = "snake_case")]
pub enum Change {
    Band(Band),
    Preset(Preset),
    HighPass(Effect),
    VocalBass(Effect),
    VocalExciter(Effect),
    DeEsser(Effect),
    MicAgc(bool),
    DialControl(u8),
    RestoreDialControl(u8),
    MicVolume(Level),
    HeadphonesVolume(Level),
    MonitorVolume(Level),
    ReadVolumes,
    SamplingRate(u32),
    ReturnSwitch(bool),
    MicrophoneDsp {
        module: u8,
        payload: serde_json::Value,
    },
}
impl Change {
    pub fn key(&self) -> String {
        match self {
            Self::Band(band) => format!("band:{}", band.band),
            Self::Preset(_) => "preset".into(),
            Self::HighPass(_) => "high_pass".into(),
            Self::VocalBass(_) => "vocal_bass".into(),
            Self::VocalExciter(_) => "vocal_exciter".into(),
            Self::DeEsser(_) => "de_esser".into(),
            Self::MicAgc(_) => "mic_agc".into(),
            Self::DialControl(_) | Self::RestoreDialControl(_) => "dial_control".into(),
            Self::MicVolume(_) => "mic_volume".into(),
            Self::HeadphonesVolume(_) => "headphones_volume".into(),
            Self::MonitorVolume(_) => "monitor_volume".into(),
            Self::ReadVolumes => "read_volumes".into(),
            Self::SamplingRate(_) => "sampling_rate".into(),
            Self::ReturnSwitch(_) => "mic_return_switch".into(),
            Self::MicrophoneDsp { module, .. } => format!("mic_dsp:{module}"),
        }
    }
    pub fn from_source(operation: &str, payload: serde_json::Value) -> anyhow::Result<Self> {
        Ok(match operation {
            "ON_SET_PARAMETRIC_EQ_SETTINGS" => Self::Band(serde_json::from_value(
                payload.get("parametricEQ").cloned().unwrap_or(payload),
            )?),
            "ON_SET_ACTIVE_PARAMETRIC_EQ" => Self::Preset(serde_json::from_value(
                payload.get("parametricEQ").cloned().unwrap_or(payload),
            )?),
            "ON_SET_HIGH_PASS_FILTER" => Self::HighPass(serde_json::from_value(
                payload
                    .get("highPassFilterInPEQ")
                    .cloned()
                    .unwrap_or(payload),
            )?),
            "ON_SET_VOCAL_BASS" => Self::VocalBass(serde_json::from_value(
                payload.get("vocalBass").cloned().unwrap_or(payload),
            )?),
            "ON_SET_VOCAL_EXCITER" => Self::VocalExciter(serde_json::from_value(
                payload.get("vocalExciter").cloned().unwrap_or(payload),
            )?),
            "ON_SET_MIC_DEESSER" => Self::DeEsser(serde_json::from_value(
                payload.get("micDeEsser").cloned().unwrap_or(payload),
            )?),
            "ON_SET_MIC_AGC" => {
                let value = payload.get("micAGC").unwrap_or(&payload);
                let enabled = value.get("isEnabled").unwrap_or(value);
                Self::MicAgc(enabled == true || enabled == 1 || enabled == "true" || enabled == "1")
            }
            "ON_SET_DIAL_CONTROL" => Self::DialControl(serde_json::from_value(
                payload.get("dialControl").cloned().unwrap_or(payload),
            )?),
            "ON_SET_MIC_VOLUME_ENABLED" | "ON_SET_MIC_VOLUME_VALUE" => Self::MicVolume(
                serde_json::from_value(payload.get("micVolume").cloned().unwrap_or(payload))?,
            ),
            "ON_CHANGE_VOLUME_ENABLED" | "ON_CHANGE_VOLUME_VALUE" => Self::HeadphonesVolume(
                serde_json::from_value(payload.get("volume").cloned().unwrap_or(payload))?,
            ),
            "ON_SET_MIC_SIDE_TONE" => Self::MonitorVolume(serde_json::from_value(
                payload.get("micSideTone").cloned().unwrap_or(payload),
            )?),
            "ON_SET_SAMPLING_RATE_VALUE" => Self::SamplingRate(serde_json::from_value(
                payload.get("samplingRate").cloned().unwrap_or(payload),
            )?),
            "ON_MIC_RETURN_SWITCH" => {
                let state = payload
                    .get("switchstate")
                    .and_then(serde_json::Value::as_u64)
                    .context("Microphone return switch state is absent or not an integer")?;
                ensure!(
                    state <= 1,
                    "Microphone return switch state exceeds source choices"
                );
                Self::ReturnSwitch(state == 1)
            }
            "ON_SET_NOISE_GATE"
            | "ON_SET_AI_NOISE_SUPPRESSION"
            | "ON_SET_MIC_COMPRESSOR"
            | "ON_SET_MIC_LIMITER"
            | "ON_SET_MIC_REVERB" => {
                let (module, key) = match operation {
                    "ON_SET_NOISE_GATE" => (2, "noiseGate"),
                    "ON_SET_AI_NOISE_SUPPRESSION" => (3, "AINoiseSuppression"),
                    "ON_SET_MIC_COMPRESSOR" => (4, "micCompressor"),
                    "ON_SET_MIC_LIMITER" => (5, "micLimiter"),
                    _ => (6, "micReverb"),
                };
                Self::MicrophoneDsp {
                    module,
                    payload: payload.get(key).cloned().unwrap_or(payload),
                }
            }
            _ => anyhow::bail!("Seiren EQ operation has no current source caller: {operation}"),
        })
    }
    pub fn requires_native(&self) -> bool {
        matches!(
            self,
            Self::MicVolume(_)
                | Self::HeadphonesVolume(_)
                | Self::MonitorVolume(_)
                | Self::ReadVolumes
                | Self::SamplingRate(_)
        )
    }
    pub fn persists_local_settings(&self) -> bool {
        !matches!(
            self,
            Self::MicVolume(_)
                | Self::HeadphonesVolume(_)
                | Self::ReadVolumes
                | Self::ReturnSwitch(_)
        )
    }
}

// Current 23374 mapping and 26212 preset constants: custom is 16, not 11.
pub fn preset_ids(name: &str) -> anyhow::Result<(u8, u8)> {
    Ok(match name {
        "podcast" => (6, 1),
        "arena" => (7, 2),
        "fpscallout" => (8, 3),
        "studio" => (9, 4),
        "instrument" => (10, 5),
        "custom" => (16, 6),
        _ => anyhow::bail!("Unknown current Seiren EQ preset: {name}"),
    })
}
fn preset_profile(id: u8) -> anyhow::Result<u8> {
    match id {
        6..=10 => Ok(id - 5),
        16 => Ok(6),
        _ => anyhow::bail!("Device EQ preset {id} has no source profile mapping"),
    }
}
pub fn validate_change(product_id: u32, change: &Change) -> anyhow::Result<()> {
    capability(product_id).context("Seiren EQ product source is absent")?;
    match change {
        Change::Band(band) => band.validate()?,
        Change::Preset(preset) => {
            preset_ids(&preset.eq_name)?;
            ensure!(
                preset.parametric_eq_settings.len() <= 8,
                "Seiren EQ preset has more than eight UI bands"
            );
            let mut bands = std::collections::BTreeSet::new();
            for band in &preset.parametric_eq_settings {
                band.validate()?;
                ensure!(bands.insert(band.band), "Duplicate Seiren EQ band");
            }
            ensure!(
                preset.high_pass_filter.value <= 5,
                "Seiren HPF state exceeds source range"
            );
            for effect in [&preset.vocal_bass, &preset.vocal_exciter]
                .into_iter()
                .flatten()
            {
                ensure!(
                    effect.value <= 100,
                    "Seiren vocal amount exceeds source range"
                );
            }
        }
        Change::HighPass(effect) => {
            ensure!(effect.value <= 5, "Seiren HPF state exceeds source range")
        }
        Change::VocalBass(effect) | Change::VocalExciter(effect) | Change::DeEsser(effect) => {
            ensure!(
                effect.value <= 100,
                "Seiren EQ effect amount exceeds source range"
            )
        }
        Change::MicAgc(_) | Change::ReadVolumes => {}
        Change::SamplingRate(rate) => ensure!(
            matches!(rate, 48_000 | 96_000),
            "Sampling rate differs from current source choices"
        ),
        Change::ReturnSwitch(_) => {}
        Change::MicrophoneDsp { module, payload } => microphone_dsp::validate(*module, payload)?,
        Change::DialControl(value) | Change::RestoreDialControl(value) => {
            ensure!(
                *value <= 1,
                "Seiren dial selection exceeds current microphone/headphone choices"
            )
        }
        Change::MicVolume(level)
        | Change::HeadphonesVolume(level)
        | Change::MonitorVolume(level) => level.validate()?,
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Stage {
    command: [u8; 3],
    transaction: u8,
    setter: bool,
    sent: bool,
    response: Option<Vec<u8>>,
    error: Option<String>,
}
impl Stage {
    pub fn command(&self) -> [u8; 3] {
        self.command
    }
    pub fn response(&self) -> Option<&[u8]> {
        self.response.as_deref()
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Report {
    requested: Change,
    stages: Vec<Stage>,
    source_result: bool,
    #[serde(default)]
    native_stages: Vec<NativeStage>,
    #[serde(default)]
    source_skipped: Option<String>,
    #[serde(default)]
    native_levels: Option<NativeLevels>,
}
impl Report {
    pub fn requested(&self) -> &Change {
        &self.requested
    }
    pub fn stages(&self) -> &[Stage] {
        &self.stages
    }
    pub fn source_result(&self) -> bool {
        self.source_result
    }
    pub fn confirmed(&self) -> bool {
        self.source_result
            && self.source_skipped.is_none()
            && self.has_stages()
            && self
                .stages
                .iter()
                .all(|stage| stage.error.is_none() && stage.response.is_some())
            && self.native_stages.iter().all(NativeStage::confirmed)
    }
    pub fn setter_sent(&self) -> bool {
        self.stages.iter().any(|stage| stage.setter && stage.sent)
            || self.native_stages.iter().any(NativeStage::setter_sent)
    }
    pub fn native_stages(&self) -> &[NativeStage] {
        &self.native_stages
    }
    pub fn has_stages(&self) -> bool {
        !self.stages.is_empty() || !self.native_stages.is_empty()
    }
    pub fn source_skipped(&self) -> Option<&str> {
        self.source_skipped.as_deref()
    }
    pub fn native_levels(&self) -> Option<&NativeLevels> {
        self.native_levels.as_ref()
    }
}

struct Executor<'a, N, V> {
    device: &'a dyn FeatureTransport,
    cap: &'a DeviceReadCapability,
    next: N,
    validate: V,
    report: Report,
}
impl<N: FnMut() -> anyhow::Result<u8>, V: Fn() -> anyhow::Result<()>> Executor<'_, N, V> {
    fn stage(
        &mut self,
        command: [u8; 3],
        payload: &[u8],
        min: usize,
        setter: bool,
    ) -> anyhow::Result<Option<Vec<u8>>> {
        (self.validate)()?;
        let transaction = (self.next)()?;
        let mut sent = false;
        let reply = feature_exchange::exchange_with_report_data_size_observing_send(
            self.device,
            self.cap,
            command,
            payload,
            transaction,
            command[0],
            &self.validate,
            &mut || sent = true,
        )
        .and_then(|data| {
            ensure!(
                data.len() >= min,
                "Seiren EQ response is truncated for command {command:?}"
            );
            Ok(data)
        });
        // A cancellation/deadline/physical change always stops the task, even
        // when the source feature ignores a particular command's return value.
        (self.validate)()?;
        let response = reply.as_ref().ok().cloned();
        self.report.stages.push(Stage {
            command,
            transaction,
            setter,
            sent,
            response: response.clone(),
            error: reply.err().map(|error| format!("{error:#}")),
        });
        Ok(response)
    }
    fn current_profile(&mut self) -> anyhow::Result<Option<u8>> {
        let Some(data) = self.stage([2, 8, 130], &[0, 0], 2, false)? else {
            return Ok(None);
        };
        match preset_profile(data[1]) {
            Ok(profile) => Ok(Some(profile)),
            Err(error) => {
                self.report.stages.last_mut().expect("read stage").error = Some(error.to_string());
                Ok(None)
            }
        }
    }
    fn band(&mut self, profile: u8, band: &Band) -> anyhow::Result<bool> {
        Ok(self
            .stage([9, 8, 26], &band.wire(profile)?, 9, true)?
            .is_some())
    }
    fn vocal(&mut self, profile: u8, effect: &Effect, exciter: bool) -> anyhow::Result<bool> {
        // Marie_FW: mappingVocalGain(value) = signed tenths of (value/10 - 5).
        let gain = (i16::from(effect.value) - 50) as i8 as u8;
        let (band, freq, filter) = if exciter { (9, 6000u16, 4) } else { (8, 300u16, 3) };
        let freq = freq.to_be_bytes();
        Ok(self
            .stage(
                [9, 8, 26],
                &[
                    profile,
                    0,
                    band,
                    u8::from(effect.is_enabled),
                    freq[0],
                    freq[1],
                    gain,
                    7,
                    filter,
                ],
                9,
                true,
            )?
            .is_some())
    }
    fn high_pass(&mut self, effect: &Effect) -> anyhow::Result<bool> {
        Ok(self
            .stage([2, 8, 21], &[1, effect.effective()], 2, true)?
            .is_some())
    }
}
pub fn execute(
    product_id: u32,
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    change: &Change,
    next: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Report> {
    validate_change(product_id, change)?;
    ensure!(
        cap.product_id == product_id && cap.source_class == "rzDevice25AudioMarieT1",
        "Seiren EQ transport product/class differs"
    );
    let mut executor = Executor {
        device,
        cap,
        next,
        validate,
        report: Report {
            requested: change.clone(),
            stages: Vec::new(),
            source_result: false,
            native_stages: Vec::new(),
            source_skipped: None,
            native_levels: None,
        },
    };
    let result = match change {
        Change::ReturnSwitch(enabled) => {
            executor.stage([2, 8, 34], &[0, u8::from(*enabled)], 2, true)?;
            // Marie_FW returns true after awaiting this call even for a
            // negative firmware acknowledgement; keep that ACK separately.
            true
        }
        Change::MicrophoneDsp { module, payload } => {
            microphone_dsp::execute(&mut executor, *module, payload)?
        }
        Change::Band(band) => {
            if let Some(profile) = executor.current_profile()? {
                executor.band(profile, band)?
            } else {
                false
            }
        }
        Change::VocalBass(effect) | Change::VocalExciter(effect) => {
            if let Some(profile) = executor.current_profile()? {
                executor.vocal(profile, effect, matches!(change, Change::VocalExciter(_)))?
            } else {
                false
            }
        }
        Change::HighPass(effect) => {
            executor.high_pass(effect)?;
            true
        }
        Change::DeEsser(effect) => executor
            .stage([2, 8, 30], &[1, effect.effective()], 0, true)?
            .is_some(),
        Change::MicAgc(enabled) => executor
            .stage([3, 8, 22], &[1, 1, u8::from(*enabled)], 3, true)?
            .is_some(),
        Change::DialControl(value) => executor.stage([2, 8, 24], &[1, *value], 2, true)?.is_some(),
        Change::RestoreDialControl(value) => {
            // DialControlFeature.restoreDeviceData compares the current device
            // value. A failed getter still attempts the source setter; retain
            // the getter error independently from that setter's return value.
            let observed = executor.stage([2, 8, 152], &[1, 0], 2, false)?;
            if observed.as_ref().is_some_and(|data| data[1] == *value) {
                true
            } else {
                executor.stage([2, 8, 24], &[1, *value], 2, true)?.is_some()
            }
        }
        Change::MicVolume(_)
        | Change::HeadphonesVolume(_)
        | Change::MonitorVolume(_)
        | Change::ReadVolumes => {
            anyhow::bail!("Seiren native audio task was sent to the HID executor")
        }
        Change::SamplingRate(_) => {
            anyhow::bail!("Seiren native sampling task was sent to HID executor")
        }
        Change::Preset(preset) => {
            let (id, profile) = preset_ids(&preset.eq_name)?;
            executor.stage([11, 8, 33], &[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 11, true)?;
            executor.stage([2, 8, 2], &[1, id], 2, true)?;
            for band in &preset.parametric_eq_settings {
                executor.band(profile, band)?;
            }
            let bass = if let Some(effect) = &preset.vocal_bass {
                executor.vocal(profile, effect, false)?
            } else {
                true
            };
            let exciter = if bass {
                if let Some(effect) = &preset.vocal_exciter {
                    executor.vocal(profile, effect, true)?
                } else {
                    true
                }
            } else {
                false
            };
            if bass && exciter {
                executor.high_pass(&preset.high_pass_filter)?;
                true
            } else {
                false
            }
        }
    };
    executor.report.source_result = result;
    (executor.validate)()?;
    Ok(executor.report)
}

#[cfg(test)]
#[path = "seiren_eq_tests.rs"]
mod tests;
