//! Worker adapter from observed Windows identity to the portable HID backend.
//! Contains no native DLL loading or OS HID calls. Protocols live above it.
use anyhow::ensure;
use razer_device::backend::{
    FeatureTransport, checked_native_io, checked_native_read, require_complete_send,
};
use razer_hid::with_backend;
use serde_json::Value;

pub(super) struct Device {
    transport: Box<dyn FeatureTransport>,
    feature_report_bytes: usize,
}

pub(super) fn open_validated(
    path: &str,
    feature_report_bytes: usize,
    validate: razer_device::backend::ExchangeValidation<'_>,
) -> anyhow::Result<Device> {
    validate()?;
    ensure!(feature_report_bytes > 0, "缺少实际 Feature Report 长度");
    with_backend(|backend| {
        let nodes = backend.enumerate()?;
        let matches = nodes
            .iter()
            .filter(|node| {
                // SetupAPI paths are UTF-16; current callers verify ASCII before
                // reaching here. Preserve hidapi's actual path when opening it.
                node.path.eq_ignore_ascii_case(path.as_bytes())
            })
            .collect::<Vec<_>>();
        ensure!(matches.len() == 1, "观察路径不对应唯一的 HID collection");
        Ok(Device {
            transport: backend.open_validated(matches[0], validate)?,
            feature_report_bytes,
        })
    })
}

impl Device {
    pub(super) fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        let count = self.send_feature_counted(report)?;
        require_complete_send(count, report.len())
    }

    fn send_feature_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        // New hidapi may pad a short Windows Feature report. Do not silently
        // change the original HID.node's exact-length HidD_SetFeature call.
        checked_native_io(
            &|| {
                ensure!(
                    report.len() == self.feature_report_bytes,
                    "源报文长度 {} 与接口实际 Feature 长度 {} 不一致",
                    report.len(),
                    self.feature_report_bytes
                );
                Ok(())
            },
            || self.transport.send_feature_counted(report),
            &no_validation,
        )
    }

    pub(super) fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        let length = report.len();
        checked_native_read(
            &|| {
                ensure!(
                    length == self.feature_report_bytes,
                    "Feature 响应缓冲区与实际接口长度不一致"
                );
                Ok(())
            },
            report,
            |report| self.transport.get_feature(report),
            &no_validation,
        )
    }

    pub(super) fn metadata(&self) -> Value {
        self.transport.metadata()
    }
}

impl FeatureTransport for Device {
    fn begin_exchange(
        &self,
        options: razer_device::backend::ExchangeOptions,
    ) -> anyhow::Result<Box<dyn razer_device::backend::ExchangeLease + '_>> {
        Ok(razer_device::backend::checked_exchange(
            self.transport.begin_exchange(options)?,
            &no_validation,
            &no_validation,
            Some(self.feature_report_bytes),
        ))
    }
    fn begin_exchange_validated<'a>(
        &'a self,
        options: razer_device::backend::ExchangeOptions,
        validate: razer_device::backend::ExchangeValidation<'a>,
    ) -> anyhow::Result<Box<dyn razer_device::backend::ExchangeLease + 'a>> {
        Ok(razer_device::backend::checked_exchange(
            self.transport.begin_exchange_validated(options, validate)?,
            &no_validation,
            &no_validation,
            Some(self.feature_report_bytes),
        ))
    }
    fn hid_node(&self) -> Option<&razer_device::backend::HidNode> {
        self.transport.hid_node()
    }
    fn send_feature_batch(
        &self,
        batch: &razer_device::backend::FeatureBatch,
    ) -> anyhow::Result<razer_device::backend::FeatureBatchReceipt> {
        razer_device::backend::checked_feature_batch(
            &|| {
                for operation in batch.operations() {
                    if let razer_device::backend::FeatureBatchOperation::Report { bytes } =
                        operation
                    {
                        ensure!(
                            bytes.len() == self.feature_report_bytes,
                            "Batch Feature send differs from the observed interface length"
                        );
                    }
                }
                Ok(())
            },
            || self.transport.send_feature_batch(batch),
            &|| Ok(()),
        )
    }

    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        Device::send_feature(self, report)
    }
    fn send_feature_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        Device::send_feature_counted(self, report)
    }
    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        Device::get_feature(self, report)
    }

    fn get_feature_at_collection_max(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        let length = report.len();
        checked_native_read(
            &|| {
                ensure!(
                    length == self.feature_report_bytes,
                    "Collection Feature buffer differs from the observed interface length"
                );
                Ok(())
            },
            report,
            |report| self.transport.get_feature_at_collection_max(report),
            &no_validation,
        )
    }
    fn send_feature_at_collection_max(&self, report: &[u8]) -> anyhow::Result<()> {
        let count = self.send_feature_at_collection_max_counted(report)?;
        require_complete_send(count, report.len())
    }
    fn send_feature_at_collection_max_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        checked_native_io(
            &|| {
                ensure!(
                    report.len() == self.feature_report_bytes,
                    "Collection Feature send differs from the observed interface length"
                );
                Ok(())
            },
            || {
                self.transport
                    .send_feature_at_collection_max_counted(report)
            },
            &no_validation,
        )
    }
    fn metadata(&self) -> Value {
        Device::metadata(self)
    }
}

