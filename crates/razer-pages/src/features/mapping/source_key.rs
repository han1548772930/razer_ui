//! The complete source key record shared by focused keyboard capture tables.
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SourceKey {
    pub(crate) name: String,
    #[serde(rename = "inputID", skip_serializing_if = "Option::is_none")]
    pub(crate) input_id: Option<String>,
    #[serde(rename = "type")]
    pub(crate) kind: String,
    pub(crate) scancode: u64,
    pub(crate) virtual_key: String,
    pub(crate) key_code: String,
    pub(crate) flag: u64,
    #[serde(rename = "pageID", skip_serializing_if = "Option::is_none")]
    pub(crate) page_id: Option<String>,
    #[serde(rename = "HID", skip_serializing_if = "Option::is_none")]
    pub(crate) hid: Option<String>,
    #[serde(rename = "ExtendedKey", skip_serializing_if = "Option::is_none")]
    pub(crate) extended_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) modifier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) output_flag: Option<u64>,
}
