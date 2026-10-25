//! Hardware-observed PID747 controller values in thousandth units.
pub use crate::mapping_observation::{Receipt, Target};
use razer_device::keyboard_keypad_observation::Sample;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Delivery {
    sample: Sample,
    time_tick: u64,
}
impl Delivery {
    pub fn new(sample: Sample, time_tick: u64) -> Self {
        Self { sample, time_tick }
    }
    pub fn sample(&self) -> &Sample {
        &self.sample
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
            "Keypad observer identity changed"
        );
        Ok(self.deliveries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn keypad_wire_requires_complete_thousandth_sample_and_current_observer() {
        let wire = json!({"sample":{"left_thumb_x":-1000,"left_thumb_y":1000,
            "right_thumb_x":-23,"right_thumb_y":47,"left_trigger":1000,
            "right_trigger":0},"time_tick":41});
        let delivery: Delivery = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(delivery.sample().right_thumb_y(), 47);
        assert_eq!(delivery.time_tick(), 41);
        for field in [
            "left_thumb_x",
            "left_thumb_y",
            "right_thumb_x",
            "right_thumb_y",
            "left_trigger",
            "right_trigger",
        ] {
            let mut missing = wire.clone();
            missing["sample"].as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<Delivery>(missing).is_err());
            let mut out_of_range = wire.clone();
            out_of_range["sample"][field] = json!(1001);
            assert!(serde_json::from_value::<Delivery>(out_of_range).is_err());
        }
        assert!(
            Batch::new("old".into(), vec![delivery])
                .into_deliveries("new")
                .is_err()
        );
    }
}
