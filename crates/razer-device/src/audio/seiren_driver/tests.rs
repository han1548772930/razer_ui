use super::*;
use std::cell::RefCell;

struct Script(RefCell<Vec<(u32, Vec<u8>, usize, Result<Vec<u8>>)>>);
impl Transport for Script {
    fn exchange(&self, code: u32, input: &[u8], capacity: usize) -> Result<Vec<u8>> {
        let (expected, bytes, size, result) = self.0.borrow_mut().remove(0);
        assert_eq!((code, input, capacity), (expected, bytes.as_slice(), size));
        result
    }
}
fn script(code: u32, input: Vec<u8>, capacity: usize, response: Vec<u8>) -> Script {
    Script(RefCell::new(vec![(code, input, capacity, Ok(response))]))
}

#[test]
fn mix_route_is_packed_little_endian_and_does_not_swap_source_destination() {
    let routes = [Route {
        input: Channel { kind: ChannelKind::DeviceInput, index: 2 },
        output: Channel { kind: ChannelKind::AppRecordingVirtual, index: 3 },
    }];
    assert_eq!(route_gain(&routes, 50, true).unwrap(),
        [77, 0, 0, 0, 200, 0, 0, 0, 1, 2, 2, 3, 0, 0, 128, 0]);
    assert_eq!(&route_gain(&routes, 99, false).unwrap()[12..], &[0, 0, 0, 0]);
    let reversed = [Route { input: routes[0].output, output: routes[0].input }];
    assert!(route_gain(&reversed, 50, true).is_err());
    assert!(route_gain(&routes, 101, true).is_err());
}

#[test]
fn output_gain_uses_six_byte_entries_and_unsigned_fixed_point_truncation() {
    let channels = [Channel { kind: ChannelKind::DeviceOutput, index: 0 }, Channel { kind: ChannelKind::DeviceOutput, index: 1 }];
    assert_eq!(output_gain(&channels, 100, true).unwrap(),
        [77, 0, 0, 0, 201, 0, 0, 0, 1, 0, 0, 0, 0, 1, 1, 1, 0, 0, 0, 1]);
    assert_eq!(gain(33, true).unwrap(), 5_536_481);
    assert_eq!(meter_enable(true), [77, 0, 0, 0, 44, 1, 0, 0, 1, 0, 0, 0]);
}

#[test]
fn actual_sample_rate_survives_and_truncated_or_failed_reads_do_not_become_defaults() {
    assert_eq!(sample_rate(&script(0x80882104, vec![], 4, vec![0, 119, 1, 0])).unwrap(), 96_000);
    for data in [vec![], vec![0, 119, 1], vec![0; 4], vec![0; 5]] {
        assert!(sample_rate(&script(0x80882104, vec![], 4, data)).is_err());
    }
    let transport = Script(RefCell::new(vec![(0x80882104, vec![], 4, Err(anyhow::anyhow!("disconnected")))]));
    assert!(sample_rate(&transport).is_err());
}

fn formats(count: u32, selected: i32) -> Vec<u8> {
    let mut data = vec![0; 8 + 2 * 332];
    data[..4].copy_from_slice(&count.to_le_bytes());
    data[4..8].copy_from_slice(&selected.to_le_bytes());
    data[8..12].copy_from_slice(&17u32.to_le_bytes());
    data[340..344].copy_from_slice(&83u32.to_le_bytes());
    data
}

#[test]
fn current_format_uses_returned_selection_and_rejects_absent_out_of_bounds_or_short_lists() {
    assert_eq!(current_stream_id(&script(0x808821c4, vec![], 10632, formats(2, 1)), Flow::Playback).unwrap(), 83);
    for (count, selected) in [(2, -1), (2, 2), (33, 0), (0, 0), (3, 1)] {
        assert!(current_stream_id(&script(0x808821c0, vec![], 10632, formats(count, selected)), Flow::Capture).is_err());
    }
}

#[test]
fn mode_preserves_unknown_profile_and_does_not_infer_from_stream_channel_count() {
    for (name, expected) in [("plb_base", Mode::Basic), ("plb_advanced", Mode::Advanced), ("future", Mode::Unknown("future".into()))] {
        let response = name.encode_utf16().chain(Some(0)).flat_map(u16::to_le_bytes).collect();
        let transport = Script(RefCell::new(vec![
            (0x808821c4, vec![], 10632, Ok(formats(2, 1))),
            (0x80882428, vec![83, 0, 0, 0], 384, Ok(response)),
        ]));
        assert_eq!(mode(&transport).unwrap(), expected);
        assert!(transport.0.borrow().is_empty());
    }
}

