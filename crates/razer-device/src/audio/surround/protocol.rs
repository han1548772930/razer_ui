//! Wire types recovered from the current routing client descriptor.
//! Schema: thx_wasapi_routing.proto, Carol v1.0.80.0 RVA 0x200940,
//! plus the identical Clio X 1.2.0.51 / Soma Pro 1.2.1.59_2 extensions.
//! Messages are local-service observations or requests, never device defaults.

use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct ApiState {
    #[prost(uint32, tag = "1")]
    pub api_version: u32,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct ClientConnect {
    #[prost(string, tag = "1")]
    pub file_version: String,
    #[prost(string, tag = "2")]
    pub api_version: String,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct ClientConnectAcknowledge {
    #[prost(string, tag = "1")]
    pub file_version: String,
    #[prost(string, tag = "2")]
    pub api_version: String,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct StreamProperties {
    #[prost(string, tag = "1")]
    pub endpoint_id: String,
    #[prost(bytes = "vec", tag = "2")]
    pub wave_format_extensible: Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub audio_client_properties: Vec<u8>,
    #[prost(uint32, tag = "4")]
    pub stream_flags: u32,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct StateHeader {
    #[prost(string, tag = "1")]
    pub id: String,
    #[prost(uint64, tag = "2")]
    pub revision: u64,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct WasapiManagerState {
    #[prost(message, optional, tag = "1")]
    pub state_header: Option<StateHeader>,
    #[prost(message, repeated, tag = "2")]
    pub active_streams: Vec<StreamProperties>,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct EndpointStreamState {
    #[prost(string, tag = "1")]
    pub endpoint_id: String,
    #[prost(enumeration = "StreamState", tag = "2")]
    pub stream_state: i32,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct StreamStates {
    #[prost(message, optional, tag = "1")]
    pub state_header: Option<StateHeader>,
    #[prost(message, repeated, tag = "2")]
    pub endpoint_stream_state: Vec<EndpointStreamState>,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct ChannelProperties {
    #[prost(int32, tag = "1")]
    pub channel: i32,
    #[prost(float, tag = "2")]
    pub gain_db: f32,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct ChannelMap {
    #[prost(int32, tag = "1")]
    pub input_channel: i32,
    #[prost(message, repeated, tag = "2")]
    pub output_channels: Vec<ChannelProperties>,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct ChannelMaps {
    #[prost(string, tag = "1")]
    pub endpoint_id: String,
    #[prost(message, repeated, tag = "2")]
    pub channel_maps: Vec<ChannelMap>,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct ChannelMapsState {
    #[prost(message, optional, tag = "1")]
    pub state_header: Option<StateHeader>,
    #[prost(uint64, repeated, tag = "2")]
    pub vector_clock: Vec<u64>,
    #[prost(message, repeated, tag = "3")]
    pub endpoint_channel_maps: Vec<ChannelMaps>,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct VadChannelVolumesState {
    #[prost(message, optional, tag = "1")]
    pub state_header: Option<StateHeader>,
    #[prost(float, repeated, tag = "2")]
    pub volumes: Vec<f32>,
    #[prost(float, tag = "3")]
    pub multiplier: f32,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct EndpointsState {
    #[prost(message, optional, tag = "1")]
    pub state_header: Option<StateHeader>,
    #[prost(message, repeated, tag = "2")]
    pub endpoints_properties: Vec<EndpointProperties>,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct EndpointProperties {
    #[prost(string, tag = "1")]
    pub endpoint_id: String,
    #[prost(string, tag = "2")]
    pub friendly_name: String,
    #[prost(uint32, tag = "3")]
    pub form_factor: u32,
    #[prost(bytes = "vec", tag = "4")]
    pub wave_format_extensible: Vec<u8>,
    #[prost(uint32, tag = "5")]
    pub thx_endpoint_id: u32,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct EndpointVolumeState {
    #[prost(message, optional, tag = "1")]
    pub state_header: Option<StateHeader>,
    #[prost(bool, tag = "2")]
    pub muted: bool,
    #[prost(float, tag = "3")]
    pub volume: f32,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct EqBand {
    #[prost(uint32, tag = "1")]
    pub index: u32,
    #[prost(enumeration = "EqType", tag = "2")]
    pub filter_type: i32,
    #[prost(float, tag = "3")]
    pub frequency: f32,
    #[prost(float, tag = "4")]
    pub gain_db: f32,
    #[prost(float, tag = "5")]
    pub q: f32,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct EndpointEqs {
    #[prost(string, tag = "1")]
    pub endpoint_id: String,
    #[prost(message, repeated, tag = "2")]
    pub eq_bands: Vec<EqBand>,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct EqState {
    #[prost(message, optional, tag = "1")]
    pub state_header: Option<StateHeader>,
    #[prost(message, repeated, tag = "2")]
    pub endpoint_eqs: Vec<EndpointEqs>,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct EndpointDelay {
    #[prost(string, tag = "1")]
    pub endpoint_id: String,
    #[prost(float, tag = "2")]
    pub delay_milliseconds: f32,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct DelayState {
    #[prost(message, optional, tag = "1")]
    pub state_header: Option<StateHeader>,
    #[prost(message, repeated, tag = "2")]
    pub endpoint_delays: Vec<EndpointDelay>,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct PlayDelayTestSignal {}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct StopDelayTestSignal {}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    prost::Enumeration,
    Serialize,
    Deserialize,
)]
#[repr(i32)]
pub enum StreamState {
    Starting = 0,
    Streaming = 1,
    Stopped = 2,
    Failed = 3,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    prost::Enumeration,
    Serialize,
    Deserialize,
)]
#[repr(i32)]
pub enum EqType {
    Peak = 0,
    LowShelf = 1,
    HighShelf = 2,
    HighPass = 3,
    LowPass = 4,
    Notch = 5,
    BandPass = 6,
    AllPass = 7,
    LowPass1P = 8,
    HighPass1P = 9,
    PbSecondLowpassFq = 10,
    PbSecondHighpassFq = 11,
    PbFirstLowpassF = 12,
    PbFirstHighpassF = 13,
    PbSecondAllpassFq = 14,
    PbFirstAllpassF = 15,
    PbSecondPeakFqg = 16,
    PbSecondDipFqg = 17,
    PbGain = 18,
    PbInvert = 19,
    PbSecondBassShelfFqFq = 20,
    PbSecondTrebleShelfFqFq = 21,
    PbFirstBassShelfFF = 22,
    PbFirstTrebleShelfFF = 23,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct DisablableEqBand {
    #[prost(uint32, tag = "1")]
    pub index: u32,
    #[prost(bool, tag = "2")]
    pub enabled: bool,
    #[prost(enumeration = "EqType", tag = "3")]
    pub filter_type: i32,
    #[prost(float, tag = "4")]
    pub frequency: f32,
    #[prost(float, tag = "5")]
    pub gain_db: f32,
    #[prost(float, tag = "6")]
    pub q: f32,
    #[prost(float, tag = "7")]
    pub frequency2: f32,
    #[prost(float, tag = "8")]
    pub q2: f32,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct CtcBandConfig {
    #[prost(bool, tag = "1")]
    pub active: bool,
    #[prost(float, tag = "2")]
    pub gain_db: f32,
    #[prost(float, tag = "3")]
    pub race_attenuation_db: f32,
    #[prost(int32, tag = "4")]
    pub race_delay_samples: i32,
    #[prost(message, repeated, tag = "5")]
    pub race_biquads: Vec<DisablableEqBand>,
    #[prost(message, repeated, tag = "6")]
    pub correction_biquads: Vec<DisablableEqBand>,
    #[prost(message, repeated, tag = "7")]
    pub bandlimiting_biquads: Vec<DisablableEqBand>,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct CtcBassBandConfig {
    #[prost(bool, tag = "1")]
    pub active: bool,
    #[prost(float, tag = "2")]
    pub gain_db: f32,
    #[prost(message, repeated, tag = "3")]
    pub bass_biquads: Vec<DisablableEqBand>,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct CtcConfig {
    #[prost(int32, tag = "1")]
    pub delay_samples: i32,
    #[prost(message, optional, tag = "2")]
    pub high_band_config: Option<CtcBandConfig>,
    #[prost(message, optional, tag = "3")]
    pub low_band_config: Option<CtcBandConfig>,
    #[prost(message, optional, tag = "4")]
    pub bass_band_config: Option<CtcBassBandConfig>,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct FrontCtcState {
    #[prost(message, optional, tag = "1")]
    pub state_header: Option<StateHeader>,
    #[prost(message, optional, tag = "2")]
    pub ctc_config: Option<CtcConfig>,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct SideCtcState {
    #[prost(message, optional, tag = "1")]
    pub state_header: Option<StateHeader>,
    #[prost(message, optional, tag = "2")]
    pub ctc_config: Option<CtcConfig>,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct RearCtcState {
    #[prost(message, optional, tag = "1")]
    pub state_header: Option<StateHeader>,
    #[prost(message, optional, tag = "2")]
    pub ctc_config: Option<CtcConfig>,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct VirtualizerConfig {
    #[prost(bool, tag = "1")]
    pub active: bool,
    #[prost(string, tag = "2")]
    pub fir_name: String,
    #[prost(message, repeated, tag = "3")]
    pub correction_biquads: Vec<DisablableEqBand>,
    #[prost(float, repeated, tag = "4")]
    pub fir_l: Vec<f32>,
    #[prost(float, repeated, tag = "5")]
    pub fir_r: Vec<f32>,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct FrontVirtualizerState {
    #[prost(message, optional, tag = "1")]
    pub state_header: Option<StateHeader>,
    #[prost(message, optional, tag = "2")]
    pub virtualizer_config: Option<VirtualizerConfig>,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct SideVirtualizerState {
    #[prost(message, optional, tag = "1")]
    pub state_header: Option<StateHeader>,
    #[prost(message, optional, tag = "2")]
    pub virtualizer_config: Option<VirtualizerConfig>,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct RearVirtualizerState {
    #[prost(message, optional, tag = "1")]
    pub state_header: Option<StateHeader>,
    #[prost(message, optional, tag = "2")]
    pub virtualizer_config: Option<VirtualizerConfig>,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct SpatialGlobalSettingsState {
    #[prost(message, optional, tag = "1")]
    pub state_header: Option<StateHeader>,
    #[prost(bool, tag = "2")]
    pub enabled: bool,
    #[prost(int32, tag = "3")]
    pub center_delay_samples: i32,
    #[prost(int32, tag = "4")]
    pub lfe_delay_samples: i32,
    #[prost(float, tag = "5")]
    pub center_gain_db: f32,
    #[prost(float, tag = "6")]
    pub lfe_gain_db: f32,
    #[prost(float, tag = "7")]
    pub front_gain_db: f32,
    #[prost(float, tag = "8")]
    pub side_gain_db: f32,
    #[prost(float, tag = "9")]
    pub rear_gain_db: f32,
    #[prost(bool, tag = "10")]
    pub center_muted: bool,
    #[prost(bool, tag = "11")]
    pub lfe_muted: bool,
    #[prost(bool, tag = "12")]
    pub front_l_muted: bool,
    #[prost(bool, tag = "13")]
    pub front_r_muted: bool,
    #[prost(bool, tag = "14")]
    pub side_l_muted: bool,
    #[prost(bool, tag = "15")]
    pub side_r_muted: bool,
    #[prost(bool, tag = "16")]
    pub rear_l_muted: bool,
    #[prost(bool, tag = "17")]
    pub rear_r_muted: bool,
    #[prost(message, repeated, tag = "18")]
    pub center_biquads: Vec<DisablableEqBand>,
    #[prost(message, repeated, tag = "19")]
    pub lfe_biquads: Vec<DisablableEqBand>,
}

#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
pub struct BassManagementState {
    #[prost(message, optional, tag = "1")]
    pub state_header: Option<StateHeader>,
    #[prost(bool, tag = "2")]
    pub enabled: bool,
    #[prost(float, tag = "3")]
    pub bass_cutoff_hz: f32,
    #[prost(float, tag = "4")]
    pub bass_gain_db: f32,
    #[prost(float, tag = "5")]
    pub bass_steering: f32,
}
