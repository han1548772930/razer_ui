//! Read-only host observations for discovery diagnostics. Never loads libraries.
use razer_storage::{discovery_diagnostics::Query, file_transfer::Operation};
use serde::Serialize;
use std::{path::PathBuf, time::UNIX_EPOCH};

#[derive(Serialize)]
pub struct Snapshot {
    executable: Executable,
    platform_capabilities: Capabilities,
}

#[derive(Serialize)]
struct Executable {
    path: Query<PathBuf>,
    bytes: Query<u64>,
    modified_unix_millis: Query<u128>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Capability {
    Supported,
    Unsupported,
}

impl Capability {
    fn windows() -> Self {
        if cfg!(windows) {
            Self::Supported
        } else {
            Self::Unsupported
        }
    }
}

#[derive(Serialize)]
struct Capabilities {
    windows_usb_interfaces: Capability,
    windows_container_queries: Capability,
    service_version: Capability,
    audio_devices: Capability,
    portable_hid_enumeration_completeness: &'static str,
}

fn executable() -> Executable {
    let path = match std::env::current_exe() {
        Ok(path) => path,
        Err(error) => {
            let error = error.to_string();
            return Executable {
                path: Query::Failed {
                    error: error.clone(),
                },
                bytes: Query::Failed {
                    error: error.clone(),
                },
                modified_unix_millis: Query::Failed { error },
            };
        }
    };
    let (bytes, modified_unix_millis) = match std::fs::metadata(&path) {
        Ok(metadata) => (
            Query::Received {
                response: metadata.len(),
            },
            Query::from_result(
                metadata
                    .modified()
                    .map_err(|error| error.to_string())
                    .and_then(|time| {
                        time.duration_since(UNIX_EPOCH)
                            .map(|duration| duration.as_millis())
                            .map_err(|error| error.to_string())
                    }),
            ),
        ),
        Err(error) => {
            let error = error.to_string();
            (
                Query::Failed {
                    error: error.clone(),
                },
                Query::Failed { error },
            )
        }
    };
    Executable {
        path: Query::Received { response: path },
        bytes,
        modified_unix_millis,
    }
}

/// Runs on the connection actor. Cancellation stops the filesystem observation and
/// prevents publication through the same operation's atomic commit gate.
pub fn observe(operation: &Operation) -> Result<Snapshot, String> {
    operation.check()?;
    let executable = executable();
    operation.check()?;
    Ok(Snapshot {
        executable,
        platform_capabilities: Capabilities {
            windows_usb_interfaces: Capability::windows(),
            windows_container_queries: Capability::windows(),
            service_version: Capability::windows(),
            audio_devices: Capability::windows(),
            portable_hid_enumeration_completeness: "not_reported_by_backend",
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelled_observation_does_not_read_filesystem() {
        let request = razer_storage::file_transfer::Transfer::new();
        let operation = request.accept().unwrap();
        request.cancel();
        assert!(observe(&operation).is_err());
    }
}
