//! Mounted 67742/12383/Y TwoTap state and full-list merge.
//! Raw input is never interpreted as a successful device operation.
use super::*;
use gpui_kit::base::ElementExt as _;
use gpui_kit::component::{Sizable, spinner::Spinner};
use std::time::Duration;

#[derive(Deserialize)]
struct Product {
    product_id: u32,
    info: Info,
    sync_excluded: Vec<String>,
    analog_v1: bool,
    low_profile: bool,
    double_mapping: bool,
    disable_analog: bool,
}
#[derive(Deserialize)]
struct Info {
    min: f32,
    max: f32,
    unit: f32,
    #[serde(rename = "defaultValue")]
    default_value: f32,
    #[serde(rename = "minBreak")]
    min_break: f32,
}
fn data(pid: u32) -> &'static Product {
    static DATA: OnceLock<Vec<Product>> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("keyboard_mapping_two_tap_data.json"))
            .expect("audited mounted TwoTap data")
    })
    .iter()
    .find(|product| product.product_id == pid)
    .expect("audited TwoTap product")
}
#[derive(Default)]
pub(super) struct State {
    sliders: [Option<Entity<SliderState>>; 2],
    subscriptions: Vec<Subscription>,
    syncing_setting: bool,
    sync_timer: Option<Task<()>>,
}
pub(super) fn extended(pid: u32) -> bool {
    data(pid).double_mapping
}
/// 31867.fH.createDefaultActuation differs from actuation reducer defaults.
pub(super) fn default_points(workspace: &KeyboardProductWorkspace) -> Value {
    let product = data(workspace.spec.product_id);
    let high = &workspace.spec.config["DeviceInfo"]["analogSpecs"]["actuationHighSensitivityInfo"];
    let use_high =
        workspace.draft["guid"] == "177a20d5-f30a-4a52-923e-4fadb7db8392" && high.is_object();
    let make = if use_high {
        high["defaultValue"]
            .as_f64()
            .unwrap_or(product.info.default_value as f64) as f32
    } else {
        product.info.default_value
    };
    let unit = if use_high {
        high["unit"].as_f64().unwrap_or(product.info.unit as f64) as f32
    } else {
        product.info.unit
    };
    let min_break = if use_high {
        high["minBreak"]
            .as_f64()
            .unwrap_or(product.info.min_break as f64) as f32
    } else {
        product.info.min_break
    };
    json!({"0":make as u32,"1":(make-unit).max(min_break) as u32})
}
pub(super) fn disabled_secondary(points: Value) -> Value {
    json!({"actuationPoint":points,"isDefault":false,"outputType":"defaultGroup","isHyperShift":false})
}
fn raw(slot: &Slot, point: &str) -> Option<f32> {
    slot.staged["actuationPoint"][point]
        .as_f64()
        .map(|n| n as f32)
}
fn primary_analog(workspace: &KeyboardProductWorkspace, edit: &Edit) -> bool {
    controller::is_analog(
        workspace.spec.product_id,
        edit.slots[0].staged["controllerGroup"]["controllerAssignment"]
            .as_str()
            .unwrap_or_default(),
    )
}
fn joystick_direction(slot: &Slot) -> bool {
    slot.category == "JOYSTICK" && slot.staged["joystickGroup"]["isJoystickMovement"] == true
}
pub(super) fn secondary_available(workspace: &KeyboardProductWorkspace) -> bool {
    let Some(edit) = &workspace.analog_gamepad.mapping.edit else {
        return false;
    };
    if edit.button["inputType"] != "AnalogInput" {
        return false;
    }
    let product = data(workspace.spec.product_id);
    if product.disable_analog
        && matches!(edit.slots[0].category.as_str(), "CONTROLLER" | "JOYSTICK")
    {
        return false;
    }
    edit.slots[0].category == "KEYBOARD_FUNCTION"
        || product.double_mapping
            && (edit.slots[0].category == "JOYSTICK"
                || edit.slots[0].category == "CONTROLLER" && !product.disable_analog)
}
fn secondary_disabled(workspace: &KeyboardProductWorkspace, edit: &Edit) -> bool {
    let product = data(workspace.spec.product_id);
    let make = raw(&edit.slots[0], "0").unwrap_or(product.info.default_value) / product.info.unit;
    if product.low_profile {
        raw(&edit.slots[1], "0")
            .map(|other| (other / product.info.unit).round() - make.round() < 10.)
            .unwrap_or(make.round() > 19.)
    } else {
        make.round() > if product.analog_v1 { 10. } else { 30. }
    }
}
pub(super) fn category_visible(
    workspace: &KeyboardProductWorkspace,
    slot: usize,
    name: &str,
) -> bool {
    let Some(edit) = &workspace.analog_gamepad.mapping.edit else {
        return false;
    };
    if name == "RAZER_HYPERSHIFT" && (edit.shift || slot == 1) || name == "INTERDEVICE" && slot == 1
    {
        return false;
    }
    if name == "APP_SPECIFIC"
        && ![
            "Adobe Photoshop",
            "Adobe Illustrator",
            "Adobe After Effect",
            "Adobe Premiere Pro",
            "Google Chrome",
            "Microsoft Word",
            "Microsoft PowerPoint",
            "Microsoft Excel",
            "Microsoft Teams",
            "Microsoft Edge",
            "Firefox",
            "Davinci Resolve",
        ]
        .contains(&workspace.draft["name"].as_str().unwrap_or_default())
    {
        return false;
    }
    // AI availability needs a real 30078 query observation. Its absence is not
    // a fabricated negative result; no implementation subset hides a category.
    if slot == 0 {
        return true;
    }
    let product = data(workspace.spec.product_id);
    let allowed = if product.double_mapping {
        &[
            "JOYSTICK",
            "CONTROLLER",
            "KEYBOARD_FUNCTION",
            "MOUSE_FUNCTION",
        ][..]
    } else {
        &["KEYBOARD_FUNCTION"][..]
    };
    if !allowed.contains(&name) || product.disable_analog && name == "CONTROLLER" {
        return false;
    }
    if name == "MOUSE_FUNCTION"
        && (edit.slots[0].category == "CONTROLLER" && !primary_analog(workspace, edit)
            || edit.slots[0].category == "JOYSTICK" && !joystick_direction(&edit.slots[0])
            || edit.slots[0].category == "KEYBOARD_FUNCTION")
    {
        return false;
    }
    true
}
pub(super) fn controller_choice_disabled(
    workspace: &KeyboardProductWorkspace,
    slot: usize,
    assignment: &str,
) -> bool {
    let Some(edit) = &workspace.analog_gamepad.mapping.edit else {
        return false;
    };
    if slot == 1
        && (primary_analog(workspace, edit) || joystick_direction(&edit.slots[0]))
        && controller::is_analog(workspace.spec.product_id, assignment)
    {
        return true;
    }
    false
}
fn slot_valid(workspace: &KeyboardProductWorkspace, slot: &Slot) -> bool {
    if category(&slot.staged) != slot.category {
        return false;
    }
    match slot.category.as_str() {
        "KEYBOARD_FUNCTION" => {
            // An existing empty keyboardGroup is the source representation of
            // a default key with a custom threshold. A newly selected category
            // starts empty and requires an explicit choice.
            let existing = slot.original["keyboardGroup"].is_object()
                && slot.original["keyboardGroup"] == slot.staged["keyboardGroup"];
            existing
                || source(workspace.spec.product_id)
                    .key_targets
                    .iter()
                    .any(|key| {
                        key["inputID"].is_string()
                            && key["inputID"] == slot.staged["keyboardGroup"]["key"]
                    })
        }
        "CONTROLLER" => {
            let existing = slot.original["controllerGroup"]
                .get("controllerAssignment")
                .is_some()
                && slot.original["controllerGroup"] == slot.staged["controllerGroup"];
            existing || controller::valid(workspace.spec.product_id, &slot.staged)
        }
        "JOYSTICK" => joystick::valid(workspace.spec.product_id, slot),
        _ => true,
    }
}
pub(super) fn valid(workspace: &KeyboardProductWorkspace, edit: &Edit) -> bool {
    if !slot_valid(workspace, &edit.slots[0]) {
        return false;
    }
    if edit.secondary_enabled {
        if !slot_valid(workspace, &edit.slots[1]) {
            return false;
        }
        if edit.slots[1].category == "KEYBOARD_FUNCTION" && secondary_disabled(workspace, edit) {
            return false;
        }
        if let (Some(primary), Some(secondary)) =
            (raw(&edit.slots[0], "0"), raw(&edit.slots[1], "0"))
        {
            if secondary - primary < 16380. {
                return false;
            }
        }
    }
    true
}
fn preserve_metadata(old: &Value, new: &mut Value) {
    for field in ["actuationPoint", "rapidTrigger", "isEnableCustomRelease"] {
        if let Some(value) = old.get(field) {
            new[field] = value.clone();
        }
    }
}
pub(super) fn replace_slot(slot: &mut Slot, mut payload: Value) {
    preserve_metadata(&slot.staged, &mut payload);
    slot.staged = payload;
}
fn clean_input_fields(slot: &mut Value) {
    if let Some(object) = slot.as_object_mut() {
        for field in [
            "analogInput",
            "inputType",
            "inputID",
            "isHyperShift",
            "isActiveMapping",
        ] {
            object.remove(field);
        }
    }
}
/// Exact saveTwoTapChanges list semantics, including branch-specific rapid and
/// opposite-layer synchronization. Caller emits the real submission intent.
pub(super) fn merge(
    workspace: &KeyboardProductWorkspace,
    edit: &Edit,
    existing: &[Value],
) -> Vec<Value> {
    let mut mappings = existing.to_vec();
    let input = edit.button["inputID"].as_str().unwrap_or_default();
    let index = mappings.iter().position(|entry| {
        entry["inputID"] == input && entry["isHyperShift"].as_bool().unwrap_or(false) == edit.shift
    });
    let analog = edit.button["inputType"] == "AnalogInput";
    let mut primary = edit.slots[0].staged.clone();
    if !analog {
        let default = primary["isDefault"] == true;
        let mut entry =
            json!({"inputID":input,"inputType":edit.button["inputType"],"isHyperShift":edit.shift});
        entry
            .as_object_mut()
            .unwrap()
            .extend(primary.as_object().unwrap().clone());
        if default {
            if let Some(index) = index {
                mappings.remove(index);
            }
        } else if let Some(index) = index {
            mappings[index] = entry;
        } else {
            mappings.push(entry);
        }
        if default
            && !edit.shift
            && matches!(
                edit.button["assignment"].as_str(),
                Some("RAZER_HYPERSHIFT" | "DISABLE")
            )
        {
            mappings.retain(|entry| !(entry["inputID"] == input && entry["isHyperShift"] == true));
        }
        return mappings;
    }
    let defaults = default_points(workspace);
    let points = primary
        .get("actuationPoint")
        .cloned()
        .unwrap_or_else(|| defaults.clone());
    primary["actuationPoint"] = points.clone();
    if primary["isDefault"] == true
        && (points["0"].as_f64() != defaults["0"].as_f64() || primary["rapidTrigger"].is_object())
    {
        primary["keyboardGroup"] = json!({});
        primary["outputType"] = json!("keyboardGroup");
        primary.as_object_mut().unwrap().remove("isDefault");
    }
    let mut secondary = if edit.secondary_enabled {
        edit.slots[1].staged.clone()
    } else {
        disabled_secondary(defaults.clone())
    };
    clean_input_fields(&mut primary);
    clean_input_fields(&mut secondary);
    let mut entry = json!({"inputID":input,"inputType":edit.button["inputType"],"isHyperShift":edit.shift,"mapping":[primary,secondary]});
    if let Some(analog_input) = edit.button.get("analogInput") {
        entry["analogInput"] = analog_input.clone();
    }
    let default = entry["mapping"]
        .as_array()
        .unwrap()
        .iter()
        .any(|slot| slot["isDefault"] == true);
    let custom = points["0"].as_f64() != defaults["0"].as_f64();
    let mut sync_opposite = true;
    let mut sync_other_indices = false;
    if default {
        if let Some(index) = index {
            mappings.remove(index);
        }
    } else if let Some(index) = index {
        if mappings[index]
            .pointer("/mapping/0/outputType")
            .is_some_and(|group| group == "hyperShiftGroup")
        {
            mappings[index] = entry.clone();
            // Source searches isHyperShift without comparing to the active
            // layer; preserve its original Hyper replacement branch.
            if let Some(hyper) = mappings
                .iter()
                .position(|entry| entry["inputID"] == input && entry["isHyperShift"] == true)
            {
                mappings.remove(hyper);
            }
            sync_opposite = false;
        } else if mappings[index]["mapping"].is_array() || mappings[index]["macroGroup"].is_object()
        {
            sync_other_indices = mappings[index]["mapping"].is_array();
            sync_opposite = sync_other_indices;
            if let Some(old_rapid) = mappings[index].pointer("/mapping/0/rapidTrigger") {
                if entry["mapping"][0]["rapidTrigger"].is_null() {
                    entry["mapping"][0]["rapidTrigger"] = old_rapid.clone();
                }
            }
            if matches!(
                entry["mapping"][0]["outputType"].as_str(),
                Some("controllerGroup" | "joystickGroup")
            ) || entry["mapping"][1]["outputType"] != "defaultGroup"
            {
                entry["mapping"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("rapidTrigger");
            }
            mappings[index] = entry.clone();
        } else {
            // Source only converts a legacy flat macroGroup in this branch.
            sync_opposite = false;
        }
    } else {
        mappings.push(entry.clone());
    }
    if sync_opposite {
        if custom
            && !mappings.iter().any(|entry| {
                entry["inputID"] == input
                    && entry["isHyperShift"].as_bool().unwrap_or(false) != edit.shift
            })
        {
            let mut counterpart = entry.clone();
            counterpart["isHyperShift"] = json!(!edit.shift);
            counterpart["mapping"][0] =
                json!({"actuationPoint":points,"keyboardGroup":{},"outputType":"keyboardGroup"});
            mappings.push(counterpart);
        }
        for (other_index, other) in mappings.iter_mut().enumerate() {
            if other["inputID"] == input
                && if sync_other_indices {
                    Some(other_index) != index
                } else {
                    other["isHyperShift"].as_bool().unwrap_or(false) != edit.shift
                }
                && other["mapping"].is_array()
            {
                other["mapping"][0]["actuationPoint"] = points.clone();
            }
        }
    }
    mappings
}
fn actuation_visible(workspace: &KeyboardProductWorkspace, slot: usize, edit: &Edit) -> bool {
    if edit.button["inputType"] != "AnalogInput" || edit.slots[slot].category == "DISABLE" {
        return false;
    }
    if joystick_direction(&edit.slots[slot]) {
        return false;
    }
    if edit.slots[slot].category == "CONTROLLER" {
        if slot == 0 {
            return !primary_analog(workspace, edit);
        }
        if slot == 1 && (primary_analog(workspace, edit) || joystick_direction(&edit.slots[0])) {
            return !controller::is_analog(
                workspace.spec.product_id,
                edit.slots[1].staged["controllerGroup"]["controllerAssignment"]
                    .as_str()
                    .unwrap_or_default(),
            );
        }
    }
    true
}
pub(super) fn actuation_element(
    workspace: &KeyboardProductWorkspace,
    slot: usize,
    cx: &Context<KeyboardProductWorkspace>,
) -> Option<AnyElement> {
    let edit = workspace.analog_gamepad.mapping.edit.as_ref()?;
    if !actuation_visible(workspace, slot, edit) {
        return None;
    }
    let slider = workspace.analog_gamepad.mapping.two_tap.sliders[slot].as_ref()?;
    let slider_for_paint = slider.clone();
    let product_id = workspace.spec.product_id;
    let info = &data(workspace.spec.product_id).info;
    let make = raw(&edit.slots[slot], "0").unwrap_or(info.default_value);
    let disabled = slot == 1 && secondary_disabled(workspace, edit);
    Some(
        v_flex()
            .mx(surface::css(20.))
            .mt(surface::css(20.))
            .pt(surface::css(20.))
            .border_t_1()
            .border_color(rgb(0x5d5d5d))
            .child(
                div()
                    .text_color(rgb(0x999999))
                    .text_size(surface::css(14.))
                    .pb(surface::css(10.))
                    .child(t("ACTUATION_TEXT_GUIDE")),
            )
            .child(
                h_flex()
                    .justify_between()
                    .child(
                        v_flex()
                            .child(t("ACTUATION"))
                            .child(format!("{:.1} mm", (make / info.unit).round() / 10.)),
                    )
                    .child(surface::help_control(
                        SharedString::from(format!("two-tap-actuation-help-{slot}")),
                        t("ACTUATION_POINT_TOOLTIP"),
                    )),
            )
            .child(
                div()
                    .on_prepaint(move |_, window, cx| {
                        slider_for_paint.update(cx, |slider, cx| {
                            let value = (make / data(product_id).info.unit).round();
                            if slider.value().start() != value {
                                slider.set_value(value, window, cx);
                            }
                        });
                    })
                    .child(Slider::new(slider).disabled(disabled)),
            )
            .child(
                h_flex()
                    .justify_between()
                    .child(format!("{:.1}", (info.min / info.unit).round() / 10.))
                    .child(format!("{:.1}", (info.max / info.unit).round() / 10.)),
            )
            .when(slot == 0, |body| {
                body.child(
                    BaseButton::new("two-tap-sync-actuation")
                        .mt(surface::css(20.))
                        .when(
                            workspace.analog_gamepad.mapping.two_tap.syncing_setting,
                            |button| button.child(Spinner::new().small()),
                        )
                        .child(t("SYNC_ACTUATION_RELEASE_POINT"))
                        .on_click(cx.listener(|this, _, _, cx| this.sync_two_tap_actuation(cx))),
                )
            })
            .into_any_element(),
    )
}
pub(super) fn secondary_element(
    workspace: &KeyboardProductWorkspace,
    slot: usize,
    cx: &Context<KeyboardProductWorkspace>,
) -> Option<AnyElement> {
    let edit = workspace.analog_gamepad.mapping.edit.as_ref()?;
    if slot == 1 {
        return Some(
            BaseButton::new("two-tap-remove-secondary")
                .m(surface::css(20.))
                .child(t("REMOVE_SECONDARY_FUNC"))
                .on_click(cx.listener(|this, _, _, cx| this.set_two_tap_secondary(false, cx)))
                .into_any_element(),
        );
    }
    if edit.secondary_enabled || !secondary_available(workspace) {
        return None;
    }
    Some(
        h_flex()
            .mx(surface::css(20.))
            .my(surface::css(20.))
            .w(surface::css(210.))
            .h(surface::css(58.))
            .border_1()
            .border_color(rgb(0x707070))
            .rounded(surface::css(3.))
            .child(
                BaseButton::new("two-tap-add-secondary")
                    .flex_1()
                    .disabled(secondary_disabled(workspace, edit))
                    .child(t("TEXT_ADD_SECONDARY_FUNCTION"))
                    .on_click(cx.listener(|this, _, _, cx| this.set_two_tap_secondary(true, cx))),
            )
            .child(surface::help_control(
                "two-tap-secondary-help",
                t("TEXT_ADD_SECONDARY_FUNCTION_TIP"),
            ))
            .into_any_element(),
    )
}
impl KeyboardProductWorkspace {
    pub(super) fn init_two_tap_sliders(&mut self, cx: &mut Context<Self>) {
        self.analog_gamepad.mapping.two_tap = State::default();
        let Some(edit) = &self.analog_gamepad.mapping.edit else {
            return;
        };
        if edit.button["inputType"] != "AnalogInput" {
            return;
        }
        let info = &data(self.spec.product_id).info;
        let makes = [
            raw(&edit.slots[0], "0").unwrap_or(info.default_value),
            raw(&edit.slots[1], "0").unwrap_or(info.default_value),
        ];
        for slot in 0..2 {
            let slider = cx.new(|_| {
                SliderState::new()
                    .min((info.min / info.unit).round())
                    .max((info.max / info.unit).round())
                    .step(1.)
                    .default_value((makes[slot] / info.unit).round())
            });
            let subscription = cx.subscribe(&slider, move |this, _, event: &SliderEvent, cx| {
                if let SliderEvent::Change(value) = event {
                    this.change_two_tap_make(slot, value.start(), cx);
                }
            });
            self.analog_gamepad.mapping.two_tap.sliders[slot] = Some(slider);
            self.analog_gamepad
                .mapping
                .two_tap
                .subscriptions
                .push(subscription);
        }
    }
    pub(super) fn set_two_tap_secondary(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if enabled && !secondary_available(self) {
            return;
        }
        let product = data(self.spec.product_id);
        let Some(edit) = &mut self.analog_gamepad.mapping.edit else {
            return;
        };
        if enabled {
            if edit.slots[1].category == "DEFAULT" {
                edit.slots[1] = Slot::new(
                    json!({"outputType":"keyboardGroup","keyboardGroup":{"keyGroup":1,"key":"","modifiers":[]},"actuationPoint":{"0":(product.info.unit*36.)as u32,"1":product.info.default_value as u32}}),
                );
            }
        }
        if !enabled {
            let points = default_points(self);
            if let Some(edit) = &mut self.analog_gamepad.mapping.edit {
                edit.slots[1] = Slot::new(disabled_secondary(points));
            }
        }
        self.analog_gamepad
            .mapping
            .edit
            .as_mut()
            .unwrap()
            .secondary_enabled = enabled;
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }
    fn change_two_tap_make(&mut self, slot: usize, value: f32, cx: &mut Context<Self>) {
        let product = data(self.spec.product_id);
        let min = if slot == 1 {
            if product.low_profile { 13. } else { 11. }
        } else {
            (product.info.min / product.info.unit).round()
        };
        let value = value.max(min);
        let make = value * product.info.unit;
        let Some(edit) = &mut self.analog_gamepad.mapping.edit else {
            return;
        };
        let release = raw(&edit.slots[slot], "1").unwrap_or(product.info.default_value);
        // Y.onMakeActuationPointChange compares slider input against the raw
        // release value before its toMappingVal conversion; retain this source
        // unit asymmetry instead of silently normalizing it.
        let release = if value <= release {
            value - 2.
        } else {
            release
        };
        edit.slots[slot].staged["actuationPoint"] = json!({"0":make as u32,"1":release});
        let gap = product.info.unit * 10.;
        if slot == 1 {
            if raw(&edit.slots[0], "0").is_some_and(|primary| make - primary < gap) {
                let primary = (make - gap).max(product.info.min);
                edit.slots[0].staged["actuationPoint"]["0"] = json!(primary as u32);
            }
        } else if raw(&edit.slots[1], "0").is_some_and(|secondary| secondary - make < gap) {
            let ceiling = if product.low_profile {
                28.
            } else if product.analog_v1 {
                21.
            } else {
                40.
            } * product.info.unit;
            let secondary = (make + gap).min(ceiling);
            if !product.low_profile || make + gap <= ceiling {
                edit.slots[1].staged["actuationPoint"] = json!({"0":secondary as u32,"1":release});
            }
        }
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }
    fn sync_two_tap_actuation(&mut self, cx: &mut Context<Self>) {
        let Some(edit) = &self.analog_gamepad.mapping.edit else {
            return;
        };
        let Some(make) = raw(&edit.slots[0], "0") else {
            return;
        };
        let path = self.mapping_path();
        let Some(mut mappings) = self.draft.pointer(&path).and_then(Value::as_array).cloned()
        else {
            return;
        };
        let product = data(self.spec.product_id);
        let points = json!({"0":make as u32,"1":make as u32});
        let defaults = default_points(self);
        // 3342.QY intentionally observes the first mapping for each inputID.
        let ids: Vec<_> = self
            .mapping_buttons()
            .into_iter()
            .filter_map(|button| {
                let id = button["inputID"].as_str()?;
                if product.sync_excluded.iter().any(|key| key == id) {
                    return None;
                }
                let disabled = mappings
                    .iter()
                    .find(|entry| entry["inputID"] == id)
                    .is_some_and(|entry| {
                        let first = &entry["mapping"][0];
                        let second = &entry["mapping"][1];
                        matches!(
                            first["outputType"].as_str(),
                            Some("disableGroup" | "dynamicKeyStrokeGroup")
                        ) || first.pointer("/joystickGroup/isJoystickMovement")
                            == Some(&Value::Bool(true))
                            || first.pointer("/controllerGroup/controllerMode/isAnalog")
                                == Some(&Value::Bool(true))
                            || second["outputType"]
                                .as_str()
                                .and_then(|kind| second.get(kind))
                                .and_then(Value::as_object)
                                .is_some_and(|group| !group.is_empty())
                    });
                (!disabled).then(|| (id.to_owned(), button["inputType"].clone()))
            })
            .collect();
        // 3342.aC first updates existing primary actuation and removes empty
        // default assignments, then creates both layers for missing custom keys.
        for entry in &mut mappings {
            if ids.iter().any(|(id, _)| entry["inputID"] == id.as_str())
                && entry["mapping"][0].is_object()
            {
                entry["mapping"][0]["actuationPoint"] = points.clone();
            }
        }
        mappings.retain(|entry| {
            if entry["inputType"] != "AnalogInput" {
                return true;
            }
            let first = &entry["mapping"][0];
            first["outputType"] == "disableGroup"
                || first["outputType"]
                    .as_str()
                    .and_then(|kind| first.get(kind))
                    .and_then(Value::as_object)
                    .is_some_and(|group| !group.is_empty())
                || first["rapidTrigger"].is_object()
                || first["actuationPoint"]["0"]
                    .as_f64()
                    .is_some_and(|value| value != defaults["0"].as_f64().unwrap())
        });
        for (id, input_type) in ids {
            if points["0"].as_f64() != defaults["0"].as_f64()
                && !mappings.iter().any(|entry| entry["inputID"] == id)
            {
                for shift in [false, true] {
                    mappings.push(json!({"inputID":id,"inputType":input_type,"isHyperShift":shift,
                        "mapping":[{"outputType":"keyboardGroup","keyboardGroup":{},"actuationPoint":points},disabled_secondary(defaults.clone())]}));
                }
            }
        }
        if let Some(target) = self.draft.pointer_mut(&path) {
            *target = json!(mappings);
        }
        self.request_actuation_mapping(cx);
        self.request_actuation_message(
            json!({"type":"ON_SYNC_GLOBAL_ACTUATION","payload":{"value":points}}),
            cx,
        );
        self.analog_gamepad.mapping.two_tap.syncing_setting = true;
        self.analog_gamepad.mapping.two_tap.sync_timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            let _ = this.update(cx, |this, cx| {
                this.analog_gamepad.mapping.two_tap.syncing_setting = false;
                cx.notify();
            });
        }));
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }
}
