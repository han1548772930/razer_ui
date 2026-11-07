//! Current VERSION_4 input switch: local confirmation, then one unsaved device command.
use super::*;
use razer_device::earbuds_auto_pause::Operation;
use razer_widgets::{keyboard_navigation::Button, theme::AudioEnhancementColors as Colors};

impl AudioProductWorkspace {
    pub(super) fn confirm_earbuds_input(&mut self, cx: &mut Context<Self>) {
        if !self.earbuds.input_popup {
            return;
        }
        self.earbuds.input_popup = false;
        if self.earbuds.active
            && self.earbuds.initialized
            && self
                .earbuds
                .controls
                .as_ref()
                .is_some_and(|c| !c.earbud_disconnected)
        {
            self.earbuds.input_result = None;
            self.earbuds.queue_edit(Operation::SwitchInputSource);
            self.start_earbuds_request(cx);
        }
        cx.notify();
    }

    pub(super) fn earbuds_input_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let disabled = !self.earbuds.active
            || !self.earbuds.initialized
            || self
                .earbuds
                .controls
                .as_ref()
                .is_none_or(|c| c.earbud_disconnected);
        surface::panel_with_control(
            t("INPUT_SOURCE"),
            surface::help_control("earbuds-input-help", t("INPUT_SOURCE_TIP2")),
            cx,
        )
        .relative()
        .child(
            Button::new("earbuds-input-open")
                .disabled(disabled)
                .self_start()
                .h(surface::css(27.))
                .px(surface::css(16.))
                .rounded(surface::css(3.))
                .bg(Colors::mapping_cancel())
                .text_color(Colors::mapping_white())
                .text_size(surface::css(12.))
                .hover(|s| s.bg(Colors::input_button_hover()))
                .child(t("SWITCH_TO_BLE").to_uppercase())
                .on_click(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.earbuds.input_popup = true;
                    cx.notify();
                })),
        )
        .child(
            div()
                .id("earbuds-input-description")
                .test_support()
                .my(surface::css(20.))
                .child(t("SWITCH_TO_BLE_DESC_V4")),
        )
        .when(self.earbuds.input_popup, |panel| {
            panel.child(self.earbuds_input_confirmation(cx))
        })
        .into_any_element()
    }

    fn earbuds_input_confirmation(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .id("earbuds-input-confirmation")
            .test_support()
            .absolute()
            .top(relative(-1.28))
            .w(surface::css(360.))
            .p(surface::css(20.))
            .gap(surface::css(10.))
            .items_center()
            .justify_center()
            .bg(Colors::background())
            .border_1()
            .border_color(Colors::input_warning())
            .rounded(surface::css(5.))
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .text_center()
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.earbuds.input_popup = false;
                    cx.notify();
                }),
            )
            .child(
                h_flex()
                    .gap(surface::css(10.))
                    .items_center()
                    .child(img("synapse/audio-1352-warning.svg").size(surface::css(20.)))
                    .child(
                        div()
                            .font_family("RazerF5")
                            .text_size(surface::css(16.))
                            .text_color(Colors::input_warning())
                            .child(t("INPUT_CONFIRM_POPUP_HEADER").to_uppercase()),
                    ),
            )
            .child(
                div()
                    .text_color(Colors::text())
                    .child(t("INPUT_CONFIRM_POPUP_CONTENT_4")),
            )
            .child(
                div()
                    .text_color(Colors::secondary())
                    .child(t("INPUT_CONFIRM_POPUP_TIP_4")),
            )
            .child(
                Button::new("earbuds-input-confirm")
                    .mt(surface::css(10.))
                    .px(surface::css(16.))
                    .py(surface::css(6.))
                    .rounded(surface::css(3.))
                    .bg(Colors::mapping_cancel())
                    .text_color(Colors::mapping_white())
                    .text_size(surface::css(12.))
                    .line_height(surface::css(14.))
                    .hover(|s| s.bg(Colors::input_button_hover()))
                    .child(t("SWITCH_TO_BLE").to_uppercase())
                    .on_click(cx.listener(|this, _, _, cx| this.confirm_earbuds_input(cx))),
            )
            .into_any_element()
    }
}
