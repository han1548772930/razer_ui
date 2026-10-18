use super::*;
use std::collections::VecDeque;

struct Reply {
    code: u32,
    input: Vec<u8>,
    output: Vec<u8>,
    returned: usize,
}

struct Script(VecDeque<Reply>);

impl Transport for Script {
    fn ioctl(&mut self, code: u32, input: &[u8], output: &mut [u8]) -> anyhow::Result<usize> {
        let reply = self.0.pop_front().expect("unexpected driver operation");
        assert_eq!(code, reply.code);
        assert_eq!(input, reply.input);
        assert_eq!(output.len(), reply.output.len());
        output.copy_from_slice(&reply.output);
        Ok(reply.returned)
    }
}

fn driver(code: u32, input: &[u8], output: &[u8], returned: usize) -> Driver<Script> {
    Driver::new(Script(VecDeque::from([Reply {
        code,
        input: input.into(),
        output: output.into(),
        returned,
    }])))
}

#[test]
fn msr_wire_low_and_high_words_are_not_out_parameter_order() {
    let mut driver = driver(
        0x9c40_2440,
        &[0x9c, 1, 0, 0],
        &[0x78, 0x56, 0x34, 0x12, 0xef, 0xcd, 0xab, 0x90],
        8,
    );
    assert_eq!(driver.read_msr(0x19c).unwrap(), 0x90ab_cdef_1234_5678);
    assert!(driver.into_transport().0.is_empty());
}

#[test]
fn smn_uses_exact_pci_method_and_register_order() {
    let request = [0, 0xc3, 0x24, 0x80, 2, 0, 0, 0, 0, 0x98, 5, 0];
    assert_eq!(driver(0x9c40_2584, &request, &[0x56, 0x34, 0x12, 0x80], 4)
        .read_smn(0x24, 24, 3, 2, 0x59800).unwrap(), 0x8012_3456);
    for value in [0xaaaa_aaaa_u32, 0xbbbb_bbbb, 0xcccc_cccc] {
        assert!(driver(0x9c40_2584, &request, &value.to_le_bytes(), 4)
            .read_smn(0x24, 24, 3, 2, 0x59800).is_err());
    }
    for length in [0, 3, 5] {
        assert!(driver(0x9c40_2584, &request, &[0; 4], length)
            .read_smn(0x24, 24, 3, 2, 0x59800).is_err());
    }
}

#[test]
fn pci_read_requires_driver_count_and_preserves_extended_offset() {
    let request = [0, 0, 0, 0, 24, 0, 0, 0, 0, 0, 0, 0, 4, 12, 0, 0];
    assert_eq!(driver(0x9c40_2720, &request, &[4, 0, 0, 0, 0x80, 0, 0, 0], 8)
        .read_pci_u32(0, 24, 0, 0xc04).unwrap(), 0x80);
    assert!(driver(0x9c40_2720, &request, &[0; 8], 8)
        .read_pci_u32(0, 24, 0, 0xc04).is_err());
}

#[test]
fn missing_msr_is_not_a_valid_observation_but_zero_is() {
    assert!(
        driver(0x9c40_2440, &[0x9c, 1, 0, 0], &[255; 8], 8)
            .read_msr(0x19c)
            .is_err()
    );
    assert_eq!(
        driver(0x9c40_2440, &[0x9c, 1, 0, 0], &[0; 8], 8)
            .read_msr(0x19c)
            .unwrap(),
        0
    );
}

#[test]
fn short_or_overreported_reply_cannot_become_zero_or_partial_measurement() {
    for returned in [0, 4, 7, 9] {
        assert!(
            driver(0x9c40_2440, &[0x9c, 1, 0, 0], &[0; 8], returned)
                .read_msr(0x19c)
                .is_err()
        );
    }
}

#[test]
fn measured_tsc_uses_high_word_first_and_source_integer_megahertz_conversion() {
    // 5,500,999,999 Hz; the source divides the signed counter result before
    // converting to float. Using little-endian u64 or rounding is incorrect.
    let mut driver = driver(
        0x9c40_2608,
        &[0x10, 0, 0, 0],
        &[1, 0, 0, 0, 0x3f, 0x99, 0xe2, 0x47],
        8,
    );
    assert_eq!(driver.measured_tsc_mhz().unwrap(), 5500.0);
}

#[test]
fn counter_failures_cannot_be_reported_as_a_frequency() {
    for output in [[0; 8], [255; 8]] {
        assert!(
            driver(0x9c40_2608, &[0x10, 0, 0, 0], &output, 8)
                .measured_tsc_mhz()
                .is_err()
        );
    }
}

#[test]
fn port_reads_keep_valid_all_ones_and_consume_only_the_requested_width() {
    assert_eq!(
        driver(0x9c40_2480, &[0x95, 2, 0, 0], &[255; 8], 8)
            .read_port_u8(0x295)
            .unwrap(),
        255
    );
    assert_eq!(
        driver(
            0x9c40_2484,
            &[0x95, 2, 0, 0],
            &[0x34, 0x12, 9, 8, 7, 6, 5, 4],
            8
        )
        .read_port_u16(0x295)
        .unwrap(),
        0x1234
    );
    assert_eq!(
        driver(
            0x9c40_2488,
            &[0x95, 2, 0, 0],
            &[0x78, 0x56, 0x34, 0x12, 7, 6, 5, 4],
            8
        )
        .read_port_u32(0x295)
        .unwrap(),
        0x12345678
    );
}

#[test]
fn driver_version_is_an_observation_from_the_protocol() {
    assert_eq!(
        driver(0x9c40_2400, &[], &[0x65, 1, 0, 0], 4)
            .protocol_version()
            .unwrap(),
        0x165
    );
    assert!(
        driver(0x9c40_2400, &[], &[255; 4], 4)
            .protocol_version()
            .is_err()
    );
}
