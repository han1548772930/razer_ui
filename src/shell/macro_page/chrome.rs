use super::*;

#[derive(Clone, Copy)]
pub(super) enum Menu {
    Selector,
    More,
    Sort,
    Tree(u64),
}

fn icon_button(
    id: &'static str,
    normal: &'static str,
    hover: &'static str,
    active: &'static str,
    label: String,
) -> BaseButton {
    BaseButton::new(id)
        .group(id)
        .accessibility_label(label)
        .size(css(20.))
        .p_0()
        .flex_shrink_0()
        .child(
            div()
                .relative()
                .size_full()
                .child(img(normal).size_full().group_hover(id, |s| s.opacity(0.)))
                .child(
                    img(hover)
                        .absolute()
                        .inset_0()
                        .size_full()
                        .opacity(0.)
                        .group_hover(id, |s| s.opacity(1.)),
                )
                .child(
                    img(active)
                        .absolute()
                        .inset_0()
                        .size_full()
                        .opacity(0.)
                        .group_active(id, |s| s.opacity(1.)),
                ),
        )
}

impl MacroPage {
    pub(super) fn navigation(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let help = self.tab == MacroTab::Help;
        // Mn gates the profile bar; dn also gates the initial selector.
        let profile_disabled = self.profile_actions_blocked();
        let selector_disabled = profile_disabled || self.tutorial == Tutorial::Initial;
        let angle = motion::transition(
            "macro-selector-angle",
            if self.selector_open {
                std::f32::consts::PI
            } else {
                0.
            },
            Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
            window,
            cx,
        );
        let hover = window.use_keyed_state("macro-selector-hover", cx, |_, _| false);
        let border_progress = motion::transition(
            "macro-selector-border",
            if self.selector_open || *hover.read(cx) && !selector_disabled {
                1.
            } else {
                0.
            },
            Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
            window,
            cx,
        );
        let channel = |to: u8| (81. + (f32::from(to) - 81.) * border_progress) / 255.;
        let border = Rgba {
            r: channel(0x44),
            g: channel(0xd6),
            b: channel(0x2c),
            a: 1.,
        };
        let selector = BaseButton::new("macro-selector")
            .w_full()
            .h(css(27.))
            .p_0()
            .px(css(5.))
            .justify_start()
            .disabled(selector_disabled)
            .border_1()
            .border_color(border)
            .on_hover(move |value, _, cx| {
                hover.update(cx, |hover, cx| {
                    *hover = *value;
                    cx.notify();
                })
            })
            .text_size(css(14.))
            .line_height(css(17.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_left()
                    .child(self.selected_name()),
            )
            .child(
                div()
                    .w(css(29.))
                    .h(css(25.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::default()
                            .path("synapse/expand.svg")
                            .size(css(10.))
                            .transform(Transformation::rotate(radians(angle))),
                    ),
            );
        let field = if self.rename.is_some() && !self.rename_in_tree {
            self.rename_field(cx)
        } else if selector_disabled {
            selector.into_any_element()
        } else {
            self.popup("macro-selector-popup", selector, Menu::Selector, window, cx)
        };
        let more = BaseButton::new("macro-more")
            .size(css(24.))
            .p_0()
            .border_1()
            .border_color(rgb(0x515151))
            .disabled(selector_disabled)
            .hover(|s| s.border_color(rgb(0x44d62c)))
            .child(img("synapse/profile-more.svg").size_full());
        let more = if selector_disabled {
            more.into_any_element()
        } else {
            self.popup("macro-more-popup", more, Menu::More, window, cx)
        };
        h_flex()
            .id("macro-navbar")
            .h(css(46.))
            .w_full()
            .flex_shrink_0()
            .bg(cx.theme().sidebar)
            .border_b_2()
            .border_color(cx.theme().title_bar)
            .items_center()
            .child(
                h_flex()
                    .id("macro-profile-bar")
                    .flex_1()
                    .when(profile_disabled, |v| v.opacity(0.8))
                    .when(self.recording_busy(), |v| v.opacity(0.3))
                    .flex_basis(relative(0.25))
                    .ml(css(10.))
                    .child(
                        img(if help {
                            "synapse/profile-unsupported.svg"
                        } else {
                            "synapse/profile.svg"
                        })
                        .size(css(26.))
                        .flex_shrink_0(),
                    )
                    .child(div().w(css(250.)).flex_shrink_0().px(css(10.)).child(field))
                    .child(more)
                    .child(div().w(css(1.)).h(css(20.)).ml(css(10.)).bg(rgb(0x515151)))
                    .child(
                        div()
                            .relative()
                            .ml(css(10.))
                            .child(
                                BaseButton::new("macro-new-link")
                                    .p_0()
                                    .text_size(css(12.))
                                    .underline()
                                    .disabled(profile_disabled)
                                    .hover(|s| s.text_color(rgb(0x44d62c)))
                                    .active(|s| s.opacity(0.7))
                                    .child(tr("TEXT_PROFILE_BAR_NEW_MACRO"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.create_entry(EntryKind::Macro, window, cx)
                                    })),
                            )
                            .when(
                                self.tutorial == Tutorial::Initial
                                    && self.macro_count() == 0
                                    && !help,
                                |v| {
                                    v.child(
                                        TutorialIndicator::new("macro-new-indicator")
                                            .absolute()
                                            .left(relative(1.))
                                            .ml(css(-9.))
                                            .top(relative(0.5))
                                            .mt(css(-18.)),
                                    )
                                },
                            ),
                    ),
            )
            .child(
                h_flex()
                    .h(css(46.))
                    .flex_shrink_0()
                    .justify_center()
                    .items_center()
                    .children(
                        [MacroTab::MyMacros, MacroTab::KeyBinds]
                            .into_iter()
                            .map(|tab| {
                                let selected = self.tab == tab;
                                BaseButton::new(tab.id())
                                    .role(Role::Tab)
                                    .selected(selected)
                                    .mx(css(10.))
                                    .px(css(10.))
                                    .py(css(7.))
                                    .rounded(css(14.))
                                    .text_size(css(12.))
                                    .line_height(css(14.))
                                    .font_family("Roboto")
                                    .disabled(
                                        tab == MacroTab::KeyBinds
                                            && !selected
                                            && self.macro_count() == 0,
                                    )
                                    .bg(if selected { rgb(0x44d62c) } else { rgba(0) })
                                    .text_color(if selected { rgb(0) } else { rgb(0x5d5d5d) })
                                    .when(!selected, |b| {
                                        b.hover(|s| s.bg(rgb(0x2d2d2d)).text_color(rgb(0xcccccc)))
                                    })
                                    .child(tr(tab.key()).to_uppercase())
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.set_tab(tab, window, cx)
                                    }))
                            }),
                    ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .flex_basis(relative(0.25))
                    .justify_end()
                    .pr(css(10.))
                    .child(
                        icon_button(
                            "macro-help-button",
                            if help {
                                "synapse/help-active.svg"
                            } else {
                                "synapse/help-default.svg"
                            },
                            if help {
                                "synapse/help-active.svg"
                            } else {
                                "synapse/help-hover.svg"
                            },
                            "synapse/help-active.svg",
                            tr("HELP"),
                        )
                        .size(css(24.))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.set_tab(MacroTab::Help, window, cx)
                        })),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn rename_field(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("macro-rename-editor")
            .w_full()
            .capture_action(cx.listener(
                |this, _: &gpui_kit::component::input::Escape, window, cx| {
                    this.cancel_rename(window, cx);
                    cx.stop_propagation();
                },
            ))
            .child(
                Input::new(&self.name)
                    .appearance(false)
                    .w_full()
                    .h(css(27.))
                    .px(css(5.))
                    .py_0()
                    .bg(rgb(0x111111))
                    .border_1()
                    .border_color(rgb(0x44d62c))
                    .rounded_none()
                    .text_size(css(14.))
                    .line_height(css(17.)),
            )
            .into_any_element()
    }

    fn selector_menu(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let opacity = Presence::new("macro-selector-opacity", true)
            .transition(Transition::new(Duration::from_millis(100)).easing(Easing::Linear))
            .sample(window, cx)
            .progress;
        let search = div()
            .relative()
            .w_full()
            .h(css(27.))
            .border_1()
            .border_color(rgb(0x515151))
            .hover(|s| s.border_color(rgb(0x44d62c)))
            .child(
                Input::new(&self.search)
                    .appearance(false)
                    .w_full()
                    .h_full()
                    .border_0()
                    .p_0()
                    .pl(css(25.))
                    .pr(css(24.))
                    .text_size(css(14.))
                    .line_height(css(17.)),
            )
            .child(
                img("synapse/profiles-search.svg")
                    .absolute()
                    .left(css(5.))
                    .top(css(5.))
                    .size(css(15.)),
            )
            .when(!self.search.read(cx).value().is_empty(), |v| {
                v.child(
                    BaseButton::new("macro-clear-search")
                        .absolute()
                        .right_0()
                        .top_0()
                        .size(css(25.))
                        .p_0()
                        .child(img("synapse/profiles-clear.svg").size(css(15.)))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.search.update(cx, |s, cx| s.set_value("", window, cx))
                        })),
                )
            });
        let sort = BaseButton::new("macro-sort")
            .h(css(27.))
            .w(css(180.))
            .px(css(5.))
            .py_0()
            .border_1()
            .border_color(rgb(0x515151))
            .hover(|s| s.border_color(rgb(0x44d62c)))
            .justify_between()
            .text_size(css(14.))
            .child(tr(self.sort.key()))
            .child(img("synapse/expand.svg").size(css(10.)));
        v_flex()
            .id("macro-dropdown")
            .occlude()
            .w(css(340.))
            .min_h(css(200.))
            .p(css(20.))
            .bg(rgb(0))
            .border_1()
            .border_color(rgb(0x515151))
            .opacity(opacity)
            .text_color(rgb(0xcccccc))
            .child(search)
            .child(
                h_flex()
                    .my(css(13.))
                    .justify_between()
                    .child(div().text_size(css(12.)).child(tr("TEXT_MACRO_SORTBY")))
                    .child(self.popup("macro-sort-popup", sort, Menu::Sort, window, cx)),
            )
            .child(
                h_flex()
                    .my(css(10.))
                    .py(css(10.))
                    .border_t_1()
                    .border_color(rgb(0x515151))
                    .child(
                        div()
                            .flex_1()
                            .child(tr("TEXT_PROFILE_BAR_DROPDOWN_ADD_MACRO")),
                    )
                    .child(
                        icon_button(
                            "macro-dropdown-new",
                            "synapse/macro/new.svg",
                            "synapse/macro/new-hover.svg",
                            "synapse/macro/new-active.svg",
                            tr("TEXT_PROFILE_BAR_NEW_MACRO"),
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.create_entry(EntryKind::Macro, window, cx)
                        })),
                    )
                    .child(
                        icon_button(
                            "macro-dropdown-folder",
                            "synapse/macro/folder-add.svg",
                            "synapse/macro/folder-add-hover.svg",
                            "synapse/macro/folder-add-active.svg",
                            tr("TEXT_NEW_MACRO_FOLDER_TOOLTIP"),
                        )
                        .ml(css(10.))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.create_entry(EntryKind::Folder, window, cx)
                        })),
                    ),
            )
            .child(self.tree(window, cx))
            .into_any_element()
    }

    fn more_menu(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let opacity = Presence::new("macro-more-opacity", true)
            .transition(Transition::new(Duration::from_millis(100)).easing(Easing::Linear))
            .sample(window, cx)
            .progress;
        v_flex()
            .id("macro-more-menu")
            .occlude()
            .w(css(180.))
            .bg(rgb(0))
            .border_1()
            .border_color(rgb(0x5d5d5d))
            .opacity(opacity)
            .child(
                super::tree::menu_action(
                    "macro-menu-add",
                    "TEXT_PROFILE_BAR_S3_DROPDOWN_ADD",
                    false,
                    window,
                    cx,
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.create_entry(EntryKind::Macro, window, cx)
                })),
            )
            .child(
                super::tree::menu_action(
                    "macro-menu-import",
                    "TEXT_PROFILE_BAR_S3_DROPDOWN_IMPORT",
                    self.transfer_busy || self.recording_busy(),
                    window,
                    cx,
                )
                .on_click(cx.listener(|this, _, window, cx| this.import_xml(window, cx))),
            )
            .child(div().h(css(1.)).mx(css(6.)).my(css(4.)).bg(rgb(0x5d5d5d)))
            .child(
                super::tree::menu_action(
                    "macro-menu-rename",
                    "TEXT_PROFILE_BAR_S3_DROPDOWN_RENAME",
                    self.current.is_none(),
                    window,
                    cx,
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    if let Some(id) = this.current {
                        this.start_rename(id, false, window, cx);
                    }
                })),
            )
            .child(
                super::tree::menu_action(
                    "macro-menu-duplicate",
                    "TEXT_PROFILE_BAR_S3_DROPDOWN_DUPLICATE",
                    self.current.is_none(),
                    window,
                    cx,
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.finish_pending_edits(window, cx);
                    this.duplicate_current(cx)
                })),
            )
            .child(
                super::tree::menu_action(
                    "macro-menu-export",
                    "TEXT_PROFILE_BAR_S3_DROPDOWN_EXPORT",
                    self.current.is_none() || self.transfer_busy || self.recording_busy(),
                    window,
                    cx,
                )
                .on_click(cx.listener(|this, _, window, cx| this.export_xml(window, cx))),
            )
            .child(div().h(css(1.)).mx(css(6.)).my(css(4.)).bg(rgb(0x5d5d5d)))
            .child(
                super::tree::menu_action(
                    "macro-menu-delete",
                    "TEXT_PROFILE_BAR_S3_DROPDOWN_DELETE",
                    self.current.is_none(),
                    window,
                    cx,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    if let Some(id) = this.current {
                        this.request_delete(id, cx);
                    }
                })),
            )
            .into_any_element()
    }

    pub(super) fn popup(
        &self,
        id: impl Into<ElementId>,
        trigger: BaseButton,
        menu: Menu,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let owner = cx.entity().downgrade();
        let open = match menu {
            Menu::Selector => self.selector_open,
            Menu::More => self.more_open,
            Menu::Sort => self.sort_open,
            Menu::Tree(id) => self.tree_menu == Some(id),
        };
        gpui_kit::base::Popover::new(id)
            .anchor(Anchor::TopLeft)
            // Pt .profile-act top26 follows a 24px more trigger.
            .offset(if matches!(menu, Menu::More | Menu::Tree(_)) {
                window.rem_size() * (2. / 16.)
            } else {
                px(0.)
            })
            .open(open)
            .overlay_closable(
                !matches!(menu, Menu::Selector) || self.tree_menu.is_none() && !self.sort_open,
            )
            .trigger_with(move |_, _, _| trigger.into_any_element())
            .on_open_change(cx.listener(move |this, open: &bool, _, cx| {
                match menu {
                    Menu::Selector => {
                        this.selector_open = *open;
                        this.sort_open = false;
                        this.tree_menu = None;
                        if *open {
                            this.more_open = false;
                        }
                    }
                    Menu::More => {
                        this.more_open = *open;
                        if *open {
                            this.selector_open = false;
                            this.sort_open = false;
                            this.tree_menu = None;
                        }
                    }
                    Menu::Sort => this.sort_open = *open,
                    Menu::Tree(id) => this.tree_menu = if *open { Some(id) } else { None },
                }
                cx.notify();
            }))
            .content(move |_, window, cx| {
                owner
                    .update(cx, |this, cx| match menu {
                        Menu::Selector => this.selector_menu(window, cx),
                        Menu::More => this.more_menu(window, cx),
                        Menu::Sort => this.sort_options(cx),
                        Menu::Tree(id) => this.tree_menu(id, window, cx),
                    })
                    .unwrap_or_else(|_| div().into_any_element())
            })
            .into_any_element()
    }

    fn sort_options(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .id("macro-sort-options")
            .occlude()
            .w(css(180.))
            .bg(rgb(0))
            .border_1()
            .border_color(rgb(0x515151))
            .children(Sort::ALL.into_iter().enumerate().map(|(i, sort)| {
                BaseButton::new(("macro-sort-option", i))
                    .w_full()
                    .h(css(27.))
                    .px(css(5.))
                    .py_0()
                    .justify_start()
                    .hover(|s| s.bg(rgba(0xffffff1a)))
                    .child(tr(sort.key()))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.sort = sort;
                        this.sort_open = false;
                        cx.notify();
                    }))
            }))
            .into_any_element()
    }
}
