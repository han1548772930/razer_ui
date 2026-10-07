//! Last real discovery response, stored locally so failures can be diagnosed
//! without replacing them with registry history or guessed hardware readings.
use super::Readings;
use anyhow::Context as _;
use serde_json::{Value, json};
use std::{io::Write, path::PathBuf};

fn value_count(values: &crate::backend::device_reads::DeviceReadValues) -> usize {
    [
        values.firmware.is_some(),
        values.battery_percent.is_some(),
        values.charging_status.is_some(),
        values.polling_hz.is_some(),
        values.dpi.is_some(),
    ]
    .into_iter()
    .filter(|present| *present)
    .count()
}

pub(super) fn read_count(snapshot: &crate::backend::discovery::DiscoverySnapshot) -> usize {
    snapshot
        .devices()
        .iter()
        .filter_map(|device| device.read_values())
        .map(value_count)
        .sum()
}

pub(super) fn device_values(
    product_id: u32,
    values: &crate::backend::device_reads::DeviceReadValues,
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

fn query(value: &Option<Result<Value, String>>) -> Value {
    match value {
        Some(Ok(value)) => json!({"status": "received", "response": value}),
        Some(Err(error)) => json!({"status": "failed", "error": error}),
        None => json!({"status": "not_requested"}),
    }
}

impl Readings {
    /// Called by the connection actor after all queries finish, never by render.
    pub(super) fn write_diagnostic_report(&self) -> anyhow::Result<PathBuf> {
        let path = crate::store::store_path().with_file_name("discovery-latest.json");
        let executable = std::env::current_exe().ok();
        let metadata = executable
            .as_ref()
            .and_then(|path| std::fs::metadata(path).ok());
        let discovery = match &self.discovery {
            Some(Ok(snapshot)) => json!({
                "status": if snapshot.errors().is_empty() { "received" } else { "partial" },
                "observations": snapshot.devices().iter().map(|device| json!({
                    "product_id": device.product_id(),
                    "connection_observation": format!("{:?}", device.connection()),
                    "read_values": device.read_values(),
                })).collect::<Vec<_>>(),
                "errors": snapshot.errors(),
            }),
            Some(Err(error)) => json!({"status": "failed", "error": error}),
            None => json!({"status": "not_requested"}),
        };
        let value = json!({
            "format_version": 1,
            "recorded_at_utc": chrono::Utc::now().to_rfc3339(),
            "executable": executable,
            "executable_bytes": metadata.as_ref().map(|metadata| metadata.len()),
            "executable_modified_utc": metadata.and_then(|metadata| metadata.modified().ok())
                .map(|modified| chrono::DateTime::<chrono::Utc>::from(modified).to_rfc3339()),
            "worker_connected": self.connected,
            "services_requested": self.services_requested,
            "usb": query(&self.usb),
            "hid": query(&self.hid),
            "discovery": discovery,
            "device_configuration": {
                "scope": "source_verified_basic_queries_only",
                "successful_fields": self.discovery.as_ref().and_then(|result| result.as_ref().ok()).map_or(0, read_count),
                "complete_profile_read": false,
            },
        });
        let parent = path.parent().context("设备发现记录目录无效")?;
        std::fs::create_dir_all(parent).context("无法创建设备发现记录目录")?;
        let temporary = path.with_extension(format!("json.{}.tmp", std::process::id()));
        let result = (|| {
            let mut file = std::fs::File::create(&temporary)?;
            file.write_all(&serde_json::to_vec_pretty(&value)?)?;
            file.sync_all()?;
            drop(file);
            std::fs::rename(&temporary, &path)?;
            Ok::<_, anyhow::Error>(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result.context("无法保存本次设备发现详情")?;
        Ok(path)
    }
}
