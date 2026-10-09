//! Product 691: current 5171 base CSS, OLED lazy root and its six widgets.
use super::*;
use gpui_kit::base::{Slider as BaseSlider, SliderIndicator, SliderThumb, SliderTrack};
use razer_i18n::t;

impl SourceControls {
    pub(super) fn render_oled_page(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let compact = surface::stacked_device_columns(
            f32::from(window.viewport_size().width),
            f32::from(window.rem_size()),
        );
        let enabled =
            self.draft.pointer("/oled/homeScreenDisplay/enabled") == Some(&Value::Bool(true));
        let home = surface::panel_with_title_switch(
            t("OLED_HOME_SCREEN_DISPLAY_TITLE"),
            surface::SynapseSwitch::new("oled-home-switch")
                .accessibility_label(t("OLED_HOME_SCREEN_DISPLAY_TITLE"))
                .checked(enabled)
                .on_change(cx.listener(|this, value: &bool, window, cx| {
                    this.edit("691:oled-home-enabled", Value::Bool(*value), window, cx);
                })),
            div(),
            cx,
        )
        .relative()
        .w(surface::css(if compact { 600. } else { 1220. }))
        .min_w(surface::css(if compact { 600. } else { 1220. }))
        .when(compact, |panel| panel.px(surface::css(25.)))
        .child(oled_help("home", "OLED_HOME_SCREEN_DISPLAY_TIPS"))
        .child(div().child(t("OLED_HOME_SCREEN_DISPLAY_DESC")))
        .child(self.render_oled_presets(!enabled, window, cx));
        let brightness = oled_panel("OLED_BRIGHTNESS_TITLE", "OLED_BRIGHTNESS_TIPS", cx)
            .child(div().child(t("OLED_BRIGHTNESS_DESC")))
            .child(self.oled_brightness());
        let language_control = self
            .control("691:oled-language")
            .expect("OLED language descriptor");
        let staged = self.selection_value(language_control);
        let language = oled_panel("OLED_LANGUAGE_TITLE", "OLED_LANGUAGE_TIPS", cx)
            .when(self.is_ble, |panel| panel.opacity(0.3))
            .child(div().child(t("OLED_LANGUAGE_SELECT_LABEL")))
            .child(
                h_flex()
                    .my(surface::css(10.))
                    .mb(surface::css(20.))
                    .gap(surface::css(10.))
                    .child(
                        surface::select(&self.selects["691:oled-language"]).disabled(self.is_ble),
                    )
                    .child(
                        gpui_kit::base::Button::new("oled-language-apply")
                            .accessibility_label(t("APPLY"))
                            .disabled(
                                self.is_ble || staged.as_ref() == self.value(language_control),
                            )
                            .styles(|s| s.disabled(|s| s.opacity(0.3)))
                            .h(surface::css(27.))
                            .px(surface::css(15.))
                            .rounded(surface::css(3.))
                            .bg(rgb(0x44d62c))
                            .text_color(rgb(0))
                            .text_size(surface::css(14.))
                            .child(t("APPLY").to_uppercase())
                            .on_click(cx.listener(|this, _, window, cx| {
                                if let Some(value) = this
                                    .control("691:oled-language")
                                    .and_then(|c| this.selection_value(c))
                                {
                                    this.edit("691:oled-language", value, window, cx);
                                }
                            })),
                    ),
            )
            .child(
                div()
                    .text_color(rgb(0x999999))
                    .child(t("OLED_LANGUAGE_DESC")),
            );
        let home_delay = oled_panel(
            "OLED_TIME_TO_HOME_SCREEN_TITLE",
            "OLED_TIME_TO_HOME_SCREEN_TIPS",
            cx,
        )
        .child(surface::h1_body(t("OLED_TIME_TO_HOME_SCREEN_DESC"), cx))
        .child(
            div()
                .mt(surface::css(22.))
                .child(self.oled_numeric("691:oled-home", cx)),
        );
        let dim = oled_panel("OLED_DIM_DISPLAY_TITLE", "OLED_DIM_DISPLAY_TIPS", cx)
            .child(div().child(t("OLED_DIM_DISPLAY_DESC")))
            .child(
                div()
                    .mt(surface::css(20.))
                    .child(self.oled_numeric("691:oled-dim", cx)),
            );
        let screensaver = oled_panel("OLED_SCREEN_SAVER_TITLE", "OLED_SCREEN_SAVER_TIPS", cx)
            .child(div().child(t("OLED_SCREEN_SAVER_DESC")))
            .child(self.oled_screensavers(cx));
        super::super::product_surface::body()
            .child(
                v_flex()
                    .w_full()
                    .max_w(surface::css(1240.))
                    .mx_auto()
                    .items_center()
                    .child(home)
                    .child(
                        surface::page_columns()
                            .child(surface::page_column(
                                v_flex().child(brightness).child(language),
                            ))
                            .child(surface::page_column(
                                v_flex().child(home_delay).child(dim).child(screensaver),
                            )),
                    ),
            )
            .into_any_element()
    }

