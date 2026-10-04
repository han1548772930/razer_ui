//! Source dropdown presentations over Kit's select, popup and list behavior.
//!
//! SelectState remains the committed value and event source. The owner supplies its current
//! options because Kit 0.7 does not expose SelectState's delegate or its menu renderer.
use gpui_kit::base::motion::{self, Easing, Presence, Sequence, Transition};
use gpui_kit::base::{DeferredPopover, GlobalState, Popover, Popup, Select as BaseSelect};
use gpui_kit::component::{
    ActiveTheme, Disableable, ElementExt, Icon, IndexPath, Selectable, StyledExt,
    list::{List, ListDelegate, ListState},
    select::{SelectEvent, SelectItem, SelectState},
};
use gpui_kit::{prelude::FluentBuilder as _, *};

use super::{
    surface::css,
    theme::{AlexaColors, DropdownColors},
};
use std::time::Duration;

#[derive(Clone, Copy, Default, PartialEq)]
enum Presentation {
    #[default]
    Synapse,
    Alexa,
}

impl Presentation {
    fn row_height(self) -> f32 {
        match self {
            Self::Synapse => 25.,
            // `.dropdown-item`: 14px * 1.36 line-height + 4px top/bottom padding.
            Self::Alexa => 14. * 1.36 + 8.,
        }
    }

    fn max_height(self) -> f32 {
        match self {
            Self::Synapse => 180.,
            Self::Alexa => 110.,
        }
    }
}

pub(crate) fn select<I: SelectItem<Value = String> + 'static>(
    state: &Entity<SelectState<Vec<I>>>,
) -> SynapseSelect<I> {
    SynapseSelect {
        state: state.clone(),
        id: ("synapse-select", state.entity_id()).into(),
        items: Vec::new(),
        style: StyleRefinement::default(),
        disabled: false,
        label: None,
        placeholder: None,
        presentation: Presentation::Synapse,
    }
}

/// Alexa's `.dropdown-selector/.dropdown-razer-2`, using the same committed state.
pub(crate) fn select_alexa<I: SelectItem<Value = String> + 'static>(
    state: &Entity<SelectState<Vec<I>>>,
) -> SynapseSelect<I> {
    SynapseSelect {
        presentation: Presentation::Alexa,
        ..select(state)
    }
}

#[derive(IntoElement)]
pub(crate) struct SynapseSelect<I: SelectItem<Value = String> + 'static> {
    state: Entity<SelectState<Vec<I>>>,
    id: ElementId,
    items: Vec<I>,
    style: StyleRefinement,
    disabled: bool,
    label: Option<SharedString>,
    placeholder: Option<SharedString>,
    presentation: Presentation,
}

impl<I: SelectItem<Value = String> + 'static> SynapseSelect<I> {
    pub(crate) fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }

    /// Supply the same current options used by the owning feature's SelectState.
    pub(crate) fn items(mut self, items: Vec<I>) -> Self {
        self.items = items;
        self
    }

    pub(crate) fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Text shown while no domain value is selected; never added to the menu.
    pub(crate) fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }
}

impl<I: SelectItem<Value = String> + 'static> Disableable for SynapseSelect<I> {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl<I: SelectItem<Value = String> + 'static> Styled for SynapseSelect<I> {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

struct SynapseSelectView<I: SelectItem<Value = String> + 'static> {
    state: Entity<SelectState<Vec<I>>>,
    list: Entity<ListState<Options<I>>>,
    id: ElementId,
    style: StyleRefinement,
    disabled: bool,
    label: Option<SharedString>,
    placeholder: Option<SharedString>,
    presentation: Presentation,
    open: bool,
    hovered: bool,
    width: Pixels,
    _popup_context: Option<DeferredPopover>,
    _state_observer: Subscription,
}

