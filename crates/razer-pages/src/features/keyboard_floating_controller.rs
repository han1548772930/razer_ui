//! Current inline floating Controller assignment palette, independent of tester data.
use super::*;
use crate::keyboard_geometry::KeyboardKeyExt;
use gpui_kit::base::ElementExt as _;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[path = "keyboard_controller_drop.rs"]
mod controller_drop;

#[derive(Deserialize)]
struct Source {
    product_id: u32,
    buttons: Vec<SourceButton>,
    description: String,
    tooltip_type: String,
}
#[derive(Deserialize)]
struct SourceButton {
    id: String,
    name: String,
    icon: String,
    mapping: Value,
    key: razer_assets::KeyboardKey,
}
fn source(pid: u32) -> &'static Source {
    static DATA: OnceLock<Vec<Source>> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("keyboard_floating_controller_data.json"))
            .expect("current floating Controller source")
    })
    .iter()
    .find(|row| row.product_id == pid)
    .expect("audited floating Controller product")
}

#[derive(Default)]
pub(super) struct State {
    pub(super) open: bool,
    pub(super) config_bounds: Rc<Cell<Bounds<Pixels>>>,
    pub(super) keyboard_bounds: Rc<Cell<Bounds<Pixels>>>,
    pub(super) key_bounds:
        Rc<RefCell<BTreeMap<String, (&'static razer_assets::KeyboardKey, Bounds<Pixels>)>>>,
    geometry: Rc<Cell<Geometry>>,
    panel_drag: Option<(Point<Pixels>, [f32; 2])>,
    pub(super) button_drag: Option<usize>,
    hover: Option<usize>,
    pointer: Point<Pixels>,
    outside_window: bool,
    panel_drag_moved: bool,
}
#[derive(Clone, Copy, Default)]
struct Geometry {
    origin: Option<[f32; 2]>,
    area: [f32; 4],
    panel: [f32; 2],
    panel_bounds: Bounds<Pixels>,
    previous_drawer: bool,
    shifted: bool,
}
/// Preserve the source max-then-min order, including a too-small drag area.
fn clamp(origin: [f32; 2], area: [f32; 2], panel: [f32; 2], drawer: bool) -> [f32; 2] {
    [
        origin[0]
            .max(if drawer { 200. } else { 0. })
            .min(area[0] - panel[0]),
        origin[1].max(0.).min(area[1] - panel[1]),
    ]
}
fn drawer_shift(origin: &mut [f32; 2], previous: &mut bool, shifted: &mut bool, drawer: bool) {
    if drawer != *previous {
        if drawer && origin[0] <= 200. {
            origin[0] += 200.;
            *shifted = true;
        } else if !drawer && *shifted {
            origin[0] -= 200.;
            *shifted = false;
        }
        *previous = drawer;
    }
}
impl State {
    pub(super) fn toggle(&mut self) {
        self.open = !self.open;
        self.cancel_drag();
    }
    pub(super) fn cancel_pointer(&mut self) {
        self.cancel_drag();
    }
    fn cancel_drag(&mut self) {
        self.panel_drag = None;
        self.button_drag = None;
        self.hover = None;
        self.panel_drag_moved = false;
    }
    pub(super) fn unmount(&mut self) {
        *self = Self::default();
    }
    /// Raw source palette assignment, before the caller's duplicate/drop guards.
    pub(super) fn drag_mapping(&self, pid: u32) -> Option<Value> {
        self.button_drag
            .map(|index| source(pid).buttons[index].mapping.clone())
    }
    fn move_panel(&mut self, position: Point<Pixels>, scale: f32, drawer: bool) {
        let Some((start, origin)) = self.panel_drag else {
            return;
        };
        let mut geometry = self.geometry.get();
        let point = [
            origin[0] + f32::from(position.x - start.x) / scale,
            origin[1] + f32::from(position.y - start.y) / scale,
        ];
        geometry.origin = Some(clamp(
            point,
            [geometry.area[2], geometry.area[3]],
            geometry.panel,
            drawer,
        ));
        self.geometry.set(geometry);
    }
}

impl KeyboardProductWorkspace {
    fn drop_floating_controller(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let state = &self.analog_gamepad.floating;
        let Some(mut slots) = state.drag_mapping(self.spec.product_id) else {
            return;
        };
        if state.geometry.get().panel_bounds.contains(&position) {
            return;
        }
        let input = state
            .key_bounds
            .borrow()
            .values()
            .find_map(|(key, bounds)| {
                key.contains_window_point(position, *bounds)
                    .then(|| key.id.clone())
            });
        let Some(input) = input else {
            return;
        };
        let Some(mut target) = self
            .mapping_buttons()
            .iter()
            .find(|button| button["inputID"] == input)
            .cloned()
        else {
            return;
        };
        let defaults = self.analog_default_points();
        let Some(mapping) = slots.as_array_mut() else {
            return;
        };
        for slot in mapping {
            if slot.get("actuationPoint").is_some() {
                slot["actuationPoint"] = defaults.clone();
            }
        }
        let path = self.mapping_path();
        let Some(list) = self.draft.pointer(&path).and_then(Value::as_array) else {
            return;
        };
        if list.iter().any(|entry| {
            entry["inputID"] == target["inputID"]
                && entry["isHyperShift"].as_bool() == Some(self.hypershift)
                && entry["mapping"][0]["outputType"] == "hyperShiftGroup"
        }) {
            target["assignment"] = json!("RAZER_HYPERSHIFT");
        }
        let Some(not_remapped) =
            controller_drop::not_remapped(self.spec.product_id, &target, self.hypershift)
        else {
            return;
        };
        let Some(merged) = controller_drop::merge(
            self.spec.product_id,
            list,
            &target,
            not_remapped,
            self.hypershift,
            &json!({"data":slots}),
            &defaults,
        ) else {
            return;
        };
        if let Some(list) = self.draft.pointer_mut(&path) {
            *list = json!(merged);
        }
        cx.emit(KeyboardProductChanged);
        self.request_actuation_mapping(cx);
        cx.notify();
    }
    pub(super) fn floating_controller_layer(&self, cx: &Context<Self>) -> AnyElement {
        if !self.analog_gamepad.floating.open {
            return div().into_any_element();
        }
        div()
            .children(self.analog_gamepad.floating.panel_drag_moved.then(|| {
                deferred(FloatingLayer {
                    owner: cx.entity(),
                    kind: LayerKind::Area,
                })
                .with_priority(9)
            }))
            .child(
                deferred(FloatingLayer {
                    owner: cx.entity(),
                    kind: LayerKind::Panel,
                })
                .with_priority(10),
            )
            .children(self.analog_gamepad.floating.hover.map(|index| {
                deferred(FloatingLayer {
                    owner: cx.entity(),
                    kind: LayerKind::Tooltip(index),
                })
                .with_priority(100)
            }))
            .children(self.analog_gamepad.floating.button_drag.map(|index| {
                deferred(FloatingLayer {
                    owner: cx.entity(),
                    kind: LayerKind::Preview(index),
                })
                .with_priority(100)
            }))
            .into_any_element()
    }
}

enum LayerKind {
    Area,
    Panel,
    Tooltip(usize),
    Preview(usize),
}
struct FloatingLayer {
    owner: Entity<KeyboardProductWorkspace>,
    kind: LayerKind,
}
struct LayerLayout {
    content: AnyElement,
    size: Size<Pixels>,
}
impl IntoElement for FloatingLayer {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for FloatingLayer {
    type RequestLayoutState = LayerLayout;
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayerLayout) {
        let mut content = match self.kind {
            LayerKind::Area => {
                let config = self
                    .owner
                    .read(cx)
                    .analog_gamepad
                    .floating
                    .config_bounds
                    .get();
                div()
                    .w(window.viewport_size().width
                        - surface::css(60.).to_pixels(window.rem_size()))
                    .h(config.size.height)
                    .bg(rgba(0x0000001a))
                    .into_any_element()
            }
            LayerKind::Panel => panel(&self.owner, window, cx),
            LayerKind::Tooltip(index) => tooltip(&self.owner, index, cx),
            LayerKind::Preview(index) => preview(&self.owner, index, cx),
        };
        let width = match self.kind {
            LayerKind::Panel => {
                AvailableSpace::Definite(surface::css(360.).to_pixels(window.rem_size()))
            }
            _ => AvailableSpace::MaxContent,
        };
        let size = content.layout_as_root(size(width, AvailableSpace::MinContent), window, cx);
        let id = window.request_layout(Style::default(), [], cx);
        (id, LayerLayout { content, size })
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        layout: &mut LayerLayout,
        window: &mut Window,
        cx: &mut App,
    ) {
        let workspace = self.owner.read(cx);
        let state = &workspace.analog_gamepad.floating;
        let scale = f32::from(surface::css(1.).to_pixels(window.rem_size()));
        let position = match self.kind {
            LayerKind::Area => point(px(30. * scale), state.config_bounds.get().top()),
            LayerKind::Panel => {
                let config = state.config_bounds.get();
                let keyboard = state.keyboard_bounds.get();
                let left = 30. * scale;
                let width = f32::from(window.viewport_size().width) / scale - 60.;
                let height = f32::from(config.size.height) / scale;
                let mut geometry = state.geometry.get();
                let panel = [
                    f32::from(layout.size.width) / scale,
                    f32::from(layout.size.height) / scale,
                ];
                let mut origin = geometry.origin.unwrap_or([
                    f32::from(keyboard.left()) / scale - panel[0] - 30.,
                    height / 2. - panel[1] / 2.,
                ]);
                drawer_shift(
                    &mut origin,
                    &mut geometry.previous_drawer,
                    &mut geometry.shifted,
                    workspace.button_drawer_open,
                );
                origin = clamp(origin, [width, height], panel, workspace.button_drawer_open);
                geometry.origin = Some(origin);
                geometry.panel = panel;
                geometry.area = [30., f32::from(config.top()) / scale, width, height];
                let position = point(
                    px(left + origin[0] * scale),
                    config.top() + px(origin[1] * scale),
                );
                geometry.panel_bounds = Bounds::new(position, layout.size);
                state.geometry.set(geometry);
                position
            }
            LayerKind::Preview(_) => state.pointer + point(px(10. * scale), px(10. * scale)),
            LayerKind::Tooltip(index) => {
                let geometry = state.geometry.get();
                let [x, y, w, h] = source(workspace.spec.product_id).buttons[index].key.bounds;
                let image_width = (geometry.panel[0] - 42.) * scale;
                let k = image_width / 320.;
                let shape = Bounds::new(
                    geometry.panel_bounds.origin
                        + point(px(21. * scale + x * k), px(21. * scale + y * k)),
                    size(px(w * k), px(h * k)),
                );
                let panel = geometry.panel_bounds;
                let gap = px(10. * scale);
                let mut top = shape.top() - layout.size.height - gap;
                if top < panel.top() {
                    top = shape.bottom() + gap;
                }
                let mut left = shape.left() + shape.size.width / 2. - layout.size.width / 2.;
                if left < panel.left() + gap {
                    left = panel.left() + gap;
                } else if left + layout.size.width > panel.right() - gap {
                    left = panel.right() - gap - layout.size.width;
                }
                point(left, top)
            }
        };
        layout.content.prepaint_at(position, window, cx);
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        layout: &mut LayerLayout,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        layout.content.paint(window, cx);
    }
}

fn panel(
    owner: &Entity<KeyboardProductWorkspace>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let workspace = owner.read(cx);
    let data = source(workspace.spec.product_id);
    let state = &workspace.analog_gamepad.floating;
    let color = if workspace.hypershift {
        rgb(0xfd8611)
    } else {
        rgb(0x44d62c)
    };
    let content_width = 318.; // current universal border-box: 360 - 2*20 padding - 2*1 border.
    let k = content_width / 320.;
    let image = div()
        .relative()
        .w_full()
        .h(surface::css(content_width * 220. / 320.))
        .child(
            img("synapse/keyboard-floating-controller-skeleton.72fd3181.svg")
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
        )
        .children(data.buttons.iter().enumerate().map(|(index, button)| {
            let [x, y, w, h] = button.key.bounds;
            let visible = state.hover == Some(index) || state.button_drag == Some(index);
            let key = &button.key;
            let bounds = Rc::new(Cell::new(Bounds::default()));
            let hit_bounds = bounds.clone();
            let hover_bounds = bounds.clone();
            let region = div()
                .id(SharedString::from(format!(
                    "floating-controller-{}",
                    button.id
                )))
                .absolute()
                .left(surface::css(x * k))
                .top(surface::css(y * k))
                .w(surface::css(w * k))
                .h(surface::css(h * k))
                .cursor(if state.button_drag == Some(index) {
                    CursorStyle::ClosedHand
                } else if state.hover == Some(index) {
                    CursorStyle::OpenHand
                } else {
                    CursorStyle::Arrow
                })
                .on_prepaint(move |rect, _, _| bounds.set(rect))
                .on_mouse_move(window.listener_for(
                    owner,
                    move |this, event: &MouseMoveEvent, _, cx| {
                        let hit = key.contains_window_point(event.position, hover_bounds.get());
                        let state = &mut this.analog_gamepad.floating;
                        if hit && state.hover != Some(index) {
                            state.hover = Some(index);
                            cx.notify();
                        } else if !hit && state.hover == Some(index) {
                            state.hover = None;
                            cx.notify();
                        }
                    },
                ))
                .on_hover(
                    window.listener_for(owner, move |this, hover: &bool, _, cx| {
                        if !*hover && this.analog_gamepad.floating.hover == Some(index) {
                            this.analog_gamepad.floating.hover = None;
                            cx.notify();
                        }
                    }),
                )
                .child(
                    canvas(
                        |_, _, _| (),
                        move |rect, _, window, _| {
                            if visible {
                                let width = 2. * f32::from(rect.size.width) / key.bounds[2];
                                let stroke = StrokeOptions::default()
                                    .with_line_width(width)
                                    .with_line_join(lyon_tessellation::LineJoin::Round)
                                    .with_miter_limit(10.);
                                if let Ok(path) = key
                                    .path(rect, true)
                                    .with_style(PathStyle::Stroke(stroke))
                                    .build()
                                {
                                    window.paint_path(path, color);
                                }
                            }
                        },
                    )
                    .size_full(),
                );
            MouseButton::all()
                .into_iter()
                .fold(region, |region, mouse_button| {
                    let hit_bounds = hit_bounds.clone();
                    region.on_mouse_down(
                        mouse_button,
                        window.listener_for(owner, move |this, event: &MouseDownEvent, _, cx| {
                            if !key.contains_window_point(event.position, hit_bounds.get()) {
                                return;
                            }
                            cx.stop_propagation();
                            let state = &mut this.analog_gamepad.floating;
                            state.panel_drag = None;
                            state.button_drag = Some(index);
                            state.pointer = event.position;
                            cx.notify();
                        }),
                    )
                })
        }));
    let event_owner = owner.clone();
    let button_dragging = state.button_drag.is_some();
    let panel = v_flex()
        .id("keyboard-floating-controller")
        .occlude()
        .relative()
        .w(surface::css(360.))
        .p(surface::css(20.))
        .border_1()
        .border_color(rgb(0x5d5d5d))
        .rounded(surface::css(5.))
        .bg(rgb(0x111111))
        .child(div().mb(surface::css(20.)).child(image))
        .child(
            img("synapse/keyboard-floating-icon_draggable.be674683.svg")
                .absolute()
                .top(surface::css(10.))
                .left(relative(0.5))
                .ml(surface::css(-9.))
                .w(surface::css(18.))
                .h(surface::css(8.)),
        )
        .child(
            div()
                .id("floating-controller-close")
                .absolute()
                .top_0()
                .right_0()
                .size(surface::css(34.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                .on_mouse_down(MouseButton::Middle, |_, _, cx| cx.stop_propagation())
                .on_mouse_down(
                    MouseButton::Navigate(NavigationDirection::Back),
                    |_, _, cx| cx.stop_propagation(),
                )
                .on_mouse_down(
                    MouseButton::Navigate(NavigationDirection::Forward),
                    |_, _, cx| cx.stop_propagation(),
                )
                .on_click(window.listener_for(owner, |this, _, _, cx| {
                    this.analog_gamepad.floating.toggle();
                    cx.notify();
                }))
                .child(
                    img("synapse/keyboard-floating-icon_close.55fe41f1.svg")
                        .size(surface::css(20.)),
                ),
        )
        .child(
            div()
                .text_size(surface::css(14.))
                .text_center()
                .child(t(&data.description)),
        )
        .child(
            canvas(
                move |_, window, _| {
                    button_dragging.then(|| {
                        window.insert_hitbox(
                            Bounds::new(Point::default(), window.viewport_size()),
                            HitboxBehavior::Normal,
                        )
                    })
                },
                move |_, cursor_hitbox, window, cx| {
                    if let Some(hitbox) = cursor_hitbox {
                        window.set_cursor_style(CursorStyle::ClosedHand, &hitbox);
                    }
                    let moving = event_owner.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                        if !phase.capture() {
                            return;
                        }
                        moving.update(cx, |this, cx| {
                            let drawer = this.button_drawer_open;
                            let state = &mut this.analog_gamepad.floating;
                            state.outside_window =
                                !Bounds::new(Point::default(), window.viewport_size())
                                    .contains(&event.position);
                            if state.panel_drag.is_some() || state.button_drag.is_some() {
                                let scale =
                                    f32::from(surface::css(1.).to_pixels(window.rem_size()));
                                if state.panel_drag.is_some() && !state.outside_window {
                                    state.move_panel(event.position, scale, drawer);
                                    state.panel_drag_moved = true;
                                }
                                state.pointer = event.position;
                                cx.notify();
                            }
                        });
                    });
                    let outside = event_owner.clone();
                    window.on_mouse_event(move |_: &MouseExitEvent, phase, _, cx| {
                        if phase.capture() {
                            outside.update(cx, |this, _| {
                                this.analog_gamepad.floating.outside_window = true
                            });
                        }
                    });
                    let released = event_owner.clone();
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                        if phase.capture() {
                            released.update(cx, |this, cx| {
                                this.drop_floating_controller(event.position, cx)
                            });
                        }
                        if !phase.bubble() {
                            return;
                        }
                        released.update(cx, |this, cx| {
                            let drawer = this.button_drawer_open;
                            let state = &mut this.analog_gamepad.floating;
                            if state.panel_drag.is_some() || state.button_drag.is_some() {
                                state.move_panel(
                                    event.position,
                                    f32::from(surface::css(1.).to_pixels(window.rem_size())),
                                    drawer,
                                );
                                state.cancel_drag();
                                cx.notify();
                            }
                        });
                    });
                },
            )
            .absolute()
            .size_0(),
        );
    MouseButton::all()
        .into_iter()
        .fold(panel, |panel, mouse_button| {
            panel.on_mouse_down(
                mouse_button,
                window.listener_for(owner, |this, event: &MouseDownEvent, _, cx| {
                    let state = &mut this.analog_gamepad.floating;
                    if let Some(origin) = state.geometry.get().origin {
                        state.panel_drag = Some((event.position, origin));
                        cx.notify();
                    }
                }),
            )
        })
        .into_any_element()
}
fn tooltip(owner: &Entity<KeyboardProductWorkspace>, index: usize, cx: &App) -> AnyElement {
    let data = source(owner.read(cx).spec.product_id);
    v_flex()
        .min_h(surface::css(75.))
        .max_w(surface::css(300.))
        .justify_center()
        .px(surface::css(10.))
        .py(surface::css(8.))
        .border_1()
        .border_color(rgb(0x5d5d5d))
        .bg(rgb(0x000000))
        .text_color(rgb(0xcccccc))
        .text_center()
        .child(
            div()
                .text_size(surface::css(12.))
                .line_height(surface::css(14.))
                .child(t(&data.tooltip_type).to_uppercase()),
        )
        .child(
            div()
                .text_size(surface::css(18.))
                .line_height(surface::css(18.))
                .child(t(&data.buttons[index].name)),
        )
        .into_any_element()
}
fn preview(owner: &Entity<KeyboardProductWorkspace>, index: usize, cx: &App) -> AnyElement {
    let workspace = owner.read(cx);
    let button = &source(workspace.spec.product_id).buttons[index];
    h_flex()
        .h(surface::css(40.))
        .p(surface::css(10.))
        .items_center()
        .rounded(surface::css(5.))
        .bg(if workspace.hypershift {
            rgb(0xfd8611)
        } else {
            rgb(0x44d62c)
        })
        .cursor(CursorStyle::ClosedHand)
        .child(
            img(SharedString::from(button.icon.clone()))
                .size(surface::css(20.))
                .mr(surface::css(10.)),
        )
        .child(
            div()
                .text_size(surface::css(14.))
                .font_bold()
                .text_color(rgb(0x212121))
                .whitespace_nowrap()
                .child(t(&button.name)),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{State, clamp, drawer_shift};
    #[test]
    fn floating_controller_narrow_area_preserves_original_clamp_order() {
        assert_eq!(
            clamp([0., 0.], [300., 200.], [360., 250.], true),
            [-60., -50.]
        );
        assert_eq!(
            clamp([1000., -20.], [1000., 500.], [360., 300.], true),
            [640., 0.]
        );
    }
    #[test]
    fn floating_controller_drawer_shift_is_reversed_only_when_applied() {
        let mut origin = [150., 20.];
        let (mut previous, mut shifted) = (false, false);
        drawer_shift(&mut origin, &mut previous, &mut shifted, true);
        assert_eq!(origin, [350., 20.]);
        drawer_shift(&mut origin, &mut previous, &mut shifted, true);
        assert_eq!(origin, [350., 20.]);
        drawer_shift(&mut origin, &mut previous, &mut shifted, false);
        assert_eq!(origin, [150., 20.]);
        origin = [300., 20.];
        drawer_shift(&mut origin, &mut previous, &mut shifted, true);
        drawer_shift(&mut origin, &mut previous, &mut shifted, false);
        assert_eq!(origin, [300., 20.]);
    }
    #[test]
    fn floating_controller_close_retains_position_and_clears_drag() {
        let mut state = State::default();
        let mut geometry = state.geometry.get();
        geometry.origin = Some([235., 42.]);
        state.geometry.set(geometry);
        state.toggle();
        state.button_drag = Some(3);
        state.toggle();
        state.toggle();
        assert!(state.open);
        assert_eq!(state.geometry.get().origin, Some([235., 42.]));
        assert_eq!(state.button_drag, None);
    }
}