#[test]
fn profile_write_preserves_actual_stream_ids_and_fixed_utf16_field_boundaries() {
    let bytes = profiles(17, 83, true);
    assert_eq!(&bytes[..4], &[17, 0, 0, 0]);
    assert_eq!(&bytes[132..136], &[83, 0, 0, 0]);
    assert_eq!(&bytes[4..12], &[b'p', 0, b'l', 0, b'b', 0, b'_', 0]);
    assert_eq!(&bytes[136..144], &[b'r', 0, b'e', 0, b'c', 0, b'_', 0]);
    assert!(bytes[28..132].iter().chain(bytes[160..].iter()).all(|b| *b == 0));
}

#[test]
fn driver_handshake_checks_protocol_values_without_source_binary_hashes() {
    let mut data = vec![0; 24];
    data[0] = 6; data[4] = 10;
    assert!(verify_driver(&script(0x80882004, vec![], 24, data.clone()), Product::Pro).is_ok());
    assert!(verify_driver(&script(0x80882004, vec![], 24, data.clone()), Product::Chroma).is_err());
    data[4] = 14;
    assert!(verify_driver(&script(0x80882004, vec![], 24, data.clone()), Product::Chroma).is_ok());
    assert!(verify_driver(&script(0x80882004, vec![], 24, data.clone()), Product::Pro).is_err());
    data[4] = 9;
    assert!(verify_driver(&script(0x80882004, vec![], 24, data), Product::Pro).is_err());
}

#[test]
fn product_branch_selects_driver_interface_and_exact_source_endpoint_family() {
    assert_ne!(Product::Pro.interface_guid(), Product::Chroma.interface_guid());
    assert_eq!(Product::from_id(1446).unwrap().endpoint_name(), "Razer Seiren V3 Chroma 32-Bit DSP");
    assert!(Product::from_id(1342).is_err());
}

#[test]
fn monitor_requests_keep_usb_unit_selector_channel_payload_and_timeout_distinct() {
    let enabled = AudioControlWrite::monitor_enabled(true);
    assert_eq!(enabled.header, [1,0,0,0, 1,0,0,0, 1,0,0,0, 1,0,0,0, 0,1,0,10, 100,0,0,0]);
    assert_eq!(enabled.payload, [0]);
    assert_eq!(AudioControlWrite::monitor_enabled(false).payload, [1]);
    let minimum = AudioControlWrite::monitor_level(0).unwrap();
    let maximum = AudioControlWrite::monitor_level(100).unwrap();
    for (index, (low, high)) in minimum.iter().zip(&maximum).enumerate() {
        assert_eq!(low.header[16..20], [index as u8 + 1, 2, 0, 10]);
        assert_eq!(low.payload, [0, 10]);
        assert_eq!(high.payload, [0, 31]);
    }
    assert!(AudioControlWrite::monitor_level(101).is_err());
}

#[test]
fn meter_observations_keep_wire_direction_and_signed_db_without_missing_value_fallback() {
    let channels = [Channel { kind: ChannelKind::DeviceInput, index: 0 }, Channel { kind: ChannelKind::DeviceOutput, index: 1 }];
    let request = vec![77,0,0,0,45,1,0,0, 0,0,0,0,1,0,0,0, 1,0,0,0,1,1,0,0];
    let mut response = request.clone();
    response[14..16].copy_from_slice(&(-3000i16).to_le_bytes());
    response[22..24].copy_from_slice(&(-1i16).to_le_bytes());
    let readings = meters(&script(GET_DSP, request.clone(), 24, response.clone()), &channels).unwrap();
    assert_eq!(readings[0].percent(), 50);
    assert_eq!(readings[1].percent(), 99);
    response[20] = 2;
    assert!(meters(&script(GET_DSP, request.clone(), 24, response), &channels).is_err());
    assert!(meters(&script(GET_DSP, request, 24, vec![0; 23]), &channels).is_err());
}
