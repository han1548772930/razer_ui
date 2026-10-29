//! Worker adapter from observed Windows identity to the portable HID backend.
//! Contains no native DLL loading or OS HID calls. Protocols live above it.
use anyhow::ensure;
use razer_device::backend::FeatureTransport;
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
        // New hidapi may pad a short Windows Feature report. Do not silently
        // change the original HID.node's exact-length HidD_SetFeature call.
        ensure!(
            report.len() == self.feature_report_bytes,
            "源报文长度 {} 与接口实际 Feature 长度 {} 不一致",
            report.len(),
            self.feature_report_bytes
        );
        self.transport.send_feature(report)
    }

    pub(super) fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        ensure!(
            report.len() == self.feature_report_bytes,
            "Feature 响应缓冲区与实际接口长度不一致"
        );
        self.transport.get_feature(report)
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

    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        Device::send_feature(self, report)
    }
    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        Device::get_feature(self, report)
    }

    fn get_feature_at_collection_max(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        ensure!(
            report.len() == self.feature_report_bytes,
            "Collection Feature buffer differs from the observed interface length"
        );
        self.transport.get_feature_at_collection_max(report)
    }
    fn send_feature_at_collection_max(&self, report: &[u8]) -> anyhow::Result<()> {
        ensure!(
            report.len() == self.feature_report_bytes,
            "Collection Feature send differs from the observed interface length"
        );
        self.transport.send_feature_at_collection_max(report)
    }
    fn metadata(&self) -> Value {
        Device::metadata(self)
    }
}

fn no_validation() -> anyhow::Result<()> {
    Ok(())
}
