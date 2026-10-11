//! Current CmMixerLib DSP and hardware endpoint reports, independent of OS/DLL.
//! Recipes are source-gated by tools/audit-cmmixer-controls-current.py.
use crate::backend::{FeatureTransport, ReportLengths};
use anyhow::{Context as _, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::OnceLock, thread, time::Duration};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MixerControl {
    DspFirmware,
    EqEnabled,
    PageEqEnabled,
    EqBand,
    MagicVoiceEnabled,
    PageMagicVoiceEnabled,
    MagicVoice,
    /// Current AudioMixer JS fixed-template setter. Native MagicVoice retains
    /// its separately recovered read/modify/write codec.
    PageMagicVoice,
    EchoReverbEnabled,
    PageEchoReverbEnabled,
    ReverbRoom,
    PageReverbRoom,
    ReverbDecay,
    PageReverbDecay,
    EchoGain,
    PageEchoGain,
    EchoDelay,
    PageEchoDelay,
    NoiseGateEnabled,
    NoiseGateThreshold,
    NoiseGateTargetGain,
    NoiseGateAttack,
    NoiseGateRelease,
    CompressorEnabled,
    CompressorThreshold,
    CompressorKnee,
    CompressorRatio,
    CompressorMakeupGain,
    CompressorAttack,
    CompressorRelease,
    KeyShift,
    PageKeyShift,
    VocalFadingEnabled,
    PageVocalFadingEnabled,
    VocalFadingLevel,
    PageVocalFadingLevel,
    MicMonitorVolume,
    /// Mounted 1342 JS copies its first two query data bytes into template
    /// positions 7/8. This differs from the native property's bit-preserving RMW.
    PageMicMonitorVolume,
    HeadphonesVolume,
    HeadphonesMuted,
    HeadphonesPeak,
    LineOutVolume,
    LineOutMuted,
    LineOutPeak,
    MicrophoneVolume,
    MicrophoneMuted,
    MicrophonePeak,
    LineInVolume,
    LineInMuted,
    LineInPeak,
    ConsoleVolume,
    ConsoleMuted,
    ConsolePeak,
    MicrophoneToHeadphones,
    MicrophoneToLineOut,
    ConsoleToHeadphones,
    ConsoleToLineOut,
    LineInToHeadphones,
    LineInToLineOut,
}

