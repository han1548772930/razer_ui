//! Current 3946 AutomationModal → EH: footer-anchored deletion confirmation.
use super::*;
use gpui_kit::base::{Button as BaseButton, Dialog, DialogPopup, motion};
use gpui_kit::component::tooltip::Tooltip;
use std::time::Duration;

pub(super) struct DeleteDecision(pub(super) bool);
impl EventEmitter<DeleteDecision> for DeleteConfirmation {}

pub(super) struct DeleteConfirmation {
    anchor: Rc<Cell<Bounds<Pixels>>>,
    footer: Rc<Cell<Bounds<Pixels>>>,
    focus: FocusHandle,
    cancel_focus: FocusHandle,
    hovered: [bool; 2],
    pressed: [bool; 2],
    resolved: bool,
}

fn policy() -> motion::Transition {
    motion::Transition::new(Duration::from_millis(200)).easing(motion::Easing::Ease)
}

impl DeleteConfirmation {
    fn new(
        anchor: Rc<Cell<Bounds<Pixels>>>,
        footer: Rc<Cell<Bounds<Pixels>>>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            anchor,
            footer,
            focus: cx.focus_handle(),
            cancel_focus: cx.focus_handle().tab_stop(true),
            hovered: [false; 2],
            pressed: [false; 2],
            resolved: false,
        }
    }

    fn decide(&mut self, confirm: bool, cx: &mut Context<Self>) {
        if self.resolved {
            return;
        }
        self.resolved = true;
        cx.emit(DeleteDecision(confirm));
    }

    fn action(&self, confirm: bool, window: &mut Window, cx: &mut Context<Self>) -> BaseButton {
        let index = usize::from(confirm);
        let id = if confirm {
            "automation-confirm-delete"
        } else {
            "automation-cancel-delete"
        };
        let color = if confirm {
            if self.hovered[index] {
                Colors::delete_pressed()
            } else {
                Colors::danger()
            }
        } else if self.hovered[index] {
            Colors::delete_cancel_hover()
        } else {
            Colors::delete_cancel()
        };
        let background = motion::transition((id, "background"), color, policy(), window, cx);
        let opacity = motion::transition(
            (id, "opacity"),
            if self.pressed[index] {
                0.6
            } else if self.hovered[index] {
                0.8
            } else {
                1.
            },
            policy(),
            window,
            cx,
        );
        BaseButton::new(id)
            .accessibility_label(text(if confirm { "DELETE" } else { "CANCEL" }))
            .when(!confirm, |button| button.track_focus(&self.cancel_focus))
            // `.keymap-action.flex > div.thx-btn` outranks the injected
            // two-class padding/border rule: the actual buttons stay 27px.
            .h(surface::css(27.))
            .min_w(surface::css(100.))
            .flex_shrink_0()
            .pt(surface::css(6.))
            .pr(surface::css(10.))
            .pb(surface::css(7.))
            .pl(surface::css(6.))
            .border_1()
            .border_color(Colors::black())
            .rounded(surface::css(3.))
            .text_size(surface::css(12.))
            .line_height(surface::css(14.))
            .text_color(if confirm {
                Colors::black()
            } else {
                Colors::white()
            })
            .text_center()
            .bg(background)
            .opacity(opacity)
            .focus_visible(|style| style.border_color(Colors::foreground()))
            .child(text(if confirm { "DELETE" } else { "CANCEL" }).to_uppercase())
            .on_hover(cx.listener(move |this, hovered, _, cx| {
                this.hovered[index] = *hovered;
                if !*hovered {
                    this.pressed[index] = false;
                }
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.pressed[index] = true;
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.pressed[index] = false;
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.pressed[index] = false;
                    cx.notify();
                }),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.decide(confirm, cx)))
    }
}

