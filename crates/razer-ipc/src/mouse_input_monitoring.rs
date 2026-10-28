//! The current host's input-monitor callback, kept separate from redirected
//! mapping input. Device identity belongs to each native batch.
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};

pub const MAX_CALLBACK_BYTES: usize = 1024 * 1024;
pub const MAX_BATCH_EVENTS: usize = 128;
pub const MAX_BATCH_BYTES: usize = 3 * 1024 * 1024;

/// One client's native input-monitor lifetime. A lost START acknowledgement
/// retains the STOP obligation until that same owner's stop is acknowledged.
pub struct Session {
    owner: String,
    phase: CapturePhase,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum CapturePhase {
    Idle,
    StartPending,
    Capturing,
    StopPending,
}
impl Session {
    pub fn new(owner: String) -> anyhow::Result<Self> {
        ensure!(!owner.is_empty(), "Input monitor owner is empty");
        Ok(Self {
            owner,
            phase: CapturePhase::Idle,
        })
    }
    pub fn possibly_started(&self) -> bool {
        self.phase != CapturePhase::Idle
    }
    pub fn start(
        &mut self,
        request: impl FnMut(crate::ServiceRequest) -> anyhow::Result<serde_json::Value>,
    ) -> anyhow::Result<()> {
        ensure!(
            self.phase == CapturePhase::Idle,
            "Input monitor still requires stop acknowledgement"
        );
        self.phase = CapturePhase::StartPending;
        let acknowledgement = self.request_start(request)?;
        match acknowledgement.validate(&self.owner)? {
            StartStatus::Started => {
                self.phase = CapturePhase::Capturing;
                Ok(())
            }
            StartStatus::AlreadyStarted => anyhow::bail!(
                "Input monitor START was rejected: {}",
                acknowledgement.reason()
            ),
        }
    }

    /// Repeats the source START on the same confirmed capture. Its negative
    /// acknowledgement is returned intact and does not release the STOP duty.
    pub fn repeat_start(
        &mut self,
        request: impl FnMut(crate::ServiceRequest) -> anyhow::Result<serde_json::Value>,
    ) -> anyhow::Result<StartAcknowledgement> {
        ensure!(
            self.phase == CapturePhase::Capturing,
            "Input monitor has no confirmed active START to repeat"
        );
        self.request_start(request)
    }

    fn request_start(
        &self,
        mut request: impl FnMut(crate::ServiceRequest) -> anyhow::Result<serde_json::Value>,
    ) -> anyhow::Result<StartAcknowledgement> {
        let acknowledgement: StartAcknowledgement =
            serde_json::from_value(request(crate::ServiceRequest::MouseInputMonitoringStart {
                owner_guid: self.owner.clone(),
            })?)
            .context("Invalid input monitor start acknowledgement")?;
        acknowledgement.validate(&self.owner)?;
        Ok(acknowledgement)
    }
    /// Stops after this client's observer has been removed. Final callbacks
    /// still cross the transport and are validated, but have no consumer here.
    pub fn stop(
        &mut self,
        request: impl FnMut(crate::ServiceRequest) -> anyhow::Result<serde_json::Value>,
    ) -> anyhow::Result<()> {
        self.stop_with(request, |_| Ok(()))
    }