impl MixerControl {
    fn key(self) -> &'static str {
        match self {
            Self::DspFirmware => "dsp_firmware",
            Self::EqEnabled | Self::PageEqEnabled => "eq_enabled",
            Self::EqBand => "eq_band",
            Self::MagicVoiceEnabled => "magic_voice_enabled",
            Self::PageMagicVoiceEnabled => "magic_voice_enabled",
            Self::MagicVoice => "magic_voice",
            Self::PageMagicVoice => "magic_voice",
            Self::EchoReverbEnabled => "echo_reverb_enabled",
            Self::PageEchoReverbEnabled => "echo_reverb_enabled",
            Self::ReverbRoom => "reverb_room",
            Self::PageReverbRoom => "reverb_room",
            Self::ReverbDecay => "reverb_decay",
            Self::PageReverbDecay => "reverb_decay",
            Self::EchoGain => "echo_gain",
            Self::PageEchoGain => "echo_gain",
            Self::EchoDelay => "echo_delay",
            Self::PageEchoDelay => "echo_delay",
            Self::NoiseGateEnabled => "noise_gate_enabled",
            Self::NoiseGateThreshold => "noise_gate_threshold",
            Self::NoiseGateTargetGain => "noise_gate_target_gain",
            Self::NoiseGateAttack => "noise_gate_attack",
            Self::NoiseGateRelease => "noise_gate_release",
            Self::CompressorEnabled => "compressor_enabled",
            Self::CompressorThreshold => "compressor_threshold",
            Self::CompressorKnee => "compressor_knee",
            Self::CompressorRatio => "compressor_ratio",
            Self::CompressorMakeupGain => "compressor_makeup_gain",
            Self::CompressorAttack => "compressor_attack",
            Self::CompressorRelease => "compressor_release",
            Self::KeyShift => "key_shift",
            Self::PageKeyShift => "key_shift",
            Self::VocalFadingEnabled => "vocal_fading_enabled",
            Self::PageVocalFadingEnabled => "vocal_fading_enabled",
            Self::VocalFadingLevel => "vocal_fading_level",
            Self::PageVocalFadingLevel => "vocal_fading_level",
            Self::MicMonitorVolume => "mic_monitor_volume",
            Self::PageMicMonitorVolume => "mic_monitor_volume",
            Self::HeadphonesVolume => "headphones_volume",
            Self::HeadphonesMuted => "headphones_muted",
            Self::HeadphonesPeak => "headphones_peak",
            Self::LineOutVolume => "line_out_volume",
            Self::LineOutMuted => "line_out_muted",
            Self::LineOutPeak => "line_out_peak",
            Self::MicrophoneVolume => "microphone_volume",
            Self::MicrophoneMuted => "microphone_muted",
            Self::MicrophonePeak => "microphone_peak",
            Self::LineInVolume => "line_in_volume",
            Self::LineInMuted => "line_in_muted",
            Self::LineInPeak => "line_in_peak",
            Self::ConsoleVolume => "console_volume",
            Self::ConsoleMuted => "console_muted",
            Self::ConsolePeak => "console_peak",
            Self::MicrophoneToHeadphones => "microphone_to_headphones",
            Self::MicrophoneToLineOut => "microphone_to_line_out",
            Self::ConsoleToHeadphones => "console_to_headphones",
            Self::ConsoleToLineOut => "console_to_line_out",
            Self::LineInToHeadphones => "line_in_to_headphones",
            Self::LineInToLineOut => "line_in_to_line_out",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MixerTarget {
    pub control: MixerControl,
    /// Required only for EqBand; the native table accepts indexes 0..9.
    pub band: Option<u8>,
    /// Original VolumeControl channel argument. No inferred left/right naming:
    /// native 0/1 byte order differs between the Console and other endpoints.
    #[serde(default)]
    pub channel: MixerChannel,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MixerChannel {
    #[default]
    Both,
    Channel0,
    Channel1,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MixerValue {
    Boolean {
        enabled: bool,
    },
    Scalar {
        value: f32,
    },
    /// Raw first field and signed gain in the original native two-int layout.
    /// The first field's physical unit is not inferred from its name.
    EqBand {
        data: u32,
        gain: i32,
    },
    /// DSP uint32; no guessed major/minor version components.
    Firmware {
        raw: u32,
    },
    /// Original GetPeakValue returns two independent uint16/32768 samples.
    Peak {
        left: f32,
        right: f32,
    },
}

#[derive(Deserialize)]
struct Property {
    key: String,
    source_property: String,
    command: u32,
    recipe: String,
    mask: Option<u32>,
    selector: Option<u32>,
    selector_preserve: Option<u32>,
    encoding: Option<String>,
    scale: Option<f32>,
    min: Option<f32>,
    max: Option<f32>,
    shift: Option<u32>,
    read_flag: Option<u32>,
    write_flag: Option<u32>,
    write_preserve: Option<u32>,
    busy_mask: Option<u32>,
    bands: Option<u8>,
    codes: Option<BTreeMap<String, u16>>,
    width: Option<u8>,
    raw_zero: Option<f32>,
    raw_step: Option<f32>,
    raw_bias: Option<f32>,
    volume_mask: Option<u32>,
    inverted: Option<bool>,
    decoded_max: Option<f32>,
    channel_shifts: Option<[u32; 2]>,
    volume_write_preserve: Option<u32>,
    source_input_type: Option<u8>,
    source_output_type: Option<u8>,
}

#[derive(Deserialize)]
struct Reports {
    query_id: u8,
    query_response_id: u8,
    write_id: u8,
    query_u16_response_id: u8,
    write_u16_id: u8,
    max_output_bytes: usize,
    query_delay_ms: u64,
    poll_delay_ms: u64,
    poll_attempts: usize,
}

#[derive(Deserialize)]
struct Capability {
    schema_version: u8,
    product_id: u32,
    vendor_id: u16,
    physical_product_id: u16,
    properties: Vec<Property>,
    reports: Reports,
    mic_monitor_limits: Vec<SourceLimit>,
    endpoint_limits: Vec<EndpointLimit>,
    driver: MixerDriverSpec,
}

/// Current binary driver wire format. OS handles and interface enumeration
/// belong to the platform adapter, never to this shared protocol module.
#[derive(Deserialize)]
pub struct MixerDriverSpec {
    pub device_interface_guid: String,
    pub matrix_read_ioctl: u32,
    pub matrix_write_ioctl: u32,
    pub reset_stream_ioctl: u32,
    pub matrix_bytes: usize,
    input_offsets: Vec<usize>,
    output_bases: Vec<usize>,
    rejected_indices: Vec<i32>,
    pub stream_indices: Vec<u32>,
}

pub fn driver_spec() -> &'static MixerDriverSpec {
    &capability().driver
}

/// Exact original two-int route layout, without inferred bus names.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MixerRoute {
    pub input: u8,
    pub output: u8,
}

pub enum MatrixRoute {
    Hid(MixerTarget),
    Driver { index: usize },
}

pub fn matrix_route(route: &MixerRoute) -> anyhow::Result<MatrixRoute> {
    let spec = driver_spec();
    let input = spec
        .input_offsets
        .get(usize::from(route.input))
        .context("混音输入不在原表中")?;
    let output = spec
        .output_bases
        .get(usize::from(route.output))
        .context("混音输出不在原表中")?;
    if let Some(prop) = capability().properties.iter().find(|p| {
        p.source_input_type == Some(route.input) && p.source_output_type == Some(route.output)
    }) {
        return Ok(MatrixRoute::Hid(MixerTarget {
            control: serde_json::from_value(serde_json::Value::String(prop.key.clone()))?,
            band: None,
            channel: MixerChannel::Both,
        }));
    }
    let index = input + output;
    ensure!(
        !spec.rejected_indices.contains(&(index as i32)) && index * 4 + 4 <= spec.matrix_bytes,
        "原驱动矩阵拒绝此输入/输出组合"
    );
    Ok(MatrixRoute::Driver { index })
}

pub trait MixerDriverTransport {
    fn read_matrix(&self) -> anyhow::Result<Vec<u8>>;
    fn write_matrix(&self, bytes: &[u8]) -> anyhow::Result<()>;
    /// Native property return code is data (0 / 0x10001 / 0x10003),
    /// separately from IPC/identity/transport exceptions.
    fn reset_stream(&self, index: u32) -> anyhow::Result<u32>;
}

fn driver_index(route: &MixerRoute) -> anyhow::Result<usize> {
    match matrix_route(route)? {
        MatrixRoute::Driver { index } => Ok(index),
        MatrixRoute::Hid(_) => bail!("此原路由必须走 HID，不能改走驱动矩阵"),
    }
}

fn matrix_value(bytes: &[u8], index: usize) -> anyhow::Result<bool> {
    ensure!(
        bytes.len() == driver_spec().matrix_bytes,
        "驱动矩阵实际长度与原码不符"
    );
    let selected = bytes
        .get(index * 4..index * 4 + 4)
        .context("驱动矩阵索引超出范围")?;
    // The original ucomiss/parity branch treats NaN and all non-1 values false.
    Ok(f32::from_le_bytes(selected.try_into()?) == 1.0)
}

pub fn read_driver_route(
    device: &dyn MixerDriverTransport,
    route: &MixerRoute,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<bool> {
    let index = driver_index(route)?;
    validate()?;
    let bytes = device.read_matrix()?;
    validate()?;
    matrix_value(&bytes, index)
}

#[derive(Serialize)]
pub struct MatrixWriteResult {
    pub requested: bool,
    pub previous: bool,
    pub transport_completed: bool,
}

pub fn write_driver_route(
    device: &dyn MixerDriverTransport,
    route: &MixerRoute,
    enabled: bool,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<MatrixWriteResult> {
    let index = driver_index(route)?;
    validate()?;
    let mut bytes = device
        .read_matrix()
        .context("设置前读取矩阵失败，未发送设置")?;
    let previous = matrix_value(&bytes, index)?;
    // Preserve every other byte, including arbitrary float/NaN bit patterns.
    bytes[index * 4..index * 4 + 4]
        .copy_from_slice(&(if enabled { 1.0f32 } else { 0.0 }).to_le_bytes());
    validate()?;
    device
        .write_matrix(&bytes)
        .context("矩阵设置传输失败；未获得驱动完成结果")?;
    validate().context("矩阵传输已完成，但身份或期限发生变化")?;
    // Source 0xDA46 -> 0xDA4C -> 0xDA84 continues enumeration after
    // successful IOCTL; it does not issue a post-write matrix getter.
    Ok(MatrixWriteResult {
        requested: enabled,
        previous,
        transport_completed: true,
    })
}

#[derive(Deserialize, Serialize)]
pub struct StreamsResetResult {
    pub completed_indices: Vec<u32>,
    pub return_codes: Vec<(u32, u32)>,
    pub source_sequence_completed: bool,
    pub confirmation: String,
}

pub fn restart_streams(
    device: &dyn MixerDriverTransport,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<StreamsResetResult> {
    let mut completed = Vec::new();
    let mut return_codes = Vec::new();
    for &index in &driver_spec().stream_indices {
        validate().with_context(|| format!("流重置停止；已完成索引 {completed:?}"))?;
        let code = device.reset_stream(index).with_context(|| {
            format!("流 {index} 重置未确认；已完成 {completed:?}，当前流可能已重置")
        })?;
        return_codes.push((index, code));
        if code == 0 {
            completed.push(index);
        }
        // Source restartAudioDriver parses each FFI JSON response but never
        // branches on its native code; a failed native call does not skip 0..8.
        validate().with_context(|| format!("流重置后身份或期限变化；已完成索引 {completed:?}"))?;
    }
    Ok(StreamsResetResult {
        completed_indices: completed,
        return_codes,
        source_sequence_completed: true,
        confirmation: "ioctl_completion_only_no_audio_state_readback".into(),
    })
}

#[derive(Deserialize, Serialize)]
pub struct SourceLimit {
    pub key: String,
    pub value: f32,
    pub source_property: String,
    pub origin: String,
}

/// Original DLL constants, not values queried from connected hardware.
pub fn mic_monitor_limits() -> &'static [SourceLimit] {
    &capability().mic_monitor_limits
}

#[derive(Deserialize, Serialize)]
pub struct EndpointLimit {
    pub key: String,
    pub min: f32,
    pub max: f32,
    pub step: f32,
    pub source_jack_type: u32,
    pub source_data_flow: u32,
    pub origin: String,
    pub source_properties: Vec<String>,
}

pub fn endpoint_limits() -> &'static [EndpointLimit] {
    &capability().endpoint_limits
}

fn capability() -> &'static Capability {
    static CAPABILITY: OnceLock<Capability> = OnceLock::new();
    CAPABILITY.get_or_init(|| {
        let result: Capability = serde_json::from_str(include_str!(
            "../../../assets/data/audio-mixer-protocol.json"
        ))
        .expect("source-derived Audio Mixer protocol");
        assert_eq!(result.schema_version, 1);
        result
    })
}

pub fn accepts(product_id: u32, vendor_id: u16, physical_product_id: u16) -> bool {
    let cap = capability();
    (product_id, vendor_id, physical_product_id)
        == (cap.product_id, cap.vendor_id, cap.physical_product_id)
}

/// Current JS writes fixed per-band templates, rather than preserving getter
/// data (whose native bitfield overlaps gain). This is not an inferred unit.
pub fn mic_eq_values(bands: &[i32; 10]) -> anyhow::Result<[MixerValue; 10]> {
    #[derive(Deserialize)]
    struct Recipe {
        product_id: u32,
        data_by_band: [u32; 10],
        enable_before_bands: bool,
        band_order: [u8; 10],
    }
    static RECIPE: OnceLock<Recipe> = OnceLock::new();
    let recipe = RECIPE.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../assets/data/audio-mixer-mic-eq-current.json"
        ))
        .expect("source-derived microphone EQ templates")
    });
    ensure!(
        recipe.product_id == 1342
            && recipe.enable_before_bands
            && recipe.band_order == [0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
        "Mic EQ source recipe changed"
    );
    ensure!(
        bands.iter().all(|gain| (-12..=12).contains(gain)),
        "Mic EQ gain exceeds current source range"
    );
    Ok(std::array::from_fn(|index| MixerValue::EqBand {
        data: recipe.data_by_band[index],
        gain: bands[index],
    }))
}

