//! The current host's input-monitor callback, kept separate from redirected
//! mapping input. Device identity belongs to each native batch.
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};

pub const MAX_CALLBACK_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct InputEvent {
    #[serde(rename = "containerId")]
    container_id: String,
    #[serde(rename = "productId")]
    product_id: u32,
    #[serde(rename = "vendorId")]
    vendor_id: u32,
    #[serde(rename = "usbInstanceId")]
    usb_instance_id: Option<String>,
    #[serde(rename = "inputMonitoringEventType")]
    event_type: i32,
    #[serde(rename = "jsonEvent")]
    json_event: String,
}

impl InputEvent {
    /// `device_info` and `json_event` are copied while the callback buffers are
    /// valid. Parsing movement rows remains with the mouse domain owner.
    pub fn from_native(
        device_info: &str,
        event_type: i32,
        json_event: String,
    ) -> anyhow::Result<Self> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Identity {
            container_id: String,
            product_id: u32,
            vendor_id: u32,
            usb_instance_id: Option<String>,
        }
        ensure!(
            device_info.len().saturating_add(json_event.len()) <= MAX_CALLBACK_BYTES,
            "Input monitor callback exceeds the transport limit"
        );
        let info: Identity = serde_json::from_str(device_info)
            .context("Input monitor callback has invalid device identity")?;
        ensure!(
            !info.container_id.is_empty() && info.product_id != 0 && info.vendor_id != 0,
            "Input monitor callback has incomplete device identity"
        );
        Ok(Self {
            container_id: info.container_id,
            product_id: info.product_id,
            vendor_id: info.vendor_id,
            usb_instance_id: info.usb_instance_id,
            event_type,
            json_event,
        })
    }

    pub fn container_id(&self) -> &str {
        &self.container_id
    }

    pub fn product_id(&self) -> u32 {
        self.product_id
    }

    pub fn json_event(&self) -> &str {
        &self.json_event
    }

    fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            !self.container_id.is_empty() && self.product_id != 0 && self.vendor_id != 0,
            "Input monitor batch has incomplete device identity"
        );
        let bytes = self
            .container_id
            .len()
            .saturating_add(self.json_event.len())
            .saturating_add(self.usb_instance_id.as_ref().map_or(0, String::len));
        ensure!(
            bytes <= MAX_CALLBACK_BYTES,
            "Input monitor batch exceeds its callback limit"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Receipt {
    owner_guid: String,
    monitoring: bool,
}

impl Receipt {
    pub fn new(owner_guid: String, monitoring: bool) -> Self {
        Self {
            owner_guid,
            monitoring,
        }
    }

    pub fn validate(&self, owner: &str, monitoring: bool) -> anyhow::Result<()> {
        ensure!(
            !owner.is_empty() && self.owner_guid == owner && self.monitoring == monitoring,
            "Input monitor acknowledgement belongs to another owner or state"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Poll {
    owner_guid: String,
    events: Vec<InputEvent>,
    failure: Option<String>,
}

impl Poll {
    pub fn new(owner_guid: String, events: Vec<InputEvent>, failure: Option<String>) -> Self {
        Self {
            owner_guid,
            events,
            failure,
        }
    }

    pub fn into_events(self, owner: &str) -> anyhow::Result<Vec<InputEvent>> {
        ensure!(
            !owner.is_empty() && self.owner_guid == owner,
            "Input monitor batch belongs to another owner"
        );
        if let Some(failure) = self.failure {
            anyhow::bail!("{failure}");
        }
        ensure!(
            self.events.len() <= 128,
            "Input monitor poll exceeds its batch limit"
        );
        for event in &self.events {
            event.validate()?;
        }
        Ok(self.events)
    }
}

#[cfg(test)]
mod tests;
