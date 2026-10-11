//! Mounted Analog Controller module84350, not MapControllerV2.
//! Current source literals and graph presets are prepared without executing JS.
use super::*;
use gpui_kit::base::ElementExt as _;
use std::{cell::Cell, rc::Rc};

#[derive(Deserialize)]
struct Product {
    product_id: u32,
    controller: Controller,
}
#[derive(Deserialize)]
struct Controller {
    analog: Vec<Choice>,
    digital: Vec<Choice>,
    modes: Vec<Mode>,
    graph: BTreeMap<String, BTreeMap<String, Node>>,
    without_synapse: Vec<String>,
    disable_analog: bool,
    heading: String,
    placeholder: String,
}
#[derive(Deserialize)]
struct Choice {
    content: String,
}
#[derive(Deserialize)]
struct Mode {
    name: String,
    mode: String,
}
#[derive(Clone, Copy, Deserialize)]
pub(super) struct Node {
    x: f32,
    y: f32,
}
fn data(pid: u32) -> &'static Controller {
    static DATA: OnceLock<Vec<Product>> = OnceLock::new();
    &DATA
        .get_or_init(|| {
            serde_json::from_str(include_str!("keyboard_mapping_controller_data.json"))
                .expect("audited mounted controller metadata")
        })
        .iter()
        .find(|product| product.product_id == pid)
        .expect("audited controller caller")
        .controller
}
#[derive(Default)]
pub(super) struct State {
    menu: bool,
    drag: Option<usize>,
    bounds: Rc<Cell<Bounds<Pixels>>>,
}
fn nodes(pid: u32, slot: &Value) -> [Node; 4] {
    let group = &slot["controllerGroup"];
    if let Some(points) = group
        .pointer("/analogSensitivityList/analogSensitivityAssignment")
        .and_then(Value::as_array)
    {
        if points.len() == 4 {
            let values = points
                .iter()
                .map(|point| serde_json::from_value::<Node>(point.clone()))
                .collect::<Result<Vec<_>, _>>();
            if let Ok(values) = values {
                return values.try_into().ok().expect("four source points");
            }
        }
    }
    let controller = data(pid);
    let name = group["analogSensitivityType"]
        .as_str()
        .unwrap_or("STANDARD");
    let graph = controller
        .graph
        .get(name)
        .unwrap_or(&controller.graph["STANDARD"]);
    [graph["p1"], graph["p2"], graph["p3"], graph["p4"]]
}
pub(super) fn payload(pid: u32, assignment: &str, mode: &str, points: Option<[Node; 4]>) -> Value {
    let controller = data(pid);
    let analog = controller
        .analog
        .iter()
        .any(|choice| choice.content == assignment);
    let mut group = json!({"controllerAssignment":assignment,"controllerMode":{"isAnalog":analog},"analogSensitivityType":mode});
    if analog {
        let graph = controller
            .graph
            .get(mode)
            .unwrap_or(&controller.graph["STANDARD"]);
        let values = points.unwrap_or([graph["p1"], graph["p2"], graph["p3"], graph["p4"]]);
        let fixed = controller
            .without_synapse
            .iter()
            .any(|value| value == assignment);
        group["analogSensitivityList"] = json!({"analogSensitivityAssignment":values.map(|node|json!({"x":if fixed{node.x}else{(node.x*100.).round()/100.},"y":if fixed{node.y}else{node.y.round()}}))});
    }
    json!({"outputType":"controllerGroup","controllerGroup":group})
}
pub(super) fn is_analog(pid: u32, assignment: &str) -> bool {
    data(pid)
        .analog
        .iter()
        .any(|choice| choice.content == assignment)
}
pub(super) fn valid(pid: u32, slot: &Value) -> bool {
    let assignment = slot
        .pointer("/controllerGroup/controllerAssignment")
        .and_then(Value::as_str);
    assignment.is_some_and(|assignment| {
        data(pid)
            .analog
            .iter()
            .chain(data(pid).digital.iter())
            .any(|choice| choice.content == assignment)
    })
}
fn duplicate(mappings: &[Value], shift: bool, assignment: &str) -> bool {
    mappings.iter().any(|mapping| {
        mapping["isHyperShift"].as_bool().unwrap_or(false) == shift
            && mapping["mapping"].as_array().is_some_and(|slots| {
                slots.iter().any(|slot| {
                    slot.pointer("/controllerGroup/controllerAssignment")
                        .and_then(Value::as_str)
                        == Some(assignment)
                })
            })
    })
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
        .expect("mounted controller edit");
    let controller = data(workspace.spec.product_id);
    let state = &workspace.analog_gamepad.mapping.controller[slot];
    let assignment = edit.slots[slot]
        .staged
        .pointer("/controllerGroup/controllerAssignment")
        .and_then(Value::as_str);
    let mappings = workspace
        .draft
        .pointer(&workspace.mapping_path())
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let mut body = v_flex().mx(surface::css(20.)).child(
        BaseButton::new(SharedString::from(format!(
            "analog-controller-selector-{slot}"
        )))
        .w_full()
        .child(
            assignment
                .map(t)
                .unwrap_or_else(|| t(&controller.placeholder)),
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            this.analog_gamepad.mapping.controller[slot].menu =
                !this.analog_gamepad.mapping.controller[slot].menu;
            cx.notify();
        })),
    );
    if state.menu {
        body = body.child(
            v_flex()
                .id(SharedString::from(format!(
                    "analog-controller-choices-{slot}"
                )))
                .max_h(surface::css(220.))
                .overflow_y_scroll()
                .children(
                    controller
                        .analog
                        .iter()
                        .filter(|_| !controller.disable_analog)
                        .chain(controller.digital.iter())
                        .map(|choice| {
                            let assignment = choice.content.clone();
                            let used = duplicate(mappings, edit.shift, &assignment);
                            BaseButton::new(SharedString::from(format!(
                                "analog-controller-choice-{slot}-{assignment}"
                            )))
                            .py(surface::css(5.))
                            .w_full()
                            .disabled(
                                used || two_tap::controller_choice_disabled(
                                    workspace,
                                    slot,
                                    &assignment,
                                ),
                            )
                            .child(t(&assignment))
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    let pid = this.spec.product_id;
                                    if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                                        two_tap::replace_slot(
                                            &mut edit.slots[slot],
                                            payload(pid, &assignment, "STANDARD", None),
                                        );
                                    }
                                    this.analog_gamepad.mapping.controller[slot].menu = false;
                                    if slot == 0 {
                                        this.correct_secondary_joystick(cx);
                                    }
                                    cx.emit(KeyboardProductChanged);
                                    cx.notify();
                                },
                            ))
                        }),
                ),
        );
    }
    let graph_visible = assignment.is_some_and(|assignment| {
        is_analog(workspace.spec.product_id, assignment)
            && !controller
                .without_synapse
                .iter()
                .any(|value| value == assignment)
    });
    if graph_visible {
        let mode = edit.slots[slot]
            .staged
            .pointer("/controllerGroup/analogSensitivityType")
            .and_then(Value::as_str)
            .unwrap_or("STANDARD");
        body = body
            .child(div().mt(surface::css(20.)).child(t(&controller.heading)))
            .child(
                h_flex()
                    .flex_wrap()
                    .children(controller.modes.iter().map(|choice| {
                        let name = choice.mode.clone();
                        Button::new(SharedString::from(format!(
                            "analog-controller-mode-{slot}-{name}"
                        )))
                        .label(t(&choice.name))
                        .outline()
                        .selected(mode == name)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let pid = this.spec.product_id;
                            if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                                if let Some(assignment) = edit.slots[slot]
                                    .staged
                                    .pointer("/controllerGroup/controllerAssignment")
                                    .and_then(Value::as_str)
                                    .map(str::to_owned)
                                {
                                    two_tap::replace_slot(
                                        &mut edit.slots[slot],
                                        payload(pid, &assignment, &name, None),
                                    );
                                }
                            }
                            cx.emit(KeyboardProductChanged);
                            cx.notify();
                        }))
                    })),
            );
        let points = nodes(workspace.spec.product_id, &edit.slots[slot].staged);
        let graph = &controller.graph["STANDARD"];
        let low = graph["p0"].x;
        let high = graph["p5"].x;
        let bounds = state.bounds.clone();
        body = body.child(
            div()
                .id(SharedString::from(format!(
                    "analog-controller-graph-{slot}"
                )))
                .relative()
                .w(surface::css(181.))
                .h(surface::css(128.))
                .on_prepaint(move |rect, _, _| bounds.set(rect))
                .child(
                    canvas(
                        |_, _, _| (),
                        move |bounds, _, window, _| {
                            let pos = |node: Node| {
                                point(
                                    bounds.left()
                                        + bounds.size.width * ((node.x - low) / (high - low)),
                                    bounds.bottom() - bounds.size.height * (node.y / 255.),
                                )
                            };
                            let mut grid = PathBuilder::stroke(px(1.));
                            for tick in 0..=4 {
                                let x = bounds.left() + bounds.size.width * (tick as f32 / 4.);
                                grid.move_to(point(x, bounds.top()));
                                grid.line_to(point(x, bounds.bottom()));
                            }
                            for tick in 0..=3 {
                                let y = bounds.top() + bounds.size.height * (tick as f32 / 3.);
                                grid.move_to(point(bounds.left(), y));
                                grid.line_to(point(bounds.right(), y));
                            }
                            if let Ok(path) = grid.build() {
                                window.paint_path(path, rgb(0x707070));
                            }
                            let mut fill = PathBuilder::fill();
                            fill.move_to(pos(points[0]));
                            for node in &points[1..] {
                                fill.line_to(pos(*node));
                            }
                            fill.line_to(point(bounds.right(), bounds.bottom()));
                            fill.line_to(point(bounds.left(), bounds.bottom()));
                            fill.close();
                            if let Ok(path) = fill.build() {
                                window.paint_path(path, rgba(0x44d62c4d));
                            }
                            let mut stroke = PathBuilder::stroke(px(1.));
                            stroke.move_to(pos(points[0]));
                            for node in &points[1..] {
                                stroke.line_to(pos(*node));
                            }
                            if let Ok(path) = stroke.build() {
                                window.paint_path(path, rgb(0x44d62c));
                            }
                        },
                    )
                    .size_full(),
                )
                .children(points.into_iter().enumerate().map(|(index, node)| {
                    BaseButton::new(SharedString::from(format!(
                        "analog-controller-dot-{slot}-{index}"
                    )))
                    .absolute()
                    .left(surface::css((node.x - low) / (high - low) * 181. - 4.))
                    .top(surface::css(128. - node.y / 255. * 128. - 4.))
                    .size(surface::css(8.))
                    .rounded_full()
                    .border_1()
                    .border_color(rgb(0x44d62c))
                    .bg(rgb(0x222222))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.analog_gamepad.mapping.controller[slot].drag = Some(index);
                            cx.notify();
                        }),
                    )
                }))
                .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                    this.move_analog_controller_point(slot, event.position, cx);
                }))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.analog_gamepad.mapping.controller[slot].drag = None;
                        cx.notify();
                    }),
                )
                .on_mouse_up_out(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.analog_gamepad.mapping.controller[slot].drag = None;
                        cx.notify();
                    }),
                ),
        );
    }
    body.into_any_element()
}
impl KeyboardProductWorkspace {
    fn move_analog_controller_point(
        &mut self,
        slot: usize,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.analog_gamepad.mapping.controller[slot].drag else {
            return;
        };
        let bounds = self.analog_gamepad.mapping.controller[slot].bounds.get();
        if bounds.size.width <= px(0.) || bounds.size.height <= px(0.) {
            return;
        }
        let Some(edit) = &self.analog_gamepad.mapping.edit else {
            return;
        };
        let pid = self.spec.product_id;
        let mut points = nodes(pid, &edit.slots[slot].staged);
        let graph = &data(pid).graph["STANDARD"];
        let low = graph["p0"].x;
        let high = graph["p5"].x;
        let x = low
            + (f32::from(position.x - bounds.left()) / f32::from(bounds.size.width)).clamp(0., 1.)
                * (high - low);
        let y = (1. - f32::from(position.y - bounds.top()) / f32::from(bounds.size.height))
            .clamp(0., 1.)
            * 255.;
        points[index].y = y;
        let separation = 3. / 181. * (high - low);
        if index == 1 {
            points[index].x = x.clamp(points[0].x + separation, points[2].x - separation);
        }
        if index == 2 {
            points[index].x = x.clamp(points[1].x + separation, points[3].x - separation);
        }
        let assignment = edit.slots[slot]
            .staged
            .pointer("/controllerGroup/controllerAssignment")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        if let Some(edit) = &mut self.analog_gamepad.mapping.edit {
            two_tap::replace_slot(
                &mut edit.slots[slot],
                payload(pid, &assignment, "CUSTOM", Some(points)),
            );
        }
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }
}
