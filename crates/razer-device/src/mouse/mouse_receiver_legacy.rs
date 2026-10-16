//! Current mounted PID158/192/194 single receiver LED. Commands are resolved
//! from each original wrapper's actual imports by the unified legacy query.
use crate::{
    backend::{FeatureTransport, HidNode},
    device_reads::{DeviceReadCapability, DeviceReadTarget},
    feature_exchange,
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

pub const GET: [u8; 3] = [1, 7, 144];
pub const SET: [u8; 3] = [1, 7, 16];
pub const SERIAL: [u8; 3] = [22, 0, 130];
pub const BINDINGS: [u8; 3] = [80, 0, 191];

/// he() filters only the sentinel and DeviceInfo.dongleId. In Te's current
/// V=false branch, even a zero-ID or unrelated remaining binding rejects
/// recovery; source does not restrict this decision to the requested peer.
pub fn has_source_bindings(rows: &[(u16, u8)]) -> bool {
    rows.iter().any(|(pid, _)| *pid != u16::MAX && *pid != 193)
}

/// TopView's he() reads the CURRENT runtime device, including the wired mouse.
/// Its transport is independent of the receiver-only single LED feature.
pub fn binding_capability(
    product_id: u32,
    physical_product_id: u32,
) -> Option<&'static DeviceReadCapability> {
    static DATA: OnceLock<Vec<DeviceReadCapability>> = OnceLock::new();
    DATA.get_or_init(|| make_mouse_binding_query_capabilities_table())
        .iter()
        .find(|cap| cap.product_id == product_id && cap.direct_pids.contains(&physical_product_id))
}
pub fn change_capability(
    product_id: u32,
    physical_product_id: u32,
    change: &Change,
) -> Option<&'static DeviceReadCapability> {
    if matches!(change, Change::ReadBindings) {
        binding_capability(product_id, physical_product_id)
    } else {
        capability(product_id)
    }
}
pub fn accepts_change(product_id: u32, physical: u32, peer: Option<u32>, change: &Change) -> bool {
    peer.is_none()
        && change_capability(product_id, physical, change)
            .is_some_and(|cap| cap.direct_pids.contains(&physical))
}