#[derive(Deserialize)]
struct EchoRecipe {
    presets: BTreeMap<String, [f32; 4]>,
    gain_table: [u16; 11],
}
fn echo_recipe() -> &'static EchoRecipe {
    static RECIPE: OnceLock<EchoRecipe> = OnceLock::new();
    RECIPE.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../assets/data/audio-mixer-echo-current.json"
        ))
        .expect("source-derived Echo/Reverb recipe")
    })
}
pub fn echo_presets() -> &'static BTreeMap<String, [f32; 4]> {
    &echo_recipe().presets
}

fn property(target: &MixerTarget) -> anyhow::Result<(&'static Property, u32)> {
    let prop = capability()
        .properties
        .iter()
        .find(|prop| prop.key == target.control.key())
        .context("该混音器控制没有当前 DLL 报文依据")?;
    ensure!(
        target.channel == MixerChannel::Both || prop.recipe == "endpoint_volume",
        "原码仅为端点音量提供此声道参数"
    );
    let command = if prop.recipe == "eq_band" {
        let band = target.band.context("EQ 控制缺少频段索引")?;
        ensure!(
            band < prop.bands.context("源 EQ 频段数量缺失")?,
            "EQ 频段索引超出原码范围"
        );
        prop.command + u32::from(band) * 4
    } else {
        ensure!(target.band.is_none(), "此控制不接受 EQ 频段参数");
        prop.command
    };
    Ok((prop, command))
}

pub fn source_property(target: &MixerTarget) -> anyhow::Result<&'static str> {
    Ok(&property(target)?.0.source_property)
}

/// Select a source-compatible collection from descriptor observations without
/// sending anything. The same checks gate every session recipe at execution.
pub fn validate_report_lengths(
    target: &MixerTarget,
    lengths: &ReportLengths,
) -> anyhow::Result<()> {
    validate_property_reports(property(target)?.0, lengths)
}

fn report_size(lengths: &BTreeMap<u8, usize>, id: u8, minimum: usize) -> anyhow::Result<usize> {
    let size = *lengths
        .get(&id)
        .context("实际 descriptor 缺少源 Report ID")?;
    let wire_size = *lengths.values().max().context("descriptor 未声明报告")?;
    ensure!(
        size >= minimum && wire_size <= capability().reports.max_output_bytes,
        "实际报告长度与原 helper 缓冲边界不符"
    );
    Ok(wire_size)
}

fn validate_property_reports(prop: &Property, lengths: &ReportLengths) -> anyhow::Result<()> {
    let reports = &capability().reports;
    report_size(&lengths.output, reports.query_id, 5)?;
    match prop.width {
        Some(16) => {
            report_size(&lengths.input, reports.query_u16_response_id, 3)?;
            report_size(&lengths.output, reports.write_u16_id, 7)?;
        }
        None | Some(32) => {
            report_size(&lengths.input, reports.query_response_id, 5)?;
            if prop.recipe != "firmware" {
                report_size(&lengths.output, reports.write_id, 9)?;
            }
        }
        _ => bail!("源寄存器宽度未实现"),
    }
    Ok(())
}

/// One retained handle and OS lock across mailbox selection, polling and RMW.
pub struct MixerSession<'a> {
    device: &'a dyn FeatureTransport,
    lengths: ReportLengths,
    validate: &'a dyn Fn() -> anyhow::Result<()>,
}

impl<'a> MixerSession<'a> {
    pub fn new(
        device: &'a dyn FeatureTransport,
        validate: &'a dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Self> {
        validate()?;
        let lengths = device.report_lengths()?;
        Ok(Self {
            device,
            lengths,
            validate,
        })
    }

    fn report_size(
        &self,
        lengths: &BTreeMap<u8, usize>,
        id: u8,
        minimum: usize,
    ) -> anyhow::Result<usize> {
        // Native helpers use HIDP_CAPS.Input/OutputReportByteLength, i.e. the
        // collection maximum, rather than the size of the selected Report ID.
        // Reject caps larger than their 66-byte local buffer rather than
        // reproducing truncation/overflow or relying on OS padding.
        report_size(lengths, id, minimum)
    }

    // Check all reports used by this recipe before even sending its first
    // query. Unrelated 16/32-bit reports do not constrain other recipes.
    fn validate_reports(&self, prop: &Property) -> anyhow::Result<()> {
        validate_property_reports(prop, &self.lengths)
    }

    fn send(&self, id: u8, command: u32, value: Option<u32>) -> anyhow::Result<()> {
        (self.validate)()?;
        let size = self.report_size(
            &self.lengths.output,
            id,
            if value.is_some() { 9 } else { 5 },
        )?;
        let mut report = vec![0; size];
        report[0] = id;
        report[1..5].copy_from_slice(&command.to_be_bytes());
        if let Some(value) = value {
            report[5..9].copy_from_slice(&value.to_be_bytes());
        }
        self.device.write_output(&report)?;
        (self.validate)()
    }

    fn query(&self, command: u32) -> anyhow::Result<u32> {
        let config = &capability().reports;
        self.send(config.query_id, command, None)?;
        thread::sleep(Duration::from_millis(config.query_delay_ms));
        (self.validate)()?;
        let count = self.report_size(&self.lengths.input, config.query_response_id, 5)?;
        let mut reply = vec![0; count];
        reply[0] = config.query_response_id;
        let received = self.device.get_input(&mut reply)?;
        ensure!(
            (received == count
                || Some(&received) == self.lengths.input.get(&config.query_response_id))
                && reply[0] == config.query_response_id,
            "DSP Input Report 的 ID 或实际长度不匹配"
        );
        (self.validate)()?;
        Ok(u32::from_le_bytes(
            reply[1..5].try_into().expect("checked report length"),
        ))
    }

