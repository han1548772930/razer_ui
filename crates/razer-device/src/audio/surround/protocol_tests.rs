use super::{
    Product,
    observed::{Observations, Update},
    protocol::*,
    routing::*,
    wire::Payload,
};

fn header(id: &str, revision: u64) -> Option<StateHeader> {
    Some(StateHeader {
        id: id.to_owned(),
        revision,
    })
}
fn endpoint(id: &str, channels: u16) -> EndpointProperties {
    let mut wave = vec![0; 40];
    wave[2..4].copy_from_slice(&channels.to_le_bytes());
    EndpointProperties {
        endpoint_id: id.to_owned(),
        friendly_name: id.to_owned(),
        form_factor: 1,
        wave_format_extensible: wave,
        thx_endpoint_id: 0,
    }
}

#[test]
fn independent_protobuf_fixture_preserves_actual_volume_and_mute() {
    let payload = Payload::decode(include_bytes!(
        "../../../testdata/re/surround-volume-observation.bin"
    ))
    .unwrap();
    let Payload::EndpointVolumeState(state) = payload else {
        panic!("wrong type")
    };
    assert_eq!(state.state_header, header("front-device", 9));
    assert!(state.muted);
    assert_eq!(state.volume, 0.37);
}

#[test]
fn malformed_or_missing_headers_are_errors_not_empty_observations() {
    let bytes = include_bytes!("../../../testdata/re/surround-volume-observation.bin");
    assert!(Payload::decode(&bytes[..bytes.len() - 1]).is_err());
    assert!(Payload::decode(&[]).is_err());
    let missing = Payload::EndpointVolumeState(EndpointVolumeState {
        state_header: None,
        muted: false,
        volume: 0.5,
    });
    assert!(Payload::decode(&missing.encode()).is_err());
    let invalid = Payload::EndpointVolumeState(EndpointVolumeState {
        state_header: header("front", 1),
        muted: false,
        volume: f32::NAN,
    });
    assert!(Payload::decode(&invalid.encode()).is_err());
}

#[test]
fn stale_and_conflicting_revisions_do_not_overwrite_actual_values() {
    let mut observed = Observations::default();
    let state = EndpointVolumeState {
        state_header: header("front", 10),
        muted: true,
        volume: 0.83,
    };
    assert_eq!(
        observed
            .apply(Payload::EndpointVolumeState(state.clone()))
            .unwrap(),
        Update::Changed
    );
    assert_eq!(
        observed
            .apply(Payload::EndpointVolumeState(state.clone()))
            .unwrap(),
        Update::Duplicate
    );
    let mut stale = state.clone();
    stale.state_header.as_mut().unwrap().revision = 9;
    stale.volume = 0.2;
    assert_eq!(
        observed.apply(Payload::EndpointVolumeState(stale)).unwrap(),
        Update::Stale
    );
    let mut conflict = state.clone();
    conflict.volume = 0.3;
    assert!(
        observed
            .apply(Payload::EndpointVolumeState(conflict))
            .is_err()
    );
    assert_eq!(observed.volumes["front"], state);
}

#[test]
fn mutations_preserve_service_revision_mute_and_unrelated_endpoint_fields() {
    let mut observed = Observations::default();
    observed
        .apply(Payload::EndpointVolumeState(EndpointVolumeState {
            state_header: header("front", 27),
            muted: true,
            volume: 0.4,
        }))
        .unwrap();
    let Payload::EndpointVolumeState(request) =
        observed.volume_request("front", Some(0.7), None).unwrap()
    else {
        panic!()
    };
    assert!(request.muted);
    assert_eq!(request.state_header, header("front", 27));
    assert_eq!(observed.volumes["front"].volume, 0.4);
    assert!(observed.volume_request("absent", Some(0.7), None).is_err());
    observed.channel_maps = Some(ChannelMapsState {
        state_header: header("GLOBAL", 8),
        vector_clock: vec![3, 8],
        endpoint_channel_maps: vec![rear_map("unrelated", false, false)],
    });
    let Payload::ChannelMapsState(request) =
        observed.map_request(rear_map("rear", true, true)).unwrap()
    else {
        panic!()
    };
    assert_eq!(request.vector_clock, vec![3, 8]);
    assert_eq!(request.state_header, header("GLOBAL", 8));
    assert_eq!(
        request.endpoint_channel_maps[0],
        rear_map("unrelated", false, false)
    );
    assert_eq!(request.endpoint_channel_maps.len(), 2);
}

