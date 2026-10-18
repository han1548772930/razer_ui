//! Original unsupported-mapping event, retaining input, output and timer tick.

use serde_json::Value;

#[derive(Clone)]
pub struct MappingCallbackRequested {
    raw: Value,
}

impl MappingCallbackRequested {
    pub(crate) fn new(raw: Value) -> Self {
        Self { raw }
    }

    pub fn raw(&self) -> &Value {
        &self.raw
    }
}
