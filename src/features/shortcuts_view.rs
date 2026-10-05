// Current 94608/Oe, Te, be and 55.4e8559cb CSS. Recording belongs to a row.
impl Shortcuts {
    fn shortcut_row(
        &self,
        item: &Shortcut,
        placeholder: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = self
            .draft
            .as_ref()
            .is_some_and(|draft| draft.value.id == item.id);
        let warning = !item.input.is_empty()
            && self
                .items
                .iter()
                .any(|other| other.id != item.id && other.same_chord(item));
        let recording = self.recording_id.as_deref() == Some(item.id.as_str());
        let menu = self.menu_for.as_deref() == Some(item.id.as_str());
        let row_id = item.id.clone();
        let edit_id = item.id.clone();
        let record_id = item.id.clone();
        let more_id = item.id.clone();
        let menu_owner = cx.entity().downgrade();
        let mut row = h_flex()
            .id(SharedString::from(format!("shortcut-row-{}", item.id)))
            .test_support()
            .relative()
            .w(surface::css(600.))
            .ml(surface::css(-10.))
            .px(surface::css(20.))
            .py(surface::css(15.))
            .justify_between()
            .border_1()
            .border_color(if selected { rgb(0x44d62c) } else { rgba(0) })
            .hover(|s| s.bg(rgba(0xffffff1a)))
            .child(
                gpui_kit::base::Button::new(SharedString::from(format!(
                    "shortcut-edit-{}",
                    item.id
                )))
                .accessibility_label(i18n::t("EDIT"))
                .w(relative(0.58))
                .min_w_0()
                .flex_shrink_1()
                .on_click(cx.listener(move |this, _, window, cx| {
                    if !this.recording {
                        this.begin(Some(edit_id.clone()), false, window, cx);
                    }
                }))
                .child(if placeholder {
                    v_flex()
                        .child(
                            div()
                                .my(surface::css(14.))
                                .w(surface::css(59.))
                                .h(surface::css(9.))
                                .bg(rgba(0x7070704d)),
                        )
                        .child(
                            div()
                                .relative()
                                .top(surface::css(-5.))
                                .w(surface::css(128.))
                                .h(surface::css(15.))
                                .bg(rgba(0xcccccc4d)),
                        )
                        .into_any_element()
                } else {
                    v_flex()
                        .w_full()
                        .text_left()
                        .child(
                            div()
                                .text_size(surface::css(10.))
                                .text_color(rgb(0x6a6a6a))
                                .child(i18n::t(item.output.category()).to_uppercase()),
                        )
                        .child(
                            div()
                                .mr(surface::css(10.))
                                .whitespace_normal()
                                .child(self.output_label(&item.output)),
                        )
                        .into_any_element()
                }),
            );
        if !placeholder {
            row = row.when(warning, |row| {
                row.child(
                    div()
                        .id(SharedString::from(format!("shortcut-warning-{}", row_id)))
                        .group(SharedString::from(format!("shortcut-warning-{}", row_id)))
                        .relative()
                        .size(surface::css(20.))
                        .flex_shrink_0()
                        .mr(surface::css(10.))
                        .mt(surface::css(10.))
                        .child(img("synapse/automation-icon_warning.svg").size_full())
                        .child(
                            div()
                                .absolute()
                                .right(surface::css(2.))
                                .top(surface::css(22.))
                                .w(surface::css(300.))
                                .h(surface::css(49.))
                                .px(surface::css(10.))
                                .py(surface::css(8.))
                                .bg(rgb(0))
                                .border_1()
                                .border_color(rgb(0x5d5d5d))
                                .invisible()
                                .group_hover(
                                    SharedString::from(format!("shortcut-warning-{}", row_id)),
                                    |s| s.visible(),
                                )
                                .child(i18n::t("DUPLICATE_SHORTCUT_MESSAGE")),
                        ),
                )
            });
            let duplicate_id = item.id.clone();
            let delete_id = item.id.clone();
            let edit_id = item.id.clone();
            let chord = item.chord();
            let record = div()
                .id(SharedString::from(format!("shortcut-record-{}", item.id)))
                .test_support()
                .role(Role::Button)
                .aria_label(i18n::t("SHORTCUT_KEY_INPUT"))
                .when(recording, |s| s.track_focus(&self.recording_focus))
                .capture_key_down(
                    cx.listener(|this, event, window, cx| this.capture(event, window, cx)),
                )
                .w(surface::css(190.))
                .min_h(surface::css(27.))
                .flex_shrink_0()
                .border_1()
                .border_color(if warning {
                    rgb(0xfd8611)
                } else if recording {
                    rgb(0x44d62c)
                } else {
                    rgb(0x5d5d5d)
                })
                .when(!warning, |s| s.hover(|s| s.border_color(rgb(0x44d62c))))
                .pl(surface::css(5.))
                .pr(surface::css(22.))
                .py(surface::css(5.))
                .text_size(surface::css(14.))
                .line_height(surface::css(17.))
                .text_color(if chord.is_empty() {
                    rgb(0x707070)
                } else {
                    rgb(0xcccccc)
                })
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(move |this, _, window, cx| {
                        if this.recording_id.as_deref() != Some(record_id.as_str()) {
                            this.start_recording(record_id.clone(), window, cx);
                        }
                    }),
                )
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(|this, event: &MouseDownEvent, _, cx| {
                        this.record_input("RightClick".into(), event.modifiers, cx);
                    }),
                )
                .on_mouse_down(
                    MouseButton::Middle,
                    cx.listener(|this, event: &MouseDownEvent, _, cx| {
                        this.record_input("ScrollButton".into(), event.modifiers, cx);
                    }),
                )
                .on_mouse_down(
                    MouseButton::Navigate(NavigationDirection::Back),
                    cx.listener(|this, event: &MouseDownEvent, _, cx| {
                        this.record_input("Button4".into(), event.modifiers, cx);
                    }),
                )
                .on_mouse_down(
                    MouseButton::Navigate(NavigationDirection::Forward),
                    cx.listener(|this, event: &MouseDownEvent, _, cx| {
                        this.record_input("Button5".into(), event.modifiers, cx);
                    }),
                )
                .child(if chord.is_empty() && !recording {
                    i18n::t("SHORTCUT_KEY_INPUT")
                } else {
                    chord
                });
            let actions = h_flex()
                .relative()
                .flex_shrink_0()
                .child(record)
                .child(
                    gpui_kit::base::Button::new(SharedString::from(format!(
                        "shortcut-more-{}",
                        item.id
                    )))
                    .accessibility_label(i18n::t("EDIT"))
                    .ml(surface::css(20.))
                    .size(surface::css(26.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_1()
                    .border_color(if menu { rgb(0x44d62c) } else { rgba(0) })
                    .hover(|s| s.border_color(rgb(0x777777)))
                    .active(|s| s.border_color(rgb(0x44d62c)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.menu_for = if this.menu_for.as_deref() == Some(more_id.as_str()) {
                            None
                        } else {
                            Some(more_id.clone())
                        };
                        cx.notify();
                    }))
                    .child(
                        img(if menu {
                            "synapse/nav-more-hover.svg"
                        } else {
                            "synapse/macro/binding-more.svg"
                        })
                        .size(surface::css(20.)),
                    )
                    .when(menu, |button| {
                        button.child(
                            canvas(
                                |_, _, _| (),
                                move |bounds, _, _, cx| {
                                    let _ = menu_owner
                                        .update(cx, |this, _| this.menu_button_bounds = bounds);
                                },
                            )
                            .absolute()
                            .inset_0(),
                        )
                    }),
                )
                .when(menu, |view| {
                    view.child(
                        deferred(
                            v_flex()
                                .id(SharedString::from(format!("shortcut-menu-{}", item.id)))
                                .absolute()
                                .right(surface::css(
                                    if window.viewport_size().width / window.rem_size() * 16.
                                        <= 850.
                                    {
                                        0.
                                    } else {
                                        -124.
                                    },
                                ))
                                .top(surface::css(27.))
                                .w(surface::css(150.))
                                .h(surface::css(85.))
                                .border_1()
                                .border_color(rgb(0x5d5d5d))
                                .bg(rgb(0))
                                .on_mouse_down_out(cx.listener(
                                    |this, event: &MouseDownEvent, _, cx| {
                                        // Te.clickAway excludes the original dots trigger.
                                        if !this.menu_button_bounds.contains(&event.position) {
                                            this.menu_for = None;
                                            cx.notify();
                                        }
                                    },
                                ))
                                .child(
                                    shortcut_menu_action(
                                        format!("shortcut-menu-edit-{}", item.id),
                                        "EDIT",
                                    )
                                    .on_click(cx.listener(
                                        move |this, _, window, cx| {
                                            this.begin(Some(edit_id.clone()), false, window, cx)
                                        },
                                    )),
                                )
                                .child(
                                    shortcut_menu_action(
                                        format!("shortcut-duplicate-{}", item.id),
                                        "DUPLICATE",
                                    )
                                    .on_click(cx.listener(
                                        move |this, _, window, cx| {
                                            this.begin(Some(duplicate_id.clone()), true, window, cx)
                                        },
                                    )),
                                )
                                .child(
                                    shortcut_menu_action(
                                        format!("shortcut-delete-{}", item.id),
                                        "DELETE",
                                    )
                                    .on_click(cx.listener(
                                        move |this, _, window, cx| {
                                            this.menu_for = None;
                                            this.delete(delete_id.clone(), window, cx);
                                        },
                                    )),
                                ),
                        )
                        .priority(3),
                    )
                });
            row = row.child(actions);
        }
        row.when(
            self.delete_confirmation.as_deref() == Some(item.id.as_str()),
            |row| row.child(self.delete_popover(item, cx)),
        )
        .into_any_element()
    }
}

