//! Original `.save-alert` composition on Base Dialog's modal/focus lifecycle.
#[cfg(test)]
#[path = "source_alert_tests.rs"]
mod tests;
use super::surface;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    scroll::ScrollableElement as _,
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::rc::Rc;

#[derive(Clone, Copy)]
pub(crate) enum AlertPlacement {
    // 182: top:50%; translateY(-100%).
    AboveCenter,
    // 653 final CSS override: top:30%; translateY(-50%).
    UpperCenter,
}

#[derive(Clone)]
pub(crate) struct AlertAction {
    id: &'static str,
    label: SharedString,
    primary: bool,
    disabled: bool,
    action: Rc<dyn Fn(&mut Window, &mut App)>,
}
impl AlertAction {
    pub(crate) fn new(
        id: &'static str,
        label: impl Into<SharedString>,
        action: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id,
            label: label.into(),
            primary: false,
            disabled: false,
            action: Rc::new(action),
        }
    }
    pub(crate) fn primary(mut self) -> Self {
        self.primary = true;
        self
    }
    pub(crate) fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

pub(crate) struct SourceAlert {
    title: SharedString,
    body: SharedString,
    close_id: &'static str,
    actions: Vec<AlertAction>,
    placement: AlertPlacement,
    focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    open: bool,
}
impl SourceAlert {
    pub(crate) fn open(
        title: impl Into<SharedString>,
        body: impl Into<SharedString>,
        close_id: &'static str,
        actions: Vec<AlertAction>,
        placement: AlertPlacement,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        let view = cx.new(|cx| Self {
            title: title.into(),
            body: body.into(),
            close_id,
            actions,
            placement,
            focus: cx.focus_handle(),
            return_focus: window.focused(cx),
            open: true,
        });
        let focus = view.read(cx).focus.clone();
        focus.focus(window, cx);
        view
    }
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(focus) = self.return_focus.take() {
            focus.focus(window, cx);
        }
        self.open = false;
        self.actions.clear();
        cx.notify();
    }
}
impl Render for SourceAlert {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }
        let width = (window.rem_size() * 25.)
            .min((window.viewport_size().width - window.rem_size() * 2.).max(px(0.)));
        let upper_height = window.viewport_size().height
            * match self.placement {
                AlertPlacement::AboveCenter => 0.5,
                AlertPlacement::UpperCenter => 0.6,
            };
        let panel = v_flex()
            .id("source-save-alert")
            .test_support()
            .relative()
            .occlude()
            .w(width)
            .max_h(upper_height)
            .flex_shrink_0()
            .py(surface::css(20.))
            .px(surface::css(30.))
            .bg(cx.theme().popover)
            .border_1()
            .border_color(cx.theme().primary)
            .rounded(surface::css(5.))
            .text_color(cx.theme().foreground)
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .text_center()
            .child(
                div()
                    .flex_shrink_0()
                    .font_family("RazerF5")
                    .text_size(surface::css(16.))
                    .line_height(surface::css(19.))
                    .text_color(cx.theme().primary)
                    .mb(surface::css(20.))
                    .child(self.title.clone()),
            )
            .child(
                div()
                    .id("source-save-alert-body")
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(self.body.clone()),
            )
            .child(
                h_flex()
                    .flex_shrink_0()
                    .justify_center()
                    .flex_wrap()
                    .gap(surface::css(10.))
                    .mt(surface::css(20.))
                    .children(self.actions.iter().map(|action| {
                        let callback = action.action.clone();
                        Button::new(action.id)
                            .label(action.label.clone())
                            .xsmall()
                            .disabled(action.disabled)
                            .h(surface::css(27.))
                            .min_w(surface::css(100.))
                            .px(surface::css(10.))
                            .py_0()
                            .border_1()
                            .border_color(cx.theme().title_bar)
                            .rounded(cx.theme().font_size * (3. / 16.))
                            .text_size(surface::css(12.))
                            .line_height(surface::css(14.))
                            .when(action.primary, |button| button.primary())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.close(window, cx);
                                callback(window, cx);
                            }))
                    })),
            )
            .child(
                Button::new(self.close_id)
                    .ghost()
                    .absolute()
                    .top(surface::css(8.))
                    .right(surface::css(8.))
                    .size(surface::css(20.))
                    .p_0()
                    .accessibility_label("继续编辑")
                    .tooltip("继续编辑")
                    .child(img("synapse/mapping-close.svg").size_full())
                    .on_click(cx.listener(|this, _, window, cx| this.close(window, cx))),
            );
        gpui_kit::base::Dialog::new(cx)
            .focus_handle(self.focus.clone())
            .close_on_backdrop_press(false)
            .on_ok(|_, _, _| false)
            .on_close(cx.listener(|this, _, window, cx| this.close(window, cx)))
            .backdrop(
                div()
                    .absolute()
                    .inset_0()
                    .bg(cx.theme().title_bar.opacity(0.5)),
            )
            .popup(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w(window.viewport_size().width)
                    .h(upper_height)
                    .flex()
                    .justify_center()
                    .when(
                        matches!(self.placement, AlertPlacement::AboveCenter),
                        |this| this.items_end(),
                    )
                    .when(
                        matches!(self.placement, AlertPlacement::UpperCenter),
                        |this| this.items_center(),
                    )
                    .child(panel),
            )
            .into_any_element()
    }
}
