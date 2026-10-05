//! Current Dashboard 44442/ae service projection. A snapshot is an observation,
//! never inferred from the local workspace, a click, or the supported PID list.
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// Preserve vendor fields we do not yet consume instead of losing them on save.
pub(crate) type Record = Map<String, Value>;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModuleServiceSnapshot {
    // These lists are required together: absent inventory is not empty inventory.
    installed_devices: Vec<Record>,
    installed_modules: Vec<Record>,
    connected_devices: Vec<Record>,
    device_runtime_data: Vec<Record>,
    cached_device_info: Vec<Record>,
    device_manifest: Vec<Record>,
    uninstalling_devices: Vec<u32>,
    uninstalling_modules: Vec<String>,
    firmware_update_devices: Vec<Record>,
    installer_status: BTreeMap<String, Record>,
    #[serde(default)]
    is_online: Option<bool>,
    #[serde(default)]
    can_remove_macro: Option<bool>,
    #[serde(default)]
    armory_available: Option<bool>,
    #[serde(flatten)]
    extra: Record,
}

pub(crate) fn string(record: &Record, field: &str) -> String {
    record
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .into()
}
pub(crate) fn number(record: &Record, field: &str) -> u32 {
    record
        .get(field)
        .and_then(|v| v.as_u64().or_else(|| v.as_str()?.parse().ok()))
        .and_then(|v| v.try_into().ok())
        .unwrap_or_default()
}
pub(crate) fn flag(record: &Record, field: &str) -> bool {
    record.get(field).and_then(Value::as_bool) == Some(true)
}
pub(crate) fn localized(value: Option<&Value>, locale: &str) -> String {
    let Some(value) = value else {
        return String::new();
    };
    value
        .as_str()
        .or_else(|| value.get(locale).and_then(Value::as_str))
        .or_else(|| value.get("en").and_then(Value::as_str))
        .unwrap_or_default()
        .into()
}
pub(crate) fn identity(record: &Record) -> String {
    format!(
        "{}-{}",
        number(record, "productId"),
        string(record, "serialNumber")
    )
}

pub(crate) struct ServiceGroups {
    pub(crate) firmware: Vec<Record>,
    pub(crate) new_devices: Vec<Record>,
    pub(crate) installed_devices: Vec<Record>,
    pub(crate) installed_modules: Vec<Record>,
}
impl ModuleServiceSnapshot {
    pub(crate) fn online(&self) -> bool {
        self.is_online == Some(true)
    }
    pub(crate) fn macro_removable(&self) -> bool {
        self.can_remove_macro == Some(true)
    }
    pub(crate) fn installed_module(&self, id: &str) -> bool {
        self.installed_modules
            .iter()
            .any(|r| string(r, "moduleName") == id)
    }
    pub(crate) fn progress(&self, id: &str) -> Option<&Record> {
        self.installer_status.get(id)
    }
    pub(crate) fn extra(&self, key: &str) -> Option<&Value> {
        self.extra.get(key)
    }

