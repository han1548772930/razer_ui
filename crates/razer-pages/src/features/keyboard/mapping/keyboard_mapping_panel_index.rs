//! The actual ButtonPanel index helper, with its sticky overflow state.
use super::*;
use std::{cell::RefCell, collections::BTreeSet, time::Duration};

#[derive(Deserialize)]
struct Product {
    product_id: u32,
    split_min_utf16: usize,
}
fn split_min(product_id: u32) -> usize {
    static DATA: OnceLock<Vec<Product>> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("keyboard_mapping_panel_index_data.json"))
            .expect("audited source index thresholds")
    })
    .iter()
    .find(|row| row.product_id == product_id)
    .expect("current ButtonPanel product")
    .split_min_utf16
}

#[derive(Default)]
pub(super) struct State {
    sticky: Rc<RefCell<BTreeSet<usize>>>,
    hovered: Option<usize>,
    bounds: Rc<RefCell<BTreeMap<usize, Bounds<Pixels>>>>,
    tip_bounds: Rc<RefCell<BTreeMap<usize, Bounds<Pixels>>>>,
}

#[derive(IntoElement)]
pub(super) struct Index {
    pub(super) label: String,
    pub(super) product_id: u32,
    pub(super) index: usize,
    pub(super) owner: WeakEntity<KeyboardProductWorkspace>,
    pub(super) sticky: Rc<RefCell<BTreeSet<usize>>>,
    pub(super) bounds: Rc<RefCell<BTreeMap<usize, Bounds<Pixels>>>>,
    pub(super) tip_bounds: Rc<RefCell<BTreeMap<usize, Bounds<Pixels>>>>,
    pub(super) open: bool,
    pub(super) disabled: bool,
    pub(super) tooltip_priority: usize,
}
impl Index {
    pub(super) fn new(
        workspace: &KeyboardProductWorkspace,
        index: usize,
        label: String,
        disabled: bool,
        cx: &Context<KeyboardProductWorkspace>,
    ) -> Self {
        let state = &workspace.analog_gamepad.mapping.panel_index;
        Self {
            label,
            product_id: workspace.spec.product_id,
            index,
            owner: cx.entity().downgrade(),
            sticky: state.sticky.clone(),
            bounds: state.bounds.clone(),
            tip_bounds: state.tip_bounds.clone(),
            open: state.hovered == Some(index),
            disabled,
            tooltip_priority: if keypad_panel::supported(workspace.spec.product_id) {
                11
            } else {
                151
            },
        }
    }
}
fn tip_text() -> Div {
    div()
        .px(surface::css(10.))
        .py(surface::css(8.))
        .border_1()
        .border_color(rgb(0x5d5d5d))
        .bg(rgb(0x000000))
        .text_color(rgb(0xcccccc))
        .font_family("Roboto")
        .text_size(surface::css(14.))
        .line_height(surface::css(16.))
        .text_left()
}
fn hover(
    owner: &WeakEntity<KeyboardProductWorkspace>,
    index: usize,
    entered: bool,
    tip: bool,
    window: &Window,
    cx: &mut App,
) {
    let _ = owner.update(cx, |this, cx| {
        let state = &mut this.analog_gamepad.mapping.panel_index;
        let other = if tip { &state.bounds } else { &state.tip_bounds };
        let inside = entered
            || (state.hovered == Some(index)
                && other
                    .borrow()
                    .get(&index)
                    .is_some_and(|bounds| bounds.contains(&window.mouse_position())));
        let next = if inside {
            Some(index)
        } else if state.hovered == Some(index) {
            None
        } else {
            state.hovered
        };
        if next != state.hovered {
            state.hovered = next;
            cx.notify()
        }
    });
}
impl RenderOnce for Index {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let index = self.index;
        let slash = self.label.contains('/');
        let split = self
            .label
            .split(if slash { '/' } else { ' ' })
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let parts = if split
            .iter()
            .map(|part| part.encode_utf16().count())
            .sum::<usize>()
            >= split_min(self.product_id)
        {
            split
        } else {
            vec![self.label.clone()]
        };
        let mut measurement = tip_text().child(self.label.clone()).into_any_element();
        let preferred = measurement.layout_as_root(
            size(AvailableSpace::MaxContent, AvailableSpace::MinContent),
            window,
            cx,
        );
        let mut minimum = tip_text().child(self.label.clone()).into_any_element();
        let min = minimum.layout_as_root(
            size(AvailableSpace::MinContent, AvailableSpace::MinContent),
            window,
            cx,
        );
        // Inline-block shrink-to-fit in the source 200px tip. scrollWidth
        // includes padding, and excludes its two 1px borders.
        let tip_width = preferred.width.min(
            surface::css(200.)
                .to_pixels(window.rem_size())
                .max(min.width),
        );
        let ratio = f32::from((tip_width - px(2.)).max(px(0.)))
            / f32::from(surface::css(56.).to_pixels(window.rem_size()));
        let font = if ratio <= 1. {
            14.
        } else if ratio <= 1.125 {
            13.
        } else {
            10.
        };
        let newly_sticky = ratio > 1.125
            && parts.len() > 1
            && parts.iter().any(|part| part.encode_utf16().count() > 9);
        let sticky = self.sticky.borrow().contains(&index) || newly_sticky;
        let allow_hover = (sticky || slash) && !self.disabled;
        let trigger = Rc::new(Cell::new(Bounds::default()));
        let tip_bounds = self.tip_bounds.clone();
        let owner = self.owner.clone();
        let opacity = gpui_kit::base::motion::transition(
            (index, "keyboard-panel-index-tip"),
            if self.open && allow_hover { 1. } else { 0. },
            gpui_kit::base::motion::Transition::new(Duration::from_millis(300))
                .easing(gpui_kit::base::motion::Easing::Linear),
            window,
            cx,
        );
        let tooltip = div()
            .w(surface::css(200.))
            .opacity(opacity)
            .when(!self.open || !allow_hover, |tip| tip.invisible())
            .child(
                tip_text()
                    .id(("keyboard-panel-index-tip-surface", index))
                    .w(tip_width)
                    .on_prepaint(move |rect, _, _| {
                        tip_bounds.borrow_mut().insert(index, rect);
                    })
                    .on_hover(move |entered, window, cx| {
                        hover(&owner, index, *entered, true, window, cx)
                    })
                    .child(self.label),
            )
            .into_any_element();
        let sticky_cache = self.sticky;
        let bounds = self.bounds;
        let owner = self.owner;
        let observed_trigger = trigger.clone();
        div()
            .id(("keyboard-panel-index", index))
            .w_full()
            .on_prepaint(move |rect, _, _| {
                observed_trigger.set(rect);
                bounds.borrow_mut().insert(index, rect);
                if newly_sticky {
                    sticky_cache.borrow_mut().insert(index);
                }
            })
            .when(allow_hover, |index_view| {
                index_view.on_hover(move |entered, window, cx| {
                    hover(&owner, index, *entered, false, window, cx)
                })
            })
            .children(parts.into_iter().map(|part| {
                div()
                    .text_size(surface::css(font))
                    .when(sticky, |part| {
                        part.overflow_hidden().whitespace_nowrap().text_ellipsis()
                    })
                    .child(part)
            }))
            .child(
                deferred(header::tip_position(
                    trigger,
                    point(
                        surface::css(20.).to_pixels(window.rem_size()),
                        surface::css(35.).to_pixels(window.rem_size()),
                    ),
                    false,
                    tooltip,
                ))
                // Source local z100 is inside drawer z150, above the
                // independent common popup z103. Flatten that order for GPUI.
                .with_priority(self.tooltip_priority),
            )
    }
}
