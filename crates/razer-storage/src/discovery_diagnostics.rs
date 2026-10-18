//! Typed discovery reports and request-owned local publication.
use crate::file_transfer::{Operation, Publication};
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

const MAX_REPORT_BYTES: usize = 32 * 1024 * 1024;

/// Raw responses remain at their actual local-service JSON boundary.
#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Query<T> {
    Received { response: T },
    Failed { error: String },
    NotRequested,
}

impl<T> Query<T> {
    pub fn from_result(value: Result<T, String>) -> Self {
        match value {
            Ok(response) => Self::Received { response },
            Err(error) => Self::Failed { error },
        }
    }
}

pub fn query<T>(value: &Option<Result<T, String>>) -> Query<&T> {
    match value {
        Some(Ok(response)) => Query::Received { response },
        Some(Err(error)) => Query::Failed {
            error: error.clone(),
        },
        None => Query::NotRequested,
    }
}

#[derive(Serialize)]
pub struct ServiceQueries<'a> {
    usb: Query<&'a Value>,
    hid: Query<&'a Value>,
    hid_nodes: Query<&'a Value>,
    service_version: Query<&'a Value>,
    audio_devices: Query<&'a Value>,
}

impl<'a> ServiceQueries<'a> {
    pub fn new(
        usb: Query<&'a Value>,
        hid: Query<&'a Value>,
        hid_nodes: Query<&'a Value>,
        service_version: Query<&'a Value>,
        audio_devices: Query<&'a Value>,
    ) -> Self {
        Self {
            usb,
            hid,
            hid_nodes,
            service_version,
            audio_devices,
        }
    }
}

#[derive(Serialize)]
pub struct DeviceObservation<'a, R, H> {
    product_id: u32,
    identity_scope: &'a str,
    hid_node: Option<&'a H>,
    connection_observation: String,
    read_values: Option<&'a R>,
    declared_libraries: Vec<String>,
}

impl<'a, R, H> DeviceObservation<'a, R, H> {
    pub fn new(
        product_id: u32,
        identity_scope: &'a str,
        hid_node: Option<&'a H>,
        connection_observation: String,
        read_values: Option<&'a R>,
        declared_libraries: Vec<String>,
    ) -> Self {
        Self {
            product_id,
            identity_scope,
            hid_node,
            connection_observation,
            read_values,
            declared_libraries,
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Discovery<'a, R, H> {
    Received {
        observations: Vec<DeviceObservation<'a, R, H>>,
        errors: &'a [String],
    },
    Partial {
        observations: Vec<DeviceObservation<'a, R, H>>,
        errors: &'a [String],
    },
    Failed {
        error: &'a str,
    },
    NotRequested,
}

#[derive(Serialize)]
pub struct NativeObservation<'a> {
    product_id: u32,
    device_container_id: &'a str,
    library: &'a str,
    query: Query<&'a Value>,
}

impl<'a> NativeObservation<'a> {
    pub fn new(
        product_id: u32,
        device_container_id: &'a str,
        library: &'a str,
        result: &'a Result<Value, String>,
    ) -> Self {
        Self {
            product_id,
            device_container_id,
            library,
            query: match result {
                Ok(response) => Query::Received { response },
                Err(error) => Query::Failed {
                    error: error.clone(),
                },
            },
        }
    }
}

#[derive(Serialize)]
struct DeviceConfiguration {
    scope: &'static str,
    successful_fields: usize,
    complete_profile_read: bool,
}

/// Platform observations retain their adapter's typed schema without making
/// storage depend on platform, discovery, device protocols, or a GUI crate.
#[derive(Serialize)]
pub struct Report<'a, R, H, P> {
    format_version: u32,
    platform: &'static str,
    recorded_at_utc: String,
    worker_connected: bool,
    services_requested: bool,
    #[serde(flatten)]
    queries: ServiceQueries<'a>,
    discovery: Discovery<'a, R, H>,
    device_configuration: DeviceConfiguration,
    #[serde(flatten)]
    platform_observation: P,
    native_device_values: Vec<NativeObservation<'a>>,
}

