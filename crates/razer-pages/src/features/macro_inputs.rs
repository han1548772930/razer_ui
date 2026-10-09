//! Read-only projection of current product input catalogs for displayMode=macro.
//! DEFAULTPROFILE mappings are not a physical input catalog. Audited standalone
//! catalogs and prepared-layout policies are selected by capability data.
use razer_assets as resources;
use razer_model::model::Device;
use serde::Deserialize;
use serde_json::Value;
use std::sync::OnceLock;

#[derive(Clone)]
pub struct MacroInput {
    id: String,
    label: String,
    enabled: bool,
    bounds: Option<[f32; 4]>,
    shape: Option<&'static resources::KeyboardKey>,
}
impl MacroInput {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn label(&self) -> &str {
        &self.label
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn bounds(&self) -> Option<[f32; 4]> {
        self.bounds
    }
    pub fn shape(&self) -> Option<&'static resources::KeyboardKey> {
        self.shape
    }
}

#[derive(Clone)]
pub struct MacroInputLayout {
    inputs: Vec<MacroInput>,
    image: Option<String>,
    viewbox: [f32; 2],
    mouse_diagram: bool,
}
impl MacroInputLayout {
    pub fn inputs(&self) -> &[MacroInput] {
        &self.inputs
    }
    pub fn image(&self) -> Option<&str> {
        self.image.as_deref()
    }
    pub fn viewbox(&self) -> [f32; 2] {
        self.viewbox
    }
    pub fn mouse_diagram(&self) -> bool {
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
        razer_i18n::t(key).to_string()
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

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum InputSource {
    PhysicalGroups,
    PreparedKeyboardLayout,
}

#[derive(Deserialize)]
struct MacroInputCatalog {
    product_id: u32,
    input_source: InputSource,
    #[serde(default)]
    groups: Vec<Value>,
    viewbox: [f32; 2],
    mouse_diagram: bool,
}

#[derive(Deserialize)]
struct MacroInputCatalogs {
    products: Vec<MacroInputCatalog>,
}

fn source_catalog(product_id: u32) -> Option<&'static MacroInputCatalog> {
    static CATALOGS: OnceLock<MacroInputCatalogs> = OnceLock::new();
    CATALOGS
        .get_or_init(|| {
            serde_json::from_str(include_str!("macro_input_catalogs.json"))
                .expect("audited Macro input catalog capabilities")
        })
        .products
        .iter()
        .find(|catalog| catalog.product_id == product_id)
}

impl MacroInputCatalog {
    fn for_device(&self, device: &Device) -> Option<MacroInputLayout> {
        let inputs = match self.input_source {
            InputSource::PhysicalGroups => group_inputs(&self.groups),
            InputSource::PreparedKeyboardLayout => {
                // The prepared catalog owns its legacy/default layout policy.
                // Unknown layouts must not fall through to a different catalog.
                let source: Vec<Value> =
                    serde_json::from_str(resources::keyboard_source_for_layout(device.layout_id)?)
                        .ok()?;
                let shapes = resources::keyboard_keys_for_layout(device.layout_id);
                source
                    .iter()
                    .flat_map(|group| {
                        group["group"]["buttonList"]
                            .as_array()
                            .into_iter()
                            .flatten()
                    })
                    .filter_map(|value| {
                        let shape = shapes
                            .iter()
                            .find(|shape| value["inputID"].as_str() == Some(&shape.id))?;
                        input(value, Some(shape))
                    })
                    .collect()
            }
        };
        Some(MacroInputLayout {
            inputs,
            image: resources::device_image(
                razer_model::demo::artwork_product_id(device.product_id),
                device.edition_id,
                device.layout_id,
                resources::DeviceImage::Product,
            )
            .map(str::to_owned),
            viewbox: self.viewbox,
            mouse_diagram: self.mouse_diagram,
        })
    }
}

pub fn for_device(device: &Device) -> Option<MacroInputLayout> {
    if let Some(catalog) = source_catalog(device.product_id) {
        return catalog.for_device(device);
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

#[cfg(test)]
#[path = "macro_inputs_tests.rs"]
mod tests;