fn shortcut_menu_action(id: String, key: &str) -> gpui_kit::base::Button {
    gpui_kit::base::Button::new(SharedString::from(id))
        .w_full()
        .h(surface::css(27.))
        .px(surface::css(6.))
        .py(surface::css(5.))
        .text_left()
        .line_height(surface::css(17.))
        .hover(|s| s.bg(rgb(0x222222)))
        .child(i18n::t(key))
}

impl Render for Shortcuts {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_emoji_scroll(cx);
        let editing = self.draft.is_some();
        let rows = self
            .items
            .iter()
            .map(|item| self.shortcut_row(item, false, window, cx))
            .collect::<Vec<_>>();
        let placeholder = self
            .draft
            .as_ref()
            .filter(|draft| !self.items.iter().any(|item| item.id == draft.value.id))
            .map(|draft| self.shortcut_row(&draft.value, true, window, cx));
        v_flex()
            .id("global-shortcuts-workspace")
            .test_support()
            .relative()
            .track_focus(&self.list_focus)
            .w_full()
            .min_w(surface::css(600.))
            .items_center()
            .mt(surface::css(10.))
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .font_family("Roboto")
            .text_color(rgb(0xcccccc))
            .child(
                v_flex()
                    .id("global-shortcuts")
                    .relative()
                    .w(surface::css(600.))
                    .flex_shrink_0()
                    .pt(surface::css(20.))
                    .px(surface::css(10.))
                    .pb(surface::css(30.))
                    .mb(surface::css(10.))
                    .bg(rgb(0x111111))
                    .rounded(surface::css(5.))
                    .child(
                        div()
                            .mx(surface::css(10.))
                            .mb(surface::css(20.))
                            .font_family("RazerF5")
                            .text_size(surface::css(16.))
                            .text_color(rgb(0x44d62c))
                            .child(i18n::t("SHORTCUTS_HEADER").to_uppercase()),
                    )
                    .child(
                        gpui_kit::base::Button::new("shortcut-add-icon")
                            .group("shortcut-add-icon")
                            .accessibility_label(i18n::t("ADD_SHORTCUT"))
                            .absolute()
                            .top(surface::css(20.))
                            .right(surface::css(20.))
                            .size(surface::css(18.))
                            .disabled(editing)
                            .styles(|s| s.disabled(|s| s.opacity(0.3)))
                            .child(
                                img("synapse/dashboard-add.svg")
                                    .absolute()
                                    .inset_0()
                                    .size_full()
                                    .group_hover("shortcut-add-icon", |s| s.invisible()),
                            )
                            .child(
                                img("synapse/dashboard-add-hover.svg")
                                    .absolute()
                                    .inset_0()
                                    .size_full()
                                    .invisible()
                                    .group_hover("shortcut-add-icon", |s| s.visible()),
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.begin(None, false, window, cx)
                            })),
                    )
                    .child(
                        div()
                            .px(surface::css(10.))
                            .mb(surface::css(20.))
                            .child(i18n::t("SHORTCUTS_MESSAGE")),
                    )
                    .children(rows)
                    .children(placeholder)
                    .child(
                        gpui_kit::base::Button::new("shortcut-add-card")
                            .accessibility_label(i18n::t("ADD_SHORTCUT"))
                            .relative()
                            .top(surface::css(17.))
                            .mb(surface::css(25.))
                            .w_full()
                            .h(surface::css(70.))
                            .px(surface::css(20.))
                            .py(surface::css(8.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(surface::css(5.))
                            .border_2()
                            .border_dashed()
                            .border_color(rgb(0x5d5d5d))
                            .hover(|s| s.border_color(rgb(0x44d62c)))
                            .active(|s| s.border_color(rgba(0x44d62cb3)))
                            .disabled(editing)
                            .styles(|s| s.disabled(|s| s.opacity(0.3)))
                            .child(i18n::t("ADD_SHORTCUT"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.begin(None, false, window, cx)
                            })),
                    ),
            )
            .when(editing, |view| view.child(self.editor(window, cx)))
            .when_some(self.source_alert.clone(), |view, alert| view.child(alert))
    }
}