    /// The original C++ helper uses Output Report 0x04 and Input Report 0x02
    /// for endpoint values. Keep this separate from the 32-bit DSP mailbox.
    fn query_u16(&self, command: u32) -> anyhow::Result<u16> {
        let config = &capability().reports;
        self.send(config.query_id, command, None)?;
        thread::sleep(Duration::from_millis(config.query_delay_ms));
        (self.validate)()?;
        let count = self.report_size(&self.lengths.input, config.query_u16_response_id, 3)?;
        let mut reply = vec![0; count];
        reply[0] = config.query_u16_response_id;
        let received = self.device.get_input(&mut reply)?;
        ensure!(
            (received == count
                || Some(&received) == self.lengths.input.get(&config.query_u16_response_id))
                && reply[0] == config.query_u16_response_id,
            "16 位 Input Report 的 ID 或实际长度不匹配"
        );
        (self.validate)()?;
        Ok(u16::from_le_bytes(
            reply[1..3].try_into().expect("checked report length"),
        ))
    }

    fn write(&self, command: u32, value: u32) -> anyhow::Result<()> {
        self.send(capability().reports.write_id, command, Some(value))
    }

    /// The original C++ helper writes Output Report 0x03 with a big-endian
    /// uint16 payload and has no device acknowledgement of its own.
    fn write_u16(&self, command: u32, value: u16) -> anyhow::Result<()> {
        (self.validate)()?;
        let id = capability().reports.write_u16_id;
        let size = self.report_size(&self.lengths.output, id, 7)?;
        let mut report = vec![0; size];
        report[0] = id;
        report[1..5].copy_from_slice(&command.to_be_bytes());
        report[5..7].copy_from_slice(&value.to_be_bytes());
        self.device.write_output(&report)?;
        (self.validate)()
    }

    fn poll(&self, command: u32, busy_mask: u32) -> anyhow::Result<u32> {
        let config = &capability().reports;
        let mut result = 0;
        for _ in 0..config.poll_attempts {
            thread::sleep(Duration::from_millis(config.poll_delay_ms));
            result = self.query(command)?;
            if result & busy_mask == 0 {
                return Ok(result);
            }
        }
        // CmMixerLib formats the last result even when its busy bit remains
        // set after the fifth query (IDA 0xe1e0 / 0xe4c0 / 0xf190). Only a
        // failed transport query takes the original error branch.
        Ok(result)
    }

    pub fn read(&self, target: &MixerTarget) -> anyhow::Result<MixerValue> {
        let (prop, command) = property(target)?;
        self.validate_reports(prop)?;
        let initial = self.query_property(prop, command)?;
        Ok(match prop.recipe.as_str() {
            "firmware" => MixerValue::Firmware { raw: initial },
            "toggle" => {
                let mask = prop.mask.context("源开关掩码缺失")?;
                MixerValue::Boolean {
                    enabled: initial & mask == mask,
                }
            }
            "eq_band" => decode_eq(initial),
            "register_scalar" => MixerValue::Scalar {
                value: decode_scalar(
                    prop,
                    ((initial & prop.mask.context("源字段掩码缺失")?)
                        >> prop.shift.context("源字段位移缺失")?) as u16,
                )?,
            },
            "mailbox" => {
                let selection = select(prop, initial)?;
                self.write(command, selection | prop.read_flag.context("源读标志缺失")?)?;
                let result = self.poll(command, prop.busy_mask.context("源繁忙位缺失")?)?;
                let raw = ((result >> 8) & 0xffff) as u16;
                let value = decode_scalar(prop, raw)?;
                MixerValue::Scalar { value }
            }
            "magic_voice" => {
                self.write(command, (initial & !0xf0) | 0xc0000000)?;
                let code = ((self.poll(command, 0x80000000)? >> 8) & 0xffff) as u16;
                let value = prop
                    .codes
                    .as_ref()
                    .context("源变声代码表缺失")?
                    .iter()
                    .find_map(|(key, value)| {
                        (*value == code).then(|| key.parse::<i32>().ok()).flatten()
                    })
                    .unwrap_or(-1);
                MixerValue::Scalar {
                    value: value as f32,
                }
            }
            "endpoint_volume" => MixerValue::Scalar {
                value: decode_volume(prop, initial, target.channel)?,
            },
            "endpoint_mute" => MixerValue::Boolean {
                // The original read branch observes the upper-channel mute
                // bit. The Both setter writes both channel bits together.
                enabled: (initial & 0x8000 != 0) ^ prop.inverted.context("源静音极性缺失")?,
            },
            "endpoint_peak" => {
                let scale = prop.scale.context("源峰值比例缺失")?;
                let value = MixerValue::Peak {
                    left: (initial & 0xffff) as f32 / scale,
                    right: (initial >> 16) as f32 / scale,
                };
                // Original 0xde60 calls 0xc460 after publishing the samples
                // and ignores its return. This value confirms the read only;
                // it does not claim that the clear request succeeded.
                let _ = self.write(command, 0);
                value
            }
            _ => bail!("该 DSP 原件操作尚未实现"),
        })
    }

    fn query_property(&self, prop: &Property, command: u32) -> anyhow::Result<u32> {
        match prop.width {
            None | Some(32) => self.query(command),
            Some(16) => Ok(u32::from(self.query_u16(command)?)),
            _ => bail!("源寄存器宽度未实现"),
        }
    }

    fn write_property(&self, prop: &Property, command: u32, payload: u32) -> anyhow::Result<()> {
        match prop.width {
            None | Some(32) => self.write(command, payload),
            Some(16) => self.write_u16(command, u16::try_from(payload).context("16 位报文溢出")?),
            _ => bail!("源寄存器宽度未实现"),
        }
    }

