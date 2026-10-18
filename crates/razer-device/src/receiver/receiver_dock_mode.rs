//! Current 164/241 factory mode probe: primary linker getDockExtendedFirmwareVersion.
//! SUCCESS means dongle; failure leaves the source factory's false default.
use crate::{backend::FeatureTransport, device_reads::DeviceReadCapability, feature_exchange};
use anyhow::{Context as _, ensure};

pub fn capability(product_id: u16) -> anyhow::Result<DeviceReadCapability> {
    ensure!(
        matches!(product_id, 164 | 241),
        "Product has no captured dock mode probe"
    );
    let source = crate::receiver_capabilities::capability(product_id)
        .context("Dock mode primary linker transport is absent")?;
    Ok(DeviceReadCapability {
        product_id: u32::from(product_id),
        source_class: source.source_class.clone(),
        direct_pids: vec![u32::from(product_id)],
        vendor_id: source.vendor_id,
        claim_interface: source.claim_interface,
        claim_interfaces_by_pid: Default::default(),
        physical_transports_by_pid: Default::default(),
        report_id: source.report_id,
        report_bytes: source.report_bytes,
        transaction_prefix: source.transaction_prefix,
        transaction_modulus: source.transaction_modulus,
        queries: Vec::new(),
        polling_codes: Default::default(),
        polling_value_offset: None,
        polling_default_hz: None,
        polling_physical_product_id: None,
        charging_codes: Default::default(),
        max_retry_in: source.max_retry_in,
        max_retry_out: source.max_retry_out,
        sleep_between_out_ms: source.sleep_between_out_ms,
        sleep_between_out_in_ms: source.sleep_between_out_in_ms,
        sleep_between_in_ms: source.sleep_between_in_ms,
    })
}

/// Preserve the complete returned data separately from the boolean factory mode.
/// The factory tests successful status, without requiring version bytes to exist.
pub fn read(
    device: &dyn FeatureTransport,
    product_id: u16,
    transaction: u8,
    guard: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Vec<u8>> {
    let cap = capability(product_id)?;
    feature_exchange::exchange(device, &cap, [4, 0, 147], &[], transaction, &guard)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    struct Mock {
        sent: Mutex<Vec<Vec<u8>>>,
        get_ids: Mutex<Vec<u8>>,
    }
    impl FeatureTransport for Mock {
        fn send_feature(&self, bytes: &[u8]) -> anyhow::Result<()> {
            self.sent.lock().unwrap().push(bytes.to_vec());
            Ok(())
        }
        fn get_feature(&self, bytes: &mut [u8]) -> anyhow::Result<usize> {
            self.get_ids.lock().unwrap().push(bytes[0]);
            let sent = self.sent.lock().unwrap().last().unwrap().clone();
            bytes.fill(0);
            bytes[1] = 2;
            bytes[2] = sent[2];
            bytes[7..9].copy_from_slice(&sent[7..9]);
            Ok(91)
        }
        fn metadata(&self) -> serde_json::Value {
            serde_json::json!({"simulated":true})
        }
    }
    #[test]
    fn mode_uses_primary_linker_command_and_status_without_synthetic_version() {
        for (product_id, report_id) in [(164, 0), (241, 3)] {
            let mock = Mock {
                sent: Mutex::new(Vec::new()),
                get_ids: Mutex::new(Vec::new()),
            };
            assert!(read(&mock, product_id, 224, || Ok(())).unwrap().is_empty());
            let sent = mock.sent.lock().unwrap();
            assert_eq!(sent[0][0], report_id);
            assert_eq!(mock.get_ids.lock().unwrap().as_slice(), [report_id]);
            assert_eq!(sent[0][2], 224);
            assert_eq!(sent[0][6..9], [4, 0, 147]);
        }
        assert!(capability(756).is_err());
    }
}
