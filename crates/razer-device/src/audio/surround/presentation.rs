//! Projection of actual routing publications into the mounted Surround controls.
//! The current front/rear maps, filter definitions and signed delay encoding are
//! those constructed by the routing client. Other configurations stay unknown.
use super::{Channel, Delay, Endpoint, Equalizer, Volume, observed::ServiceObservation, routing};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Observed<T> {
    Unobserved,
    Unknown,
    Value(T),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio_surround::protocol::*;

    fn service() -> ServiceObservation {
        let mut service = ServiceObservation::default();
        service.connected = true;
        service.front_endpoint = Some("front".into());
        service.observations.endpoints = Some(EndpointsState {
            state_header: None,
            endpoints_properties: [("front", 0), ("rear", 2)]
                .map(|(id, role)| EndpointProperties {
                    endpoint_id: id.into(),
                    friendly_name: id.into(),
                    thx_endpoint_id: role,
                    form_factor: 1,
                    wave_format_extensible: Vec::new(),
                })
                .to_vec(),
        });
        for (id, volume) in [("front", 0.37), ("rear", 0.83)] {
            service.observations.volumes.insert(
                id.into(),
                EndpointVolumeState {
                    state_header: None,
                    volume,
                    muted: false,
                },
            );
        }
        service.observations.channel_maps = Some(ChannelMapsState {
            state_header: None,
            vector_clock: Vec::new(),
            endpoint_channel_maps: vec![routing::rear_map("rear", true, true)],
        });
        service.observations.equalizer = Some(EqState {
            state_header: None,
            endpoint_eqs: vec![EndpointEqs {
                endpoint_id: "front".into(),
                eq_bands: routing::equalizer_bands(&[2., -3., 5.]).unwrap(),
            }],
        });
        service.observations.delay = Some(DelayState {
            state_header: None,
            endpoint_delays: vec![
                EndpointDelay {
                    endpoint_id: "front".into(),
                    delay_milliseconds: 0.,
                },
                EndpointDelay {
                    endpoint_id: "rear".into(),
                    delay_milliseconds: 37.,
                },
            ],
        });
        service
    }
    #[test]
    fn actual_values_reach_controls_without_default_or_cached_substitution() {
        let controls = Controls::from_service(&service());
        assert_eq!(controls.mode(), &Observed::Value(true));
        assert_eq!(controls.replicate(), &Observed::Value(true));
        assert_eq!(
            controls.volume(Channel::Front).value().unwrap().percent(),
            37
        );
        assert_eq!(
            controls.volume(Channel::Rear).value().unwrap().percent(),
            83
        );
        assert_eq!(
            controls.equalizer(Channel::Front).value().unwrap().bands(),
            [2, -3, 5]
        );
        assert_eq!(controls.equalizer(Channel::Rear), &Observed::Unobserved);
        assert_eq!(controls.delay().value().unwrap().milliseconds(), -37);
    }
    #[test]
    fn disconnected_service_cannot_expose_retained_publications_as_current() {
        let mut service = service();
        service.connected = false;
        assert_eq!(Controls::from_service(&service), Controls::default());
    }
    #[test]
    fn unknown_map_filter_and_dual_delays_remain_unknown() {
        let mut service = service();
        service
            .observations
            .channel_maps
            .as_mut()
            .unwrap()
            .endpoint_channel_maps[0]
            .channel_maps
            .clear();
        service
            .observations
            .equalizer
            .as_mut()
            .unwrap()
            .endpoint_eqs[0]
            .eq_bands[0]
            .frequency = 81.;
        service.observations.delay.as_mut().unwrap().endpoint_delays[0].delay_milliseconds = 1.;
        let controls = Controls::from_service(&service);
        assert_eq!(controls.mode(), &Observed::Unknown);
        assert_eq!(controls.replicate(), &Observed::Unknown);
        assert_eq!(controls.equalizer(Channel::Front), &Observed::Unknown);
        assert_eq!(controls.delay(), &Observed::Unknown);
    }
}
impl<T> Default for Observed<T> {
    fn default() -> Self {
        Self::Unobserved
    }
}
impl<T> Observed<T> {
    pub fn value(&self) -> Option<&T> {
        if let Self::Value(v) = self { Some(v) } else { None }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Controls {
    mode: Observed<bool>,
    replicate: Observed<bool>,
    front: Option<Endpoint>,
    front_volume: Observed<Volume>,
    rear_volume: Observed<Volume>,
    front_equalizer: Observed<Equalizer>,
    rear_equalizer: Observed<Equalizer>,
    delay: Observed<Delay>,
}
impl Controls {
    pub fn mode(&self) -> &Observed<bool> {
        &self.mode
    }
    pub fn replicate(&self) -> &Observed<bool> {
        &self.replicate
    }
    pub fn front(&self) -> Option<&Endpoint> {
        self.front.as_ref()
    }
    pub fn volume(&self, channel: Channel) -> &Observed<Volume> {
        match channel {
            Channel::Front => &self.front_volume,
            Channel::Rear => &self.rear_volume,
        }
    }
    pub fn equalizer(&self, channel: Channel) -> &Observed<Equalizer> {
        match channel {
            Channel::Front => &self.front_equalizer,
            Channel::Rear => &self.rear_equalizer,
        }
    }
    pub fn delay(&self) -> &Observed<Delay> {
        &self.delay
    }
    pub fn from_service(service: &ServiceObservation) -> Self {
        let Some(state) = service.current() else {
            return Self::default();
        };
        let front = service
            .front_endpoint
            .as_deref()
            .and_then(|id| state.endpoint(id));
        let rear = state.endpoint_with_role(2).ok();
        let volume = |endpoint: Option<&super::protocol::EndpointProperties>| {
            let Some(value) = endpoint.and_then(|e| state.volumes.get(&e.endpoint_id)) else {
                return Observed::Unobserved;
            };
            match Volume::from_native(value.volume) {
                Ok(v) => Observed::Value(v),
                Err(_) => Observed::Unknown,
            }
        };
        let eq = |endpoint: Option<&super::protocol::EndpointProperties>| {
            let Some(endpoint) = endpoint else {
                return Observed::Unobserved;
            };
            let Some(bands) = state.equalizer.as_ref().and_then(|eq| {
                eq.endpoint_eqs
                    .iter()
                    .find(|e| e.endpoint_id == endpoint.endpoint_id)
            }) else {
                return Observed::Unobserved;
            };
            let expected =
                routing::equalizer_bands(&[0.; 3]).expect("fixed three-band definitions");
            if bands.eq_bands.len() != expected.len() {
                return Observed::Unknown;
            }
            let mut gains = [0; 3];
            for definition in expected {
                let Some(band) = bands.eq_bands.iter().find(|b| b.index == definition.index) else {
                    return Observed::Unknown;
                };
                if band.filter_type != definition.filter_type
                    || band.frequency != definition.frequency
                    || band.q != definition.q
                    || !band.gain_db.is_finite()
                    || band.gain_db.fract() != 0.
                    || !(-6. ..=6.).contains(&band.gain_db)
                {
                    return Observed::Unknown;
                }
                gains[definition.index as usize] = band.gain_db as i8;
            }
            Observed::Value(Equalizer::new(gains).expect("validated three-band gains"))
        };
        let (mode, replicate) = match rear.and_then(|rear| {
            state.channel_maps.as_ref().and_then(|maps| {
                maps.endpoint_channel_maps
                    .iter()
                    .find(|m| m.endpoint_id == rear.endpoint_id)
            })
        }) {
            None => (Observed::Unobserved, Observed::Unobserved),
            Some(map) if *map == routing::rear_map(&map.endpoint_id, false, false) => {
                (Observed::Value(false), Observed::Unobserved)
            }
            Some(map) if *map == routing::rear_map(&map.endpoint_id, true, false) => {
                (Observed::Value(true), Observed::Value(false))
            }
            Some(map) if *map == routing::rear_map(&map.endpoint_id, true, true) => {
                (Observed::Value(true), Observed::Value(true))
            }
            Some(_) => (Observed::Unknown, Observed::Unknown),
        };
        let delay = match (front, rear, &state.delay) {
            (Some(front), Some(rear), Some(delays)) => {
                let f = delays
                    .endpoint_delays
                    .iter()
                    .find(|d| d.endpoint_id == front.endpoint_id);
                let r = delays
                    .endpoint_delays
                    .iter()
                    .find(|d| d.endpoint_id == rear.endpoint_id);
                match (f, r) {
                    (Some(f), Some(r)) => {
                        let (f, r) = (f.delay_milliseconds, r.delay_milliseconds);
                        if !f.is_finite()
                            || !r.is_finite()
                            || f < 0.
                            || r < 0.
                            || f.fract() != 0.
                            || r.fract() != 0.
                            || f > 100.
                            || r > 100.
                            || (f != 0. && r != 0.)
                        {
                            Observed::Unknown
                        } else {
                            Observed::Value(
                                Delay::new((f - r) as i16).expect("validated signed delay"),
                            )
                        }
                    }
                    _ => Observed::Unobserved,
                }
            }
            _ => Observed::Unobserved,
        };
        Self {
            mode,
            replicate,
            front: front
                .and_then(|e| Endpoint::new(e.endpoint_id.clone(), e.friendly_name.clone()).ok()),
            front_volume: volume(front),
            rear_volume: volume(rear),
            front_equalizer: eq(front),
            rear_equalizer: eq(rear),
            delay,
        }
    }
}
