//! Read-only projection of current product input catalogs for displayMode=macro.
//! DEFAULTPROFILE mappings are not a physical input catalog. In particular,
//! 182 uses its audited 1368 groupList, not its broader default mapping array.
use crate::{model::Device, resources};
use serde_json::Value;
use std::sync::OnceLock;

#[derive(Clone)]
pub(crate) struct MacroInput {
    id: String,
    label: String,
    enabled: bool,
    bounds: Option<[f32; 4]>,
    shape: Option<&'static resources::KeyboardKey>,
}
impl MacroInput {
    pub(crate) fn id(&self) -> &str {
        &self.id
    }
    pub(crate) fn label(&self) -> &str {
        &self.label
    }
    pub(crate) fn enabled(&self) -> bool {
        self.enabled
    }
    pub(crate) fn bounds(&self) -> Option<[f32; 4]> {
        self.bounds
    }
    pub(crate) fn shape(&self) -> Option<&'static resources::KeyboardKey> {
        self.shape
    }
}

#[derive(Clone)]
pub(crate) struct MacroInputLayout {
    inputs: Vec<MacroInput>,
    image: Option<String>,
    viewbox: [f32; 2],
    mouse_diagram: bool,
}
impl MacroInputLayout {
    pub(crate) fn inputs(&self) -> &[MacroInput] {
        &self.inputs
    }
    pub(crate) fn image(&self) -> Option<&str> {
        self.image.as_deref()
    }
    pub(crate) fn viewbox(&self) -> [f32; 2] {
        self.viewbox
    }
    pub(crate) fn mouse_diagram(&self) -> bool {
        self.mouse_diagram
    }
}

fn input(value: &Value, shape: Option<&'static resources::KeyboardKey>) -> Option<MacroInput> {
    let id = value["inputID"].as_str()?;
    let supported = value["functionList"]
        .as_array()?
        .iter()
        .any(|kind| kind == "MACRO");
    let label = shape.map(|shape| shape.label.clone()).unwrap_or_else(|| {
        let key = value["defaultValue"]
            .as_str()
            .or_else(|| value["counter"].as_str())
            .unwrap_or(id);
        crate::i18n::t(key).to_string()
    });
    Some(MacroInput {
        id: id.into(),
        label,
        enabled: supported && value["isEnabled"].as_bool().unwrap_or(false),
        bounds: shape.map(|shape| shape.bounds),
        shape,
    })
}

fn group_inputs(groups: &[Value]) -> Vec<MacroInput> {
    let mut inputs = Vec::new();
    for group in groups {
        let group = group.get("group").unwrap_or(group);
        for value in group["buttonList"].as_array().into_iter().flatten() {
            if let Some(input) = input(value, None) {
                if !inputs.iter().any(|other: &MacroInput| other.id == input.id) {
                    inputs.push(input);
                }
            }
        }
    }
    inputs
}

pub(crate) fn for_device(device: &Device) -> Option<MacroInputLayout> {
    if device.product_id == 182 {
        static GROUPS: OnceLock<Vec<Value>> = OnceLock::new();
        let groups = GROUPS.get_or_init(|| {
            serde_json::from_str(include_str!("macro_inputs_182.json"))
                .expect("audited 182 input catalog")
        });
        return Some(MacroInputLayout {
            inputs: group_inputs(groups),
            image: resources::device_image(
                182,
                device.edition_id,
                device.layout_id,
                resources::DeviceImage::Product,
            )
            .map(str::to_owned),
            viewbox: [770., 340.],
            mouse_diagram: true,
        });
    }
    if device.product_id == 653 {
        let source: Vec<Value> =
            serde_json::from_str(resources::keyboard_source_for_layout(device.layout_id)?).ok()?;
        let shapes = resources::keyboard_keys_for_layout(device.layout_id);
        let values = source.iter().flat_map(|group| {
            group["group"]["buttonList"]
                .as_array()
                .into_iter()
                .flatten()
        });
        let inputs = values
            .filter_map(|value| {
                let shape = shapes
                    .iter()
                    .find(|shape| value["inputID"].as_str() == Some(&shape.id))?;
                input(value, Some(shape))
            })
            .collect();
        return Some(MacroInputLayout {
            inputs,
            image: resources::device_image(
                653,
                device.edition_id,
                device.layout_id,
                resources::DeviceImage::Product,
            )
            .map(str::to_owned),
            viewbox: [730., 340.],
            mouse_diagram: false,
        });
    }
    if let Some(spec) = super::keyboard_products::source_product(device.product_id) {
        let inputs = spec
            .macro_keys()
            .iter()
            .filter_map(|value| {
                let shape = spec
                    .macro_shapes()
                    .iter()
                    .find(|shape| value["inputID"].as_str() == Some(&shape.id));
                input(value, shape)
            })
            .collect::<Vec<_>>();
        if inputs.is_empty() {
            return None;
        }
        return Some(MacroInputLayout {
            inputs,
            image: spec.macro_image().map(str::to_owned),
            viewbox: spec.macro_viewbox(),
            mouse_diagram: false,
        });
    }
    let spec = super::mouse_products::source_product(device.product_id)?;
    let inputs = group_inputs(spec.macro_groups());
    if inputs.is_empty() {
        return None;
    }
    Some(MacroInputLayout {
        inputs,
        image: spec.macro_image().map(str::to_owned),
        viewbox: [600., 340.],
        mouse_diagram: false,
    })
}
