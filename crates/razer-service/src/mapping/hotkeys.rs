//! Typed hotkey registrations and events owned by one shared mapping scope.
use anyhow::{Result, ensure};
use razer_ipc::ServiceRequest;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Shortcut {
    #[serde(rename = "virtualKey")]
    pub virtual_key: u32,
    pub modifiers: u32,
    pub argument: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub shortcut: Shortcut,
    pub time_tick: u64,
}

pub struct Session(super::Client);
impl Session {
    /// Acquires a scope on the background caller; it does not create another
    /// OS hotkey listener or drain events belonging to other scopes.
    pub fn start() -> Result<Self> { Ok(Self(super::Client::spawn()?)) }
    pub fn register(&mut self, shortcut: Shortcut) -> Result<()> {
        register(&mut self.0, shortcut)
    }
    pub fn unregister(&mut self, key: u32, modifiers: u32) -> Result<()> {
        unregister(&mut self.0, key, modifiers)
    }
    pub fn registered(&mut self) -> Result<Vec<Shortcut>> {
        Ok(serde_json::from_value(self.0.request(ServiceRequest::GlobalShortcuts)?)?)
    }
    pub fn enable(&mut self, enabled: bool) -> Result<()> { enable(&mut self.0, enabled) }
    pub fn drain(&mut self) -> Result<Vec<Event>> { drain(&mut self.0) }
    pub fn close(&mut self) -> Result<()> { super::shutdown(&mut self.0).map_err(anyhow::Error::msg) }
}

#[derive(Deserialize)]
struct Registration {
    registered: bool,
    vkey_code: u32,
    modifiers: u32,
}
fn registration(value: serde_json::Value, key: u32, modifiers: u32, expected: bool) -> Result<()> {
    let receipt: Registration = serde_json::from_value(value)?;
    ensure!(receipt.vkey_code == key && receipt.modifiers == modifiers,
        "Hotkey receipt belongs to another chord");
    ensure!(receipt.registered == expected, "Hotkey registration was not confirmed");
    Ok(())
}
fn register(client: &mut impl super::Transport, shortcut: Shortcut) -> Result<()> {
    let key = shortcut.virtual_key;
    let modifiers = shortcut.modifiers;
    registration(client.request(ServiceRequest::RegisterShortcut {
        vkey_code: key, modifiers, argument: shortcut.argument,
    })?, key, modifiers, true)
}
fn unregister(client: &mut impl super::Transport, key: u32, modifiers: u32) -> Result<()> {
    registration(client.request(ServiceRequest::UnregisterShortcut {
        vkey_code: key, modifiers,
    })?, key, modifiers, false)
}
fn enable(client: &mut impl super::Transport, enabled: bool) -> Result<()> {
    #[derive(Deserialize)]
    struct Receipt { enabled: bool }
    let receipt: Receipt = serde_json::from_value(client.request(
        ServiceRequest::EnableGlobalShortcuts { enable: enabled })?)?;
    ensure!(receipt.enabled == enabled, "Hotkey execution mode was not confirmed");
    Ok(())
}
fn drain(client: &mut impl super::Transport) -> Result<Vec<Event>> {
    #[derive(Deserialize)]
    struct Envelope {
        #[serde(rename = "eventType")]
        event_type: u32,
        event: String,
        #[serde(rename = "timeTick")]
        time_tick: u64,
    }
    let events: Vec<Envelope> = serde_json::from_value(client.request(ServiceRequest::ShortcutEvents)?)?;
    events.into_iter().map(|event| {
        ensure!(event.event_type == 0, "Unknown hotkey event type {}", event.event_type);
        Ok(Event { shortcut: serde_json::from_str(&event.event)?, time_tick: event.time_tick })
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    struct Reply(Value);
    impl crate::mapping::Transport for Reply {
        fn request(&mut self, _: ServiceRequest) -> Result<Value> { Ok(self.0.clone()) }
        fn is_stopped(&self) -> bool { false }
    }
    #[test]
    fn receipts_reject_false_missing_and_other_chord_results() {
        for value in [json!({}), json!({"registered":false,"vkey_code":65,"modifiers":32}),
            json!({"registered":true,"vkey_code":66,"modifiers":32})] {
            assert!(register(&mut Reply(value), Shortcut { virtual_key:65, modifiers:32, argument:"2".into() }).is_err());
        }
        assert!(registration(json!({"registered":true,"vkey_code":65,"modifiers":32}),65,32,true).is_ok());
        assert!(unregister(&mut Reply(json!({"registered":true,"vkey_code":65,"modifiers":32})),65,32).is_err());
        assert!(enable(&mut Reply(json!({"enabled":false})),true).is_err());
    }
    #[test]
    fn callbacks_keep_source_tick_and_reject_malformed_or_unknown_fields() {
        let payload = json!({"virtualKey":65,"modifiers":32,"argument":"2"}).to_string();
        let events = drain(&mut Reply(json!([{"eventType":0,"event":payload,"timeTick":317}]))).unwrap();
        assert_eq!(events[0].time_tick,317);
        assert_eq!(events[0].shortcut.argument,"2");
        assert_eq!(events[0].shortcut.modifiers,32);
        assert!(drain(&mut Reply(json!([{"eventType":9,"event":payload,"timeTick":317}]))).is_err());
        assert!(drain(&mut Reply(json!([{"eventType":0,"event":"{}","timeTick":317}]))).is_err());
        assert!(drain(&mut Reply(json!([{"eventType":0,"event":payload}]))).is_err());
    }
}
