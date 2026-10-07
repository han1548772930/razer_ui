//! Current 58190.Ka/Fr, 68511.SG and the SU/vI/dt/T9 reducers.
use super::*;
use crate::features::macro_library::MacroType;
use std::rc::Rc;

pub(super) struct PhasedUi {
    mounted: Option<MacroType>,
    pub(super) expanded: [bool; 3],
}
impl Default for PhasedUi {
    fn default() -> Self {
        Self {
            mounted: None,
            expanded: [true; 3],
        }
    }
}

const PHASES: [(&str, &str, &str); 3] = [
    ("ON_PRESS", "ON_PRESS_TOOLTIP", "synapse/macro/key-down.svg"),
    (
        "WHILE_HELD",
        "WHILE_HELD_TOOLTIP",
        "synapse/macro/phase-held.svg",
    ),
    (
        "ON_RELEASE",
        "ON_RELEASE_TOOLTIP",
        "synapse/macro/key-up.svg",
    ),
];

impl MacroPage {
    /// Move a drag payload to a phase boundary and mark the moved rows with
    /// that phase.  The regular row drop handler only reorders rows; phase
    /// drops additionally update the source phase field so the rows remain in
    /// the selected section after the reorder.
    pub(super) fn drop_phase_actions(
        &mut self,
        drag: &ActionDrag,
        phase: u8,
        cx: &mut Context<Self>,
    ) {
        let boundary = self.phase_insertion_boundary(phase);
        if self.recording_busy()
            || self.record_ui.open
            || self.tutorial != Tutorial::Complete
            || drag.page != cx.entity_id()
            || drag.document != self.current
            || self.actions_for != self.current
            || self.actions() != drag.baseline.as_slice()
            || !drag.allows(boundary)
        {
            return;
        }
        let before = self.actions.clone();
        let undo_len = self.undo.len();
        let old_len = self.actions.len();
        let insertion = boundary.min(old_len);
        let moved: Vec<_> = drag
            .indices
            .iter()
            .filter_map(|&index| drag.baseline.get(index).cloned())
            .collect();
        self.drop_actions(drag, boundary, cx);
        let order_changed = self.undo.len() > undo_len;

        // Palette drops create fresh rows at the insertion point. Existing
        // row drops are matched by value; duplicate rows are harmless because
        // all matching moved rows receive the same phase.
        let mut phase_changed = false;
        if drag.palette_kind.is_some() {
            let added = self.actions.len().saturating_sub(old_len);
            for item in self.actions.iter_mut().skip(insertion).take(added) {
                if item.phase != Some(phase) {
                    item.phase = Some(phase);
                    phase_changed = true;
                }
            }
        } else if !moved.is_empty() {
            let mut used = vec![false; self.actions.len()];
            for wanted in moved {
                if let Some((index, item)) = self
                    .actions
                    .iter_mut()
                    .enumerate()
                    .find(|(index, item)| !used[*index] && **item == wanted)
                {
                    if item.phase != Some(phase) {
                        item.phase = Some(phase);
                        phase_changed = true;
                    }
                    used[index] = true;
                }
            }
        }
        if phase_changed {
            // drop_actions already recorded the pre-move snapshot when the
            // order changed. For a phase-only move, create that snapshot here
            // so undo restores both the rows and their phase assignment.
            if !order_changed {
                self.undo.push(before);
            }
            self.clear_action_editors();
            self.redo.clear();
            cx.notify();
        }
    }

    pub(super) fn active_phase(&self) -> Option<u8> {
        self.entries
            .iter()
            .find(|entry| Some(entry.id) == self.current)?
            .active_phase
    }

    pub(super) fn sync_phased_mount(&mut self, cx: &mut Context<Self>) {
        if self.tab != MacroTab::MyMacros {
            // Ka is only mounted on the My Macros tab. Its effects and local
            // expanded state restart after a tab switch, but remain intact
            // when the selected document changes while Ka stays mounted.
            self.phased_ui.mounted = None;
            return;
        }
        let kind = self.current_macro_type();
        // Fr's effect depends on macroType, not the current document identity.
        if self.phased_ui.mounted != Some(kind) {
            self.phased_ui.mounted = Some(kind);
            self.phased_ui.expanded = [true; 3];
            self.set_active_phase((kind == MacroType::Phased).then_some(0), cx);
        }
    }