fn no_validation() -> anyhow::Result<()> {
    Ok(())
}

#[cfg(test)]
mod native_io_tests {
    use super::*;
    use razer_device::backend::{NativeIoOutcome, native_io_outcome};
    use std::{cell::Cell, rc::Rc};

    struct Native(Rc<Cell<usize>>);
    impl FeatureTransport for Native {
        fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
            panic!("Windows adapter must retain the counted native route")
        }
        fn send_feature_counted(&self, _: &[u8]) -> anyhow::Result<usize> {
            self.0.set(self.0.get() + 1);
            Ok(23)
        }
        fn send_feature_at_collection_max_counted(&self, _: &[u8]) -> anyhow::Result<usize> {
            self.0.set(self.0.get() + 1);
            Ok(29)
        }
        fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
            self.0.set(self.0.get() + 1);
            report[1] = 0x71;
            Ok(17)
        }
        fn get_feature_at_collection_max(&self, report: &mut [u8]) -> anyhow::Result<usize> {
            self.0.set(self.0.get() + 1);
            report[1] = 0x73;
            Ok(19)
        }
        fn metadata(&self) -> Value {
            Value::Null
        }
    }

    fn device() -> (Device, Rc<Cell<usize>>) {
        let calls = Rc::new(Cell::new(0));
        (
            Device {
                transport: Box::new(Native(calls.clone())),
                feature_report_bytes: 91,
            },
            calls,
        )
    }

    #[test]
    fn exact_windows_adapter_preserves_counts_and_requires_complete_legacy_sends() {
        let (device, calls) = device();
        let mut report = [0; 91];
        assert_eq!(device.send_feature_counted(&report).unwrap(), 23);
        assert_eq!(
            device
                .send_feature_at_collection_max_counted(&report)
                .unwrap(),
            29
        );
        for collection in [false, true] {
            let error = if collection {
                device.send_feature_at_collection_max(&report)
            } else {
                device.send_feature(&report)
            }
            .unwrap_err();
            assert_eq!(
                native_io_outcome(&error),
                Some(NativeIoOutcome::Returned {
                    byte_count: if collection { 29 } else { 23 },
                })
            );
        }
        assert_eq!(device.get_feature(&mut report).unwrap(), 17);
        assert_eq!(report[1], 0x71);
        assert_eq!(
            device.get_feature_at_collection_max(&mut report).unwrap(),
            19
        );
        assert_eq!(report[1], 0x73);
        assert_eq!(calls.get(), 6);
    }

    #[test]
    fn windows_length_preflight_never_invokes_native_io() {
        let (device, calls) = device();
        for length in [0, 90, 92] {
            let mut report = vec![0; length];
            let errors = [
                device.send_feature_counted(&report).unwrap_err(),
                device
                    .send_feature_at_collection_max_counted(&report)
                    .unwrap_err(),
                device.get_feature(&mut report).unwrap_err(),
                device
                    .get_feature_at_collection_max(&mut report)
                    .unwrap_err(),
            ];
            for error in errors {
                assert_eq!(native_io_outcome(&error), Some(NativeIoOutcome::NotCalled));
            }
        }
        assert_eq!(calls.get(), 0);
    }
}
