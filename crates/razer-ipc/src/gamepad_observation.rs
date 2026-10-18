//! Computed mapping output, independent of driver completion or device readback.
pub use crate::mapping_observation::{Receipt, Target};
use razer_device::mapping_native_output::GamepadOutputState;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Registration {
    Events { skip: u32 },
    Interval { milliseconds: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Delivery {
    events: Vec<GamepadOutputState>,
    time_tick: u64,
}
impl Delivery {
    pub fn new(events: Vec<GamepadOutputState>, time_tick: u64) -> Self {
        Self { events, time_tick }
    }
    pub fn events(&self) -> &[GamepadOutputState] {
        &self.events
    }
    pub fn into_events(self) -> Vec<GamepadOutputState> {
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
            "Gamepad observer identity changed"
        );
        Ok(self.deliveries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn samples_require_complete_typed_state_and_matching_observer() {
        let state = GamepadOutputState {
            buttons: 0x1020,
            left_trigger: 255,
            right_trigger: 1,
            left_thumb: [i16::MIN, i16::MAX],
            right_thumb: [-1, 23],
        };
        let delivery = Delivery::new(vec![state], 42);
        let value = serde_json::to_value(&delivery).unwrap();
        assert_eq!(
            serde_json::from_value::<Delivery>(value.clone()).unwrap(),
            delivery
        );
        for field in [
            "buttons",
            "left_trigger",
            "right_trigger",
            "left_thumb",
            "right_thumb",
        ] {
            let mut incomplete = value.clone();
            incomplete["events"][0]
                .as_object_mut()
                .unwrap()
                .remove(field);
            assert!(serde_json::from_value::<Delivery>(incomplete).is_err());
        }
        let mut invalid = value;
        invalid["events"][0]["left_trigger"] = serde_json::json!(256);
        assert!(serde_json::from_value::<Delivery>(invalid).is_err());
        assert!(
            Batch::new("old".into(), vec![delivery])
                .into_deliveries("new")
                .is_err()
        );
        assert!(
            Batch::new("current".into(), vec![])
                .into_deliveries("current")
                .unwrap()
                .is_empty()
        );
    }
}
