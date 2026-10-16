//! The secondary marker actually styled by current Keypad sidebars.
use super::*;
use std::{cell::RefCell, time::Duration};

#[derive(Deserialize)]
pub(super) struct Metadata {
    pub(super) group_field: String,
    pub(super) assignment_field: String,
    pub(super) default_assignment: String,
    pub(super) label: String,
    pub(super) asset: String,
}
#[derive(Default)]
pub(super) struct State {
    hovered: Option<usize>,
    position: Rc<Cell<Bounds<Pixels>>>,
    trigger_bounds: Rc<RefCell<BTreeMap<usize, Bounds<Pixels>>>>,
    tip_bounds: Rc<RefCell<BTreeMap<usize, Bounds<Pixels>>>>,
}
pub(super) fn marker(
    workspace: &KeyboardProductWorkspace,
    metadata: &Metadata,
    button: &Value,
    index: usize,
    disabled: bool,
    cx: &Context<KeyboardProductWorkspace>,
) -> Option<AnyElement> {
    // Source tests `!== undefined`: a present null group is still present.
    if button.get(&metadata.group_field).is_none()
        || button[&metadata.assignment_field] == metadata.default_assignment
    {
        return None;
    }
    let assignment = &button[&metadata.assignment_field];
    // Actual Keypad render passes hyperShiftOn, not the different
    // isHyperShiftOn prop used by getRenderKeyMappingType. Its absent value
    // keeps this path on the original secondary assignment label.
    let kind = if panel_truthy(assignment) {
        t(&text(assignment))
    } else {
        String::new()
    };
    let value = button
        .get(format!("{}Value", metadata.assignment_field))
        .map(text)
        .unwrap_or_default();
    let state = &workspace.analog_gamepad.mapping.panel_secondary;
    Some(
        Marker {
            index,
            owner: cx.entity().downgrade(),
            asset: metadata.asset.clone(),
            label: t(&metadata.label),
            kind,
            value: t(&value),
            shown: state.hovered == Some(index),
            disabled,
            position: state.position.clone(),
            trigger_bounds: state.trigger_bounds.clone(),
            tip_bounds: state.tip_bounds.clone(),
        }
        .into_any_element(),
    )
}
#[derive(IntoElement)]
struct Marker {
    index: usize,
    owner: WeakEntity<KeyboardProductWorkspace>,
    asset: String,
    label: String,
    kind: String,
    value: String,
    shown: bool,
    disabled: bool,
    position: Rc<Cell<Bounds<Pixels>>>,
    trigger_bounds: Rc<RefCell<BTreeMap<usize, Bounds<Pixels>>>>,
    tip_bounds: Rc<RefCell<BTreeMap<usize, Bounds<Pixels>>>>,
}
fn tip_surface(label: String, kind: String, value: String) -> Div {
    div()
        .font_family("Roboto")
        .max_w(surface::css(300.))
        .min_h(surface::css(75.))
        .bg(rgb(0x000000))
        .border_1()
        .border_color(rgb(0x44d62c))
        .line_height(surface::css(14.))
        .text_center()
        .child(
            v_flex()
                .px(surface::css(10.))
                .py(surface::css(8.))
                .child(
                    div()
                        .font_family("Roboto")
                        .text_size(surface::css(14.))
                        .mb(surface::css(5.))
                        .text_color(rgb(0x707070))
                        .child(label),
                )
                .child(
                    div()
                        .text_size(surface::css(12.))
                        .text_color(rgb(0xcccccc))
                        .line_height(surface::css(14.))
                        .child(kind.to_uppercase()),
                )
                .child(
                    div()
                        .text_size(surface::css(18.))
                        .text_color(rgb(0xcccccc))
                        .line_height(surface::css(18.))
                        .min_h(surface::css(19.))
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(value),
                ),
        )
}
fn hover(
    owner: &WeakEntity<KeyboardProductWorkspace>,
    index: usize,
    entered: bool,
    tip: bool,
    size: Size<Pixels>,
    window: &Window,
    cx: &mut App,
) {
    let _ = owner.update(cx, |this, cx| {
        let state = &mut this.analog_gamepad.mapping.panel_secondary;
        let other = if tip {
            &state.trigger_bounds
        } else {
            &state.tip_bounds
        };
        let inside = entered
            || state.hovered == Some(index)
                && other
                    .borrow()
                    .get(&index)
                    .is_some_and(|bounds| bounds.contains(&window.mouse_position()));
        let next = if inside {
            Some(index)
        } else if state.hovered == Some(index) {
            None
        } else {
            state.hovered
        };
        if next != state.hovered {
            if entered && !tip {
                if let Some(rect) = state.trigger_bounds.borrow().get(&index) {
                    state.position.set(Bounds::new(
                        point(
                            rect.left() + surface::css(20.).to_pixels(window.rem_size())
                                - size.width / 2.,
                            rect.top()
                                - surface::css(2.).to_pixels(window.rem_size())
                                - size.height,
                        ),
                        Size::default(),
                    ));
                }
            }
            state.hovered = next;
            cx.notify();
        }
    });
}
impl RenderOnce for Marker {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut measurement =
            tip_surface(self.label.clone(), self.kind.clone(), self.value.clone())
                .into_any_element();
        let size = measurement.layout_as_root(
            size(AvailableSpace::MaxContent, AvailableSpace::MinContent),
            window,
            cx,
        );
        let index = self.index;
        let opacity = gpui_kit::base::motion::transition(
            (index, "keypad-secondary-opacity"),
            if self.shown { 1. } else { 0. },
            gpui_kit::base::motion::Transition::new(Duration::from_millis(300))
                .easing(gpui_kit::base::motion::Easing::Ease),
            window,
            cx,
        );
        let owner_tip = self.owner.clone();
        let owner_trigger = self.owner.clone();
        let tips = self.tip_bounds.clone();
        let triggers = self.trigger_bounds.clone();
        let tip = tip_surface(self.label, self.kind, self.value)
            .id(("keypad-secondary-tip", index))
            .w(size.width)
            .opacity(opacity)
            .when(!self.shown, |tip| tip.invisible())
            .on_prepaint(move |rect, _, _| {
                tips.borrow_mut().insert(index, rect);
            })
            .on_hover(move |entered, window, cx| {
                hover(&owner_tip, index, *entered, true, size, window, cx)
            })
            .into_any_element();
        div()
            .id(("keypad-secondary-marker", index))
            .absolute()
            .right_0()
            .top(surface::css(15.))
            .ml(surface::css(-30.))
            .size(surface::css(20.))
            .on_prepaint(move |rect, _, _| {
                triggers.borrow_mut().insert(index, rect);
            })
            .when(!self.disabled, |marker| {
                marker.on_hover(move |entered, window, cx| {
                    hover(&owner_trigger, index, *entered, false, size, window, cx)
                })
            })
            .child(img(self.asset).size_full())
            .child(
                deferred(header::tip_position(
                    self.position,
                    point(px(0.), px(0.)),
                    false,
                    tip,
                ))
                // Source marker z1e17 is above the dropdown's local z102,
                // while both remain inside the Keypad drawer's z10 context.
                .with_priority(14),
            )
    }
}
