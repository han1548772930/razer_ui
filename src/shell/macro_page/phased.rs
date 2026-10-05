//! Current 58190.Ka/Fr, 68511.SG and the SU/vI/dt/T9 reducers.
use super::*;
use crate::features::macro_library::MacroType;
use std::rc::Rc;

pub(super) struct PhasedUi {
    mounted: Option<MacroType>,
    pub(super) expanded: [bool; 3],
}
impl Default for PhasedUi {
    fn default() -> Self { Self { mounted: None, expanded: [true; 3] } }
}

const PHASES: [(&str, &str, &str); 3] = [
    ("ON_PRESS", "ON_PRESS_TOOLTIP", "synapse/macro/key-down.svg"),
    ("WHILE_HELD", "WHILE_HELD_TOOLTIP", "synapse/macro/phase-held.svg"),
    ("ON_RELEASE", "ON_RELEASE_TOOLTIP", "synapse/macro/key-up.svg"),
];

impl MacroPage {
    pub(super) fn active_phase(&self) -> Option<u8> {
        self.entries.iter().find(|entry| Some(entry.id) == self.current)?.active_phase
    }

    pub(super) fn sync_phased_mount(&mut self, cx: &mut Context<Self>) {
        let kind = self.current_macro_type();
        // Fr's effect depends on macroType, not the current document identity.
        if self.phased_ui.mounted != Some(kind) {
            self.phased_ui.mounted = Some(kind);
            self.phased_ui.expanded = [true; 3];
            self.set_active_phase((kind == MacroType::Phased).then_some(0), cx);
        }
    }

