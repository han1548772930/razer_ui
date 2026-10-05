// Current Dashboard 34198/b + v/D/R and .maptext rules in 55.4e8559cb.
use crate::ui::scroll::SourceScrollable as _;

struct TextTipVisibility {
    target: bool,
    visible: bool,
    changed_at: Instant,
}

/// `.maptext .tip` delays both visibility directions by one second. Opacity
/// then interpolates for 300ms, even while the source visibility is hidden.
fn text_tip_sample(id: ElementId, hovered: bool, window: &mut Window, cx: &mut App) -> (f32, bool) {
    let now = cx.background_executor().now();
    let visibility = window.use_keyed_state((id.clone(), "visibility-delay"), cx, |_, _| {
        TextTipVisibility {
            target: false,
            visible: false,
            changed_at: now,
        }
    });
    let reduced = cx.reduce_motion();
    let visible = visibility.update(cx, |state, _| {
        if state.target != hovered {
            state.target = hovered;
            state.changed_at = now;
        }
        if reduced || now.duration_since(state.changed_at) >= Duration::from_secs(1) {
            state.visible = hovered;
        }
        state.visible
    });
    if visible != hovered {
        window.request_animation_frame();
    }
    let opacity = motion::transition_with_status(
        (id, "opacity"),
        if hovered { 1. } else { 0. },
        Transition::new(Duration::from_millis(300))
            .delay(Duration::from_secs(1))
            .easing(Easing::Linear),
        window,
        cx,
    );
    (opacity.value, visible)
}

fn text_tip_surface(label: String, opacity: f32) -> Div {
    div()
        .absolute()
        .left_0()
        .px(surface::css(10.))
        .py(surface::css(8.))
        .border_1()
        .border_color(rgb(0x5d5d5d))
        .bg(rgb(0))
        .text_color(rgb(0xcccccc))
        .font_family("Roboto")
        .text_size(surface::css(14.))
        .line_height(surface::css(16.))
        .whitespace_nowrap()
        .opacity(opacity)
        .child(label)
}

fn text_toolbar_button(
    id: &'static str,
    label: String,
    asset: &'static str,
    hover: &'static str,
    show_tip: bool,
    window: &mut Window,
    cx: &mut App,
) -> ShortcutButton {
    let state = window.use_keyed_state((ElementId::from(id), "hover"), cx, |_, _| false);
    let hovered = show_tip && *state.read(cx);
    let (opacity, visible) = text_tip_sample(id.into(), hovered, window, cx);
    ShortcutButton::new(id)
        .accessibility_label(label.clone())
        .relative()
        .group(id)
        .size(surface::css(20.))
        .flex_shrink_0()
        .on_hover(window.listener_for(&state, |state, hovered, _, cx| {
            *state = *hovered;
            cx.notify();
        }))
        .child(img(asset).size_full())
        .child(
            img(hover)
                .absolute()
                .inset_0()
                .size_full()
                .opacity(0.)
                .group_hover(id, |s| s.opacity(1.)),
        )
        .when(show_tip && visible, |button| {
            button.child(
                deferred(text_tip_surface(label, opacity).bottom(surface::css(-38.))).priority(105),
            )
        })
}

impl Shortcuts {
    fn reset_emoji_mapping(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.emoji_open = false;
        self.emoji_hover = None;
        self.emoji_category = 0;
        self.emoji_cursor = 0;
        self.emoji_scroll.set_offset(point(px(0.), px(0.)));
        self.emoji_last_scroll = px(0.);
        self.emoji_resetting = false;
        self.emoji_search_generation = self.emoji_search_generation.wrapping_add(1);
        self.emoji_search
            .update(cx, |input, cx| input.set_value("", window, cx));
    }

