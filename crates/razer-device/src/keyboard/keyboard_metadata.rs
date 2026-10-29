use crate::{backend::FeatureTransport, device_reads::DeviceReadCapability};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::OnceLock, time::Duration};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct KeyboardMetadataReading {
    pub keyboard_layout: u8,
    pub edition_id: u8,
    pub firmware_id: Option<u8>,
}

struct KeyboardMetadataDescriptor {
    product_id: u32,
    physical_product_id: u32,
    wireless: bool,
    source_class: &'static str,
    claim_interface: u8,
    transaction_prefix: u8,
    max_retry_in: u8,
    max_retry_out: u8,
    sleep_between_out_ms: u64,
    sleep_between_out_in_ms: u64,
    sleep_between_in_ms: u64,
}

include!("keyboard_metadata_data_generated.rs");

fn catalog() -> &'static [DeviceReadCapability] {
    static CATALOG: OnceLock<Vec<DeviceReadCapability>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        AUDITED_KEYBOARDS
            .iter()
            .map(|descriptor| DeviceReadCapability {
                product_id: descriptor.product_id,
                source_class: descriptor.source_class.into(),
                direct_pids: vec![descriptor.physical_product_id],
                vendor_id: 0x1532,
                claim_interface: descriptor.claim_interface,
                // Each metadata descriptor is already one physical factory
                // branch, including radio descriptors; it is not a PID map.
                claim_interfaces_by_pid: BTreeMap::new(),
                physical_transports_by_pid: Default::default(),
                // Receiver bindings independently retain the selected sender's
                // report prefix. Wired base/subclass factories explicitly use 0.
                report_id: u16::try_from(descriptor.physical_product_id)
                    .ok()
                    .and_then(crate::receiver_capabilities::capability)
                    .filter(|receiver| {
                        u32::from(receiver.source_product_id) == descriptor.product_id
                            && receiver.source_class == descriptor.source_class
                            && receiver.claim_interface == descriptor.claim_interface
                    })
                    .map_or(0, |receiver| receiver.report_id),
                report_bytes: 91,
                transaction_prefix: descriptor.transaction_prefix,
                transaction_modulus: 31,
                queries: Vec::new(),
                polling_codes: BTreeMap::new(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: BTreeMap::new(),
                max_retry_in: descriptor.max_retry_in,
                max_retry_out: descriptor.max_retry_out,
                sleep_between_out_ms: descriptor.sleep_between_out_ms,
                sleep_between_out_in_ms: descriptor.sleep_between_out_in_ms,
                sleep_between_in_ms: descriptor.sleep_between_in_ms,
            })
            .collect()
    })
}

pub fn capability(product_id: u32) -> Option<&'static DeviceReadCapability> {
    AUDITED_KEYBOARDS
        .iter()
        .position(|descriptor| descriptor.product_id == product_id && !descriptor.wireless)
        .map(|index| &catalog()[index])
}

pub fn radio_capability(product_id: u32) -> Option<&'static DeviceReadCapability> {
    AUDITED_KEYBOARDS
        .iter()
        .position(|descriptor| descriptor.product_id == product_id && descriptor.wireless)
        .map(|index| &catalog()[index])
}

pub fn hid_capability(
    product_id: u32,
    physical_product_id: u32,
) -> Option<&'static DeviceReadCapability> {
    catalog()
        .iter()
        .find(|cap| cap.product_id == product_id && cap.direct_pids.contains(&physical_product_id))
}

pub fn ble_product_id(product_id: u32) -> Option<u32> {
    AUDITED_BLE_KEYBOARDS
        .iter()
        .find(|row| row.0 == product_id)
        .map(|row| row.1)
}

pub fn relay_capability(
    product_id: u32,
    physical_product_id: u32,
) -> anyhow::Result<DeviceReadCapability> {
    let source =
        radio_capability(product_id).context("Keyboard source has no radio metadata factory")?;
    ensure!(
        source.source_class == "rzDevice25DualLinkKeyboard",
        "Keyboard source does not have the cached-master factory"
    );
    let receiver = u16::try_from(physical_product_id)
        .ok()
        .and_then(crate::receiver_capabilities::capability)
        .context("Cached master receiver has no verified query interface")?;
    let mut cap = source.clone();
    cap.direct_pids = vec![physical_product_id];
    cap.claim_interface = receiver.claim_interface;
    cap.report_id = receiver.claim_interface;
    cap.claim_interfaces_by_pid.clear();
    Ok(cap)
}

