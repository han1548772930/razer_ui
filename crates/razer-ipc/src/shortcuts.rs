//! The retained global mapping owner and its observed native receipts.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Initialization and unsupported-mapping callback registration were acknowledged.
/// No shortcut document or device configuration is confirmed by this receipt.
pub struct RegistrationReceipt {
    owner_guid: String,
    registered: bool,
}
impl RegistrationReceipt {
    pub fn new(owner_guid: String, registered: bool) -> Self {
        Self {
            owner_guid,
            registered,
        }
    }
    pub fn owner_guid(&self) -> &str {
        &self.owner_guid
    }
    pub fn registered(&self) -> bool {
        self.registered
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// All five native storage setters acknowledged their documents. The native
/// parser and mapping output can still report unsupported data separately.
pub struct PublicationReceipt {
    owner_guid: String,
    revision: u64,
    published: bool,
}
impl PublicationReceipt {
    pub fn new(owner_guid: String, revision: u64, published: bool) -> Self {
        Self {
            owner_guid,
            revision,
            published,
        }
    }
    pub fn owner_guid(&self) -> &str {
        &self.owner_guid
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn published(&self) -> bool {
        self.published
    }
}

/// Host createCbGlobalShortcutUnsupportedMappingEvent forwards native type 3.
/// The device, input and output arguments remain the original JSON strings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsupportedMapping {
    event_type: i32,
    device_info: String,
    input: String,
    output: String,
    time_tick: u64,
}
impl UnsupportedMapping {
    pub fn new(device_info: String, input: String, output: String, time_tick: u64) -> Self {
        Self {
            event_type: 3,
            device_info,
            input,
            output,
            time_tick,
        }
    }
    pub fn event_type(&self) -> i32 {
        self.event_type
    }
    pub fn device_info(&self) -> &str {
        &self.device_info
    }
    pub fn input(&self) -> &str {
        &self.input
    }
    pub fn output(&self) -> &str {
        &self.output
    }
    pub fn time_tick(&self) -> u64 {
        self.time_tick
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventBatch {
    owner_guid: String,
    events: Vec<UnsupportedMapping>,
}
impl EventBatch {
    pub fn new(owner_guid: String, events: Vec<UnsupportedMapping>) -> Self {
        Self { owner_guid, events }
    }
    pub fn owner_guid(&self) -> &str {
        &self.owner_guid
    }
    pub fn events(&self) -> &[UnsupportedMapping] {
        &self.events
    }
    pub fn into_events(self) -> Vec<UnsupportedMapping> {
        self.events
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StopReceipt {
    owner_guid: String,
    stopped: bool,
}
impl StopReceipt {
    pub fn new(owner_guid: String, stopped: bool) -> Self {
        Self {
            owner_guid,
            stopped,
        }
    }
    pub fn owner_guid(&self) -> &str {
        &self.owner_guid
    }
    pub fn stopped(&self) -> bool {
        self.stopped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn observed_shortcut_events_preserve_source_strings_and_timestamp() {
        let event =
            UnsupportedMapping::new("{\"productId\":70}".into(), "[]".into(), "{}".into(), 42);
        let expected = json!({"owner_guid":"owner", "events":[{
            "event_type":3,"device_info":"{\"productId\":70}","input":"[]","output":"{}","time_tick":42
        }]});
        let batch = EventBatch::new("owner".into(), vec![event]);
        assert_eq!(serde_json::to_value(&batch).unwrap(), expected);
        let decoded: EventBatch = serde_json::from_value(expected).unwrap();
        assert_eq!(decoded, batch);
    }

    #[test]
    fn shortcut_receipts_require_owner_and_complete_typed_observations() {
        for value in [
            json!({}),
            json!({"owner_guid":"owner"}),
            json!({"owner_guid":"owner","registered":"true"}),
        ] {
            assert!(serde_json::from_value::<RegistrationReceipt>(value).is_err());
        }
        for value in [
            json!({"owner_guid":"owner","published":true}),
            json!({"owner_guid":"owner","revision":null,"published":true}),
        ] {
            assert!(serde_json::from_value::<PublicationReceipt>(value).is_err());
        }
        for value in [
            json!({"events":[]}),
            json!({"owner_guid":"owner","events":[{"event_type":3}]}),
        ] {
            assert!(serde_json::from_value::<EventBatch>(value).is_err());
        }
        assert!(
            serde_json::from_value::<StopReceipt>(json!({"owner_guid":"owner","stopped":null}))
                .is_err()
        );
    }
}
