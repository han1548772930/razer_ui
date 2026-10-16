//! Last real discovery response, stored locally so failures can be diagnosed
//! without replacing them with registry history or guessed hardware readings.
use super::Readings;
use anyhow::Context as _;
use serde_json::{Value, json};
use std::{io::Write, path::PathBuf};

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

fn query(value: &Option<Result<Value, String>>) -> Value {
    match value {
        Some(Ok(value)) => json!({"status": "received", "response": value}),
        Some(Err(error)) => json!({"status": "failed", "error": error}),
        None => json!({"status": "not_requested"}),
    }
}

/// Declared native libraries and whether any source-derived candidate path
/// exists. Presence is file-system evidence only: no library is loaded here, so
/// a present file must not read as a working feature and a missing one must not
/// read as a failure of the device itself.
fn native_libraries() -> Value {
    use razer_catalog::native_library;
    use razer_platform::native_paths::native_library_candidates;
    json!({
        "scope": "generated inventory + file presence only; no library is loaded",
        "source": "assets/data/native-library-inventory.json",
        "declared": native_library::all().len(),
        "declared_functions": native_library::declared_function_count(),
        "libraries": native_library::all().iter().map(|library| {
            let candidates = native_library_candidates(&library.id, None).unwrap_or_default();
            let present: Vec<&str> = candidates
                .iter()
                .filter(|(_, path)| path.is_file())
                .map(|(rule, _)| rule.as_str())
                .collect();
            json!({
                "id": library.id,
                "kind": format!("{:?}", library.kind),
                "files": library.file_names,
                "rules": library.path_rules,
                "declared_functions": library.declared_functions.len(),
                "load_blocked": razer_catalog::engines::blocks_load(&library.id),
                "verified_resources": library.resources.len(),
                "source_verified_container_queries": library.products.iter().map(|product|json!({
                    "product_id":product,
                    "exports":razer_catalog::native_library::readable_exports(library,*product),
                })).collect::<Vec<_>>(),
                "present_rules": present,
            })
        }).collect::<Vec<_>>(),
    })
}

impl Readings {
    /// Called by the connection actor after all queries finish, never by render.
    pub fn write_diagnostic_report(&self) -> anyhow::Result<PathBuf> {
        let path = razer_storage::store_path().with_file_name("discovery-latest.json");
        let executable = std::env::current_exe().ok();
        let metadata = executable
            .as_ref()
            .and_then(|path| std::fs::metadata(path).ok());
        let discovery = match &self.discovery {
            Some(Ok(snapshot)) => json!({
                "status": if snapshot.errors().is_empty() { "received" } else { "partial" },
                "observations": snapshot.devices().iter().map(|device| json!({
                    "product_id": device.product_id(),
                    // Portable identity scopes are collection keys, never ContainerIds.
                    "identity_scope": device.container(),
                    "hid_node": device.hid_node(),
                    "connection_observation": format!("{:?}", device.connection()),
                    "read_values": device.read_values(),
                    // Declared-but-unqueried: which native libraries this product's
                    // own app declares. Naming them is not a read result.
                    "declared_libraries": razer_catalog::native_library::libraries_for_product(device.product_id())
                        .iter().map(|library| library.id.clone()).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
                "errors": snapshot.errors(),
            }),
            Some(Err(error)) => json!({"status": "failed", "error": error}),
            None => json!({"status": "not_requested"}),
        };
        let value = json!({
            "format_version": 1,
            "platform": std::env::consts::OS,
            "recorded_at_utc": chrono::Utc::now().to_rfc3339(),
            "executable": executable,
            "executable_bytes": metadata.as_ref().map(|metadata| metadata.len()),
            "executable_modified_utc": metadata.and_then(|metadata| metadata.modified().ok())
                .map(|modified| chrono::DateTime::<chrono::Utc>::from(modified).to_rfc3339()),
            "worker_connected": self.connected,
            "services_requested": self.services_requested,
            "usb": query(&self.usb),
            "hid": query(&self.hid),
            "hid_nodes": query(&self.hid_nodes),
            "service_version": query(&self.version),
            "audio_devices": query(&self.audio),
            "platform_capabilities": {
                "windows_usb_interfaces": if cfg!(windows) { "supported" } else { "unsupported" },
                "windows_container_queries": if cfg!(windows) { "supported" } else { "unsupported" },
                "service_version": if cfg!(windows) { "supported" } else { "unsupported" },
                "audio_devices": if cfg!(windows) { "supported" } else { "unsupported" },
                "portable_hid_enumeration_completeness": "not_reported_by_backend",
            },
            "discovery": discovery,
            "device_configuration": {
                "scope": "source_verified_basic_queries_only",
                "successful_fields": self.discovery.as_ref().and_then(|result| result.as_ref().ok()).map_or(0, read_count),
                "complete_profile_read": false,
            },
            "native_libraries": native_libraries(),
            "native_device_values": self.native.iter().map(|observation|json!({
                "product_id":observation.product_id,"device_container_id":observation.container,
                "library":observation.library,
                "query":match &observation.result {
                    Ok(value)=>json!({"status":"received","response":value}),
                    Err(error)=>json!({"status":"failed","error":error}),
                },
            })).collect::<Vec<_>>(),
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
