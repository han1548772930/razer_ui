//! 4130 bi/be: source card geometry, short-drag actions and per-group order.
//! GPUI owns drag capture/release, focus, keyboard activation and hit testing.
use crate::{preferences::DashboardPreferences, ui::surface::css};
use gpui_kit::base::{
    ElementExt as _, StyledExt as _, TestSupportExt as _,
    motion::{self, Easing, Transition},
};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::{cell::RefCell, rc::Rc, time::Duration};

#[cfg(test)]
#[path = "dashboard_grid_tests.rs"]
mod tests;

type Action = Rc<dyn Fn(&mut Window, &mut App)>;

pub(in crate::shell) struct DashboardState {
    preferences: DashboardPreferences,
    saved: DashboardPreferences,
    press: Option<Press>,
    release: Option<Subscription>,
}
pub(in crate::shell) struct DashboardChanged;
impl EventEmitter<DashboardChanged> for DashboardState {}

impl DashboardState {
    pub(in crate::shell) fn new(preferences: DashboardPreferences) -> Self {
        Self {
            saved: preferences.clone(),
            preferences,
            press: None,
            release: None,
        }
    }
    pub(in crate::shell) fn snapshot(&self) -> DashboardPreferences {
        self.preferences.clone()
    }
    pub(in crate::shell) fn pending(&self) -> bool {
        self.preferences != self.saved
    }
    pub(in crate::shell) fn mark_saved(&mut self, captured: DashboardPreferences) {
        self.saved = captured;
    }
    pub(in crate::shell) fn collapsed(&self, group: &str) -> bool {
        self.preferences
            .groups_collapsed
            .get(group)
            .copied()
            .unwrap_or(false)
    }
    pub(in crate::shell) fn toggle(&mut self, group: &str, cx: &mut Context<Self>) {
        self.preferences
            .groups_collapsed
            .insert(group.into(), !self.collapsed(group));
        cx.emit(DashboardChanged);
        cx.notify();
    }
    fn order(&self, group: &str, available: &[String]) -> Vec<String> {
        let saved = self.preferences.items_order.get(group);
        let mut order = available.to_vec();
        // Stable sort: known IDs retain their saved position; new IDs append.
        order.sort_by_key(|id| {
            saved
                .and_then(|saved| saved.iter().position(|key| key == id))
                .unwrap_or(usize::MAX)
        });
        order
    }
    fn move_to(&mut self, pointer: Point<Pixels>, window: &Window, cx: &mut Context<Self>) {
        let Some(press) = self.press.as_mut() else {
            return;
        };
        let metrics = press.metrics.borrow();
        if !metrics.items.contains(&press.card) {
            return;
        }
        let scale = f32::from(window.rem_size()) / 16.;
        let delta = (pointer - metrics.bounds.origin) / scale - press.pointer;
        let next = clamp_position(
            press.initial + point(f32::from(delta.x), f32::from(delta.y)),
            metrics.columns,
            metrics.items.len(),
        );
        if (next.x - press.initial.x).abs() > 10. || (next.y - press.initial.y).abs() > 10. {
            press.short = false;
        }
        press.position = next;
        let group = press.group;
        let card = press.card.clone();
        let columns = metrics.columns;
        let available = metrics.items.clone();
        drop(metrics);
        let mut order = self.order(group, &available);
        let target = target_index(next, columns, order.len());
        if let Some(current) = order.iter().position(|id| *id == card) {
            if current != target {
                let card = order.remove(current);
                order.insert(target, card);
                self.preferences.items_order.insert(group.into(), order);
            }
        }
        cx.notify();
    }
    fn finish(
        &mut self,
        released: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Action> {
        let press = self.press.take()?;
        self.release = None;
        let still_present = press.metrics.borrow().items.contains(&press.card);
        if !released || !still_present {
            // Native cancellation must not launch the grabbed card. Restore only
            // this group's order; unrelated edits made during the drag survive.
            match press.previous_saved_order {
                Some(order) => {
                    self.preferences
                        .items_order
                        .insert(press.group.into(), order);
                }
                None => {
                    self.preferences.items_order.remove(press.group);
                }
            }
            if self.preferences.items_order.get(press.group)
                != self.saved.items_order.get(press.group)
            {
                cx.emit(DashboardChanged);
            }
        } else if self.preferences.items_order.get(press.group)
            != press.previous_saved_order.as_ref()
        {
            cx.emit(DashboardChanged);
        }
        window.refresh();
        cx.notify();
        (released && still_present && press.short)
            .then_some(press.action)
            .flatten()
    }
}

#[derive(Default)]
struct Metrics {
    bounds: Bounds<Pixels>,
    columns: usize,
    items: Vec<String>,
}
struct Press {
    group: &'static str,
    card: String,
    initial: Point<f32>,
    position: Point<f32>,
    pointer: Point<Pixels>,
    metrics: Rc<RefCell<Metrics>>,
    short: bool,
    session: Option<EntityId>,
    action: Option<Action>,
    previous_saved_order: Option<Vec<String>>,
}
#[derive(Clone)]
struct DragCard {
    group: &'static str,
    card: String,
}
struct DragPreview;
impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

fn slot(index: usize, columns: usize) -> Point<f32> {
    point(
        (index % columns) as f32 * 310.,
        (index / columns) as f32 * 240.,
    )
}
fn clamp_position(position: Point<f32>, columns: usize, count: usize) -> Point<f32> {
    point(
        position.x.clamp(0., (columns - 1) as f32 * 310.),
        position
            .y
            .clamp(0., (count.max(1).div_ceil(columns) - 1) as f32 * 240.),
    )
}
fn target_index(position: Point<f32>, columns: usize, count: usize) -> usize {
    let column = ((position.x - 145.) / 310.).floor() as isize + 1;
    let row = ((position.y - 110.) / 240.).floor() as isize + 1;
    (column + row * columns as isize)
        .max(0)
        .min(count.saturating_sub(1) as isize) as usize
}

pub(super) struct DashboardCard {
    id: String,
    label: SharedString,
    content: AnyElement,
    action: Option<Action>,
    url: Option<&'static str>,
    focus: Option<FocusHandle>,
    draggable: bool,
}
impl DashboardCard {
    pub(super) fn button(
        id: impl Into<String>,
        label: impl Into<SharedString>,
        content: impl IntoElement,
        action: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            content: content.into_any_element(),
            action: Some(Rc::new(action)),
            url: None,
            focus: None,
            draggable: true,
        }
    }
    pub(super) fn link(
        id: impl Into<String>,
        label: impl Into<SharedString>,
        url: &'static str,
        content: impl IntoElement,
    ) -> Self {
        let mut card = Self::button(id, label, content, move |_, cx| cx.open_url(url));
        card.url = Some(url);
        card
    }
    pub(super) fn unavailable(mut self, unavailable: bool) -> Self {
        if unavailable {
            self.action = None;
        }
        self
    }
    pub(super) fn focus(mut self, focus: FocusHandle) -> Self {
        self.focus = Some(focus);
        self
    }
    pub(super) fn empty(content: impl IntoElement) -> Self {
        Self {
            id: "noDevice".into(),
            label: "".into(),
            content: content.into_any_element(),
            action: None,
            url: None,
            focus: None,
            draggable: false,
        }
    }
}

