//! Each page, functional caller, setter and original mat is audited separately
//! in mouse-preset-calibration-current-source.json. PID 185 stays independent.
use super::*;
use gpui_kit::base::Button as BaseButton;

#[derive(Deserialize)]
pub(super) struct Spec {
    product_id: u32,
    radio: bool,
    pub(super) initial_state: Value,
}
pub(super) fn source_spec(pid: u32) -> Option<&'static Spec> {
    static SPECS: OnceLock<Vec<Spec>> = OnceLock::new();
    SPECS
        .get_or_init(|| read_mouse_preset_calibration_catalog())
        .iter()
        .find(|spec| spec.product_id == pid)
}
pub(super) fn supports(pid: u32) -> bool {
    source_spec(pid).is_some()
}

impl MouseProductWorkspace {
    pub(super) fn calibration_preset(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let spec = source_spec(self.spec.product_id).expect("mounted Smart/Preset page");
        let manual = self.calibration_state.calibration_smart["calibrationTrackingMode"] == true;
        let mode = BaseButton::new("mouse-preset-calibration-mode")
            .role(Role::Switch)
            .selected(manual)
            .accessibility_label(t(if spec.radio {
                "PRESETCALIBRATION"
            } else {
                "MANUALCALIBRATION"
            }))
            .rounded(surface::css(18.))
            .border_1()
            .border_color(cx.theme().border)
            .h(surface::css(36.))
            .p(surface::css(5.))
            .mb(surface::css(20.))
            .child(h_flex().children([false, true].into_iter().map(|selected| {
                div()
                    .px(surface::css(10.))
                    .py(surface::css(5.))
                    .rounded(surface::css(13.))
                    .when(selected == manual, |item| {
                        item.bg(cx.theme().primary).text_color(rgb(0x000000))
                    })
                    .child(t(if selected {
                        if spec.radio {
                            "PRESETCALIBRATION"
                        } else {
                            "MANUALCALIBRATION"
                        }
                    } else {
                        "SMARTTRACKING"
                    }))
            })))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.toggle_preset_calibration_mode(window, cx)
            }));
        v_flex()
            .w_full()
            .children(
                self.calibration_state
                    .welcome
                    .then(|| self.calibration_welcome(cx)),
            )
            .child(h_flex().justify_center().child(mode))
            .child(if manual {
                self.calibration_surface_rows(window, cx)
            } else {
                self.preset_smart_tracking_panel(spec.radio, window, cx)
            })
            .into_any_element()
    }

    fn preset_smart_tracking_panel(
        &self,
        radio: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut panel = surface::panel(t("SMARTTRACKING"), cx)
            .child(
                div()
                    .mb(surface::css(20.))
                    .child(t("SMARTTRACKINGMESSAGEINFO")),
            )
            .child(
                div()
                    .mb(surface::css(10.))
                    .child(t("TRACKINGDISTANCE").to_uppercase()),
            );
        if radio {
            let lift = self.number("/smartTracking/liftOffDistance");
            panel = panel.child(h_flex().h(surface::css(50.)).children([2_u32,3].into_iter().map(|value| {
                gpui_kit::component::radio::Radio::new(SharedString::from(format!("mouse-185-smart-lift-{value}")))
                    .label(t(if value == 2 {"LOW"} else {"HIGH"})).checked(lift == value as f32)
                    .on_click(cx.listener(move |this, _, _, cx| {
                    // Both original radio controls invoke C without a value.
                    // Clicking the already active control also toggles the pair.
                    let lift = if this.number("/smartTracking/liftOffDistance") == 2. {3} else {2};
                    this.set_preset_smart_tracking(json!({"isAsymmetric":false,"trackingDistance":2,"liftOffDistance":lift,"landingDistance":1}), cx);
                }))
            })));
        } else {
            let asymmetric = self.boolean("/smartTracking/isAsymmetric");
            panel = panel.child(
                h_flex()
                    .items_start()
                    .child(
                        surface::check_item(
                            "mouse-preset-asymmetric",
                            t("ENABLEASYMMETRICCUTOFF"),
                            asymmetric,
                            false,
                            window,
                            cx,
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.cancel_smart_calibration_drag(window, cx);
                                let mut value = this.draft["smartTracking"].clone();
                                value["isAsymmetric"] = json!(!asymmetric);
                                this.set_preset_smart_tracking(value, cx);
                            },
                        )),
                    )
                    .child(surface::help_control(
                        "mouse-preset-asymmetric-help",
                        t("SMARTTRACKINGTOOLTIP1"),
                    )),
            );
            if asymmetric {
                panel = panel
                    .child(div().child(format!(
                        "{} {} {}",
                        t("LANDINGDISTANCE_1"),
                        self.number("/smartTracking/landingDistance"),
                        t("MM")
                    )))
                    .child(div().child(format!(
                        "{} {} {}",
                        t("LIFTOFFDISTANCE_1"),
                        self.number("/smartTracking/liftOffDistance"),
                        t("MM")
                    )))
                    .child(self.slider_190("smart-preset", "", None, "", false, true, cx));
            } else {
                panel = panel.child(self.slider_190(
                    "/smartTracking/trackingDistance",
                    &t("LOW"),
                    Some(&t("MEDIUM")),
                    &t("HIGH"),
                    true,
                    true,
                    cx,
                ));
            }
        }
        surface::page_columns()
            .justify_center()
            .child(panel)
            .into_any_element()
    }

    pub(super) fn preset_calibration_tracking_panel(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tracking = self
            .calibration_state
            .preset_tracking_ui
            .as_ref()
            .unwrap_or(&self.calibration_state.calibration_smart["presetTracking"]);
        let asymmetric = tracking["isAsymmetric"] == true;
        let disabled = !self.preset_tracking_enabled();
        let max_landing = tracking["liftOffDistance"]
            .as_u64()
            .unwrap_or(5)
            .saturating_sub(1);
        let mut panel = surface::panel_with_help(
            t("MANUALCALIBRATION"),
            surface::help_control(
                "mouse-preset-basic-calibration-help",
                t("TOOLTIP_BASIC_CALIBRATION"),
            ),
            cx,
        )
        .w(surface::css(290.))
        .flex_shrink_0()
        .child(
            div()
                .mb(surface::css(10.))
                .child(t("MANUALCALIBRATIONMESSAGE")),
        )
        .child(
            h_flex()
                .items_start()
                .child(
                    surface::check_item(
                        "mouse-preset-enable-landing",
                        t("ENABLEASYMMETRICCUTOFF"),
                        asymmetric,
                        disabled,
                        window,
                        cx,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if this.preset_tracking_enabled() {
                            this.set_preset_tracking_field(
                                "isAsymmetric",
                                json!(!asymmetric),
                                window,
                                cx,
                            );
                        }
                    })),
                )
                .child(surface::help_control(
                    "mouse-preset-landing-help",
                    t("CALIBRATIONDETAILS3"),
                )),
        );
        if asymmetric {
            panel = panel
                .child(
                    div()
                        .mb(surface::css(10.))
                        .child(t("LIFTOFFDISTANCE").to_uppercase()),
                )
                .child(self.slider_190(
                    "/presetTracking/liftOffDistance",
                    &t("LOW"),
                    None,
                    &t("HIGH"),
                    false,
                    !disabled,
                    cx,
                ))
                .child(
                    div()
                        .mb(surface::css(10.))
                        .child(t("LANDINGDISTANCE").to_uppercase()),
                )
                .child(self.slider_190(
                    "/presetTracking/landingDistance",
                    "LOW",
                    None,
                    "HIGH",
                    false,
                    !disabled && max_landing > 1,
                    cx,
                ));
        } else {
            panel = panel
                .child(
                    div()
                        .mb(surface::css(10.))
                        .child(t("SURFACEDISTANCE").to_uppercase()),
                )
                .child(self.slider_190(
                    "/presetTracking/surfaceDistance",
                    &t("LOW"),
                    None,
                    &t("HIGH"),
                    false,
                    !disabled,
                    cx,
                ));
        }
        panel
            .child(div().mb(surface::css(10.)).child(t("RESET").to_uppercase()))
            .child(t("RESET_DESC1"))
            .into_any_element()
    }
}

// Generated catalog reader: razer-pages/mouse_preset_calibration_data.
fn read_mouse_preset_calibration_catalog() -> Vec<Spec> {
    fn decode_0(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<Spec>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Spec> {
        let value = match r.tag()? {
            0 => Spec {
                product_id: r.child(decode_2)?,
                radio: r.child(decode_3)?,
                initial_state: r.child(decode_4)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<serde_json::Value> {
        let value = match r.tag()? {
            0 => r.external_value()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_MOUSE_PRESET_CALIBRATION_DATA,
        0,
        "d0b4fe3a941797ef3ec941eb26849c4628f506781fbd042cb469ff882b8c2812",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
#[test]
fn packaged_catalog_mouse_preset_calibration_data_decodes() {
    let _ = read_mouse_preset_calibration_catalog();
}
// End generated catalog reader: razer-pages/mouse_preset_calibration_data.
