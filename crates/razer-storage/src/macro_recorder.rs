//! Current Macro producer and PID192 An/q WindowStorage lock semantics.
//! Evidence: host-current/local-audit.json, macro_recorder complete owners.
//! WindowStorage is process-local; this flag is never a native recorder receipt.
use anyhow::{Context, Result};
use serde_json::{Value, json};

pub const STORAGE_KEY: &str = "RzWinStorage_macro_recorder";

pub fn software_exclusive_value() -> String {
    json!({"mode":"exclusive","payload":{"macro_recorder_mode":"kSoftware"}}).to_string()
}

/// Current keyboard ON_OTFM writes an object before broadcasting start-otfm.
/// This is deliberately not an exclusive owner and not a JSON string.
pub fn otfm_trigger_value(product_id: u32, container_id: &str) -> Value {
    json!({"macro_recorder_mode":"kOTFM",
        "deviceId":format!("usb_5426_{product_id}_{container_id}"),
        "containerId":container_id})
}

/// Background lo publishes before awaiting the global recorder lock.
pub fn otfm_exclusive_value() -> String {
    json!({"payload":{"macro_recorder_mode":"kOTFM"},"mode":"exclusive"}).to_string()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecorderMode {
    Software,
    Otfm,
}
impl RecorderMode {
    fn from_value(value: &Value) -> Option<Self> {
        match value.as_str()? {
            "kSoftware" => Some(Self::Software),
            "kOTFM" => Some(Self::Otfm),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecorderNotification {
    Started(RecorderMode),
    Stopped(RecorderMode),
}

#[derive(Default)]
pub struct RecorderLock {
    exclusive: bool,
}
impl RecorderLock {
    pub fn is_exclusive(&self) -> bool {
        self.exclusive
    }

    /// An stops at the first exclusive owner and reads mode at the TOP LEVEL.
    /// The current software producer instead nests that field in payload.
    pub fn initialize(&mut self, aggregate: Option<&str>) -> Option<RecorderNotification> {
        match first_exclusive(aggregate) {
            Ok(Some(value)) => {
                self.exclusive = true;
                RecorderMode::from_value(&value["macro_recorder_mode"])
                    .map(RecorderNotification::Started)
            }
            Ok(None) | Err(_) => {
                self.exclusive = false;
                None
            }
        }
    }

    /// q reads live modes from PAYLOAD. Removal re-reads every remaining owner;
    /// a different exclusive owner must keep access held. A malformed aggregate
    /// follows the source catch and clears the flag without inventing a reply.
    pub fn event(
        &mut self,
        event: &Value,
        aggregate: Option<&str>,
    ) -> Result<Option<RecorderNotification>> {
        if event["key"] != STORAGE_KEY {
            return Ok(None);
        }
        let new = event_json(event.get("newValue"));
        let old = event_json(event.get("oldValue"));
        anyhow::ensure!(!new.is_null(), "Macro storage newValue is JSON null");
        if new["mode"] == "exclusive" {
            self.exclusive = true;
            return Ok(RecorderMode::from_value(&new["payload"]["macro_recorder_mode"])
                .map(RecorderNotification::Started));
        }
        if matches!(event["keyState"].as_str(), Some("removed" | "cleared"))
            && old["mode"] == "exclusive"
        {
            self.exclusive = first_exclusive(aggregate).ok().flatten().is_some();
            if !self.exclusive {
                return Ok(RecorderMode::from_value(&old["payload"]["macro_recorder_mode"])
                    .map(RecorderNotification::Stopped));
            }
        }
        Ok(None)
    }
}

fn event_json(value: Option<&Value>) -> Value {
    value
        .and_then(Value::as_str)
        .and_then(|value| serde_json::from_str(value).ok())
        .unwrap_or_else(|| json!({}))
}

fn first_exclusive(aggregate: Option<&str>) -> Result<Option<Value>> {
    let rows: Value = serde_json::from_str(aggregate.filter(|value| !value.is_empty()).unwrap_or("[]"))?;
    for row in rows.as_array().context("Macro storage aggregate is not an array")? {
        let value: Value = serde_json::from_str(row["value"].as_str().context("Macro owner value is not a JSON string")?)?;
        anyhow::ensure!(!value.is_null(), "Macro owner value is JSON null");
        if value["mode"] == "exclusive" {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

#[cfg(test)]
#[path = "macro_recorder_tests.rs"]
mod tests;
