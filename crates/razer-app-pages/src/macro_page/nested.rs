//! Current 58190 En/je, 38162 and 25572.C (NH): nested Macro choice.
use super::*;

impl MacroPage {
    /// Source En follows the live macroList graph, including inactive drafts.
    /// Recompute after every edit; a visited set also terminates malformed or
    /// cyclic local input instead of reproducing the source's unbounded walk.
    fn nested_reaches_current(&self, candidate: u64) -> bool {
        let mut pending = vec![candidate];
        let mut seen = std::collections::HashSet::new();
        while let Some(id) = pending.pop() {
            if Some(id) == self.current {
                return true;
            }
            if !seen.insert(id) {
                continue;
            }
            let actions = if self.actions_for == Some(id) {
                &self.actions
            } else if let Some(draft) = self.inactive_drafts.get(&id) {
                &draft.actions
            } else if let Some(entry) = self.entries.iter().find(|entry| entry.id == id) {
                &entry.actions
            } else {
                continue;
            };
            pending.extend(actions.iter().filter_map(|action| {
                (action.kind == ActionKind::Macro)
                    .then_some(action.macro_id)
                    .flatten()
            }));
        }
        false
    }

    fn choose_nested_macro(&mut self, index: usize, id: u64, cx: &mut Context<Self>) {
        let Some(entry) = self.entries.iter().find(|entry| {
            entry.id == id && entry.kind == EntryKind::Macro && Some(id) != self.current
        }) else {
            return;
        };
        let name = entry.name.clone();
        if self.nested_reaches_current(id) || self.actions_for != self.current {
            return;
        }
        let Some(action) = self
            .actions
            .get(index)
            .filter(|action| action.kind == ActionKind::Macro)
        else {
            return;
        };
        if action.macro_id != Some(id) {
            self.undo.push(self.actions.clone());
            self.actions[index].macro_id = Some(id);
            self.actions[index].xml_macro_guid = None;
            self.actions[index].value = name;
            self.redo.clear();
            // je restores the text only when its selected index changes.
            self.choice_action = None;
        }
        cx.notify();
    }

