use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum InputCounter {
    Label(String),
    Number(u32),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KeyboardSourceInput {
    button_key: String,
    default_value: String,
    is_focused: bool,
    is_mapped: bool,
    is_enabled: bool,
    input_type: String,
    #[serde(rename = "inputID")]
    input_id: String,
    counter: InputCounter,
    #[serde(skip_serializing_if = "Option::is_none")]
    button_pos: Option<String>,
    assignment: String,
    assignment_value: String,
    #[serde(rename = "pageID", skip_serializing_if = "Option::is_none")]
    page_id: Option<String>,
    #[serde(rename = "HID", skip_serializing_if = "Option::is_none")]
    hid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    d: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    function_list: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    y: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    height: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cx: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cy: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    r: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_side_panel_list: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    display: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    children_key: Option<Vec<String>>,
    #[serde(rename = "MappingGroup", skip_serializing_if = "Option::is_none")]
    mapping_group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    disable_hypershift_mapping: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    disabled: Option<bool>,
}
impl KeyboardSourceInput {
    pub fn id(&self) -> &str {
        &self.input_id
    }
    pub fn button_key(&self) -> &str {
        &self.button_key
    }
    pub fn default_value(&self) -> &str {
        &self.default_value
    }
    pub fn enabled(&self) -> bool {
        self.is_enabled
    }
    pub fn input_type(&self) -> &str {
        &self.input_type
    }
    pub fn counter(&self) -> &InputCounter {
        &self.counter
    }
    pub fn assignment(&self) -> &str {
        &self.assignment
    }
    pub fn hid(&self) -> Option<&str> {
        self.hid.as_deref()
    }
    pub fn side_panel(&self) -> Option<bool> {
        self.is_side_panel_list
    }
    pub fn disabled(&self) -> bool {
        self.disabled.unwrap_or(false)
    }
    pub fn disable_hypershift_mapping(&self) -> bool {
        self.disable_hypershift_mapping.unwrap_or(false)
    }
    pub fn functions(&self) -> &[String] {
        self.function_list.as_deref().unwrap_or(&[])
    }
    pub fn has_geometry(&self) -> bool {
        self.d.is_some() || self.r.is_some() || self.x.is_some()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KeyboardSourceButtons {
    group_name: String,
    button_list: Vec<KeyboardSourceInput>,
}
impl KeyboardSourceButtons {
    pub fn inputs(&self) -> &[KeyboardSourceInput] {
        &self.button_list
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KeyboardSourceGroup {
    group: KeyboardSourceButtons,
}
impl KeyboardSourceGroup {
    pub fn inputs(&self) -> &[KeyboardSourceInput] {
        self.group.inputs()
    }
}

#[derive(Deserialize)]
struct KeyboardSourceLayout {
    layout_id: u32,
    groups: Vec<KeyboardSourceGroup>,
}

pub fn keyboard_source_for_layout(layout_id: u32) -> Option<&'static [KeyboardSourceGroup]> {
    static LAYOUTS: OnceLock<Vec<KeyboardSourceLayout>> = OnceLock::new();
    let layout_id = if layout_id == 0 { 1 } else { layout_id };
    LAYOUTS
        .get_or_init(|| crate::data::keyboard_sources!())
        .iter()
        .find(|layout| layout.layout_id == layout_id)
        .map(|layout| layout.groups.as_slice())
}