    fn set_active_phase(&mut self, phase: Option<u8>, cx: &mut Context<Self>) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| Some(entry.id) == self.current)
            && entry.active_phase != phase
        {
            entry.active_phase = phase;
            self.publish_library(cx);
            cx.notify();
        }
    }

    pub(super) fn phase_insertion_boundary(&self, phase: u8) -> usize {
        self.actions().iter().rposition(|item| item.phase.is_some_and(|p| p < phase))
            .map_or(0, |index| index + 1)
    }

    pub(super) fn phase_row_offset(&self, index: usize) -> f32 {
        if self.current_macro_type() != MacroType::Phased { return index as f32 * 42.; }
        let Some(phase) = self.actions().get(index).and_then(|item| item.phase) else { return 0.; };
        let mut offset = 0.;
        for p in 0..=phase {
            offset += 51.; // section's 1px top border + 50px header
            if self.phased_ui.expanded[p as usize] {
                offset += self.actions().iter().enumerate().filter(|(i, item)|
                    item.phase == Some(p) && (p < phase || *i < index)).count() as f32 * 42.;
            }
        }
        offset
    }

    pub(super) fn normalize_phased_rows(&mut self) {
        if self.current_macro_type() != MacroType::Phased { return; }
        // xs sorts valid phase rows stably. Preserve unknown legacy slots;
        // do not invent a phase for an old row or infer it from its position.
        let mut order: Vec<_> = (0..self.actions.len()).collect();
        let slots: Vec<_> = order.iter().copied().filter(|&i| self.actions[i].phase.is_some()).collect();
        let mut sorted = slots.clone();
        sorted.sort_by_key(|&i| self.actions[i].phase);
        for (slot, old) in slots.into_iter().zip(sorted) { order[slot] = old; }
        let next = order.iter().map(|&i| self.actions[i].clone()).collect();
        self.selected_actions = order.iter().enumerate()
            .filter_map(|(next, old)| self.selected_actions.contains(old).then_some(next)).collect();
        self.actions = next;
    }

    fn delete_phase(&mut self, phase: u8, window: &mut Window, cx: &mut Context<Self>) {
        if self.record_ui.open || self.tutorial != Tutorial::Complete || self.all_actions_selected()
            || !self.actions().iter().any(|item| item.phase == Some(phase)) { return; }
        self.finish_pending_edits(window, cx);
        self.undo.push(self.actions.clone());
        self.actions.retain(|item| item.phase != Some(phase));
        self.selected_actions.clear();
        self.redo.clear();
        self.clear_action_editors();
        cx.notify();
    }

    pub(super) fn phased_editor(&self, height: f32, disabled: bool,
        window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let bounds = self.text_ui.editor_bounds.clone();
        let baseline = Rc::new(self.actions().to_vec());
        let all = self.all_actions_selected();
        let page = cx.entity_id();
        v_flex().id("macro-phased-item-list").w_full().h(css(height)).min_h(css(420.))
            .bg(rgb(0x111111)).text_color(rgb(0xcccccc))
            .on_prepaint(move |rect, _, _| bounds.set(rect))
            .when(disabled, |v| v.opacity(0.7).capture_any_mouse_down(|_, _, cx| cx.stop_propagation()))
            .scrollable_y()
            .children(PHASES.into_iter().enumerate().map(|(phase, (label, help, icon))| {
                let has_rows = self.actions().iter().any(|item| item.phase == Some(phase as u8));
                let unavailable = disabled || all || !has_rows;
                let expanded = self.phased_ui.expanded[phase];
                let group: SharedString = format!("macro-phase-header-{phase}").into();
                let hover = window.use_keyed_state(("macro-phase-hover", phase), cx, |_, _| false);
                let hovered = *hover.read(cx);
                let angle = motion::transition((ElementId::from(("macro-phase-chevron", phase)), "angle"),
                    if expanded { 360. } else { 270. },
                    Transition::new(Duration::from_millis(300)).easing(Easing::Ease), window, cx);
                let boundary = self.phase_insertion_boundary(phase as u8);
                let header = h_flex().id(("macro-phase-header", phase)).group(group.clone())
                    .w_full().h(css(50.)).flex_shrink_0().pl(css(12.)).pr(css(10.))
                    .justify_between().items_center().bg(rgb(0x111111))
                    .border_b_1().border_color(rgb(0x1a1a1a))
                    .font_family("Roboto").text_size(css(14.)).line_height(css(17.))
                    .on_hover(move |value, _, cx| hover.update(cx, |state, cx| { *state = *value; cx.notify(); }))
                    .drag_over::<ActionDrag>(move |style, drag, _, _| row_drag::target_style(style, drag, page, boundary))
                    .on_drop(cx.listener(move |this, drag: &ActionDrag, _, cx| {
                        this.drop_phase_actions(drag, phase as u8, cx);
                    }))
                    .child(h_flex().items_center().gap(css(10.))
                        .child(self.phase_radio(phase, label, icon, disabled || all, window, cx))
                        .child(phase_help(phase, help, window, cx)))
                    .child(h_flex().items_center().gap(css(10.))
                        .child(phase_button(("macro-phase-record", phase), "record", "TEXT_RECORD_COMMAND_DELAY",
                            disabled, window, cx)
                            .opacity(if hovered { 1. } else { 0. })
                            .hover(|s| s.bg(rgba(0xffffff1a)).rounded(css(5.)))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.set_active_phase(Some(phase as u8), cx);
                                window.push_notification("宏录制服务尚未连接；已选择阶段，未开始录制。", cx);
                            })))
                        .child(phase_button(("macro-phase-delete", phase), "delete", "TEXT_DELETE_ITEM_CATEGORY",
                            unavailable, window, cx)
                            .opacity(if hovered { if unavailable { 0.5 } else { 1. } } else { 0. })
                            .on_click(cx.listener(move |this, _, window, cx| this.delete_phase(phase as u8, window, cx))))
                        .child(BaseButton::new(("macro-phase-collapse", phase)).p_0().size(css(24.))
                            .disabled(unavailable).opacity(if unavailable { 0.5 } else { 1. })
                            .active(|s| s.opacity(0.7))
                            .child(div().size(css(24.)).transform(Transformation::rotate(radians(angle.to_radians())))
                                .child(img(if hovered { "synapse/macro/phase-chevron-hover.svg" } else { "synapse/macro/phase-chevron.svg" }).size(css(24.))))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.finish_pending_edits(window, cx);
                                this.clear_action_editors();
                                this.phased_ui.expanded[phase] = !this.phased_ui.expanded[phase];
                                cx.notify();
                            }))));
                v_flex().id(("macro-phase-section", phase)).w_full().flex_shrink_0()
                    .bg(rgb(0x1a1a1a)).border_t_1().border_color(rgb(0x5d5d5d))
                    .child(header)
                    .child(v_flex().w_full().children(self.actions().iter().enumerate()
                        .filter(|(_, item)| expanded && item.phase == Some(phase as u8))
                        .map(|(index, _)| self.action_row(index, baseline.clone(), disabled, window, cx)))
                        // Ka's phase drop_space div has no height in current CSS.
                        .child(div().id(("macro-phase-drop-space", phase)).w_full().h(css(0.))))
            })).into_any_element()
    }

    fn phase_radio(&self, phase: usize, label: &str, icon: &'static str, disabled: bool,
        window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.active_phase() == Some(phase as u8);
        let alpha = Presence::new((ElementId::from(("macro-phase-radio", phase)), "selected"), selected)
            .transition(Transition::new(Duration::from_millis(200)).easing(Easing::Ease))
            .sample(window, cx).progress;
        BaseButton::new(("macro-phase-radio", phase)).role(Role::RadioButton).selected(selected)
            .p_0().disabled(disabled).justify_start().text_size(css(14.)).line_height(css(20.))
            .child(div().relative().size(css(20.)).flex_shrink_0().mr(css(10.))
                .rounded_full().border_1().border_color(rgb(0x737373)).opacity(if disabled { 0.3 } else { 1. })
                .child(div().absolute().left(css(4. + 5. * (1. - alpha))).top(css(4. + 5. * (1. - alpha)))
                    .size(css(10. * alpha)).rounded_full().bg(rgb(0x44d62c)).opacity(alpha)))
            .child(img(icon).size(css(24.)).ml(css(7.)))
            .child(tr(label))
            .on_click(cx.listener(move |this, _, _, cx| this.set_active_phase(Some(phase as u8), cx)))
            .into_any_element()
    }
}