    /// Retains the same native STOP obligation until all final callback pages
    /// are received. Continuation pages drain the stopped owner without another
    /// native STOP. Observer failures do not abandon the remaining pages.
    pub fn stop_with(
        &mut self,
        mut request: impl FnMut(crate::ServiceRequest) -> anyhow::Result<serde_json::Value>,
        mut observe: impl FnMut(Vec<InputEvent>) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let mut failures = Vec::new();
        if self.possibly_started() {
            self.phase = CapturePhase::StopPending;
        }
        while self.possibly_started() {
            let received = (|| {
                let receipt: StopReceipt = serde_json::from_value(request(
                    crate::ServiceRequest::MouseInputMonitoringStop {
                        owner_guid: self.owner.clone(),
                    },
                )?)
                .context("Invalid input monitor stop receipt")?;
                receipt.acknowledgement.validate(&self.owner, false)?;
                ensure!(
                    !receipt.more || !receipt.events.is_empty(),
                    "Input monitor stop continuation made no progress"
                );
                Ok(receipt)
            })();
            let receipt = match received {
                Ok(receipt) => receipt,
                Err(error) => {
                    failures.push(format!("{error:#}"));
                    break;
                }
            };
            self.phase = if receipt.more {
                CapturePhase::StopPending
            } else {
                CapturePhase::Idle
            };
            let result = validate_events(&receipt.events).and_then(|()| {
                if let Some(failure) = receipt.failure {
                    anyhow::bail!("{failure}");
                }
                if failures.is_empty() && !receipt.events.is_empty() {
                    observe(receipt.events)?;
                }
                Ok(())
            });
            if let Err(error) = result {
                let error = format!("{error:#}");
                if !failures.contains(&error) {
                    failures.push(error);
                }
            }
        }
        ensure!(failures.is_empty(), "{}", failures.join("; "));
        Ok(())
    }
    pub fn poll(
        &self,
        mut request: impl FnMut(crate::ServiceRequest) -> anyhow::Result<serde_json::Value>,
    ) -> anyhow::Result<Vec<InputEvent>> {
        ensure!(
            self.phase == CapturePhase::Capturing,
            "Input monitor has no confirmed active START"
        );
        let poll: Poll =
            serde_json::from_value(request(crate::ServiceRequest::MouseInputMonitoringPoll {
                owner_guid: self.owner.clone(),
            })?)
            .context("Invalid input monitor poll receipt")?;
        poll.into_events(&self.owner)
    }
}

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

    pub fn vendor_id(&self) -> u32 {
        self.vendor_id
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

/// The native START callback has the same result/reason shape as the current
/// host wrapper. A false result is an observed negative acknowledgement, not a
/// successful start. `owner_guid` remains part of this local IPC boundary so
/// a stale response cannot be accepted by another session.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct StartAcknowledgement {
    owner_guid: String,
    result: bool,
    reason: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartStatus {
    Started,
    AlreadyStarted,
}

impl StartAcknowledgement {
    pub fn started(owner_guid: String) -> Self {
        Self {
            owner_guid,
            result: true,
            reason: String::new(),
        }
    }

    pub fn rejected(owner_guid: String, reason: impl Into<String>) -> Self {
        Self {
            owner_guid,
            result: false,
            reason: reason.into(),
        }
    }

    pub fn validate(&self, owner: &str) -> anyhow::Result<StartStatus> {
        ensure!(
            !owner.is_empty() && self.owner_guid == owner,
            "Input monitor START acknowledgement belongs to another owner"
        );
        if self.result {
            ensure!(
                self.reason.is_empty(),
                "Input monitor START succeeded with an error reason"
            );
            return Ok(StartStatus::Started);
        }
        ensure!(
            !self.reason.is_empty(),
            "Input monitor START failed without a reason"
        );
        if self.reason == "already started" {
            Ok(StartStatus::AlreadyStarted)
        } else {
            anyhow::bail!("Input monitor START failed: {}", self.reason)
        }
    }

    pub fn result(&self) -> bool {
        self.result
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }
}

/// A page of callbacks retained after the native thread has stopped. `more`
/// keeps this IPC owner reserved until its final page has been handed off.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StopReceipt {
    #[serde(flatten)]
    acknowledgement: Receipt,
    events: Vec<InputEvent>,
    more: bool,
    failure: Option<String>,
}

impl StopReceipt {
    pub fn new(
        owner_guid: String,
        events: Vec<InputEvent>,
        more: bool,
        failure: Option<String>,
    ) -> Self {
        Self {
            acknowledgement: Receipt::new(owner_guid, false),
            events,
            more,
            failure,
        }
    }

    pub fn has_more(&self) -> bool {
        self.more
    }

    pub fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }
}

fn validate_events(events: &[InputEvent]) -> anyhow::Result<()> {
    ensure!(
        events.len() <= MAX_BATCH_EVENTS,
        "Input monitor poll exceeds its batch limit"
    );
    for event in events {
        event.validate()?;
    }
    ensure!(
        serde_json::to_vec(events)?.len() <= MAX_BATCH_BYTES,
        "Input monitor batch exceeds IPC capacity"
    );
    Ok(())
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
        validate_events(&self.events)?;
        Ok(self.events)
    }
}

#[cfg(test)]
mod tests;