    /// Follow the source setter reports; completion is transport completion,
    /// not an additional getter or a claimed observed device state.
    pub fn apply(
        &self,
        target: &MixerTarget,
        requested: &MixerValue,
    ) -> anyhow::Result<MixerWriteResult> {
        let (prop, command) = property(target)?;
        validate_value(prop, requested)?;
        if let MixerValue::Scalar { value } = requested {
            let page_range = match target.control {
                MixerControl::PageReverbRoom => Some((-43., -19.)),
                MixerControl::PageReverbDecay => Some((0.6, 2.7)),
                MixerControl::PageEchoGain => Some((0., 1.)),
                MixerControl::PageEchoDelay => Some((110., 200.)),
                _ => None,
            };
            if let Some((min, max)) = page_range {
                ensure!(
                    (min..=max).contains(value),
                    "Echo field exceeds the current page caller range"
                );
            }
        }
        self.validate_reports(prop)?;
        let fixed_scalar = matches!(
            target.control,
            MixerControl::PageMagicVoice
                | MixerControl::PageReverbRoom
                | MixerControl::PageReverbDecay
                | MixerControl::PageEchoGain
                | MixerControl::PageEchoDelay
                | MixerControl::PageKeyShift
                | MixerControl::PageVocalFadingLevel
        );
        // Source fixed scalar templates and EQ bands do not query their prior value.
        let initial = if fixed_scalar
            || matches!(
                target.control,
                MixerControl::EqBand | MixerControl::PageEqEnabled
            ) {
            0
        } else {
            self.query_property(prop, command)?
        };
        let payload = if matches!(target.control, MixerControl::PageMicMonitorVolume) {
            let MixerValue::Scalar { value } = requested else {
                bail!("Current Mic Monitor level must be numeric");
            };
            // L query / B template in the actual AudioMixer instance:
            // i[0][7]=n.data[0], i[0][8]=n.data[1], i[0][6]=abs(trunc(t)).
            // Query data are LE. Preserve this unusual copy order verbatim.
            ((*value).trunc().abs() as u32) << 16 | (initial & 0xff) << 8 | (initial >> 8) & 0xff
        } else if matches!(target.control, MixerControl::PageEqEnabled) {
            let MixerValue::Boolean { enabled } = requested else {
                bail!("Current page EQ switch must be boolean");
            };
            if *enabled { 0x40000091 } else { 0x40000090 }
        } else if matches!(
            target.control,
            MixerControl::PageMagicVoiceEnabled
                | MixerControl::PageEchoReverbEnabled
                | MixerControl::PageVocalFadingEnabled
        ) {
            let MixerValue::Boolean { enabled } = requested else {
                bail!("Current page DSP switch must be boolean");
            };
            // Current JS he/De/et copy the three low query bytes into their
            // fixed C0/80/00 high byte, then change the source gate.
            let (high, mask) = match target.control {
                MixerControl::PageMagicVoiceEnabled => (0xc0000000, 2),
                MixerControl::PageEchoReverbEnabled => (0x80000000, 12),
                MixerControl::PageVocalFadingEnabled => (0, 8),
                _ => unreachable!(),
            };
            let copied = high | (initial & 0x00ffffff);
            if *enabled {
                copied | mask
            } else {
                copied & !mask
            }
        } else if matches!(target.control, MixerControl::PageMagicVoice) {
            let MixerValue::Scalar { value } = requested else {
                bail!("Current page Magic Voice mode must be numeric");
            };
            ensure!(
                (0. ..=3.).contains(value),
                "Current page Magic Voice accepts only source modes 0..3"
            );
            let mut flags = 0u32;
            for (control, mask) in [
                (MixerControl::EqEnabled, 1u32),
                (MixerControl::MagicVoiceEnabled, 2),
                (MixerControl::EchoReverbEnabled, 12),
            ] {
                let observed = self.read(&MixerTarget {
                    control,
                    band: None,
                    channel: MixerChannel::Both,
                })?;
                if matches!(observed, MixerValue::Boolean { enabled: true }) {
                    flags |= mask;
                }
            }
            // Current setMagicVoiceMode queries pe after its three gate reads.
            let _ = self.query_property(prop, command)?;
            let code = prop
                .codes
                .as_ref()
                .context("Source Magic Voice table missing")?
                .get(&(*value as i32).to_string())
                .context("Source Magic Voice mode missing")?;
            // Current JS ge/ye/fe/Se are fixed templates. Rebuild only the
            // three source gates; do not carry native reserved bits forward.
            0x80000000 | (u32::from(*code) << 8) | flags
        } else if matches!(
            target.control,
            MixerControl::PageReverbRoom
                | MixerControl::PageReverbDecay
                | MixerControl::PageEchoGain
                | MixerControl::PageEchoDelay
                | MixerControl::PageKeyShift
                | MixerControl::PageVocalFadingLevel
        ) {
            let MixerValue::Scalar { value } = requested else {
                bail!("Current page DSP field must be numeric");
            };
            let mut flags = prop.selector.context("Source Echo selector missing")?;
            for (control, mask) in [
                (MixerControl::EqEnabled, 1u32),
                (MixerControl::MagicVoiceEnabled, 2),
                (MixerControl::EchoReverbEnabled, 12),
            ] {
                if matches!(
                    self.read(&MixerTarget {
                        control,
                        band: None,
                        channel: MixerChannel::Both
                    })?,
                    MixerValue::Boolean { enabled: true }
                ) {
                    flags |= mask;
                }
            }
            let raw = if matches!(target.control, MixerControl::PageEchoGain) {
                let index = (*value * 10.).trunc() as usize;
                *echo_recipe()
                    .gain_table
                    .get(index)
                    .context("Echo gain source index exceeds 0..10")?
            } else {
                numeric_raw(prop, *value)?
            };
            0x80000000 | (u32::from(raw) << 8) | flags
        } else {
            encode(prop, initial, requested, target.channel)?
        };
        self.write_property(prop, command, payload)
            .context("DSP 设置发送未能确认；设备可能已经接受，请重新读取")?;
        (self.validate)()?;
        Ok(MixerWriteResult {
            requested: requested.clone(),
            transport_completed: true,
        })
    }
}

#[derive(Deserialize, Serialize)]
pub struct MixerWriteResult {
    pub requested: MixerValue,
    pub transport_completed: bool,
}

fn select(prop: &Property, initial: u32) -> anyhow::Result<u32> {
    Ok(
        (initial & prop.selector_preserve.context("源选择保留位缺失")?)
            | prop.selector.context("源选择位缺失")?,
    )
}

fn decode_eq(raw: u32) -> MixerValue {
    // CmMixerLib sub_18000DD30: the 19-bit getter data overlaps the gain
    // field. movsx cl followed by signed /4 truncates toward zero. Preserve
    // those source semantics rather than assuming setter/getter roundtrip.
    MixerValue::EqBand {
        data: (raw >> 1) & 0x7ffff,
        gain: i32::from(((raw >> 14) as u8) as i8) / 4,
    }
}

fn validate_value(prop: &Property, value: &MixerValue) -> anyhow::Result<()> {
    match (prop.recipe.as_str(), value) {
        ("toggle" | "endpoint_mute", MixerValue::Boolean { .. }) => Ok(()),
        ("mailbox" | "register_scalar" | "endpoint_volume", MixerValue::Scalar { value }) => {
            ensure!(value.is_finite(), "DSP 设置必须为有限数值");
            ensure!(
                (prop.min.context("源最小值缺失")?..=prop.max.context("源最大值缺失")?)
                    .contains(value),
                "DSP 设置超出原码范围"
            );
            if !matches!(prop.encoding.as_deref(), Some("float_scaled"))
                && !matches!(prop.recipe.as_str(), "register_scalar" | "endpoint_volume")
            {
                ensure!(value.fract() == 0.0, "此 DSP 原码参数必须是整数");
            }
            Ok(())
        }
        ("magic_voice", MixerValue::Scalar { value }) => {
            ensure!(
                value.is_finite() && value.fract() == 0.0 && (-1.0..=3.0).contains(value),
                "变声代码不在原表中"
            );
            Ok(())
        }
        ("eq_band", MixerValue::EqBand { gain, .. }) => {
            // The native setter validates only the signed gain and masks the
            // entire uint32 data argument to 15 bits. Getter data includes
            // overlapping gain bits, so rejecting those bits breaks RMW.
            ensure!((-32..=31).contains(gain), "EQ 参数超出源位域范围");
            Ok(())
        }
        ("firmware", _) => bail!("DSP 固件版本属性拒绝写入，不能用作升级入口"),
        ("endpoint_peak", _) => bail!("峰值属性不接受设置；读取包含原码的峰值清零操作"),
        _ => bail!("请求值类型与 DSP 原件操作不一致"),
    }
}

fn numeric_raw(prop: &Property, value: f32) -> anyhow::Result<u16> {
    let scale = prop.scale.context("源比例缺失")?;
    Ok(match prop.encoding.as_deref() {
        Some("negative_unsigned") => (-value) as u16,
        Some("decibel") => (10.0f32.powf(value / 20.0) * scale + 0.5) as u16,
        Some("signed_scaled") => ((value * scale) as i32 as i16) as u16,
        Some("unsigned_scaled") | Some("float_scaled") => (value * scale) as u16,
        _ => bail!("DSP 数值编码未实现"),
    })
}

