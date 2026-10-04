use super::*;

const PALETTE: [(&str, &str); 8] = [
    ("delay", "TEXT_ADD_MENU_DELAY"),
    ("keyboard", "TEXT_ADD_MENU_KEYBOARD"),
    ("mouse", "TEXT_ADD_MENU_MOUSE_FUNCTION"),
    ("macro", "TEXT_ADD_MENU_MACRO"),
    ("launch", "TEXT_ADD_MENU_LAUNCH"),
    ("command", "TEXT_ADD_MENU_RUN_COMMAND"),
    ("text", "TEXT_ADD_MENU_TEXT_FUNCTION"),
    ("loop", "TEXT_ADD_MENU_LOOP"),
];

impl MacroPage {
    pub(super) fn editor(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let viewport = window.viewport_size();
        let width = f32::from(viewport.width / window.rem_size()) * 16.;
        let height = f32::from(viewport.height / window.rem_size()) * 16.;
        // main .wrapper:1280px minimum, changed to 900 only below 900px;
        // editor's 270px margin override belongs to the <=1120px media query.
        let minimum = if width <= 900. { 900. } else { 1280. };
        let left = if width <= 1120. {
            290.
        } else {
            width.max(minimum) * 0.5 - 290.
        };
        let list_height = (height - 208.).max(450.);
        let empty = self.macro_count() == 0;
        let palette_disabled =
            empty || matches!(self.tutorial, Tutorial::Initial | Tutorial::Record);
        let page = cx.entity_id();
        div()
            .id("macro-source-wrapper")
            .relative()
            .w_full()
            .min_w(css(minimum))
            .h(css(list_height + 74.))
            .child(
                div()
                    .absolute()
                    .left(css(left))
                    .top(css(20.))
                    .w(css(600.))
                    .child(
                        v_flex()
                            .id("macro-editor")
                            .w_full()
                            .when(empty, |v| v.opacity(0.7))
                            .child(self.action_bar(cx))
                            .child(self.item_editor(list_height, window, cx)),
                    )
                    .child(
                        v_flex()
                            .id("macro-palette")
                            .absolute()
                            .left(css(-260.))
                            .top_0()
                            .w(css(250.))
                            .py(css(10.))
                            .bg(rgb(0x111111))
                            .rounded(css(5.))
                            .when(palette_disabled, |v| v.opacity(0.7))
                            // AI group is absent until an explicit source capability is supplied.
                            .child(
                                div()
                                    .h(css(30.))
                                    .px(css(10.))
                                    .font_family("RazerF5")
                                    .text_size(css(16.))
                                    .child(tr("TEXT_ADD_MENU").to_uppercase()),
                            )
                            .children(PALETTE.into_iter().map(|(kind, key)| {
                                let id = SharedString::from(format!("macro-palette-{kind}"));
                                BaseButton::new(id.clone())
                                    .group(id)
                                    .w_full()
                                    .h(css(40.))
                                    .px(css(10.))
                                    .py_0()
                                    .justify_start()
                                    .disabled(palette_disabled)
                                    .text_size(css(12.))
                                    .line_height(css(14.))
                                    .child(
                                        img(SharedString::from(format!(
                                            "synapse/macro/{kind}.svg"
                                        )))
                                        .size(css(20.))
                                        .mr(css(12.)),
                                    )
                                    .child(tr(key))
                                    .when(!palette_disabled, |button| {
                                        button.on_drag(
                                            ActionDrag {
                                                page,
                                                index: 0,
                                                palette_kind: Some(kind),
                                            },
                                            |_, _, _, cx| cx.new(|_| ActionDragPreview),
                                        )
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.add_action(kind, cx);
                                    }))
                            })),
                    )
                    .when(self.tutorial != Tutorial::Complete, |v| {
                        v.child(self.onboarding(cx))
                    }),
            )
            .into_any_element()
    }

    /// The 58190 editor keeps a 100px trailing drop-space below event rows.
    /// The action bar itself is mounted by `action_bar()` above, including
    /// when the event list is empty; this wrapper supplies source-sized local
    /// action rows and the reserved trailing drop area.
    fn item_editor(
        &self,
        list_height: f32,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let disabled = self.macro_count() == 0 || self.tutorial != Tutorial::Complete;
        let selected_count = self.selected_actions.len();
        let page = cx.entity_id();
        let action_count = self.actions().len();
        v_flex()
            .id("macro-item-list")
            .w_full()
            .h(css(list_height))
            .min_h(css(420.))
            .scrollable_y()
            .when(disabled, |v| v.opacity(0.7))
            .children(self.actions().iter().enumerate().map(|(index, action)| {
                let selected = self.selected_actions.contains(&index);
                let kind = action.kind;
                let dragging = ActionDrag {
                    page,
                    index,
                    palette_kind: None,
                };
                h_flex()
                    .id(("macro-action-row", index))
                    .w_full()
                    .h(css(42.))
                    .flex_shrink_0()
                    .items_center()
                    .px(css(12.))
                    .bg(if selected {
                        rgb(0x44d62c33)
                    } else {
                        rgb(0x222222)
                    })
                    .border_b_1()
                    .border_color(rgb(0x333333))
                    .hover(|s| s.bg(rgb(0x2d2d2d)))
                    .on_drag(dragging, move |_, _, _, cx| cx.new(|_| ActionDragPreview))
                    .drag_over::<ActionDrag>(move |style, drag, _, _| {
                        if drag.page == page && (drag.palette_kind.is_some() || drag.index != index)
                        {
                            style
                                .border_b_2()
                                .border_dashed()
                                .border_color(rgb(0x44d62c))
                        } else {
                            style
                        }
                    })
                    .on_drop(cx.listener(move |this, drag: &ActionDrag, _, cx| {
                        if drag.page == cx.entity_id() {
                            if let Some(kind) = drag.palette_kind {
                                this.add_action_at(kind, index, cx);
                            } else {
                                this.move_action(drag.index, index, cx);
                            }
                        }
                    }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_action_selection(index, cx);
                    }))
                    .child(
                        div()
                            .size(css(16.))
                            .mr(css(10.))
                            .border_1()
                            .border_color(if selected {
                                rgb(0x44d62c)
                            } else {
                                rgb(0x515151)
                            })
                            .bg(if selected { rgb(0x44d62c) } else { rgba(0) })
                            .child(if selected {
                                div()
                                    .text_size(css(12.))
                                    .text_color(rgb(0x111111))
                                    .child("✓")
                                    .into_any_element()
                            } else {
                                div().into_any_element()
                            }),
                    )
                    .child(
                        img(SharedString::from(format!(
                            "synapse/macro/{}.svg",
                            action.kind.icon()
                        )))
                        .size(css(20.))
                        .mr(css(12.)),
                    )
                    .child(tr(kind.label()))
                    .child(self.action_value_editor(index, kind, cx))
                    .into_any_element()
            }))
            .when(selected_count > 0, |v| {
                v.child(
                    h_flex()
                        .id("macro-selected-actions-bar")
                        .w_full()
                        .h(css(40.))
                        .flex_shrink_0()
                        .items_center()
                        .px(css(12.))
                        .bg(rgb(0x111111))
                        .border_t_1()
                        .border_color(rgb(0x333333))
                        .child(format!("{} {}", selected_count, tr("TEXT_ITEMS_SELECTED")))
                        .child(
                            BaseButton::new("macro-delete-selected-actions")
                                .ml_auto()
                                .size(css(24.))
                                .p_0()
                                .accessibility_label(tr("TEXT_TOOLTIP_DELETE_SELECTED_ITEMS"))
                                .child(
                                    div()
                                        .text_size(css(18.))
                                        .line_height(css(18.))
                                        .text_color(rgb(0xcccccc))
                                        .child("×"),
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.delete_selected_actions(cx);
                                })),
                        ),
                )
            })
            .child(
                div()
                    .id("macro-item-drop-space")
                    .w_full()
                    .h(css(100.))
                    .flex_shrink_0()
                    .bg(rgb(0x111111))
                    .drag_over::<ActionDrag>(move |style, drag, _, _| {
                        if drag.page == page {
                            style
                                .border_t_2()
                                .border_dashed()
                                .border_color(rgb(0x44d62c))
                        } else {
                            style
                        }
                    })
                    .on_drop(cx.listener(move |this, drag: &ActionDrag, _, cx| {
                        if drag.page == cx.entity_id() {
                            if let Some(kind) = drag.palette_kind {
                                this.add_action_at(kind, action_count, cx);
                            } else {
                                this.move_action(drag.index, action_count, cx);
                            }
                        }
                    })),
            )
            .into_any_element()
    }

    fn action_value_editor(
        &self,
        index: usize,
        kind: ActionKind,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let editable = kind != ActionKind::Macro;
        let item = self
            .actions()
            .get(index)
            .cloned()
            .unwrap_or_else(|| ActionItem::new(kind));
        if kind == ActionKind::Launch {
            return self.launch_value_popup(index, item, cx);
        }
        if matches!(kind, ActionKind::Delay | ActionKind::Loop)
            && self.editing_action == Some(index)
        {
            return self.numeric_value_editor(index, kind, cx);
        }
        if editable && kind != ActionKind::Text && self.editing_action == Some(index) {
            return Input::new(&self.action_editor)
                .id(("macro-action-value", index))
                .appearance(false)
                .w(css(match kind {
                    ActionKind::Delay | ActionKind::Loop => 88.,
                    ActionKind::Command => 200.,
                    ActionKind::Keyboard => 210.,
                    _ => 180.,
                }))
                .h(css(27.))
                .px(css(5.))
                .bg(rgb(0x111111))
                .border_1()
                .border_color(rgb(0x44d62c))
                .text_size(css(13.))
                .into_any_element();
        }
        if kind == ActionKind::Delay {
            return self.delay_value_editor(index, item, cx);
        }
        let value = item.value.clone();
        let label = if value.is_empty() {
            tr(match kind {
                ActionKind::Text => "TEXT_ENTER_TEXT",
                ActionKind::Command => "TEXT_INSERT_COMMAND",
                ActionKind::Launch => "TEXT_PROGRAM_OR_WEBSITE",
                ActionKind::Keyboard => "TEXT_NO_KEY_SET",
                ActionKind::Mouse => "TEXT_SELECT_AN_ACTION",
                _ => "TEXT_SELECT_AN_ACTION",
            })
        } else if kind == ActionKind::Delay {
            format!("{value}s")
        } else if kind == ActionKind::Loop {
            format!("{}: {}", tr(editors::loop_state_key(&item.state)), value)
        } else if kind.value_is_translation_key(&value) {
            tr(&value)
        } else {
            value
        };
        if kind == ActionKind::Text {
            return self.text_value_popup(index, label, cx);
        }
        if kind == ActionKind::Mouse {
            let trigger = BaseButton::new(("macro-mouse-value", index))
                .max_w(css(350.))
                .h(css(27.))
                .p_0()
                .text_size(css(14.))
                .text_color(if item.value.is_empty() {
                    rgb(0x707070)
                } else {
                    rgb(0xcccccc)
                })
                .hover(|s| s.text_color(rgb(0x44d62c)))
                .child(div().truncate().child(label));
            let owner = cx.entity().downgrade();
            return gpui_kit::base::Popover::new(("macro-mouse-options", index))
                .open(self.choice_action == Some(index))
                .anchor(Anchor::TopLeft)
                .trigger_with(move |_, _, _| trigger.into_any_element())
                .on_open_change(cx.listener(move |this, open: &bool, _, cx| {
                    this.choice_action = if *open { Some(index) } else { None };
                    cx.notify();
                }))
                .content(move |_, _, cx| {
                    owner
                        .update(cx, |_, cx| {
                            v_flex()
                                .w(css(160.))
                                .bg(rgb(0x111111))
                                .border_1()
                                .border_color(rgb(0x515151))
                                .children(editors::MOUSE_ACTION_KEYS.into_iter().enumerate().map(
                                    |(choice, key)| {
                                        BaseButton::new(("macro-mouse-option", choice))
                                            .w_full()
                                            .h(css(27.))
                                            .px(css(5.))
                                            .py_0()
                                            .justify_start()
                                            .text_size(css(14.))
                                            .hover(|s| {
                                                s.bg(rgb(0x222222)).text_color(rgb(0x44d62c))
                                            })
                                            .child(tr(key))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.choose_action_value(index, key, cx);
                                            }))
                                    },
                                ))
                                .into_any_element()
                        })
                        .unwrap_or_else(|_| div().into_any_element())
                })
                .into_any_element();
        }
        if kind == ActionKind::Loop {
            return self.loop_value_editor(index, item, cx);
        }
        BaseButton::new(("macro-action-edit", index))
            .when(
                editable
                    || matches!(
                        kind,
                        ActionKind::Delay
                            | ActionKind::Keyboard
                            | ActionKind::Mouse
                            | ActionKind::Loop
                    ),
                |b| {
                    b.min_w(css(150.))
                        .max_w(css(250.))
                        .h(css(24.))
                        .px(css(6.))
                        .py_0()
                        .text_size(css(11.))
                        .bg(rgb(0x333333))
                        .hover(|s| s.bg(rgb(0x444444)))
                        .justify_start()
                        .child(div().truncate().child(label))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            cx.stop_propagation();
                            if editable {
                                this.begin_action_edit(index, window, cx);
                            }
                        }))
                },
            )
            .into_any_element()
    }

    fn delay_value_editor(
        &self,
        index: usize,
        item: ActionItem,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let randomized = item.state == "randomized";
        let mode = BaseButton::new(("macro-delay-mode", index))
            .h(css(27.))
            .px(css(5.))
            .py_0()
            .text_size(css(10.))
            .bg(rgb(0x333333))
            .hover(|s| s.bg(rgb(0x444444)))
            .child(tr(if randomized {
                "RANDOMIZE_DELAY_TILE"
            } else {
                "TEXT_DELAY_SETTINGS"
            }))
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.toggle_delay_randomized(index, cx);
            }));
        let value = if randomized {
            let min = item.number_min.clone();
            let max = item.number_max.clone();
            if self.randomized_open == Some(index) {
                h_flex()
                    .items_center()
                    .child(
                        Input::new(&self.delay_min_editor)
                            .id(("macro-delay-min-input", index))
                            .appearance(false)
                            .w(css(60.))
                            .h(css(27.))
                            .px(css(4.))
                            .bg(rgb(0x111111))
                            .border_1()
                            .border_color(rgb(0x44d62c))
                            .text_size(css(11.)),
                    )
                    .child(div().text_size(css(11.)).child(" - "))
                    .child(
                        Input::new(&self.delay_max_editor)
                            .id(("macro-delay-max-input", index))
                            .appearance(false)
                            .w(css(60.))
                            .h(css(27.))
                            .px(css(4.))
                            .bg(rgb(0x111111))
                            .border_1()
                            .border_color(rgb(0x44d62c))
                            .text_size(css(11.)),
                    )
                    .into_any_element()
            } else {
                h_flex()
                    .items_center()
                    .child(
                        BaseButton::new(("macro-delay-min", index))
                            .h(css(27.))
                            .w(css(60.))
                            .px(css(3.))
                            .py_0()
                            .text_size(css(11.))
                            .justify_start()
                            .child(format!("({min}s"))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.begin_delay_bound_edit(index, DelayBound::Min, window, cx);
                            })),
                    )
                    .child(div().text_size(css(11.)).child(" - "))
                    .child(
                        BaseButton::new(("macro-delay-max", index))
                            .h(css(27.))
                            .w(css(60.))
                            .px(css(3.))
                            .py_0()
                            .text_size(css(11.))
                            .justify_start()
                            .child(format!("{max}s)"))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.begin_delay_bound_edit(index, DelayBound::Max, window, cx);
                            })),
                    )
                    .into_any_element()
            }
        } else {
            BaseButton::new(("macro-delay-value", index))
                .h(css(27.))
                .w(css(88.))
                .px(css(5.))
                .py_0()
                .text_size(css(11.))
                .justify_start()
                .child(format!("{}s", item.value))
                .on_click(cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.begin_action_edit(index, window, cx);
                }))
                .into_any_element()
        };
        h_flex()
            .items_center()
            .child(mode)
            .child(div().ml(css(4.)).child(value))
            .into_any_element()
    }

    fn loop_value_editor(
        &self,
        index: usize,
        item: ActionItem,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state_button = BaseButton::new(("macro-loop-state", index))
            .h(css(27.))
            .px(css(5.))
            .py_0()
            .text_size(css(11.))
            .bg(rgb(0x333333))
            .hover(|s| s.bg(rgb(0x444444)))
            .child(tr(editors::loop_state_key(&item.state)))
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.toggle_loop_state(index, cx);
            }));
        let up = BaseButton::new(("macro-loop-up", index))
            .w(css(18.))
            .h(css(13.5))
            .p_0()
            .focusable(false)
            .child(img("synapse/stepper-up.svg").size(css(8.)))
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.step_loop(index, 1, cx);
            }));
        let down = BaseButton::new(("macro-loop-down", index))
            .w(css(18.))
            .h(css(13.5))
            .p_0()
            .focusable(false)
            .child(img("synapse/stepper-down.svg").size(css(8.)))
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.step_loop(index, -1, cx);
            }));
        let value = BaseButton::new(("macro-loop-count", index))
            .h(css(27.))
            .w(css(70.))
            .px(css(5.))
            .py_0()
            .text_size(css(11.))
            .justify_start()
            .child(format!("{}x", item.value))
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                this.begin_action_edit(index, window, cx);
            }));
        h_flex()
            .items_center()
            .child(state_button)
            .child(
                h_flex()
                    .ml(css(4.))
                    .w(css(88.))
                    .h(css(27.))
                    .border_1()
                    .border_color(rgb(0x5d5d5d))
                    .child(value)
                    .child(v_flex().h(css(27.)).child(up).child(down)),
            )
            .into_any_element()
    }

    fn launch_value_popup(
        &self,
        index: usize,
        item: ActionItem,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let website = item.state == "website";
        let target = if website {
            &item.secondary_value
        } else {
            &item.value
        };
        let label = if target.is_empty() {
            tr("TEXT_PROGRAM_OR_WEBSITE")
        } else {
            target.clone()
        };
        let trigger = BaseButton::new(("macro-launch-target", index))
            .min_w(css(200.))
            .max_w(css(350.))
            .h(css(27.))
            .p_0()
            .text_size(css(14.))
            .text_color(if target.is_empty() {
                rgb(0x707070)
            } else {
                rgb(0xcccccc)
            })
            .hover(|s| s.text_color(rgb(0x44d62c)))
            .child(div().truncate().child(label));
        let owner = cx.entity().downgrade();
        gpui_kit::base::Popover::new(("macro-launch-popup", index))
            .open(self.launch_open == Some(index))
            .anchor(Anchor::TopLeft)
            .trigger_with(move |_, _, _| trigger.into_any_element())
            .on_open_change(cx.listener(move |this, open: &bool, window, cx| {
                if *open {
                    this.open_launch_editor(index, window, cx);
                } else {
                    this.launch_open = None;
                    this.launch_is_website = false;
                    cx.notify();
                }
            }))
            .content(move |_, _, cx| {
                owner
                    .update(cx, |this, cx| {
                        let is_website = this.launch_is_website;
                        let active_radio = |active: bool| {
                            div()
                                .size(css(14.))
                                .mr(css(8.))
                                .rounded_full()
                                .border_1()
                                .border_color(if active { rgb(0x44d62c) } else { rgb(0x707070) })
                                .bg(if active { rgb(0x44d62c) } else { rgb(0x111111) })
                        };
                        let program_radio = BaseButton::new(("macro-launch-program-radio", index))
                            .w_full()
                            .h(css(27.))
                            .p_0()
                            .justify_start()
                            .child(active_radio(!is_website))
                            .child(tr("TEXT_LAUNCH_PROGRAM"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.launch_is_website = false;
                                cx.notify();
                            }));
                        let website_radio = BaseButton::new(("macro-launch-website-radio", index))
                            .w_full()
                            .h(css(27.))
                            .p_0()
                            .justify_start()
                            .child(active_radio(is_website))
                            .child(tr("TEXT_LAUNCH_WEBSITE"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.launch_is_website = true;
                                cx.notify();
                            }));
                        let program = Input::new(&this.launch_program)
                            .id(("macro-launch-program-input", index))
                            .appearance(false)
                            .ml(css(30.))
                            .w(css(142.))
                            .h(css(27.))
                            .px(css(6.))
                            .bg(rgb(0x111111))
                            .border_1()
                            .border_color(rgb(0x5d5d5d))
                            .disabled(is_website)
                            .text_size(css(13.));
                        let website = Input::new(&this.launch_website)
                            .id(("macro-launch-website-input", index))
                            .appearance(false)
                            .ml(css(30.))
                            .w(css(164.))
                            .h(css(27.))
                            .px(css(6.))
                            .bg(rgb(0x111111))
                            .border_1()
                            .border_color(rgb(0x5d5d5d))
                            .disabled(!is_website)
                            .text_size(css(13.));
                        v_flex()
                            .relative()
                            .w(css(250.))
                            .p(css(20.))
                            .bg(rgb(0x111111))
                            .border_1()
                            .border_color(rgb(0x5d5d5d))
                            .rounded(css(5.))
                            .child(
                                BaseButton::new(("macro-launch-close", index))
                                    .absolute()
                                    .right_0()
                                    .top_0()
                                    .size(css(36.))
                                    .p_0()
                                    .hover(|s| s.bg(rgba(0xffffff1a)))
                                    .child(img("synapse/macro/close.svg").size(css(20.)))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.launch_open = None;
                                        this.launch_is_website = false;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .font_family("RazerF5")
                                    .text_size(css(16.))
                                    .line_height(css(19.))
                                    .text_color(rgb(0x44d62c))
                                    .mb(css(20.))
                                    .child(tr("TEXT_ADD_MENU_LAUNCH").to_uppercase()),
                            )
                            .child(program_radio)
                            .child(program)
                            .child(website_radio)
                            .child(website)
                            .child(
                                h_flex()
                                    .w_full()
                                    .justify_between()
                                    .mt(css(20.))
                                    .child(
                                        BaseButton::new(("macro-launch-cancel", index))
                                            .w(css(90.))
                                            .h(css(27.))
                                            .p_0()
                                            .bg(rgb(0x555555))
                                            .child(tr("TEXT_LAUNCH_CANCEL").to_uppercase())
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.launch_open = None;
                                                this.launch_is_website = false;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        BaseButton::new(("macro-launch-save", index))
                                            .w(css(90.))
                                            .h(css(27.))
                                            .p_0()
                                            .bg(rgb(0x44d62c))
                                            .text_color(rgb(0x111111))
                                            .child(tr("TEXT_LAUNCH_SAVE").to_uppercase())
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.save_launch_editor(cx);
                                            })),
                                    ),
                            )
                            .into_any_element()
                    })
                    .unwrap_or_else(|_| div().into_any_element())
            })
            .into_any_element()
    }

    fn text_value_popup(&self, index: usize, label: String, cx: &mut Context<Self>) -> AnyElement {
        let trigger = BaseButton::new(("macro-text-value", index))
            .max_w(css(350.))
            .h(css(27.))
            .p_0()
            .text_size(css(14.))
            .hover(|s| s.text_color(rgb(0x44d62c)))
            .child(div().truncate().child(label));
        let owner = cx.entity().downgrade();
        gpui_kit::base::Popover::new(("macro-text-popup", index))
            .open(self.editing_action == Some(index))
            .anchor(Anchor::TopLeft)
            .trigger_with(move |_, _, _| trigger.into_any_element())
            .on_open_change(cx.listener(move |this, open: &bool, window, cx| {
                if *open {
                    this.begin_action_edit(index, window, cx);
                } else if this.editing_action == Some(index) {
                    this.editing_action = None;
                    cx.notify();
                }
            }))
            .content(move |_, _, cx| {
                owner
                    .update(cx, |this, cx| {
                        let text = this.text_editor.clone();
                        let count = text.read(cx).value().encode_utf16().count();
                        v_flex()
                            .relative()
                            .w(css(250.))
                            .p(css(20.))
                            .bg(rgb(0x111111))
                            .border_1()
                            .border_color(rgb(0x5d5d5d))
                            .rounded(css(5.))
                            .child(
                                BaseButton::new(("macro-text-close", index))
                                    .absolute()
                                    .right_0()
                                    .top_0()
                                    .size(css(36.))
                                    .p_0()
                                    .hover(|s| s.bg(rgba(0xffffff1a)))
                                    .child(img("synapse/macro/close.svg").size(css(20.)))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.editing_action = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .font_family("RazerF5")
                                    .text_size(css(16.))
                                    .line_height(css(19.))
                                    .text_color(rgb(0x44d62c))
                                    .mb(css(20.))
                                    .child(tr("TEXT_TEXT_FUNCTION").to_uppercase()),
                            )
                            .child(
                                Textarea::new(&text)
                                    .appearance(false)
                                    .w(css(210.))
                                    .h(css(96.))
                                    .p(css(5.))
                                    .bg(rgb(0x111111))
                                    .border_1()
                                    .border_color(rgb(0x5d5d5d))
                                    .text_size(css(14.)),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .text_right()
                                    .text_size(css(14.))
                                    .text_color(rgb(0x707070))
                                    .mt(css(5.))
                                    .mb(css(20.))
                                    .child(format!("{count}/250")),
                            )
                            .child(
                                h_flex()
                                    .w_full()
                                    .justify_between()
                                    .child(
                                        BaseButton::new(("macro-text-cancel", index))
                                            .w(css(90.))
                                            .h(css(27.))
                                            .p_0()
                                            .bg(rgb(0x555555))
                                            .child(tr("TEXT_TEXT_FUNCTION_CANCEL").to_uppercase())
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.editing_action = None;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        BaseButton::new(("macro-text-save", index))
                                            .disabled(count == 0)
                                            .w(css(90.))
                                            .h(css(27.))
                                            .p_0()
                                            .bg(rgb(0x44d62c))
                                            .text_color(rgb(0x111111))
                                            .child(tr("TEXT_TEXT_FUNCTION_SAVE").to_uppercase())
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.finish_action_edit(window, cx);
                                            })),
                                    ),
                            )
                            .into_any_element()
                    })
                    .unwrap_or_else(|_| div().into_any_element())
            })
            .into_any_element()
    }

    fn action_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .id("macro-action-bar")
            .w_full()
            .h(css(54.))
            .pl(css(12.))
            .pr(css(10.))
            .py(css(12.))
            .rounded_t(css(5.))
            .bg(rgb(0x111111))
            .border_b_1()
            .border_color(rgb(0x222222))
            .child(
                h_flex()
                    .flex_1()
                    .mt(css(1.))
                    .child(
                        div()
                            .size(css(16.))
                            .mr(css(10.))
                            .border_1()
                            .border_color(rgb(0x515151)),
                    )
                    .child(img("synapse/macro/delay.svg").size(css(20.)).mr(css(10.)))
                    .when(self.current.is_some(), |v| {
                        v.child(div().text_size(css(14.)).child("0.000s"))
                    }),
            )
            .child(
                h_flex().flex_1().justify_center().child(
                    h_flex()
                        .h(css(27.))
                        .bg(rgb(0x707070))
                        .rounded(css(3.))
                        .border_1()
                        .border_color(rgba(0x0000004d))
                        .child(
                            BaseButton::new("macro-record")
                                .disabled(true)
                                .h_full()
                                .min_w(css(94.))
                                .px(css(10.))
                                .py_0()
                                .text_size(css(12.))
                                .line_height(css(14.))
                                .text_color(rgb(0xffffff))
                                .border_r_1()
                                .border_color(rgba(0x0000004d))
                                .child(img("synapse/macro/record.svg").size(css(12.)).mr(css(4.)))
                                .child(tr("TEXT_ACTION_BAR_RECORD").to_uppercase()),
                        )
                        .child(
                            BaseButton::new("macro-record-options")
                                .disabled(true)
                                .w(css(27.))
                                .h_full()
                                .p_0()
                                .child(img("synapse/macro/record-expand.svg").size(css(10.))),
                        ),
                ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .justify_end()
                    .child(
                        BaseButton::new("macro-undo")
                            .disabled(!self.can_undo())
                            .size(css(20.))
                            .p_0()
                            .mr(css(16.))
                            .child(img("synapse/macro/undo.svg").size_full())
                            .on_click(cx.listener(|this, _, _, cx| this.undo_action(cx))),
                    )
                    .child(
                        BaseButton::new("macro-redo")
                            .disabled(!self.can_redo())
                            .size(css(20.))
                            .p_0()
                            .flex_1()
                            .justify_start()
                            .child(img("synapse/macro/redo.svg").size(css(20.)))
                            .on_click(cx.listener(|this, _, _, cx| this.redo_action(cx))),
                    )
                    .child(
                        BaseButton::new("macro-save")
                            .disabled(!self.can_save())
                            .opacity(if self.can_save() { 1. } else { 0.3 })
                            .min_w(css(100.))
                            .h(css(27.))
                            .mr(css(10.))
                            .px(css(10.))
                            .py_0()
                            .rounded(css(3.))
                            .border_1()
                            .border_color(rgba(0x0000004d))
                            .bg(rgb(0x44d62c))
                            .text_color(rgb(0))
                            .text_size(css(12.))
                            .line_height(css(14.))
                            .child(tr("TEXT_LAUNCH_SAVE"))
                            .on_click(cx.listener(|this, _, _, cx| this.save_actions(cx))),
                    ),
            )
            .into_any_element()
    }

    fn onboarding(&self, cx: &mut Context<Self>) -> AnyElement {
        let initial = self.tutorial == Tutorial::Initial;
        let adding = self.tutorial == Tutorial::Add;
        let step = if initial {
            0
        } else if adding {
            2
        } else {
            1
        };
        v_flex()
            .id("macro-onboarding")
            .absolute()
            .top(css(63.))
            .left(css(if adding { 0. } else { 150. }))
            .w(css(300.))
            .p(css(20.))
            .bg(rgb(0x111111))
            .border_1()
            .border_color(rgb(0xfd8611))
            .rounded(css(3.))
            .text_center()
            .text_size(css(14.))
            .line_height(css(17.))
            .items_center()
            .shadow(vec![BoxShadow {
                inset: false,
                color: rgba(0x00000033).into(),
                offset: point(px(0.), px(6.)),
                blur_radius: px(10.),
                spread_radius: px(0.),
            }])
            .when(initial, |v| v.h(css(97.)))
            .when(adding, |v| v.h(css(239.)))
            .when(!initial, |v| {
                v.child(
                    BaseButton::new("macro-onboarding-skip")
                        .absolute()
                        .right(css(10.))
                        .top(css(10.))
                        .p_0()
                        .text_size(css(12.))
                        .underline()
                        .hover(|s| s.text_color(rgb(0x44d62c)))
                        .active(|s| s.opacity(0.7))
                        .child(tr("TEXT_MACRO_CONTENT_SKIP").to_uppercase())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.tutorial = Tutorial::Complete;
                            cx.notify();
                        })),
                )
                .child(
                    img(if adding {
                        "synapse/macro/onboarding-add.svg"
                    } else {
                        "synapse/macro/onboarding-record.svg"
                    })
                    .w(css(260.))
                    .h(css(if adding { 48.5 } else { 68. }))
                    .mb(css(10.)),
                )
                .child(
                    TutorialIndicator::new("macro-onboarding-indicator")
                        .absolute()
                        .left(css(if adding { -45. } else { 94. }))
                        .top(css(if adding { 19.12 } else { -55. })),
                )
            })
            .child(div().w_full().mb(css(10.)).child(tr(if initial {
                "TEXT_MACRO_CONTENT_FIRST"
            } else if adding {
                "TEXT_MACRO_CONTENT_THIRD"
            } else {
                "TEXT_MACRO_CONTENT_SECOND"
            })))
            .when(!initial, |v| {
                v.child(
                    BaseButton::new("macro-onboarding-next")
                        .w(css(90.))
                        .h(css(27.))
                        .p_0()
                        .mb(css(10.))
                        .bg(rgb(0xfd8611))
                        .border_1()
                        .border_color(rgba(0x0000004d))
                        .rounded(css(3.))
                        .text_color(rgb(0x212121))
                        .text_size(css(12.))
                        .hover(|s| s.bg(rgb(0xfcae61)))
                        .active(|s| s.bg(rgba(0xfd8611b3)))
                        .child(
                            tr(if adding {
                                "TEXT_MACRO_CONTENT_DONE"
                            } else {
                                "TEXT_MACRO_CONTENT_NEXT"
                            })
                            .to_uppercase(),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.tutorial = if this.tutorial == Tutorial::Record {
                                Tutorial::Add
                            } else {
                                Tutorial::Complete
                            };
                            cx.notify();
                        })),
                )
            })
            .child(h_flex().justify_center().children((0..3).map(|i| {
                div()
                    .size(css(6.))
                    .mr(css(6.))
                    .rounded_full()
                    .bg(if i == step {
                        rgb(0xfd8611)
                    } else {
                        rgb(0xcccccc)
                    })
            })))
            .into_any_element()
    }

    pub(super) fn key_binds(&self, _cx: &mut Context<Self>) -> AnyElement {
        // 21700: currentProfile == "" still mounts the warning and inert card;
        // only null returns no page. No validDevices are invented for assignment.
        v_flex()
            .id("macro-key-binds-page")
            .w_full()
            .min_w(css(900.))
            .child(
                h_flex()
                    .justify_center()
                    .mt(css(22.))
                    .text_size(css(14.))
                    .child(
                        img("synapse/macro/warning.svg")
                            .w(css(20.))
                            .h(css(17.))
                            .mr(css(10.)),
                    )
                    .child(tr("TEXT_MARCRO_WARING")),
            )
            .child(
                h_flex()
                    .w_full()
                    .justify_center()
                    .mt(css(20.))
                    .opacity(0.3)
                    .child(
                        v_flex()
                            .w(css(300.))
                            .h(css(230.))
                            .px(css(20.))
                            .py(css(8.))
                            .bg(rgb(0x222222))
                            .rounded(css(5.))
                            .border_2()
                            .border_dashed()
                            .border_color(rgb(0x5d5d5d))
                            .items_center()
                            .child(
                                div().relative().w(css(250.)).h(css(140.)).child(
                                    img("synapse/macro/add.svg")
                                        .absolute()
                                        .top(css(50.))
                                        .left(css(105.))
                                        .size(css(40.)),
                                ),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .text_center()
                                    .text_size(css(14.))
                                    .line_height(css(16.))
                                    .child(tr("TEXT_ASSIGN_MACRO_TO_DEVICES")),
                            ),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn help(&self, _cx: &mut Context<Self>) -> AnyElement {
        // 1519 C: body-widgets > widget-col.left > Support widget > anchor.
        div()
            .w_full()
            .min_w(css(900.))
            .pt(css(20.))
            .flex()
            .justify_center()
            .child(
                v_flex()
                    .w(css(600.))
                    .px(css(40.))
                    .py(css(30.))
                    .bg(rgb(0x111111))
                    .rounded(css(5.))
                    .child(
                        div()
                            .font_family("RazerF5")
                            .text_size(css(16.))
                            .text_color(rgb(0x44d62c))
                            .mb(css(20.))
                            .child(tr("SUPPORT").to_uppercase()),
                    )
                    .child(
                        BaseButton::new("macro-support-link")
                            .p_0()
                            .justify_start()
                            .text_color(rgb(0xcccccc))
                            .text_size(css(14.))
                            .child(tr("VISIT_MACRO_SUPPORT_PAGE"))
                            .on_click(|_, _, cx| {
                                cx.open_url("https://www.razer.com/search/macros?sel=support")
                            }),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn delete_confirmation(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(id) = self.deletion else {
            return div().into_any_element();
        };
        let folder = self
            .entries
            .iter()
            .find(|e| e.id == id)
            .is_some_and(|e| e.kind == EntryKind::Folder);
        let opacity = Presence::new("macro-delete-opacity", true)
            .transition(Transition::new(Duration::from_millis(300)).easing(Easing::Linear))
            .sample(window, cx)
            .progress;
        // Ut -> xt.profile-del: anchored after the profile menu, no modal scrim.
        v_flex()
            .id("macro-delete-confirmation")
            .absolute()
            .left(css(280.))
            .top(css(53.))
            .occlude()
            .w(css(300.))
            .p(css(20.))
            .items_center()
            .bg(rgb(0x111111))
            .rounded(css(3.))
            .border_1()
            .border_color(rgb(0xfd4949))
            .opacity(opacity)
            .text_size(css(14.))
            .line_height(css(17.))
            .child(
                div()
                    .text_color(rgb(0xc83200))
                    .font_weight(FontWeight::BOLD)
                    .mb(css(10.))
                    .child(
                        tr(if folder {
                            "TEXT_DELETE_FOLDER_LABEL"
                        } else {
                            "TEXT_DELETE_MACRO_LABEL"
                        })
                        .to_uppercase(),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .text_center()
                    .mb(css(10.))
                    .child(tr(if folder {
                        "TEXT_DELETE_FOLDER_DESC"
                    } else {
                        "TEXT_DELETE_MACRO_DESC"
                    })),
            )
            .child(
                BaseButton::new("macro-delete-confirm")
                    .min_w(css(90.))
                    .h(css(27.))
                    .px(css(10.))
                    .py(css(4.))
                    .bg(rgb(0xfd4949))
                    .border_1()
                    .border_color(rgba(0x0000004d))
                    .text_color(rgb(0x111111))
                    .text_size(css(12.))
                    .child(tr("TEXT_TOOLTIP_DELETE"))
                    .on_click(cx.listener(move |this, _, _, cx| this.delete_entry(id, cx))),
            )
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.deletion = None;
                cx.notify();
            }))
            .into_any_element()
    }
}
