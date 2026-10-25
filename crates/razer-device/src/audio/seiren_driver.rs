//! Marie USB audio driver protocol. RzNative 058e/05a6 RVAs 3f40–44a0
//! define DSP messages; the current driver API RVAs d270/d350/e0f0/e1d0/
//! 123d0/12790/12860 define IOCTL framing. No vendor code executes here.
use anyhow::{Context, Result, ensure};

pub const DRIVER_INFO: u32 = 0x8088_2004;
pub const GET_SAMPLE_RATE: u32 = 0x8088_2104;
pub const SET_SAMPLE_RATE: u32 = 0x8088_2108;
pub const GET_PLAYBACK_FORMATS: u32 = 0x8088_21c4;
pub const GET_CAPTURE_FORMATS: u32 = 0x8088_21c0;
pub const SET_PROFILES: u32 = 0x8088_21e8;
pub const GET_DSP: u32 = 0x8088_2200;
pub const SET_DSP: u32 = 0x8088_2204;
pub const GET_PROFILE_NAME: u32 = 0x8088_2428;
pub const SET_AUDIO_CONTROL: u32 = 0x8088_2081;
pub const STREAM_FORMAT_BYTES: usize = 332;
pub const STREAM_FORMAT_LIST_BYTES: usize = 8 + 32 * STREAM_FORMAT_BYTES;

pub trait Transport {
    /// Returns only bytes actually returned by the driver. Successful writes
    /// with no output return an empty buffer; errors never become zero data.
    fn exchange(&self, code: u32, input: &[u8], output_capacity: usize) -> Result<Vec<u8>>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Product {
    Pro,
    Chroma,
}
impl Product {
    pub fn from_id(id: u32) -> Result<Self> {
        match id {
            1422 => Ok(Self::Pro),
            1446 => Ok(Self::Chroma),
            _ => anyhow::bail!("Product {id} has no Marie USB driver contract"),
        }
    }
    /// Current driver packages' custom.ini DriverInterface.InterfaceGUID.
    pub fn interface_guid(self) -> u128 {
        match self {
            Self::Pro => 0xd132049c_ee3b_4647_ac9e_e754fa90fd46,
            Self::Chroma => 0xbb4bba6b_01ef_4028_9bb7_6e03446cc32f,
        }
    }
    pub fn endpoint_name(self) -> &'static str {
        match self {
            Self::Pro => "Razer Seiren V3 Pro",
            Self::Chroma => "Razer Seiren V3 Chroma 32-Bit DSP",
        }
    }
    fn driver_protocol(self) -> (u32, u32) {
        // Each current API's device-open handshake validates its own driver
        // protocol: Pro 9fc0, Chroma 9f90. They are not interchangeable.
        match self {
            Self::Pro => (6, 10),
            Self::Chroma => (6, 14),
        }
    }
}

fn word(bytes: &[u8], offset: usize) -> Result<u32> {
    let data = bytes
        .get(offset..offset + 4)
        .context("Truncated Marie driver word")?;
    Ok(u32::from_le_bytes(data.try_into()?))
}
fn exact(bytes: &[u8], expected: usize) -> Result<()> {
    ensure!(
        bytes.len() == expected,
        "Marie driver returned {} bytes; expected {expected}",
        bytes.len()
    );
    Ok(())
}

pub fn verify_driver(transport: &impl Transport, product: Product) -> Result<()> {
    let bytes = transport.exchange(DRIVER_INFO, &[], 24)?;
    exact(&bytes, 24)?;
    let expected = product.driver_protocol();
    ensure!(
        (word(&bytes, 0)?, word(&bytes, 4)?) == expected,
        "Marie USB driver protocol differs from verified {}.{} for {product:?}",
        expected.0,
        expected.1
    );
    Ok(())
}

pub fn sample_rate(transport: &impl Transport) -> Result<u32> {
    let bytes = transport.exchange(GET_SAMPLE_RATE, &[], 4)?;
    exact(&bytes, 4)?;
    let rate = word(&bytes, 0)?;
    ensure!(rate != 0, "Marie driver returned zero sample rate");
    Ok(rate)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    Playback,
    Capture,
}

