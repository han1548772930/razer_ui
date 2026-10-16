//! Current RzAudioUtil AudioEnumerator notification semantics, without COM.
//! IDA: CRzAudioEnumerator callback 0x3bd0 and event formatter 0x3360.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndpointChange {
    Removed,
    Added,
    DefaultChanged,
    StateChanged,
    PropertyChanged,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioNotification {
    pub event: String,
    #[serde(rename = "eventType")]
    pub event_type: String,
    #[serde(rename = "endpointId")]
    pub endpoint_id: String,
}

#[derive(Default)]
pub struct NotificationQueue {
    subscriptions: usize,
    events: VecDeque<AudioNotification>,
}

impl NotificationQueue {
    pub fn subscriptions(&self) -> usize {
        self.subscriptions
    }

    /// Each native enable appends the same callback type. Only the first one
    /// registers the COM listener; later enables duplicate event delivery.
    pub fn enabled(&mut self) -> anyhow::Result<()> {
        self.subscriptions = self
            .subscriptions
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("AudioEnumerator subscription count overflow"))?;
        Ok(())
    }

    /// Native disable removes every callback of this type, rather than one.
    pub fn disabled(&mut self) {
        self.subscriptions = 0;
    }

    pub fn observe(&mut self, change: EndpointChange, endpoint_id: &str) {
        // 0x459b0/45a60/45b10 reject empty/null IDs. 0x3bd0 deliberately
        // ignores the native default and property-change notifications.
        if endpoint_id.is_empty()
            || !matches!(
                change,
                EndpointChange::Removed | EndpointChange::Added | EndpointChange::StateChanged
            )
        {
            return;
        }
        for _ in 0..self.subscriptions {
            self.events.push_back(AudioNotification {
                event: "RzAudioUtilEvent".into(),
                event_type: "AudioEnumerator_DeviceChange".into(),
                endpoint_id: endpoint_id.into(),
            });
        }
    }

    pub fn drain(&mut self) -> Vec<AudioNotification> {
        self.events.drain(..).collect()
    }
}

/// Formatter uses an integer (JSON tag 5), although the wrapper sends bool.
pub fn source_enable_response(enabled: bool) -> Value {
    json!({"response": {"enabled": i32::from(enabled)}})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_filter_payload_and_duplicate_callback_semantics() {
        let mut queue = NotificationQueue::default();
        queue.observe(EndpointChange::Added, "before");
        queue.enabled().unwrap();
        queue.enabled().unwrap();
        for change in [
            EndpointChange::DefaultChanged,
            EndpointChange::PropertyChanged,
        ] {
            queue.observe(change, "ignored");
        }
        queue.observe(EndpointChange::Added, "");
        for change in [
            EndpointChange::Removed,
            EndpointChange::Added,
            EndpointChange::StateChanged,
        ] {
            queue.observe(change, "{original-id}");
        }
        let events = queue.drain();
        assert_eq!(events.len(), 6);
        assert_eq!(
            serde_json::to_value(&events[0]).unwrap(),
            json!({
                "event":"RzAudioUtilEvent", "eventType":"AudioEnumerator_DeviceChange", "endpointId":"{original-id}"
            })
        );
        queue.disabled();
        queue.observe(EndpointChange::Added, "after");
        assert_eq!(queue.subscriptions(), 0);
        assert!(queue.drain().is_empty());
        assert_eq!(
            source_enable_response(true),
            json!({"response":{"enabled":1}})
        );
        assert_eq!(
            source_enable_response(false),
            json!({"response":{"enabled":0}})
        );
    }
}