    fn set_active_phase(&mut self, phase: Option<u8>, cx: &mut Context<Self>) {
        if self.recording_busy() {
            return;
        }
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| Some(entry.id) == self.current)
            && entry.active_phase != phase
        {
            entry.active_phase = phase;
            self.publish_library(cx);
            cx.notify();
        }
    }

    pub(super) fn phase_insertion_boundary(&self, phase: u8) -> usize {
        self.actions()
            .iter()
            .rposition(|item| item.phase.is_some_and(|p| p < phase))
            .map_or(0, |index| index + 1)
    }

    /// T9 assigns an ordinary row drop the phase of the row immediately
    /// before it, falling back to the next defined phase. The assignment is
    /// performed in moved-row order, then Gs applies the stable phase sort.
    pub(super) fn dragged_phase_assignments(
        &self,
        drag: &ActionDrag,
        target: usize,
        order: &[usize],
    ) -> Vec<(usize, u8)> {
        if self.current_macro_type() != MacroType::Phased || drag.palette_kind.is_some() {
            return Vec::new();
        }
        // T9 treats dropping into the selected group as a no-op.
        if target > 0 && drag.indices.contains(&(target - 1)) {
            return Vec::new();
        }
        let mut planned: Vec<_> = self.actions.iter().map(|item| item.phase).collect();
        let mut assignments = Vec::new();
        for &old_index in &drag.indices {
            let Some(index) = order.iter().position(|&old| old == old_index) else {
                continue;
            };
            let phase = order[..index]
                .iter()
                .rev()
                .find_map(|&old| planned[old])
                .or_else(|| order[index + 1..].iter().find_map(|&old| planned[old]));
            if let Some(phase) = phase.filter(|phase| *phase <= 2) {
                planned[old_index] = Some(phase);
                assignments.push((old_index, phase));
            }
        }
        assignments
    }

    pub(super) fn phase_row_offset(&self, index: usize) -> f32 {
        if self.current_macro_type() != MacroType::Phased {
            return index as f32 * 42.;
        }
        let Some(phase) = self.actions().get(index).and_then(|item| item.phase) else {
            return 0.;
        };
        let mut offset = 0.;
        for p in 0..=phase {
            offset += 51.; // section's 1px top border + 50px header
            if self.phased_ui.expanded[p as usize] {
                offset += self
                    .actions()
                    .iter()
                    .enumerate()
                    .filter(|(i, item)| item.phase == Some(p) && (p < phase || *i < index))
                    .count() as f32
                    * 42.;
            }
        }
        offset
    }

    pub(super) fn normalize_phased_rows(&mut self) {
        if self.current_macro_type() != MacroType::Phased {
            return;
        }
        // xs sorts valid phase rows stably. Preserve unknown legacy slots;
        // do not invent a phase for an old row or infer it from its position.
        let mut order: Vec<_> = (0..self.actions.len()).collect();
        let slots: Vec<_> = order
            .iter()
            .copied()
            .filter(|&i| self.actions[i].phase.is_some())
            .collect();
        let mut sorted = slots.clone();
        sorted.sort_by_key(|&i| self.actions[i].phase);
        for (slot, old) in slots.into_iter().zip(sorted) {
            order[slot] = old;
        }
        let next = order.iter().map(|&i| self.actions[i].clone()).collect();
        self.selected_actions = order
            .iter()
            .enumerate()
            .filter_map(|(next, old)| self.selected_actions.contains(old).then_some(next))
            .collect();
        self.actions = next;
    }

    fn delete_phase(&mut self, phase: u8, window: &mut Window, cx: &mut Context<Self>) {
        if self.recording_busy()
            || self.record_ui.open
            || self.tutorial != Tutorial::Complete
            || self.all_actions_selected()
            || !self.actions().iter().any(|item| item.phase == Some(phase))
        {
            return;
        }
        self.finish_pending_edits(window, cx);
        self.undo.push(self.actions.clone());
        self.actions.retain(|item| item.phase != Some(phase));
        self.selected_actions.clear();
        self.redo.clear();
        self.clear_action_editors();
        cx.notify();
    }

    pub(super) fn phased_editor(
        &self,
        height: f32,
        disabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let bounds = self.text_ui.editor_bounds.clone();
        let baseline = Rc::new(self.actions().to_vec());
        let trailing_len = baseline.len();
        let all = self.all_actions_selected();
        let page = cx.entity_id();
        v_flex()
            .id("macro-phased-item-list")
            .test_support()
            .w_full()
            .h(css(height))
            .min_h(css(420.))
            .bg(rgb(0x111111))
            .text_color(rgb(0xcccccc))
            .on_prepaint(move |rect, _, _| bounds.set(rect))
            .when(disabled, |v| {
                v.opacity(0.7)
                    .capture_any_mouse_down(|_, _, cx| cx.stop_propagation())
            })
            .scrollable_y()
            .track_scroll(&self.recording.scroll)
            .children(
                PHASES
                    .into_iter()
                    .enumerate()
                    .map(|(phase, (label, help, icon))| {
                        let has_rows = self
                            .actions()
                            .iter()
                            .any(|item| item.phase == Some(phase as u8));
                        let unavailable = disabled || all || !has_rows;
                        let expanded = self.phased_ui.expanded[phase];
                        let group: SharedString = format!("macro-phase-header-{phase}").into();
                        let hover =
                            window.use_keyed_state(("macro-phase-hover", phase), cx, |_, _| false);
                        let hovered = *hover.read(cx);
                        let chevron_hover = window.use_keyed_state(
                            ("macro-phase-chevron-hover", phase),
                            cx,
                            |_, _| false,
                        );
                        let chevron_hovered = *chevron_hover.read(cx);
                        let angle = motion::transition(
                            (ElementId::from(("macro-phase-chevron", phase)), "angle"),
                            if expanded { 360. } else { 270. },
                            Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
                            window,
                            cx,
                        );
                        let boundary = self.phase_insertion_boundary(phase as u8);
                        let header = h_flex()
                            .id(("macro-phase-header", phase))
                            .group(group.clone())
                            .w_full()
                            .h(css(50.))
                            .flex_shrink_0()
                            .pl(css(12.))
                            .pr(css(10.))
                            .justify_between()
                            .items_center()
                            .bg(rgb(0x111111))
                            .border_b_1()
                            .border_color(rgb(0x1a1a1a))
                            .font_family("Roboto")
                            .text_size(css(14.))
                            .line_height(css(17.))
                            .on_hover(move |value, _, cx| {
                                hover.update(cx, |state, cx| {
                                    *state = *value;
                                    cx.notify();
                                })
                            })
                            .drag_over::<ActionDrag>(move |style, drag, _, _| {
                                row_drag::target_style(style, drag, page, boundary)
                            })
                            .on_drop(cx.listener(move |this, drag: &ActionDrag, _, cx| {
                                this.drop_phase_actions(drag, phase as u8, cx);
                            }))
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap(css(10.))
                                    .child(self.phase_radio(
                                        phase,
                                        label,
                                        icon,
                                        disabled || all,
                                        window,
                                        cx,
                                    ))
                                    .child(phase_help(phase, help, window, cx)),
                            )
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap(css(10.))
                                    .child(
                                        phase_button(
                                            ("macro-phase-record", phase),
                                            "record",
                                            "TEXT_RECORD_COMMAND_DELAY",
                                            disabled,
                                            window,
                                            cx,
                                        )
                                        .opacity(if hovered { 1. } else { 0. })
                                        .hover(|s| s.bg(rgba(0xffffff1a)).rounded(css(5.)))
                                        .on_click(
                                            cx.listener(move |this, _, window, cx| {
                                                this.set_active_phase(Some(phase as u8), cx);
                                                window.push_notification(
                                                    "宏录制服务尚未连接；已选择阶段，未开始录制。",
                                                    cx,
                                                );
                                            }),
                                        ),
                                    )
                                    .child(
                                        phase_button(
                                            ("macro-phase-delete", phase),
                                            "delete",
                                            "TEXT_DELETE_ITEM_CATEGORY",
                                            unavailable,
                                            window,
                                            cx,
                                        )
                                        .opacity(if hovered {
                                            if unavailable { 0.5 } else { 1. }
                                        } else {
                                            0.
                                        })
                                        .on_click(
                                            cx.listener(move |this, _, window, cx| {
                                                this.delete_phase(phase as u8, window, cx)
                                            }),
                                        ),
                                    )
                                    .child(
                                        BaseButton::new(("macro-phase-collapse", phase))
                                            .p_0()
                                            .size(css(24.))
                                            .disabled(unavailable)
                                            .opacity(if unavailable { 0.5 } else { 1. })
                                            .active(|s| s.opacity(0.7))
                                            .child(
                                                div().size(css(24.)).child(
                                                    Icon::default()
                                                        .path(if chevron_hovered {
                                                            "synapse/macro/phase-chevron-hover.svg"
                                                        } else {
                                                            "synapse/macro/phase-chevron.svg"
                                                        })
                                                        .size(css(24.))
                                                        .transform(Transformation::rotate(
                                                            radians(angle.to_radians()),
                                                        )),
                                                ),
                                            )
                                            .on_hover(move |value, _, cx| {
                                                chevron_hover.update(cx, |state, cx| {
                                                    *state = *value;
                                                    cx.notify();
                                                })
                                            })
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.finish_pending_edits(window, cx);
                                                this.clear_action_editors();
                                                this.phased_ui.expanded[phase] =
                                                    !this.phased_ui.expanded[phase];
                                                cx.notify();
                                            })),
                                    ),
                            );
                        v_flex()
                            .id(("macro-phase-section", phase))
                            .w_full()
                            .flex_shrink_0()
                            .bg(rgb(0x1a1a1a))
                            .border_t_1()
                            .border_color(rgb(0x5d5d5d))
                            .child(header)
                            .child(
                                v_flex()
                                    .w_full()
                                    .children(
                                        self.actions()
                                            .iter()
                                            .enumerate()
                                            .filter(|(_, item)| {
                                                expanded && item.phase == Some(phase as u8)
                                            })
                                            .map(|(index, _)| {
                                                self.action_row(
                                                    index,
                                                    baseline.clone(),
                                                    disabled,
                                                    window,
                                                    cx,
                                                )
                                            }),
                                    )
                                    // Ka's phase drop_space div has no height in current CSS.
                                    .child(
                                        div()
                                            .id(("macro-phase-drop-space", phase))
                                            .w_full()
                                            .h(css(0.)),
                                    ),
                            )
                    }),
            )
            .child(
                div()
                    .id("macro-phased-trailing-drop-space")
                    .w_full()
                    .h(css(100.))
                    .flex_shrink_0()
                    .bg(rgb(0x111111))
                    .on_drag_move::<ActionDrag>(move |event, _, cx| {
                        let drag = event.drag(cx);
                        if drag.page == page && event.bounds.contains(&event.event.position) {
                            drag.allowed.set(drag.allows(trailing_len));
                        }
                    })
                    .on_drop(cx.listener(move |this, drag: &ActionDrag, _, cx| {
                        this.drop_actions(drag, trailing_len, cx)
                    })),
            )
            .into_any_element()
    }

    fn phase_radio(
        &self,
        phase: usize,
        label: &str,
        icon: &'static str,
        disabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = self.active_phase() == Some(phase as u8);
        let alpha = Presence::new(
            (ElementId::from(("macro-phase-radio", phase)), "selected"),
            selected,
        )
        .transition(Transition::new(Duration::from_millis(200)).easing(Easing::Ease))
        .sample(window, cx)
        .progress;
        BaseButton::new(("macro-phase-radio", phase))
            .role(Role::RadioButton)
            .selected(selected)
            .p_0()
            .disabled(disabled)
            .justify_start()
            .text_size(css(14.))
            .line_height(css(20.))
            .child(
                div()
                    .relative()
                    .size(css(20.))
                    .flex_shrink_0()
                    .mr(css(10.))
                    .rounded_full()
                    .border_1()
                    .border_color(rgb(0x737373))
                    .opacity(if disabled { 0.3 } else { 1. })
                    .child(
                        div()
                            .absolute()
                            .left(css(4. + 5. * (1. - alpha)))
                            .top(css(4. + 5. * (1. - alpha)))
                            .size(css(10. * alpha))
                            .rounded_full()
                            .bg(rgb(0x44d62c))
                            .opacity(alpha),
                    ),
            )
            .child(img(icon).size(css(24.)).ml(css(7.)))
            .child(tr(label))
            .on_click(
                cx.listener(move |this, _, _, cx| this.set_active_phase(Some(phase as u8), cx)),
            )
            .into_any_element()
    }
}

