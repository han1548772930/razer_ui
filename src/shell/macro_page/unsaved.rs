//! Current Macro Mn/Bt/an/zn/Gr and 25572 save/discard/duplicate contracts.
use super::*;

pub(super) struct ActionDraft {
    pub(super) actions: Vec<ActionItem>,
    pub(super) undo: Vec<Vec<ActionItem>>,
    pub(super) redo: Vec<Vec<ActionItem>>,
    pub(super) selected: Vec<usize>,
}

#[derive(Clone, Copy)]
pub(super) enum PendingAction {
    New,
    Select(u64),
    Refresh,
}

impl MacroPage {
    pub(super) fn stash_current_draft(&mut self) {
        if let Some(id) = self.actions_for.filter(|id| Some(*id) == self.current) {
            self.inactive_drafts.insert(
                id,
                ActionDraft {
                    actions: self.actions.clone(),
                    undo: self.undo.clone(),
                    redo: self.redo.clone(),
                    selected: self.selected_actions.clone(),
                },
            );
        }
    }

    pub(super) fn finish_pending_edits(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_action_edit(window, cx);
        self.finish_randomized_range(false, window, cx);
        self.finish_rename(cx);
    }

    pub(super) fn request_action(
        &mut self,
        action: PendingAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.suspended_action.is_some() {
            return;
        }
        self.finish_pending_edits(window, cx);
        if self.can_save() {
            self.selector_open = false;
            self.more_open = false;
            self.tree_menu = None;
            self.suspended_action = Some(action);
            self.unsaved_return_focus = window.focused(cx);
            self.unsaved_focus.focus(window, cx);
            cx.notify();
        } else {
            self.perform_action(action, window, cx);
        }
    }

    fn perform_action(
        &mut self,
        action: PendingAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            PendingAction::New => self.create_entry_now(EntryKind::Macro, window, cx),
            PendingAction::Select(id) => self.select_entry_now(id, cx),
            PendingAction::Refresh => {
                // Source reload reconstructs every draft from savedMacros.
                self.inactive_drafts.clear();
                self.load_current_actions();
                self.dismiss_transient_ui(window, cx);
            }
        }
    }

    pub(super) fn cancel_suspended_action(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.suspended_action = None;
        if let Some(focus) = self.unsaved_return_focus.take() {
            focus.focus(window, cx);
        }
        cx.notify();
    }

    fn resolve_suspended_action(
        &mut self,
        save: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(action) = self.suspended_action else {
            return;
        };
        if save {
            self.save_actions(cx);
        } else {
            if let Some(id) = self.current {
                self.inactive_drafts.remove(&id);
            }
            self.load_current_actions();
        }
        self.cancel_suspended_action(window, cx);
        self.perform_action(action, window, cx);
    }

    pub(super) fn unsaved_confirmation(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let panel = v_flex()
            .relative()
            .occlude()
            .w(css(400.))
            .flex_shrink_0()
            .px(css(30.))
            .py(css(20.))
            .bg(rgb(0x111111))
            .border_1()
            .border_color(rgb(0x44d62c))
            .rounded(css(5.))
            .text_color(rgb(0xcccccc))
            .text_size(css(14.))
            .line_height(css(17.))
            .child(
                div()
                    .font_family("RazerF5")
                    .text_size(css(16.))
                    .line_height(css(19.))
                    .text_color(rgb(0x44d62c))
                    .text_center()
                    .mb(css(20.))
                    .child(tr("SAVE_MACRO").to_uppercase()),
            )
            .child(tr("SAVE_REMAPPED_BUTTON_MSG1"))
            .child(div().mt(css(17.)).child(tr("SAVE_REMAPPED_BUTTON_MSG2")))
            .child(
                h_flex()
                    .justify_center()
                    .mt(css(20.))
                    .mr(css(-10.))
                    .children([(false, "DONT_SAVE"), (true, "SAVE")].into_iter().map(
                        |(save, key)| {
                            let id = if save {
                                "macro-unsaved-save"
                            } else {
                                "macro-unsaved-discard"
                            };
                            let pointer = crate::ui::surface::pointer_state(id, window, cx);
                            let (hovered, pressed) = pointer.read(cx).sample();
                            let opacity = crate::ui::surface::fade_opacity(
                                id,
                                if pressed {
                                    0.6
                                } else if hovered {
                                    0.8
                                } else {
                                    1.
                                },
                                300,
                                window,
                                cx,
                            );
                            crate::ui::surface::track_pointer(BaseButton::new(id), &pointer, window)
                                .min_w(css(100.))
                                .h(css(27.))
                                .mr(css(10.))
                                .px(css(10.))
                                .pt(css(6.))
                                .pb(css(7.))
                                .border_1()
                                .border_color(rgb(0))
                                .rounded(css(3.))
                                .bg(rgb(if save { 0x44d62c } else { 0x707070 }))
                                .text_color(rgb(if save { 0 } else { 0xffffff }))
                                .text_size(css(12.))
                                .line_height(css(14.))
                                .opacity(opacity)
                                .child(tr(key).to_uppercase())
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.resolve_suspended_action(save, window, cx)
                                }))
                        },
                    )),
            )
            .child(
                BaseButton::new("macro-unsaved-cancel")
                    .absolute()
                    .top(css(8.))
                    .right(css(8.))
                    .size(css(20.))
                    .p_0()
                    .accessibility_label(i18n::t("CANCEL"))
                    .child(img("synapse/macro/binding-close.svg").size_full())
                    .on_click(
                        cx.listener(|this, _, window, cx| this.cancel_suspended_action(window, cx)),
                    ),
            );
        gpui_kit::base::Dialog::new(cx)
            .focus_handle(self.unsaved_focus.clone())
            .close_on_backdrop_press(false)
            .on_ok(|_, _, _| false)
            .on_close(cx.listener(|this, _, window, cx| this.cancel_suspended_action(window, cx)))
            .backdrop(div().absolute().inset_0().bg(rgba(0x00000080)))
            .popup(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .h_full()
                    .w(window.viewport_size().width + window.rem_size() * (8. / 16.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(panel),
            )
            .into_any_element()
    }
}
