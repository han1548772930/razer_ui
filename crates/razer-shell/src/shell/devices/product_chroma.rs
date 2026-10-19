//! Shared local profile, installation and host memory observations.
use anyhow::{Context as _, ensure};
use razer_storage::{
    chroma_profiles::{self, Snapshot},
    host::{HostStorage, HostStorageCall, HostStorageReply, HostStoreKind},
};
use serde::Deserialize;
use serde_json::json;

#[cfg(test)]
#[path = "product_chroma_tests.rs"]
mod tests;

pub(super) const CONNECTED_DEVICES_KEY: &str = "lighting-engine-chroma-devices";

/// Local storage and OS installation failures have independent results. This
/// snapshot does not claim a current native engine or physical device state.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Readback {
    pub(super) snapshot: Result<Snapshot, String>,
    pub(super) dynamic_lighting: Result<Option<bool>, String>,
    pub(super) module_installed: Result<Option<bool>, String>,
    pub(super) installation:
        Result<Option<razer_platform::chroma_installation::Installation>, String>,
}

#[derive(Clone, Copy)]
pub(super) enum InstallationRead {
    Required,
    NotRequested,
}

impl Readback {
    pub(super) fn result(&self) -> Result<(), String> {
        let errors = [
            ("Chroma profiles", self.snapshot.as_ref().err()),
            ("Dynamic lighting", self.dynamic_lighting.as_ref().err()),
            ("Chroma module", self.module_installed.as_ref().err()),
            ("Chroma installation", self.installation.as_ref().err()),
        ]
        .into_iter()
        .filter_map(|(stage, error)| error.map(|error| format!("{stage}: {error}")))
        .collect::<Vec<_>>();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }
}

/// The caller owns background execution and scope/cancellation admission.
pub(super) fn readback() -> Readback {
    readback_scope(InstallationRead::Required)
}

pub(super) fn readback_scope(installation: InstallationRead) -> Readback {
    readback_with(
        || {
            match chroma_profiles::path().and_then(|path| {
                Ok((path, razer_storage::source_local::path()?))
            }) {
                Ok((path, source_path)) => chroma_profiles::observe(&path, &source_path),
                Err(error) => {
                    let error = format!("{error:#}");
                    chroma_profiles::Observation {
                        snapshot: Err(error.clone()),
                        dynamic_lighting: Err(error.clone()),
                        module_installed: Err(error),
                    }
                }
            }
        },
        installation,
        razer_platform::chroma_installation::inspect,
    )
}

pub(super) fn from_snapshot(snapshot: Snapshot) -> Readback {
    let mut observation = readback();
    if !observation
        .snapshot
        .as_ref()
        .is_ok_and(|current| current.supersedes(&snapshot))
    {
        observation.snapshot = Ok(snapshot);
    }
    observation
}

fn readback_with(
    storage: impl FnOnce() -> chroma_profiles::Observation,
    scope: InstallationRead,
    installation: impl FnOnce() -> anyhow::Result<razer_platform::chroma_installation::Installation>,
) -> Readback {
    let chroma_profiles::Observation {
        snapshot,
        dynamic_lighting,
        module_installed,
    } = storage();
    let installation = match scope {
        InstallationRead::Required => installation()
            .map(Some)
            .map_err(|error| format!("{error:#}")),
        InstallationRead::NotRequested => Ok(None),
    };
    Readback {
        snapshot,
        dynamic_lighting,
        module_installed,
        installation,
    }
}

pub(super) fn connected_products(
    storage: &mut HostStorage,
    sender_id: u64,
) -> anyhow::Result<Option<Vec<u32>>> {
    let reply = storage.call(HostStorageCall {
        store: HostStoreKind::Memory,
        sender_id,
        action: "getMemoryStorageItem".into(),
        payload: json!({"key": CONNECTED_DEVICES_KEY}),
        target_url_array: Vec::new(),
    })?;
    decode_connected_products(reply)
}

fn decode_connected_products(reply: HostStorageReply) -> anyhow::Result<Option<Vec<u32>>> {
    #[derive(Deserialize)]
    struct Row {
        value: String,
    }
    ensure!(reply.defined, "Chroma memory getter did not return a value");
    let rows: Vec<Row> = serde_json::from_str(
        reply
            .value
            .as_str()
            .context("Chroma memory response is not a string")?,
    )?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    decode_connected_devices_value(&row.value).map(Some)
}

pub(super) fn decode_connected_devices_value(value: &str) -> anyhow::Result<Vec<u32>> {
    #[derive(Deserialize)]
    struct Envelope {
        payload: Devices,
    }
    #[derive(Deserialize)]
    struct Devices {
        devices: Vec<Product>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Product {
        product_id: u32,
    }
    let devices: Envelope = serde_json::from_str(value)?;
    Ok(devices
        .payload
        .devices
        .into_iter()
        .map(|device| device.product_id)
        .collect())
}
