//! 1303 `_P/Wm/wm` and 1304 `ip/sp/kM`, independently read from current bundles.
//! These speakers mount only the display-off checkbox; their stored idle fields
//! do not establish a mounted idle control.
use super::*;
use gpui_kit::base::{Slider as BaseSlider, SliderIndicator, SliderTrack};

impl AudioProductWorkspace {
    pub(super) fn commit_nommo_brightness(
        &mut self,
        value: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value = value.round().clamp(0., 100.) as u32;
        let brightness = json!({"value": value, "isEnabled": value != 0});
        if self.draft["profile"]["brightness"] != brightness {
            self.draft["profile"]["brightness"] = brightness;
            cx.emit(AudioProductChanged);
        }
        self.sync(window, cx);
        cx.notify();
    }

    pub(super) fn nommo_lighting_section(
        &self,
        section: &AudioSection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let enabled = self.draft["profile"]["brightness"]["isEnabled"] == true;
        match section.title.as_str() {
            "BRIGHTNESS_HEADER" => Some(
                surface::panel_with_title_switch(
                    t("BRIGHTNESS_HEADER"),
                    surface::SynapseSwitch::new("nommo-brightness-switch")
                        .accessibility_label(t("BRIGHTNESS_HEADER"))
                        .checked(enabled)
                        .on_change(cx.listener(|this, next: &bool, window, cx| {
                            this.edit("/profile/brightness/isEnabled", json!(*next), window, cx);
                        })),
                    surface::help_control("nommo-brightness-help", t("BRIGHTNESS_TOOLTIP")),
                    cx,
                )
                .child(self.nommo_brightness_slider(enabled, window, cx))
                .into_any_element(),
            ),
            "SWITCH_OFF_LIGHTING_HEADER" => {
                let checked = self.draft["profile"]["switchOffLighting"]["isDisplayOn"] == true;
                Some(
                    surface::panel_with_control(
                        t("SWITCH_OFF_LIGHTING_HEADER"),
                        surface::help_control(
                            "nommo-switch-off-help",
                            t("SWITCH_OFF_LIGHTING_TOOLTIP"),
                        ),
                        cx,
                    )
                    .child(
                        surface::check_item(
                            "nommo-switch-off-display",
                            t("DISPLAY_TURNED_OFF"),
                            checked,
                            !enabled,
                            window,
                            cx,
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.edit(
                                    "/profile/switchOffLighting/isDisplayOn",
                                    json!(!checked),
                                    window,
                                    cx,
                                );
                            },
                        )),
                    )
                    .into_any_element(),
                )
            }
            _ => None,
        }
    }

    fn nommo_brightness_slider(
        &self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state = &self.sliders["/profile/brightness/value"];
        let value = state.read(cx).value().start();
        let progress = value / 100.;
        let active = if self.nommo_brightness_dragging {
            value != 0.
        } else {
            enabled
        };
        let opacity = surface::fade_opacity(
            "nommo-brightness-opacity",
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
                        this.nommo_brightness_dragging = true;
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
                                crate::ui::source_slider::source_thumb(state, active, window, cx)
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
                    .text_color(rgb(0xcccccc))
                    .child("0")
                    .child("100"),
            )
            .into_any_element()
    }
}
