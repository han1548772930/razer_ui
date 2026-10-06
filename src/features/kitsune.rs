//! Current 4115 hP/PP/CP/OP. Evidence: kitsune-current-evidence.json.
use super::*;
use gpui_kit::base::motion::{self, Easing, Transition};
use gpui_kit::base::{Button as BaseButton, Radio};
use std::time::Duration;

#[derive(Deserialize)]
struct Data {
    labels: BTreeMap<String, String>,
    assets: BTreeMap<String, String>,
}
fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("kitsune_data.json")).expect("audited Kitsune data")
    })
}
fn label(symbol: &str) -> String {
    t(&data().labels[symbol])
}
fn panel(title: &str, tip: &str, cx: &App) -> Div {
    surface::panel(label(title), cx).relative().child(
        div()
            .absolute()
            .right(surface::css(10.))
            .top(surface::css(10.))
            .child(surface::help_control(
                SharedString::from(format!("kitsune-help-{tip}")),
                label(tip),
            )),
    )
}

#[derive(IntoElement)]
struct SourceRadio {
    path: &'static str,
    value: String,
    label: String,
    selected: bool,
    owner: WeakEntity<GamepadProductWorkspace>,
}
impl RenderOnce for SourceRadio {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = SharedString::from(format!("kitsune-{}-{}", self.path, self.value));
        let progress = motion::transition(
            id.clone(),
            if self.selected { 1_f32 } else { 0. },
            Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
            window,
            cx,
        );
        Radio::new(id)
            .checked(self.selected)
            .accessibility_label(self.label.clone())
            .flex()
            .items_center()
            .mx(surface::css(20.))
            .text_size(surface::css(14.))
            .line_height(surface::css(20.))
            .text_color(rgb(0xcccccc))
            .child(
                div()
                    .relative()
                    .size(surface::css(20.))
                    .flex_shrink_0()
                    .rounded_full()
                    .border_1()
                    .border_color(rgb(0x737373))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .size(surface::css(10. * progress))
                            .rounded_full()
                            .bg(rgb(0x44d62c))
                            .opacity(progress),
                    ),
            )
            .child(div().ml(surface::css(10.)).child(self.label))
            .on_change(move |_, _, _, cx| {
                let _ = self.owner.update(cx, |owner, cx| {
                    owner.write(self.path, json!(self.value), cx)
                });
            })
    }
}

impl GamepadProductWorkspace {
    fn kitsune_radio(
        &self,
        path: &'static str,
        value: &str,
        symbol: &str,
        cx: &Context<Self>,
    ) -> SourceRadio {
        SourceRadio {
            path,
            value: value.into(),
            label: label(symbol),
            selected: self.draft.pointer(path).and_then(Value::as_str) == Some(value),
            owner: cx.entity().downgrade(),
        }
    }

    pub(super) fn kitsune_customize(&self, cx: &Context<Self>) -> AnyElement {
        let preview = div()
            .relative()
            .w_full()
            .min_w(surface::css(770.))
            .max_w(surface::css(1220.))
            .h(surface::css(340.))
            .mx_auto()
            .flex_shrink_0()
            .child(surface::dot_background(cx))
            .child(
                div()
                    .relative()
                    .w(surface::css(770.))
                    .h_full()
                    .mx_auto()
                    .flex()
                    .items_center()
                    .justify_center()
                    // pP resolves only 4115_0/svg_prods/0.svg in this production
                    // context; other editions use its explicit edition-zero fallback.
                    .when(self.layout_id == 0, |view| {
                        view.child(
                            img(data().assets["device"].clone())
                                .h(surface::css(251.))
                                .w(surface::css(251. * 712. / 540.)),
                        )
                    }),
            );
        let polling = panel("lGq", "orU", cx)
            .child(div().mb(surface::css(10.)).child(label("PDD")))
            .child(h_flex().flex_wrap().gap(surface::css(10.)).children(
                self.spec.polling_rates.iter().map(|rate| {
                    let rate = *rate;
                    let selected = self
                        .draft
                        .pointer("/profile/pollingRate")
                        .and_then(Value::as_i64)
                        == Some(rate)
                        || self.spec.polling_rates.len() == 1;
                    BaseButton::new(SharedString::from(format!("kitsune-polling-{rate}")))
                        .accessibility_label(rate.to_string())
                        .w(surface::css(72.))
                        .h(surface::css(27.))
                        .p_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(surface::css(3.))
                        .bg(rgb(0x222222))
                        .border_1()
                        .border_color(if selected {
                            rgb(0x44d62c)
                        } else {
                            rgb(0x5d5d5d)
                        })
                        .text_color(rgb(0xcccccc))
                        .text_size(surface::css(14.))
                        .hover(|style| style.border_color(rgb(0x44d62c)))
                        .child(rate.to_string())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if !selected {
                                this.write("/profile/pollingRate", json!(rate), cx);
                            }
                        }))
                }),
            ))
            .child(div().mt(surface::css(10.)));
        let mut mode = panel("iYC", "Jrg", cx);
        for (value, title, description) in [("safe", "e4Q", "kto"), ("standard", "q_4", "WYV")] {
            mode = mode.child(
                div()
                    .ml(surface::css(-20.))
                    .mb(surface::css(16.))
                    .child(self.kitsune_radio("/controller/modeSwitcher/mode", value, title, cx))
                    .child(
                        div()
                            .ml(surface::css(50.))
                            .mt(surface::css(5.))
                            .text_size(surface::css(14.))
                            .child(label(description)),
                    ),
            );
        }
        let mut settings = div().flex_1().min_w_0();
        for (key, title, description) in [
            ("NEUTRAL", "gJm", "s1g"),
            ("FIRST", "kM3", "Obv"),
            ("LAST", "USp", "eJd"),
            ("ABSOLUTE_UP", "CKV", "oDm"),
            ("NONE", "wAK", "Qeq"),
        ] {
            if let Some(value) = self.spec.socd_modes.get(key) {
                settings = settings.child(
                    div()
                        .ml(surface::css(-20.))
                        .mt(surface::css(10.))
                        .child(self.kitsune_radio(
                            "/controller/dpad/socdSettings/value",
                            value,
                            title,
                            cx,
                        ))
                        .child(
                            div()
                                .ml(surface::css(50.))
                                .mt(surface::css(5.))
                                .text_size(surface::css(12.))
                                .opacity(0.7)
                                .child(label(description)),
                        ),
                );
            }
        }
        let socd = panel("SYq", "IlY", cx)
            .child(div().mb(surface::css(20.)).child(label("asZ")))
            .child(
                h_flex().items_start().child(settings).child(
                    img(data().assets["socd"].clone())
                        .w(surface::css(228.))
                        .h(surface::css(271.))
                        .mt(surface::css(10.))
                        .flex_shrink_0(),
                ),
            );
        v_flex()
            .w_full()
            .child(preview)
            .child(
                surface::page_columns()
                    .child(surface::page_column(v_flex().child(polling).child(mode)))
                    .child(surface::page_column(socd)),
            )
            .into_any_element()
    }
}
