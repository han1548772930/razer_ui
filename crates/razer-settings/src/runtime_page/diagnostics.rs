//! Last real discovery response, stored locally so failures can be diagnosed
//! without replacing them with registry history or guessed hardware readings.
use super::Readings;
use razer_storage::discovery_diagnostics::{
    self as reports, DeviceObservation, Discovery, Report, ServiceQueries, query,
};
use razer_storage::file_transfer::Operation;
use std::path::PathBuf;

fn value_count(values: &razer_device::device_reads::DeviceReadValues) -> usize {
    [
        values.firmware.is_some(),
        values.battery_percent.is_some(),
        values.charging_status.is_some(),
        values.polling_hz.is_some(),
        values.dpi.is_some(),
        values.idle_raw_time.is_some(),
    ]
    .into_iter()
    .filter(|present| *present)
    .count()
}

pub fn read_count(snapshot: &razer_discovery::discovery::DiscoverySnapshot) -> usize {
    snapshot
        .devices()
        .iter()
        .filter_map(|device| device.read_values())
        .map(value_count)
        .sum()
}

pub fn device_values(
    product_id: u32,
    values: &razer_device::device_reads::DeviceReadValues,
    cx: &gpui_kit::App,
) -> gpui_kit::AnyElement {
    use gpui_kit::{IntoElement, ParentElement, Styled};
    let mut fields = Vec::new();
    if let Some(version) = &values.firmware {
        fields.push(format!("固件：{version}"));
    }
    if let Some(percent) = values.battery_percent {
        fields.push(format!("电量：{percent}%"));
    }
    if let Some(state) = &values.charging_status {
        fields.push(format!("充电状态：{state}"));
    }
    if let Some(hz) = values.polling_hz {
        fields.push(format!("回报率：{hz} Hz"));
    }
    if let Some((x, y)) = values.dpi {
        fields.push(format!("当前 DPI：X {x} / Y {y}"));
    }
    for (field, error) in &values.errors {
        fields.push(format!("{field} 读取失败：{error}"));
    }
    gpui_kit::div()
        .child(super::surface::note(
            format!("产品 {product_id} 的本次读取"),
            cx,
        ))
        .children(
            fields
                .into_iter()
                .map(|text| gpui_kit::div().whitespace_normal().child(text)),
        )
        .into_any_element()
}

impl Readings {
    /// The retained connection actor owns observation and report completion.
    pub fn write_diagnostic_report(&self, operation: &Operation) -> Result<PathBuf, String> {
        let platform = razer_platform::discovery_diagnostics::observe(operation)?;
        let discovery = match &self.discovery {
            Some(Ok(snapshot)) => {
                let observations = snapshot
                    .devices()
                    .iter()
                    .map(|device| {
                        DeviceObservation::new(
                            device.product_id(),
                            device.container(),
                            device.hid_node(),
                            format!("{:?}", device.connection()),
                            device.read_values(),
                            razer_catalog::native_library::libraries_for_product(
                                device.product_id(),
                            )
                            .iter()
                            .map(|library| library.id.clone())
                            .collect(),
                        )
                    })
                    .collect();
                if snapshot.errors().is_empty() {
                    Discovery::Received {
                        observations,
                        errors: snapshot.errors(),
                    }
                } else {
                    Discovery::Partial {
                        observations,
                        errors: snapshot.errors(),
                    }
                }
            }
            Some(Err(error)) => Discovery::Failed { error },
            None => Discovery::NotRequested,
        };
        let report = Report::new(
            chrono::Utc::now().to_rfc3339(),
            self.connected,
            self.services_requested,
            ServiceQueries::new(
                query(&self.usb),
                query(&self.hid),
                query(&self.hid_nodes),
                query(&self.version),
                query(&self.audio),
            ),
            discovery,
            platform,
            self.discovery
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .map_or(0, read_count),
        );
        reports::write(&report, operation)
    }
}