    fn oled_numeric(&self, key: &'static str, cx: &mut Context<Self>) -> AnyElement {
        let control = self.control(key).expect("OLED numeric descriptor");
        let disabled = self.disabled(control);
        h_flex()
            .relative()
            .flex_wrap()
            .gap(surface::css(10.))
            .children(control.options.iter().map(|option| {
                let value = option.value.clone();
                let selected = self.value(control) == Some(&value);
                gpui_kit::base::Button::new(SharedString::from(format!("{key}-{value}")))
                    .accessibility_label(t(&option.label))
                    .disabled(disabled)
                    .w(surface::css(48.))
                    .h(surface::css(27.))
                    .rounded(surface::css(3.))
                    .border_1()
                    .border_color(if selected {
                        rgb(0x44d62c)
                    } else {
                        rgb(0x5d5d5d)
                    })
                    .bg(rgb(0x222222))
                    .text_color(rgb(0xcccccc))
                    .text_size(surface::css(14.))
                    .hover(|s| s.border_color(rgb(0x44d62c)))
                    .child(t(&option.label))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.edit(key, value.clone(), window, cx)
                    }))
            }))
            .when(disabled, |row| {
                row.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .w(surface::css(300.))
                        .h(surface::css(30.))
                        .bg(rgb(0x111111))
                        .opacity(0.5)
                        .occlude(),
                )
            })
            .into_any_element()
    }

    fn oled_screensavers(&self, cx: &mut Context<Self>) -> AnyElement {
        let control = self
            .control("691:oled-screensaver")
            .expect("OLED screensaver descriptor");
        h_flex()
            .w(surface::css(530.))
            .mt(surface::css(20.))
            .flex_wrap()
            .gap(surface::css(10.))
            .children(control.options.iter().map(|option| {
                let value = option.value.clone();
                let selected = self.value(control) == Some(&value);
                gpui_kit::base::Button::new(SharedString::from(format!("oled-screensaver-{value}")))
                    .accessibility_label(t(&option.label))
                    .disabled(selected)
                    .w(surface::css(260.))
                    .h(surface::css(68.))
                    .flex_shrink_0()
                    .border_1()
                    .bg(rgb(0))
                    .text_color(rgb(0x999999))
                    .text_size(surface::css(14.))
                    .border_color(rgb(0x5d5d5d))
                    .when(selected, |button| {
                        button.border_2().border_color(rgb(0x44d62c))
                    })
                    .hover(|button| button.border_2().border_color(rgb(0x166809)))
                    .child(match &option.image {
                        Some(source) => img(SharedString::from(source.clone()))
                            .w(surface::css(256.))
                            .h(surface::css(64.))
                            .into_any_element(),
                        None => div()
                            .child(format!("({})", t(&option.label)))
                            .into_any_element(),
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.edit("691:oled-screensaver", value.clone(), window, cx)
                    }))
            }))
            .into_any_element()
    }

    fn oled_brightness(&self) -> AnyElement {
        let state = &self.sliders["691:oled-brightness"];
        let value = self
            .draft
            .pointer("/oled/oledBrightness")
            .and_then(Value::as_f64)
            .unwrap_or(50.) as f32;
        let progress = ((value - 20.) / 80.).clamp(0., 1.);
        BaseSlider::new(state)
            .relative()
            .w_full()
            .h(surface::css(64.))
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
                                SliderThumb::new(state)
                                    .absolute()
                                    .left(relative(progress))
                                    .ml(surface::css(-8.))
                                    .size(surface::css(16.))
                                    .rounded_full()
                                    .bg(rgb(0x44d62c))
                                    .hover(|s| {
                                        s.bg(rgb(0x5d5d5d)).border_2().border_color(rgb(0x44d62c))
                                    })
                                    .active(|s| {
                                        s.bg(rgb(0x383838)).border_2().border_color(rgb(0x44d62c))
                                    }),
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
                    .text_size(surface::css(14.))
                    .child("20")
                    .child("100"),
            )
            .into_any_element()
    }
}

fn oled_panel(title: &'static str, help: &'static str, cx: &App) -> Div {
    surface::panel(t(title), cx)
        .relative()
        .child(oled_help(title, help))
}
fn oled_help(id: &'static str, text: &'static str) -> AnyElement {
    div()
        .absolute()
        .top(surface::css(10.))
        .right(surface::css(10.))
        .child(surface::help_control(
            SharedString::from(format!("oled-help-{id}")),
            t(text),
        ))
        .into_any_element()
}