fn phase_button(id: (&'static str, usize), icon: &'static str, tip: &'static str, disabled: bool,
    window: &mut Window, cx: &mut App) -> BaseButton {
    let hover = window.use_keyed_state((ElementId::from(id), "hover"), cx, |_, _| false);
    let hovered = *hover.read(cx) && !disabled;
    let opacity = motion::transition((ElementId::from(id), "tooltip"), if hovered { 1. } else { 0. },
        Transition::new(Duration::from_millis(300)).easing(Easing::Linear), window, cx);
    BaseButton::new(id).relative().p_0().size(css(24.)).disabled(disabled)
        .child(div().size(css(24.)).flex().items_center().justify_center()
            .child(img(SharedString::from(format!("synapse/macro/{icon}{}.svg",
                if icon == "delete" && hovered { "-hover" } else { "" }))).size(css(20.))))
        .on_hover(move |value, _, cx| hover.update(cx, |state, cx| { *state = *value; cx.notify(); }))
        .when(hovered, |button| button.child(deferred(div().absolute().right(css(-45.)).top(css(30.))
            .w_auto().whitespace_nowrap().px(css(10.)).py(css(8.)).bg(rgb(0)).border_1()
            .border_color(rgb(0x5d5d5d)).text_color(rgb(0xcccccc)).text_size(css(14.))
            .line_height(css(16.)).opacity(opacity).child(tr(tip))).with_priority(999)))
}

fn phase_help(phase: usize, key: &'static str, window: &mut Window, cx: &mut App) -> AnyElement {
    let hover = window.use_keyed_state(("macro-phase-help", phase), cx, |_, _| false);
    let hovered = *hover.read(cx);
    let alpha = motion::transition((ElementId::from(("macro-phase-help", phase)), "opacity"),
        if hovered { 1. } else { 0. },
        Transition::new(Duration::from_millis(300)).easing(Easing::Linear), window, cx);
    div().id(("macro-phase-help", phase)).relative().size(css(14.)).rounded_full()
        .bg(rgb(0x6c6c6c)).child(img("synapse/gr-help.svg").size_full())
        .on_hover(move |value, _, cx| hover.update(cx, |state, cx| { *state = *value; cx.notify(); }))
        .when(hovered, |v| v.child(deferred(div().absolute().left_0().top(css(19.))
            .w_auto().whitespace_normal().bg(rgb(0)).border_1().border_color(rgb(0x5d5d5d))
            .px(css(10.)).py(css(8.)).text_size(css(14.)).line_height(css(16.))
            .text_color(rgb(0xcccccc)).opacity(alpha).child(tr(key))).with_priority(100)))
        .into_any_element()
}
