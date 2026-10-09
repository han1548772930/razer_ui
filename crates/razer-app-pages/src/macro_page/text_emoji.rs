//! Macro's own emoji picker (58190.on/en/an), independent of Dashboard MapText.
use super::text::{text_source, tip_opacity, tip_surface};
use super::text_overlay::{TextAnchor, TextOverlay};
use super::*;
use razer_widgets::surface;
use std::{cell::Cell, rc::Rc};

impl MacroPage {
    fn text_emoji_groups(&self, index: usize, cx: &App) -> Vec<(String, Vec<String>)> {
        let row = self.text_row(index, cx);
        let row = row.borrow();
        if cx.background_executor().now().duration_since(row.mounted) < Duration::from_millis(30) {
            return vec![];
        }
        let source = text_source();
        let allowed = |value: &&String| {
            self.text_ui.windows_version.as_deref() != Some("11")
                || !source.excluded_windows_11.contains(value)
        };
        let query = row.applied_query.to_lowercase();
        if query.is_empty() {
            source
                .groups
                .iter()
                .map(|group| {
                    (
                        if group.label.is_empty() {
                            String::new()
                        } else {
                            tr(&group.name)
                        },
                        group.values.iter().filter(allowed).cloned().collect(),
                    )
                })
                .collect()
        } else {
            vec![(
                String::new(),
                source
                    .search
                    .iter()
                    .filter(|item| item.name.to_lowercase().contains(&query) && allowed(&&item.emo))
                    .map(|item| item.emo.clone())
                    .collect(),
            )]
        }
    }

