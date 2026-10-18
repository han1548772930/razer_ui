//! Device-scoped mapping observations, separate from input notifications.
use razer_device::mapping_native_lookup::{Input, parse_input};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Target {
    product_id: u32,
    container_id: String,
}
impl Target {
    pub fn new(product_id: u32, container_id: String) -> Self {
        Self {
            product_id,
            container_id,
        }
    }
    pub fn product_id(&self) -> u32 {
        self.product_id
    }
    pub fn container_id(&self) -> &str {
        &self.container_id
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapTapEvent {
    #[serde(
        serialize_with = "serialize_input",
        deserialize_with = "deserialize_input"
    )]
    input: Input,
    snaptap: bool,
    #[serde(rename = "timeTick")]
    time_tick: u64,
}
impl SnapTapEvent {
    pub fn new(input: Input, snaptap: bool, time_tick: u64) -> Self {
        Self {
            input,
            snaptap,
            time_tick,
        }
    }
    pub fn input(&self) -> &Input {
        &self.input
    }
    pub fn replayed(&self) -> bool {
        self.snaptap
    }
    pub fn time_tick(&self) -> u64 {
        self.time_tick
    }
}
pub(crate) fn serialize_input<S: Serializer>(
    input: &Input,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    input.to_native_json().serialize(serializer)
}
pub(crate) fn deserialize_input<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Input, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    parse_input(&value).map_err(serde::de::Error::custom)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Receipt {
    observer_guid: String,
    registered: bool,
}
impl Receipt {
    pub fn new(observer_guid: String, registered: bool) -> Self {
        Self {
            observer_guid,
            registered,
        }
    }
    pub fn matches(&self, observer: &str, registered: bool) -> bool {
        self.observer_guid == observer && self.registered == registered
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Batch<E = SnapTapEvent> {
    observer_guid: String,
    events: Vec<E>,
}
impl<E> Batch<E> {
    pub fn new(observer_guid: String, events: Vec<E>) -> Self {
        Self {
            observer_guid,
            events,
        }
    }
    pub fn into_events(self, observer: &str) -> anyhow::Result<Vec<E>> {
        anyhow::ensure!(
            self.observer_guid == observer,
            "Mapping observer identity changed"
        );
        Ok(self.events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn snaptap_event_preserves_native_input_and_requires_observation_fields() {
        let source = json!({"input":{"type":"analogKey","scancode":30,"flag":3,"modifiers":9},"snaptap":true,"timeTick":123});
        let event: SnapTapEvent = serde_json::from_value(source.clone()).unwrap();
        assert!(event.replayed());
        assert_eq!(event.time_tick(), 123);
        assert_eq!(serde_json::to_value(event).unwrap(), source);
        for field in ["input", "snaptap", "timeTick"] {
            let mut missing = source.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<SnapTapEvent>(missing).is_err());
        }
        assert!(
            Batch::<SnapTapEvent>::new("old".into(), vec![])
                .into_events("new")
                .is_err()
        );
    }
}
