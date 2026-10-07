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
        let palette_disabled = empty
            || self.recording_busy()
            || self.record_ui.open
            || matches!(self.tutorial, Tutorial::Initial | Tutorial::Record);
        let page = cx.entity_id();
        let baseline = std::rc::Rc::new(self.actions().to_vec());
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
                            .child(self.action_bar(window, cx))
                            .when(
                                self.recording_busy()
                                    || !self.recording.status.is_empty()
                                    || self.recording.error.is_some(),
                                |v| v.child(self.recording_status()),
                            )
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
                                let palette_disabled = palette_disabled
                                    || kind == "delay"
                                        && self.current_macro_type()
                                            != crate::features::macro_library::MacroType::Standard;
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
                                            self.action_drag(0, Some(kind), page, baseline.clone()),
                                            |drag, offset, _, cx| {
                                                cx.new(|_| ActionDragPreview {
                                                    drag: drag.clone(),
                                                    offset,
                                                })
                                            },
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let disabled = self.macro_count() == 0
            || self.recording_busy()
            || self.record_ui.open
            || self.tutorial != Tutorial::Complete;
        let page = cx.entity_id();
        if self.current_macro_type() == crate::features::macro_library::MacroType::Phased {
            return self.phased_editor(list_height, disabled, window, cx);
        }
        let action_count = self.actions().len();
        let baseline = std::rc::Rc::new(self.actions().to_vec());
        let editor_bounds = self.text_ui.editor_bounds.clone();
        v_flex()
            .id("macro-item-list")
            .test_support()
            .on_prepaint(move |bounds, _, _| editor_bounds.set(bounds))
            .w_full()
            .h(css(list_height))
            .min_h(css(420.))
            .when(disabled, |v| {
                v.capture_any_mouse_down(|_, _, cx| cx.stop_propagation())
                    .opacity(0.7)
            })
            .scrollable_y()
            .track_scroll(&self.recording.scroll)
            .children(
                (0..action_count)
                    .map(|index| self.action_row(index, baseline.clone(), disabled, window, cx)),
            )
            .child(
                div()
                    .id("macro-item-drop-space")
                    .w_full()
                    .h(css(100.))
                    .flex_shrink_0()
                    .bg(rgb(0x111111))
                    .on_drag_move::<ActionDrag>(move |event, _, cx| {
                        let drag = event.drag(cx);
                        if drag.page == page && event.bounds.contains(&event.event.position) {
                            drag.allowed.set(drag.allows(action_count));
                        }
                    })
                    .on_drop(cx.listener(move |this, drag: &ActionDrag, _, cx| {
                        this.drop_actions(drag, action_count, cx)
                    })),
            )
            .into_any_element()
    }

    pub(super) fn action_value_editor(
        &self,
        index: usize,
        kind: ActionKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let editable = kind != ActionKind::Macro;
        let item = self
            .actions()
            .get(index)
            .cloned()
            .unwrap_or_else(|| ActionItem::new(kind));
        if item.mouse_movement.is_some() {
            return h_flex()
                .id(("macro-movement-value", index))
                .test_support()
                .text_size(css(14.))
                .child(tr("TEXT_MOUSE_MOVEMENT"))
                .child(img("synapse/macro/delay.svg").size(css(20.)).mx(css(10.)))
                .child(format!("{}s", item.value))
                .into_any_element();
        }
        if kind == ActionKind::Macro {
            return self.nested_macro_editor(index, &item, window, cx);
        }
        if kind == ActionKind::Launch {
            return self.launch_value_popup(index, item, window, cx);
        }
        if kind == ActionKind::Keyboard {
            return self.keyboard_value_editor(index, &item, cx);
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
            return self.text_value_popup(index, label, window, cx);
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
                                                this.choose_mouse_action(index, choice as u8, cx);
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
        BaseButton::new(("macro-loop-count", index))
            .h(css(27.))
            .p_0()
            .text_size(css(14.))
            .justify_start()
            .hover(|s| s.text_color(rgb(0x44d62c)))
            .child(item.value)
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                this.begin_action_edit(index, window, cx);
            }))
            .into_any_element()
    }

    fn action_bar(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let record_trigger = self.record_ui.trigger_bounds.clone();
        let selected_count = self.selected_actions.len();
        let page = cx.entity_id();
        let selection_disabled =
            self.tutorial != Tutorial::Complete || self.recording_busy() || self.record_ui.open;
        h_flex()
            .id("macro-action-bar")
            .when(
                self.current_macro_type() != crate::features::macro_library::MacroType::Phased,
                |bar| {
                    bar.drag_over::<ActionDrag>(move |style, drag, _, _| {
                        row_drag::target_style(style, drag, page, 0)
                    })
                    .on_drop(
                        cx.listener(|this, drag: &ActionDrag, _, cx| {
                            this.drop_actions(drag, 0, cx)
                        }),
                    )
                },
            )
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
                        selection::checkbox(
                            "macro-select-all",
                            self.all_actions_selected(),
                            selection_disabled,
                            window,
                            cx,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.toggle_all_actions(cx);
                        })),
                    )
                    .when(selected_count == 0, |v| {
                        v.child(img("synapse/macro/delay.svg").size(css(20.)).mr(css(10.)))
                            .when(self.current.is_some(), |v| {
                                v.child(div().text_size(css(14.)).child(self.macro_type_status()))
                            })
                    })
                    .when(selected_count > 0, |v| {
                        v.child(
                            BaseButton::new("macro-delete-selected-actions")
                                .group("macro-delete-selected")
                                .disabled(selection_disabled)
                                .size(css(20.))
                                .mr(css(5.))
                                .p_0()
                                .active(|s| s.opacity(0.7))
                                .accessibility_label(tr("TEXT_TOOLTIP_DELETE_SELECTED_ITEMS"))
                                .child(
                                    div()
                                        .relative()
                                        .size_full()
                                        .child(img("synapse/macro/delete.svg").size_full())
                                        .child(
                                            img("synapse/macro/delete-hover.svg")
                                                .absolute()
                                                .inset_0()
                                                .size_full()
                                                .opacity(0.)
                                                .group_hover("macro-delete-selected", |s| {
                                                    s.opacity(1.)
                                                }),
                                        ),
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.delete_selected_actions(cx);
                                })),
                        )
                        .child(div().text_size(css(14.)).child(format!(
                            "{} {}",
                            selected_count,
                            tr("TEXT_ITEMS_SELECTED")
                        )))
                    }),
            )
            .child(
                h_flex().flex_1().justify_center().child(
                    h_flex()
                        .relative()
                        .h(css(27.))
                        .bg(rgb(0x707070))
                        .rounded(css(3.))
                        .border_1()
                        .border_color(rgba(0x0000004d))
                        .child(
                            BaseButton::new("macro-record")
                                .disabled(
                                    self.current.is_none()
                                        || self.tutorial != Tutorial::Complete
                                        || self.recording.stage == recording::Stage::Stopping,
                                )
                                .accessibility_label(self.record_label())
                                .h_full()
                                .min_w(css(94.))
                                .px(css(10.))
                                .py_0()
                                .text_size(css(12.))
                                .line_height(css(14.))
                                .text_color(rgb(0xffffff))
                                .border_r_1()
                                .border_color(rgba(0x0000004d))
                                .hover(|s| s.bg(rgba(0xffffff4d)))
                                .active(|s| s.bg(rgba(0x0000004d)).text_color(rgba(0xffffff4d)))
                                .when(self.recording_controls_active(), |v| v.border_r_0())
                                .when(!self.recording_controls_active(), |v| {
                                    v.child(
                                        img("synapse/macro/record.svg").size(css(12.)).mr(css(4.)),
                                    )
                                })
                                .child(self.record_label())
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.toggle_recording(window, cx)
                                })),
                        )
                        .when(!self.recording_controls_active(), |v| {
                            v.child(
                                BaseButton::new("macro-record-options")
                                    .group("macro-record-options")
                                    .on_prepaint(move |bounds, _, _| record_trigger.set(bounds))
                                    .disabled(
                                        self.recording_busy()
                                            || self.current.is_none()
                                            || self.tutorial != Tutorial::Complete,
                                    )
                                    .w(css(27.))
                                    .h_full()
                                    .p_0()
                                    .rounded_r(css(3.))
                                    .hover(|s| s.bg(rgba(0xffffff4d)))
                                    .active(|s| s.bg(rgba(0x0000004d)))
                                    .when(self.record_ui.open, |v| {
                                        v.bg(rgba(0x0000004d)).hover(|s| s.bg(rgba(0x0000004d)))
                                    })
                                    .child(
                                        div()
                                            .relative()
                                            .size(css(10.))
                                            .child(
                                                img("synapse/macro/record-expand.svg").size_full(),
                                            )
                                            .when(!self.record_ui.open, |v| {
                                                v.child(
                                                    img("synapse/macro/record-expand-hover.svg")
                                                        .absolute()
                                                        .inset_0()
                                                        .size_full()
                                                        .opacity(0.)
                                                        .group_hover("macro-record-options", |s| {
                                                            s.opacity(1.)
                                                        }),
                                                )
                                            })
                                            .child(
                                                img("synapse/macro/record-expand.svg")
                                                    .absolute()
                                                    .inset_0()
                                                    .size_full()
                                                    .opacity(0.)
                                                    .group_active("macro-record-options", |s| {
                                                        s.opacity(1.)
                                                    }),
                                            ),
                                    )
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.toggle_record_options(window, cx)
                                    })),
                            )
                        })
                        .when_some(self.record_countdown_label(), |v, label| {
                            v.child(
                                div()
                                    .id("macro-record-countdown")
                                    .test_support()
                                    .absolute()
                                    .top(css(58.))
                                    .w(css(123.))
                                    .h(css(40.))
                                    .px(css(16.))
                                    .py(css(12.))
                                    .rounded(css(5.))
                                    .bg(rgb(0x44d62c))
                                    .text_color(rgb(0x212121))
                                    .text_size(css(12.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_center()
                                    .aria_label(label.clone())
                                    .child(label),
                            )
                        })
                        .when(self.record_ui.open, |v| {
                            v.child(deferred(self.record_options(window, cx)).with_priority(99))
                        }),
                ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .justify_end()
                    .child(
                        BaseButton::new("macro-undo")
                            .group("undo")
                            .disabled(!self.can_undo())
                            .size(css(20.))
                            .p_0()
                            .mr(css(16.))
                            .active(|s| s.opacity(0.7))
                            .child(selection::history_icon("undo", self.can_undo()))
                            .on_click(cx.listener(|this, _, _, cx| this.undo_action(cx))),
                    )
                    .child(
                        BaseButton::new("macro-redo")
                            .group("redo")
                            .disabled(!self.can_redo())
                            .size(css(20.))
                            .p_0()
                            .flex_1()
                            .justify_start()
                            .active(|s| s.opacity(0.7))
                            .child(selection::history_icon("redo", self.can_redo()))
                            .on_click(cx.listener(|this, _, _, cx| this.redo_action(cx))),
                    )
                    .child(
                        BaseButton::new("macro-save")
                            .disabled(!self.can_save_with_pending(cx))
                            .opacity(if self.can_save_with_pending(cx) {
                                1.
                            } else {
                                0.3
                            })
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
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.finish_pending_edits(window, cx);
                                this.save_actions(cx)
                            })),
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
                            this.publish_library(cx);
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
                            this.publish_library(cx);
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
