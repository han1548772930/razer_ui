//! Complete physical identity and joined worker state for bounded HID commands.
use crate::{backend::HidNode, device_reads::DeviceReadTarget};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Target {
    Collection {
        node: HidNode,
        product_id: u32,
        peer_product_id: Option<u32>,
    },
    WindowsContainer(DeviceReadTarget),
}
impl Target {
    pub fn product_id(&self) -> u32 {
        match self {
            Self::Collection { product_id, .. } => *product_id,
            Self::WindowsContainer(target) => target.product_id,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Progress<S, R> {
    pub operation_id: String,
    pub target: Target,
    pub requested: S,
    pub terminal: bool,
    pub canceled: bool,
    pub report: Option<R>,
    pub error: Option<String>,
}