fn encode(
    prop: &Property,
    initial: u32,
    value: &MixerValue,
    channel: MixerChannel,
) -> anyhow::Result<u32> {
    Ok(match value {
        MixerValue::Boolean { enabled } => {
            let mask = prop.mask.context("源开关掩码缺失")?;
            let enabled = *enabled ^ prop.inverted.unwrap_or(false);
            (initial & !mask) | if enabled { mask } else { 0 }
        }
        MixerValue::Scalar { value } if prop.recipe == "endpoint_volume" => {
            let raw = ((*value - prop.raw_zero.context("源音量零点缺失")?)
                / prop.raw_step.context("源音量步长缺失")?
                + prop.raw_bias.context("源音量偏移缺失")?) as u32;
            let raw = raw & 0xff;
            let native_preserve = prop.volume_write_preserve;
            let shifts = prop.channel_shifts.context("源声道顺序缺失")?;
            match channel {
                MixerChannel::Both => {
                    // Mic/LineIn/LineOut Both replaces the entire uint16.
                    // Headphones/Console retain precisely the native mask.
                    (initial & native_preserve.unwrap_or(0)) | raw | (raw << 8)
                }
                MixerChannel::Channel0 | MixerChannel::Channel1 => {
                    let shift = shifts[usize::from(channel == MixerChannel::Channel1)];
                    (initial & native_preserve.unwrap_or(0xffff & !(0xff << shift)))
                        | (raw << shift)
                }
            }
        }
        MixerValue::Scalar { value } if prop.recipe == "mailbox" => {
            (select(prop, initial)? & prop.write_preserve.context("源保留位缺失")?)
                | (u32::from(numeric_raw(prop, *value)?) << 8)
                | prop.write_flag.context("源写标志缺失")?
        }
        MixerValue::Scalar { value } if prop.recipe == "register_scalar" => {
            let mask = prop.mask.context("源字段掩码缺失")?;
            (initial & !mask)
                | ((u32::from(numeric_raw(prop, *value)?)
                    << prop.shift.context("源字段位移缺失")?)
                    & mask)
        }
        MixerValue::Scalar { value } if prop.recipe == "magic_voice" => {
            let code = prop
                .codes
                .as_ref()
                .context("源变声代码表缺失")?
                .get(&(*value as i32).to_string())
                .context("源变声代码不存在")?;
            (initial & 0x3f00000f) | (u32::from(*code) << 8) | 0x80000000
        }
        MixerValue::EqBand { data, gain } => {
            (((((*gain as u32) & 0x3f) << 15) | (*data & 0x7fff)) << 1) | u32::from(*gain != 0)
        }
        _ => bail!("源 DSP 写入编码不匹配"),
    })
}

fn decode_volume(prop: &Property, raw: u32, channel: MixerChannel) -> anyhow::Result<f32> {
    let mask = prop.volume_mask.context("源音量掩码缺失")?;
    let sample = match channel {
        MixerChannel::Both => (raw & mask).max((raw >> 8) & mask),
        MixerChannel::Channel0 | MixerChannel::Channel1 => {
            let shifts = prop.channel_shifts.context("源声道顺序缺失")?;
            (raw >> shifts[usize::from(channel == MixerChannel::Channel1)]) & mask
        }
    };
    let value = (sample as f32 - prop.raw_bias.context("源音量偏移缺失")?)
        * prop.raw_step.context("源音量步长缺失")?
        + prop.raw_zero.context("源音量零点缺失")?;
    Ok(prop.decoded_max.map_or(value, |maximum| value.min(maximum)))
}

