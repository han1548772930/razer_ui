use super::*;
fn counts() -> ChannelCounts {
    ChannelCounts {
        device_output: 2,
        device_input: 3,
        app_playback: 2,
        app_recording: 1,
        app_playback_virtual: 18,
        app_recording_virtual: 4,
    }
}
fn mixer(mode: Mode) -> Mixer {
    let mut mixer = Mixer::new(counts(), mode).unwrap();
    mixer
        .observe_endpoint(
            MICROPHONE,
            EndpointObservation {
                level: 43,
                muted: false,
            },
        )
        .unwrap();
    mixer
}
#[test]
fn source_node_defaults_are_drafts_and_missing_mic_observation_cannot_enable_monitor() {
    let mut mixer = Mixer::new(counts(), Mode::Advanced).unwrap();
    assert_eq!(mixer.endpoint_observation(MICROPHONE), None);
    assert!(
        mixer
            .set_route_enabled(PLAYBACK, MICROPHONE_ALIAS, true)
            .is_err()
    );
    mixer
        .observe_endpoint(
            MICROPHONE,
            EndpointObservation {
                level: 43,
                muted: true,
            },
        )
        .unwrap();
    let writes = mixer
        .set_route_enabled(PLAYBACK, MICROPHONE_ALIAS, true)
        .unwrap();
    assert_eq!(
        writes[1],
        Write::Control(AudioControlWrite::monitor_enabled(false))
    );
    assert_eq!(mixer.endpoint_observation(MICROPHONE).unwrap().level, 43);
}

#[test]
fn disconnected_mic_cannot_drive_monitor_from_old_observation() {
    let mut mixer = mixer(Mode::Advanced);
    mixer.set_route_enabled(PLAYBACK, MICROPHONE, true).unwrap();
    mixer.invalidate_endpoint(MICROPHONE);
    assert_eq!(mixer.endpoint_observation(MICROPHONE), None);
    assert!(mixer.refresh_monitor().is_err());
    mixer
        .observe_endpoint(
            MICROPHONE,
            EndpointObservation {
                level: 17,
                muted: true,
            },
        )
        .unwrap();
    assert_eq!(
        mixer.refresh_monitor().unwrap()[0],
        Write::Control(AudioControlWrite::monitor_enabled(false))
    );
}
#[test]
fn mono_mic_duplicates_first_input_for_stereo_routes_and_monitor_uses_usb_control() {
    let mut mixer = mixer(Mode::Advanced);
    let writes = mixer
        .set_route_enabled(PLAYBACK, MICROPHONE_ALIAS, true)
        .unwrap();
    assert_eq!(writes.len(), 6);
    assert_eq!(
        writes[0],
        Write::Dsp(vec![
            77, 0, 0, 0, 200, 0, 0, 0, 1, 0, 2, 2, 0, 0, 0, 1, 1, 0, 2, 3, 0, 0, 0, 1
        ])
    );
    assert_eq!(
        writes[1],
        Write::Control(AudioControlWrite::monitor_enabled(true))
    );
    assert_eq!(
        writes[5],
        Write::Dsp(vec![
            77, 0, 0, 0, 200, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 1, 0, 1, 1, 0, 0, 0, 0
        ])
    );
}
#[test]
fn mix_level_first_enables_old_level_then_sends_new_level_with_truncated_monitor_product() {
    let mut mixer = mixer(Mode::Advanced);
    mixer.set_volume(PLAYBACK, 33).unwrap();
    let writes = mixer.set_route_level(PLAYBACK, MICROPHONE, 47).unwrap();
    assert_eq!(writes.len(), 12);
    assert_eq!(
        writes[2],
        Write::Control(AudioControlWrite::monitor_level(33).unwrap()[0].clone())
    );
    assert_eq!(
        writes[8],
        Write::Control(AudioControlWrite::monitor_level(15).unwrap()[0].clone())
    );
}
#[test]
fn basic_mode_keeps_saved_monitor_and_forces_headphone_playback_unity() {
    let mut mixer = mixer(Mode::Basic);
    mixer.set_monitor_level(27).unwrap();
    mixer.set_monitor_enabled(true);
    let writes = mixer.set_route_level(PLAYBACK, MICROPHONE, 1).unwrap();
    assert_eq!(
        writes[8],
        Write::Control(AudioControlWrite::monitor_level(27).unwrap()[0].clone())
    );
    let writes = mixer
        .set_route_enabled(PLAYBACK, HEADPHONES_INPUT, false)
        .unwrap();
    let Write::Dsp(bytes) = &writes[1] else {
        panic!("missing HP route");
    };
    assert_eq!(&bytes[12..16], &[0, 0, 0, 1]);
    let writes = mixer.set_playback_external(true).unwrap();
    let Write::Dsp(bytes) = &writes[1] else {
        panic!("missing HP gain");
    };
    assert_eq!(&bytes[10..14], &[0, 0, 0, 1]);
}
#[test]
fn stream_monitor_projects_stream_to_playback_and_hp_and_suppresses_playback_draft_writes() {
    let mut mixer = mixer(Mode::Advanced);
    mixer.set_stream_monitor(true).unwrap();
    assert!(
        mixer
            .set_route_enabled(PLAYBACK, 65537, true)
            .unwrap()
            .is_empty()
    );
    assert!(mixer.set_volume(PLAYBACK, 42).unwrap().is_empty());
    assert_eq!(
        mixer.set_route_enabled(STREAM, 65537, true).unwrap().len(),
        3
    );
    let writes = mixer.set_stream_monitor(false).unwrap();
    let Write::Dsp(bytes) = &writes[0] else {
        panic!("missing playback gain");
    };
    assert_eq!(
        u32::from_le_bytes(bytes[10..14].try_into().unwrap()),
        driver::gain(42, true).unwrap()
    );
}
#[test]
fn meters_filter_basic_mode_and_missing_channels_are_errors_instead_of_zero() {
    let mixer = mixer(Mode::Basic);
    let readings = driver::meter_channels(counts())
        .unwrap()
        .into_iter()
        .map(|channel| MeterReading {
            channel,
            hundredths_db: -3000,
        })
        .collect::<Vec<_>>();
    let levels = mixer.meter_levels(&readings).unwrap();
    assert_eq!(
        levels,
        BTreeMap::from([(MICROPHONE, vec![50]), (HEADPHONES, vec![50, 50])])
    );
    assert!(mixer.meter_levels(&[]).is_err());
}
#[test]
fn unknown_modes_and_truncated_channel_pairs_are_not_invented_capabilities() {
    assert!(Mixer::new(counts(), Mode::Unknown("future".into())).is_err());
    let mut counts = counts();
    counts.app_recording_virtual = 3;
    assert!(Mixer::new(counts, Mode::Advanced).is_err());
}