#[derive(Deserialize)]
struct SourceCapability {
    #[serde(flatten)]
    transport: DeviceReadCapability,
    initial_indicator_getter_enabled: bool,
}
fn source_capability(product_id: u32) -> Option<&'static SourceCapability> {
    static DATA: OnceLock<Vec<SourceCapability>> = OnceLock::new();
    DATA.get_or_init(|| make_mouse_receiver_legacy_capabilities_table())
        .iter()
        .find(|cap| cap.transport.product_id == product_id)
}
pub fn capability(product_id: u32) -> Option<&'static DeviceReadCapability> {
    source_capability(product_id).map(|cap| &cap.transport)
}
pub fn initial_indicator_getter_enabled(product_id: u32) -> bool {
    source_capability(product_id).is_some_and(|cap| cap.initial_indicator_getter_enabled)
}
pub fn accepts(product_id: u32, physical: u32, peer: Option<u32>) -> bool {
    peer.is_none() && capability(product_id).is_some_and(|cap| cap.direct_pids.contains(&physical))
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Target {
    Collection { node: HidNode, product_id: u32 },
    WindowsContainer(DeviceReadTarget),
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Change {
    Read,
    Set(u8),
    ReadSerial,
    ReadBindings,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Report {
    pub queried_status: Option<u8>,
    pub requested_status: Option<u8>,
    pub acknowledgement: Option<Vec<u8>>,
    pub serial_bytes: Option<Vec<u8>>,
    pub bindings: Option<Vec<(u16, u8)>>,
}
impl Report {
    pub fn serial_number(&self) -> anyhow::Result<String> {
        let bytes = self
            .serial_bytes
            .as_deref()
            .context("Receiver serial query is absent")?;
        // The current parser filters every response byte to ASCII32..126;
        // embedded NUL/control bytes are skipped, not string terminators.
        // Runtime's se() gives a successful empty query its source sentinel.
        let serial: String = bytes
            .iter()
            .copied()
            .filter(|byte| (32..127).contains(byte))
            .map(char::from)
            .collect();
        Ok(if serial.is_empty() {
            "NOSERIALNUMBER".into()
        } else {
            serial
        })
    }
}
/// One source command per call. The middleware owns GET -> decision -> SET;
/// the UI publication between them cannot be replaced with an ACK callback.
pub fn execute(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    change: &Change,
    mut next: impl FnMut() -> anyhow::Result<u8>,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Report> {
    let source = if matches!(change, Change::ReadBindings) {
        cap.direct_pids
            .first()
            .and_then(|physical| binding_capability(cap.product_id, *physical))
    } else {
        capability(cap.product_id)
    }
    .context("Current mouse receiver command source is absent")?;
    ensure!(
        cap.direct_pids == source.direct_pids
            && cap.vendor_id == source.vendor_id
            && cap.report_id == 0
            && cap.report_bytes == 91,
        "Single receiver LED transport identity differs"
    );
    let mut report = Report::default();
    match change {
        Change::Read => {
            let data = feature_exchange::exchange(device, cap, GET, &[0], next()?, validate)?;
            report.queried_status = data.first().copied();
        }
        Change::Set(status) => {
            // The actual protocol wrapper writes Uint8Array[0] without an
            // enum guard. Truthy unknown values from initialization survive.
            report.requested_status = Some(*status);
            report.acknowledgement = Some(feature_exchange::exchange(
                device,
                cap,
                SET,
                &[*status],
                next()?,
                validate,
            )?);
        }
        Change::ReadSerial => {
            report.serial_bytes = Some(feature_exchange::exchange(
                device,
                cap,
                SERIAL,
                &[],
                next()?,
                validate,
            )?);
        }
        Change::ReadBindings => {
            let data = feature_exchange::exchange(device, cap, BINDINGS, &[], next()?, validate)?;
            let count = usize::from(data.first().copied().unwrap_or(0));
            ensure!(
                count == 0 || data.len() >= 1 + count * 3,
                "Binding getter has a truncated status/PID record"
            );
            // Current84816.J preserves zero IDs, duplicates and every status;
            // the UI caller filters only 65535 and its own dongle ID later.
            report.bindings = Some(
                data.get(1..1 + count * 3)
                    .unwrap_or_default()
                    .chunks_exact(3)
                    .map(|row| (u16::from_be_bytes([row[1], row[2]]), row[0]))
                    .collect(),
            );
        }
    }
    validate()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyboard_control_test_transport::{Mock, assert_report_integrity, reply, sequence};
    fn cap(product: u32) -> DeviceReadCapability {
        let mut cap = capability(product).unwrap().clone();
        cap.sleep_between_out_ms = 0;
        cap.sleep_between_out_in_ms = 0;
        cap.sleep_between_in_ms = 0;
        cap.max_retry_out = 1;
        cap.max_retry_in = 1;
        cap
    }
    fn binding_test_cap(physical: u32) -> DeviceReadCapability {
        let mut cap = binding_capability(192, physical).unwrap().clone();
        cap.sleep_between_out_ms = 0;
        cap.sleep_between_out_in_ms = 0;
        cap.sleep_between_in_ms = 0;
        cap.max_retry_out = 1;
        cap.max_retry_in = 1;
        cap
    }
    #[test]
    fn receiver_legacy_binding_getter_uses_current_wired_and_dongle_factories() {
        for (physical, milliseconds) in [(192, 5), (193, 25)] {
            let source = binding_capability(192, physical).unwrap();
            assert_eq!(source.direct_pids, vec![physical]);
            assert_eq!(source.sleep_between_out_ms, milliseconds);
            assert_eq!(source.sleep_between_out_in_ms, milliseconds);
            assert_eq!(source.sleep_between_in_ms, milliseconds);
            assert!(accepts_change(192, physical, None, &Change::ReadBindings));
            assert!(!accepts_change(
                192,
                physical,
                Some(192),
                &Change::ReadBindings
            ));
            let mock = Mock::new([reply(&[4, 0, 0, 0, 7, 0, 183, 1, 0, 183, 255, 255, 255])]);
            let report = execute(
                &mock,
                &binding_test_cap(physical),
                &Change::ReadBindings,
                sequence(),
                &|| Ok(()),
            )
            .unwrap();
            assert_eq!(
                report.bindings,
                Some(vec![(0, 0), (183, 7), (183, 1), (65535, 255)])
            );
            let sent = mock.sent();
            assert_eq!(sent.len(), 1);
            assert_eq!(&sent[0][6..9], &[80, 0, 191]);
            assert!(sent[0][9..89].iter().all(|byte| *byte == 0));
            assert_report_integrity(&sent);
        }
        assert!(binding_capability(192, 194).is_none());
        assert!(!accepts(192, 192, None));
    }
    #[test]
    fn receiver_legacy_binding_getter_distinguishes_empty_reply_from_read_failure() {
        for payload in [&[][..], &[0][..]] {
            let mock = Mock::new([reply(payload)]);
            assert_eq!(
                execute(
                    &mock,
                    &binding_test_cap(192),
                    &Change::ReadBindings,
                    sequence(),
                    &|| Ok(())
                )
                .unwrap()
                .bindings,
                Some(vec![])
            );
        }
        let missing = Mock::new([]);
        assert!(
            execute(
                &missing,
                &binding_test_cap(192),
                &Change::ReadBindings,
                sequence(),
                &|| Ok(())
            )
            .is_err()
        );
        let truncated = Mock::new([reply(&[1, 1, 0])]);
        assert!(
            execute(
                &truncated,
                &binding_test_cap(193),
                &Change::ReadBindings,
                sequence(),
                &|| Ok(())
            )
            .is_err()
        );
        let canceled = Mock::new([]);
        assert!(
            execute(
                &canceled,
                &binding_test_cap(192),
                &Change::ReadBindings,
                sequence(),
                &|| anyhow::bail!("canceled")
            )
            .is_err()
        );
        assert!(canceled.sent().is_empty());
    }
    #[test]
    fn receiver_legacy_unpair_recovery_keeps_zero_duplicates_and_unrelated_bindings() {
        assert!(!has_source_bindings(&[]));
        assert!(!has_source_bindings(&[(193, 1), (65535, 0)]));
        for rows in [
            vec![(0, 0)],
            vec![(183, 0)],
            vec![(193, 1), (183, 7), (183, 0)],
        ] {
            assert!(has_source_bindings(&rows));
        }
    }
    #[test]
    fn receiver_legacy_reads_do_not_send_a_default_or_implicit_setter() {
        for data in [&[][..], &[0][..], &[255][..]] {
            let mock = Mock::new([reply(data)]);
            let report = execute(&mock, &cap(158), &Change::Read, sequence(), &|| Ok(())).unwrap();
            assert_eq!(report.queried_status, data.first().copied());
            assert!(report.requested_status.is_none());
            assert_eq!(mock.sent().len(), 1);
            assert_eq!(&mock.sent()[0][6..10], &[1, 7, 144, 0]);
        }
    }
    #[test]
    fn receiver_legacy_setter_preserves_runtime_status_and_ack_does_not_read_back() {
        let mock = Mock::new([reply(&[])]);
        let report = execute(&mock, &cap(192), &Change::Set(255), sequence(), &|| Ok(())).unwrap();
        assert_eq!(report.requested_status, Some(255));
        assert!(report.queried_status.is_none());
        let sent = mock.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(&sent[0][6..10], &[1, 7, 16, 255]);
        assert_report_integrity(&sent);
    }
    #[test]
    fn receiver_legacy_cancellation_and_physical_identity_are_explicit() {
        let mock = Mock::new([]);
        assert!(
            execute(
                &mock,
                &cap(194),
                &Change::Set(1),
                sequence(),
                &|| anyhow::bail!("canceled")
            )
            .is_err()
        );
        assert!(mock.sent().is_empty());
        for (product, receiver) in [(158, 159), (192, 193), (194, 195)] {
            assert!(accepts(product, receiver, None));
            assert!(!accepts(product, product, None));
            assert!(!accepts(product, receiver, Some(product)));
            assert!(!accepts(product, 179, None));
        }
        assert!(capability(211).is_none());
        assert!(capability(222).is_none());
    }
    #[test]
    fn receiver_legacy_serial_filters_all_bytes_and_requires_an_actual_reply() {
        let mock = Mock::new([reply(&[b'A', 0, b'B', 31, 32, 126, 127, 255, b'C'])]);
        let report = execute(
            &mock,
            &cap(158),
            &Change::ReadSerial,
            sequence(),
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(report.serial_number().unwrap(), "AB ~C");
        let sent = mock.sent();
        assert_eq!(&sent[0][6..9], &SERIAL);
        assert_eq!(sent[0][9], 0);
        assert_report_integrity(&sent);
        assert_eq!(
            Report {
                serial_bytes: Some(Vec::new()),
                ..Default::default()
            }
            .serial_number()
            .unwrap(),
            "NOSERIALNUMBER"
        );
        assert!(Report::default().serial_number().is_err());
        let failed = Mock::new([]);
        assert!(
            execute(&failed, &cap(158), &Change::ReadSerial, sequence(), &|| Ok(
                ()
            ))
            .is_err()
        );
    }
}

// Generated typed table: razer-device/mouse_binding_query_capabilities.
#[rustfmt::skip]
fn make_mouse_binding_query_capabilities_table() -> Vec<DeviceReadCapability> {
    vec![
        DeviceReadCapability{
            product_id: 192u32,
            source_class: "rzDevice25".into(),
            direct_pids: vec![192u32],
            vendor_id: 5426u16,
            claim_interface: 0u8,
            claim_interfaces_by_pid: Default::default(),
            physical_transports_by_pid: Default::default(),
            report_id: 0u8,
            report_bytes: 91usize,
            transaction_prefix: 0u8,
            transaction_modulus: 31u8,
            queries: vec![],
            polling_codes: vec![].into_iter().collect(),
            polling_value_offset: None,
            polling_default_hz: None,
            polling_physical_product_id: None,
            charging_codes: vec![].into_iter().collect(),
            max_retry_in: 10u8,
            max_retry_out: 20u8,
            sleep_between_out_ms: 5u64,
            sleep_between_out_in_ms: 5u64,
            sleep_between_in_ms: 5u64
        },
        DeviceReadCapability{
            product_id: 192u32,
            source_class: "rzDevice25DualLinkMouse".into(),
            direct_pids: vec![193u32],
            vendor_id: 5426u16,
            claim_interface: 0u8,
            claim_interfaces_by_pid: Default::default(),
            physical_transports_by_pid: Default::default(),
            report_id: 0u8,
            report_bytes: 91usize,
            transaction_prefix: 0u8,
            transaction_modulus: 31u8,
            queries: vec![],
            polling_codes: vec![].into_iter().collect(),
            polling_value_offset: None,
            polling_default_hz: None,
            polling_physical_product_id: None,
            charging_codes: vec![].into_iter().collect(),
            max_retry_in: 10u8,
            max_retry_out: 20u8,
            sleep_between_out_ms: 25u64,
            sleep_between_out_in_ms: 25u64,
            sleep_between_in_ms: 25u64
        }
    ]
}

// Generated typed table: razer-device/mouse_receiver_legacy_capabilities.
#[rustfmt::skip]
fn make_mouse_receiver_legacy_capabilities_table() -> Vec<SourceCapability> {
    vec![
        SourceCapability{
            transport: DeviceReadCapability{
                product_id: 158u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![159u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: Default::default(),
                physical_transports_by_pid: Default::default(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            },
            initial_indicator_getter_enabled: true
        },
        SourceCapability{
            transport: DeviceReadCapability{
                product_id: 192u32,
                source_class: "rzDevice25DualLinkMouse".into(),
                direct_pids: vec![193u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: Default::default(),
                physical_transports_by_pid: Default::default(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 25u64,
                sleep_between_out_in_ms: 25u64,
                sleep_between_in_ms: 25u64
            },
            initial_indicator_getter_enabled: true
        },
        SourceCapability{
            transport: DeviceReadCapability{
                product_id: 194u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![195u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: Default::default(),
                physical_transports_by_pid: Default::default(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            },
            initial_indicator_getter_enabled: false
        }
    ]
}
