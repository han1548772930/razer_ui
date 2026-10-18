//! Protobuf Any envelope used by the installed routing service's REQ/PUB sockets.
use super::protocol::*;
use anyhow::{Context as _, Result, bail, ensure};
use prost::Message;
use serde::{Deserialize, Serialize};

const TYPE_PREFIX: &str = "type.googleapis.com/thx_wasapi_routing_proto.";

#[derive(Clone, PartialEq, Message)]
struct Any {
    #[prost(string, tag = "1")]
    type_url: String,
    #[prost(bytes = "vec", tag = "2")]
    value: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Payload {
    ClientConnect(ClientConnect),
    ClientConnectAcknowledge(ClientConnectAcknowledge),
    WasapiManagerState(WasapiManagerState),
    StreamStates(StreamStates),
    ChannelMapsState(ChannelMapsState),
    EqState(EqState),
    DelayState(DelayState),
    EndpointsState(EndpointsState),
    EndpointVolumeState(EndpointVolumeState),
    FrontCtcState(FrontCtcState),
    SideCtcState(SideCtcState),
    RearCtcState(RearCtcState),
    FrontVirtualizerState(FrontVirtualizerState),
    SideVirtualizerState(SideVirtualizerState),
    RearVirtualizerState(RearVirtualizerState),
    SpatialGlobalSettingsState(SpatialGlobalSettingsState),
    BassManagementState(BassManagementState),
    PlayDelayTestSignal(PlayDelayTestSignal),
    StopDelayTestSignal(StopDelayTestSignal),
}

impl Payload {
    pub fn name(&self) -> &'static str {
        match self {
            Self::ClientConnect(..) => "ClientConnect",
            Self::ClientConnectAcknowledge(..) => "ClientConnectAcknowledge",
            Self::WasapiManagerState(..) => "WasapiManagerState",
            Self::StreamStates(..) => "StreamStates",
            Self::ChannelMapsState(..) => "ChannelMapsState",
            Self::EqState(..) => "EqState",
            Self::DelayState(..) => "DelayState",
            Self::EndpointsState(..) => "EndpointsState",
            Self::EndpointVolumeState(..) => "EndpointVolumeState",
            Self::FrontCtcState(..) => "FrontCtcState",
            Self::SideCtcState(..) => "SideCtcState",
            Self::RearCtcState(..) => "RearCtcState",
            Self::FrontVirtualizerState(..) => "FrontVirtualizerState",
            Self::SideVirtualizerState(..) => "SideVirtualizerState",
            Self::RearVirtualizerState(..) => "RearVirtualizerState",
            Self::SpatialGlobalSettingsState(..) => "SpatialGlobalSettingsState",
            Self::BassManagementState(..) => "BassManagementState",
            Self::PlayDelayTestSignal(..) => "PlayDelayTestSignal",
            Self::StopDelayTestSignal(..) => "StopDelayTestSignal",
        }
    }
    pub fn encode(&self) -> Vec<u8> {
        let value = match self {
            Self::ClientConnect(v) => v.encode_to_vec(),
            Self::ClientConnectAcknowledge(v) => v.encode_to_vec(),
            Self::WasapiManagerState(v) => v.encode_to_vec(),
            Self::StreamStates(v) => v.encode_to_vec(),
            Self::ChannelMapsState(v) => v.encode_to_vec(),
            Self::EqState(v) => v.encode_to_vec(),
            Self::DelayState(v) => v.encode_to_vec(),
            Self::EndpointsState(v) => v.encode_to_vec(),
            Self::EndpointVolumeState(v) => v.encode_to_vec(),
            Self::FrontCtcState(v) => v.encode_to_vec(),
            Self::SideCtcState(v) => v.encode_to_vec(),
            Self::RearCtcState(v) => v.encode_to_vec(),
            Self::FrontVirtualizerState(v) => v.encode_to_vec(),
            Self::SideVirtualizerState(v) => v.encode_to_vec(),
            Self::RearVirtualizerState(v) => v.encode_to_vec(),
            Self::SpatialGlobalSettingsState(v) => v.encode_to_vec(),
            Self::BassManagementState(v) => v.encode_to_vec(),
            Self::PlayDelayTestSignal(v) => v.encode_to_vec(),
            Self::StopDelayTestSignal(v) => v.encode_to_vec(),
        };
        Any { type_url: format!("{TYPE_PREFIX}{}", self.name()), value }.encode_to_vec()
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let envelope = Any::decode(bytes).context("Invalid routing protobuf envelope")?;
        let name = envelope.type_url.strip_prefix(TYPE_PREFIX).context("Unexpected routing protobuf namespace")?;
        let bytes = envelope.value.as_slice();
        let payload = match name {
            "ClientConnect" => Self::ClientConnect(ClientConnect::decode(bytes)?),
            "ClientConnectAcknowledge" => Self::ClientConnectAcknowledge(ClientConnectAcknowledge::decode(bytes)?),
            "WasapiManagerState" => Self::WasapiManagerState(WasapiManagerState::decode(bytes)?),
            "StreamStates" => Self::StreamStates(StreamStates::decode(bytes)?),
            "ChannelMapsState" => Self::ChannelMapsState(ChannelMapsState::decode(bytes)?),
            "EqState" => Self::EqState(EqState::decode(bytes)?),
            "DelayState" => Self::DelayState(DelayState::decode(bytes)?),
            "EndpointsState" => Self::EndpointsState(EndpointsState::decode(bytes)?),
            "EndpointVolumeState" => Self::EndpointVolumeState(EndpointVolumeState::decode(bytes)?),
            "FrontCtcState" => Self::FrontCtcState(FrontCtcState::decode(bytes)?),
            "SideCtcState" => Self::SideCtcState(SideCtcState::decode(bytes)?),
            "RearCtcState" => Self::RearCtcState(RearCtcState::decode(bytes)?),
            "FrontVirtualizerState" => Self::FrontVirtualizerState(FrontVirtualizerState::decode(bytes)?),
            "SideVirtualizerState" => Self::SideVirtualizerState(SideVirtualizerState::decode(bytes)?),
            "RearVirtualizerState" => Self::RearVirtualizerState(RearVirtualizerState::decode(bytes)?),
            "SpatialGlobalSettingsState" => Self::SpatialGlobalSettingsState(SpatialGlobalSettingsState::decode(bytes)?),
            "BassManagementState" => Self::BassManagementState(BassManagementState::decode(bytes)?),
            "PlayDelayTestSignal" => Self::PlayDelayTestSignal(PlayDelayTestSignal::decode(bytes)?),
            "StopDelayTestSignal" => Self::StopDelayTestSignal(StopDelayTestSignal::decode(bytes)?),
            _ => bail!("Unknown routing message {name}"),
        };
        payload.validate()?;
        Ok(payload)
    }
    pub fn header(&self) -> Option<&StateHeader> {
        match self {
            Self::WasapiManagerState(v) => v.state_header.as_ref(),
            Self::StreamStates(v) => v.state_header.as_ref(),
            Self::ChannelMapsState(v) => v.state_header.as_ref(),
            Self::EqState(v) => v.state_header.as_ref(),
            Self::DelayState(v) => v.state_header.as_ref(),
            Self::EndpointsState(v) => v.state_header.as_ref(),
            Self::EndpointVolumeState(v) => v.state_header.as_ref(),
            Self::FrontCtcState(v) => v.state_header.as_ref(),
            Self::SideCtcState(v) => v.state_header.as_ref(),
            Self::RearCtcState(v) => v.state_header.as_ref(),
            Self::FrontVirtualizerState(v) => v.state_header.as_ref(),
            Self::SideVirtualizerState(v) => v.state_header.as_ref(),
            Self::RearVirtualizerState(v) => v.state_header.as_ref(),
            Self::SpatialGlobalSettingsState(v) => v.state_header.as_ref(),
            Self::BassManagementState(v) => v.state_header.as_ref(),
            _ => None,
        }
    }
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::ClientConnect(v) => ensure!(!v.file_version.is_empty() && v.api_version == "6", "Unsupported routing client handshake"),
            Self::ClientConnectAcknowledge(v) => ensure!(!v.file_version.is_empty() && v.api_version == "6", "Unsupported routing service handshake"),
            Self::PlayDelayTestSignal(_) | Self::StopDelayTestSignal(_) => {},
            _ => {
                let header = self.header().context("Routing state header is absent")?;
                ensure!(!header.id.is_empty() && !header.id.contains('\0'), "Invalid routing state identity");
                if !matches!(self, Self::EndpointVolumeState(_)) {
                    ensure!(header.id == "GLOBAL", "Unexpected global routing state identity");
                }
            }
        }
        match self {
            Self::EndpointsState(v) => {
                let mut ids = std::collections::BTreeSet::new();
                for e in &v.endpoints_properties {
                    ensure!(!e.endpoint_id.is_empty() && ids.insert(&e.endpoint_id), "Missing or duplicate routing endpoint");
                    ensure!(e.wave_format_extensible.len() == 40, "Invalid endpoint WAVEFORMATEXTENSIBLE length");
                }
            }
            Self::EndpointVolumeState(v) => ensure!(v.volume.is_finite() && (0.0..=1.0).contains(&v.volume), "Invalid endpoint volume"),
            Self::EqState(v) => {
                for e in &v.endpoint_eqs {
                    ensure!(!e.endpoint_id.is_empty(), "EQ endpoint is absent");
                    for band in &e.eq_bands {
                        ensure!(band.frequency.is_finite() && band.gain_db.is_finite() && band.q.is_finite(), "Invalid EQ parameters");
                    }
                }
            }
            Self::DelayState(v) => {
                for e in &v.endpoint_delays {
                    ensure!(!e.endpoint_id.is_empty() && e.delay_milliseconds.is_finite() && e.delay_milliseconds >= 0.0, "Invalid endpoint delay");
                }
            }
            Self::ChannelMapsState(v) => {
                for e in &v.endpoint_channel_maps {
                    ensure!(!e.endpoint_id.is_empty(), "Channel-map endpoint is absent");
                    for map in &e.channel_maps {
                        ensure!(map.input_channel >= 0, "Invalid input channel");
                        ensure!(map.output_channels.iter().all(|c| c.channel >= 0 && c.gain_db.is_finite()), "Invalid output channel");
                    }
                }
            }
            _ => {},
        }
        Ok(())
    }
}
