use super::super::startup::read_startup_with_wait;
use super::*;

fn device_packet() -> Vec<u8> {
    let mut packet = vec![0; 64];
    packet[..2].copy_from_slice(&[2, 2]);
    packet[2..17].copy_from_slice(&[
        0, b'A', 31, b'Z', 32, 126, 127, 255, b'1', b'2', b'3', b'4', b'5', b'6', b'7',
    ]);
    packet[26..34].copy_from_slice(&[0x80, 0x12, 0xfe, 0x92, 0xaf, 0xfd, 0x47, 0xe1]);
    packet
}

fn error_packet(flags: u8) -> Vec<u8> {
    let mut packet = vec![0; 64];
    packet[..3].copy_from_slice(&[161, 2, flags]);
    packet
}

#[test]
fn hanbo_startup_requests_use_distinct_exact_source_headers_and_full_output_buffer() {
    for (query, header, reply) in [
        (StartupQuery::DeviceInformation, [1, 1], [2, 2]),
        (StartupQuery::ErrorcodeInfo, [160, 1], [161, 2]),
    ] {
        let mut expected = [0; 66];
        expected[1..3].copy_from_slice(&header);
        assert_eq!(query.report(), expected);
        assert_eq!(query.reply_header(), reply);
    }
}

#[test]
fn hanbo_startup_device_information_preserves_raw_identity_unknown_codes_and_packed_versions() {
    let packet = device_packet();
    let information = decode_device_information(&packet).unwrap();
    assert_eq!(information.device_id, packet[2..17]);
    assert_eq!(information.device_id_ascii(), "AZ ~1234567");
    assert_eq!(information.edition, 0x1280);
    assert_eq!(information.mode, 0xfe);
    assert_eq!(
        information.firmware,
        DeviceVersion {
            major: 0x92,
            minor: 0xaf
        }
    );
    assert_eq!(information.firmware.to_string(), "146.10.15");
    assert_eq!(information.firmware_debug_flag, 0xfd);
    assert_eq!(information.hardware.to_string(), "71.14.1");
    for (low, high, edition) in [(0, 0, 0), (128, 0, 128), (0xff, 0xff, 0xffff)] {
        let mut packet = device_packet();
        packet[26..28].copy_from_slice(&[low, high]);
        assert_eq!(decode_device_information(&packet).unwrap().edition, edition);
    }
}

#[test]
fn hanbo_startup_error_bits_are_data_without_a_status_one_acknowledgement() {
    for (bit, expected) in [
        (128, [true, false, false, false, false, false, false, false]),
        (64, [false, true, false, false, false, false, false, false]),
        (32, [false, false, true, false, false, false, false, false]),
        (16, [false, false, false, true, false, false, false, false]),
        (8, [false, false, false, false, true, false, false, false]),
        (4, [false, false, false, false, false, true, false, false]),
        (2, [false, false, false, false, false, false, true, false]),
        (1, [false, false, false, false, false, false, false, true]),
        (0, [false; 8]),
        (255, [true; 8]),
    ] {
        let error = decode_errorcode_info(&error_packet(bit)).unwrap();
        assert_eq!(
            [
                error.pump_not_connected,
                error.temperature_error,
                error.pump_tach_frequency_error,
                error.fan_not_connected,
                error.fan_number_error,
                error.fan_tach_frequency_error,
                error.fan_led_broken,
                error.power_not_connected
            ],
            expected
        );
        assert_eq!(error.has_errors(), bit != 0);
    }
}

#[test]
fn hanbo_startup_getters_reject_truncated_overlong_and_different_responses() {
    for packet in [
        device_packet()[..63].to_vec(),
        [device_packet(), vec![0]].concat(),
        error_packet(0),
        Vec::new(),
    ] {
        assert!(decode_device_information(&packet).is_none());
    }
    for packet in [
        error_packet(0)[..63].to_vec(),
        [error_packet(0), vec![0]].concat(),
        device_packet(),
        Vec::new(),
    ] {
        assert!(decode_errorcode_info(&packet).is_none());
    }
}

#[test]
fn hanbo_startup_information_read_is_independent_of_later_errorcode_failure() {
    let transport = FakeTransport::new([Ok(device_packet())]);
    let information = read_startup_with_wait(
        &transport,
        StartupQuery::DeviceInformation,
        || Ok(()),
        |_| {},
    )
    .unwrap();
    assert!(matches!(
        information,
        StartupReading::DeviceInformation(DeviceInformation {
            edition: 0x1280,
            ..
        })
    ));
    assert_eq!(transport.writes.borrow().len(), 1);
    assert_eq!(&transport.writes.borrow()[0][..3], &[0, 1, 1]);
    assert!(
        read_startup_with_wait(&transport, StartupQuery::ErrorcodeInfo, || Ok(()), |_| {}).is_err()
    );
    assert_eq!(information.query(), StartupQuery::DeviceInformation);
}

#[test]
fn hanbo_startup_errorcode_read_discards_a_stale_information_response() {
    let transport = FakeTransport::new([Ok(device_packet()), Ok(error_packet(0xa5))]);
    let reading =
        read_startup_with_wait(&transport, StartupQuery::ErrorcodeInfo, || Ok(()), |_| {}).unwrap();
    let StartupReading::ErrorcodeInfo(errors) = reading else {
        panic!("getter response changed");
    };
    assert!(
        errors.pump_not_connected
            && errors.pump_tach_frequency_error
            && errors.fan_tach_frequency_error
            && errors.power_not_connected
    );
    assert!(
        !errors.temperature_error
            && !errors.fan_not_connected
            && !errors.fan_number_error
            && !errors.fan_led_broken
    );
    assert_eq!(transport.writes.borrow().len(), 1);
    assert_eq!(transport.reads.get(), 2);
}

#[test]
fn hanbo_startup_cancellation_rejects_an_already_returned_information_packet() {
    let transport = FakeTransport::new([Ok(device_packet())]);
    let result = read_startup_with_wait(
        &transport,
        StartupQuery::DeviceInformation,
        || {
            ensure!(transport.reads.get() == 0, "startup owner changed");
            Ok(())
        },
        |_| {},
    );
    assert_eq!(
        result.unwrap_err().root_cause().to_string(),
        "startup owner changed"
    );
    assert_eq!(transport.writes.borrow().len(), 1);
    assert_eq!(transport.reads.get(), 1);
}
