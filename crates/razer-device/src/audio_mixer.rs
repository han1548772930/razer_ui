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
    EqBand,
    MagicVoiceEnabled,
    MagicVoice,
    EchoReverbEnabled,
    ReverbRoom,
    ReverbDecay,
    EchoGain,
    EchoDelay,
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
    VocalFadingEnabled,
    VocalFadingLevel,
    MicMonitorVolume,
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
            Self::EqEnabled => "eq_enabled",
            Self::EqBand => "eq_band",
            Self::MagicVoiceEnabled => "magic_voice_enabled",
            Self::MagicVoice => "magic_voice",
            Self::EchoReverbEnabled => "echo_reverb_enabled",
            Self::ReverbRoom => "reverb_room",
            Self::ReverbDecay => "reverb_decay",
            Self::EchoGain => "echo_gain",
            Self::EchoDelay => "echo_delay",
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
            Self::VocalFadingEnabled => "vocal_fading_enabled",
            Self::VocalFadingLevel => "vocal_fading_level",
            Self::MicMonitorVolume => "mic_monitor_volume",
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
    fn reset_stream(&self, index: u32) -> anyhow::Result<()>;
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
    pub observed: bool,
    pub verified: bool,
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
        .context("矩阵设置未能确认；驱动可能已接受，请重新读取")?;
    validate().context("矩阵已发送，身份或期限变化，未确认设置")?;
    let observed = matrix_value(
        &device.read_matrix().context("矩阵已发送，但回读失败")?,
        index,
    )?;
    validate()?;
    ensure!(observed == enabled, "矩阵回读与请求不符，未确认设置成功");
    Ok(MatrixWriteResult {
        requested: enabled,
        previous,
        observed,
        verified: true,
    })
}

#[derive(Serialize)]
pub struct StreamsResetResult {
    pub completed_indices: Vec<u32>,
    pub confirmation: &'static str,
}

pub fn restart_streams(
    device: &dyn MixerDriverTransport,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<StreamsResetResult> {
    let mut completed = Vec::new();
    for &index in &driver_spec().stream_indices {
        validate().with_context(|| format!("流重置停止；已完成索引 {completed:?}"))?;
        device.reset_stream(index).with_context(|| {
            format!("流 {index} 重置未确认；已完成 {completed:?}，当前流可能已重置")
        })?;
        completed.push(index);
        validate().with_context(|| format!("流重置后身份或期限变化；已完成索引 {completed:?}"))?;
    }
    Ok(StreamsResetResult {
        completed_indices: completed,
        confirmation: "ioctl_completion_only_no_audio_state_readback",
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
        let size = *lengths
            .get(&id)
            .context("实际 descriptor 缺少源 Report ID")?;
        let wire_size = *lengths.values().max().context("descriptor 未声明报告")?;
        ensure!(
            size >= minimum && wire_size <= capability().reports.max_output_bytes,
            "实际报告长度与原 helper 缓冲边界不符"
        );
        // Native helpers use HIDP_CAPS.Input/OutputReportByteLength, i.e. the
        // collection maximum, rather than the size of the selected Report ID.
        // Reject caps larger than their 66-byte local buffer rather than
        // reproducing truncation/overflow or relying on OS padding.
        Ok(wire_size)
    }

    // Check all reports used by this recipe before even sending its first
    // query. Unrelated 16/32-bit reports do not constrain other recipes.
    fn validate_reports(&self, prop: &Property) -> anyhow::Result<()> {
        let reports = &capability().reports;
        self.report_size(&self.lengths.output, reports.query_id, 5)?;
        match prop.width {
            Some(16) => {
                self.report_size(&self.lengths.input, reports.query_u16_response_id, 3)?;
                self.report_size(&self.lengths.output, reports.write_u16_id, 7)?;
            }
            None | Some(32) => {
                self.report_size(&self.lengths.input, reports.query_response_id, 5)?;
                // Mailbox queries and peak reads also write to the device.
                if prop.recipe != "firmware" {
                    self.report_size(&self.lengths.output, reports.write_id, 9)?;
                }
            }
            _ => bail!("源寄存器宽度未实现"),
        }
        Ok(())
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
        // The original formatter also consumes the fifth busy result. Expose
        // that uncertainty instead of reporting a confirmed hardware value.
        bail!("DSP 查询选择位在原码轮询次数后仍繁忙：{result:#010x}")
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
                self.write(command, 0)
                    .context("峰值已读取，但原码清零命令未能确认")?;
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

    /// The original setter confirms transport completion only. This result
    /// additionally verifies an actual readback, without fabricating an ACK.
    pub fn apply(
        &self,
        target: &MixerTarget,
        requested: &MixerValue,
    ) -> anyhow::Result<MixerWriteResult> {
        let (prop, command) = property(target)?;
        validate_value(prop, requested)?;
        let previous = self.read(target).context("设置前读取失败，未发送设置")?;
        let initial = self.query_property(prop, command)?;
        let payload = encode(prop, initial, requested, target.channel)?;
        self.write_property(prop, command, payload)
            .context("DSP 设置发送未能确认；设备可能已经接受，请重新读取")?;
        let observed = self
            .read(target)
            .context("DSP 设置已发送，但回读未能确认")?;
        let expected = match requested {
            MixerValue::Scalar { .. } if prop.recipe == "endpoint_volume" => MixerValue::Scalar {
                value: decode_volume(prop, payload, target.channel)?,
            },
            MixerValue::Scalar { value } if prop.recipe != "magic_voice" => MixerValue::Scalar {
                value: decode_scalar(prop, numeric_raw(prop, *value)?)?,
            },
            MixerValue::EqBand { .. } => decode_eq(payload),
            _ => requested.clone(),
        };
        let verified = match (&expected, &observed) {
            (MixerValue::Boolean { enabled: a }, MixerValue::Boolean { enabled: b }) => a == b,
            (MixerValue::Scalar { value: a }, MixerValue::Scalar { value: b }) => a == b,
            (MixerValue::EqBand { data: a, gain: c }, MixerValue::EqBand { data: b, gain: d }) => {
                a == b && c == d
            }
            _ => false,
        };
        ensure!(verified, "DSP 回读与原码量化后的请求值不符，未确认设置成功");
        (self.validate)()?;
        Ok(MixerWriteResult {
            requested: requested.clone(),
            previous,
            observed,
            verified,
        })
    }
}

#[derive(Serialize)]
pub struct MixerWriteResult {
    pub requested: MixerValue,
    pub previous: MixerValue,
    pub observed: MixerValue,
    pub verified: bool,
}

fn select(prop: &Property, initial: u32) -> anyhow::Result<u32> {
    Ok(
        (initial & prop.selector_preserve.context("源选择保留位缺失")?)
            | prop.selector.context("源选择位缺失")?,
    )
}

fn decode_eq(raw: u32) -> MixerValue {
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
        ("eq_band", MixerValue::EqBand { data, gain }) => {
            ensure!(
                *data <= 0x7fff && (-32..=31).contains(gain),
                "EQ 参数超出源位域范围"
            );
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