    fn emoji_search_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.emoji_category = 0;
        self.emoji_cursor = 0;
        self.emoji_hover = None;
        self.emoji_search_generation = self.emoji_search_generation.wrapping_add(1);
        let generation = self.emoji_search_generation;
        self.emoji_resetting = self.emoji_search.read(cx).value().is_empty();
        self.emoji_scroll.set_offset(point(px(0.), px(0.)));
        if self.emoji_resetting {
            cx.spawn_in(window, async move |this, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(200))
                    .await;
                let _ = this.update(cx, |this, cx| {
                    if this.emoji_search_generation == generation {
                        this.emoji_resetting = false;
                        cx.notify();
                    }
                });
            })
            .detach();
        }
        cx.notify();
    }

    fn sync_emoji_scroll(&mut self, cx: &App) {
        let offset = self.emoji_scroll.offset().y;
        if self.emoji_open && offset != self.emoji_last_scroll {
            self.emoji_last_scroll = offset;
            if self.emoji_search.read(cx).value().is_empty() {
                self.emoji_category = self
                    .emoji_scroll
                    .top_item()
                    .min(source_shortcuts().emoji.tabs.len() - 1);
                self.emoji_cursor = 0;
            }
            self.emoji_hover = None;
        }
    }
    /// The source treats a missing Windows version exactly like Windows 11.
    /// No native version reply is fabricated for the five Windows 10 cat glyphs.
    fn emoji_groups(&self, cx: &App) -> Vec<Vec<String>> {
        if self.emoji_resetting {
            return vec![];
        }
        let data = &source_shortcuts().emoji;
        let allowed = |emoji: &String| !data.excluded_without_windows_10.contains(emoji);
        let query = self.emoji_search.read(cx).value().to_lowercase();
        if !query.is_empty() {
            return vec![
                data.search
                    .iter()
                    .filter(|item| item.name.to_lowercase().contains(&query) && allowed(&item.emo))
                    .map(|item| item.emo.clone())
                    .collect(),
            ];
        }
        data.groups
            .iter()
            .map(|group| {
                group
                    .values
                    .iter()
                    .filter(|item| allowed(item))
                    .cloned()
                    .collect()
            })
            .collect()
    }

    fn close_emoji(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.emoji_open = false;
        self.emoji_hover = None;
        self.paragraph.focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    fn add_emoji(&mut self, emoji: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(draft) = &self.draft else { return };
        if draft.value.output.kind() != "text" {
            return;
        }
        // b.addEmoji checks the entire existing UTF-16 length before replacing
        // its selection, so a selection does not free capacity for this guard.
        if draft.value.output.value().encode_utf16().count() + emoji.encode_utf16().count() > 250 {
            return;
        }
        self.paragraph
            .update(cx, |input, cx| input.replace(emoji.to_string(), window, cx));
        let value = self.paragraph.read(cx).value().to_string();
        self.set_output_value(value);
        self.paragraph.focus_handle(cx).focus(window, cx);
        self.emoji_hover = None;
        self.changed(cx);
    }

    fn select_emoji_category(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.emoji_groups(cx).len() {
            return;
        }
        self.emoji_category = index;
        self.emoji_cursor = 0;
        self.emoji_hover = None;
        self.emoji_scroll.scroll_to_top_of_item(index);
        self.emoji_tab_focus.focus(window, cx);
        cx.notify();
    }

    fn emoji_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let tab = self.emoji_tab_focus.is_focused(window);
        let item = self.emoji_item_focus.is_focused(window);
        if !(tab || item) {
            return;
        }
        let groups = self.emoji_groups(cx);
        let Some(group) = groups.get(self.emoji_category) else {
            return;
        };
        let key = event.keystroke.key.as_str();
        if !["left", "right", "up", "down"].contains(&key) {
            return;
        }
        window.prevent_default();
        cx.stop_propagation();
        if tab {
            let count = source_shortcuts().emoji.tabs.len();
            let next = match key {
                "left" => (self.emoji_category + count - 1) % count,
                "right" => (self.emoji_category + 1) % count,
                _ => self.emoji_category,
            };
            self.select_emoji_category(next, window, cx);
        } else {
            let row = self.emoji_cursor / 10;
            let col = self.emoji_cursor % 10;
            let next = match key {
                "left" => row * 10 + (col + 9) % 10,
                "right" => row * 10 + (col + 1) % 10,
                "up" if row > 0 => (row - 1) * 10 + col,
                "down" => (row + 1) * 10 + col,
                _ => self.emoji_cursor,
            };
            if next < group.len() {
                self.emoji_cursor = next;
                // Group children supply their unscrolled bounds. Keep the
                // focused 40px row inside the 260px native scroll viewport.
                if let Some(bounds) = self.emoji_scroll.bounds_for_item(self.emoji_category) {
                    let top = bounds.top() - self.emoji_scroll.bounds().top()
                        + window.rem_size() * ((next / 10 * 40) as f32 / 16.);
                    let current = self.emoji_scroll.offset().y;
                    let height = window.rem_size() * (260. / 16.);
                    let item_height = window.rem_size() * (40. / 16.);
                    let offset = if top + current < px(0.) {
                        -top
                    } else if top + current + item_height > height {
                        height - top - item_height
                    } else {
                        current
                    };
                    self.emoji_scroll.set_offset(point(px(0.), offset));
                    self.emoji_last_scroll = self.emoji_scroll.offset().y;
                }
                cx.notify();
            }
        }
    }

    fn text_function(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let owner = cx.entity().downgrade();
        let text_owner = owner.clone();
        let toolbar_owner = owner.clone();
        let count = self
            .draft
            .as_ref()
            .map_or(0, |draft| draft.value.output.value().encode_utf16().count());
        let open = self.emoji_open;
        // b/v/81787 render two nested wrappers with the same `.visible` class.
        // Each independently has a 500ms linear opacity; composite their alpha.
        let opacity: f32 = motion::transition(
            "shortcut-emoji-popup-opacity",
            if open { 1. } else { 0. },
            Transition::new(Duration::from_millis(500)).easing(Easing::Linear),
            window,
            cx,
        );
        let emoji = text_toolbar_button(
            "shortcut-emoji-toggle",
            i18n::t("TEXT_EMOJI"),
            if open {
                "synapse/mapping-close.svg"
            } else {
                "synapse/shortcuts-emoji.svg"
            },
            if open {
                "synapse/mapping-close.svg"
            } else {
                "synapse/shortcuts-emoji-active.svg"
            },
            !open,
            window,
            cx,
        )
        .on_click(cx.listener(|this, _, window, cx| {
            this.emoji_open = !this.emoji_open;
            this.emoji_hover = None;
            if !this.emoji_open {
                this.paragraph.focus_handle(cx).focus(window, cx);
            }
            cx.notify();
        }))
        .child(
            canvas(
                move |bounds, _, cx| {
                    let _ = owner.update(cx, |this, _| this.emoji_button_bounds = bounds);
                },
                |_, _, _, _| (),
            )
            .absolute()
            .inset_0(),
        );
        let view = v_flex()
            .w(surface::css(210.))
            .child(
                div()
                    .id("shortcut-text")
                    .test_support()
                    .relative()
                    .capture_key_down(|event: &KeyDownEvent, window, cx| {
                        if event.keystroke.key == "enter" {
                            window.prevent_default();
                            cx.stop_propagation();
                        }
                    })
                    .child(
                        Textarea::new(&self.paragraph)
                            .appearance(false)
                            .bordered(false)
                            .w(surface::css(210.))
                            .h(surface::css(96.))
                            .p(surface::css(5.))
                            .border_1()
                            .border_color(rgb(0x5d5d5d))
                            .rounded_none()
                            .bg(rgb(0x111111))
                            .text_size(surface::css(14.))
                            .line_height(surface::css(17.)),
                    )
                    .child(
                        canvas(
                            move |bounds, _, cx| {
                                let _ = text_owner
                                    .update(cx, |this, _| this.emoji_text_bounds = bounds);
                            },
                            |_, _, _, _| (),
                        )
                        .absolute()
                        .inset_0(),
                    ),
            )
            .child(
                h_flex()
                    .relative()
                    .h(surface::css(20.))
                    .mb(surface::css(20.))
                    .child(emoji)
                    .child(
                        text_toolbar_button(
                            "shortcut-character-map",
                            i18n::t("LAUNCH_CHARACTER_MAP"),
                            "synapse/shortcuts-character.svg",
                            "synapse/shortcuts-character-active.svg",
                            true,
                            window,
                            cx,
                        )
                        .ml(surface::css(10.))
                        .on_click(
                            cx.listener(|_, _, _, cx| cx.emit(ShortcutsOpenModule::CharacterMap)),
                        ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_right()
                            .text_color(rgb(0x707070))
                            .line_height(surface::css(17.))
                            .child(format!("{count}/250")),
                    )
                    .child(
                        canvas(
                            move |bounds, _, cx| {
                                let _ = toolbar_owner
                                    .update(cx, |this, _| this.emoji_toolbar_bounds = bounds);
                            },
                            |_, _, _, _| (),
                        )
                        .absolute()
                        .inset_0(),
                    ),
            );
        // visibility:0s hides immediately on close; retain the sampled opacity
        // while closed so a quick reopen reverses the in-flight transition.
        view.when(open, |view| {
            view.child(self.emoji_popup(opacity * opacity, window, cx))
        })
        .into_any_element()
    }

    fn emoji_popup(&self, opacity: f32, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let groups = self.emoji_groups(cx);
        // Deferred tips are outside the popup hitbox. Retain all visible tip
        // bounds for this frame, including tips waiting for their hide delay.
        let tip_regions = std::rc::Rc::new(std::cell::RefCell::new(Vec::<Bounds<Pixels>>::new()));
        let mut main = v_flex()
            .id("shortcut-emoji-list")
            .w_full()
            .h(surface::css(260.))
            .px(surface::css(10.))
            .flex_shrink_0();
        for (group_index, values) in groups.iter().enumerate() {
            let mut group = div()
                .flex()
                .flex_wrap()
                .w_full()
                .mb(surface::css(20.))
                .flex_shrink_0();
            // Current L receives p:!0, so p.label is undefined. Its group label
            // node is empty; preserve the actual blank heading, not the data key.
            for (index, emoji) in values.iter().enumerate() {
                let value = emoji.clone();
                let variants = source_shortcuts().emoji.variants.get(emoji);
                let cell_bounds =
                    std::rc::Rc::new(std::cell::Cell::new(Bounds::<Pixels>::default()));
                let tip_bounds =
                    std::rc::Rc::new(std::cell::Cell::new(Bounds::<Pixels>::default()));
                let (variant_opacity, variant_visible) = if variants.is_some() {
                    text_tip_sample(
                        SharedString::from(format!("shortcut-emoji-tip-{group_index}-{index}"))
                            .into(),
                        self.emoji_hover == Some((group_index, index)),
                        window,
                        cx,
                    )
                } else {
                    (0., false)
                };
                let active = group_index == self.emoji_category && index == self.emoji_cursor;
                let mut item = div()
                    .id(SharedString::from(format!(
                        "shortcut-emoji-cell-{group_index}-{index}"
                    )))
                    .relative()
                    .size(surface::css(40.))
                    .flex_shrink_0()
                    .when(variants.is_some(), |item| {
                        let tip_bounds = tip_bounds.clone();
                        item.on_hover(cx.listener(move |this, hovered: &bool, window, cx| {
                            if *hovered {
                                this.emoji_hover = Some((group_index, index));
                            } else if this.emoji_hover == Some((group_index, index))
                                && !tip_bounds.get().contains(&window.mouse_position())
                            {
                                this.emoji_hover = None;
                            }
                            cx.notify();
                        }))
                    })
                    .child(
                        ShortcutButton::new(SharedString::from(format!(
                            "shortcut-emoji-{group_index}-{index}"
                        )))
                        .accessibility_label(value.clone())
                        .tab_index(if active { 0 } else { -1 })
                        .when(active, |button| button.track_focus(&self.emoji_item_focus))
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .hover(|s| s.bg(rgb(0x707070)))
                        .font_family("Segoe UI")
                        .text_size(surface::css(20.))
                        .line_height(surface::css(27.))
                        .child(value.clone())
                        .on_click(cx.listener(
                            move |this, _, window, cx| this.add_emoji(&value, window, cx),
                        )),
                    );
                if variant_visible && let Some(variants) = variants {
                    let cell_measurement = cell_bounds.clone();
                    item = item.child(
                        canvas(
                            move |bounds, _, _| cell_measurement.set(bounds),
                            |_, _, _, _| (),
                        )
                        .absolute()
                        .inset_0(),
                    );
                    let col = index % 10;
                    let left = if col >= 7 {
                        160. - col as f32 * 40.
                    } else if col > 2 {
                        -80.
                    } else {
                        -(col as f32 * 40.)
                    };
                    let y = self
                        .emoji_scroll
                        .bounds_for_item(group_index)
                        .map_or(px(0.), |b| b.top() - self.emoji_scroll.bounds().top())
                        + self.emoji_scroll.offset().y
                        + window.rem_size() * ((index / 10 * 40) as f32 / 16.);
                    let tip = h_flex()
                        .absolute()
                        .left(surface::css(left))
                        .top(surface::css(if y <= window.rem_size() * (44. / 16.) {
                            40.
                        } else {
                            -40.
                        }))
                        .border_1()
                        .border_color(rgb(0x5d5d5d))
                        .bg(rgb(0))
                        // GPUI deferred draws do not retain ancestor opacity.
                        .opacity(variant_opacity * opacity)
                        .id(SharedString::from(format!(
                            "shortcut-emoji-variants-{group_index}-{index}"
                        )))
                        .on_hover(cx.listener(move |this, hovered: &bool, window, cx| {
                            if *hovered {
                                this.emoji_hover = Some((group_index, index));
                                cx.notify();
                            } else if this.emoji_hover == Some((group_index, index))
                                && !cell_bounds.get().contains(&window.mouse_position())
                            {
                                this.emoji_hover = None;
                                cx.notify();
                            }
                        }))
                        .children(variants.iter().enumerate().map(|(variant_index, value)| {
                            let value = value.clone();
                            ShortcutButton::new(SharedString::from(format!(
                                "shortcut-emoji-variant-{variant_index}"
                            )))
                            .accessibility_label(value.clone())
                            .size(surface::css(40.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .font_family("Segoe UI")
                            .text_size(surface::css(20.))
                            .line_height(surface::css(27.))
                            .hover(|s| s.bg(rgb(0x707070)))
                            .child(value.clone())
                            .on_click(cx.listener(
                                move |this, _, window, cx| this.add_emoji(&value, window, cx),
                            ))
                        }))
                        .child(
                            canvas(
                                {
                                    let tip_regions = tip_regions.clone();
                                    move |bounds, _, _| {
                                        tip_bounds.set(bounds);
                                        tip_regions.borrow_mut().push(bounds);
                                    }
                                },
                                |_, _, _, _| (),
                            )
                            .absolute()
                            .inset_0(),
                        );
                    item = item.child(deferred(tip).priority(105));
                }
                group = group.child(item);
            }
            main = main.child(group);
        }
        let search_active = !self.emoji_search.read(cx).value().is_empty();
        let panel = v_flex()
            .id("shortcut-emoji-popup")
            .test_support()
            .role(Role::Dialog)
            .aria_label(i18n::t("TEXT_EMOJI"))
            .w(surface::css(430.))
            .h(surface::css(337.))
            .border_1()
            .border_color(rgb(0x707070))
            .bg(rgb(0))
            .opacity(opacity)
            .capture_key_down(
                cx.listener(|this, event, window, cx| this.emoji_key(event, window, cx)),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    this.close_emoji(window, cx);
                    cx.stop_propagation();
                }
            }))
            .on_mouse_down_out(cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                if !this.emoji_button_bounds.contains(&event.position)
                    && !tip_regions
                        .borrow()
                        .iter()
                        .any(|bounds| bounds.contains(&event.position))
                {
                    this.emoji_open = false;
                    this.emoji_hover = None;
                    cx.notify();
                }
            }))
            .child(
                h_flex()
                    .w_full()
                    .h(surface::css(35.))
                    .flex_shrink_0()
                    .bg(rgba(0x5d5d5d99))
                    .children(source_shortcuts().emoji.tabs.iter().enumerate().map(
                        |(index, tab)| {
                            let selected = index == self.emoji_category;
                            let label = tab.name.clone();
                            let tip_id: ElementId =
                                SharedString::from(format!("shortcut-emoji-header-tip-{index}"))
                                    .into();
                            let state =
                                window.use_keyed_state((tip_id.clone(), "hover"), cx, |_, _| false);
                            let hovered = *state.read(cx);
                            let (tip_opacity, visible) =
                                text_tip_sample(tip_id, hovered, window, cx);
                            ShortcutButton::new(SharedString::from(format!(
                                "shortcut-emoji-category-{index}"
                            )))
                            .accessibility_label(label.clone())
                            .relative()
                            .tab_index(if selected { 0 } else { -1 })
                            .when(selected, |button| button.track_focus(&self.emoji_tab_focus))
                            .flex_1()
                            .h_full()
                            .border_b_2()
                            .border_color(if selected { rgb(0x44d62c) } else { rgba(0) })
                            .flex()
                            .justify_center()
                            .items_start()
                            .pt(surface::css(2.))
                            .child(
                                div()
                                    .font_family("Segoe UI")
                                    .text_size(surface::css(20.))
                                    .line_height(surface::css(27.))
                                    .opacity(if selected { 1. } else { 0.6 })
                                    .child(tab.emo.clone()),
                            )
                            .on_hover(window.listener_for(&state, |state, hovered, _, cx| {
                                *state = *hovered;
                                cx.notify();
                            }))
                            .when(visible, |button| {
                                button.child(
                                    deferred(
                                        text_tip_surface(label, tip_opacity * opacity)
                                            .top(relative(1.)),
                                    )
                                    .priority(105),
                                )
                            })
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    this.select_emoji_category(index, window, cx)
                                },
                            ))
                        },
                    )),
            )
            .child(
                h_flex()
                    .relative()
                    .w_full()
                    .h(surface::css(40.))
                    .flex_shrink_0()
                    .border_b_2()
                    .border_color(rgba(0))
                    .when(
                        self.emoji_search
                            .focus_handle(cx)
                            .contains_focused(window, cx),
                        |view| view.border_color(rgb(0x44d62c)),
                    )
                    .hover(|s| s.border_color(rgb(0x44d62c)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.emoji_search.focus_handle(cx).focus(window, cx);
                        }),
                    )
                    .child(
                        div()
                            .w(surface::css(40.))
                            .h_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                img("synapse/shortcuts-emoji-search.svg").size(surface::css(18.)),
                            ),
                    )
                    .child(
                        Input::new(&self.emoji_search)
                            .id("shortcut-emoji-search")
                            .appearance(false)
                            .bordered(false)
                            .flex_1()
                            .h(surface::css(38.))
                            .p_0()
                            .pr(surface::css(30.))
                            .bg(rgb(0))
                            .text_color(rgb(0xcccccc))
                            .text_size(surface::css(14.))
                            .rounded_none(),
                    )
                    .when(search_active, |view| {
                        view.child(
                            text_toolbar_button(
                                "shortcut-emoji-clear",
                                i18n::t("CLEAR"),
                                "synapse/shortcuts-search-clear.svg",
                                "synapse/shortcuts-search-clear-active.svg",
                                false,
                                window,
                                cx,
                            )
                            .absolute()
                            .right(surface::css(10.))
                            .top(surface::css(12.))
                            .size(surface::css(16.))
                            .on_click(cx.listener(
                                |this, _, window, cx| {
                                    this.emoji_search
                                        .update(cx, |input, cx| input.set_value("", window, cx));
                                    this.emoji_search_changed(window, cx);
                                },
                            )),
                        )
                    }),
            )
            .child(main.scrollable_y().track_scroll(&self.emoji_scroll));
        deferred(ShortcutEmojiPosition {
            owner: cx.entity().downgrade(),
            child: panel.into_any_element(),
        })
        .priority(104)
        .into_any_element()
    }
}

/// Resolve against this frame's textarea/toolbar prepaint bounds. The source
/// recomputes on resize, flips to the left only at >=600px, and never clamps Y.
struct ShortcutEmojiPosition {
    owner: WeakEntity<Shortcuts>,
    child: AnyElement,
}
impl IntoElement for ShortcutEmojiPosition {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for ShortcutEmojiPosition {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let child = self.child.request_layout(window, cx);
        (
            window.request_layout(
                Style {
                    position: Position::Absolute,
                    display: Display::Flex,
                    ..Style::default()
                },
                [child],
                cx,
            ),
            (),
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(owner) = self.owner.upgrade() else {
            return;
        };
        let page = owner.read(cx);
        let width = window.rem_size() * (430. / 16.);
        let left = page.emoji_text_bounds.left();
        let origin = point(
            if left + width > window.viewport_size().width
                && window.viewport_size().width >= window.rem_size() * (600. / 16.)
            {
                left - width - window.rem_size() * (5. / 16.)
            } else {
                left
            },
            page.emoji_toolbar_bounds.bottom(),
        );
        window.with_element_offset(origin - bounds.origin, |window| {
            self.child.prepaint(window, cx)
        });
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}
