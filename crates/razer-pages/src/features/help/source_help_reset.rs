//! Current PID 756 Tt Help: source reset waiting, acknowledgement and BLE gate.
use super::*;
use razer_widgets::theme::HelpResetColors as Colors;

impl SourceHelp {
    /// Source componentDidUpdate: actual initObmCompleted/profiles change.
    /// An accepted write request alone must not call this observation.
    pub(in crate::features) fn observe_obm_reset_change(&mut self, cx: &mut Context<Self>) {
        if self.reset_cooldown
            && self
                .metadata()
                .is_some_and(|(_, page)| page.wait_for_obm_reset_completion)
        {
            self.reset_cooldown = false;
            self.reset_cooldown_task = None;
            cx.notify();
        }
    }

    pub(in crate::features) fn observe_connection(&mut self, is_ble: bool, cx: &mut Context<Self>) {
        let changed = self.observed_ble.unwrap_or(self.device.use_ble) != is_ble;
        self.observed_ble = Some(is_ble);
        if changed
            && self.metadata().is_some_and(|(_, page)| {
                matches!(page.popup_kind.as_deref(), Some("oled" | "audio-oled"))
            })
        {
            self.inline_reset = false;
            self.mount_inline_popup_layout(cx);
        }
        if self
            .metadata()
            .is_some_and(|(_, page)| self.factory_reset_disabled(page))
        {
            self.inline_reset = false;
        }
        cx.notify();
    }

    /// Actual oledResetReducer state, separate from a locally accepted request.
    pub(in crate::features) fn observe_oled_resetting(
        &mut self,
        is_resetting: bool,
        cx: &mut Context<Self>,
    ) {
        if self.observed_oled_resetting != is_resetting {
            self.observed_oled_resetting = is_resetting;
            if is_resetting {
                self.inline_reset = false;
            }
            cx.notify();
        }
    }

    pub(super) fn factory_reset_disabled(&self, page: &HelpPage) -> bool {
        page.disable_factory_reset_for_ble && self.observed_ble.unwrap_or(self.device.use_ble)
    }

    pub(super) fn factory_reset_overlay(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        // The source fixed overlay covers the product WebContents below TabUI.
        let size = window.viewport_size();
        let top = window.rem_size() / 16. * 42.;
        let layer = v_flex()
            .id("source-help-factory-reset-overlay")
            .track_focus(&self.reset_overlay_focus)
            .w(size.width)
            .h((size.height - top).max(px(0.)))
            .items_center()
            .justify_center()
            .occlude()
            .bg(Colors::shade())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .on_key_down(|_, _, cx| cx.stop_propagation())
            .when(!self.reset_overlay_acknowledged, |layer| {
                layer.child(
                    v_flex()
                        .w(surface::css(400.))
                        .p(surface::css(20.))
                        .gap(surface::css(10.))
                        .items_center()
                        .text_center()
                        .bg(Colors::background())
                        .border_1()
                        .border_color(Colors::warning())
                        .font_family("Roboto")
                        .text_size(surface::css(14.))
                        .line_height(surface::css(17.))
                        .text_color(Colors::text())
                        .child(
                            h_flex()
                                .gap(surface::css(6.))
                                .font_weight(FontWeight::BOLD)
                                .text_color(Colors::warning())
                                .child("⚠")
                                .child(i18n::t("FACTORY_RESET").to_uppercase()),
                        )
                        .child(i18n::t("FACTORY_RESET_IN_PROGRESS"))
                        .child(
                            BaseButton::new("source-help-factory-reset-acknowledge")
                                .accessibility_label("OK")
                                .min_w(surface::css(90.))
                                .px(surface::css(16.))
                                .pt(surface::css(7.))
                                .pb(surface::css(6.))
                                .rounded(surface::css(3.))
                                .border_1()
                                .border_color(Colors::button_border())
                                .bg(Colors::button())
                                .text_color(Colors::button_text())
                                .child("OK")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.reset_overlay_acknowledged = true;
                                    cx.notify();
                                })),
                        ),
                )
            });
        razer_assets::deferred_resource(
            anchored()
                .anchor(Anchor::TopLeft)
                .position(point(px(0.), top))
                .child(layer),
        )
        .priority(100)
        .into_any_element()
    }
}
