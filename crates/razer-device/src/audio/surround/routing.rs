//! Routing mutations translated from the three current client implementations.
use super::{Channel, Product, observed::Observations, protocol::*, wire::Payload};
use anyhow::{Context as _, Result, ensure};

fn mapping(input: i32, outputs: &[(i32, f32)]) -> ChannelMap {
    ChannelMap {
        input_channel: input,
        output_channels: outputs
            .iter()
            .map(|&(channel, gain_db)| ChannelProperties { channel, gain_db })
            .collect(),
    }
}
fn direct(count: i32) -> Vec<ChannelMap> {
    (0..count).map(|c| mapping(c, &[(c, 0.0)])).collect()
}
fn stereo() -> Vec<ChannelMap> {
    let mut maps = direct(2);
    maps.push(mapping(2, &[(0, -3.0), (1, -3.0)]));
    maps.push(mapping(3, &[(0, -3.0), (1, -3.0)]));
    maps
}

/// Carol retains the last selected front template for channel counts absent from its switch.
/// Its initialized template is the six-channel map at client RVA 0x202c90.
pub struct FrontMapper {
    product: Product,
    previous_channels: u16,
}
impl FrontMapper {
    pub fn new(product: Product) -> Self {
        Self {
            product,
            previous_channels: 6,
        }
    }
    pub fn map(&mut self, endpoint: &EndpointProperties) -> Result<ChannelMaps> {
        ensure!(
            endpoint.wave_format_extensible.len() == 40,
            "Invalid front wave format"
        );
        let channels = u16::from_le_bytes(endpoint.wave_format_extensible[2..4].try_into()?);
        if self.product == Product::Clio && !matches!(channels, 2 | 4 | 6 | 7) {
            let mut template = endpoint.clone();
            template.wave_format_extensible[2..4]
                .copy_from_slice(&self.previous_channels.to_le_bytes());
            front_map(self.product, &template)
        } else {
            self.previous_channels = channels;
            front_map(self.product, endpoint)
        }
    }
}

pub fn front_map(product: Product, endpoint: &EndpointProperties) -> Result<ChannelMaps> {
    ensure!(
        endpoint.wave_format_extensible.len() == 40,
        "Front endpoint wave format is invalid"
    );
    let channels = u16::from_le_bytes(endpoint.wave_format_extensible[2..4].try_into()?);
    let maps = match channels {
        2 => stereo(),
        4 => {
            let mut maps = stereo();
            maps.push(mapping(4, &[(2, 0.0)]));
            maps.push(mapping(5, &[(3, 0.0)]));
            maps
        }
        7 if product == Product::Clio => {
            let mut maps = direct(4);
            maps.push(mapping(4, &[(4, -3.0)]));
            maps.push(mapping(5, &[(5, -3.0)]));
            maps.push(mapping(4, &[(6, -3.0)]));
            maps.push(mapping(5, &[(7, -3.0)]));
            maps
        }
        8 if product == Product::Phantom => direct(8),
        8 if product == Product::VirtualRoute => {
            let mut maps = direct(4);
            maps.push(mapping(4, &[(4, -3.0)]));
            maps.push(mapping(5, &[(5, -3.0)]));
            maps
        }
        6 => direct(6),
        _ if product != Product::Clio => direct(6),
        _ => anyhow::bail!(
            "Carol source retains its previous front mapping for {channels} channels; no verified mapping is available for this connection"
        ),
    };
    Ok(ChannelMaps {
        endpoint_id: endpoint.endpoint_id.clone(),
        channel_maps: maps,
    })
}

pub fn rear_map(endpoint_id: &str, combo: bool, replicate: bool) -> ChannelMaps {
    let maps = if combo {
        let mut maps = if replicate { direct(2) } else { Vec::new() };
        maps.push(mapping(4, &[(0, 0.0)]));
        maps.push(mapping(5, &[(1, 0.0)]));
        maps
    } else {
        let mut maps = stereo();
        maps.push(mapping(4, &[(0, -3.0)]));
        maps.push(mapping(5, &[(1, -3.0)]));
        maps
    };
    ChannelMaps {
        endpoint_id: endpoint_id.to_owned(),
        channel_maps: maps,
    }
}

/// The global and front/rear filter definitions are fixed client data, not observations.
pub fn equalizer_bands(gains: &[f32]) -> Result<Vec<EqBand>> {
    let definitions = match gains.len() {
        3 => vec![
            (EqType::Peak, 80.0, 0.35),
            (EqType::Peak, 850.0, 0.23),
            (EqType::HighShelf, 4000.0, 0.5),
        ],
        10 => [
            31.0, 62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
        ]
        .into_iter()
        .map(|f| (EqType::Peak, f, std::f32::consts::FRAC_1_SQRT_2))
        .collect(),
        _ => anyhow::bail!("Unexpected routing EQ band count"),
    };
    ensure!(gains.iter().all(|v| v.is_finite()), "Non-finite EQ gain");
    Ok(definitions
        .into_iter()
        .zip(gains)
        .enumerate()
        .map(|(index, ((kind, frequency, q), gain))| EqBand {
            index: index as u32,
            filter_type: kind as i32,
            frequency,
            gain_db: *gain,
            q,
        })
        .collect())
}