impl<I: SelectItem<Value = String> + 'static> SynapseSelectView<I> {
    fn new(
        state: Entity<SelectState<Vec<I>>>,
        id: ElementId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let view = cx.entity().downgrade();
        let list = cx.new(|cx| {
            ListState::new(
                Options {
                    items: Vec::new(),
                    committed: None,
                    cursor: None,
                    presentation: Presentation::Synapse,
                    view,
                },
                window,
                cx,
            )
        });
        Self {
            _state_observer: cx.observe(&state, |view, state, cx| {
                let selected = state.read(cx).selected_value().cloned();
                view.list.update(cx, |list, cx| {
                    if list.delegate().committed != selected {
                        list.delegate_mut().committed = selected;
                        cx.notify();
                    }
                });
                cx.notify();
            }),
            state,
            list,
            id,
            style: StyleRefinement::default(),
            disabled: false,
            label: None,
            placeholder: None,
            presentation: Presentation::Synapse,
            open: false,
            hovered: false,
            width: px(0.),
            _popup_context: None,
        }
    }

    /// Glue the controlled Base Select and Popover to the framework List cursor.
    fn set_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        let open = open && !self.disabled;
        if self.open == open {
            return;
        }
        self.open = open;
        self._popup_context = (self.presentation == Presentation::Alexa && open)
            .then(|| GlobalState::register_deferred_popover(cx));
        if open {
            let selected = self.state.read(cx).selected_value().cloned();
            self.list.update(cx, |list, cx| {
                let index = list
                    .delegate()
                    .items
                    .iter()
                    .position(|item| Some(item.value()) == selected.as_ref())
                    .map(IndexPath::new);
                list.set_selected_index(index, window, cx);
                list.scroll_to_selected_item(window, cx);
            });
            if self.presentation == Presentation::Alexa {
                self.list.focus_handle(cx).focus(window, cx);
            }
        } else {
            self.state.update(cx, |state, cx| {
                state.focus(window, cx);
                cx.emit(DismissEvent);
            });
        }
        cx.notify();
    }

    fn confirm(&mut self, value: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || !self.open {
            return;
        }
        self.state.update(cx, |state, cx| {
            state.set_selected_value(&value, window, cx);
            cx.emit(SelectEvent::Confirm(state.selected_value().cloned()));
            cx.notify();
        });
        self.set_open(false, window, cx);
    }

    fn sync_items(&mut self, items: Vec<I>, window: &mut Window, cx: &mut Context<Self>) {
        let committed = self.state.read(cx).selected_value().cloned();
        let open = self.open;
        let presentation = self.presentation;
        let changed = self.list.update(cx, |list, cx| {
            let previous = list.delegate();
            let items_changed = previous.items.len() != items.len()
                || previous.items.iter().zip(&items).any(|(old, new)| {
                    old.value() != new.value()
                        || old.title() != new.title()
                        || old.disabled() != new.disabled()
                });
            if !items_changed
                && previous.committed == committed
                && previous.presentation == presentation
            {
                return false;
            }

            // A positional cursor must follow its value when rows before it are
            // inserted or removed. If that value disappeared, use the owner's
            // surviving committed value; no value is committed during this sync.
            let cursor_value = list
                .selected_index()
                .and_then(|index| previous.items.get(index.row))
                .map(|item| item.value().clone());
            let cursor = cursor_value
                .as_ref()
                .and_then(|value| items.iter().position(|item| item.value() == value))
                .or_else(|| {
                    committed
                        .as_ref()
                        .and_then(|value| items.iter().position(|item| item.value() == value))
                })
                .map(IndexPath::new);
            list.delegate_mut().items = items;
            list.delegate_mut().committed = committed;
            list.delegate_mut().presentation = presentation;
            if items_changed && list.selected_index() != cursor {
                list.set_selected_index(cursor, window, cx);
                if open && let Some(cursor) = cursor {
                    list.scroll_to_item(cursor, ScrollStrategy::Nearest, window, cx);
                }
            }
            // ListState::render remeasures rows and rebuilds its cache when the
            // row count changes. Notify only on changed presentation or selection,
            // so an ordinary parent redraw does not restart navigation or redraw.
            cx.notify();
            true
        });
        if changed {
            cx.notify();
        }
    }
}

impl<I: SelectItem<Value = String> + 'static> RenderOnce for SynapseSelect<I> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let view = window.use_keyed_state(self.id.clone(), cx, |window, cx| {
            SynapseSelectView::new(self.state, self.id, window, cx)
        });
        view.update(cx, |view, cx| {
            view.style = self.style;
            view.label = self.label;
            view.placeholder = self.placeholder;
            view.disabled = self.disabled;
            view.presentation = self.presentation;
            if self.disabled {
                view.set_open(false, window, cx);
            }
            view.sync_items(self.items, window, cx);
        });
        view
    }
}