fn phase_button(
    id: (&'static str, usize),
    icon: &'static str,
    tip: &'static str,
    disabled: bool,
    window: &mut Window,
    cx: &mut App,
) -> BaseButton {
    let hover = window.use_keyed_state((ElementId::from(id), "hover"), cx, |_, _| false);
    let hovered = *hover.read(cx) && !disabled;
    let opacity = motion::transition(
        (ElementId::from(id), "tooltip"),
        if hovered { 1. } else { 0. },
        Transition::new(Duration::from_millis(300)).easing(Easing::Linear),
        window,
        cx,
    );
    BaseButton::new(id)
        .relative()
        .p_0()
        .size(css(24.))
        .disabled(disabled)
        .child(
            div()
                .size(css(24.))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    img(SharedString::from(format!(
                        "synapse/macro/{icon}{}.svg",
                        if icon == "delete" && hovered {
                            "-hover"
                        } else {
                            ""
                        }
                    )))
                    .size(css(20.)),
                ),
        )
        .on_hover(move |value, _, cx| {
            hover.update(cx, |state, cx| {
                *state = *value;
                cx.notify();
            })
        })
        .when(hovered, |button| {
            button.child(
                deferred(
                    div()
                        .absolute()
                        .right(css(-45.))
                        .top(css(30.))
                        .w_auto()
                        .whitespace_nowrap()
                        .px(css(10.))
                        .py(css(8.))
                        .bg(rgb(0))
                        .border_1()
                        .border_color(rgb(0x5d5d5d))
                        .text_color(rgb(0xcccccc))
                        .text_size(css(14.))
                        .line_height(css(16.))
                        .opacity(opacity)
                        .child(tr(tip)),
                )
                .with_priority(999),
            )
        })
}