pub fn current_stream_id(transport: &impl Transport, flow: Flow) -> Result<u32> {
    let code = match flow {
        Flow::Playback => GET_PLAYBACK_FORMATS,
        Flow::Capture => GET_CAPTURE_FORMATS,
    };
    let bytes = transport.exchange(code, &[], STREAM_FORMAT_LIST_BYTES)?;
    ensure!(
        bytes.len() >= 8 && bytes.len() <= STREAM_FORMAT_LIST_BYTES,
        "Invalid Marie stream format list length"
    );
    let count = word(&bytes, 0)? as usize;
    let selected = word(&bytes, 4)? as i32;
    ensure!(
        count <= 32 && selected >= 0 && (selected as usize) < count,
        "Marie stream format selection is absent or outside returned list"
    );
    ensure!(
        bytes.len() >= 8 + count * STREAM_FORMAT_BYTES,
        "Marie stream format list is truncated"
    );
    word(&bytes, 8 + selected as usize * STREAM_FORMAT_BYTES)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    Basic,
    Advanced,
    Unknown(String),
}
pub fn mode(transport: &impl Transport) -> Result<Mode> {
    let id = current_stream_id(transport, Flow::Playback)?;
    // Native 4c980 uses ExtendedInfo(10), current stream ID, and a 384-byte
    // UTF-16 buffer. Unknown profiles remain observations, never Basic.
    let bytes = transport.exchange(GET_PROFILE_NAME, &id.to_le_bytes(), 384)?;
    ensure!(
        !bytes.is_empty() && bytes.len() <= 384 && bytes.len() % 2 == 0,
        "Invalid Marie profile name length"
    );
    let units = bytes
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect::<Vec<_>>();
    let end = units
        .iter()
        .position(|v| *v == 0)
        .context("Marie profile name lacks terminator")?;
    let name = String::from_utf16(&units[..end]).context("Marie profile name is invalid UTF-16")?;
    ensure!(!name.is_empty(), "Marie profile name is empty");
    Ok(match name.as_str() {
        "plb_base" => Mode::Basic,
        "plb_advanced" => Mode::Advanced,
        _ => Mode::Unknown(name),
    })
}

