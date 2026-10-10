//! Current PID 70 surface calibration page (nR/mS/LS).
//! Only calibration profiles are rendered; DEVICE_SUPPORTED_MATS is the add
//! dialog catalog, not a list of saved calibrations. Edits are local drafts.
use super::*;
use razer_widgets::source_slider::SourceSlider;

impl MouseProductWorkspace {
    pub(super) fn calibration_70(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let default = [json!({"surfaceId":"default","type":"default"})];
        let profiles = self
            .draft
            .pointer("/calibration/profiles")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&default);
        let selected = self.draft.pointer("/calibration/selectedProfile/guid");
        let mut order: Vec<usize> = (0..profiles.len()).collect();
        order.sort_by(|a, b| {
            let a_default = profiles[*a]["type"] == "default";
            let b_default = profiles[*b]["type"] == "default";
            b_default.cmp(&a_default).then_with(|| {
                profiles[*a]["calibrationDate"]
                    .as_str()
                    .unwrap_or_default()
                    .cmp(profiles[*b]["calibrationDate"].as_str().unwrap_or_default())
            })
        });
        let surfaces = h_flex()
            .flex_wrap()
            .items_start()
            .gap(surface::css(20.))
            .children(order.into_iter().map(|index| {
                let profile = &profiles[index];
                let guid = profile.get("guid").cloned();
                let is_default = profile["type"] == "default";
                let active = selected == guid.as_ref();
                let name = if is_default {
                    t("DEFAULT").to_string()
                } else {
                    profile["name"].as_str().unwrap_or_default().to_owned()
                };
                div()
                    .id(SharedString::from(format!(
                        "mouse-70-calibration-profile-{index}"
                    )))
                    .relative()
                    .w(surface::css(290.))
                    .h(surface::css(200.))
                    .px(surface::css(20.))
                    .py(surface::css(8.))
                    .rounded(surface::css(5.))
                    .bg(rgb(0x111111))
                    .child(
                        div()
                            .absolute()
                            .top(surface::css(10.))
                            .left(surface::css(10.))
                            .size(surface::css(20.))
                            .rounded(surface::css(10.))
                            .bg(if active {
                                rgb(0x44d62c).into()
                            } else {
                                rgba(0).into()
                            }),
                    )
                    .child(div().h(surface::css(140.)))
                    .child(
                        div()
                            .text_center()
                            .text_size(surface::css(14.))
                            .line_height(surface::css(16.))
                            .child(name),
                    )
                    .children(is_default.then(|| {
                        div()
                            .text_center()
                            .text_size(surface::css(12.))
                            .text_color(rgb(0x707070))
                            .child(t("NO_CALIBRATION"))
                    }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(guid) = &guid {
                            this.write("/calibration/selectedProfile", json!({"guid":guid}), cx);
                        }
                    }))
            }));
        let mut page = h_flex()
            .w_full()
            .items_start()
            .gap(surface::css(20.))
            .child(surfaces);
        // Source nR renders LS only after a second calibration profile exists.
        if profiles.len() > 1 {
            let is_default = profiles
                .iter()
                .find(|profile| profile.get("guid") == selected)
                .is_none_or(|profile| profile["type"] == "default");
            let value = self.number("/calibration/liftOffRangeValue");
            page = page.child(
                div()
                    .w(surface::css(290.))
                    .p(surface::css(20.))
                    .bg(rgb(0x111111))
                    .rounded(surface::css(5.))
                    .child(
                        div()
                            .text_color(rgb(0x44d62c))
                            .text_size(surface::css(16.))
                            .mb(surface::css(20.))
                            .child(t("LIFT_OFF_RANGE_HEADER")),
                    )
                    .child(div().mb(surface::css(10.)).child(t("LIFT_OFF_RANGE_DESC")))
                    .child(
                        div()
                            .relative()
                            .w(surface::css(250.))
                            .h(surface::css(64.))
                            .mb(surface::css(20.))
                            .child(
                                SourceSlider::new(
                                    &self.sliders["/calibration/liftOffRangeValue"],
                                    (value - 1.) / 9.,
                                )
                                .enabled(!is_default)
                                .tip(Some(format!("{value:.0}"))),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .bottom_0()
                                    .w_full()
                                    .child(surface::slider_tags("1", None, "10", None)),
                            ),
                    )
                    .child(
                        div()
                            .text_size(surface::css(16.))
                            .mb(surface::css(10.))
                            .child(t("RESET")),
                    )
                    .child(t("RESET_DESC1")),
            );
        }
        page.into_any_element()
    }
}