fn phase_help(phase: usize, key: &'static str, window: &mut Window, cx: &mut App) -> AnyElement {
    let hover = window.use_keyed_state(("macro-phase-help", phase), cx, |_, _| false);
    let hovered = *hover.read(cx);
    let alpha = motion::transition(
        (ElementId::from(("macro-phase-help", phase)), "opacity"),
        if hovered { 1. } else { 0. },
        Transition::new(Duration::from_millis(300)).easing(Easing::Linear),
        window,
        cx,
    );
    div()
        .id(("macro-phase-help", phase))
        .relative()
        .size(css(14.))
        .rounded_full()
        .bg(rgb(0x6c6c6c))
        .child(img("synapse/gr-help.svg").size_full())
        .on_hover(move |value, _, cx| {
            hover.update(cx, |state, cx| {
                *state = *value;
                cx.notify();
            })
        })
        .when(hovered, |v| {
            v.child(
                deferred(
                    div()
                        .absolute()
                        .left_0()
                        .top(css(19.))
                        .w_auto()
                        .whitespace_normal()
                        .bg(rgb(0))
                        .border_1()
                        .border_color(rgb(0x5d5d5d))
                        .px(css(10.))
                        .py(css(8.))
                        .text_size(css(14.))
                        .line_height(css(16.))
                        .text_color(rgb(0xcccccc))
                        .opacity(alpha)
                        .child(tr(key)),
                )
                .with_priority(100),
            )
        })
        .into_any_element()
}
