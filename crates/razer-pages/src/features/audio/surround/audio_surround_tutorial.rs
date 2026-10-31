//! Current six-step muted onboarding mounted by Carol owners and Soma Surround.
use super::*;
use razer_widgets::keyboard_navigation::Button as BaseButton;
use razer_widgets::theme::HelpOwnerColors as Colors;

impl AudioProductWorkspace {
    pub(super) fn surround_tutorial_button(
        &self,
        id: &'static str,
        step: usize,
        cx: &Context<Self>,
    ) -> AnyElement {
        BaseButton::new(id)
            .accessibility_label(t("TUTORIAL"))
            .size(surface::css(if self.spec.product_id == 3942 {
                26.
            } else {
                25.
            }))
            .opacity(0.8)
            .hover(|style| style.opacity(1.))
            .child(img("synapse/audio-surround-tutorial-book.svg").size_full())
            .on_click(cx.listener(move |this, _, _, cx| {
                this.surround.tutorial = Some(step);
                cx.notify();
            }))
            .into_any_element()
    }
    fn close_surround_tutorial(&mut self, cx: &mut Context<Self>) {
        self.surround.tutorial = None;
        if matches!(self.spec.product_id, 1392 | 1442) {
            if let Some(owner) = &self.source_owner {
                owner.update(cx, |owner, cx| {
                    cx.emit(owner.request(
                        super::super::super::source_help_owner::HelpOwnerCommand::DismissTutorial,
                    ))
                });
            }
        }
        cx.notify();
    }
    pub(in crate::features) fn surround_tutorial(
        &self,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        if !matches!(self.spec.product_id, 1392 | 1442 | 3942) {
            return None;
        }
        let step = self.surround.tutorial?;
        let soma = self.spec.product_id == 3942;
        let heading = match step {
            0 if soma => "SURROUND_HEADING_1",
            0 => "INTRODUCING_RAZER_CLIO",
            1 => "AUDIO_EQUALIZER",
            2 => "COMBINE_WITH_OTHER_SPEAKERS",
            3 => "FINE_TUNE_FOR_SURROUND",
            4 => "OPEN_WINDOW_VOLUME_MIXER",
            5 => "TIME_ALIGNMENT",
            _ => return None,
        };
        let mut contents = v_flex().child(
            div()
                .text_size(surface::css(20.))
                .mb(surface::css(20.))
                .child(t(heading)),
        );
        let paragraph = |head: Option<&str>, body: &str| {
            v_flex()
                .children(head.map(|key| {
                    div()
                        .font_family("RazerF5")
                        .text_size(surface::css(14.))
                        .line_height(surface::css(14.))
                        .text_color(Colors::accent())
                        .mb(surface::css(5.))
                        .child(t(key).to_uppercase())
                }))
                .child(
                    div()
                        .mb(surface::css(20.))
                        .text_size(surface::css(14.))
                        .line_height(surface::css(17.))
                        .child(t(body)),
                )
        };
        contents = match step {
            0 => contents.child(paragraph(
                Some("CLIO_HEAD_1_1"),
                if soma { "SURROUND_BODY_1_1" } else { "CLIO_BODY_1_1" },
            )),
            1 => contents
                .child(paragraph(Some("CLIO_HEAD_2_1"), "CLIO_BODY_2_1"))
                .child(paragraph(
                    Some("CLIO_HEAD_2_2"),
                    if soma { "SURROUND_BODY_2_2" } else { "CLIO_BODY_2_2" },
                )),
            2 => contents
                .child(paragraph(
                    None,
                    if soma { "SURROUND_DESC_3" } else { "CLIO_DESC_3" },
                ))
                .child(paragraph(Some("CLIO_HEAD_3_1"), "CLIO_BODY_3_1"))
                .child(paragraph(
                    Some("CLIO_HEAD_3_2"),
                    if soma { "SURROUND_BODY_3_2" } else { "CLIO_BODY_3_2" },
                )),
            3 => contents.child(paragraph(Some("CLIO_HEAD_4_1"), "CLIO_BODY_4_1")),
            4 => contents.child(paragraph(Some("CLIO_HEAD_5_1"), "CLIO_BODY_5_1")),
            _ => contents.child(paragraph(Some("CLIO_HEAD_6_1"), "CLIO_BODY_6_1")),
        };
        let button = |id: &'static str, key: &str, primary| {
            BaseButton::new(id)
                .accessibility_label(t(key))
                .w(surface::css(100.))
                .h(surface::css(27.))
                .rounded(surface::css(3.))
                .text_size(surface::css(12.))
                .bg(if primary {
                    Colors::accent()
                } else {
                    Colors::button()
                })
                .text_color(if primary {
                    Colors::button_text()
                } else {
                    Colors::plain_button_text()
                })
                .child(t(key).to_uppercase())
        };
        let controls = v_flex()
            .child(
                h_flex()
                    .gap(surface::css(12.))
                    .mb(surface::css(16.))
                    .ml(surface::css(5.))
                    .children((0..6).map(|index| {
                        div()
                            .size(surface::css(4.))
                            .rounded_full()
                            .bg(if step == index {
                                Colors::plain_button_text()
                            } else {
                                Colors::button()
                            })
                    })),
            )
            .child(
                h_flex()
                    .gap(surface::css(10.))
                    .child(
                        button("surround-tutorial-back", "BACK", false)
                            .disabled(step == 0)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.surround.tutorial =
                                    this.surround.tutorial.map(|step| step.saturating_sub(1));
                                cx.notify();
                            })),
                    )
                    .child(
                        button(
                            "surround-tutorial-next",
                            if step == 5 { "DONE" } else { "NEXT" },
                            true,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            if this.surround.tutorial == Some(5) {
                                this.close_surround_tutorial(cx)
                            } else {
                                this.surround.tutorial =
                                    this.surround.tutorial.map(|step| step + 1);
                                cx.notify();
                            }
                        })),
                    ),
            );
        let card = h_flex()
            .relative()
            .w(surface::css(800.))
            .min_h(surface::css(420.))
            .rounded(surface::css(5.))
            .overflow_hidden()
            .border_1()
            .border_color(Colors::accent())
            .bg(Colors::background())
            .text_color(Colors::text())
            .font_family("Roboto")
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .child(
                image_cache(razer_assets::resource_image_cache())
                    .w(surface::css(320.))
                    .h(surface::css(420.))
                    .flex_shrink_0()
                    .child(
                        img(SharedString::from(format!(
                            "synapse/audio-surround-{}-tutorial-{}.webp",
                            self.spec.product_id,
                            step + 1
                        )))
                        .id(("surround-tutorial-media", step as u64))
                        .size_full()
                        .object_fit(ObjectFit::Cover),
                    ),
            )
            .child(
                v_flex()
                    .w(surface::css(480.))
                    .p(surface::css(50.))
                    .justify_between()
                    .child(contents)
                    .child(controls),
            )
            .child(
                BaseButton::new("surround-tutorial-close")
                    .accessibility_label(t("CLOSE"))
                    .absolute()
                    .top(surface::css(11.))
                    .right(surface::css(11.))
                    .size(surface::css(24.))
                    .child(img("synapse/audio-surround-tutorial-close.svg").size_full())
                    .on_click(cx.listener(|this, _, _, cx| this.close_surround_tutorial(cx))),
            );
        let host_tab = window.rem_size() / 16. * 42.;
        Some(
            razer_assets::deferred_resource(
                anchored()
                    .anchor(Anchor::TopLeft)
                    .position(point(px(0.), host_tab))
                    .child(
                        h_flex()
                            .items_start()
                            .justify_center()
                            .w(window.viewport_size().width)
                            .h(window.viewport_size().height - host_tab)
                            .bg(Colors::shade())
                            .child(div().mt(surface::css(134.)).child(card)),
                    ),
            )
            .priority(105)
            .into_any_element(),
        )
    }
}
