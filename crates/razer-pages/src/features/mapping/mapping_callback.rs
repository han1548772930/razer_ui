//! Original unsupported-mapping event, retaining input, output and timer tick.

use serde_json::Value;

/// A saved logical document offered to its retained physical receiver owner.
/// The owner reports the corresponding publication result before UI acceptance.
#[derive(Clone)]
pub struct ReceiverMappingPublication {
    owner: uuid::Uuid,
    product_id: u32,
    revision: u64,
    document: Value,
}

impl ReceiverMappingPublication {
    pub(crate) fn new(owner: uuid::Uuid, product_id: u32, revision: u64, document: Value) -> Self {
        Self { owner, product_id, revision, document }
    }
    pub fn owner(&self) -> uuid::Uuid { self.owner }
    pub fn product_id(&self) -> u32 { self.product_id }
    pub fn revision(&self) -> u64 { self.revision }
    pub fn document(&self) -> &Value { &self.document }
}

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
