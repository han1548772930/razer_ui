use super::artwork::paired_mouse_image;
use super::*;
use gpui_kit::base::{Button as BaseButton, Slider as BaseSlider, SliderTrack};
use razer_widgets::source_slider::source_thumb;
use razer_widgets::theme::HyperfluxColors as Colors;

impl SourceControls {
    fn hyperflux_help(&self, id: &'static str, key: &'static str) -> AnyElement {
        div()
            .absolute()
            .right(surface::css(10.))
            .top(surface::css(10.))
            .child(surface::receiver_help_control(id, razer_i18n::t(key)))
            .into_any_element()
    }
    fn hyperflux_mouse_name(&self) -> String {
        let locale = razer_i18n::locale().to_lowercase();
        self.hyperflux
            .mouse()
            .and_then(|p| {
                p["productName"]
                    .get(&locale)
                    .filter(|v| truthy(v))
                    .or_else(|| p["productName"].get("en"))
            })
            .and_then(Value::as_str)
            .unwrap_or_default()
            .into()
    }
    fn hyperflux_paired_description(&self, cx: &mut Context<Self>) -> AnyElement {
        let template = razer_i18n::t("SEAMLESS_AUTO_PAIRING_PAIRED_DESCRIPTION");
        let Some((before, after)) = template.split_once("{{deviceName}}") else {
            return div().child(template).into_any_element();
        };
        // Ye.A underlines only the inserted name, and activates it only on status 1.
        let mut name = BaseButton::new("hyperflux-paired-mouse")
            .disabled(!self.hyperflux.auto_enabled() || !self.hyperflux.mouse_on())
            .accessibility_label(self.hyperflux_mouse_name())
            .underline()
            .text_color(Colors::text())
            .child(self.hyperflux_mouse_name());
        if self.hyperflux.mouse_on() {
            name = name.on_click(cx.listener(|this, _, _, cx| {
                if let Some(name) = &this.hyperflux.mouse_window {
                    this.hyperflux_request(
                        HyperfluxOperation::OpenPairedMouse {
                            mw_window_name: name.clone(),
                        },
                        cx,
                    );
                }
            }));
        }
        div()
            .flex()
            .flex_wrap()
            .items_start()
            .child(before.to_owned())
            .child(name)
            .child(after.to_owned())
            .into_any_element()
    }
    fn render_hyperflux_auto(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let enabled = self.hyperflux.auto_enabled();
        let switch = surface::SynapseSwitch::new("hyperflux-auto-pairing-switch")
            .checked(enabled)
            .accessibility_label(razer_i18n::t("SEAMLESS_AUTO_PAIRING"))
            .on_change(cx.listener(|this, value, _, cx| this.set_hyperflux_auto(*value, cx)));
        let mut row = h_flex()
            .items_start()
            .child(
                img("synapse/hyperflux-207/seamlessautopairing.svg")
                    .w(surface::css(44.))
                    .h(surface::css(40.))
                    .mr(surface::css(13.))
                    .opacity(if enabled { 1. } else { 0.3 }),
            )
            .child(
                div()
                    .min_w_0()
                    .max_w(surface::css(299.))
                    .line_height(relative(1.4))
                    .opacity(if enabled { 1. } else { 0.3 })
                    .child(if self.hyperflux.mouse().is_some() {
                        self.hyperflux_paired_description(cx)
                    } else {
                        div()
                            .child(razer_i18n::t("SEAMLESS_AUTO_PAIRING_DESCRIPTION"))
                            .into_any_element()
                    }),
            );
        if self.hyperflux.mouse().is_some() {
            let mut unpair = div()
                .relative()
                .ml(surface::css(40.))
                .flex_shrink_0()
                .opacity(if enabled { 1. } else { 0.3 })
                .child(
                    BaseButton::new("hyperflux-auto-unpair")
                        .disabled(!enabled)
                        .accessibility_label(razer_i18n::t("UNPAIR"))
                        .px(surface::css(10.))
                        .h(surface::css(28.))
                        .rounded(surface::css(2.))
                        .bg(Colors::cancel())
                        .text_color(Colors::white())
                        .text_size(surface::css(12.))
                        .hover(|s| s.bg(Colors::cancel_hover()))
                        .child(razer_i18n::t("UNPAIR").to_uppercase())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.hyperflux.unpair_open = true;
                            cx.notify();
                        })),
                );
            if self.hyperflux.unpair_open {
                unpair = unpair.child(
                    div()
                        .id("hyperflux-unpair-confirm")
                        .absolute()
                        .left(surface::css(30.))
                        .top(surface::css(36.))
                        .w(surface::css(230.))
                        .py(surface::css(20.))
                        .rounded(surface::css(3.))
                        .border_1()
                        .border_color(Colors::warning())
                        .bg(Colors::panel())
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                            this.hyperflux.unpair_open = false;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .w(surface::css(190.))
                                .mx_auto()
                                .mb(surface::css(9.))
                                .text_center()
                                .text_size(surface::css(14.))
                                .line_height(surface::css(17.))
                                .child(razer_i18n::t("SEAMLESS_AUTO_PAIRING_UNPAIR_DIALOG")),
                        )
                        .child(
                            h_flex()
                                .justify_center()
                                .child(
                                    BaseButton::new("hyperflux-unpair-cancel")
                                        .h(surface::css(28.))
                                        .min_w(surface::css(90.))
                                        .px(surface::css(10.))
                                        .mr(surface::css(10.))
                                        .border_1()
                                        .border_color(Colors::black())
                                        .rounded(surface::css(3.))
                                        .bg(Colors::cancel())
                                        .text_color(Colors::white())
                                        .opacity(surface::fade_opacity(
                                            "hyperflux-cancel-opacity",
                                            0.7,
                                            300,
                                            window,
                                            cx,
                                        ))
                                        .hover(|s| s.opacity(1.))
                                        .child(razer_i18n::t("CANCEL").to_uppercase())
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.hyperflux.unpair_open = false;
                                            cx.stop_propagation();
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    BaseButton::new("hyperflux-unpair-confirm-action")
                                        .h(surface::css(28.))
                                        .min_w(surface::css(90.))
                                        .px(surface::css(10.))
                                        .border_1()
                                        .border_color(Colors::black())
                                        .rounded(surface::css(3.))
                                        .bg(Colors::green())
                                        .text_color(Colors::button_text())
                                        .hover(|s| s.opacity(0.7))
                                        .child(razer_i18n::t("CONFIRM").to_uppercase())
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.confirm_hyperflux_unpair(cx);
                                            cx.stop_propagation();
                                        })),
                                ),
                        ),
                );
            }
            row = row.child(unpair);
        }
        surface::panel_with_title_switch(razer_i18n::t("SEAMLESS_AUTO_PAIRING"), switch, div(), cx)
            .relative()
            .child(row)
            .child(
                div()
                    .mt(surface::css(20.))
                    .text_color(Colors::secondary())
                    .opacity(if enabled { 0. } else { 1. })
                    .child(razer_i18n::t("USE_MULTI_DEVICE_PAIRING_UTILITY")),
            )
            .child(self.hyperflux_help("hyperflux-auto-help", "SEAMLESS_AUTO_PAIRING_TOOLTIP"))
            .into_any_element()
    }
    fn render_hyperflux_battery_slider(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state = self
            .hyperflux
            .slider
            .as_ref()
            .expect("207 retained battery slider");
        let enabled = self.hyperflux.enabled()
            && (self.hyperflux.mouse_on() || !self.hyperflux.charging_override());
        let pointer = enabled && self.hyperflux.mouse_on() && !self.hyperflux.charging_override();
        let value = state.read(cx).value().start();
        let focus = window
            .use_keyed_state(
                ("hyperflux-battery-focus", state.entity_id()),
                cx,
                |_, cx| cx.focus_handle(),
            )
            .read(cx)
            .clone();
        let pointer_focus = focus.clone();
        let mut range = div()
            .id(("hyperflux-battery-range", state.entity_id()))
            .relative()
            .w_full()
            .h(surface::css(64.))
            .mt(surface::css(-15.))
            .opacity(surface::fade_opacity(
                "hyperflux-battery-opacity",
                if enabled { 1. } else { 0.3 },
                300,
                window,
                cx,
            ))
            .child(
                div()
                    .absolute()
                    .bottom(surface::css(25.))
                    .h(surface::css(6.))
                    .w_full()
                    .rounded(surface::css(3.))
                    .bg(Colors::button_text()),
            )
            .child(
                div()
                    .absolute()
                    .bottom(surface::css(25.))
                    .left(relative(0.5))
                    .w(relative(0.3))
                    .h(surface::css(6.))
                    .rounded(surface::css(3.))
                    .bg(Colors::cancel()),
            )
            // xm.updateValue assigns p*fullWidth, with CSS max-width 30%.
            .child(
                div()
                    .absolute()
                    .bottom(surface::css(25.))
                    .left(relative(0.5))
                    .w(relative(((value - 50.) / 30.).clamp(0., 0.3)))
                    .h(surface::css(6.))
                    .rounded(surface::css(3.))
                    .bg(Colors::green()),
            )
            .child(
                div()
                    .id("hyperflux-battery-input")
                    .absolute()
                    .left(relative(0.5))
                    .w(relative(0.3))
                    .bottom(surface::css(20.))
                    .h(surface::css(16.))
                    .track_focus(&focus)
                    .tab_stop(true)
                    .capture_any_mouse_down(cx.listener(
                        move |this, event: &MouseDownEvent, window, cx| {
                            if !pointer {
                                cx.stop_propagation();
                                return;
                            }
                            if event.button == MouseButton::Left {
                                this.hyperflux.slider_dragging = true;
                                pointer_focus.focus(window, cx);
                            }
                        },
                    ))
                    // Source ancestors use pointer-events, never a disabled range.
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        let current = this
                            .hyperflux
                            .slider
                            .as_ref()
                            .unwrap()
                            .read(cx)
                            .value()
                            .start();
                        let next = match event.keystroke.key.as_str() {
                            "up" | "right" => current + 5.,
                            "down" | "left" => current - 5.,
                            "home" => 50.,
                            "end" => 80.,
                            _ => return,
                        };
                        this.set_hyperflux_battery(true, next, window, cx);
                        cx.stop_propagation();
                    }))
                    .child(
                        BaseSlider::new(state).relative().size_full().child(
                            SliderTrack::new(state)
                                .disabled(!pointer)
                                .relative()
                                .h_full()
                                .w_full()
                                .child(
                                    source_thumb(state, pointer, window, cx)
                                        .size(surface::css(16.))
                                        .rounded(surface::css(8.)),
                                ),
                        ),
                    ),
            );
        for (label, left, right, width) in [
            ("0", Some(0.), None, None),
            ("50", Some(0.), None, Some(1.03)),
            ("80", Some(0.), None, Some(1.57)),
            ("100", None, Some(0.), None),
        ] {
            range = range.child(
                div()
                    .absolute()
                    .bottom(surface::css(-2.))
                    .when_some(left, |v, left| v.left(relative(left)))
                    .when_some(right, |v, right| v.right(relative(right)))
                    .when_some(width, |v, width| v.w(relative(width)).text_center())
                    .child(label),
            );
        }
        range.into_any_element()
    }
    fn render_hyperflux_battery(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let powered = self.hyperflux.mouse_on();
        let enabled = self.hyperflux.enabled();
        let override_on = self.hyperflux.charging_override();
        let switch = surface::SynapseSwitch::new("hyperflux-battery-switch")
            .checked(enabled)
            .disabled(!powered)
            .accessibility_label(razer_i18n::t("BATTERY_HEALTH_OPTIMIZER"))
            .on_change(cx.listener(|this, value, window, cx| {
                this.set_hyperflux_battery(*value, this.hyperflux.limit(), window, cx)
            }));
        let opacity = surface::fade_opacity(
            "hyperflux-battery-card-opacity",
            if powered { 1. } else { 0.3 },
            200,
            window,
            cx,
        );
        let body = div()
            .opacity(if override_on { 0.3 } else { 1. })
            .child(razer_i18n::t(
                "BATTERY_HEALTH_OPTIMIZER_MOUSEMAT_DESCRIPTION",
            ))
            .child(
                h_flex()
                    .mt(surface::css(20.))
                    .items_center()
                    .child(
                        surface::select(
                            self.hyperflux
                                .select
                                .as_ref()
                                .expect("207 retained battery selector"),
                        )
                        .items(Self::hyperflux_choices())
                        .w(surface::css(62.))
                        .disabled(!enabled || !powered || override_on)
                        .accessibility_label(razer_i18n::t("BATTERY_HEALTH_OPTIMIZER")),
                    )
                    .child(
                        div()
                            .pl(surface::css(5.))
                            .opacity(if override_on { 0.3 } else { 1. })
                            .child("%"),
                    ),
            )
            .child(self.render_hyperflux_battery_slider(window, cx));
        let card =
            v_flex()
                .relative()
                .w_full()
                .flex_shrink_0()
                .my(surface::css(10.))
                .py(surface::css(30.))
                .px(surface::css(40.))
                .bg(Colors::panel())
                .rounded(surface::css(5.))
                .child(
                    h_flex()
                        .opacity(opacity)
                        .mb(surface::css(20.))
                        .items_center()
                        .gap(surface::css(10.))
                        .child(
                            div()
                                .font_family("RazerF5")
                                .text_size(surface::css(16.))
                                .text_color(Colors::green())
                                .child(razer_i18n::t("BATTERY_HEALTH_OPTIMIZER").to_uppercase()),
                        )
                        .child(switch),
                )
                .child(div().opacity(opacity).child(body))
                .child(div().opacity(opacity).child(
                    self.hyperflux_help("hyperflux-battery-help", "MOUSEMAT_BATTERY_TOOLTIP"),
                ));
        // QS centerTips is mounted when disabled OR active === false, with hover
        // visibility at the widget-container level (not as permanent body text).
        let mut wrapper = div().id("hyperflux-battery-card").relative().child(card);
        if !powered || !enabled {
            let hover = window.use_keyed_state("hyperflux-battery-center-hover", cx, |_, _| false);
            let alpha = surface::fade_opacity(
                "hyperflux-battery-center-opacity",
                if *hover.read(cx) { 1. } else { 0. },
                300,
                window,
                cx,
            );
            wrapper = wrapper
                .on_hover(window.listener_for(&hover, |hover, value, _, cx| {
                    *hover = *value;
                    cx.notify();
                }))
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .w(surface::css(280.))
                                .px(surface::css(10.))
                                .py(surface::css(8.))
                                .border_1()
                                .border_color(Colors::border())
                                .bg(Colors::black())
                                .text_color(Colors::text())
                                .line_height(surface::css(16.))
                                .opacity(alpha)
                                .child(razer_i18n::t("BATTERY_HEALTH_OPTIMIZER_MOUSEMAT_TOOLTIP")),
                        ),
                );
        }
        wrapper.into_any_element()
    }
    fn render_hyperflux_pairing(&self, cx: &mut Context<Self>) -> AnyElement {
        let same_container = self
            .hyperflux
            .valid_devices
            .iter()
            .find(|d| d["productId"] == 207)
            .is_some_and(|owner| {
                self.hyperflux.valid_devices.iter().any(|d| {
                    matches!(d["productId"].as_u64(), Some(179 | 164))
                        && d["deviceContainerId"] == owner["deviceContainerId"]
                })
            });
        let disabled = !self.hyperflux.is_dongle
            || (!self.hyperflux.feature
                && (same_container || self.hyperflux.master_blocks_pairing));
        surface::panel(razer_i18n::t("HYPER_SPEED_MULTI_DEVICE_PAIRING"), cx)
            .relative()
            .child(
                h_flex()
                    .items_center()
                    .child(
                        img("synapse/hyperflux-207/multideviceparing.svg")
                            .size(surface::css(44.))
                            .mr(surface::css(20.)),
                    )
                    .child(
                        BaseButton::new("hyperflux-pairing-utility")
                            .disabled(disabled)
                            .accessibility_label(razer_i18n::t("OPEN_PAIRING_UTILITY"))
                            .text_color(Colors::text())
                            .underline()
                            .line_height(surface::css(44.))
                            .opacity(if disabled { 0.45 } else { 1. })
                            .when(disabled, |b| b.relative().top(surface::css(-10.)))
                            .hover(|s| s.text_color(Colors::green()))
                            .active(|s| s.opacity(0.7))
                            .child(razer_i18n::t("OPEN_PAIRING_UTILITY"))
                            .on_click(
                                cx.listener(|this, _, _, cx| this.open_hyperflux_pairing(cx)),
                            ),
                    ),
            )
            .when(disabled, |p| {
                p.child(
                    div()
                        .relative()
                        .top(surface::css(-20.))
                        .ml(surface::css(63.))
                        .child(razer_i18n::t("ENABLE_LAUNCH_PAIRING_UTILITY_INFO")),
                )
            })
            .child(self.hyperflux_help(
                "hyperflux-pairing-help",
                "HYPERFLUX_DUALINK_PROPERTIES_TOOLTIP",
            ))
            .into_any_element()
    }
    pub(in crate::features::source_controls) fn render_hyperflux(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let edition = if self.hyperflux.edition == 128 { 128 } else { 0 };
        let mut artwork = div()
            .relative()
            .w_full()
            .min_w(surface::css(1024.))
            .max_w(surface::css(1220.))
            .h(surface::css(250.))
            .mx_auto()
            .my(surface::css(10.))
            .child(surface::dot_background(cx))
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .justify_center()
                    .items_center()
                    .child(
                        img(SharedString::from(format!(
                            "synapse/hyperflux-207/product-{edition}-3x.png"
                        )))
                        .h(surface::css(250.))
                        .object_fit(ObjectFit::Contain),
                    ),
            );
        if let Some(mouse) = self.hyperflux.mouse() {
            if let Some(image) = paired_mouse_image(mouse) {
                artwork = artwork.child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .justify_center()
                        .items_center()
                        .child(
                            img(image)
                                .h(surface::css(100.))
                                .object_fit(ObjectFit::Contain),
                        ),
                );
            }
        }
        // Current QS wraps each widget in `.widget-container{display:flex}`,
        // so the flattened panels must retain both 10px vertical margins.
        let left = v_flex()
            .child(self.render_hyperflux_auto(window, cx))
            .child(self.render_hyperflux_battery(window, cx));
        let right = self.render_hyperflux_pairing(cx);
        v_flex()
            .w_full()
            // Current BodyWrapper keeps the common min-width/padding for 207.
            .min_w(surface::css(600.))
            .pt(surface::css(10.))
            .px(surface::css(20.))
            .pb(surface::css(20.))
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .text_color(Colors::text())
            .child(artwork)
            .child(
                surface::page_columns()
                    .child(surface::page_column(left))
                    .child(surface::page_column(right)),
            )
            .child(self.render_hyperflux_pairing_modal(window, cx))
            .into_any_element()
    }
}