impl<I: SelectItem<Value = String> + 'static> Render for SynapseSelectView<I> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus = self.state.focus_handle(cx);
        let content_focus = self.list.focus_handle(cx);
        let options = self.list.read(cx).delegate();
        let selected = self.state.read(cx).selected_value();
        let title = options
            .items
            .iter()
            .find(|item| Some(item.value()) == selected)
            .map(SelectItem::title)
            .unwrap_or_else(|| self.placeholder.clone().unwrap_or_default());
        let presentation = self.presentation;
        let alexa = presentation == Presentation::Alexa;
        let menu_height = (options.items.len() as f32 * presentation.row_height() + 2.)
            .min(presentation.max_height());
        let width = self.width;
        let list = self.list.clone();
        let open_change = cx.entity().downgrade();
        let popup_change = cx.entity().downgrade();
        let focused = focus.is_focused(window);
        let open = self.open;
        let colors = DropdownColors::new();
        let border = if !self.disabled && (open || focused || (alexa && self.hovered)) {
            cx.theme().primary
        } else if alexa {
            cx.theme().border
        } else {
            cx.theme().input
        };
        let border = if alexa {
            motion::transition(
                (self.id.clone(), "alexa-trigger-border"),
                border,
                Transition::new(Duration::from_millis(100)).easing(Easing::EaseInOut),
                window,
                cx,
            )
        } else {
            border
        };
        // `$p` first mounts at height 0, waits 100ms, then sets the final height.
        // Its close path sets height 0 immediately and unmounts after 100ms.
        // Sequence retains the previous timing when retargeted during the delay
        // or animation, so reversing does not jump to the other curve's value.
        let alexa_reveal = alexa.then(|| {
            Sequence::new((self.id.clone(), "alexa-options-height"), 0.)
                .with_step(
                    if open { 1. } else { 0. },
                    Transition::new(Duration::from_millis(100))
                        .delay(if open {
                            Duration::from_millis(100)
                        } else {
                            Duration::ZERO
                        })
                        .easing(Easing::EaseInOut),
                )
                .sample(window, cx)
        });
        let trigger = div()
            .id("input")
            .test_support()
            .w_full()
            .h(css(25.))
            .flex()
            .items_center()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .pl(css(if alexa { 6. } else { 4. }))
                    .truncate()
                    .child(title.clone()),
            )
            .child(
                div()
                    .w(css(if alexa { 30. } else { 29. }))
                    .h_full()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::default()
                            .path("synapse/expand.svg")
                            .size(css(10.))
                            .when(alexa, |this| this.text_color(AlexaColors::dropdown_arrow()))
                            .transform(Transformation::rotate(radians(motion::transition(
                                (self.id.clone(), "arrow-angle"),
                                if open { std::f32::consts::PI } else { 0. },
                                if alexa {
                                    Transition::new(Duration::from_millis(100))
                                        .easing(Easing::EaseInOut)
                                } else {
                                    Transition::new(Duration::from_millis(300)).easing(Easing::Ease)
                                },
                                window,
                                cx,
                            )))),
                    ),
            );

        div()
            .id("source-dropdown")
            .test_support()
            .h(css(27.))
            .when(alexa, |this| this.w(css(230.)).mb(css(10.)))
            .min_w_0()
            .border_1()
            .border_color(border)
            .text_size(css(14.))
            .line_height(css(if alexa { 26. } else { 17. }))
            .text_color(cx.theme().foreground)
            .when(!self.disabled && !alexa, |this| {
                this.hover(|style| style.border_color(cx.theme().primary))
            })
            .when(alexa, |this| {
                this.on_hover(cx.listener(|view, hovered, _, cx| {
                    view.hovered = *hovered;
                    cx.notify();
                }))
            })
            .when(self.disabled, |this| this.opacity(0.3))
            .refine_style(&self.style)
            .on_prepaint({
                let view = cx.entity().downgrade();
                move |bounds, _, cx| {
                    _ = view.update(cx, |view, _| view.width = bounds.size.width);
                }
            })
            .child(
                BaseSelect::new(self.id.clone())
                    .open(open)
                    .disabled(self.disabled)
                    .focus_handle(&focus)
                    .content_focus_handle(&content_focus)
                    .accessibility_value(title)
                    .when_some(self.label.clone(), |this, label| {
                        this.accessibility_label(label)
                    })
                    .on_open_change(move |open, window, cx| {
                        _ = open_change.update(cx, |view, cx| view.set_open(open, window, cx));
                    })
                    .size_full()
                    .child(if self.disabled {
                        trigger.into_any_element()
                    } else if let Some(reveal) = alexa_reveal {
                        let progress = *reveal.value();
                        let visible = open || progress > 0.;
                        let toggle = cx.entity().downgrade();
                        let dismiss = cx.entity().downgrade();
                        // Base Select retains keyboard/accessible activation and focus.
                        // Popup retains placement and deferred painting while its body
                        // finishes closing, independently of the logical open state.
                        Popup::new(
                            "alexa-options-popup",
                            trigger.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                cx.stop_propagation();
                                _ = toggle.update(cx, |view, cx| {
                                    // An outside-down handler may have closed it
                                    // earlier in this same trigger gesture.
                                    if view.open == open {
                                        view.set_open(!open, window, cx);
                                    }
                                });
                            }),
                        )
                        // One trigger border pixel plus the source's 2px outer gap.
                        .offset(px(1.) + css(2.).to_pixels(window.rem_size()))
                        .size_full()
                        .when(visible, |this| {
                            this.content(
                                // Resolve against the final size throughout the animation.
                                div().w(width).h(css(menu_height)).child(
                                    div()
                                        .id("source-options")
                                        .test_support()
                                        .w(width)
                                        .h(css(menu_height * progress))
                                        .max_h(css(110.))
                                        .ml(-px(1.))
                                        .border_x_1()
                                        .border_t(px(progress))
                                        .border_b(px(progress))
                                        .border_color(cx.theme().border)
                                        .rounded_none()
                                        .bg(colors.surface())
                                        .overflow_hidden()
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation();
                                        })
                                        .when(open, |this| {
                                            this.on_mouse_down_out(move |_, window, cx| {
                                                _ = dismiss.update(cx, |view, cx| {
                                                    view.set_open(false, window, cx);
                                                });
                                            })
                                        })
                                        .when(!open, |this| {
                                            // Exit is presentation only: intercept before the
                                            // retained List can refocus or activate a row.
                                            this.capture_any_mouse_down(|_, _, cx| {
                                                cx.stop_propagation()
                                            })
                                            .capture_any_mouse_up(|_, _, cx| cx.stop_propagation())
                                        })
                                        .child(
                                            List::new(&list)
                                                .p_0()
                                                .h(css(menu_height - 2.))
                                                .scrollbar_visible(open && reveal.is_finished()),
                                        ),
                                ),
                            )
                        })
                        .into_any_element()
                    } else {
                        Popover::new("options-popover")
                            .open(open)
                            .track_focus(&content_focus)
                            // The trigger sits inside the 1px border; 2px reaches the
                            // original dropdown bottom plus `.s3-options { margin-top:1px }`.
                            .offset(px(2.))
                            .size_full()
                            .trigger_with(move |_, _, _| trigger.into_any_element())
                            .on_open_change(move |open, window, cx| {
                                _ = popup_change
                                    .update(cx, |view, cx| view.set_open(*open, window, cx));
                            })
                            .content(move |_, window, cx| {
                                // height:auto; max-height:0 -> 180px, CSS ease.
                                // Closing sets height:0 without a slide or fade.
                                let reveal = Presence::new("options-reveal", true)
                                    .transition(
                                        Transition::new(Duration::from_millis(200))
                                            .easing(Easing::Ease),
                                    )
                                    .sample(window, cx)
                                    .progress;
                                // Resolve placement against the final size, so
                                // a popup near the bottom cannot flip mid-animation.
                                div().w(width).h(css(menu_height)).child(
                                    div()
                                        .id("source-options")
                                        .test_support()
                                        .w(width)
                                        .h(css(menu_height.min(180. * reveal)))
                                        .max_h(css(180.))
                                        .ml(-px(1.))
                                        .border_1()
                                        .border_color(cx.theme().input)
                                        .rounded_none()
                                        .bg(colors.surface())
                                        .overflow_hidden()
                                        .child(List::new(&list).p_0().h(css(menu_height - 2.))),
                                )
                            })
                            .into_any_element()
                    }),
            )
    }
}