impl Render for DeleteConfirmation {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let anchor = self.anchor.get();
        let footer = self.footer.get();
        // Je.left = trigger.left + 150; Dialog CSS translates by -50% of 300.
        let left = anchor.origin.x;
        let top =
            footer.origin.y - footer.size.height - surface::css(12.).to_pixels(window.rem_size());
        let panel = v_flex()
            .id("automation-delete-confirmation")
            .w(surface::css(300.))
            .max_w_full()
            .max_h(surface::css(130.))
            .p(surface::css(20.))
            .border_1()
            .border_color(Colors::danger())
            .rounded(surface::css(3.))
            .bg(Colors::panel())
            .font_family("Roboto")
            .text_color(Colors::foreground())
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .text_center()
            .scrollable_y()
            .child(
                div()
                    .w_full()
                    .flex_shrink_0()
                    .text_size(surface::css(16.))
                    .line_height(surface::css(19.))
                    .text_color(Colors::danger())
                    .mb(surface::css(10.))
                    .child(text("DELETE_ACTION").to_uppercase()),
            )
            .child(
                div()
                    .w_full()
                    .flex_shrink_0()
                    .child(text("DELETE_ACTION_DESC")),
            )
            .child(
                h_flex()
                    .flex_shrink_0()
                    .justify_center()
                    .gap(surface::css(10.))
                    .mt(surface::css(10.))
                    .child(self.action(false, window, cx))
                    .child(self.action(true, window, cx)),
            );
        Dialog::new(cx)
            .layer(3, true)
            .focus_handle(self.focus.clone())
            // EH/ZL have no Escape, Enter or backdrop dismissal callback.
            // Consume those dialog actions instead of closing the parent editor.
            .on_cancel(|_, _, _| false)
            .on_ok(|_, _, _| false)
            .close_on_backdrop_press(false)
            .dismiss_below_y(surface::css(109.).to_pixels(window.rem_size()))
            .backdrop(
                div()
                    .absolute()
                    .top(surface::css(109.))
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .occlude(),
            )
            .popup(
                DialogPopup::new()
                    .absolute()
                    .left(left)
                    .top(top)
                    .child(panel),
            )
    }
}

impl AutomationEditor {
    pub(super) fn open_delete_confirmation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.editing || self.delete_confirmation.is_some() {
            return;
        }
        let popup = cx.new(|cx| {
            DeleteConfirmation::new(
                self.delete_trigger_bounds.clone(),
                self.delete_footer_bounds.clone(),
                cx,
            )
        });
        self.delete_subscription = Some(cx.subscribe_in(
            &popup,
            window,
            |this, _, decision: &DeleteDecision, window, cx| {
                this.delete_confirmation = None;
                this.delete_subscription = None;
                if decision.0 {
                    // Current confirm first closes/reset the editor, then removes
                    // the selected existing rule. No draft Save occurs here.
                    let id = this.original.id;
                    window.close_dialog(cx);
                    cx.emit(EditorEvent::Delete(id));
                } else {
                    this.delete_trigger_focus.focus(window, cx);
                    cx.notify();
                }
            },
        ));
        popup.update(cx, |popup, cx| popup.cancel_focus.focus(window, cx));
        self.delete_confirmation = Some(popup);
        cx.notify();
    }

    pub(super) fn delete_trigger(&self, window: &mut Window, cx: &mut Context<Self>) -> BaseButton {
        let target = if self.editing && self.delete_hovered && self.draft != self.original {
            Colors::delete_dirty_hover()
        } else if self.editing && (self.delete_hovered || self.delete_confirmation.is_some()) {
            Colors::danger()
        } else {
            Colors::foreground()
        };
        let color =
            motion::transition(("automation-delete", "color"), target, policy(), window, cx);
        let bounds = self.delete_trigger_bounds.clone();
        BaseButton::new("automation-delete")
            .absolute()
            .left(surface::css(31.))
            .track_focus(&self.delete_trigger_focus)
            .accessibility_label(text("DELETE"))
            .tooltip(|window, cx| Tooltip::new(text("DELETE")).build(window, cx))
            .size(surface::css(27.))
            .p_0()
            .flex()
            .items_center()
            .justify_center()
            .disabled(!self.editing)
            .opacity(if self.editing { 1. } else { 0.5 })
            .text_color(color)
            .focus_visible(|style| style.border_1().border_color(Colors::danger()))
            .child(
                Icon::default()
                    .path("synapse/automation-delete-action.svg")
                    .size(surface::css(24.)),
            )
            .child(
                canvas(
                    move |rect, window, _| {
                        if bounds.replace(rect) != rect {
                            window.refresh();
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
            .on_hover(cx.listener(|this, hovered, _, cx| {
                this.delete_hovered = *hovered;
                cx.notify();
            }))
            .on_click(cx.listener(|this, _, window, cx| this.open_delete_confirmation(window, cx)))
    }
}
