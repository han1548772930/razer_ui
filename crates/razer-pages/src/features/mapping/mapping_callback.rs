//! Original unsupported-mapping event, retaining input, output and timer tick.

use serde_json::Value;

#[derive(Clone)]
pub struct MappingCallbackRequested {
    raw: Value,
    connection_generation: u64,
}

impl MappingCallbackRequested {
    pub(crate) fn new_scoped(raw: Value, connection_generation: u64) -> Self {
        Self {
            raw,
            connection_generation,
        }
    }

    pub(crate) fn matches_generation(&self, generation: u64) -> bool {
        self.connection_generation == generation
    }

    pub fn raw(&self) -> &Value {
        &self.raw
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping_callback_does_not_acquire_a_replacement_connection_while_delivery_is_queued() {
        let raw = serde_json::json!({"eventName":"unsupportedMapping","output":"{\"type\":\"lightpac\",\"id\":\"local-b\"}"});
        let queued = MappingCallbackRequested::new_scoped(raw.clone(), 41);
        assert!(queued.matches_generation(41));
        assert!(!queued.matches_generation(42));
        assert_eq!(queued.raw(), &raw);
    }
}