/// Only the List's transient cursor lives here. Its value is never the committed selection.
struct Options<I: SelectItem<Value = String> + 'static> {
    items: Vec<I>,
    committed: Option<String>,
    cursor: Option<IndexPath>,
    presentation: Presentation,
    view: WeakEntity<SynapseSelectView<I>>,
}

impl<I: SelectItem<Value = String> + 'static> ListDelegate for Options<I> {
    type Item = OptionRow;

    fn items_count(&self, _: usize, _: &App) -> usize {
        self.items.len()
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let item = self.items.get(ix.row)?;
        Some(OptionRow {
            id: SharedString::from(format!("option-{}", item.value())).into(),
            title: item.title(),
            checked: self.committed.as_ref() == Some(item.value()),
            highlighted: false,
            disabled: item.disabled(),
            presentation: self.presentation,
        })
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) {
        self.cursor = ix;
    }

    fn confirm(&mut self, _: bool, window: &mut Window, cx: &mut Context<ListState<Self>>) {
        let Some(item) = self.cursor.and_then(|ix| self.items.get(ix.row)) else {
            return;
        };
        if item.disabled() {
            return;
        }
        let value = item.value().clone();
        let view = self.view.clone();
        // Release the List borrow before an owner callback can refresh its options.
        window.defer(cx, move |window, cx| {
            _ = view.update(cx, |view, cx| view.confirm(value, window, cx));
        });
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        div()
    }
}

