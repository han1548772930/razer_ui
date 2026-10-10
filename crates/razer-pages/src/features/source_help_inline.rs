//! Current Help `On`/`In`: 300px card-local profile-del alert, one red button.
use super::*;

impl SourceHelp {
    pub(super) fn reset_popup(&self, cx: &Context<Self>) -> AnyElement {
        let Some((_, page)) = self.metadata() else {
            return div().into_any_element();
        };
        let message = i18n::t(if page.obm {
            "FACTORY_RESET_MSG"
        } else {
            "FACTORY_RESET_MSG_NO_OBM_DEVICE"
        });
        v_flex()
            .id("source-help-reset-popup")
            .absolute()
            .left_0()
            .top(surface::css(27.))
            .w(surface::css(300.))
            .p(surface::css(20.))
            .items_center()
            .bg(rgb(0x111111))
            .border_1()
            .border_color(rgb(0xfd4949))
            .rounded(surface::css(3.))
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .occlude()
            .shadow(vec![BoxShadow {
                inset: false,
                color: rgba(0x00000033).into(),
                offset: point(px(0.), surface::css(6.).to_pixels(cx.theme().font_size)),
                blur_radius: surface::css(10.).to_pixels(cx.theme().font_size),
                spread_radius: px(0.),
            }])
            .child(
                div()
                    .mb(surface::css(10.))
                    .text_color(rgb(0xfd4949))
                    .text_center()
                    .child(self.device.display_name().to_uppercase()),
            )
            .child(div().mb(surface::css(10.)).text_center().child(message))
            .child(
                BaseButton::new("source-help-reset-confirm")
                    .accessibility_label(i18n::t("RESET"))
                    .h(surface::css(27.))
                    .min_w(surface::css(90.))
                    .px(surface::css(5.))
                    .py(surface::css(4.))
                    .border_1()
                    .border_color(rgba(0x0000004d))
                    .rounded(surface::css(3.))
                    .bg(rgb(0xfd4949))
                    .text_color(rgb(0x111111))
                    .text_size(surface::css(12.))
                    .line_height(surface::css(14.))
                    .hover(|style| style.opacity(0.8))
                    .active(|style| style.opacity(0.6))
                    .child(i18n::t("RESET").to_uppercase())
                    .on_click(cx.listener(|this, _, window, cx| {
                        cx.stop_propagation();
                        this.confirm_inline_reset(window, cx);
                    })),
            )
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.inline_reset = false;
                this.confirmation_generation = this.confirmation_generation.wrapping_add(1);
                cx.notify();
            }))
            .into_any_element()
    }

    fn confirm_inline_reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((_, page)) = self.metadata() else {
            return;
        };
        if !page.inline_confirmation || !self.inline_reset || self.reset_cooldown {
            return;
        }
        self.inline_reset = false;
        self.pending_reset = true;
        self.reset_error = None;
        self.reset_generation = self.reset_generation.wrapping_add(1);
        // The independently checked route mounts resetObm; Hm selects it only
        // while not using BLE. Preserve that command identity through the host.
        self.reset_source_action =
            if page.obm && (!self.device.use_ble || page.obm_reset_during_ble) {
                "ON_RESET_OBM"
            } else {
                "ON_RESET_DEVICE"
            };
        let request = HelpResetRequest {
            generation: self.reset_generation,
            audio_streams: false,
            source_action: Some(self.reset_source_action),
        };
        self.reset_cooldown = true;
        self.reset_cooldown_task = Some(cx.spawn_in(window, async move |view, cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            let _ = view.update(cx, |view, cx| {
                if view.reset_generation == request.generation {
                    view.reset_cooldown = false;
                    cx.notify();
                }
            });
        }));
        cx.emit(HelpResetEvent::Requested(request));
        cx.notify();
    }
}
