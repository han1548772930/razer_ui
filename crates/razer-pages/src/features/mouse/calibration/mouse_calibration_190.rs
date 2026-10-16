//! Current 190 mS/US/GS caller, main.81b09779.js offset 4621310.
use super::*;
use gpui_kit::base::Button as BaseButton;
use razer_widgets::source_slider::SourceSlider;
use razer_widgets::theme::DrawerColors;

pub(super) fn supports_smart_only_calibration(product_id: u32) -> bool {
    static PRODUCTS: OnceLock<Vec<u32>> = OnceLock::new();
    PRODUCTS
        .get_or_init(|| crate::data::mouse_smart_calibration_family_data!())
        .contains(&product_id)
}
pub(super) fn smart_calibration_commits_on_release(product_id: u32) -> bool {
    (supports_smart_only_calibration(product_id) && product_id != 196)
        || calibration_preset::supports(product_id)
        || product_id == 162
}
pub(super) fn smart_calibration_cancels_on_value_change(product_id: u32) -> bool {
    supports_smart_only_calibration(product_id) && !matches!(product_id, 196 | 229 | 45066)
}

impl MouseProductWorkspace {
    pub(super) fn cancel_smart_calibration_drag(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.smart_calibration_drag.take() else {
            return;
        };
        self.smart_calibration_drag_cancelled = false;
        self.smart_calibration_preview.remove(&path);
        let value = if path == "smart-preset" {
            self.smart_preset() as f32
        } else {
            self.number(&path)
        };
        if let Some(slider) = self.sliders.get(&path) {
            let was_syncing = self.syncing;
            self.syncing = true;
            slider.update(cx, |slider, cx| slider.set_value(value, window, cx));
            self.syncing = was_syncing;
        }
    }

    pub(super) fn finish_smart_calibration_drag(
        &mut self,
        path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.smart_calibration_drag.as_deref() != Some(path) {
            return;
        }
        self.smart_calibration_drag = None;
        if self.smart_calibration_drag_cancelled {
            self.smart_calibration_drag_cancelled = false;
            self.smart_calibration_preview.remove(path);
            cx.notify();
            return;
        }
        let value = self
            .smart_calibration_preview
            .remove(path)
            .unwrap_or_else(|| self.sliders[path].read(cx).value().start());
        // Current module130 calls changeValue once on mouseup even if the
        // pointer did not move. Draft/device submission occurs here, not on drag.
        if path == "smart-preset" {
            let (landing, lift) = match value.round() as i32 {
                1 => (1, 2),
                2 => (1, 3),
                _ => (2, 3),
            };
            let mut tracking = self.draft["smartTracking"].clone();
            tracking["landingDistance"] = json!(landing);
            tracking["liftOffDistance"] = json!(lift);
            self.set_preset_smart_tracking(tracking, cx);
        } else {
            self.write_number(path, value, window, cx);
        }
    }
    pub(super) fn power_190(&self, cx: &Context<Self>) -> AnyElement {
        self.source_page_columns(
            self.source_widget_column()
                .child(self.power_saving_card(cx).mb_0()),
            self.source_widget_column()
                .child(self.low_power_card(cx).mb_0()),
        )
    }

