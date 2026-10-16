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
        .get_or_init(|| read_keyboard_sources_catalog())
        .iter()
        .find(|layout| layout.layout_id == layout_id)
        .map(|layout| layout.groups.as_slice())
}

// Generated catalog reader: razer-assets/keyboard_sources.
fn read_keyboard_sources_catalog() -> Vec<KeyboardSourceLayout> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<KeyboardSourceLayout>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<KeyboardSourceLayout> {
        let value = match r.tag()? {
            0 => KeyboardSourceLayout {
                layout_id: r.child(decode_2)?,
                groups: r.child(decode_3)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<KeyboardSourceGroup>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_4)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<KeyboardSourceGroup> {
        let value = match r.tag()? {
            0 => KeyboardSourceGroup {
                group: r.child(decode_5)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<KeyboardSourceButtons> {
        let value = match r.tag()? {
            0 => KeyboardSourceButtons {
                group_name: r.child(decode_6)?,
                button_list: r.child(decode_7)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<KeyboardSourceInput>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_8)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_8(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<KeyboardSourceInput> {
        let value = match r.tag()? {
            0 => KeyboardSourceInput {
                button_key: r.child(decode_6)?,
                default_value: r.child(decode_6)?,
                is_focused: r.child(decode_9)?,
                is_mapped: r.child(decode_9)?,
                is_enabled: r.child(decode_9)?,
                input_type: r.child(decode_6)?,
                input_id: r.child(decode_6)?,
                counter: r.child(decode_10)?,
                button_pos: r.child(decode_11)?,
                assignment: r.child(decode_6)?,
                assignment_value: r.child(decode_6)?,
                page_id: r.child(decode_11)?,
                hid: r.child(decode_11)?,
                d: r.child(decode_11)?,
                function_list: r.child(decode_12)?,
                x: None,
                y: None,
                width: None,
                height: None,
                cx: None,
                cy: None,
                r: None,
                is_side_panel_list: None,
                display: None,
                children_key: None,
                mapping_group: None,
                disable_hypershift_mapping: None,
                disabled: None,
            },
            1 => KeyboardSourceInput {
                button_key: r.child(decode_6)?,
                default_value: r.child(decode_6)?,
                is_focused: r.child(decode_9)?,
                is_mapped: r.child(decode_9)?,
                is_enabled: r.child(decode_9)?,
                input_type: r.child(decode_6)?,
                input_id: r.child(decode_6)?,
                counter: r.child(decode_10)?,
                button_pos: r.child(decode_11)?,
                assignment: r.child(decode_6)?,
                assignment_value: r.child(decode_6)?,
                page_id: r.child(decode_11)?,
                hid: r.child(decode_11)?,
                d: None,
                function_list: r.child(decode_12)?,
                x: r.child(decode_14)?,
                y: r.child(decode_14)?,
                width: r.child(decode_14)?,
                height: r.child(decode_14)?,
                cx: None,
                cy: None,
                r: None,
                is_side_panel_list: None,
                display: None,
                children_key: None,
                mapping_group: None,
                disable_hypershift_mapping: None,
                disabled: None,
            },
            2 => KeyboardSourceInput {
                button_key: r.child(decode_6)?,
                default_value: r.child(decode_6)?,
                is_focused: r.child(decode_9)?,
                is_mapped: r.child(decode_9)?,
                is_enabled: r.child(decode_9)?,
                input_type: r.child(decode_6)?,
                input_id: r.child(decode_6)?,
                counter: r.child(decode_10)?,
                button_pos: None,
                assignment: r.child(decode_6)?,
                assignment_value: r.child(decode_6)?,
                page_id: None,
                hid: None,
                d: None,
                function_list: None,
                x: None,
                y: None,
                width: None,
                height: None,
                cx: r.child(decode_14)?,
                cy: r.child(decode_14)?,
                r: r.child(decode_14)?,
                is_side_panel_list: r.child(decode_16)?,
                display: None,
                children_key: None,
                mapping_group: None,
                disable_hypershift_mapping: None,
                disabled: None,
            },
            3 => KeyboardSourceInput {
                button_key: r.child(decode_6)?,
                default_value: r.child(decode_6)?,
                is_focused: r.child(decode_9)?,
                is_mapped: r.child(decode_9)?,
                is_enabled: r.child(decode_9)?,
                input_type: r.child(decode_6)?,
                input_id: r.child(decode_6)?,
                counter: r.child(decode_10)?,
                button_pos: None,
                assignment: r.child(decode_6)?,
                assignment_value: r.child(decode_6)?,
                page_id: None,
                hid: None,
                d: r.child(decode_11)?,
                function_list: r.child(decode_12)?,
                x: None,
                y: None,
                width: None,
                height: None,
                cx: None,
                cy: None,
                r: None,
                is_side_panel_list: None,
                display: None,
                children_key: None,
                mapping_group: None,
                disable_hypershift_mapping: None,
                disabled: None,
            },
            4 => KeyboardSourceInput {
                button_key: r.child(decode_6)?,
                default_value: r.child(decode_6)?,
                is_focused: r.child(decode_9)?,
                is_mapped: r.child(decode_9)?,
                is_enabled: r.child(decode_9)?,
                input_type: r.child(decode_6)?,
                input_id: r.child(decode_6)?,
                counter: r.child(decode_10)?,
                button_pos: None,
                assignment: r.child(decode_6)?,
                assignment_value: r.child(decode_6)?,
                page_id: None,
                hid: None,
                d: None,
                function_list: r.child(decode_12)?,
                x: None,
                y: None,
                width: None,
                height: None,
                cx: r.child(decode_14)?,
                cy: r.child(decode_14)?,
                r: r.child(decode_14)?,
                is_side_panel_list: None,
                display: None,
                children_key: None,
                mapping_group: None,
                disable_hypershift_mapping: None,
                disabled: None,
            },
            5 => KeyboardSourceInput {
                button_key: r.child(decode_6)?,
                default_value: r.child(decode_6)?,
                is_focused: r.child(decode_9)?,
                is_mapped: r.child(decode_9)?,
                is_enabled: r.child(decode_9)?,
                input_type: r.child(decode_6)?,
                input_id: r.child(decode_6)?,
                counter: r.child(decode_10)?,
                button_pos: None,
                assignment: r.child(decode_6)?,
                assignment_value: r.child(decode_6)?,
                page_id: None,
                hid: None,
                d: None,
                function_list: r.child(decode_12)?,
                x: None,
                y: None,
                width: None,
                height: None,
                cx: None,
                cy: None,
                r: None,
                is_side_panel_list: None,
                display: r.child(decode_11)?,
                children_key: None,
                mapping_group: None,
                disable_hypershift_mapping: None,
                disabled: None,
            },
            6 => KeyboardSourceInput {
                button_key: r.child(decode_6)?,
                default_value: r.child(decode_6)?,
                is_focused: r.child(decode_9)?,
                is_mapped: r.child(decode_9)?,
                is_enabled: r.child(decode_9)?,
                input_type: r.child(decode_6)?,
                input_id: r.child(decode_6)?,
                counter: r.child(decode_10)?,
                button_pos: None,
                assignment: r.child(decode_6)?,
                assignment_value: r.child(decode_6)?,
                page_id: r.child(decode_11)?,
                hid: r.child(decode_11)?,
                d: r.child(decode_11)?,
                function_list: None,
                x: None,
                y: None,
                width: None,
                height: None,
                cx: None,
                cy: None,
                r: None,
                is_side_panel_list: r.child(decode_16)?,
                display: None,
                children_key: r.child(decode_12)?,
                mapping_group: None,
                disable_hypershift_mapping: None,
                disabled: None,
            },
            7 => KeyboardSourceInput {
                button_key: r.child(decode_6)?,
                default_value: r.child(decode_6)?,
                is_focused: r.child(decode_9)?,
                is_mapped: r.child(decode_9)?,
                is_enabled: r.child(decode_9)?,
                input_type: r.child(decode_6)?,
                input_id: r.child(decode_6)?,
                counter: r.child(decode_10)?,
                button_pos: r.child(decode_11)?,
                assignment: r.child(decode_6)?,
                assignment_value: r.child(decode_6)?,
                page_id: r.child(decode_11)?,
                hid: r.child(decode_11)?,
                d: r.child(decode_11)?,
                function_list: r.child(decode_12)?,
                x: None,
                y: None,
                width: None,
                height: None,
                cx: None,
                cy: None,
                r: None,
                is_side_panel_list: None,
                display: None,
                children_key: None,
                mapping_group: r.child(decode_11)?,
                disable_hypershift_mapping: r.child(decode_16)?,
                disabled: None,
            },
            8 => KeyboardSourceInput {
                button_key: r.child(decode_6)?,
                default_value: r.child(decode_6)?,
                is_focused: r.child(decode_9)?,
                is_mapped: r.child(decode_9)?,
                is_enabled: r.child(decode_9)?,
                input_type: r.child(decode_6)?,
                input_id: r.child(decode_6)?,
                counter: r.child(decode_10)?,
                button_pos: r.child(decode_11)?,
                assignment: r.child(decode_6)?,
                assignment_value: r.child(decode_6)?,
                page_id: None,
                hid: None,
                d: r.child(decode_11)?,
                function_list: r.child(decode_12)?,
                x: None,
                y: None,
                width: None,
                height: None,
                cx: None,
                cy: None,
                r: None,
                is_side_panel_list: r.child(decode_16)?,
                display: None,
                children_key: None,
                mapping_group: r.child(decode_11)?,
                disable_hypershift_mapping: None,
                disabled: None,
            },
            9 => KeyboardSourceInput {
                button_key: r.child(decode_6)?,
                default_value: r.child(decode_6)?,
                is_focused: r.child(decode_9)?,
                is_mapped: r.child(decode_9)?,
                is_enabled: r.child(decode_9)?,
                input_type: r.child(decode_6)?,
                input_id: r.child(decode_6)?,
                counter: r.child(decode_10)?,
                button_pos: None,
                assignment: r.child(decode_6)?,
                assignment_value: r.child(decode_6)?,
                page_id: None,
                hid: None,
                d: None,
                function_list: r.child(decode_12)?,
                x: None,
                y: None,
                width: None,
                height: None,
                cx: None,
                cy: None,
                r: None,
                is_side_panel_list: None,
                display: None,
                children_key: None,
                mapping_group: None,
                disable_hypershift_mapping: None,
                disabled: r.child(decode_16)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_9(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_10(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<InputCounter> {
        let value = match r.tag()? {
            0 => InputCounter::Label(r.child(decode_6)?),
            1 => InputCounter::Number(r.child(decode_2)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_11(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<String>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_6)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_12(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<std::vec::Vec<String>>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_13)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_13(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<String>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_6)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_14(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<f64>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_15)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_15(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<f64> {
        let value = match r.tag()? {
            0 => r.f64()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_16(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<bool>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_9)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_ASSETS_KEYBOARD_SOURCES,
        0,
        "96595876b76bd621d2da3d685d84aff687fd33e0e23c0e68367bb8db5091cf24",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
#[test]
fn packaged_catalog_keyboard_sources_decodes() {
    let _ = read_keyboard_sources_catalog();
}
// End generated catalog reader: razer-assets/keyboard_sources.
