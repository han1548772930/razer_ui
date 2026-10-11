//! Current mounted 59184 MapJoyStick, not the separate quick-remapping page.
use super::*;
use gpui_kit::base::ElementExt as _;
use gpui_kit::component::radio::Radio;
use std::{cell::Cell, rc::Rc};

#[derive(Deserialize)]
struct Product {
    product_id: u32,
    buttons: Vec<Choice>,
    directions: Vec<Choice>,
    button_label: String,
    direction_label: String,
    placeholder: String,
}
#[derive(Deserialize)]
struct Choice {
    name: Option<String>,
    content: Option<String>,
    #[serde(rename = "assignedValue")]
    assigned_value: Option<String>,
}
fn data(pid: u32) -> &'static Product {
    static DATA: OnceLock<Vec<Product>> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("keyboard_mapping_joystick_data.json"))
            .expect("current mounted joystick tables")
    })
    .iter()
    .find(|row| row.product_id == pid)
    .expect("current joystick caller")
}
#[derive(Default, Clone, Deserialize, Serialize)]
pub(super) struct State {
    button: Option<usize>,
    direction: Option<usize>,
    #[serde(skip)]
    menu: [bool; 2],
    #[serde(skip)]
    above: [bool; 2],
    #[serde(skip)]
    bounds: [Rc<Cell<Bounds<Pixels>>>; 2],
    // Source selection state overrides hasExistingMappedValue once picked.
    selected: bool,
    pub(super) changed: bool,
}
fn direction(slot: &Slot) -> bool {
    slot.staged["joystickGroup"]["isJoystickMovement"] == true
}
fn assignment(group: &Value) -> Option<&str> {
    if group["joystickMode"] == "button" {
        return group["joystickButtonAssignment"].as_str();
    }
    let axes = &group["joystickAxisAssignment"];
    let axis = |name: &str| axes[name].as_i64();
    if group["joystickMode"] == "axes" {
        if let (Some(x), Some(y)) = (axis("X"), axis("Y")) {
            return match (x, y) {
                (128, 128) => Some("Bottom Right"),
                (-128, -128) => Some("Top Left"),
                (128, -128) => Some("Top Right"),
                (-128, 128) => Some("Bottom Left"),
                _ => None,
            };
        }
        for (name, plus, minus) in [
            ("X", "Right", "Left"),
            ("Y", "Down", "Up"),
            ("Z", "Rotate Clockwise", "Rotate Counter Clockwise"),
        ] {
            if let Some(value) = axis(name).filter(|v| *v != 0) {
                return Some(if value == 128 { plus } else { minus });
            }
        }
    } else if group["joystickMode"] == "raxes" {
        for (name, plus, minus) in [
            ("X", "Joystick Right", "Joystick Left"),
            ("Y", "Joystick Down", "Joystick Up"),
            ("Z", "Joystick Clockwise", "Joystick Counter Clockwise"),
        ] {
            if let Some(value) = axis(name).filter(|v| *v != 0) {
                return Some(if value == 128 { plus } else { minus });
            }
        }
    }
    None
}
fn index(pid: u32, slot: &Slot, movement: bool) -> usize {
    let state = &slot.joystick;
    if let Some(index) = if movement {
        state.direction
    } else {
        state.button
    } {
        return index;
    }
    let group = &slot.staged["joystickGroup"];
    if movement != direction(slot) {
        return 0;
    }
    let selected = assignment(group);
    let choices = if movement {
        &data(pid).directions
    } else {
        &data(pid).buttons
    };
    choices
        .iter()
        .position(|choice| {
            if movement {
                choice.assigned_value.as_deref() == selected && selected.is_some()
            } else {
                choice.content.as_deref() == selected && selected.is_some()
            }
        })
        .unwrap_or(0)
}
pub(super) fn payload(pid: u32, movement: bool, index: usize) -> Value {
    let product = data(pid);
    let mut group = if movement {
        json!({"joystickMode":"","isJoystickMovement":true})
    } else {
        json!({"joystickMode":"button","isJoystickMovement":false})
    };
    if !movement {
        if let Some(content) = product
            .buttons
            .get(index)
            .and_then(|choice| choice.content.as_ref())
        {
            group["joystickButtonAssignment"] = json!(content);
        }
    } else if let Some(content) = product
        .directions
        .get(index)
        .and_then(|choice| choice.content.as_deref())
    {
        let axes: &[&str] = if content.contains('X') && content.contains('Y') {
            &["X", "Y"]
        } else if content.contains('X') {
            &["X"]
        } else if content.contains('Y') {
            &["Y"]
        } else if content.contains('Z') {
            &["Z"]
        } else {
            &[]
        };
        if !axes.is_empty() {
            group["joystickMode"] = json!(if content.contains("rX")
                || content.contains("rY")
                || content.contains("rZ")
            {
                "raxes"
            } else {
                "axes"
            });
            let mut values = serde_json::Map::new();
            for axis in axes {
                values.insert(
                    (*axis).into(),
                    json!(if content.contains(&format!("{axis}+")) {
                        128
                    } else {
                        -128
                    }),
                );
            }
            group["joystickAxisAssignment"] = Value::Object(values);
        }
    }
    json!({"outputType":"joystickGroup","joystickGroup":group})
}
pub(super) fn valid(pid: u32, slot: &Slot) -> bool {
    if slot.joystick.selected {
        return index(pid, slot, direction(slot)) > 0;
    }
    // 12383.hasExistingMappedValue accepts either original source field.
    let original = &slot.original["joystickGroup"];
    original == &slot.staged["joystickGroup"]
        && (original.get("joystickMode").is_some() || original.get("isJoystickMovement").is_some())
}
fn direction_disabled(workspace: &KeyboardProductWorkspace, slot: usize) -> bool {
    if slot == 0 {
        return false;
    }
    let Some(edit) = &workspace.analog_gamepad.mapping.edit else {
        return false;
    };
    let primary = &edit.slots[0];
    direction(primary)
        || controller::is_analog(
            workspace.spec.product_id,
            primary.staged["controllerGroup"]["controllerAssignment"]
                .as_str()
                .unwrap_or_default(),
        )
}
fn duplicate(
    choice: &Choice,
    movement: bool,
    mappings: &[Value],
    shift: bool,
    analog: bool,
) -> bool {
    if movement && choice.assigned_value.is_none() || !movement && choice.name.is_some() {
        return false;
    }
    mappings
        .iter()
        .filter(|entry| entry["isHyperShift"].as_bool().unwrap_or(false) == shift)
        .filter_map(|entry| entry["mapping"].as_array())
        .any(|slots| {
            slots
                .iter()
                .take(if analog { slots.len() } else { 1 })
                .any(|slot| {
                    let group = slot["outputType"]
                        .as_str()
                        .and_then(|kind| slot.get(kind))
                        .unwrap_or(&slot["joystickGroup"]);
                    if movement {
                        assignment(group) == choice.assigned_value.as_deref()
                            && group["joystickMode"].is_string()
                    } else {
                        group["joystickButtonAssignment"].as_str() == choice.content.as_deref()
                            && (!analog || group["joystickMode"] == "button")
                    }
                })
        })
}
fn dropdown(
    workspace: &KeyboardProductWorkspace,
    slot: usize,
    movement: bool,
    cx: &Context<KeyboardProductWorkspace>,
) -> AnyElement {
    let edit = workspace
        .analog_gamepad
        .mapping
        .edit
        .as_ref()
        .expect("mounted joystick");
    let staged = &edit.slots[slot];
    let product = data(workspace.spec.product_id);
    let choices = if movement {
        &product.directions
    } else {
        &product.buttons
    };
    let selected = index(workspace.spec.product_id, staged, movement);
    let active = direction(staged) == movement;
    let menu_slot = usize::from(movement);
    let open = staged.joystick.menu[menu_slot] && active;
    let bounds = staged.joystick.bounds[menu_slot].clone();
    let mappings = workspace
        .draft
        .pointer(&workspace.mapping_path())
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let label = if selected == 0 {
        t(&product.placeholder)
    } else {
        choices
            .get(selected)
            .map(|choice| {
                t(choice
                    .name
                    .as_deref()
                    .or(choice.content.as_deref())
                    .unwrap_or_default())
            })
            .unwrap_or_default()
    };
    let mut row = div()
        .id(SharedString::from(format!(
            "joystick-dropdown-{slot}-{movement}"
        )))
        .relative()
        .on_prepaint(move |rect, _, _| bounds.set(rect))
        .ml(surface::css(30.))
        .w(surface::css(180.))
        .h(surface::css(27.))
        .mb(surface::css(10.))
        .child(
            BaseButton::new(SharedString::from(format!(
                "joystick-selector-{slot}-{movement}"
            )))
            .w_full()
            .h_full()
            .px(surface::css(5.))
            .border_1()
            .border_color(rgb(if open { 0x44d62c } else { 0x515151 }))
            .disabled(!active)
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .child(
                        div()
                            .text_size(surface::css(14.))
                            .text_color(rgb(if selected == 0 { 0x999999 } else { 0xcccccc }))
                            .child(label),
                    )
                    .child(
                        svg()
                            .path("synapse/expand.svg")
                            .w(surface::css(10.))
                            .h(surface::css(5.))
                            .with_transformation(Transformation::rotate(radians(if open {
                                std::f32::consts::PI
                            } else {
                                0.
                            }))),
                    ),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                if this.analog_gamepad.mapping.joystick_activation.is_none() {
                    this.analog_gamepad.mapping.joystick_activation =
                        Some(cx.observe_window_activation(window, |this, window, cx| {
                            if !window.is_window_active() {
                                if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                                    for slot in &mut edit.slots {
                                        slot.joystick.menu = [false; 2];
                                    }
                                }
                                cx.notify();
                            }
                        }));
                }
                if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                    let state = &mut edit.slots[slot].joystick;
                    state.above[menu_slot] = state.bounds[menu_slot].get().bottom()
                        + surface::css(182.).to_pixels(window.rem_size())
                        > window.viewport_size().height;
                    edit.slots[slot].joystick.menu[menu_slot] =
                        !edit.slots[slot].joystick.menu[menu_slot];
                }
                cx.notify();
            })),
        );
    if open {
        row = row.child(
            deferred(
                v_flex()
                    .id(SharedString::from(format!(
                        "joystick-options-{slot}-{movement}"
                    )))
                    .absolute()
                    .when(staged.joystick.above[menu_slot], |menu| {
                        menu.bottom(surface::css(28.))
                    })
                    .when(!staged.joystick.above[menu_slot], |menu| {
                        menu.top(surface::css(28.))
                    })
                    .left_0()
                    .w_full()
                    .max_h(surface::css(180.))
                    .overflow_y_scroll()
                    .bg(rgb(0))
                    .border_1()
                    .border_color(rgb(0x515151))
                    .children(choices.iter().enumerate().skip(1).map(|(index, choice)| {
                        if choice.name.as_deref() == Some("divider") {
                            return div()
                                .h(surface::css(1.))
                                .my(surface::css(4.))
                                .bg(rgb(0x515151))
                                .into_any_element();
                        }
                        let disabled = duplicate(
                            choice,
                            movement,
                            mappings,
                            edit.shift,
                            edit.button["inputType"] == "AnalogInput",
                        );
                        BaseButton::new(SharedString::from(format!(
                            "joystick-option-{slot}-{movement}-{index}"
                        )))
                        .w_full()
                        .h(surface::css(25.))
                        .px(surface::css(4.))
                        .text_size(surface::css(14.))
                        .text_color(rgb(0xcccccc))
                        .disabled(disabled)
                        .when(disabled, |item| item.opacity(0.3))
                        .when(index == selected, |item| item.text_color(rgb(0x44d62c)))
                        .when(!disabled, |item| {
                            item.hover(|style| style.bg(rgba(0xffffff1a)))
                        })
                        .child(t(choice
                            .name
                            .as_deref()
                            .or(choice.content.as_deref())
                            .unwrap_or_default()))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.choose_mapping_joystick(slot, movement, index, cx)
                        }))
                        .into_any_element()
                    })),
            )
            .with_priority(if movement { 100 } else { 101 }),
        );
    }
    row.on_mouse_down_out(
        cx.listener(move |this, event: &MouseDownEvent, window, cx| {
            if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                let state = &edit.slots[slot].joystick;
                let bounds = state.bounds[menu_slot].get();
                let height = surface::css(180.).to_pixels(window.rem_size());
                let gap = surface::css(1.).to_pixels(window.rem_size());
                let top = if state.above[menu_slot] {
                    bounds.top() - gap - height
                } else {
                    bounds.bottom() + gap
                };
                if state.menu[menu_slot]
                    && event.position.x >= bounds.left()
                    && event.position.x <= bounds.right()
                    && event.position.y >= top
                    && event.position.y <= top + height
                {
                    return;
                }
                edit.slots[slot].joystick.menu[menu_slot] = false;
            }
            cx.notify();
        }),
    )
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{Slot, data, duplicate, index, payload, two_tap, valid};
    use serde_json::json;
    #[test]
    fn source_axis_signs_and_modes_round_trip_all_current_products() {
        for pid in [678, 679, 688] {
            for (choice, mode, axes) in [
                (1, "axes", json!({"X":128})),
                (2, "axes", json!({"X":-128})),
                (3, "axes", json!({"Y":-128})),
                (4, "axes", json!({"Y":128})),
                (5, "axes", json!({"Z":128})),
                (6, "axes", json!({"Z":-128})),
                (8, "axes", json!({"X":128,"Y":-128})),
                (9, "axes", json!({"X":-128,"Y":-128})),
                (10, "axes", json!({"X":128,"Y":128})),
                (11, "axes", json!({"X":-128,"Y":128})),
                (13, "raxes", json!({"X":128})),
                (14, "raxes", json!({"X":-128})),
                (15, "raxes", json!({"Y":128})),
                (16, "raxes", json!({"Y":-128})),
                (17, "raxes", json!({"Z":128})),
                (18, "raxes", json!({"Z":-128})),
            ] {
                let value = payload(pid, true, choice);
                assert_eq!(
                    value,
                    json!({"outputType":"joystickGroup","joystickGroup":{"joystickMode":mode,"isJoystickMovement":true,"joystickAxisAssignment":axes}})
                );
                assert_eq!(index(pid, &Slot::new(value), true), choice);
            }
        }
    }
    #[test]
    fn analog_duplicate_guard_observes_secondary_and_active_layer_only() {
        let mappings = vec![
            json!({"isHyperShift":false,"mapping":[payload(679,false,2),payload(679,true,8)]}),
        ];
        let choice = &data(679).directions[8];
        assert!(duplicate(choice, true, &mappings, false, true));
        assert!(!duplicate(choice, true, &mappings, true, true));
        assert!(!duplicate(choice, true, &mappings, false, false));
        assert!(duplicate(
            &data(679).buttons[2],
            false,
            &mappings,
            false,
            true
        ));
        assert!(!duplicate(
            &data(679).buttons[0],
            false,
            &mappings,
            false,
            true
        ));
    }
    #[test]
    fn source_placeholder_requires_selection_existing_assignment_is_retained() {
        let original = payload(679, false, 24);
        let mut slot = Slot::new(original);
        assert!(valid(679, &slot));
        slot.joystick.selected = true;
        slot.joystick.button = Some(0);
        two_tap::replace_slot(&mut slot, payload(679, false, 0));
        assert!(!valid(679, &slot));
        slot.joystick.button = Some(24);
        two_tap::replace_slot(&mut slot, payload(679, false, 24));
        assert!(valid(679, &slot));
        assert_eq!(
            slot.staged["joystickGroup"]["joystickButtonAssignment"],
            "24"
        );
    }
}
pub(super) fn element(
    workspace: &KeyboardProductWorkspace,
    slot: usize,
    cx: &Context<KeyboardProductWorkspace>,
) -> AnyElement {
    let edit = workspace
        .analog_gamepad
        .mapping
        .edit
        .as_ref()
        .expect("mounted joystick");
    let movement = direction(&edit.slots[slot]);
    let product = data(workspace.spec.product_id);
    v_flex()
        .mx(surface::css(20.))
        .mb(surface::css(10.))
        .child(
            Radio::new(SharedString::from(format!("joystick-button-radio-{slot}")))
                .label(t(&product.button_label))
                .checked(!movement)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.change_mapping_joystick_radio(slot, false, cx)
                })),
        )
        .child(dropdown(workspace, slot, false, cx))
        .when(!direction_disabled(workspace, slot), |body| {
            body.child(
                Radio::new(SharedString::from(format!(
                    "joystick-direction-radio-{slot}"
                )))
                .label(t(&product.direction_label))
                .checked(movement)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.change_mapping_joystick_radio(slot, true, cx)
                })),
            )
            .child(dropdown(workspace, slot, true, cx))
        })
        .into_any_element()
}
impl KeyboardProductWorkspace {
    fn choose_mapping_joystick(
        &mut self,
        slot: usize,
        movement: bool,
        choice: usize,
        cx: &mut Context<Self>,
    ) {
        let pid = self.spec.product_id;
        let Some(edit) = &mut self.analog_gamepad.mapping.edit else {
            return;
        };
        let state = &mut edit.slots[slot].joystick;
        if movement {
            state.direction = Some(choice);
        } else {
            state.button = Some(choice);
        }
        state.selected = true;
        state.changed = true;
        state.menu = [false; 2];
        two_tap::replace_slot(&mut edit.slots[slot], payload(pid, movement, choice));
        if slot == 0 {
            self.correct_secondary_joystick(cx);
        }
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }
    fn change_mapping_joystick_radio(
        &mut self,
        slot: usize,
        movement: bool,
        cx: &mut Context<Self>,
    ) {
        if movement && direction_disabled(self, slot) {
            return;
        }
        let Some(edit) = &self.analog_gamepad.mapping.edit else {
            return;
        };
        let choice = index(self.spec.product_id, &edit.slots[slot], movement);
        let previous_movement = direction(&edit.slots[slot]);
        let previous_choice = index(self.spec.product_id, &edit.slots[slot], previous_movement);
        let state = &mut self.analog_gamepad.mapping.edit.as_mut().unwrap().slots[slot].joystick;
        if previous_movement {
            state.direction = Some(previous_choice);
        } else {
            state.button = Some(previous_choice);
        }
        self.choose_mapping_joystick(slot, movement, choice, cx);
    }
    pub(super) fn correct_secondary_joystick(&mut self, cx: &mut Context<Self>) {
        let reset = self
            .analog_gamepad
            .mapping
            .edit
            .as_ref()
            .is_some_and(|edit| {
                edit.secondary_enabled
                    && edit.slots[1].category == "JOYSTICK"
                    && direction(&edit.slots[1])
            })
            && direction_disabled(self, 1);
        if reset {
            self.change_mapping_joystick_radio(1, false, cx);
        }
    }
}
