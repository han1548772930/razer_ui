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
    pub(super) installation: Result<razer_platform::chroma_installation::Installation, String>,
}

/// The caller owns background execution and scope/cancellation admission.
pub(super) fn readback() -> Readback {
    readback_with(
        || {
            chroma_profiles::load(
                &chroma_profiles::path(),
                &razer_storage::source_local::path(),
            )
        },
        razer_platform::chroma_installation::inspect,
    )
}

pub(super) fn from_snapshot(snapshot: Snapshot) -> Readback {
    readback_with(
        || Ok(snapshot),
        razer_platform::chroma_installation::inspect,
    )
}

fn readback_with(
    snapshot: impl FnOnce() -> anyhow::Result<Snapshot>,
    installation: impl FnOnce() -> anyhow::Result<razer_platform::chroma_installation::Installation>,
) -> Readback {
    let snapshot = snapshot().map_err(|error| format!("{error:#}"));
    let installation = installation().map_err(|error| format!("{error:#}"));
    Readback {
        snapshot,
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
    let devices: Envelope = serde_json::from_str(&row.value)?;
    Ok(Some(
        devices
            .payload
            .devices
            .into_iter()
            .map(|device| device.product_id)
            .collect(),
    ))
}