#[derive(IntoElement)]
pub(super) struct DashboardGrid {
    group: &'static str,
    state: Entity<DashboardState>,
    columns: usize,
    cards: Vec<DashboardCard>,
}
impl DashboardGrid {
    pub(super) fn new(
        group: &'static str,
        state: &Entity<DashboardState>,
        width: f32,
        cards: Vec<DashboardCard>,
    ) -> Self {
        Self {
            group,
            state: state.clone(),
            columns: ((width + 20.) / 310.).round().max(1.) as usize,
            cards,
        }
    }
}

impl RenderOnce for DashboardGrid {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let group = self.group;
        let state = self.state;
        let columns = self.columns;
        let metrics = window
            .use_keyed_state((ElementId::from(group), "grid-metrics"), cx, |_, _| {
                Rc::new(RefCell::new(Metrics::default()))
            })
            .read(cx)
            .clone();
        let available = self
            .cards
            .iter()
            .map(|card| card.id.clone())
            .collect::<Vec<_>>();
        metrics.borrow_mut().columns = columns;
        metrics.borrow_mut().items = available.clone();
        let order = state.read(cx).order(group, &available);
        let height = 220. + 240. * (self.cards.len().max(1).div_ceil(columns) - 1) as f32;
        let measurement = metrics.clone();
        let release = state.clone();
        let release_out = state.clone();
        let wheel = state.clone();
        let paint_queue = Rc::new(RefCell::new(Vec::new()));
        let flush = paint_queue.clone();
        let count = self.cards.len();
        div()
            .id(SharedString::from(format!("{group}-grid")))
            .test_support()
            .relative()
            .w_full()
            .h(css(height))
            .on_prepaint(move |bounds, _, _| measurement.borrow_mut().bounds = bounds)
            .on_drag_move(window.listener_for(
                &state,
                move |state, event: &DragMoveEvent<DragCard>, window, cx| {
                    if event.drag(cx).group == group
                        && state
                            .press
                            .as_ref()
                            .is_some_and(|press| press.card == event.drag(cx).card)
                    {
                        state.move_to(event.event.position, window, cx);
                    }
                },
            ))
            .on_mouse_move(window.listener_for(
                &state,
                move |state, event: &MouseMoveEvent, window, cx| {
                    // Source starts moving on the first mousemove. Keep sub-2px
                    // motion too; GPUI takes over capture once its drag starts.
                    if state
                        .press
                        .as_ref()
                        .is_some_and(|press| press.group == group && press.session.is_none())
                    {
                        state.move_to(event.position, window, cx);
                    }
                },
            ))
            .on_mouse_up(MouseButton::Left, move |_, window, cx| {
                finish_pointer(&release, group, window, cx)
            })
            .on_mouse_up_out(MouseButton::Left, move |_, window, cx| {
                finish_pointer(&release_out, group, window, cx)
            })
            .child(
                canvas(
                    |_, _, _| {},
                    move |_, _, window, cx| {
                        if !wheel
                            .read(cx)
                            .press
                            .as_ref()
                            .is_some_and(|press| press.group == group)
                        {
                            return;
                        }
                        let wheel = wheel.clone();
                        // bi installs a document wheel listener for the whole press,
                        // including outside the group and before GPUI starts a drag.
                        window.on_mouse_event(
                            move |event: &ScrollWheelEvent, phase, window, cx| {
                                if !phase.capture()
                                    || !wheel
                                        .read(cx)
                                        .press
                                        .as_ref()
                                        .is_some_and(|press| press.group == group)
                                {
                                    return;
                                }
                                // GPUI scroll deltas move the content; DOM WheelEvent
                                // deltaX/Y have the opposite sign (positive scrolls down).
                                let pointer =
                                    event.position - event.delta.pixel_delta(window.line_height());
                                wheel.update(cx, |state, cx| state.move_to(pointer, window, cx));
                                window.prevent_default();
                                cx.stop_propagation();
                            },
                        );
                    },
                )
                .absolute()
                .size_0(),
            )
            .children(self.cards.into_iter().map(|card| {
                let index = order.iter().position(|id| *id == card.id).unwrap_or(0);
                let initial = slot(index, columns);
                let dragging = state
                    .read(cx)
                    .press
                    .as_ref()
                    .filter(|press| press.group == group && press.card == card.id)
                    .map(|press| press.position);
                let target = dragging.unwrap_or(initial);
                let scale = f32::from(window.rem_size()) / 16.;
                let position = motion::transition(
                    (
                        SharedString::from(format!("{group}-{}", card.id)),
                        "position",
                    ),
                    point(px(target.x * scale), px(target.y * scale)),
                    Transition::new(if dragging.is_some() {
                        Duration::ZERO
                    } else {
                        Duration::from_millis(300)
                    })
                    .easing(Easing::Ease),
                    window,
                    cx,
                );
                let id: SharedString = card.id.clone().into();
                let highlight =
                    card.draggable && !matches!(card.id.as_str(), "synapse2" | "inDevelopment");
                let border = div()
                    .id("card-highlight")
                    .test_support()
                    .absolute()
                    .inset_0()
                    .border_2()
                    .rounded(css(5.))
                    .border_color(cx.theme().transparent)
                    .when(highlight, |view| {
                        view.group_hover(id.clone(), |s| {
                            s.border_color(cx.theme().primary.opacity(77. / 255.))
                        })
                        .group_active(id.clone(), |s| s.border_color(cx.theme().primary))
                    })
                    .when(dragging.is_some() && highlight, |view| {
                        view.border_color(cx.theme().primary)
                    });
                let style = StyleRefinement::default()
                    .absolute()
                    .left(position.x)
                    .top(position.y)
                    .w(css(290.))
                    .h(css(220.))
                    .rounded(css(5.))
                    .cursor_default()
                    .bg(cx.theme().group_box);
                if !card.draggable {
                    return div()
                        .id(id)
                        .refine_style(&style)
                        .child(card.content)
                        .into_any_element();
                }
                let action = card.action.clone();
                let frame = if let Some(url) = card.url {
                    let link = gpui_kit::base::Link::new(id.clone())
                        .href(url)
                        .accessibility_label(card.label)
                        .open_with(move |_, event, window, cx| {
                            if !matches!(event, ClickEvent::Mouse(_)) {
                                if let Some(action) = &action {
                                    action(window, cx);
                                }
                            }
                        })
                        .child(card.content)
                        .child(border)
                        .refine_style(&style);
                    wire_card(
                        link,
                        group,
                        id,
                        &state,
                        metrics.clone(),
                        initial,
                        card.action,
                        window,
                        cx,
                    )
                    .into_any_element()
                } else {
                    let button = gpui_kit::base::Button::new(id.clone())
                        .accessibility_label(card.label)
                        .when_some(card.focus, |button, focus| button.track_focus(&focus))
                        .focusable(card.action.is_some())
                        .tab_stop(card.action.is_some())
                        .on_click(move |event, window, cx| {
                            if !matches!(event, ClickEvent::Mouse(_)) {
                                if let Some(action) = &action {
                                    action(window, cx);
                                }
                            }
                        })
                        .child(card.content)
                        .child(border)
                        .refine_style(&style);
                    wire_card(
                        button,
                        group,
                        id,
                        &state,
                        metrics.clone(),
                        initial,
                        card.action,
                        window,
                        cx,
                    )
                    .into_any_element()
                };
                LayeredCard {
                    child: frame,
                    rank: if dragging.is_some() {
                        100
                    } else {
                        count - index + 1
                    },
                    queue: paint_queue.clone(),
                }
                .into_any_element()
            }))
            .child(
                canvas(
                    move |_, window, _| {
                        // Flush a local CSS stacking order at one global layer below
                        // popovers, while keeping each card's layout/identity unchanged.
                        let mut cards = std::mem::take(&mut *flush.borrow_mut());
                        cards.sort_by_key(|card: &CardPaint| card.rank);
                        for card in cards {
                            window.defer_draw(card.child, card.offset, 20, Some(card.mask));
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_0(),
            )
    }
}

fn finish_pointer(
    state: &Entity<DashboardState>,
    group: &'static str,
    window: &mut Window,
    cx: &mut App,
) {
    if !state
        .read(cx)
        .press
        .as_ref()
        .is_some_and(|press| press.group == group)
    {
        return;
    }
    let action = state.update(cx, |state, cx| state.finish(true, window, cx));
    if let Some(action) = action {
        action(window, cx);
    }
}

fn wire_card<T: StatefulInteractiveElement + Styled>(
    control: T,
    group: &'static str,
    id: SharedString,
    state: &Entity<DashboardState>,
    metrics: Rc<RefCell<Metrics>>,
    initial: Point<f32>,
    action: Option<Action>,
    window: &mut Window,
    cx: &mut App,
) -> T {
    let press_id = id.to_string();
    let payload = DragCard {
        group,
        card: press_id.clone(),
    };
    let start = state.clone();
    control
        .group(id)
        .focus_visible(|style| style.border_2().border_color(cx.theme().primary))
        .on_mouse_down(
            MouseButton::Left,
            window.listener_for(state, move |state, event: &MouseDownEvent, window, cx| {
                state.press = Some(Press {
                    group,
                    card: press_id.clone(),
                    initial,
                    position: initial,
                    pointer: (event.position - metrics.borrow().bounds.origin)
                        / (f32::from(window.rem_size()) / 16.),
                    metrics: metrics.clone(),
                    short: true,
                    session: None,
                    action: action.clone(),
                    previous_saved_order: state.preferences.items_order.get(group).cloned(),
                });
                cx.notify();
            }),
        )
        .on_drag(payload, move |payload, _, window, cx| {
            let preview = cx.new(|_| DragPreview);
            let session = preview.entity_id();
            start.update(cx, |state, cx| {
                if !state
                    .press
                    .as_ref()
                    .is_some_and(|press| press.group == payload.group && press.card == payload.card)
                {
                    return;
                }
                state.press.as_mut().unwrap().session = Some(session);
                state.move_to(window.mouse_position(), window, cx);
                state.release =
                    Some(
                        cx.observe_release_in(&preview, window, move |state, _, window, cx| {
                            if state
                                .press
                                .as_ref()
                                .is_some_and(|press| press.session == Some(session))
                            {
                                state.finish(false, window, cx);
                            }
                        }),
                    );
            });
            preview
        })
}

/// Local z-index ordering preserves page/collapse clipping. Ordinary deferred()
/// escapes the clip; globally assigning card ranks would also cover popovers.
struct CardPaint {
    rank: usize,
    child: AnyElement,
    offset: Point<Pixels>,
    mask: ContentMask<Pixels>,
}
struct LayeredCard {
    child: AnyElement,
    rank: usize,
    queue: Rc<RefCell<Vec<CardPaint>>>,
}
impl IntoElement for LayeredCard {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for LayeredCard {
    type RequestLayoutState = ();
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
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        _: &mut App,
    ) {
        let child = std::mem::replace(&mut self.child, Empty.into_any_element());
        self.queue.borrow_mut().push(CardPaint {
            child,
            rank: self.rank,
            offset: window.element_offset(),
            mask: window.content_mask(),
        });
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        _: &mut Window,
        _: &mut App,
    ) {
    }
}
