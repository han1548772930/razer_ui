//! Ordinary `.s3-dropdown/.s3-options` presentation over Kit's select, popup and list behavior.
//!
//! SelectState remains the committed value and event source. The owner supplies its current
//! options because Kit 0.7 does not expose SelectState's delegate or its menu renderer.
use gpui_kit::base::{Popover, Select as BaseSelect};
use gpui_kit::component::{
    ActiveTheme, Disableable, ElementExt, Icon, IndexPath, Selectable, StyledExt,
    list::{List, ListDelegate, ListState},
    select::{SelectEvent, SelectItem, SelectState},
};
use gpui_kit::{prelude::FluentBuilder as _, *};

use super::{surface::css, theme::DropdownColors};

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
    open: bool,
    width: Pixels,
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
            open: false,
            width: px(0.),
        }
    }

    /// Glue the controlled Base Select and Popover to the framework List cursor.
    fn set_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        let open = open && !self.disabled;
        if self.open == open {
            return;
        }
        self.open = open;
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
        let changed = self.list.update(cx, |list, cx| {
            let previous = list.delegate();
            let items_changed = previous.items.len() != items.len()
                || previous.items.iter().zip(&items).any(|(old, new)| {
                    old.value() != new.value()
                        || old.title() != new.title()
                        || old.disabled() != new.disabled()
                });
            if !items_changed && previous.committed == committed {
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
            view.disabled = self.disabled;
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
            .unwrap_or_default();
        let menu_height = (options.items.len() as f32 * 25. + 2.).min(180.);
        let width = self.width;
        let list = self.list.clone();
        let open_change = cx.entity().downgrade();
        let popup_change = cx.entity().downgrade();
        let focused = focus.is_focused(window);
        let open = self.open;
        let colors = DropdownColors::new();
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
                    .pl(css(4.))
                    .truncate()
                    .child(title.clone()),
            )
            .child(
                div()
                    .w(css(29.))
                    .h_full()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::default()
                            .path("synapse/expand.svg")
                            .size(css(10.))
                            .transform(Transformation::rotate(radians(if open {
                                std::f32::consts::PI
                            } else {
                                0.
                            }))),
                    ),
            );

        div()
            .id("source-dropdown")
            .test_support()
            .h(css(27.))
            .min_w_0()
            .border_1()
            .border_color(if !self.disabled && (open || focused) {
                cx.theme().primary
            } else {
                cx.theme().input
            })
            .text_size(css(14.))
            .line_height(css(17.))
            .text_color(cx.theme().foreground)
            .when(!self.disabled, |this| {
                this.hover(|style| style.border_color(cx.theme().primary))
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
                            .content(move |_, _, cx| {
                                div()
                                    .id("source-options")
                                    .test_support()
                                    .w(width)
                                    .h(css(menu_height))
                                    .max_h(css(180.))
                                    .ml(-px(1.))
                                    .border_1()
                                    .border_color(cx.theme().input)
                                    .rounded_none()
                                    .bg(colors.surface())
                                    .overflow_hidden()
                                    .child(List::new(&list).p_0().max_h(css(178.)))
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
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = DropdownColors::new();
        div()
            .id(self.id)
            .test_support()
            .role(Role::ListBoxOption)
            .aria_label(self.title.clone())
            .aria_selected(self.checked)
            .when(self.highlighted, |this| this.aria_active_descendant())
            .h(css(25.))
            .min_h(css(25.))
            .w_full()
            .p(css(4.))
            .rounded_none()
            .text_size(css(14.))
            .line_height(css(17.))
            .text_color(if self.checked {
                cx.theme().primary
            } else {
                cx.theme().foreground
            })
            .when(!self.disabled, |this| {
                this.when(self.highlighted && !self.checked, |this| {
                    this.bg(colors.hover())
                })
                .hover(|this| this.bg(colors.hover()))
                .active(|this| this.text_color(cx.theme().primary))
            })
            .when(self.disabled, |this| {
                this.text_color(cx.theme().foreground.opacity(0.3))
            })
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .child(self.title)
    }
}

#[cfg(test)]
mod tests {
    use super::{css, select};
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
            div().p_4().child(
                select(&self.state)
                    .id("choice")
                    .items(self.items.clone())
                    .accessibility_label("Choice")
                    .disabled(self.disabled)
                    .w(css(200.)),
            )
        }
    }

    #[gpui_kit::test]
    fn pointer_and_keyboard_confirm_update_the_existing_state(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
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
        cx.update(gpui_kit::init);
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
        cx.update(gpui_kit::init);
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