#[derive(IntoElement)]
struct OptionRow {
    id: ElementId,
    title: SharedString,
    checked: bool,
    highlighted: bool,
    disabled: bool,
    presentation: Presentation,
}

impl Selectable for OptionRow {
    fn selected(mut self, selected: bool) -> Self {
        self.highlighted = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.highlighted
    }
}

impl RenderOnce for OptionRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = DropdownColors::new();
        let alexa = self.presentation == Presentation::Alexa;
        let hover = if alexa {
            AlexaColors::dropdown_hover()
        } else {
            colors.hover()
        };
        let hover_motion = alexa.then(|| {
            let state =
                window.use_keyed_state((self.id.clone(), "alexa-row-hover"), cx, |_, _| false);
            let target =
                if !self.disabled && (*state.read(cx) || (self.highlighted && !self.checked)) {
                    hover
                } else {
                    cx.theme().transparent
                };
            let color = motion::transition(
                (self.id.clone(), "alexa-row-background"),
                target,
                Transition::new(Duration::from_millis(100)).easing(Easing::EaseInOut),
                window,
                cx,
            );
            (state, color)
        });
        div()
            .id(self.id)
            .test_support()
            .role(Role::ListBoxOption)
            .aria_label(self.title.clone())
            .aria_selected(self.checked)
            .when(self.highlighted, |this| this.aria_active_descendant())
            .h(css(self.presentation.row_height()))
            .min_h(css(self.presentation.row_height()))
            .w_full()
            .p(css(4.))
            .when(alexa, |this| this.px(css(6.)))
            .rounded_none()
            .text_size(css(14.))
            .line_height(css(if alexa { 14. * 1.36 } else { 17. }))
            .text_color(if self.checked {
                cx.theme().primary
            } else {
                cx.theme().foreground
            })
            .when_some(hover_motion, |this, (state, color)| {
                this.bg(color)
                    .on_hover(window.listener_for(&state, |state, hovered, _, cx| {
                        *state = *hovered;
                        cx.notify();
                    }))
            })
            .when(!self.disabled && !alexa, |this| {
                this.when(self.highlighted && !self.checked, |this| this.bg(hover))
                    .hover(|this| this.bg(hover))
                    .active(|this| this.text_color(cx.theme().primary))
            })
            .when(self.disabled, |this| {
                this.text_color(if alexa {
                    AlexaColors::dropdown_disabled()
                } else {
                    cx.theme().foreground.opacity(0.3)
                })
            })
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .child(self.title)
    }
}