    pub fn text_emoji_popup(
        &self,
        index: usize,
        opacity: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let row = self.text_row(index, cx);
        let scroll = row.borrow().scroll.clone();
        let main_bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
        let main_measure = main_bounds.clone();
        let panel_bounds = row.borrow().emoji.clone();
        let panel_measure = panel_bounds.clone();
        let mut main = v_flex()
            .id(("macro-emoji-list", index))
            .w_full()
            .h(css(260.))
            .flex_shrink_0()
            .px(css(10.))
            .on_prepaint(move |bounds, _, _| main_measure.set(bounds));
        for (group_index, (label, values)) in
            self.text_emoji_groups(index, cx).into_iter().enumerate()
        {
            // Literal emoji_popup_main_label does not match the CSS-module rule.
            let mut group = div()
                .flex()
                .flex_wrap()
                .w_full()
                .mb(css(20.))
                .flex_shrink_0()
                .child(
                    div()
                        .w_full()
                        .when(!label.is_empty(), |view| view.child(label)),
                );
            for (emoji_index, value) in values.into_iter().enumerate() {
                let variants = text_source().variants.get(&value);
                let key = SharedString::from(format!(
                    "macro-text-{index}-emoji-{group_index}-{emoji_index}"
                ));
                let cell_bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
                let cell_measure = cell_bounds.clone();
                let tip_bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
                let hover_tip = tip_bounds.clone();
                let is_hovered = self.text_ui.hovered_emoji == Some((group_index, emoji_index));
                let placement = variants.map(|_| {
                    let state = window.use_keyed_state(
                        (ElementId::from(key.clone()), "variant-placement"),
                        cx,
                        |_, _| Rc::new(Cell::new(None)),
                    );
                    let value = state.read(cx).clone();
                    if !is_hovered {
                        value.set(None);
                    }
                    value
                });
                let alpha = if variants.is_some() {
                    tip_opacity(key.clone().into(), is_hovered, window, cx)
                } else {
                    0.
                };
                let mut item = div()
                    .id(key.clone())
                    .relative()
                    .size(css(40.))
                    .flex_shrink_0()
                    .hover(|s| s.bg(rgb(0x707070)))
                    .on_prepaint(move |bounds, _, _| cell_measure.set(bounds))
                    .when(variants.is_some(), |item| {
                        item.on_hover(cx.listener(move |this, hovered: &bool, window, cx| {
                            if *hovered {
                                this.text_ui.hovered_emoji = Some((group_index, emoji_index));
                            } else if this.text_ui.hovered_emoji == Some((group_index, emoji_index))
                                && !hover_tip.get().contains(&window.mouse_position())
                            {
                                this.text_ui.hovered_emoji = None;
                            }
                            cx.notify();
                        }))
                    })
                    .child(
                        BaseButton::new((ElementId::from(key.clone()), "value"))
                            .p_0()
                            .size_full()
                            .child(
                                div()
                                    .font_family("Segoe UI")
                                    .text_size(css(20.))
                                    .h(css(27.))
                                    .line_height(css(27.))
                                    .child(value.clone()),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.append_text_emoji(&value, window, cx)
                            })),
                    );
                if is_hovered && let Some(variants) = variants {
                    let leave_cell = cell_bounds.clone();
                    let tip = tip_surface(alpha * opacity)
                        .id((ElementId::from(key.clone()), "variants"))
                        .on_prepaint(move |bounds, _, _| tip_bounds.set(bounds))
                        .on_hover(cx.listener(move |this, hovered: &bool, window, cx| {
                            if *hovered {
                                this.text_ui.hovered_emoji = Some((group_index, emoji_index));
                            } else if this.text_ui.hovered_emoji == Some((group_index, emoji_index))
                                && !leave_cell.get().contains(&window.mouse_position())
                            {
                                this.text_ui.hovered_emoji = None;
                            }
                            cx.notify();
                        }))
                        // Literal emoji_popup_main_item has no matching CSS. These
                        // are auto-sized 14px Roboto flex children, not 40px cells.
                        .children(variants.iter().enumerate().map(|(variant_index, value)| {
                            let value = value.clone();
                            div()
                                .id((key.clone(), variant_index))
                                .flex_shrink_0()
                                .child(value.clone())
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.append_text_emoji(&value, window, cx);
                                    cx.stop_propagation();
                                }))
                        }));
                    item = item.child(
                        deferred(TextOverlay {
                            content: tip.into_any_element(),
                            anchor: TextAnchor::Variant {
                                cell: cell_bounds,
                                panel: panel_bounds.clone(),
                                main: main_bounds.clone(),
                                placement: placement.expect("variant placement"),
                            },
                        })
                        .priority(105),
                    );
                }
                group = group.child(item);
            }
            main = main.child(group);
        }
        let header = h_flex()
            .w_full()
            .h(css(35.))
            .flex_shrink_0()
            .bg(rgba(0x5d5d5d99))
            .children(
                text_source()
                    .tabs
                    .iter()
                    .enumerate()
                    .map(|(tab_index, tab)| {
                        let id: ElementId =
                            SharedString::from(format!("macro-text-{index}-category-{tab_index}"))
                                .into();
                        let pointer = surface::pointer_state(id.clone(), window, cx);
                        let (hovered, _) = pointer.read(cx).sample();
                        let alpha = tip_opacity(id.clone(), hovered, window, cx);
                        let active = row.borrow().category == tab_index;
                        let choose = row.clone();
                        let pos = tab.pos;
                        surface::track_pointer(BaseButton::new(id), &pointer, window)
                            .relative()
                            .flex_1()
                            .h_full()
                            .p_0()
                            .items_start()
                            .opacity(if active || hovered { 1. } else { 0.6 })
                            .when(active, |button| {
                                button.border_b_2().border_color(rgb(0x44d62c))
                            })
                            .child(
                                div()
                                    .mt(css(2.))
                                    .h(css(27.))
                                    .font_family("Segoe UI")
                                    .text_size(css(20.))
                                    .line_height(css(27.))
                                    .child(tab.emo.clone()),
                            )
                            .when(hovered, |button| {
                                button.child(
                                    deferred(
                                        tip_surface(alpha * opacity)
                                            .absolute()
                                            .left_0()
                                            .top(css(35.))
                                            .child(tr(&tab.name)),
                                    )
                                    .priority(105),
                                )
                            })
                            .on_click(cx.listener(move |_, _, window, cx| {
                                let mut row = choose.borrow_mut();
                                row.category = tab_index;
                                let unit = window.rem_size() / 16.;
                                // group.offsetTop is measured from the absolute wrapper;
                                // header + search + border = 76; source subtracts 80.
                                let offset =
                                    row.scroll.bounds_for_item(pos).map_or(px(0.), |bounds| {
                                        (bounds.top() - row.scroll.bounds().top() - unit * 4.)
                                            .max(px(0.))
                                    });
                                row.scroll.set_offset(point(px(0.), -offset));
                                cx.notify();
                            }))
                    }),
            );
        let clear_id: ElementId = ("macro-text-search-clear", index).into();
        let clear_pointer = surface::pointer_state(clear_id.clone(), window, cx);
        let (clear_hovered, _) = clear_pointer.read(cx).sample();
        let clear_alpha = surface::fade_opacity(
            clear_id.clone(),
            if clear_hovered { 1. } else { 0. },
            200,
            window,
            cx,
        );
        let search = h_flex()
            .id(("macro-emoji-searchbox", index))
            .relative()
            .w_full()
            .h(css(40.))
            .flex_shrink_0()
            .items_start()
            .when(
                self.text_ui
                    .search
                    .focus_handle(cx)
                    .contains_focused(window, cx),
                |view| view.border_b_2().border_color(rgb(0x44d62c)),
            )
            .on_click(cx.listener(|this, _, window, cx| {
                this.text_ui.search.focus_handle(cx).focus(window, cx);
            }))
            .child(
                div()
                    .w(css(40.))
                    .h_full()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(img("synapse/shortcuts-emoji-search.svg").size(css(18.))),
            )
            .child(
                Input::new(&self.text_ui.search)
                    .id(("macro-text-search", index))
                    .appearance(false)
                    .bordered(false)
                    .flex_1()
                    .h(css(38.))
                    .p_0()
                    .bg(rgb(0))
                    .text_size(css(14.))
                    .text_color(rgb(0xcccccc))
                    .rounded_none(),
            )
            .when(!row.borrow().query.is_empty(), |view| {
                view.child(
                    surface::track_pointer(BaseButton::new(clear_id), &clear_pointer, window)
                        .absolute()
                        .top(css(12.))
                        .right(css(10.))
                        .size(css(16.))
                        .p_0()
                        .overflow_hidden()
                        .child(
                            img("synapse/shortcuts-search-clear.svg")
                                .absolute()
                                .left(css(-2.))
                                .top(css(-2.))
                                .size(css(20.)),
                        )
                        .child(
                            img("synapse/shortcuts-search-clear-active.svg")
                                .absolute()
                                .left(css(-2.))
                                .top(css(-2.))
                                .size(css(20.))
                                .opacity(clear_alpha),
                        )
                        .on_click(cx.listener(move |this, _, window, cx| {
                            // on receives resetEmoji:()=>{}: the visible input clears,
                            // but the applied filter remains until the next input change.
                            this.text_row(index, cx).borrow_mut().query.clear();
                            this.text_ui.ignored_query = Some(String::new());
                            this.text_ui
                                .search
                                .update(cx, |input, cx| input.set_value("", window, cx));
                            cx.notify();
                        })),
                )
            });
        let panel = v_flex()
            .id(("macro-emoji-panel", index))
            .w(css(430.))
            .h(css(337.))
            .border_1()
            .border_color(rgb(0x707070))
            .bg(rgb(0))
            .text_color(rgb(0xcccccc))
            .font_family("Roboto")
            .text_size(css(14.))
            .opacity(opacity)
            .occlude()
            .on_prepaint(move |bounds, _, _| panel_measure.set(bounds))
            .on_click(|_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(header)
            .child(search)
            .child(main.scrollable_y().track_scroll(&scroll));
        deferred(TextOverlay {
            content: panel.into_any_element(),
            anchor: TextAnchor::Emoji {
                modal: row.borrow().modal.clone(),
            },
        })
        .priority(104)
        .into_any_element()
    }
}