pub fn profiles(playback: u32, capture: u32, advanced: bool) -> [u8; 264] {
    let mut bytes = [0; 264];
    let names = if advanced {
        ["plb_advanced", "rec_advanced"]
    } else {
        ["plb_base", "rec_base"]
    };
    for (index, (id, name)) in [playback, capture].into_iter().zip(names).enumerate() {
        let offset = index * 132;
        bytes[offset..offset + 4].copy_from_slice(&id.to_le_bytes());
        for (i, unit) in name.encode_utf16().enumerate() {
            bytes[offset + 4 + i * 2..offset + 6 + i * 2].copy_from_slice(&unit.to_le_bytes());
        }
    }
    bytes
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Channel {
    pub kind: ChannelKind,
    pub index: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ChannelKind {
    AppPlayback = 0,
    DeviceInput = 1,
    AppPlaybackVirtual = 2,
    AppRecording = 4,
    DeviceOutput = 5,
    AppRecordingVirtual = 6,
}
impl ChannelKind {
    pub fn output(self) -> bool {
        matches!(
            self,
            Self::AppRecording | Self::DeviceOutput | Self::AppRecordingVirtual
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Route {
    pub input: Channel,
    pub output: Channel,
}

fn dsp_header(command: u32) -> Vec<u8> {
    [77u32.to_le_bytes(), command.to_le_bytes()].concat()
}

/// The source uses an unsigned shift and division, not floating-point gain.
pub fn gain(level: u32, enabled: bool) -> Result<u32> {
    ensure!(level <= 100, "Marie mix level exceeds 100");
    Ok(if enabled { (level << 24) / 100 } else { 0 })
}
pub fn route_gain(routes: &[Route], level: u32, enabled: bool) -> Result<Vec<u8>> {
    let gain = gain(level, enabled)?;
    let mut bytes = dsp_header(200);
    for route in routes {
        ensure!(
            !route.input.kind.output() && route.output.kind.output(),
            "Marie route direction is invalid"
        );
        bytes.extend_from_slice(&[
            route.input.kind as u8,
            route.input.index,
            route.output.kind as u8 & 3,
            route.output.index,
        ]);
        bytes.extend_from_slice(&gain.to_le_bytes());
    }
    Ok(bytes)
}
pub fn output_gain(channels: &[Channel], level: u32, enabled: bool) -> Result<Vec<u8>> {
    let gain = gain(level, enabled)?;
    let mut bytes = dsp_header(201);
    for channel in channels {
        ensure!(
            channel.kind.output(),
            "Marie output gain requires an output channel"
        );
        bytes.extend_from_slice(&[channel.kind as u8 & 3, channel.index]);
        bytes.extend_from_slice(&gain.to_le_bytes());
    }
    Ok(bytes)
}
pub fn meter_enable(enabled: bool) -> Vec<u8> {
    let mut bytes = dsp_header(300);
    bytes.extend_from_slice(&u32::from(enabled).to_le_bytes());
    bytes
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioControlWrite {
    pub header: [u8; 24],
    /// METHOD_IN_DIRECT carries the USB payload through the second buffer.
    pub payload: Vec<u8>,
}
impl AudioControlWrite {
    fn monitor(control: u8, channel: u8, payload: Vec<u8>) -> Self {
        // Native 4e3a0/4e430 assembly: entity 10, SET_CUR request 1,
        // selector 1 (mute) / 2 (volume), timeout 100 ms. API 17e0 ->
        // 9eb0 -> a150 constructs this exact 24-byte driver request.
        let mut header = [0; 24];
        header[..4].copy_from_slice(&1u32.to_le_bytes());
        header[4..8].copy_from_slice(&1u32.to_le_bytes());
        header[8..12].copy_from_slice(&1u32.to_le_bytes());
        header[12] = 1;
        header[16] = channel;
        header[17] = control;
        header[19] = 10;
        header[20..24].copy_from_slice(&100u32.to_le_bytes());
        Self { header, payload }
    }
    pub fn monitor_enabled(enabled: bool) -> Self {
        Self::monitor(1, 0, vec![u8::from(!enabled)])
    }
    pub fn monitor_level(level: u32) -> Result<[Self; 3]> {
        ensure!(level <= 100, "Marie monitor level exceeds source range");
        let logarithm = ((level as f32 / 100.0).clamp(0.0, 1.0) * 9.0 + 1.0).log10();
        let value = ((logarithm / 10.0f32.log10() * 21.0 + 10.0) * 256.0) as u16;
        Ok([1, 2, 3].map(|channel| Self::monitor(2, channel, value.to_le_bytes().to_vec())))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChannelCounts {
    pub device_output: u32,
    pub device_input: u32,
    pub app_playback: u32,
    pub app_recording: u32,
    pub app_playback_virtual: u32,
    pub app_recording_virtual: u32,
}
pub fn channel_counts(transport: &impl Transport) -> Result<ChannelCounts> {
    let mut request = dsp_header(42);
    request.resize(16, 0);
    let version = transport.exchange(GET_DSP, &request, request.len())?;
    exact(&version, 16)?;
    ensure!(
        word(&version, 0)? == 77 && word(&version, 4)? == 42 && word(&version, 8)? == 2,
        "Marie mixer DSP version/header differs from current contract"
    );
    let mut counts = [0; 6];
    for (index, count) in counts.iter_mut().enumerate() {
        let mut request = dsp_header(100 + index as u32);
        request.resize(12, 0);
        let response = transport.exchange(GET_DSP, &request, 12)?;
        exact(&response, 12)?;
        ensure!(
            response[..8] == request[..8],
            "Marie DSP count response header differs"
        );
        *count = word(&response, 8)?;
        // Channel IDs in the source payload have one-byte indices.
        ensure!(
            *count <= 256,
            "Marie DSP channel count exceeds wire index range"
        );
    }
    Ok(ChannelCounts {
        device_output: counts[0],
        device_input: counts[1],
        app_playback: counts[2],
        app_recording: counts[3],
        app_playback_virtual: counts[4],
        app_recording_virtual: counts[5],
    })
}

/// RVA 44a0 requests every physical and virtual channel in this order.
pub fn meter_channels(counts: ChannelCounts) -> Result<Vec<Channel>> {
    let mut channels = Vec::new();
    for (kind, count) in [
        (ChannelKind::AppPlayback, counts.app_playback),
        (ChannelKind::DeviceInput, counts.device_input),
        (ChannelKind::AppPlaybackVirtual, counts.app_playback_virtual),
        (ChannelKind::DeviceOutput, counts.device_output),
        (ChannelKind::AppRecording, counts.app_recording),
        (
            ChannelKind::AppRecordingVirtual,
            counts.app_recording_virtual,
        ),
    ] {
        ensure!(
            count <= 256,
            "Marie meter channel count exceeds wire index range"
        );
        channels.extend((0..count).map(|index| Channel {
            kind,
            index: index as u8,
        }));
    }
    Ok(channels)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeterReading {
    pub channel: Channel,
    pub hundredths_db: i16,
}
impl MeterReading {
    /// Native 4ddc0 performs single-precision arithmetic followed by truncation.
    pub fn percent(self) -> u32 {
        (((f32::from(self.hundredths_db) / 100.0).clamp(-60.0, 0.0) + 60.0) * 100.0 / 60.0) as u32
    }
}
pub fn meters(transport: &impl Transport, channels: &[Channel]) -> Result<Vec<MeterReading>> {
    let mut request = dsp_header(301);
    for channel in channels {
        request.extend_from_slice(&u32::from(channel.kind.output()).to_le_bytes());
        request.extend_from_slice(&[channel.kind as u8 & 3, channel.index, 0, 0]);
    }
    let response = transport.exchange(GET_DSP, &request, request.len())?;
    exact(&response, request.len())?;
    ensure!(
        response[..8] == request[..8],
        "Marie meter response header differs"
    );
    channels
        .iter()
        .enumerate()
        .map(|(index, channel)| {
            let offset = 8 + index * 8;
            ensure!(
                response[offset..offset + 6] == request[offset..offset + 6],
                "Marie meter response channel differs"
            );
            Ok(MeterReading {
                channel: *channel,
                hundredths_db: i16::from_le_bytes(response[offset + 6..offset + 8].try_into()?),
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "seiren_driver/tests.rs"]
mod tests;