    pub fn nested_macro_editor(
        &self,
        index: usize,
        action: &ActionItem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use super::nested_overlay::{NestedDropdownState, NestedMenuOverlay};
        let mut candidates = self
            .entries
            .iter()
            .filter(|entry| entry.kind == EntryKind::Macro && Some(entry.id) != self.current)
            .collect::<Vec<_>>();
        candidates.sort_by_key(|entry| entry.id);
        let selected = candidates
            .iter()
            .find(|entry| Some(entry.id) == action.macro_id);
        let label = selected.map_or_else(|| tr("TEXT_SELECT_A_MACRO"), |entry| entry.name.clone());
        let options = candidates
            .iter()
            .filter(|entry| !entry.name.is_empty())
            .map(|entry| {
                (
                    entry.id,
                    entry.name.clone(),
                    self.nested_reaches_current(entry.id),
                )
            })
            .collect::<Vec<_>>();
        let editing = self.choice_action == Some(index);
        // 38162 allows a one-item list; empty names produce no option DOM node.
        let available = !candidates.is_empty();
        let id = ElementId::from(("nested-macro", index));
        let state = window.use_keyed_state((id.clone(), "dropdown"), cx, |_, _| {
            NestedDropdownState::default()
        });
        if !editing {
            state.update(cx, |state, _| {
                state.expanded = false;
                state.menu.set(Bounds::default());
            });
        }
        let expanded = editing && state.read(cx).expanded && available;
        let scroll = state.read(cx).scroll.clone();
        let trigger_bounds = state.read(cx).trigger.clone();
        let menu_bounds = state.read(cx).menu.clone();
        let upward = state.read(cx).upward.clone();
        let selected_id = action.macro_id;
        let selected_row = options
            .iter()
            .position(|(id, _, disabled)| Some(*id) == selected_id && !disabled);
        let unit = window.rem_size() / 16.;
        let menu_width = options
            .iter()
            .map(|(_, name, _)| razer_widgets::surface::label_width(name, 14., window) + 12.)
            .fold(160., f32::max);
        let final_height = (options.len() as f32 * 25. + 2.).min(180.);
        let policy = || Transition::new(Duration::from_millis(300)).easing(Easing::Ease);
        let angle = motion::transition(
            (id.clone(), "chevron"),
            if expanded { std::f32::consts::PI } else { 0. },
            policy(),
            window,
            cx,
        );
        let pointer = razer_widgets::surface::pointer_state(id.clone(), window, cx);
        let (hovered, _) = pointer.read(cx).sample();
        let border = motion::transition(
            (id.clone(), "border"),
            Hsla::from(rgb(if available && (expanded || hovered) {
                0x44d62c
            } else {
                0x515151
            })),
            policy(),
            window,
            cx,
        );
        let opacity = razer_widgets::surface::fade_opacity(
            id.clone(),
            if available { 1. } else { 0.3 },
            300,
            window,
            cx,
        );
        // height:0 -> auto is discrete; max-height:0 -> 180px and border-bottom
        // interpolate. Rows disappear immediately when expanded becomes false.
        let reveal = motion::transition(
            (id.clone(), "max-height"),
            if expanded { 180f32 } else { 0. },
            Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
            window,
            cx,
        );
        let bottom = motion::transition(
            (id.clone(), "bottom-border"),
            if expanded { 1f32 } else { 0. },
            Transition::new(Duration::from_millis(100)).easing(Easing::Ease),
            window,
            cx,
        );
        let height = if expanded {
            (options.len() as f32 * 25. + 1. + bottom)
                .min(reveal)
                .max(1. + bottom)
        } else {
            bottom
        };
        let toggle = state.clone();
        let trigger = razer_widgets::surface::track_pointer(
            BaseButton::new(("nested-macro-value", index)),
            &pointer,
            window,
        )
        .p_0()
        .text_size(css(14.))
        .line_height(css(17.))
        .text_color(rgb(if selected.is_some() {
            0xcccccc
        } else {
            0x707070
        }))
        .justify_start()
        .when(!editing, |button| {
            button.hover(|style| style.text_color(rgb(0x44d62c)))
        })
        .when(editing, |button| {
            button
                .relative()
                .w(css(160.))
                .h(css(27.))
                .px(css(5.))
                .py(css(4.))
                .bg(rgb(0x111111))
                .border_1()
                .border_color(border)
                .opacity(opacity)
        })
        .child(
            div()
                .when(editing, |label| label.max_w(css(134.)))
                .truncate()
                .child(label),
        )
        .when(editing, |button| {
            button.child(
                div()
                    .absolute()
                    .right_0()
                    .top_0()
                    .w(css(29.))
                    .h(css(25.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::default()
                            .path("synapse/expand.svg")
                            .size(css(10.))
                            .text_color(rgb(0xcccccc))
                            .transform(Transformation::rotate(radians(angle))),
                    ),
            )
        })
        .on_click(cx.listener(move |this, _, window, cx| {
            this.finish_pending_edits(window, cx);
            this.choice_action = Some(index);
            toggle.update(cx, |state, cx| {
                state.expanded = available && (!editing || !state.expanded);
                if state.expanded {
                    state.upward.set(None);
                    state.scroll.set_offset(point(
                        px(0.),
                        -unit * (selected_row.unwrap_or(0) as f32 * 25.),
                    ));
                }
                cx.notify();
            });
            cx.stop_propagation();
            cx.notify();
        }));
        let outside_trigger = trigger_bounds.clone();
        let outside_menu = menu_bounds.clone();
        let close = state.clone();
        let mut root = div()
            .id(("nested-macro-input", index))
            .on_prepaint(move |bounds, _, _| trigger_bounds.set(bounds))
            .on_click(|_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .when(editing, |root| {
                root.on_mouse_down_out(cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    if !outside_trigger.get().contains(&event.position)
                        && !outside_menu.get().contains(&event.position)
                    {
                        this.choice_action = None;
                        close.update(cx, |state, cx| {
                            state.expanded = false;
                            cx.notify();
                        });
                        cx.notify();
                    }
                }))
            })
            .child(trigger);
        if editing && (expanded || bottom > 0.) {
            let measured_menu = menu_bounds.clone();
            let choose = state.clone();
            let content = v_flex()
                .id(("nested-macro-options-scroll", index))
                .w(css(menu_width))
                .h(css(height))
                .max_h(css(180.))
                .bg(rgb(0))
                .border_l(css(if expanded { 1. } else { 0. }))
                .border_r(css(if expanded { 1. } else { 0. }))
                .border_t(css(if expanded { 1. } else { 0. }))
                .border_b(css(bottom))
                .border_color(rgb(0x515151))
                .overflow_x_hidden()
                .overflow_y_scroll()
                .track_scroll(&scroll)
                .occlude()
                .on_prepaint(move |bounds, _, _| measured_menu.set(bounds))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .when(expanded, |view| {
                    view.children(options.into_iter().map(|(id, name, disabled)| {
                        let choose = choose.clone();
                        BaseButton::new(("nested-macro-option", id))
                            .disabled(disabled)
                            .aria_selected(selected_id == Some(id))
                            .w_full()
                            .h(css(25.))
                            .flex_shrink_0()
                            .px(css(5.))
                            .py(css(4.))
                            .text_size(css(14.))
                            .line_height(css(17.))
                            .justify_start()
                            .text_color(if disabled {
                                rgba(0xcccccc4d)
                            } else if selected_id == Some(id) {
                                rgba(0x44d62cff)
                            } else {
                                rgba(0xccccccff)
                            })
                            .hover(|style| style.bg(rgba(0xffffff1a)))
                            .active(|style| style.text_color(rgb(0x44d62c)))
                            .child(div().truncate().child(name))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.choose_nested_macro(index, id, cx);
                                choose.update(cx, |state, cx| {
                                    state.expanded = false;
                                    cx.notify();
                                });
                                cx.stop_propagation();
                            }))
                    }))
                })
                .into_any_element();
            root = root.child(
                deferred(NestedMenuOverlay {
                    content,
                    trigger: state.read(cx).trigger.clone(),
                    viewport: self.source_viewport.clone(),
                    upward,
                    final_height: unit * final_height,
                })
                .with_priority(100),
            );
        } else {
            menu_bounds.set(Bounds::default());
        }
        root.into_any_element()
    }
}
