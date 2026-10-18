//! Read-only host observations for discovery diagnostics. Never loads libraries.
use razer_storage::{discovery_diagnostics::Query, file_transfer::Operation};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

#[derive(Serialize)]
pub struct Snapshot {
    executable: Executable,
    platform_capabilities: Capabilities,
    native_libraries: NativeInventory,
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

#[derive(Serialize)]
struct NativeInventory {
    scope: &'static str,
    declared: usize,
    declared_functions: usize,
    libraries: Vec<NativeLibrary>,
}

#[derive(Serialize)]
struct NativeLibrary {
    id: String,
    kind: String,
    files: Vec<String>,
    rules: Vec<String>,
    declared_functions: usize,
    load_blocked: bool,
    verified_resources: usize,
    source_verified_container_queries: Vec<ContainerQueries>,
    candidates: Query<Vec<Candidate>>,
}

#[derive(Serialize)]
struct ContainerQueries {
    product_id: u32,
    exports: Vec<String>,
}

#[derive(Serialize)]
struct Candidate {
    rule: String,
    presence: Presence,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum Presence {
    File,
    Other,
    Missing,
    Failed { error: String },
}

fn presence(path: &Path) -> Presence {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => Presence::File,
        Ok(_) => Presence::Other,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Presence::Missing,
        Err(error) => Presence::Failed {
            error: error.to_string(),
        },
    }
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

/// Runs on the connection actor. Cancellation stops the inventory walk and
/// prevents publication through the same operation's atomic commit gate.
pub fn observe(operation: &Operation) -> Result<Snapshot, String> {
    use razer_catalog::native_library;
    operation.check()?;
    let executable = executable();
    let mut libraries = Vec::new();
    for library in native_library::all() {
        operation.check()?;
        let candidates = Query::from_result(
            crate::native_paths::native_library_candidates(&library.id, None)
                .ok_or_else(|| "Native-library candidate paths could not be resolved".to_owned())
                .and_then(|candidates| {
                    candidates
                        .into_iter()
                        .map(|(rule, path)| {
                            operation.check()?;
                            Ok(Candidate {
                                rule,
                                presence: presence(&path),
                            })
                        })
                        .collect::<Result<Vec<_>, String>>()
                }),
        );
        libraries.push(NativeLibrary {
            id: library.id.clone(),
            kind: format!("{:?}", library.kind),
            files: library.file_names.clone(),
            rules: library.path_rules.clone(),
            declared_functions: library.declared_functions.len(),
            load_blocked: razer_catalog::engines::blocks_load(&library.id),
            verified_resources: library.resources.len(),
            source_verified_container_queries: library
                .products
                .iter()
                .map(|product_id| ContainerQueries {
                    product_id: *product_id,
                    exports: native_library::readable_exports(library, *product_id)
                        .into_iter()
                        .map(str::to_owned)
                        .collect(),
                })
                .collect(),
            candidates,
        });
    }
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
        native_libraries: NativeInventory {
            scope: "typed inventory and file presence only; no library is loaded",
            declared: native_library::all().len(),
            declared_functions: native_library::declared_function_count(),
            libraries,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presence_distinguishes_file_directory_and_absence() {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "razer-diagnostic-presence-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("fixture");
        std::fs::write(&file, b"data").unwrap();
        assert!(matches!(presence(&file), Presence::File));
        assert!(matches!(presence(&root), Presence::Other));
        assert!(matches!(presence(&root.join("absent")), Presence::Missing));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cancelled_observation_does_not_read_inventory() {
        let request = razer_storage::file_transfer::Transfer::new();
        let operation = request.accept().unwrap();
        request.cancel();
        assert!(observe(&operation).is_err());
    }
}
