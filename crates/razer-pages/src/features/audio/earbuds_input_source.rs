//! Current VERSION_4 input switch: local confirmation, then one unsaved device command.
use super::*;
use razer_widgets::{keyboard_navigation::Button, theme::AudioEnhancementColors as Colors};

impl AudioProductWorkspace {
    pub(super) fn confirm_earbuds_input(&mut self, cx: &mut Context<Self>) {
        if !self.earbuds_sound.input_popup {
            return;
        }
        self.earbuds_sound.input_popup = false;
        if self.earbuds_sound.active() && self.earbuds_connection.ready() {
            self.earbuds_sound.input_result = None;
            self.request_earbuds_sound_action(
                super::earbuds_sound_actions::Action::InputSource,
                cx,
            );
        }
        cx.notify();
    }

    pub(super) fn earbuds_input_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let disconnected = self.earbuds_connection.disconnected();
        let disabled = !self.earbuds_sound.active() || !self.earbuds_connection.ready();
        surface::panel_with_control(
            t("INPUT_SOURCE"),
            surface::help_control("earbuds-input-help", t("INPUT_SOURCE_TIP2")),
            cx,
        )
        .id("earbuds-input-panel")
        .test_support()
        .relative()
        .when(disconnected, |panel| panel.opacity(0.5))
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
                .when(!disabled, |button| {
                    button.hover(|s| s.bg(Colors::input_button_hover()))
                })
                .child(t("SWITCH_TO_BLE").to_uppercase())
                .on_click(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.earbuds_sound.input_popup = true;
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
        .children(
            self.earbuds_sound
                .input_result
                .as_ref()
                .and_then(|result| match result {
                    Err(error)
                    | Ok(razer_device::earbuds_protocol40::WriteOutcome::Failed(error)) => {
                        Some(error)
                    }
                    _ => None,
                })
                .map(|error| {
                    div()
                        .id("earbuds-input-error")
                        .test_support()
                        .text_size(surface::css(12.))
                        .text_color(cx.theme().danger)
                        .child(error.clone())
                }),
        )
        .when(self.earbuds_sound.input_popup, |panel| {
            panel.child(
                razer_assets::deferred_resource(self.earbuds_input_confirmation(cx))
                    .with_priority(1002),
            )
        })
        .when(disconnected, |panel| {
            panel
                .child(
                    div()
                        .id("earbuds-input-disabled")
                        .test_support()
                        .absolute()
                        .inset_0()
                        .occlude()
                        .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                            if !*hovered {
                                this.earbuds_sound.input_disabled_cursor = None;
                            }
                            cx.notify();
                        }))
                        .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                            this.earbuds_sound.input_disabled_cursor = Some(event.position);
                            cx.notify();
                        })),
                )
                .children(self.earbuds_sound.input_disabled_cursor.map(|cursor| {
                    use razer_widgets::theme::AudioEqualizerColors as C;
                    razer_assets::deferred_resource(super::earbuds_equalizer_view::CursorTip::new(
                        cursor,
                        div()
                            .id("earbuds-input-disabled-tip")
                            .test_support()
                            .max_w(surface::css(320.))
                            .px(surface::css(10.))
                            .py(surface::css(8.))
                            .border_1()
                            .border_color(C::border())
                            .bg(C::tip_surface())
                            .text_color(C::text())
                            .text_size(surface::css(14.))
                            .line_height(surface::css(16.))
                            .child(
                                gpui_kit::base::Tooltip::new("earbuds-input-disabled-content")
                                    .child(t("LIMITED_FUNCTIONALITY_EARBUD")),
                            ),
                    ))
                    .with_priority(999999)
                }))
        })
        .into_any_element()
    }

    fn earbuds_input_confirmation(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .id("earbuds-input-confirmation")
            .test_support()
            .absolute()
            .occlude()
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
                    this.earbuds_sound.input_popup = false;
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