#[cfg(test)]
mod tests {
    use super::{css, select, select_alexa};
    use gpui_kit::component::{
        Disableable, IndexPath, Root,
        select::{SelectEvent, SelectState},
    };
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{
        AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Subscription,
        TestAppContext, Window, div, px, size,
    };

    struct SelectFixture {
        state: Entity<SelectState<Vec<String>>>,
        items: Vec<String>,
        disabled: bool,
        alexa: bool,
        confirmed: Vec<Option<String>>,
        _subscription: Subscription,
    }

    impl SelectFixture {
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let mut items = ["Alpha", "Beta", "Gamma"].map(String::from).to_vec();
            items.extend((4..=10).map(|number| format!("Choice {number}")));
            let state =
                cx.new(|cx| SelectState::new(items.clone(), Some(IndexPath::new(0)), window, cx));
            let subscription = cx.subscribe(&state, |this, _, event, cx| {
                let SelectEvent::Confirm(value) = event;
                this.confirmed.push(value.clone());
                cx.notify();
            });
            Self {
                state,
                items,
                disabled: false,
                alexa: false,
                confirmed: Vec::new(),
                _subscription: subscription,
            }
        }

        fn set_options(
            &mut self,
            items: &[&str],
            selected: &str,
            window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            self.items = items.iter().map(|item| (*item).to_string()).collect();
            self.state.update(cx, |state, cx| {
                state.set_items(self.items.clone(), window, cx);
                state.set_selected_value(&selected.to_string(), window, cx);
            });
            cx.notify();
        }
    }

    impl Render for SelectFixture {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let control = if self.alexa {
                select_alexa(&self.state)
            } else {
                select(&self.state).w(css(200.))
            };
            div().p_4().child(
                control
                    .id("choice")
                    .items(self.items.clone())
                    .accessibility_label("Choice")
                    .disabled(self.disabled),
            )
        }
    }

    #[gpui_kit::test]
    fn alexa_waits_before_reveal_and_keeps_closing_content_without_keeping_focus(
        cx: &mut TestAppContext,
    ) {
        use std::time::Duration;

        cx.update(gpui_kit::init);
        let mut fixture = None;
        let handle = cx.open_window(size(px(420.), px(320.)), |window, cx| {
            let view = cx.new(|cx| SelectFixture::new(window, cx));
            view.update(cx, |view, _| view.alexa = true);
            fixture = Some(view.clone());
            Root::new(view, window, cx)
        });
        let fixture = fixture.unwrap();
        let top = cx
            .update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                let trigger = window.find("source-dropdown").bounds();
                assert_eq!(trigger.size.width, px(230.));
                assert_eq!(trigger.size.height, px(27.));
                window.within("choice").click("input", cx);
                let menu = window.find("source-options").bounds();
                assert_eq!(menu.size.height, px(0.));
                assert_eq!(menu.top(), trigger.bottom() + px(2.));
                menu.top()
            })
            .unwrap();
        cx.executor().advance_clock(Duration::from_millis(75));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("source-options").bounds().size.height, px(0.));
        })
        .unwrap();
        cx.executor().advance_clock(Duration::from_millis(75));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let menu = window.find("source-options").bounds();
            assert_eq!(menu.top(), top);
            assert!(menu.size.height > px(0.) && menu.size.height < px(110.));
        })
        .unwrap();
        cx.executor().advance_clock(Duration::from_millis(50));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("source-options").bounds().size.height, px(110.));
            window.press("escape", cx);
            assert_eq!(window.find("choice").expanded(), Some(false));
            assert_eq!(window.find("choice").focused(), Some(true));
            assert!(window.try_find("source-options").is_some());
        })
        .unwrap();
        cx.executor().advance_clock(Duration::from_millis(50));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let menu = window.find("source-options").bounds();
            assert_eq!(menu.top(), top);
            assert!(menu.size.height > px(0.) && menu.size.height < px(110.));
            assert_eq!(window.find("choice").focused(), Some(true));
            window.click("option-Alpha", cx);
            assert_eq!(window.find("choice").focused(), Some(true));
            assert!(fixture.read(cx).confirmed.is_empty());
        })
        .unwrap();
        cx.executor().advance_clock(Duration::from_millis(50));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("source-options").is_none());
            assert_eq!(window.find("choice").value(), Some("Alpha"));
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn options_reveal_without_sliding_and_escape_restores_focus(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let handle = cx.open_window(size(px(420.), px(320.)), |window, cx| {
            let view = cx.new(|cx| SelectFixture::new(window, cx));
            Root::new(view, window, cx)
        });
        let top = cx
            .update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                window.within("choice").click("input", cx);
                let menu = window.find("source-options").bounds();
                assert!(menu.size.height < px(180.));
                menu.top()
            })
            .unwrap();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(100));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let menu = window.find("source-options").bounds();
            assert_eq!(menu.top(), top);
            assert!(menu.size.height > px(0.) && menu.size.height < px(180.));
        })
        .unwrap();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(100));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("source-options").bounds().size.height, px(180.));
            window.press("escape", cx);
            assert!(window.try_find("source-options").is_none());
            assert_eq!(window.find("choice").focused(), Some(true));
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn pointer_and_keyboard_confirm_update_the_existing_state(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_reduce_motion(true);
        });
        let mut fixture = None;
        let handle = cx.open_window(size(px(420.), px(320.)), |window, cx| {
            let view = cx.new(|cx| SelectFixture::new(window, cx));
            fixture = Some(view.clone());
            Root::new(view, window, cx)
        });
        let fixture = fixture.unwrap();

        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.within("choice").click("input", cx);
            assert_eq!(window.find("choice").expanded(), Some(true));
            let menu = window.find("source-options").bounds();
            assert_eq!(menu.size.height, px(180.));
            let first = window.find("option-Alpha").bounds();
            let second = window.find("option-Beta").bounds();
            assert_eq!(first.size.height, px(25.));
            assert_eq!(second.top(), first.bottom());
            assert_eq!(first.top(), menu.top() + px(1.));
            assert_eq!(
                menu.size.width,
                window.find("source-dropdown").bounds().size.width
            );
            assert_eq!(window.find("option-Alpha").selected(), Some(true));
            window.click("option-Beta", cx);
        })
        .unwrap();
        cx.run_until_parked();

        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(
                fixture
                    .read(cx)
                    .state
                    .read(cx)
                    .selected_value()
                    .map(String::as_str),
                Some("Beta")
            );
            assert_eq!(fixture.read(cx).confirmed, [Some("Beta".into())]);
            assert_eq!(window.find("choice").expanded(), Some(false));
            assert_eq!(window.find("choice").focused(), Some(true));
            window.press("enter", cx);
            window.press("up", cx);
            window.press("enter", cx);
        })
        .unwrap();
        cx.run_until_parked();

        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(
                fixture
                    .read(cx)
                    .state
                    .read(cx)
                    .selected_value()
                    .map(String::as_str),
                Some("Alpha")
            );
            assert_eq!(
                fixture.read(cx).confirmed,
                [Some("Beta".into()), Some("Alpha".into())]
            );
            window.within("choice").click("input", cx);
            window.press("down", cx);
            window.press("escape", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("choice").expanded(), Some(false));
            assert_eq!(window.find("choice").focused(), Some(true));
            assert_eq!(
                fixture
                    .read(cx)
                    .state
                    .read(cx)
                    .selected_value()
                    .map(String::as_str),
                Some("Alpha")
            );
            assert_eq!(fixture.read(cx).confirmed.len(), 2);
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn disabled_select_and_cancel_preserve_owner_supplied_value(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_reduce_motion(true);
        });
        let mut fixture = None;
        let handle = cx.open_window(size(px(420.), px(320.)), |window, cx| {
            let view = cx.new(|cx| SelectFixture::new(window, cx));
            view.update(cx, |view, _| view.disabled = true);
            fixture = Some(view.clone());
            Root::new(view, window, cx)
        });
        let fixture = fixture.unwrap();

        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.within("choice").click("input", cx);
            window.press("down", cx);
            window.press("enter", cx);
            assert_eq!(window.find("choice").expanded(), Some(false));
            assert!(window.try_find("source-options").is_none());
            assert_eq!(
                fixture
                    .read(cx)
                    .state
                    .read(cx)
                    .selected_value()
                    .map(String::as_str),
                Some("Alpha")
            );
            assert!(fixture.read(cx).confirmed.is_empty());

            fixture.update(cx, |view, cx| {
                view.disabled = false;
                view.state.update(cx, |state, cx| {
                    state.set_selected_value(&"Gamma".into(), window, cx);
                    state.focus(window, cx);
                });
                cx.notify();
            });
            window.render_frame(cx);
            assert_eq!(window.find("choice").value(), Some("Gamma"));
            window.press("enter", cx);
            assert_eq!(window.find("option-Gamma").selected(), Some(true));
            window.press("up", cx);
            window.press("escape", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(
                fixture
                    .read(cx)
                    .state
                    .read(cx)
                    .selected_value()
                    .map(String::as_str),
                Some("Gamma")
            );
            assert!(fixture.read(cx).confirmed.is_empty());
            assert_eq!(window.find("choice").value(), Some("Gamma"));
            assert_eq!(window.find("choice").focused(), Some(true));
            assert!(window.try_find("source-options").is_none());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn changing_options_refreshes_open_and_reopened_menu_without_losing_cursor(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_reduce_motion(true);
        });
        let mut fixture = None;
        let handle = cx.open_window(size(px(420.), px(320.)), |window, cx| {
            let view = cx.new(|cx| SelectFixture::new(window, cx));
            view.update(cx, |view, cx| {
                view.set_options(&["Alpha", "Beta", "Gamma"], "Alpha", window, cx);
            });
            fixture = Some(view.clone());
            Root::new(view, window, cx)
        });
        let fixture = fixture.unwrap();

        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.within("choice").click("input", cx);
            window.press("down", cx); // Cursor is Beta, committed value is Alpha.
            fixture.update(cx, |view, cx| {
                view.set_options(&["Inserted", "Alpha", "Beta", "Gamma"], "Alpha", window, cx);
            });
            window.render_frame(cx);
            assert_eq!(window.find("choice").expanded(), Some(true));
            assert!(window.find("option-Inserted").visible());
            assert_eq!(window.find("source-options").bounds().size.height, px(102.));
            assert!(fixture.read(cx).confirmed.is_empty());
            window.press("enter", cx);
        })
        .unwrap();
        cx.run_until_parked();

        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("choice").value(), Some("Beta"));
            assert_eq!(fixture.read(cx).confirmed, [Some("Beta".into())]);
            window.within("choice").click("input", cx);
            fixture.update(cx, |view, cx| {
                view.set_options(&["Alpha", "Replacement", "Tail"], "Alpha", window, cx);
            });
            window.render_frame(cx);
            assert_eq!(window.find("choice").expanded(), Some(true));
            assert!(window.try_find("option-Beta").is_none());
            assert!(window.try_find("option-Inserted").is_none());
            assert_eq!(window.find("source-options").bounds().size.height, px(77.));
            assert_eq!(window.find("option-Alpha").selected(), Some(true));
            assert_eq!(fixture.read(cx).confirmed.len(), 1);
            window.press("down", cx);
            // An unrelated parent redraw must leave the pending cursor alone.
            fixture.update(cx, |_, cx| cx.notify());
            window.render_frame(cx);
            window.press("enter", cx);
        })
        .unwrap();
        cx.run_until_parked();

        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("choice").value(), Some("Replacement"));
            assert_eq!(fixture.read(cx).confirmed.len(), 2);
            fixture.update(cx, |view, cx| {
                view.set_options(
                    &["Alpha", "Replacement", "Tail", "Reopened"],
                    "Replacement",
                    window,
                    cx,
                );
            });
            window.render_frame(cx);
            window.within("choice").click("input", cx);
            assert!(window.find("option-Reopened").visible());
            window.click("option-Reopened", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("choice").value(), Some("Reopened"));
            assert_eq!(
                fixture.read(cx).confirmed,
                [
                    Some("Beta".into()),
                    Some("Replacement".into()),
                    Some("Reopened".into()),
                ]
            );
        })
        .unwrap();
    }
}