impl Observations {
    pub fn volume_request(
        &self,
        endpoint: &str,
        volume: Option<f32>,
        mute: Option<bool>,
    ) -> Result<Payload> {
        let mut state = self
            .volumes
            .get(endpoint)
            .context("Endpoint volume is unobserved")?
            .clone();
        if let Some(value) = volume {
            ensure!((0.0..=1.0).contains(&value), "Invalid requested volume");
            state.volume = value;
        }
        if let Some(value) = mute {
            state.muted = value;
        }
        Ok(Payload::EndpointVolumeState(state))
    }
    pub fn map_request(&self, map: ChannelMaps) -> Result<Payload> {
        let mut state = self
            .channel_maps
            .clone()
            .context("Channel maps are unobserved")?;
        if let Some(current) = state
            .endpoint_channel_maps
            .iter_mut()
            .find(|v| v.endpoint_id == map.endpoint_id)
        {
            *current = map;
        } else {
            state.endpoint_channel_maps.push(map);
        }
        Ok(Payload::ChannelMapsState(state))
    }
    pub fn stream_request(&self, endpoint: &str, active: bool) -> Result<Payload> {
        let mut state = self
            .manager
            .clone()
            .context("WASAPI manager is unobserved")?;
        if active {
            let properties = self
                .endpoint(endpoint)
                .context("Stream endpoint is not present")?;
            let mut client_properties = vec![0; 16];
            client_properties[..4].copy_from_slice(&16u32.to_le_bytes());
            client_properties[8..12].copy_from_slice(&1u32.to_le_bytes());
            let stream = StreamProperties {
                endpoint_id: endpoint.to_owned(),
                wave_format_extensible: properties.wave_format_extensible.clone(),
                audio_client_properties: client_properties,
                stream_flags: 0x88040000,
            };
            if let Some(old) = state
                .active_streams
                .iter_mut()
                .find(|v| v.endpoint_id == endpoint)
            {
                *old = stream;
            } else {
                state.active_streams.push(stream);
            }
        } else {
            state.active_streams.retain(|v| v.endpoint_id != endpoint);
        }
        Ok(Payload::WasapiManagerState(state))
    }
    pub fn equalizer_request(&self, endpoint: &str, band: EqBand) -> Result<Payload> {
        let mut state = self
            .equalizer
            .clone()
            .context("Endpoint EQ is unobserved")?;
        if let Some(eq) = state
            .endpoint_eqs
            .iter_mut()
            .find(|v| v.endpoint_id == endpoint)
        {
            if let Some(old) = eq.eq_bands.iter_mut().find(|v| v.index == band.index) {
                *old = band;
            } else {
                eq.eq_bands.push(band);
            }
        } else {
            state.endpoint_eqs.push(EndpointEqs {
                endpoint_id: endpoint.to_owned(),
                eq_bands: vec![band],
            });
        }
        Ok(Payload::EqState(state))
    }
    pub fn delay_request(&self, endpoint: &str, milliseconds: f32) -> Result<Payload> {
        ensure!(
            milliseconds.is_finite() && milliseconds >= 0.0,
            "Invalid delay"
        );
        let mut state = self.delay.clone().context("Endpoint delay is unobserved")?;
        if let Some(delay) = state
            .endpoint_delays
            .iter_mut()
            .find(|v| v.endpoint_id == endpoint)
        {
            delay.delay_milliseconds = milliseconds;
        } else {
            state.endpoint_delays.push(EndpointDelay {
                endpoint_id: endpoint.to_owned(),
                delay_milliseconds: milliseconds,
            });
        }
        Ok(Payload::DelayState(state))
    }
    pub fn active_front(&self) -> Option<&str> {
        let rear = &self.endpoint_with_role(2).ok()?.endpoint_id;
        let active = self
            .streams
            .as_ref()?
            .endpoint_stream_state
            .iter()
            .filter(|s| matches!(s.stream_state, 0 | 1))
            .collect::<Vec<_>>();
        if active.len() <= 1 {
            return None;
        }
        active
            .into_iter()
            .find(|s| &s.endpoint_id != rear && self.endpoint(&s.endpoint_id).is_some())
            .map(|s| s.endpoint_id.as_str())
    }
    pub fn replicated(&self) -> Option<bool> {
        let rear = &self.endpoint_with_role(2).ok()?.endpoint_id;
        let map = self
            .channel_maps
            .as_ref()?
            .endpoint_channel_maps
            .iter()
            .find(|m| &m.endpoint_id == rear)?;
        Some(*map == rear_map(rear, true, true))
    }
}

pub fn same_committed_values(request: &Payload, response: &Payload) -> bool {
    match (request, response) {
        (Payload::EndpointVolumeState(a), Payload::EndpointVolumeState(b)) => {
            a.muted == b.muted && a.volume == b.volume
        }
        (Payload::ChannelMapsState(a), Payload::ChannelMapsState(b)) => {
            a.endpoint_channel_maps == b.endpoint_channel_maps
        }
        (Payload::WasapiManagerState(a), Payload::WasapiManagerState(b)) => {
            a.active_streams == b.active_streams
        }
        (Payload::EqState(a), Payload::EqState(b)) => a.endpoint_eqs == b.endpoint_eqs,
        (Payload::DelayState(a), Payload::DelayState(b)) => a.endpoint_delays == b.endpoint_delays,
        _ => false,
    }
}

pub fn endpoint_for<'a>(
    observed: &'a Observations,
    front: Option<&'a str>,
    channel: Channel,
) -> Result<&'a str> {
    match channel {
        Channel::Front => {
            let id = front.context("Front endpoint is not selected")?;
            ensure!(
                observed.endpoint(id).is_some(),
                "Selected front endpoint is disconnected"
            );
            Ok(id)
        }
        Channel::Rear => Ok(&observed.endpoint_with_role(2)?.endpoint_id),
    }
}
