//! Current Settings 720 notification writer and 959/2864 visibility consumer.
//! The app metadata write precedes the shared key; these are separate commits.
use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

pub(crate) const DISPLAY_KEY: &str = "displayNotifications";
const APPS_KEY: &str = "apps";
const SYNAPSE_APP: &str = "synapse";
const UPDATED_KEY: &str = "lastUpdated";

#[derive(Deserialize, Serialize)]
struct AppRecord {
    name: String,
    // The SDK permits dynamic app extensions. Keep them at this file boundary.
    #[serde(flatten)]
    extensions: BTreeMap<String, serde_json::Value>,
}

fn update_app_record(path: &Path, value: bool, updated_ms: u64) -> anyhow::Result<()> {
    crate::source_local::update(path, APPS_KEY, |previous| {
        // 7071.getAppItems treats absent and non-array values as an empty list.
        let Some(serde_json::Value::Array(items)) = previous else {
            return Ok(None);
        };
        let mut records: Vec<AppRecord> = serde_json::from_value(serde_json::Value::Array(items))
            .context("invalid local app metadata")?;
        let Some(record) = records.iter_mut().find(|item| item.name == SYNAPSE_APP) else {
            return Ok(None);
        };
        record.extensions.insert(DISPLAY_KEY.into(), value.into());
        record
            .extensions
            .insert(UPDATED_KEY.into(), updated_ms.into());
        Ok(Some(serde_json::to_value(records)?))
    })?;
    Ok(())
}

fn save_at_with(
    path: &Path,
    value: bool,
    updated_ms: u64,
    publish_key: impl FnOnce(&Path, bool) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    update_app_record(path, value, updated_ms).context("update notification app metadata")?;
    // An absent app is not created. An app publication error stops this write;
    // a later key error must leave the already-published metadata intact.
    publish_key(path, value).context("publish displayNotifications")
}

pub(crate) fn save_at(path: &Path, value: bool) -> anyhow::Result<()> {
    let updated_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis()
        .try_into()
        .context("notification metadata timestamp exceeds u64")?;
    save_at_with(path, value, updated_ms, |path, value| {
        crate::source_local::set(path, DISPLAY_KEY, &value.into())
    })
}

/// Migrate only the retired workspace choice. A current shared value wins,
/// and malformed data is preserved with an error instead of a default write.
pub(crate) fn migrate_legacy(workspace: &Path, value: bool) -> anyhow::Result<()> {
    let path = crate::source_local::beside_workspace(workspace);
    crate::source_local::update(&path, DISPLAY_KEY, |previous| {
        if let Some(previous) = previous {
            anyhow::ensure!(
                previous.is_boolean(),
                "invalid displayNotifications preference"
            );
            Ok(None)
        } else {
            Ok(Some(value.into()))
        }
    })?;
    Ok(())
}

/// The host compares strictly with true/false. Other source values leave the
/// existing visibility unchanged; they never establish an enabled state.
pub fn visibility_decision(raw: Option<&str>) -> (Option<bool>, bool) {
    match raw.map(serde_json::from_str::<serde_json::Value>) {
        None | Some(Ok(serde_json::Value::Null)) => (Some(true), true),
        Some(Ok(serde_json::Value::Bool(value))) => (Some(value), false),
        _ => (None, false),
    }
}

fn visibility_at(path: &Path) -> anyhow::Result<Option<bool>> {
    let raw = crate::source_local::get_raw(path, DISPLAY_KEY)?;
    let (value, initialize) = visibility_decision(raw.as_deref());
    if !initialize {
        return Ok(value);
    }
    crate::source_local::update(path, DISPLAY_KEY, |previous| {
        Ok(match previous {
            None | Some(serde_json::Value::Null) => Some(true.into()),
            _ => None,
        })
    })?;
    Ok(visibility_decision(crate::source_local::get_raw(path, DISPLAY_KEY)?.as_deref()).0)
}

/// Called by the notification array/measurement consumer, not by a settings
/// click. This does not clear queues, cancel timers, or gate native Toasts.
pub fn visibility() -> anyhow::Result<Option<bool>> {
    visibility_at(&crate::source_local::path()?)
}

/// Read the exact localStorage string for the mounted settings listener.
/// This observation never initializes a value or publishes app metadata.
pub fn raw_value() -> anyhow::Result<Option<String>> {
    crate::source_local::get_raw(&crate::source_local::path()?, DISPLAY_KEY)
}

#[cfg(test)]
#[path = "notification_preferences/tests.rs"]
mod tests;
