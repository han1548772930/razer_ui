//! Current 164/241 regular Lighting left-column widgets and mounted order.
use super::*;
use gpui_kit::base::{Slider as BaseSlider, SliderIndicator, SliderTrack};
impl SourceControls {
    pub(super) fn receiver_lighting_page(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let enabled = self.draft["brightness"]["isEnabled"] == true;
        let brightness_key = format!("{}:brightness-on", self.spec.product_id);
        let brightness = surface::panel_with_title_switch(
            razer_i18n::t("BRIGHTNESS_HEADER"),
            surface::SynapseSwitch::new("receiver-brightness-switch")
                .accessibility_label(razer_i18n::t("BRIGHTNESS_HEADER"))
                .checked(enabled)
                .on_change(cx.listener(move |this, next: &bool, window, cx| {
                    this.edit(&brightness_key, serde_json::json!(*next), window, cx)
                })),
            surface::help_control(
                "receiver-brightness-help",
                razer_i18n::t("BRIGHTNESS_TOOLTIP"),
            ),
            cx,
        )
        .child(self.receiver_brightness_slider(enabled, window, cx));
        let display = self.draft["switchOffLighting"]["isDisplayOn"] == true;
        let idle = self.draft["switchOffLighting"]["isIdleEnabled"] == true;
        let display_key = format!("{}:display-off", self.spec.product_id);
        let idle_key = format!("{}:idle-off", self.spec.product_id);
        let idle_state = &self.sliders[&format!("{}:idle-minutes", self.spec.product_id)];
        let idle_value = idle_state.read(cx).value().start();
        let idle_panel = surface::panel_with_control(
            razer_i18n::t("SWITCH_OFF_LIGHTING_HEADER"),
            surface::help_control(
                "receiver-lighting-idle-help",
                razer_i18n::t("SWITCH_OFF_LIGHTING_TOOLTIP"),
            ),
            cx,
        )
        .child(
            surface::check_item(
                "receiver-display-off",
                razer_i18n::t("DISPLAY_TURNED_OFF"),
                display,
                !enabled,
                window,
                cx,
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.edit(&display_key, serde_json::json!(!display), window, cx)
            })),
        )
        .child(
            surface::check_item(
                "receiver-idle-off",
                razer_i18n::t("IDLE_FOR_MIN"),
                idle,
                !enabled,
                window,
                cx,
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.edit(&idle_key, serde_json::json!(!idle), window, cx)
            })),
        )
        .child(
            div()
                .relative()
                .h(surface::css(64.))
                .w_full()
                .child(
                    razer_widgets::source_slider::SourceSlider::new(
                        idle_state,
                        (idle_value - 1.) / 14.,
                    )
                    .tip(Some(format!("{idle_value:.0}")))
                    .enabled(enabled && idle),
                )
                .child(
                    div()
                        .absolute()
                        .bottom(surface::css(-2.))
                        .w_full()
                        .flex()
                        .justify_between()
                        .text_color(rgb(0x999999))
                        .text_size(surface::css(14.))
                        .child("1")
                        .child("15"),
                ),
        );
        let quick = self
            .control(&format!("{}:quick-effect", self.spec.product_id))
            .expect("audited source effects list");
        let effects = surface::panel_with_control(
            razer_i18n::t("EFFECTS"),
            surface::help_control("receiver-effects-help", razer_i18n::t("EFFECTS_TOOLTIP")),
            cx,
        )
        .child(self.render_control(quick, window, cx));
        surface::page_columns()
            .child(surface::page_column(
                v_flex().child(brightness).child(idle_panel),
            ))
            .child(surface::page_column(effects))
            .into_any_element()
    }
    fn receiver_brightness_slider(
        &self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state = &self.sliders[&format!("{}:brightness", self.spec.product_id)];
        let value = state.read(cx).value().start();
        let progress = value / 100.;
        let active = if self.receiver_brightness_dragging {
            value != 0.
        } else {
            enabled
        };
        let opacity = surface::fade_opacity(
            "receiver-brightness-opacity",
            if active { 1. } else { 0.3 },
            300,
            window,
            cx,
        );
        // Source `no-pointer` inherits the parent's pointer events: even an
        // off brightness slider accepts dragging and turns on at mouse release.
        BaseSlider::new(state)
            .relative()
            .w_full()
            .h(surface::css(64.))
            .opacity(opacity)
            .child(
                div()
                    .absolute()
                    .bottom(surface::css(25.))
                    .w_full()
                    .h(surface::css(6.))
                    .rounded(surface::css(3.))
                    .bg(rgba(0x44d62c4d))
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |bounds, _, window, _| {
                                let inset = window.rem_size() * 0.5;
                                let width = inset + (bounds.size.width - inset * 2.) * progress;
                                window.paint_quad(PaintQuad {
                                    corner_radii: Corners::all(window.rem_size() * (3. / 16.)),
                                    ..fill(
                                        Bounds::new(bounds.origin, size(width, bounds.size.height)),
                                        rgb(0x44d62c),
                                    )
                                });
                            },
                        )
                        .size_full(),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .left(surface::css(8.))
                    .right(surface::css(8.))
                    .bottom(surface::css(42.))
                    .child(
                        div()
                            .relative()
                            .left(relative(progress))
                            .w_0()
                            .flex()
                            .justify_center()
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .px(surface::css(8.))
                                    .py(surface::css(4.))
                                    .rounded(surface::css(3.))
                                    .bg(rgb(0x44d62c))
                                    .text_color(rgb(0x212121))
                                    .text_size(surface::css(12.))
                                    .line_height(surface::css(14.))
                                    .child(format!("{value:.0}")),
                            ),
                    ),
            )
            .child(
                SliderTrack::new(state)
                    .capture_any_mouse_down(cx.listener(|this, _, _, cx| {
                        this.receiver_brightness_dragging = true;
                        let value = this.sliders[&format!("{}:brightness", this.spec.product_id)]
                            .read(cx)
                            .value()
                            .start() as u8;
                        this.preview_brightness(Some(value), cx);
                        cx.notify();
                    }))
                    .absolute()
                    .bottom(surface::css(20.))
                    .w_full()
                    .h(surface::css(16.))
                    .child(
                        SliderIndicator::new(state)
                            .absolute()
                            .left(surface::css(8.))
                            .right(surface::css(8.))
                            .h_full()
                            .child(
                                razer_widgets::source_slider::source_thumb(
                                    state, active, window, cx,
                                )
                                .disabled(false)
                                .absolute()
                                .left(relative(progress))
                                .ml(surface::css(-8.))
                                .size(surface::css(16.))
                                .rounded(surface::css(8.)),
                            ),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .bottom(surface::css(-2.))
                    .w_full()
                    .flex()
                    .justify_between()
                    .font_family("Roboto")
                    .text_size(surface::css(14.))
                    .text_color(rgb(0x999999))
                    .child("0")
                    .child("100"),
            )
            .into_any_element()
    }
}