#[test]
fn equalizer_mutation_preserves_other_bands_and_uses_verified_filter_definitions() {
    let mut observed = Observations::default();
    let original = EqBand {
        index: 7,
        filter_type: 21,
        frequency: 347.0,
        gain_db: 2.4,
        q: 0.72,
    };
    observed.equalizer = Some(EqState {
        state_header: header("GLOBAL", 3),
        endpoint_eqs: vec![EndpointEqs {
            endpoint_id: "front".to_owned(),
            eq_bands: vec![original.clone()],
        }],
    });
    let bands = equalizer_bands(&[1.0, -2.0, 3.0]).unwrap();
    assert_eq!(
        (bands[0].frequency, bands[1].frequency, bands[2].frequency),
        (80.0, 850.0, 4000.0)
    );
    assert_eq!(bands[2].filter_type, EqType::HighShelf as i32);
    let Payload::EqState(request) = observed
        .equalizer_request("front", bands[0].clone())
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(
        request.endpoint_eqs[0].eq_bands,
        vec![original, bands[0].clone()]
    );
}

#[test]
fn front_maps_keep_product_specific_seven_and_eight_channel_branches() {
    let clio = front_map(Product::Clio, &endpoint("front", 7)).unwrap();
    assert_eq!(clio.channel_maps.len(), 8);
    assert_eq!(clio.channel_maps[6].input_channel, 4);
    assert_eq!(
        clio.channel_maps[6].output_channels[0],
        ChannelProperties {
            channel: 6,
            gain_db: -3.0
        }
    );
    let phantom = front_map(Product::Phantom, &endpoint("front", 8)).unwrap();
    assert_eq!(phantom.channel_maps.len(), 8);
    assert_eq!(phantom.channel_maps[7].input_channel, 7);
    let soma = front_map(Product::VirtualRoute, &endpoint("front", 8)).unwrap();
    assert_eq!(soma.channel_maps.len(), 6);
    assert_eq!(soma.channel_maps[4].output_channels[0].gain_db, -3.0);
}

#[test]
fn carol_retains_previous_template_for_unhandled_channel_count() {
    let mut mapper = FrontMapper::new(Product::Clio);
    assert_eq!(
        mapper
            .map(&endpoint("front", 8))
            .unwrap()
            .channel_maps
            .len(),
        6
    );
    let stereo = mapper.map(&endpoint("front", 2)).unwrap();
    assert_eq!(mapper.map(&endpoint("front", 8)).unwrap(), stereo);
}

#[test]
fn stream_start_copies_observed_format_and_preserves_other_streams() {
    let mut observed = Observations::default();
    let ep = endpoint("front", 4);
    observed.endpoints = Some(EndpointsState {
        state_header: header("GLOBAL", 2),
        endpoints_properties: vec![ep.clone()],
    });
    let other = StreamProperties {
        endpoint_id: "rear".to_owned(),
        wave_format_extensible: vec![4; 40],
        audio_client_properties: vec![7; 16],
        stream_flags: 42,
    };
    observed.manager = Some(WasapiManagerState {
        state_header: header("GLOBAL", 4),
        active_streams: vec![other.clone()],
    });
    let Payload::WasapiManagerState(request) = observed.stream_request("front", true).unwrap()
    else {
        panic!()
    };
    assert_eq!(request.active_streams[0], other);
    assert_eq!(
        request.active_streams[1].wave_format_extensible,
        ep.wave_format_extensible
    );
    assert_eq!(request.active_streams[1].stream_flags, 0x88040000);
    assert_eq!(
        request.active_streams[1].audio_client_properties,
        vec![16, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0]
    );
}

#[test]
fn unknown_stream_values_cannot_select_a_front_endpoint() {
    let mut observed = Observations::default();
    let mut rear = endpoint("rear", 2);
    rear.thx_endpoint_id = 2;
    observed.endpoints = Some(EndpointsState {
        state_header: header("GLOBAL", 1),
        endpoints_properties: vec![rear, endpoint("front", 2)],
    });
    observed.streams = Some(StreamStates {
        state_header: header("GLOBAL", 2),
        endpoint_stream_state: vec![
            EndpointStreamState {
                endpoint_id: "rear".to_owned(),
                stream_state: 1,
            },
            EndpointStreamState {
                endpoint_id: "front".to_owned(),
                stream_state: 99,
            },
        ],
    });
    assert_eq!(observed.active_front(), None);
    observed.streams.as_mut().unwrap().endpoint_stream_state[1].stream_state = 0;
    assert_eq!(observed.active_front(), Some("front"));
}

#[test]
fn committed_values_must_match_the_request() {
    let requested = Payload::EndpointVolumeState(EndpointVolumeState {
        state_header: header("front", 4),
        muted: false,
        volume: 0.2,
    });
    let actual = Payload::EndpointVolumeState(EndpointVolumeState {
        state_header: header("front", 5),
        muted: false,
        volume: 0.83,
    });
    assert!(!same_committed_values(&requested, &actual));
}
