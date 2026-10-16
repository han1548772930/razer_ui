//! 1383 `Kg`: a retained local media draft with source Reset/Cancel/Apply.
use super::*;
use gpui_kit::base::Radio;
use gpui_kit::base::motion::{self, Easing, Transition};
use razer_widgets::scroll::SourceScrollable as _;
use std::time::Duration;

pub(super) struct MediaEditor {
    owner: WeakEntity<AudioProductWorkspace>,
    draft: Value,
    dialog: dialog::DialogState,
}
impl MediaEditor {
    pub(super) fn open(
        owner: WeakEntity<AudioProductWorkspace>,
        draft: Value,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        let dialog = dialog::DialogState::new(window, cx);
        cx.new(|_| Self {
            owner,
            draft,
            dialog,
        })
    }
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dialog.close(window, cx);
        cx.notify();
    }
    fn toggle_info(&mut self, cx: &mut Context<Self>) {
        let info = self.draft["info"]["enabled"] == true;
        self.draft["info"]["enabled"] = json!(!info);
        if info {
            self.draft["visualizer"]["enabled"] = json!(true);
        }
        cx.notify();
    }
    fn toggle_visualizer(&mut self, cx: &mut Context<Self>) {
        let enabled = self.draft["visualizer"]["enabled"] == true;
        self.draft["visualizer"]["enabled"] = json!(!enabled);
        if enabled {
            self.draft["info"]["enabled"] = json!(true);
        }
        cx.notify();
    }
    fn info_radio(
        &self,
        id: &'static str,
        symbol: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = self.draft["info"]["selected"] == id;
        let disabled = self.draft["info"]["enabled"] != true;
        let progress = motion::transition(
            (id, "1383-radio-checked"),
            if selected { 1_f32 } else { 0. },
            Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
            window,
            cx,
        );
        let owner = cx.entity().downgrade();
        Radio::new(id)
            .accessibility_label(label(symbol))
            .checked(selected)
            .disabled(disabled)
            .flex()
            .items_center()
            .line_height(surface::css(20.))
            .text_size(surface::css(14.))
            .text_color(Colors::text())
            .when(disabled, |radio| radio.opacity(0.3))
            .child(
                div()
                    .size(surface::css(20.))
                    .border_1()
                    .border_color(Colors::radio_border())
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .size(surface::css(10. * progress))
                            .rounded_full()
                            .bg(Colors::selected())
                            .opacity(progress),
                    ),
            )
            .child(div().ml(surface::css(10.)).child(label(symbol)))
            .on_change(move |_, _, _, cx| {
                let _ = owner.update(cx, |this, cx| {
                    this.draft["info"]["selected"] = json!(id);
                    cx.notify();
                });
            })
            .into_any_element()
    }
}
impl Render for MediaEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.dialog.is_open() {
            return div().into_any_element();
        }
        let info = self.draft["info"]["enabled"] == true;
        let visualizer = self.draft["visualizer"]["enabled"] == true;
        let selected = self.draft["visualizer"]["selected"].as_u64().unwrap_or(0) as usize;
        let preview = v_flex()
            .w(surface::css(236.))
            .child(
                div()
                    .h(surface::css(19.))
                    .p(surface::css(2.))
                    .child(label("$yX").to_uppercase()),
            )
            .child(
                v_flex()
                    .w(surface::css(234.))
                    .h(surface::css(66.))
                    .m(surface::css(1.))
                    .border_1()
                    .border_color(Colors::border())
                    .bg(Colors::black())
                    .justify_center()
                    .child(media_preview(&self.draft)),
            );
        let info_block = v_flex()
            .mt(surface::css(20.))
            .mb(surface::css(40.))
            .gap(surface::css(10.))
            .child(
                h_flex()
                    .gap(surface::css(10.))
                    .child(label("VKN").to_uppercase())
                    .child(
                        surface::SynapseSwitch::new("1383-media-info")
                            .accessibility_label(label("VKN"))
                            .checked(info)
                            .on_change(cx.listener(|this, _: &bool, _, cx| this.toggle_info(cx))),
                    ),
            )
            .child(label("CMJ"))
            .child(
                h_flex()
                    .gap(surface::css(40.))
                    .child(self.info_radio("top", "wv5", window, cx))
                    .child(self.info_radio("bottom", "_NK", window, cx)),
            );
        let visualizers = div().grid().grid_cols(3).gap(surface::css(5.)).children(
            spec().visualizers.iter().enumerate().map(|(ix, asset)| {
                BaseButton::new(SharedString::from(format!("1383-media-visualizer-{ix}")))
                    .accessibility_label(format!("{} {}", label("RYc"), ix + 1))
                    .disabled(!visualizer)
                    .styles(|s| s.disabled(|s| s.opacity(1.)))
                    .relative()
                    .flex_shrink_0()
                    .w(surface::css(236.))
                    .h(surface::css(48.))
                    .border_1()
                    .border_color(Colors::border())
                    .p(surface::css(1.))
                    .when(selected == ix, |el| {
                        el.border_2().p_0().border_color(if visualizer {
                            Colors::selected()
                        } else {
                            Colors::disabled_visualizer()
                        })
                    })
                    .child(preview_image(asset, 44.))
                    .when(!visualizer, |el| {
                        el.child(
                            div()
                                .absolute()
                                .left_0()
                                .top_0()
                                .w(surface::css(232.))
                                .h(surface::css(44.))
                                .bg(Colors::border())
                                .opacity(0.3),
                        )
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.draft["visualizer"]["selected"] = json!(ix);
                        cx.notify();
                    }))
            }),
        );
        let visualizer_block = v_flex()
            .mt(surface::css(20.))
            .mb(surface::css(40.))
            .gap(surface::css(10.))
            .child(
                h_flex()
                    .gap(surface::css(10.))
                    .child(label("RYc").to_uppercase())
                    .child(
                        surface::SynapseSwitch::new("1383-media-visualizer")
                            .accessibility_label(label("RYc"))
                            .checked(visualizer)
                            .on_change(
                                cx.listener(|this, _: &bool, _, cx| this.toggle_visualizer(cx)),
                            ),
                    ),
            )
            .child(label("GOv"))
            .child(visualizers);
        let buttons = h_flex()
            .absolute()
            .bottom(surface::css(35.))
            .left(relative(0.5))
            .ml(surface::css(-95.))
            .w(surface::css(190.))
            .gap(surface::css(10.))
            .child(
                dialog::action("1383-media-cancel", label("bOp"), false)
                    .on_click(cx.listener(|this, _, window, cx| this.close(window, cx))),
            )
            .child(
                dialog::action("1383-media-apply", label("pJk"), true).on_click(cx.listener(
                    |this, _, window, cx| {
                        let value = this.draft.clone();
                        let _ = this.owner.update(cx, |owner, cx| {
                            owner.draft["oledHome"]["media"] = value;
                            cx.emit(AudioProductChanged);
                            cx.notify();
                        });
                        this.close(window, cx);
                    },
                )),
            );
        let body = v_flex()
            .id("1383-media-scroll")
            .min_h_0()
            .flex_1()
            .pt(surface::css(20.))
            .pl(surface::css(25.))
            .pr(surface::css(25.))
            .pb(surface::css(97.))
            .scrollable_y()
            .child(
                h_flex()
                    .items_end()
                    .gap(surface::css(20.))
                    .child(div().flex_1())
                    .child(preview)
                    .child(
                        div().flex_1().child(
                            BaseButton::new("1383-media-reset")
                                .accessibility_label(label("VLI"))
                                .text_color(Colors::text())
                                .underline()
                                .hover(|s| s.text_color(Colors::selected()))
                                .child(label("VLI"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.draft = spec().media.clone();
                                    cx.notify();
                                })),
                        ),
                    ),
            )
            .child(info_block)
            .child(visualizer_block)
            .into_any_element();
        self.dialog.render(
            "1383-media",
            label("wLL"),
            body,
            buttons.into_any_element(),
            window,
            cx,
            Self::close,
        )
    }
}
