//! Input and device-mode callbacks retain their shared publication order.
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum Event {
    Input(Value),
    Mode(bool),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Batch {
    events: Vec<Event>,
}
impl Batch {
    pub fn new(events: Vec<Event>) -> Self {
        Self { events }
    }
    pub fn into_events(self) -> Vec<Event> {
        self.events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn mixed_callbacks_have_an_explicit_ordered_wire_contract() {
        let batch = Batch::new(vec![
            Event::Mode(true),
            Event::Input(json!({"input":{"key":65},"timeTick":1})),
            Event::Mode(false),
        ]);
        let expected = json!({"events":[
            {"kind":"mode","payload":true},
            {"kind":"input","payload":{"input":{"key":65},"timeTick":1}},
            {"kind":"mode","payload":false},
        ]});
        assert_eq!(serde_json::to_value(&batch).unwrap(), expected);
        assert_eq!(serde_json::from_value::<Batch>(expected).unwrap(), batch);
        for invalid in [
            json!({}),
            json!({"events":null}),
            json!({"events":[{"kind":"mode","payload":"false"}]}),
            json!({"events":[{"kind":"mode"}]}),
            json!({"events":[{"kind":"unknown","payload":false}]}),
        ] {
            assert!(serde_json::from_value::<Batch>(invalid).is_err());
        }
    }
}
