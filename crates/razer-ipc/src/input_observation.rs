//! Native type-1 input notifications consumed by rzInputEvents.
pub use crate::mapping_observation::{Receipt, Target};
use crate::mapping_observation::{deserialize_input, serialize_input};
use razer_device::mapping_native_lookup::Input;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputEvent {
    #[serde(
        serialize_with = "serialize_input",
        deserialize_with = "deserialize_input"
    )]
    input: Input,
    #[serde(rename = "timeTick")]
    time_tick: u64,
}
impl InputEvent {
    pub fn new(input: Input, time_tick: u64) -> Self {
        Self { input, time_tick }
    }
    pub fn input(&self) -> &Input {
        &self.input
    }
    pub fn time_tick(&self) -> u64 {
        self.time_tick
    }
}
pub type Batch = crate::mapping_observation::Batch<InputEvent>;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn input_observation_keeps_extended_flags_and_rejects_missing_or_foreign_data() {
        // 10CD90 emits extraInformation even when 10CBC4 initialized it to zero.
        let source = json!({"input":{"type":"keyboard","scancode":56,"flag":3,"extraInformation":0,"modifiers":9},"timeTick":931});
        let event: InputEvent = serde_json::from_value(source.clone()).unwrap();
        assert_eq!(serde_json::to_value(&event).unwrap(), source);
        assert_eq!(event.time_tick(), 931);
        for field in ["input", "timeTick"] {
            let mut missing = source.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<InputEvent>(missing).is_err());
        }
        assert!(
            Batch::new("old".into(), vec![event])
                .into_events("current")
                .is_err()
        );
    }
}
