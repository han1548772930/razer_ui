//! Current seven-product trigger panels, retained by `gamepad-triggers` in
//! gamepad-main-audit.json. The radio, reset and slider keep the source's
//! optimistic draft change; service acknowledgement is owned separately.
use super::*;
use gpui_kit::base::Radio;
use gpui_kit::base::motion::{self, Easing, Transition};
use razer_widgets::{source_slider::SourceSlider, theme::GamepadTriggerColors as Colors};
use std::time::Duration;

#[derive(IntoElement)]
struct TriggerModeRadio {
    path: String,
    value: i64,
    label: String,
    checked: bool,
    owner: WeakEntity<GamepadProductWorkspace>,
}

impl RenderOnce for TriggerModeRadio {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = SharedString::from(format!("gamepad-trigger-mode-{}-{}", self.path, self.value));
        let selected = motion::transition(
            id.clone(),
            if self.checked { 1_f32 } else { 0. },
            Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
            window,
            cx,
        );
        // Current `[type=radio]+label`: 20px ring, 10px selected disc and
        // padding-left:30px. The caller's `.radioItem` removes the usual margin.
        Radio::new(id)
            .checked(self.checked)
            .accessibility_label(self.label.clone())
            .flex()
            .items_center()
            .text_size(surface::css(14.))
            .line_height(surface::css(20.))
            .text_color(Colors::text())
            .child(
                div()
                    .size(surface::css(20.))
                    .flex_shrink_0()
                    .rounded_full()
                    .border_1()
                    .border_color(Colors::radio_border())
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .size(surface::css(10. * selected))
                            .rounded_full()
                            .bg(Colors::accent())
                            .opacity(selected),
                    ),
            )
            .child(
                div()
                    .ml(surface::css(10.))
                    .flex_shrink_0()
                    .whitespace_nowrap()
                    .child(self.label),
            )
            .on_change(move |_, _, _, cx| {
                let _ = self.owner.update(cx, |owner, cx| {
                    owner.write(&self.path, json!(self.value), cx);
                });
            })
    }
}

impl GamepadProductWorkspace {
    pub(super) fn triggers(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .child(self.trigger_visualization(window))
            .child(div().h(surface::css(17.)))
            .child(
                surface::page_columns()
                    .child(surface::page_column(self.trigger_panel(
                        "leftTrigger",
                        "LEFT",
                        window,
                        cx,
                    )))
                    .child(surface::page_column(self.trigger_panel(
                        "rightTrigger",
                        "RIGHT",
                        window,
                        cx,
                    ))),
            )
            .into_any_element()
    }

    fn trigger_panel(
        &self,
        side: &str,
        prefix: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (mode_key, range_key, point_key) = trigger_keys(prefix);
        let root = format!("/controller/{side}");
        let analog = self.number(&format!("{root}/mode")) == self.spec.analog_mode;
        let fields: &[&str] = if analog {
            &["startRange", "endRange"]
        } else {
            &["actuationPoint", "isRapidTrigger"]
        };
        let changed = fields.iter().any(|field| {
            self.draft.pointer(&format!("{root}/{field}")) != self.spec.trigger_reset.get(field)
        });
        let mut panel = surface::panel_with_help(
            t(mode_key),
            surface::help_control(
                SharedString::from(format!("gamepad-trigger-help-{side}")),
                t("ACTUATION_TRIGGER_MODE_DESC"),
            ),
            cx,
        )
        .child(
            v_flex().children(
                [
                    (self.spec.analog_mode, "ANALOG"),
                    (self.spec.digital_mode, "DIGITAL"),
                ]
                .into_iter()
                .map(|(value, label)| {
                    v_flex()
                        .child(TriggerModeRadio {
                            path: format!("{root}/mode"),
                            value,
                            label: t(label),
                            checked: self.number(&format!("{root}/mode")) == value,
                            owner: cx.entity().downgrade(),
                        })
                        // The grid's `<br>` has the inherited one-line box.
                        .child(div().child("\u{00a0}"))
                }),
            ),
        );
        let reset_root = root.clone();
        let reset = BaseButton::new(SharedString::from(format!("gamepad-reset-{side}")))
            .accessibility_label(t("RESET"))
            .disabled(!changed)
            .text_color(Colors::text())
            .when(!changed, |button| button.opacity(0.3))
            .hover(|style| style.text_color(Colors::accent()))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.reset_trigger(&reset_root, window, cx);
            }))
            // The current empty JSX reset element draws only its 16px mask.
            .child(
                razer_assets::resource_svg()
                    .path(SharedString::from(format!(
                        "synapse/gamepad-trigger-reset/{}.svg",
                        self.spec.product_id
                    )))
                    .size(surface::css(16.))
                    .mr(surface::css(4.)),
            );
        let mut branch = v_flex().mt(surface::css(10.)).child(
            div()
                .relative()
                .text_color(Colors::text())
                .child(t(if analog { range_key } else { point_key }))
                // The widget content ends 40px from its edge; reset is right:35px.
                .child(
                    div()
                        .absolute()
                        .right(surface::css(-5.))
                        .top_0()
                        .child(reset),
                ),
        );
        if analog {
            branch = branch.child(div().mt(surface::css(35.)).child(self.range_control(
                &root,
                self.number(&format!("{root}/startRange")),
                self.number(&format!("{root}/endRange")),
                cx,
            )));
        } else {
            let path = format!("{root}/actuationPoint");
            let point = self.number(&path);
            let slider = self
                .sliders
                .get(&path)
                .expect("source digital trigger slider");
            let rapid_path = format!("{root}/isRapidTrigger");
            let checked = self.checked(&rapid_path);
            let checkbox = surface::check_item(
                SharedString::from(format!("gamepad-rapid-trigger-{side}")),
                t("RAPID_TRIGGER"),
                checked,
                false,
                window,
                cx,
            )
            .mt(surface::css(2.))
            .mb_0()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.write(&rapid_path, json!(!checked), cx);
            }));
            branch = branch.child(
                v_flex()
                    .mt(surface::css(20.))
                    .child(
                        SourceSlider::new(slider, (point as f32 - 1.) / 99.)
                            .tip(Some(format!("{point}%")))
                            .range_labels("1%", "100%", Colors::text()),
                    )
                    .child(div().child("\u{00a0}"))
                    .child(
                        h_flex()
                            .mt(surface::css(10.))
                            .items_center()
                            .child(checkbox)
                            .child(div().ml(surface::css(5.)).child(
                                surface::gamepad_trigger_help_control(
                                    SharedString::from(format!(
                                        "gamepad-rapid-trigger-help-{side}"
                                    )),
                                    t("ACTUATION_TRIGGER_MODE_TIP"),
                                ),
                            )),
                    ),
            );
        }
        panel = panel.child(branch);
        panel.into_any_element()
    }
}