    pub(super) fn calibration_190(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.spec.product_id == 196 {
            return self.calibration_radio_196(cx);
        }
        let asymmetric = self.boolean("/smartTracking/isAsymmetric");
        let mut panel = surface::panel(t("SMARTTRACKING"), cx)
            .child(div().mb(surface::css(20.)).child(t("SMART_TRACKING_DISC")))
            .child(
                h_flex()
                    .items_start()
                    .child(
                        surface::check_item(
                            "mouse-190-calibration-asymmetric",
                            t("ENABLEASYMMETRICCUTOFF"),
                            asymmetric,
                            false,
                            window,
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.write("/smartTracking/isAsymmetric", json!(!asymmetric), cx);
                        })),
                    )
                    .child(div().ml(surface::css(5.)).child(surface::help_control(
                        "mouse-190-calibration-asymmetric-help",
                        t("SMARTTRACKINGTOOLTIP1"),
                    ))),
            );
        let mut controls = div().mb(surface::css(10.));
        if asymmetric {
            controls = controls
                .child(self.calibration_190_slider(
                    "/smartTracking/liftOffDistance",
                    "LIFTOFFDISTANCE",
                    false,
                    cx,
                ))
                .child(self.calibration_190_slider(
                    "/smartTracking/landingDistance",
                    "LANDINGDISTANCE",
                    false,
                    cx,
                ))
                .child(
                    div()
                        .mt(surface::css(10.))
                        .mb(surface::css(20.))
                        .text_color(rgb(0x999999))
                        .child(t("WARNING_SETTING_LANDING_DISTANCE")),
                );
        } else {
            controls = controls.child(self.calibration_190_slider(
                "/smartTracking/trackingDistance",
                "TRACKINGDISTANCE",
                true,
                cx,
            ));
        }
        panel = panel
            .child(controls)
            .child(
                div()
                    .mt(surface::css(20.))
                    .mb(surface::css(10.))
                    .child(t("RESET").to_uppercase()),
            )
            .child(div().mb(surface::css(10.)).child(t("RESET_DESC1")));
        // mS mounts Ha directly under Ur: this panel is centered by Ha's
        // margin:auto; wrapping it in a left FO would shift it by 310px.
        v_flex()
            .w_full()
            .children(
                self.calibration_state
                    .welcome
                    .then(|| self.smart_calibration_welcome(cx)),
            )
            .child(surface::page_columns().justify_center().child(panel))
            .into_any_element()
    }

    fn calibration_radio_196(&self, cx: &Context<Self>) -> AnyElement {
        let selected = self.number("/smartTracking/trackingDistance") as i32;
        let choices = [
            (1, "CALIBRATION_LOW", "1mm"),
            (2, "CALIBRATION_HIGH", "2mm"),
        ]
        .map(|(value, label, id)| {
            gpui_kit::component::radio::Radio::new(id)
                .label(t(label))
                .checked(selected == value)
                .on_click(cx.listener(move |this, _, _, cx| {
                    if this.number("/smartTracking/trackingDistance") as i32 == value {
                        return;
                    }
                    this.write("/smartTracking/trackingDistance", json!(value), cx);
                }))
        });
        let panel = surface::panel(t("SMARTTRACKING"), cx)
            .child(
                div()
                    .mb(surface::css(20.))
                    .child(t("SMARTTRACKINGMESSAGEINFO")),
            )
            .child(
                div()
                    .mt(surface::css(20.))
                    .mb(surface::css(20.))
                    .child(t("TRACKINGDISTANCE").to_uppercase()),
            )
            .child(h_flex().ml(surface::css(-20.)).children(choices));
        v_flex()
            .w_full()
            .children(
                self.calibration_state
                    .welcome
                    .then(|| self.smart_calibration_welcome(cx)),
            )
            .child(surface::page_columns().justify_center().child(panel))
            .into_any_element()
    }

    fn smart_calibration_welcome(&self, cx: &Context<Self>) -> AnyElement {
        let radio = self.spec.product_id == 196;
        h_flex()
            .justify_center()
            .mb(surface::css(if radio { 10. } else { 20. }))
            .child(
                v_flex()
                    .relative()
                    .px(surface::css(30.))
                    .py(surface::css(20.))
                    .when(radio, |welcome| {
                        welcome
                            .w_full()
                            .mt(surface::css(10.))
                            .px(surface::css(100.))
                    })
                    .rounded(surface::css(5.))
                    .bg(DrawerColors::new().surface())
                    .border_1()
                    .border_color(DrawerColors::new().surface())
                    .text_center()
                    .child(
                        div()
                            .font_family("RazerF5")
                            .text_size(surface::css(26.))
                            .line_height(surface::css(30.))
                            .mb(surface::css(10.))
                            .text_color(cx.theme().primary)
                            .child(t("MOUSE_MAT_CALIBRATION_HEADER").to_uppercase()),
                    )
                    .child(
                        div()
                            .text_size(surface::css(14.))
                            .line_height(surface::css(16.))
                            .child(t(if radio {
                                "MOUSE_MAT_CALIBRATION_DESC1"
                            } else {
                                "MOUSE_MAT_CALIBRATION"
                            })),
                    )
                    .child(
                        BaseButton::new("mouse-smart-calibration-welcome-close")
                            .group("mouse-smart-calibration-welcome-close")
                            .accessibility_label(t("CLOSE"))
                            .absolute()
                            .top_0()
                            .right_0()
                            .size(surface::css(36.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .active(|style| style.opacity(0.7))
                            .child(
                                img("synapse/mouse-calibration-70/icon_close_white.8ab462b8.svg")
                                    .size(surface::css(20.))
                                    .group_hover(
                                        "mouse-smart-calibration-welcome-close",
                                        |style| style.opacity(0.),
                                    ),
                            )
                            .child(
                                img("synapse/mouse-calibration-70/icon_close_green.45f61360.svg")
                                    .absolute()
                                    .size(surface::css(20.))
                                    .opacity(0.)
                                    .group_hover(
                                        "mouse-smart-calibration-welcome-close",
                                        |style| style.opacity(1.),
                                    ),
                            )
                            .on_click(
                                cx.listener(|this, _, _, cx| this.close_calibration_welcome(cx)),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn calibration_190_slider(
        &self,
        path: &str,
        label: &str,
        no_tip: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        div()
            .child(
                div()
                    .mt(surface::css(20.))
                    .mb(surface::css(10.))
                    .child(t(label).to_uppercase()),
            )
            .child(self.slider_190(
                path,
                &t("LOW"),
                no_tip.then(|| t("MEDIUM")).as_deref(),
                &t("HIGH"),
                no_tip,
                true,
                cx,
            ))
            .into_any_element()
    }

    pub(super) fn slider_190(
        &self,
        path: &str,
        min_tag: &str,
        mid_tag: Option<&str>,
        max_tag: &str,
        no_tip: bool,
        enabled: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let slider = &self.sliders[path];
        let min = slider.read(cx).min_value();
        let max = slider.read(cx).max_value();
        let value = self
            .smart_calibration_preview
            .get(path)
            .copied()
            .unwrap_or_else(|| {
                if path == "smart-preset" {
                    self.smart_preset() as f32
                } else {
                    self.number(path)
                }
            });
        let press_path = path.to_owned();
        let release_path = path.to_owned();
        let outside_path = path.to_owned();
        div()
            .when(
                enabled
                    && smart_calibration_commits_on_release(self.spec.product_id)
                    && (path.starts_with("/smartTracking/")
                        || path.starts_with("/presetTracking/")
                        || (matches!(self.spec.product_id, 162 | 185)
                            && path == "/calibration/liftOffRangeValue")
                        || path == "smart-preset"),
                |slider| {
                    slider
                        .capture_key_down(cx.listener(|_, event: &KeyDownEvent, window, cx| {
                            if matches!(
                                event.keystroke.key.as_str(),
                                "home" | "end" | "left" | "right" | "up" | "down"
                            ) {
                                window.prevent_default();
                                cx.stop_propagation();
                            }
                        }))
                        .capture_any_mouse_down(cx.listener(
                            move |this, event: &MouseDownEvent, _, cx| {
                                if event.button == MouseButton::Left {
                                    this.smart_calibration_drag = Some(press_path.clone());
                                    this.smart_calibration_drag_cancelled = false;
                                    cx.notify();
                                }
                            },
                        ))
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |this, _, window, cx| {
                                this.finish_smart_calibration_drag(&release_path, window, cx)
                            }),
                        )
                        .on_mouse_up_out(
                            MouseButton::Left,
                            cx.listener(move |this, _, window, cx| {
                                this.finish_smart_calibration_drag(&outside_path, window, cx)
                            }),
                        )
                },
            )
            .relative()
            .w_full()
            .h(surface::css(if no_tip { 36. } else { 64. }))
            .child(
                SourceSlider::new(slider, (value - min) / (max - min))
                    .enabled(enabled)
                    .tip((!no_tip).then(|| format!("{value:.0}"))),
            )
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .w_full()
                    .opacity(if enabled { 1. } else { 0.3 })
                    .child(surface::slider_tags(min_tag, mid_tag, max_tag, None)),
            )
            .into_any_element()
    }
}