impl<'a, R, H, P> Report<'a, R, H, P> {
    pub fn new(
        recorded_at_utc: String,
        worker_connected: bool,
        services_requested: bool,
        queries: ServiceQueries<'a>,
        discovery: Discovery<'a, R, H>,
        platform_observation: P,
        native_device_values: Vec<NativeObservation<'a>>,
        successful_fields: usize,
    ) -> Self {
        Self {
            format_version: 2,
            platform: std::env::consts::OS,
            recorded_at_utc,
            worker_connected,
            services_requested,
            queries,
            discovery,
            device_configuration: DeviceConfiguration {
                scope: "source_verified_basic_queries_only",
                successful_fields,
                complete_profile_read: false,
            },
            platform_observation,
            native_device_values,
        }
    }
}

pub fn path() -> PathBuf {
    crate::store_path().with_file_name("discovery-latest.json")
}

pub fn write(report: &impl Serialize, operation: &Operation) -> Result<PathBuf, String> {
    let path = path();
    write_at(&path, report, operation)?;
    Ok(path)
}

fn write_at(path: &Path, report: &impl Serialize, operation: &Operation) -> Result<(), String> {
    operation.check()?;
    let mut output = BoundedReport(Vec::new());
    serde_json::to_writer_pretty(&mut output, report).map_err(|error| error.to_string())?;
    operation.check()?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    operation.write(path, &output.0, MAX_REPORT_BYTES, Publication::Replace)
}

struct BoundedReport(Vec<u8>);

impl std::io::Write for BoundedReport {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > MAX_REPORT_BYTES.saturating_sub(self.0.len()) {
            return Err(std::io::Error::other(
                "Discovery report exceeds the local size limit",
            ));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_transfer::Transfer;
    use std::io::Write as _;

    fn temporary_directory() -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "razer-discovery-report-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ))
    }

    #[test]
    fn failed_queries_are_distinct_from_unrequested_and_received() {
        let received = Some(Ok(serde_json::json!({"devices": []})));
        let failed: Option<Result<Value, String>> = Some(Err("read failed".into()));
        assert_eq!(
            serde_json::to_value(query(&received)).unwrap()["status"],
            "received"
        );
        assert_eq!(
            serde_json::to_value(query(&failed)).unwrap()["error"],
            "read failed"
        );
        assert_eq!(
            serde_json::to_value(query::<Value>(&None)).unwrap()["status"],
            "not_requested"
        );
    }

    #[test]
    fn cancelled_report_preserves_previous_file_and_cleans_only_owned_stage() {
        let directory = temporary_directory();
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("discovery.json");
        let foreign = directory.join("discovery.json.foreign.tmp");
        std::fs::write(&path, "previous").unwrap();
        std::fs::write(&foreign, "unrelated").unwrap();
        let transfer = Transfer::new();
        let operation = transfer.accept().unwrap();
        transfer.cancel();
        assert!(write_at(&path, &vec![1, 2, 3], &operation).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "previous");
        assert_eq!(std::fs::read_to_string(&foreign).unwrap(), "unrelated");
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 2);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn successful_report_replaces_atomically_without_leftover_stages() {
        let directory = temporary_directory();
        let path = directory.join("discovery.json");
        let transfer = Transfer::new();
        let operation = transfer.accept().unwrap();
        write_at(&path, &vec![1, 2, 3], &operation).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "[\n  1,\n  2,\n  3\n]"
        );
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn bounded_serializer_rejects_growth_before_allocation() {
        let mut output = BoundedReport(vec![0; MAX_REPORT_BYTES - 1]);
        assert!(output.write_all(b"too large").is_err());
        assert_eq!(output.0.len(), MAX_REPORT_BYTES - 1);
    }
}
