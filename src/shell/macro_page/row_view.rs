//! Shared current 58190.va row used by both ja and Ka list roots.
use super::*;
use std::rc::Rc;

impl MacroPage {
    pub(super) fn action_row(
        &self,
        index: usize,
        baseline: Rc<Vec<ActionItem>>,
        disabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let page = cx.entity_id();
        let action = &baseline[index];

        let selected = self.selected_actions.contains(&index);
        let paired = self.action_pair_highlighted(index);
        let hover = window.use_keyed_state(("macro-action-hover", index), cx, |_, _| false);
        let hovered = *hover.read(cx);
        let checkbox_alpha = motion::transition(
            (
                ElementId::from(("macro-action-row", index)),
                "checkbox-opacity",
            ),
            if selected || hovered { 1. } else { 0. },
            Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
            window,
            cx,
        );
        let kind = action.kind;
        let dragging = self.action_drag(index, None, page, baseline.clone());
        let drop_state = window.use_keyed_state(("macro-action-drop", index), cx, |_, _| {
            std::rc::Rc::new(std::cell::Cell::new(None::<bool>))
        });
        let border_state = drop_state.read(cx).clone();
        let pointer_state = border_state.clone();
        let content = h_flex()
            .w(relative(0.7))
            .min_w_0()
            .items_center()
            .child(
                selection::checkbox(
                    ("macro-row-checkbox", index),
                    selected,
                    disabled,
                    window,
                    cx,
                )
                .opacity(checkbox_alpha * if disabled { 0.3 } else { 1. })
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.toggle_action_selection(index, cx);
                })),
            )
            .child(
                img(SharedString::from(format!(
                    "synapse/macro/{}.svg",
                    if action.mouse_movement.is_some() {
                        "mouse-movement"
                    } else {
                        kind.icon()
                    }
                )))
                .size(css(20.))
                .flex_shrink_0()
                .mr(css(10.)),
            )
            .when_some(
                row_actions::event_state(action)
                    .filter(|_| matches!(kind, ActionKind::Keyboard | ActionKind::Mouse)),
                |row, state| {
                    row.child(
                        img(if state % 2 == 0 {
                            "synapse/macro/key-down.svg"
                        } else {
                            "synapse/macro/key-up.svg"
                        })
                        .size(css(20.))
                        .flex_shrink_0()
                        .mr(css(10.)),
                    )
                },
            )
            .when(
                matches!(
                    kind,
                    ActionKind::Text | ActionKind::Command | ActionKind::Macro | ActionKind::Loop
                ),
                |row| {
                    row.child(
                        div()
                            .mr(css(10.))
                            .text_size(css(14.))
                            .when(kind == ActionKind::Text, |v| v.min_w(css(42.)))
                            .child(if kind == ActionKind::Loop {
                                format!("{}:", tr(editors::loop_state_key(&action.state)))
                            } else {
                                tr(match kind {
                                    ActionKind::Text => "TEXT_TEXT_FUNCTION_TEXT",
                                    ActionKind::Command => "TEXT_RUN_COMMAND_TEXT",
                                    _ => "TEXT_MACRO_TEXT",
                                })
                            }),
                    )
                },
            )
            .child(
                div()
                    .id(("macro-action-input", index))
                    .min_w_0()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(self.action_value_editor(index, kind, window, cx)),
            );
        h_flex()
            .id((
                if self.recording_busy() && index >= self.actions.len() {
                    "macro-recording-row"
                } else {
                    "macro-action-row"
                },
                if self.recording_busy() && index >= self.actions.len() {
                    self.recording.preview_offset + index - self.actions.len()
                } else {
                    index
                },
            ))
            .test_support()
            .aria_label(format!(
                "{} {} {}",
                kind.icon(),
                action.value,
                action
                    .keyboard
                    .as_ref()
                    .and_then(|k| k.state)
                    .or_else(|| action.mouse.as_ref().and_then(|m| m.state))
                    .map(|s| if s % 2 == 0 { "down" } else { "up" })
                    .unwrap_or("")
            ))
            .relative()
            .w_full()
            .h(css(42.))
            .flex_shrink_0()
            .items_center()
            .px(css(10.))
            .bg(if selected || paired {
                rgba(0x44d62c33)
            } else {
                rgb(0x111111)
            })
            .border_1()
            .border_color(rgba(0))
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, cx| {
                        let over = cx.has_active_drag().then(|| border_state.get()).flatten();
                        let color = match over {
                            Some(true) => 0x44d62c,
                            Some(false) => 0xc8323c,
                            None if hovered => 0x44d62c,
                            None => 0x222222,
                        };
                        let unit = window.rem_size() / 16.;
                        window.paint_quad(fill(
                            Bounds::new(
                                point(bounds.left() - unit, bounds.bottom()),
                                size(bounds.size.width + unit * 2., unit),
                            ),
                            rgb(color),
                        ));
                    },
                )
                .absolute()
                .inset_0(),
            )
            .hover(|s| s.border_color(rgb(0x44d62c)))
            .on_hover(move |value, _, cx| {
                hover.update(cx, |hover, cx| {
                    *hover = *value;
                    cx.notify();
                })
            })
            .when(!disabled, |row| {
                row.on_drag(dragging, |drag, offset, _, cx| {
                    cx.new(|_| ActionDragPreview {
                        drag: drag.clone(),
                        offset,
                    })
                })
            })
            .on_drag_move::<ActionDrag>(move |event, window, cx| {
                let drag = event.drag(cx);
                let value = (drag.page == page && event.bounds.contains(&event.event.position))
                    .then(|| drag.allows(index + 1));
                if let Some(allowed) = value {
                    drag.allowed.set(allowed);
                }
                if pointer_state.replace(value) != value {
                    window.refresh();
                }
            })
            .on_drop(cx.listener(move |this, drag: &ActionDrag, _, cx| {
                this.drop_actions(drag, index + 1, cx)
            }))
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_action_selection(index, cx)))
            .child(content)
            .child(self.row_controls(index, hovered, disabled, window, cx))
            .into_any_element()
    }
}