pub fn validate_peer(
    product_id: u32,
    peer_product_id: u32,
    peers: &[(u16, u8)],
) -> anyhow::Result<()> {
    let radio =
        radio_capability(product_id).context("Keyboard peer has no current radio source")?;
    ensure!(
        (peer_product_id == product_id || radio.direct_pids.contains(&peer_product_id))
            && peers
                .iter()
                .any(|(peer, status)| u32::from(*peer) == peer_product_id && *status == 1),
        "Receiver did not confirm the requested keyboard peer online"
    );
    Ok(())
}

fn decode(data: &[u8]) -> anyhow::Result<KeyboardMetadataReading> {
    ensure!(
        data.len() >= 2,
        "Keyboard edition response is missing layout or edition"
    );
    Ok(KeyboardMetadataReading {
        keyboard_layout: data[0],
        edition_id: data[1],
        firmware_id: data.get(2).copied(),
    })
}

fn decode_serial(data: &[u8]) -> anyhow::Result<String> {
    ensure!(
        !data.is_empty() && data.len() <= 22,
        "Keyboard serial response has invalid data length"
    );
    Ok(data
        .iter()
        .copied()
        .filter(|byte| (32..127).contains(byte))
        .map(char::from)
        .collect())
}

pub fn read_serial(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    next_transaction: impl FnOnce() -> anyhow::Result<u8>,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<String> {
    ensure!(
        capability(cap.product_id).is_some() || radio_capability(cap.product_id).is_some(),
        "Keyboard serial product is not source-audited"
    );
    let data = crate::feature_exchange::exchange(
        device,
        cap,
        [22, 0, 130],
        &[],
        next_transaction()?,
        validate,
    )?;
    validate()?;
    decode_serial(&data)
}

pub trait BleTransport {
    fn exchange(
        &mut self,
        command: [u8; 3],
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Vec<u8>>;
}

pub fn ble_frame(transaction: u8, command: [u8; 3]) -> anyhow::Result<Vec<u8>> {
    ensure!(
        transaction < 31 && matches!(command, [0, 1, 134] | [0, 1, 131]),
        "Invalid keyboard BLE metadata request"
    );
    Ok(vec![transaction, 0, 0, 0, command[1], command[2], 0, 0])
}

pub fn read_ble(
    product_id: u32,
    transport: &mut dyn BleTransport,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<KeyboardMetadataReading> {
    ensure!(
        ble_product_id(product_id).is_some(),
        "Keyboard BLE metadata product is not source-audited"
    );
    let mut last_error = None;
    for attempt in 0..3 {
        validate()?;
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(100));
            validate()?;
        }
        match transport.exchange([0, 1, 134], validate) {
            Ok(data) => {
                validate()?;
                return decode(&data);
            }
            Err(error) => {
                validate()?;
                last_error = Some(error);
            }
        }
    }
    Err(last_error.context("Keyboard BLE edition command did not run")?)
        .context("Keyboard BLE getEdition retries exhausted")
}

pub fn read_ble_serial(
    product_id: u32,
    transport: &mut dyn BleTransport,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<String> {
    ensure!(
        ble_product_id(product_id).is_some(),
        "Keyboard BLE serial product is not source-audited"
    );
    validate()?;
    let data = transport.exchange([0, 1, 131], validate)?;
    validate()?;
    decode_serial(&data)
}

pub fn read(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    mut next_transaction: impl FnMut() -> anyhow::Result<u8>,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<KeyboardMetadataReading> {
    ensure!(
        capability(cap.product_id).is_some() || radio_capability(cap.product_id).is_some(),
        "Keyboard metadata product is not source-audited"
    );
    let mut last_error = None;
    for attempt in 0..3 {
        validate()?;
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(100));
            validate()?;
        }
        match crate::feature_exchange::exchange(
            device,
            cap,
            [3, 0, 134],
            &[],
            next_transaction()?,
            validate,
        ) {
            Ok(data) => {
                validate()?;
                return decode(&data);
            }
            Err(error) => {
                validate()?;
                last_error = Some(error);
            }
        }
    }
    Err(last_error.context("Keyboard metadata command did not run")?)
        .context("Keyboard getEdition retries exhausted")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyboard_control_test_transport::{
        Mock, assert_report_integrity, assert_report_integrity_with_id, reply, sequence,
    };

    fn cap() -> DeviceReadCapability {
        let mut cap = capability(529).unwrap().clone();
        cap.max_retry_in = 1;
        cap.max_retry_out = 1;
        cap.sleep_between_out_ms = 0;
        cap.sleep_between_out_in_ms = 0;
        cap.sleep_between_in_ms = 0;
        cap
    }

    #[test]
    fn reads_actual_layout_edition_and_optional_firmware() {
        for data in [vec![8, 132, 7], vec![1, 0]] {
            let device = Mock::new([reply(&data)]);
            let observed = read(&device, &cap(), sequence(), &|| Ok(())).unwrap();
            assert_eq!(observed.keyboard_layout, data[0]);
            assert_eq!(observed.edition_id, data[1]);
            assert_eq!(observed.firmware_id, data.get(2).copied());
            let sent = device.sent();
            assert_eq!(&sent[0][6..9], &[3, 0, 134]);
            assert!(sent[0][9..89].iter().all(|byte| *byte == 0));
            assert_report_integrity(&sent);
        }
    }

    #[test]
    fn outer_retries_allocate_new_transactions_and_stop_at_three() {
        let device = Mock::new([reply(&[8, 0, 3])]).fail_sends(2);
        assert!(read(&device, &cap(), sequence(), &|| Ok(())).is_ok());
        assert_eq!(
            device
                .sent()
                .iter()
                .map(|packet| packet[2])
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        let device = Mock::new([]).fail_sends(10);
        assert!(read(&device, &cap(), sequence(), &|| Ok(())).is_err());
        assert_eq!(device.sent().len(), 3);
    }

    #[test]
    fn missing_data_never_becomes_a_default_layout() {
        for data in [vec![], vec![8]] {
            let device = Mock::new([reply(&data)]);
            assert!(read(&device, &cap(), sequence(), &|| Ok(())).is_err());
            assert_eq!(device.sent().len(), 1);
        }
    }

    #[test]
    fn cancellation_prevents_send_and_does_not_retry() {
        let device = Mock::new([]);
        let error = read(&device, &cap(), sequence(), &|| anyhow::bail!("canceled"));
        assert!(error.is_err());
        assert!(device.sent().is_empty());
        let device = Mock::new([reply(&[8, 1, 0])]);
        let guard = || {
            ensure!(device.sent().is_empty(), "canceled after send");
            Ok(())
        };
        assert!(read(&device, &cap(), sequence(), &guard).is_err());
        assert_eq!(device.sent().len(), 1);
    }

    #[test]
    fn malformed_transport_response_is_not_observed() {
        let mut malformed = reply(&[8, 1, 0]);
        malformed.count = 10;
        let device = Mock::new([malformed.clone(), malformed.clone(), malformed]);
        assert!(read(&device, &cap(), sequence(), &|| Ok(())).is_err());
        assert_eq!(device.sent().len(), 3);
    }

    #[test]
    fn descriptors_only_expose_verified_direct_interfaces() {
        for descriptor in AUDITED_KEYBOARDS {
            let cap =
                hid_capability(descriptor.product_id, descriptor.physical_product_id).unwrap();
            assert_eq!(cap.direct_pids, vec![descriptor.physical_product_id]);
            assert_eq!(
                cap.claim_interface_for(descriptor.physical_product_id)
                    .unwrap(),
                descriptor.claim_interface
            );
            assert!(cap.for_physical_product(0).is_err());
        }
        assert!(capability(0).is_none());
        assert!(capability(662).is_none());
        assert!(capability(663).is_none());
    }

    #[test]
    fn inherited_and_oled_transports_preserve_source_timing() {
        let legacy = capability(529).unwrap();
        assert_eq!(legacy.source_class, "rzDevice25LedMatrix");
        assert_eq!(legacy.claim_interface, 2);
        assert_eq!(legacy.sleep_between_out_ms, 5);
        let oled = capability(691).unwrap();
        assert_eq!(oled.source_class, "rzDevice25Oled");
        assert_eq!(oled.claim_interface, 3);
        assert_eq!(oled.sleep_between_out_ms, 25);
        assert_eq!(oled.sleep_between_out_in_ms, 25);
        assert_eq!(oled.sleep_between_in_ms, 25);
        assert_eq!(capability(708).unwrap().claim_interface, 3);
        assert_eq!(capability(660).unwrap().claim_interface, 2);
    }

    #[test]
    fn radio_classes_keep_their_source_namespaces_and_interfaces() {
        assert!(capability(585).is_none());
        assert_eq!(radio_capability(585).unwrap().transaction_prefix, 0);
        assert_eq!(radio_capability(691).unwrap().claim_interface, 2);
        assert_eq!(radio_capability(691).unwrap().transaction_prefix, 0);
        assert_eq!(radio_capability(708).unwrap().claim_interface, 2);
        assert_eq!(radio_capability(708).unwrap().transaction_prefix, 128);
        for descriptor in AUDITED_KEYBOARDS.iter().filter(|entry| entry.wireless) {
            let mut cap = radio_capability(descriptor.product_id).unwrap().clone();
            cap.sleep_between_out_ms = 0;
            cap.sleep_between_out_in_ms = 0;
            cap.sleep_between_in_ms = 0;
            let device = Mock::new((0..32).map(|_| reply(&[8, 128, 1])));
            let mut sequence = 0;
            for _ in 0..32 {
                let next = || {
                    let transaction = cap.transaction_prefix | sequence;
                    sequence = (sequence + 1) % 31;
                    Ok(transaction)
                };
                read(&device, &cap, next, &|| Ok(())).unwrap();
            }
            let sent = device.sent();
            assert_eq!(sent[0][2], descriptor.transaction_prefix);
            assert_eq!(sent[15][2], descriptor.transaction_prefix | 15);
            assert_eq!(sent[30][2], descriptor.transaction_prefix | 30);
            assert_eq!(sent[31][2], descriptor.transaction_prefix);
            assert_report_integrity_with_id(&sent, cap.report_id);
        }
        assert_eq!(radio_capability(574).unwrap().transaction_prefix, 240);
        assert_eq!(radio_capability(2595).unwrap().transaction_prefix, 240);
    }

    #[test]
    fn shared_receiver_uses_the_cached_master_interface_and_keyboard_namespace() {
        let mut cap = relay_capability(708, 241).unwrap();
        assert_eq!(cap.direct_pids, vec![241]);
        assert_eq!(
            cap.claim_interface,
            crate::receiver_capabilities::capability(241)
                .unwrap()
                .claim_interface
        );
        assert_eq!(cap.claim_interface_for(241).unwrap(), cap.claim_interface);
        assert_eq!(cap.transaction_prefix, 128);
        assert_eq!(cap.sleep_between_out_ms, 25);
        cap.sleep_between_out_ms = 0;
        cap.sleep_between_out_in_ms = 0;
        cap.sleep_between_in_ms = 0;
        let device = Mock::new([reply(&[6, 128, 2]), reply(b"KEYBOARD")]);
        assert_eq!(
            read(&device, &cap, || Ok(128), &|| Ok(()))
                .unwrap()
                .keyboard_layout,
            6
        );
        assert_eq!(
            read_serial(&device, &cap, || Ok(129), &|| Ok(())).unwrap(),
            "KEYBOARD"
        );
        assert_eq!(device.sent()[0][2], 128);
        assert_eq!(device.sent()[1][2], 129);
        assert_report_integrity_with_id(&device.sent(), 3);
    }

    #[test]
    fn shared_receiver_rejects_unproven_classes_and_unknown_masters() {
        for product_id in [529, 574, 585, 691, 2595, 65535] {
            assert!(relay_capability(product_id, 241).is_err());
        }
        assert!(relay_capability(708, 65535).is_err());
        assert!(relay_capability(708, u32::MAX).is_err());
    }

    #[test]
    fn shared_receiver_requires_the_exact_live_raw_peer() {
        assert!(validate_peer(708, 708, &[(708, 1)]).is_ok());
        assert!(validate_peer(708, 706, &[(706, 1)]).is_ok());
        for (peer, rows) in [
            (706, vec![(708, 1)]),
            (708, vec![(708, 0)]),
            (708, vec![(708, 2)]),
            (707, vec![(707, 1)]),
            (65535, vec![(65535, 1)]),
            (708, vec![]),
        ] {
            assert!(validate_peer(708, peer, &rows).is_err());
        }
    }

    #[test]
    fn serial_query_uses_source_printable_ascii_filter_without_trimming() {
        let data = [0, 31, b' ', b'A', b'Z', b'0', 126, 127, 255, b' '];
        let device = Mock::new([reply(&data)]);
        let serial = read_serial(&device, &cap(), || Ok(3), &|| Ok(())).unwrap();
        assert_eq!(serial, " AZ0~ ");
        assert_eq!(&device.sent()[0][6..9], &[22, 0, 130]);
        assert_report_integrity(&device.sent());
        assert_eq!(decode_serial(&[0; 22]).unwrap(), "");
        assert!(decode_serial(&[]).is_err());
        assert!(decode_serial(&[b'A'; 23]).is_err());
    }

    struct BleMock {
        responses: std::collections::VecDeque<Result<Vec<u8>, String>>,
        commands: Vec<[u8; 3]>,
    }

    impl BleTransport for BleMock {
        fn exchange(
            &mut self,
            command: [u8; 3],
            validate: &dyn Fn() -> anyhow::Result<()>,
        ) -> anyhow::Result<Vec<u8>> {
            validate()?;
            self.commands.push(command);
            self.responses
                .pop_front()
                .unwrap()
                .map_err(anyhow::Error::msg)
        }
    }

    #[test]
    fn ble_metadata_has_its_own_commands_and_payload_framing() {
        assert_eq!(ble_product_id(708), Some(707));
        assert_eq!(ble_product_id(691), Some(693));
        assert_eq!(ble_product_id(574), None);
        assert_eq!(
            ble_frame(30, [0, 1, 134]).unwrap(),
            vec![30, 0, 0, 0, 1, 134, 0, 0]
        );
        assert!(ble_frame(31, [0, 1, 134]).is_err());
        assert!(ble_frame(0, [3, 0, 134]).is_err());
        let mut transport = BleMock {
            responses: [Ok(vec![3, 129, 9]), Ok(vec![0, b'A', b' ', b'1', 255])].into(),
            commands: vec![],
        };
        let reading = read_ble(708, &mut transport, &|| Ok(())).unwrap();
        assert_eq!(reading.keyboard_layout, 3);
        assert_eq!(reading.edition_id, 129);
        assert_eq!(reading.firmware_id, Some(9));
        assert_eq!(
            read_ble_serial(708, &mut transport, &|| Ok(())).unwrap(),
            "A 1"
        );
        assert_eq!(transport.commands, vec![[0, 1, 134], [0, 1, 131]]);
    }

    #[test]
    fn ble_metadata_retries_failures_but_not_cancellation_or_missing_values() {
        let mut transport = BleMock {
            responses: [Err("timeout".into()), Err("failed".into()), Ok(vec![8, 0])].into(),
            commands: vec![],
        };
        assert_eq!(
            read_ble(708, &mut transport, &|| Ok(()))
                .unwrap()
                .keyboard_layout,
            8
        );
        assert_eq!(transport.commands.len(), 3);
        let mut canceled = BleMock {
            responses: [].into(),
            commands: vec![],
        };
        assert!(read_ble(708, &mut canceled, &|| anyhow::bail!("canceled")).is_err());
        assert!(canceled.commands.is_empty());
        let mut missing = BleMock {
            responses: [Ok(vec![])].into(),
            commands: vec![],
        };
        assert!(read_ble(708, &mut missing, &|| Ok(())).is_err());
        assert_eq!(missing.commands.len(), 1);
    }
}
