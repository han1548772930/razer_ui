//! Device activation-code reading and copying from the retained Sound page.
use super::*;
use razer_device::earbuds_auto_pause::ActivationCode;
use razer_widgets::{keyboard_navigation::Button, theme::AudioEnhancementColors as C};
#[derive(Default)]
pub(super) struct State {
    pub(super) code: Option<Result<ActivationCode, String>>,
    pub(super) loading: bool,
    open: bool,
    copied: bool,
}

impl State {
    pub(super) fn clear_device(&mut self) {
        self.code = None;
        self.loading = false;
        self.open = false;
        self.copied = false;
    }
    pub(super) fn cancel(&mut self) {
        self.open = false;
        self.copied = false;
    }
}
impl AudioProductWorkspace {
    fn open_earbuds_activation(&mut self, cx: &mut Context<Self>) {
        if !self.earbuds_sound.active() {
            return;
        }
        self.earbuds_activation.open = true;
        self.earbuds_activation.copied = false;
        self.earbuds_activation.loading = true;
        if self
            .earbuds_activation
            .code
            .as_ref()
            .is_some_and(Result::is_err)
        {
            self.earbuds_activation.code = None;
        }
        self.request_earbuds_sound_action(super::earbuds_sound_actions::Action::ActivationCode, cx);
        cx.notify();
    }

    pub(super) fn earbuds_activation_card(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut actions = h_flex()
            .mt(surface::css(20.))
            .justify_between()
            .items_center();
        actions = actions.child(
            Button::new("earbuds-activation-show")
                .flex()
                .items_center()
                .gap(surface::css(4.))
                .disabled(!self.earbuds_sound.active())
                .text_size(surface::css(14.))
                .underline()
                .text_color(C::text())
                .hover(|style| style.text_color(C::selected()))
                .child(
                    razer_assets::resource_svg()
                        .path("synapse/earbuds-activation-eye.svg")
                        .size(surface::css(20.)),
                )
                .child(t("SHOW_ACTIVATION_CODE"))
                .on_click(cx.listener(|this, _, _, cx| this.open_earbuds_activation(cx))),
        );
        surface::panel(t("THX_SPATIAL_AUDIO"), cx)
            .id("earbuds-activation-card")
            .test_support()
            .relative()
            .child(t("THX_SPATIAL_AUDIO_DESC"))
            .child(actions)
            .when(self.earbuds_activation.open, |card| {
                card.child(self.earbuds_activation_modal(cx))
            })
            .into_any_element()
    }

    fn earbuds_activation_modal(&self, cx: &mut Context<Self>) -> AnyElement {
        let code = self
            .earbuds_activation
            .code
            .as_ref()
            .and_then(|value| value.as_ref().ok());
        let value = code.map(|code| code.value().to_string());
        let copy_value = value.clone();
        let content = h_flex()
            .gap(surface::css(20.))
            .justify_center()
            .p(surface::css(20.))
            .child(
                div()
                    .font_family("RazerF5")
                    .text_size(surface::css(16.))
                    .child(value.map(|value| value.to_uppercase()).unwrap_or_else(|| {
                        if self.earbuds_activation.loading {
                            "..."
                        } else {
                            "—"
                        }
                        .into()
                    })),
            )
            .child(
                v_flex()
                    .relative()
                    .child(
                        Button::new("earbuds-activation-copy")
                            .flex()
                            .items_center()
                            .gap(surface::css(8.))
                            .h(surface::css(27.))
                            .disabled(code.is_none())
                            .text_size(surface::css(12.))
                            .text_color(if self.earbuds_activation.copied {
                                C::activation_copy_pressed()
                            } else {
                                C::selected()
                            })
                            .when(
                                !self.earbuds_activation.copied && code.is_some(),
                                |button| {
                                    button
                                        .hover(|style| style.text_color(C::activation_copy_hover()))
                                        .active(|style| {
                                            style.text_color(C::activation_copy_pressed())
                                        })
                                },
                            )
                            .child(
                                razer_assets::resource_svg()
                                    .path("synapse/earbuds-activation-copy.svg")
                                    .size(surface::css(20.)),
                            )
                            .child(t("COPY").to_uppercase())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(value) = &copy_value {
                                    cx.write_to_clipboard(ClipboardItem::new_string(value.clone()));
                                    this.earbuds_activation.copied = true;
                                    cx.notify();
                                }
                            })),
                    )
                    .when(self.earbuds_activation.copied, |column| {
                        column.child(
                            div()
                                .absolute()
                                .top_full()
                                .left_0()
                                .px(surface::css(10.))
                                .py(surface::css(8.))
                                .border_1()
                                .border_color(razer_widgets::theme::AudioEqualizerColors::border())
                                .bg(C::mapping_black())
                                .text_color(C::text())
                                .text_size(surface::css(14.))
                                .child(t("COPIED")),
                        )
                    }),
            );
        razer_assets::deferred_resource(
            div()
                .id("earbuds-activation-modal")
                .test_support()
                .absolute()
                .inset(surface::css(10.))
                .occlude()
                .bg(C::gesture_backdrop())
                .flex()
                .items_center()
                .justify_center()
                .child(
                    v_flex()
                        .w(surface::css(400.))
                        .max_w_full()
                        .bg(C::background())
                        .border_1()
                        .border_color(C::selected())
                        .rounded(surface::css(3.))
                        .child(
                            div()
                                .pt(surface::css(20.))
                                .px(surface::css(20.))
                                .text_center()
                                .font_family("RazerF5")
                                .text_size(surface::css(16.))
                                .text_color(C::selected())
                                .child(t("ACTIVATION_CODE_FOR_THX_SPATIAL_AUDIO").to_uppercase()),
                        )
                        .child(content)
                        .children(
                            self.earbuds_activation
                                .code
                                .as_ref()
                                .and_then(|result| result.as_ref().err())
                                .map(|error| {
                                    div()
                                        .id("earbuds-activation-error")
                                        .test_support()
                                        .px(surface::css(20.))
                                        .pb(surface::css(10.))
                                        .text_size(surface::css(12.))
                                        .text_color(cx.theme().danger)
                                        .child(error.clone())
                                }),
                        )
                        .child(
                            h_flex().justify_center().pb(surface::css(25.)).child(
                                Button::new("earbuds-activation-close")
                                    .h(surface::css(27.))
                                    .min_w(surface::css(90.))
                                    .bg(C::mapping_cancel())
                                    .hover(|style| style.bg(C::activation_close_hover()))
                                    .active(|style| style.bg(C::activation_close_pressed()))
                                    .text_color(C::mapping_white())
                                    .text_size(surface::css(12.))
                                    .child(t("CLOSE").to_uppercase())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.cancel_earbuds_activation();
                                        cx.notify();
                                    })),
                            ),
                        ),
                ),
        )
        .with_priority(1001)
        .into_any_element()
    }
}