fn decode_scalar(prop: &Property, raw: u16) -> anyhow::Result<f32> {
    let scale = prop.scale.context("源比例缺失")?;
    let value = match prop.encoding.as_deref() {
        Some("signed_scaled") => ((f32::from(raw as i16) / scale) as i32) as f32,
        Some("float_scaled") | Some("unsigned_scaled") => f32::from(raw) / scale,
        Some("negative_unsigned") => -f32::from(raw),
        Some("decibel") => {
            ensure!(raw > 0, "DSP 分贝查询返回零幅度，不能构造有效读值");
            ((f32::from(raw) / scale).log10() * 20.0) as i32 as f32
        }
        _ => bail!("源 DSP 数值解析未实现"),
    };
    ensure!(value.is_finite(), "DSP 返回不可解析的非有限数值");
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    struct MatrixMock {
        matrix: Mutex<Vec<u8>>,
        reads: Mutex<usize>,
        writes: Mutex<Vec<Vec<u8>>>,
        resets: Mutex<Vec<u32>>,
    }

    impl MatrixMock {
        fn new() -> Self {
            Self {
                matrix: Mutex::new(vec![0; driver_spec().matrix_bytes]),
                reads: Mutex::new(0),
                writes: Mutex::new(Vec::new()),
                resets: Mutex::new(Vec::new()),
            }
        }
    }

    impl MixerDriverTransport for MatrixMock {
        fn read_matrix(&self) -> anyhow::Result<Vec<u8>> {
            *self.reads.lock().unwrap() += 1;
            Ok(self.matrix.lock().unwrap().clone())
        }

        fn write_matrix(&self, bytes: &[u8]) -> anyhow::Result<()> {
            self.writes.lock().unwrap().push(bytes.to_vec());
            *self.matrix.lock().unwrap() = bytes.to_vec();
            Ok(())
        }

        fn reset_stream(&self, index: u32) -> anyhow::Result<u32> {
            self.resets.lock().unwrap().push(index);
            Ok(0)
        }
    }

    #[test]
    fn source_driver_matrix_route_preserves_bytes_without_post_write_getter() {
        let route = MixerRoute {
            input: 1,
            output: 1,
        };
        assert!(matches!(
            matrix_route(&route).unwrap(),
            MatrixRoute::Driver { index: 22 }
        ));
        let device = MatrixMock::new();
        {
            let mut matrix = device.matrix.lock().unwrap();
            matrix[22 * 4..22 * 4 + 4].copy_from_slice(&1.0f32.to_le_bytes());
            matrix[21 * 4..21 * 4 + 4].copy_from_slice(&0x7fc01234u32.to_le_bytes());
        }
        let before = device.matrix.lock().unwrap().clone();
        assert!(read_driver_route(&device, &route, || Ok(())).unwrap());
        let reads_before_write = *device.reads.lock().unwrap();
        let result = write_driver_route(&device, &route, false, || Ok(())).unwrap();
        assert!(result.previous);
        assert!(!result.requested);
        assert!(result.transport_completed);
        assert_eq!(*device.reads.lock().unwrap(), reads_before_write + 1);
        let matrix = device.matrix.lock().unwrap();
        assert_eq!(&matrix[22 * 4..22 * 4 + 4], &[0, 0, 0, 0]);
        assert_eq!(&matrix[..22 * 4], &before[..22 * 4]);
        assert_eq!(&matrix[22 * 4 + 4..], &before[22 * 4 + 4..]);
        assert_eq!(device.writes.lock().unwrap().len(), 1);
    }

    #[test]
    fn source_driver_rejects_hid_routes_and_preserves_reset_sequence() {
        let hid_route = MixerRoute {
            input: 0,
            output: 0,
        };
        assert!(matches!(
            matrix_route(&hid_route).unwrap(),
            MatrixRoute::Hid(_)
        ));
        let device = MatrixMock::new();
        assert!(driver_index(&hid_route).is_err());
        let result = restart_streams(&device, || Ok(())).unwrap();
        assert_eq!(result.completed_indices, (0..=8).collect::<Vec<_>>());
        assert_eq!(*device.resets.lock().unwrap(), (0..=8).collect::<Vec<_>>());
        assert_eq!(
            result.confirmation,
            "ioctl_completion_only_no_audio_state_readback"
        );
    }

    #[test]
    fn source_restart_continues_native_error_codes_but_stops_on_transport_exception() {
        struct ResetMock {
            indices: Mutex<Vec<u32>>,
            exception: bool,
        }
        impl MixerDriverTransport for ResetMock {
            fn read_matrix(&self) -> anyhow::Result<Vec<u8>> {
                panic!("no matrix query in stream reset")
            }
            fn write_matrix(&self, _: &[u8]) -> anyhow::Result<()> {
                panic!("no matrix write in stream reset")
            }
            fn reset_stream(&self, index: u32) -> anyhow::Result<u32> {
                self.indices.lock().unwrap().push(index);
                if index == 2 && self.exception {
                    anyhow::bail!("mock transport exception");
                }
                Ok(match index {
                    1 => 0x10001,
                    2 => 0x10003,
                    _ => 0,
                })
            }
        }
        let mock = ResetMock {
            indices: Mutex::new(Vec::new()),
            exception: false,
        };
        let result = restart_streams(&mock, || Ok(())).unwrap();
        assert_eq!(*mock.indices.lock().unwrap(), (0..=8).collect::<Vec<_>>());
        assert_eq!(result.completed_indices, vec![0, 3, 4, 5, 6, 7, 8]);
        assert_eq!(result.return_codes[1..3], [(1, 0x10001), (2, 0x10003)]);
        assert!(result.source_sequence_completed);
        let mock = ResetMock {
            indices: Mutex::new(Vec::new()),
            exception: true,
        };
        assert!(restart_streams(&mock, || Ok(())).is_err());
        assert_eq!(*mock.indices.lock().unwrap(), vec![0, 1, 2]);
    }

    #[test]
    fn source_mixer_codec_keeps_eq_band_layout_and_endpoint_channel_rules() {
        let eq = MixerTarget {
            control: MixerControl::EqBand,
            band: Some(2),
            channel: MixerChannel::Both,
        };
        assert_eq!(source_property(&eq).unwrap(), "RazerT2DSPEQBandControl");
        let raw = encode(
            property(&eq).unwrap().0,
            0,
            &MixerValue::EqBand {
                data: 0x1234,
                gain: -4,
            },
            MixerChannel::Both,
        )
        .unwrap();
        assert_eq!(raw, 0x003c2469);
        assert!(matches!(
            decode_eq(raw),
            MixerValue::EqBand {
                data: 0x61234,
                gain: -4
            }
        ));
        assert!(
            property(&MixerTarget {
                control: MixerControl::HeadphonesVolume,
                band: None,
                channel: MixerChannel::Channel0,
            })
            .is_ok()
        );
        assert_eq!(
            property(&eq).unwrap().1
                - property(&MixerTarget {
                    control: MixerControl::EqBand,
                    band: Some(1),
                    channel: MixerChannel::Both,
                })
                .unwrap()
                .1,
            4
        );
    }

    // CmMixerLib helpers C170/C460 and C300/C5D0 use Output/Input,
    // BE commands/payloads and LE replies. Never touch a real HID handle.
    struct ReportMock {
        lengths: ReportLengths,
        replies: Mutex<VecDeque<Result<Vec<u8>, &'static str>>>,
        outputs: Mutex<Vec<Vec<u8>>>,
        fail_output: Option<usize>,
    }

    impl ReportMock {
        fn new(replies: Vec<Result<Vec<u8>, &'static str>>) -> Self {
            Self {
                lengths: ReportLengths {
                    input: BTreeMap::from([(2, 3), (18, 5), (99, 12)]),
                    output: BTreeMap::from([(3, 7), (4, 5), (19, 9), (99, 12)]),
                    feature: BTreeMap::new(),
                },
                replies: Mutex::new(replies.into()),
                outputs: Mutex::new(Vec::new()),
                fail_output: None,
            }
        }

        fn outputs(&self) -> Vec<Vec<u8>> {
            self.outputs.lock().unwrap().clone()
        }
    }

    impl FeatureTransport for ReportMock {
        fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
            panic!("CmMixerLib source does not use Feature sends")
        }

        fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
            panic!("CmMixerLib source does not use Feature reads")
        }

        fn write_output(&self, report: &[u8]) -> anyhow::Result<()> {
            let mut outputs = self.outputs.lock().unwrap();
            outputs.push(report.to_vec());
            ensure!(
                self.fail_output != Some(outputs.len()),
                "mock Output failure"
            );
            Ok(())
        }

        fn get_input(&self, report: &mut [u8]) -> anyhow::Result<usize> {
            let reply = self
                .replies
                .lock()
                .unwrap()
                .pop_front()
                .expect("planned Input");
            let bytes = reply.map_err(anyhow::Error::msg)?;
            assert!(bytes.len() <= report.len());
            report[..bytes.len()].copy_from_slice(&bytes);
            Ok(bytes.len())
        }

        fn report_lengths(&self) -> anyhow::Result<ReportLengths> {
            Ok(self.lengths.clone())
        }

        fn metadata(&self) -> serde_json::Value {
            serde_json::json!({"mock": true})
        }
    }

    fn reply32(raw: u32) -> Result<Vec<u8>, &'static str> {
        let mut bytes = vec![18];
        bytes.extend_from_slice(&raw.to_le_bytes());
        Ok(bytes)
    }

    fn reply16(raw: u16) -> Result<Vec<u8>, &'static str> {
        let mut bytes = vec![2];
        bytes.extend_from_slice(&raw.to_le_bytes());
        Ok(bytes)
    }

    fn target(control: MixerControl) -> MixerTarget {
        MixerTarget {
            control,
            band: None,
            channel: MixerChannel::Both,
        }
    }

    fn wire(id: u8, command: u32, payload: &[u8]) -> Vec<u8> {
        let mut report = vec![id];
        report.extend_from_slice(&command.to_be_bytes());
        report.extend_from_slice(payload);
        report.resize(12, 0);
        report
    }

    #[test]
    fn source_helpers_keep_output_big_endian_input_little_endian_and_caps_padding() {
        let mock = ReportMock::new(vec![reply32(0x12345678), reply16(0xabcd)]);
        let validate = || Ok(());
        let session = MixerSession::new(&mock, &validate).unwrap();
        assert_eq!(session.query(0x5ffc001c).unwrap(), 0x12345678);
        assert_eq!(session.query_u16(0x1800c028).unwrap(), 0xabcd);
        session.write(0x5ffc002c, 0x12345678).unwrap();
        session.write_u16(0x1800c028, 0xabcd).unwrap();
        assert_eq!(
            mock.outputs(),
            vec![
                wire(4, 0x5ffc001c, &[]),
                wire(4, 0x1800c028, &[]),
                wire(19, 0x5ffc002c, &[0x12, 0x34, 0x56, 0x78]),
                wire(3, 0x1800c028, &[0xab, 0xcd]),
            ]
        );
        assert!(mock.replies.lock().unwrap().is_empty());
    }

    #[test]
    fn source_eq_getter_overlap_and_signed_truncation_survive_read_modify_write() {
        let eq = MixerTarget {
            control: MixerControl::EqBand,
            band: Some(2),
            channel: MixerChannel::Both,
        };
        let prop = property(&eq).unwrap().0;
        // IDA sub_18000DD30 has no data-range guard; only &0x7fff.
        let requested = MixerValue::EqBand {
            data: 0x61234,
            gain: -4,
        };
        validate_value(prop, &requested).unwrap();
        assert_eq!(
            encode(prop, u32::MAX, &requested, MixerChannel::Both).unwrap(),
            0x003c2469
        );
        assert!(matches!(
            decode_eq(0x0020ffff),
            MixerValue::EqBand {
                data: 0x7fff,
                gain: -31
            }
        ));
        let mock = ReportMock::new(vec![
            reply32(0x003c2469),
            reply32(0x003c2469),
            reply32(0x003c2469),
        ]);
        let validate = || Ok(());
        let result = MixerSession::new(&mock, &validate)
            .unwrap()
            .apply(&eq, &requested)
            .unwrap();
        assert!(result.transport_completed);
        assert_eq!(
            mock.outputs()[0],
            wire(19, 0x5ffc0078, &[0, 0x3c, 0x24, 0x69])
        );
    }

    #[test]
    fn source_endpoint_volume_quantization_and_console_channel_order() {
        let headphones = target(MixerControl::HeadphonesVolume);
        // Both preserves source mask 0x8080; -6.1 quantizes to -6.75.
        let mock = ReportMock::new(vec![reply16(0xd3d3), reply16(0xd3d3), reply16(0xcaca)]);
        let validate = || Ok(());
        let result = MixerSession::new(&mock, &validate)
            .unwrap()
            .apply(&headphones, &MixerValue::Scalar { value: -6.1 })
            .unwrap();
        assert!(result.transport_completed);
        assert_eq!(mock.outputs()[1], wire(3, 0x1800c028, &[0xca, 0xca]));
        let console = MixerTarget {
            channel: MixerChannel::Channel0,
            ..target(MixerControl::ConsoleVolume)
        };
        assert!(matches!(
            MixerSession::new(&ReportMock::new(vec![reply32(0x0a14)]), &validate)
                .unwrap()
                .read(&console)
                .unwrap(),
            MixerValue::Scalar { value: -20.0 }
        ));
        let headphones = MixerTarget {
            channel: MixerChannel::Channel0,
            ..headphones
        };
        assert!(matches!(
            decode_volume(property(&headphones).unwrap().0, 0x5046, headphones.channel).unwrap(),
            -2.25
        ));
    }

    #[test]
    fn source_read_rejects_partial_wrong_id_and_transport_failure() {
        for reply in [
            Ok(vec![18, 1, 2, 3]),
            Ok(vec![17, 1, 2, 3, 4]),
            Err("mock Input failure"),
        ] {
            let mock = ReportMock::new(vec![reply]);
            let validate = || Ok(());
            assert!(
                MixerSession::new(&mock, &validate)
                    .unwrap()
                    .read(&target(MixerControl::DspFirmware))
                    .is_err()
            );
            assert_eq!(mock.outputs().len(), 1);
        }
        for reply in [Ok(vec![2, 1]), Ok(vec![18, 1, 2])] {
            let mock = ReportMock::new(vec![reply]);
            let validate = || Ok(());
            assert!(
                MixerSession::new(&mock, &validate)
                    .unwrap()
                    .read(&target(MixerControl::HeadphonesVolume))
                    .is_err()
            );
        }
    }

    #[test]
    fn source_descriptor_missing_short_and_over_buffer_reports_fail_before_send() {
        for mode in 0..3 {
            let mut mock = ReportMock::new(vec![]);
            match mode {
                0 => {
                    mock.lengths.input.remove(&18);
                }
                1 => {
                    mock.lengths.input.insert(18, 4);
                }
                _ => {
                    mock.lengths.output.insert(99, 67);
                }
            }
            let validate = || Ok(());
            assert!(
                MixerSession::new(&mock, &validate)
                    .unwrap()
                    .read(&target(MixerControl::DspFirmware))
                    .is_err()
            );
            assert!(mock.outputs().is_empty());
        }
    }

    #[test]
    fn source_write_completion_does_not_add_a_getter() {
        let mock = ReportMock::new(vec![reply32(0xa0)]);
        let validate = || Ok(());
        let result = MixerSession::new(&mock, &validate)
            .unwrap()
            .apply(
                &target(MixerControl::EqEnabled),
                &MixerValue::Boolean { enabled: true },
            )
            .unwrap();
        assert!(result.transport_completed);
        assert_eq!(
            mock.outputs(),
            vec![
                wire(4, 0x5ffc0034, &[]),
                wire(19, 0x5ffc0034, &[0, 0, 0, 0xa1])
            ]
        );
        let mut mock = ReportMock::new(vec![reply32(0xa0)]);
        mock.fail_output = Some(2);
        assert!(
            MixerSession::new(&mock, &validate)
                .unwrap()
                .apply(
                    &target(MixerControl::EqEnabled),
                    &MixerValue::Boolean { enabled: true }
                )
                .is_err()
        );
        assert_eq!(mock.outputs().len(), 2);
    }

    #[test]
    fn source_peak_read_sends_clear_and_keeps_samples_on_clear_failure() {
        for fail in [None, Some(2)] {
            let mut mock = ReportMock::new(vec![reply32(0x40008000)]);
            mock.fail_output = fail;
            let validate = || Ok(());
            let result = MixerSession::new(&mock, &validate)
                .unwrap()
                .read(&target(MixerControl::HeadphonesPeak));
            assert!(matches!(
                result.unwrap(),
                MixerValue::Peak {
                    left: 1.0,
                    right: 0.5
                }
            ));
            assert_eq!(
                mock.outputs(),
                vec![
                    wire(4, 0x5ffc0054, &[]),
                    wire(19, 0x5ffc0054, &[0, 0, 0, 0])
                ]
            );
        }
    }

    #[test]
    fn source_mailbox_uses_selector_and_exhausts_original_poll_limit() {
        let initial = 0x11223344;
        let done = u32::from((-2500i16) as u16) << 8;
        let mock = ReportMock::new(vec![reply32(initial), reply32(0x80000000), reply32(done)]);
        let validate = || Ok(());
        assert!(matches!(
            MixerSession::new(&mock, &validate)
                .unwrap()
                .read(&target(MixerControl::ReverbRoom))
                .unwrap(),
            MixerValue::Scalar { value: -25.0 }
        ));
        assert_eq!(
            mock.outputs()[1],
            wire(19, 0x5ffc0034, &[0xd1, 0x22, 0x33, 0x54])
        );
        let mock = ReportMock::new(
            std::iter::once(reply32(0))
                .chain(std::iter::repeat_n(reply32(0x80000000), 5))
                .collect(),
        );
        assert!(matches!(
            MixerSession::new(&mock, &validate)
                .unwrap()
                .read(&target(MixerControl::ReverbRoom))
                .unwrap(),
            MixerValue::Scalar { value: 0.0 }
        ));
        assert_eq!(mock.outputs().len(), 7);
        assert!(mock.replies.lock().unwrap().is_empty());
    }

    #[test]
    fn source_last_busy_reply_is_formatted_for_both_mailboxes_and_magic_voice() {
        let validate = || Ok(());
        for (control, busy, payload, expected) in [
            (
                MixerControl::ReverbRoom,
                0x80000000,
                (-2500i16) as u16,
                -25.0,
            ),
            (MixerControl::CompressorThreshold, 0x10, 38, -38.0),
            (MixerControl::MagicVoice, 0x80000000, 0xe28f, 1.0),
        ] {
            let mock = ReportMock::new(
                std::iter::once(reply32(0))
                    .chain(std::iter::repeat_n(reply32(busy), 4))
                    .chain(std::iter::once(reply32(busy | (u32::from(payload) << 8))))
                    .collect(),
            );
            match MixerSession::new(&mock, &validate)
                .unwrap()
                .read(&target(control))
                .unwrap()
            {
                MixerValue::Scalar { value } => assert_eq!(value, expected),
                other => panic!("expected original scalar formatter, got {other:?}"),
            }
            assert_eq!(mock.outputs().len(), 7);
            assert!(mock.replies.lock().unwrap().is_empty());
        }
    }

    #[test]
    fn source_mailbox_failed_query_returns_error_without_further_polling() {
        let mock = ReportMock::new(vec![
            reply32(0),
            reply32(0x80000000),
            Err("mock mailbox input failed"),
            reply32(0),
        ]);
        let validate = || Ok(());
        assert!(
            MixerSession::new(&mock, &validate)
                .unwrap()
                .read(&target(MixerControl::ReverbRoom))
                .is_err()
        );
        assert_eq!(mock.outputs().len(), 4);
        assert_eq!(mock.replies.lock().unwrap().len(), 1);
    }
}