    fn device_rows(&self, pid: u32, serial: Option<&str>) -> Vec<Record> {
        let mut candidates: Vec<&Record> = self
            .device_runtime_data
            .iter()
            .filter(|r| {
                number(r, "productId") == pid
                    && !string(r, "serialNumber").is_empty()
                    && serial.is_none_or(|s| string(r, "serialNumber") == s)
            })
            .collect();
        let same_pid_connected = self
            .connected_devices
            .iter()
            .any(|r| number(r, "productId") == pid);
        let single_connected = same_pid_connected && candidates.len() == 1;
        if candidates.is_empty() && serial.is_none() {
            candidates = self
                .cached_device_info
                .iter()
                .filter(|r| number(r, "productId") == pid && !string(r, "serialNumber").is_empty())
                .collect();
        }
        let iot_children = self
            .device_runtime_data
            .iter()
            .find(|r| number(r, "productId") == pid && string(r, "category") == "IOT")
            .and_then(|r| r.get("subDevices"))
            .and_then(Value::as_array);
        let mut rows = Vec::new();
        for record in candidates {
            let serial = string(record, "serialNumber");
            let container = string(record, "deviceContainerId");
            let matched = if let Some(children) = iot_children {
                children
                    .iter()
                    .filter_map(Value::as_object)
                    .find(|r| number(r, "productId") == pid && string(r, "serialNumber") == serial)
            } else {
                self.device_runtime_data.iter().find(|r| {
                    number(r, "productId") == pid
                        && string(r, "serialNumber") == serial
                        && (container.is_empty() || string(r, "deviceContainerId") == container)
                })
            };
            let disconnected = if iot_children.is_some() {
                matched.is_none_or(|r| {
                    !flag(r, "isPowerOn") || flag(r, "isLocked") || !flag(r, "isOnline")
                })
            } else {
                matched.is_none()
            };
            let removable = if iot_children.is_some()
                || ["MONITOR", "IOT"].contains(&string(record, "category").as_str())
            {
                disconnected
            } else {
                disconnected && !single_connected
            };
            let mut row = record.clone();
            row.insert(
                "status".into(),
                Value::from(if disconnected {
                    "disconnected"
                } else {
                    "connected"
                }),
            );
            row.insert("removable".into(), Value::from(removable));
            // ae.v uses `{}` for a missing IoT child, which is truthy. IoT
            // disconnection therefore never enables this same-PID fallback.
            row.insert(
                "isSamePIDConnected".into(),
                Value::from(iot_children.is_none() && matched.is_none() && same_pid_connected),
            );
            row.insert(
                "deviceContainerId".into(),
                matched
                    .and_then(|r| r.get("deviceContainerId"))
                    .cloned()
                    .unwrap_or(Value::Null),
            );
            row.insert(
                "title".into(),
                record.get("productName").cloned().unwrap_or(Value::Null),
            );
            row.insert(
                "icon".into(),
                record.get("category").cloned().unwrap_or(Value::Null),
            );
            if let Some(manifest) = self
                .device_manifest
                .iter()
                .find(|r| number(r, "name") == pid)
            {
                for field in ["size", "description"] {
                    if let Some(value) = manifest.get(field) {
                        row.insert(field.into(), value.clone());
                    }
                }
            }
            rows.push(row);
        }
        // ae.v excludes a powered-off NOSERIALNUMBER alias when a serial-bearing
        // sibling with another container is active. It does not deduplicate PIDs.
        let before = rows.clone();
        rows.retain(|r| {
            let serial = string(r, "serialNumber");
            let power = r.get("powerStatus");
            let off = power.is_none_or(Value::is_null)
                || power
                    .and_then(|v| v.get("chargingStatus"))
                    .and_then(Value::as_str)
                    == Some("off")
                || string(r, "devicePowerState") == "off";
            !((serial.is_empty() || serial == "NOSERIALNUMBER")
                && off
                && before.iter().any(|other| {
                    string(other, "deviceContainerId") != string(r, "deviceContainerId")
                        && !string(other, "serialNumber").is_empty()
                        && other
                            .get("powerStatus")
                            .and_then(|v| v.get("chargingStatus"))
                            .and_then(Value::as_str)
                            != Some("off")
                        && string(other, "devicePowerState") != "off"
                }))
        });
        rows
    }
    pub(crate) fn groups(&self) -> ServiceGroups {
        let installed: Vec<u32> = self
            .installed_devices
            .iter()
            .map(|r| number(r, "productId"))
            .collect();
        let new_devices = self
            .connected_devices
            .iter()
            .filter(|r| !installed.contains(&number(r, "productId")))
            .flat_map(|r| {
                let serial = string(r, "serialNumber");
                self.device_rows(
                    number(r, "productId"),
                    (!serial.is_empty()).then_some(serial.as_str()),
                )
            })
            .collect();
        let mut installed_devices = Vec::new();
        for record in &self.installed_devices {
            let pid = number(record, "productId");
            if self.uninstalling_devices.contains(&pid) {
                continue;
            }
            for mut row in self.device_rows(pid, None) {
                if let Some(date) = record.get("installedDate") {
                    row.insert("lastUpdated".into(), date.clone());
                } else {
                    // JS undefined and null have different Date semantics.
                    row.remove("lastUpdated");
                }
                installed_devices.push(row);
            }
        }
        installed_devices.sort_by_key(|r| string(r, "status") != "connected");
        let mut removing: Vec<Record> = self
            .uninstalling_devices
            .iter()
            .flat_map(|pid| self.device_rows(*pid, None))
            .map(|mut r| {
                r.insert("status".into(), Value::from("uninstalling"));
                r
            })
            .collect();
        removing.append(&mut installed_devices);
        let mut installed_modules = Vec::new();
        for name in &self.uninstalling_modules {
            if name == "armory" && self.armory_available != Some(true) {
                continue;
            }
            installed_modules.push(Map::from_iter([
                ("moduleName".into(), Value::from(name.clone())),
                ("status".into(), Value::from("uninstalling")),
            ]));
        }
        for record in &self.installed_modules {
            let name = string(record, "moduleName");
            if self.uninstalling_modules.contains(&name)
                || (name == "armory" && self.armory_available != Some(true))
            {
                continue;
            }
            // ae.f receives only {name}; arbitrary fields on an installed
            // service record do not override the current module directory.
            let mut row = Map::from_iter([("moduleName".into(), Value::from(name))]);
            if let Some(date) = record.get("installedDate") {
                row.insert("lastUpdated".into(), date.clone());
            } else {
                row.remove("lastUpdated");
            }
            installed_modules.push(row);
        }
        let mut firmware: Vec<Record> = self
            .firmware_update_devices
            .iter()
            .filter(|r| flag(r, "needsUpgrade"))
            .cloned()
            .collect();
        let in_app_count = firmware.len();
        for record in &self.device_runtime_data {
            if !record
                .get("firmwareUpdateInfo")
                .is_some_and(Value::is_object)
            {
                continue;
            }
            // ae filters external entries against the original in-app list
            // only; multiple external records with one identity retain order.
            if firmware[..in_app_count]
                .iter()
                .any(|r| identity(r) == identity(record))
            {
                continue;
            }
            if let Some(row) = self
                .device_rows(
                    number(record, "productId"),
                    Some(&string(record, "serialNumber")),
                )
                .into_iter()
                .next()
            {
                let mut merged = record.clone();
                merged.extend(row);
                firmware.push(merged);
            }
        }
        ServiceGroups {
            firmware,
            new_devices,
            installed_devices: removing,
            installed_modules,
        }
    }
}

