//! Typed snapshots of the mapping engine's analog callback stream.
pub use crate::mapping_observation::{Receipt, Target};
use razer_device::{mapping_native_action::Output, mapping_native_lookup::Input};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Registration {
    All { skip: u32 },
    Key { input: Input, skip: u32 },
    Interval { milliseconds: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalogEvent {
    input: Input,
    key_id: u8,
    level: u16,
    output: Option<Output>,
    time_tick: u64,
}
impl AnalogEvent {
    pub fn new(
        input: Input,
        key_id: u8,
        level: u16,
        output: Option<Output>,
        time_tick: u64,
    ) -> Self {
        Self {
            input,
            key_id,
            level,
            output,
            time_tick,
        }
    }
    pub fn input(&self) -> &Input {
        &self.input
    }
    pub fn key_id(&self) -> u8 {
        self.key_id
    }
    pub fn level(&self) -> u16 {
        self.level
    }
    pub fn output(&self) -> Option<&Output> {
        self.output.as_ref()
    }
    pub fn time_tick(&self) -> u64 {
        self.time_tick
    }
}

/// One native-equivalent callback. Interval callbacks retain every event's
/// timestamp separately from the time at which the batch was delivered.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Delivery {
    events: Vec<AnalogEvent>,
    time_tick: u64,
}
impl Delivery {
    pub fn new(events: Vec<AnalogEvent>, time_tick: u64) -> Self {
        Self { events, time_tick }
    }
    pub fn events(&self) -> &[AnalogEvent] {
        &self.events
    }
    pub fn into_events(self) -> Vec<AnalogEvent> {
        self.events
    }
    pub fn time_tick(&self) -> u64 {
        self.time_tick
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Batch {
    observer_guid: String,
    deliveries: Vec<Delivery>,
}
impl Batch {
    pub fn new(observer_guid: String, deliveries: Vec<Delivery>) -> Self {
        Self {
            observer_guid,
            deliveries,
        }
    }
    pub fn into_deliveries(self, observer: &str) -> anyhow::Result<Vec<Delivery>> {
        anyhow::ensure!(
            self.observer_guid == observer,
            "Analog observer identity changed"
        );
        Ok(self.deliveries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use razer_device::mapping_native_lookup::InputKind;
    #[test]
    fn observed_state_and_full_output_cross_the_boundary() {
        let event = AnalogEvent::new(
            Input::new(InputKind::AnalogKey {
                scancode: 30,
                flag: 1,
            }),
            4,
            45864,
            Some(Output::Macro {
                guid: "macro-a".into(),
                flag: 1,
                repeat: 3,
            }),
            123,
        );
        let value = serde_json::to_value(event.clone()).unwrap();
        assert_eq!(value["key_id"], 4);
        assert_eq!(value["level"], 45864);
        assert_eq!(value["output"]["Macro"]["flag"], 1);
        assert_eq!(
            serde_json::from_value::<AnalogEvent>(value.clone()).unwrap(),
            event
        );
        for field in ["input", "key_id", "level", "time_tick"] {
            let mut missing = value.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<AnalogEvent>(missing).is_err());
        }
        assert!(
            Batch::new("old".into(), vec![])
                .into_deliveries("new")
                .is_err()
        );
    }
}
