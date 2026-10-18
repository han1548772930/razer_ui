//! Local Studio editor documents. Device enumeration and native engine receipts
//! have separate owners and are never reconstructed from this editor document.
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Layer {
    pub id: u64,
    pub name: String,
    pub title: String,
    pub value: u32,
    pub params: Value,
    #[serde(default = "empty_params")]
    pub params2: Value,
    pub paint_params: Value,
    pub visible: bool,
    pub group: bool,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}
fn empty_params() -> Value {
    serde_json::json!({})
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Document {
    pub layers: Vec<Layer>,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}
