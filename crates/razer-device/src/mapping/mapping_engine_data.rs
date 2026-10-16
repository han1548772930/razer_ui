//! Typed literals decoded from the saved current-source audit.
//! JSON remains an audit/test input; normal mapping generation never parses it.
//! The maintained source audit is `tools/workspace.py audit-family-function`
//! with `--family keyboard --function mw --product 756 --query mapping-engine`.

use serde::Serialize;

/// Full source metadata, including numeric-looking fields that are strings.
/// Optional fields represent absence; the generator rejects unexpected nulls.
#[derive(Clone, Copy, Debug, Serialize)]
pub(crate) struct KeyDefinition {
    pub name: &'static str,
    #[serde(rename = "inputID", skip_serializing_if = "Option::is_none")]
    pub input_id: Option<&'static str>,
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub scancode: u32,
    #[serde(rename = "analogKeyID")]
    pub analog_key_id: u32,
    #[serde(rename = "virtualKey")]
    pub virtual_key: &'static str,
    #[serde(rename = "keyCode")]
    pub key_code: &'static str,
    pub flag: u32,
    #[serde(rename = "pageID", skip_serializing_if = "Option::is_none")]
    pub page_id: Option<&'static str>,
    #[serde(rename = "HID", skip_serializing_if = "Option::is_none")]
    pub hid: Option<&'static str>,
    #[serde(rename = "ExtendedKey", skip_serializing_if = "Option::is_none")]
    pub extended_key: Option<&'static str>,
    #[serde(rename = "outputFlag", skip_serializing_if = "Option::is_none")]
    pub output_flag: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modifier: Option<&'static str>,
}

/// An event's type selects its fields and preserves numeric vs. string IDs.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(super) enum TurboEvent {
    Keyboard { scancode: u32, flag: u32 },
    Mouse { id: u32, data: u32 },
    Delay { ms: u32 },
    Audio { id: &'static str },
    Display { id: &'static str },
}

/// The default ID and its resolved event sequence cannot drift apart.
#[derive(Clone, Copy, Debug)]
pub(super) struct DefaultTurbo {
    pub id: &'static str,
    pub events: &'static [TurboEvent],
}

#[derive(Debug)]
pub(super) struct EngineData {
    pub keys: &'static [KeyDefinition],
    /// Original generateDefaultTurbos order, which also determines UUID usage.
    pub default_turbos: &'static [DefaultTurbo],
    pub single_state_input_ids: &'static [&'static str],
    pub scroll_input_ids: &'static [&'static str],
    pub scroll_turbo_delay_ms: u32,
}

pub(super) struct ProductData {
    pub product_id: u32,
    pub data: &'static EngineData,
}

pub(super) fn for_product(product_id: u32) -> Option<&'static EngineData> {
    PRODUCTS
        .iter()
        .find(|product| product.product_id == product_id)
        .map(|product| product.data)
}

include!("mapping_engine_data_generated.rs");
