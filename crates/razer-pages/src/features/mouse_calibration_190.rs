//! Current 190 mS/US/GS caller, main.81b09779.js offset 4621310.
use super::*;
use razer_widgets::source_slider::SourceSlider;

impl MouseProductWorkspace {
    pub(super) fn power_190(&self, cx: &Context<Self>) -> AnyElement {
        let enabled = self.low_power_enabled();
        let saving = surface::panel_with_control(
            t("POWER_SAVING_HEADER"),
            surface::help_control("mouse-190-power-saving-help", t("POWER_SAVING_TOOLTIP")),
            cx,
        )
        .child(surface::h1_body(t("POWER_SAVING_DESC"), cx))
        .child(self.slider_190(self.spec.power_path(), "1", None, "15", false, true, cx));
        let low_power = surface::panel_with_control(
            t("LOW_POWER_MODE_HEADER"),
            surface::help_control("mouse-190-low-power-help", t("LOW_POWER_MODE_TOOLTIP")),
            cx,
        )
        .child(surface::h1_body(t("LOW_POWER_MODE_DESC"), cx))
        .child(self.slider_190("/lowPowerMode", "5%", None, "100%", false, enabled, cx))
        .children((!enabled).then(|| {
            div()
                .mt(surface::css(10.))
                .text_color(rgb(0x999999))
                .child(t("LOW_POWER_MODE_WARN"))
        }));
        surface::page_columns()
            .child(surface::page_column(saving))
            .child(surface::page_column(low_power))
            .into_any_element()
    }

    pub(super) fn calibration_190(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
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
        surface::page_columns()
            .justify_center()
            .child(panel)
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

    fn slider_190(
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
        let value = self.number(path);
        div()
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