/// 44442/L: entry connection restrictions apply only to the SDK updater.
pub(crate) fn firmware_entry(record: &Record) -> (bool, Option<&'static str>) {
    if string(record, "upgradeMode") != "SDK" {
        return (true, None);
    }
    let support = record.get("supportUpgradeConnection");
    let denied = |field| support.and_then(|v| v.get(field)).and_then(Value::as_bool) == Some(false);
    match string(record, "connectType").as_str() {
        "ble" => (false, Some("FW_UPDATE_BT_DISABLED")),
        "dongle"
            if flag(record, "isSlaveDevice")
                || (flag(record, "supportDongle") && denied("dongle")) =>
        {
            (false, (!denied("usb")).then_some("FW_UPDATE_DISABLED"))
        }
        _ => (true, None),
    }
}
pub(crate) fn firmware_release(record: &Record) -> Option<&Record> {
    let info = record.get("firmwareUpdateInfo")?.as_object()?;
    if string(record, "connectType") != "dongle" {
        return Some(info);
    }
    let dongle = info.get("dongle").and_then(Value::as_object);
    let current = record.get("firmwareInfo");
    let target = dongle
        .map(|r| string(r, "targetFWVersion"))
        .unwrap_or_default();
    let dongle_version = current
        .and_then(|v| v.get("currentDongleFWVersion"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !target.is_empty() && target != dongle_version {
        return dongle;
    }
    let usb_version = current.and_then(|v| v.get("currentFWVersion"));
    // L uses !==: missing, null and an empty version are distinct values.
    if flag(record, "supportUsb") && info.get("targetFWVersion") != usb_version {
        Some(info)
    } else {
        dongle
    }
}

pub(crate) fn file_size(bytes: u64) -> String {
    if bytes == 0 {
        return "< 1 MB".into();
    }
    let units = ["Bytes", "KB", "MB", "GB", "TB"];
    let ix = ((bytes as f64).log(1024.).floor() as usize).min(4);
    format!(
        "{} {}",
        (bytes as f64 / 1024_f64.powi(ix as i32)).round() as u64,
        units[ix]
    )
}

/// O wraps installed values in JS Date. W passes release strings directly to
/// dayjs: ISO calendar-only installed dates start at UTC midnight, whereas
/// release calendar dates start at local midnight. W also treats falsy values
/// as today before applying its seconds/milliseconds numeric branch.
pub(crate) fn date_label(value: Option<&Value>, locale: &str, release: bool) -> String {
    let locale = locale.to_ascii_lowercase();
    let pattern = match locale.as_str() {
        "en" => "%b %d %Y",
        "ja" | "zh-cn" | "zh-tw" => "%Y年 %m月 %d日",
        "kr" => "%Y년 %m월 %d일",
        "ru" => "%d.%m.%Y",
        _ => "%d/%m/%Y",
    };
    let falsy = value.is_none_or(|v| match v {
        Value::Null => true,
        Value::Bool(value) => !value,
        Value::Number(value) => value.as_f64() == Some(0.),
        Value::String(value) => value.is_empty(),
        _ => false,
    });
    if release && falsy {
        return Local::now().format(pattern).to_string();
    }
    let format_millis = |millis: f64| {
        // ECMAScript TimeClip discards fractions and rejects dates outside
        // +/- 100,000,000 days. Do not reinterpret an invalid value as today.
        if !millis.is_finite() || millis.abs() > 8_640_000_000_000_000. {
            return "Invalid Date".into();
        }
        Local
            .timestamp_millis_opt(millis.trunc() as i64)
            .single()
            .map(|d| d.format(pattern).to_string())
            .unwrap_or_else(|| "Invalid Date".into())
    };
    let numeric = value.and_then(|v| {
        v.as_f64().or_else(|| {
            let raw = v.as_str()?;
            // W's /^\d+$/ excludes signs, whitespace and decimal strings.
            (release && !raw.is_empty() && raw.bytes().all(|b| b.is_ascii_digit()))
                .then(|| raw.parse::<f64>().ok())
                .flatten()
        })
    });
    if let Some(number) = numeric {
        let millis = if release && number <= 1_000_000_000_000. {
            number * 1000.
        } else {
            number
        };
        return format_millis(millis);
    }
    // new Date(null/false/true), including dayjs(true), uses milliseconds.
    match value {
        Some(Value::Null | Value::Bool(false)) => return format_millis(0.),
        Some(Value::Bool(true)) => return format_millis(1.),
        _ => {}
    }
    let text = value.and_then(Value::as_str).unwrap_or_default();
    if let Ok(date) = DateTime::parse_from_rfc3339(text) {
        return date.with_timezone(&Local).format(pattern).to_string();
    }
    if let Ok(date) = DateTime::parse_from_rfc2822(text) {
        return date.with_timezone(&Local).format(pattern).to_string();
    }
    if let Ok(date) = NaiveDate::parse_from_str(text, "%Y-%m-%d") {
        if release {
            return date.format(pattern).to_string();
        }
        return Utc
            .from_utc_datetime(&date.and_hms_opt(0, 0, 0).expect("valid midnight"))
            .with_timezone(&Local)
            .format(pattern)
            .to_string();
    }
    // Both Date and dayjs accept local date-time forms without a UTC suffix.
    for format in [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y/%m/%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M",
    ] {
        if let Ok(date) = NaiveDateTime::parse_from_str(text, format) {
            return Local
                .from_local_datetime(&date)
                .earliest()
                .map(|date| date.format(pattern).to_string())
                .unwrap_or_else(|| "Invalid Date".into());
        }
    }
    if let Ok(date) = NaiveDate::parse_from_str(text, "%Y/%m/%d") {
        return date.format(pattern).to_string();
    }
    "Invalid Date".into()
}
