//! 740/746's mounted calibration page and modal, from the current product chunks.
//! Calibration is transient device state, never a profile setting. The live
//! entry cannot advance without a transport; development samples are isolated.
use super::*;
use crate::ui::{scroll::SourceScrollable as _, theme::KeyboardCalibrationColors as Colors};
use gpui_kit::component::WindowExt as _;
use gpui_kit::prelude::FluentBuilder as _;

#[cfg(test)]
#[path = "keyboard_calibration_tests.rs"]
mod tests;

#[derive(Deserialize)]
struct Spec {
    product_id: u32,
    labels: BTreeMap<String, String>,
    modal_width: f32,
}
impl Spec {
    fn text(&self, role: &str) -> String {
        t(&self.labels[role])
    }
}
fn specification(pid: u32) -> Option<&'static Spec> {
    static SPECS: OnceLock<Vec<Spec>> = OnceLock::new();
    SPECS
        .get_or_init(|| {
            serde_json::from_str(include_str!("keyboard_calibration_data.json"))
                .expect("validated current calibration sources")
        })
        .iter()
        .find(|spec| spec.product_id == pid)
}

impl KeyboardProductWorkspace {
    pub(super) fn dismiss_calibration(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(modal) = self.calibration_modal.take() {
            modal.update(cx, |modal, cx| modal.close(window, cx));
        }
    }

    pub(super) fn calibration_page(&self, cx: &Context<Self>) -> AnyElement {
        let Some(spec) = specification(self.spec.product_id) else {
            return div().into_any_element();
        };
        v_flex()
            .gap(surface::css(20.))
            .children(self.calibration_intro_visible.then(|| {
                v_flex()
                    .relative()
                    .max_w(surface::css(760.))
                    .w_full()
                    .min_h(surface::css(129.))
                    .mx_auto()
                    .py(surface::css(27.5))
                    .px(surface::css(20.))
                    .gap(surface::css(16.))
                    .items_center()
                    .justify_center()
                    .text_center()
                    .bg(Colors::introduction())
                    .rounded(surface::css(5.))
                    .child(
                        div()
                            .font_family("RazerF5")
                            .text_size(surface::css(26.))
                            .text_color(cx.theme().primary)
                            .child(spec.text("title").to_uppercase()),
                    )
                    .child(
                        div()
                            .text_size(surface::css(14.))
                            .child(spec.text("introduction")),
                    )
                    .child(
                        close_button("keyboard-calibration-intro-close", &spec.text("cancel"), cx)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.calibration_intro_visible = false;
                                cx.notify();
                            })),
                    )
            }))
            // Source la uses pointer-events:none. Calibration must not select or
            // rewrite key mappings through the Customize hit regions.
            .child(self.keyboard_image(false, cx))
            .child(
                surface::page_columns().child(surface::page_column(
                    surface::panel(spec.text("title"), cx)
                        .child(spec.text("description"))
                        .child(
                            div().mt(surface::css(4.)).child(
                                command_button(
                                    "keyboard-calibration-start",
                                    spec.text("start"),
                                    true,
                                    false,
                                    cx,
                                )
                                .w(surface::css(146.))
                                .on_click(cx.listener(
                                    move |this, _, window, cx| {
                                        this.dismiss_calibration(window, cx);
                                        this.calibration_modal =
                                            Some(CalibrationModal::open(spec, window, cx));
                                        cx.notify();
                                    },
                                )),
                            ),
                        )
                        .child(
                            h_flex()
                                .items_start()
                                .gap(surface::css(5.))
                                .mt(surface::css(4.))
                                .text_size(surface::css(12.))
                                .text_color(cx.theme().muted_foreground)
                                .child(
                                    img("synapse/keyboard-calibration-info.svg")
                                        .size(surface::css(24.))
                                        .flex_shrink_0(),
                                )
                                .child(spec.text("note")),
                        ),
                )),
            )
            .into_any_element()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sample {
    SelectKey,
    KeySelected,
    PressKey,
    VerifyBottom,
    ReleaseKey,
    CalibrateTop,
    Success,
    Failure,
}
impl Sample {
    const ALL: [(Self, &'static str, &'static str); 8] = [
        (Self::SelectKey, "select", "等待按键"),
        (Self::KeySelected, "selected", "已选择按键"),
        (Self::PressKey, "press", "按到底"),
        (Self::VerifyBottom, "verify", "校验按下位置"),
        (Self::ReleaseKey, "release", "松开按键"),
        (Self::CalibrateTop, "top", "校验释放位置"),
        (Self::Success, "success", "成功"),
        (Self::Failure, "failure", "失败"),
    ];
    fn step(self) -> u8 {
        match self {
            Self::SelectKey | Self::KeySelected => 1,
            Self::PressKey => 2,
            Self::VerifyBottom | Self::ReleaseKey => 3,
            Self::CalibrateTop | Self::Success => 4,
            Self::Failure => 2,
        }
    }
    fn has_result(self) -> bool {
        matches!(self, Self::CalibrateTop | Self::Success | Self::Failure)
    }
    fn pending(self) -> bool {
        matches!(self, Self::VerifyBottom | Self::CalibrateTop)
    }
    fn next(self) -> Self {
        match self {
            Self::KeySelected => Self::PressKey,
            Self::PressKey => Self::VerifyBottom,
            Self::ReleaseKey => Self::CalibrateTop,
            Self::Success | Self::Failure => Self::SelectKey,
            _ => self,
        }
    }
}

pub(super) struct CalibrationModal {
    spec: &'static Spec,
    focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    open: bool,
}
impl CalibrationModal {
    fn open(spec: &'static Spec, window: &mut Window, cx: &mut App) -> Entity<Self> {
        let return_focus = window.focused(cx);
        let modal = cx.new(|cx| Self {
            spec,
            focus: cx.focus_handle(),
            return_focus,
            open: true,
        });
        modal.read(cx).focus.clone().focus(window, cx);
        modal
    }
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        self.open = false;
        if let Some(focus) = self.return_focus.take() {
            focus.focus(window, cx);
        }
        cx.notify();
    }
}
impl Render for CalibrationModal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }
        // Source .backdrop.show .choose-a-mat starts 100px below the viewport
        // and extends to its bottom; the component's unused .modal CSS is not
        // the mounted surface. Keep that distinction in the native geometry.
        let height = (window.viewport_size().height - window.rem_size() * (100. / 16.)).max(px(0.));
        let panel = v_flex()
            .id("keyboard-calibration-modal")
            .test_support()
            .occlude()
            .relative()
            .w(surface::css(self.spec.modal_width))
            .max_w(window.viewport_size().width)
            .h(height)
            .bg(cx.theme().popover)
            .rounded_t(surface::css(5.))
            .text_color(cx.theme().foreground)
            .child(
                modal_header(self.spec, cx).child(
                    close_button("keyboard-calibration-close", &self.spec.text("cancel"), cx)
                        .on_click(cx.listener(|this, _, window, cx| this.close(window, cx))),
                ),
            )
            .child(modal_content(self.spec, Sample::SelectKey, cx))
            .child(modal_footer(
                self.spec,
                Sample::SelectKey,
                false,
                cx.listener(|this, _, window, cx| this.close(window, cx)),
                |_, _, _| {},
                cx,
            ));
        gpui_kit::base::Dialog::new(cx)
            .focus_handle(self.focus.clone())
            .close_on_backdrop_press(false)
            .on_ok(|_, _, _| false)
            .on_close(cx.listener(|this, _, window, cx| this.close(window, cx)))
            .backdrop(
                div()
                    .absolute()
                    .inset_0()
                    .bg(cx.theme().title_bar.opacity(0.5)),
            )
            .popup(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_end()
                    .justify_center()
                    .child(panel),
            )
            .into_any_element()
    }
}

fn close_button(id: &'static str, label: &str, cx: &App) -> gpui_kit::base::Button {
    gpui_kit::base::Button::new(id)
        .absolute()
        .top(surface::css(10.))
        .right(surface::css(10.))
        .size(surface::css(24.))
        .p_0()
        .accessibility_label(label.to_owned())
        .focus_visible(|style| style.border_1().border_color(cx.theme().primary))
        .child(img("synapse/calibration-close.svg").size_full())
}
fn command_button(
    id: &'static str,
    label: String,
    primary: bool,
    disabled: bool,
    cx: &App,
) -> gpui_kit::base::Button {
    gpui_kit::base::Button::new(id)
        .accessibility_label(label.clone())
        .disabled(disabled)
        .h(surface::css(27.))
        .min_w(surface::css(90.))
        .px(surface::css(10.))
        .py_0()
        .flex_shrink_0()
        .text_size(surface::css(12.))
        .line_height(surface::css(14.))
        .border_1()
        .border_color(Colors::button_border())
        .rounded(surface::css(3.))
        .bg(if primary {
            cx.theme().primary
        } else {
            Colors::button()
        })
        .text_color(if primary {
            cx.theme().primary_foreground
        } else {
            Colors::button_text()
        })
        .when(disabled, |button| button.opacity(0.3))
        .when(!disabled, |button| {
            button.hover(move |style| {
                if primary {
                    style.bg(Colors::button_hover())
                } else {
                    style.opacity(0.8)
                }
            })
        })
        .focus_visible(|style| style.border_color(cx.theme().foreground))
        .child(label.to_uppercase())
}
fn modal_header(spec: &Spec, cx: &App) -> Div {
    div()
        .relative()
        .h(surface::css(36.))
        .flex_shrink_0()
        .py(surface::css(8.))
        .border_b_1()
        .border_color(cx.theme().border)
        .font_family("RazerF5")
        .text_size(surface::css(16.))
        .line_height(surface::css(19.))
        .text_center()
        .text_color(cx.theme().muted_foreground)
        .child(spec.text("title").to_uppercase())
}
fn modal_content(spec: &Spec, sample: Sample, cx: &App) -> AnyElement {
    let result = sample.has_result();
    let mut steps = h_flex().justify_center().p(surface::css(14.));
    for step in 1..=3_u8 {
        if step > 1 {
            steps = steps.child(
                h_flex()
                    .w(surface::css(80.))
                    .mx(surface::css(10.))
                    .justify_between()
                    .children((0..20).map(|_| div().size(surface::css(2.)).bg(Colors::button()))),
            );
        }
        let done = step < sample.step() || sample == Sample::ReleaseKey;
        steps = steps.child(
            div()
                .size(surface::css(34.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .text_size(surface::css(18.))
                .bg(if done {
                    cx.theme().primary
                } else {
                    Colors::step()
                })
                .text_color(Colors::step_text())
                .when(result, |circle| circle.opacity(0.3))
                .child(step.to_string()),
        );
    }
    let message = match sample {
        Sample::SelectKey | Sample::KeySelected => "select_key",
        Sample::ReleaseKey => "release_key",
        _ => "press_key",
    };
    v_flex()
        .id("keyboard-calibration-content")
        .flex_1()
        .min_h_0()
        .scrollable_y()
        .pt(surface::css(20.))
        .pl(surface::css(25.))
        .pb(surface::css(42.))
        .child(steps)
        .children(result.then(|| {
            v_flex()
                .items_center()
                .gap(surface::css(20.))
                .mt(surface::css(20.))
                .child(
                    img(if sample == Sample::Failure {
                        "synapse/keyboard-calibration-failure.svg"
                    } else {
                        "synapse/keyboard-calibration-success.svg"
                    })
                    .size(surface::css(32.)),
                )
                .child(
                    div()
                        .text_size(surface::css(16.))
                        .text_center()
                        .text_color(if sample == Sample::Failure {
                            cx.theme().danger
                        } else {
                            cx.theme().primary
                        })
                        .child(
                            spec.text(if sample == Sample::Failure {
                                "failure"
                            } else {
                                "success"
                            })
                            .to_uppercase(),
                        ),
                )
        }))
        .children((!result).then(|| {
            v_flex()
                .p(surface::css(20.))
                .child(
                    div()
                        .text_size(surface::css(16.))
                        .text_color(cx.theme().primary)
                        .mb(surface::css(10.))
                        .child(spec.text("mode").to_uppercase()),
                )
                .child(
                    h_flex()
                        .items_stretch()
                        .gap(surface::css(80.))
                        .px(surface::css(30.))
                        .mb(surface::css(20.))
                        .bg(Colors::panel())
                        .rounded(surface::css(10.))
                        .child(
                            div()
                                .flex_shrink_0()
                                .w(surface::css(80.))
                                .h(surface::css(40.))
                                .my(surface::css(30.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(Colors::key())
                                .border_1()
                                .border_color(cx.theme().popover)
                                .rounded(surface::css(5.))
                                .text_size(surface::css(14.))
                                .text_color(Colors::button_text())
                                .when(sample.step() > 1, |key| key.opacity(0.3))
                                .child(if sample == Sample::SelectKey {
                                    "|"
                                } else {
                                    "A"
                                }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .my(surface::css(20.))
                                .flex()
                                .items_center()
                                .text_size(surface::css(14.))
                                .text_color(Colors::instruction())
                                .child(spec.text(message)),
                        ),
                )
                .children(sample.pending().then(|| {
                    v_flex()
                        .gap(surface::css(8.))
                        .child(spec.text("calibrating"))
                        // A static waiting sample. No timer fabricates a device response.
                        .child(
                            div()
                                .h(surface::css(5.))
                                .rounded_full()
                                .bg(cx.theme().primary.opacity(0.3))
                                .child(
                                    div()
                                        .w(relative(0.25))
                                        .h_full()
                                        .rounded_full()
                                        .bg(cx.theme().primary),
                                ),
                        )
                }))
        }))
        .into_any_element()
}
fn modal_footer(
    spec: &Spec,
    sample: Sample,
    preview: bool,
    cancel: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    next: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> AnyElement {
    let label = if sample == Sample::Failure {
        "restart"
    } else if sample.step() > 3 {
        "done"
    } else {
        "next"
    };
    h_flex()
        .flex_shrink_0()
        .justify_center()
        .gap(surface::css(12.))
        .p(surface::css(16.))
        .child(
            command_button(
                "keyboard-calibration-cancel",
                spec.text("cancel"),
                false,
                false,
                cx,
            )
            .on_click(cancel),
        )
        .child(
            div()
                .id("keyboard-calibration-availability")
                .when(!preview, |button| {
                    button.tooltip(|window, cx| {
                        gpui_kit::component::tooltip::Tooltip::new("键盘校准暂不可用")
                            .build(window, cx)
                    })
                })
                .child(
                    command_button(
                        "keyboard-calibration-next",
                        spec.text(label),
                        true,
                        !preview || sample == Sample::SelectKey || sample.pending(),
                        cx,
                    )
                    .on_click(next),
                ),
        )
        .into_any_element()
}

struct CalibrationPreview {
    sample: Sample,
}
pub(crate) fn open_preview(window: &mut Window, cx: &mut App) {
    let preview = cx.new(|_| CalibrationPreview {
        sample: Sample::SelectKey,
    });
    window.open_dialog(cx, move |dialog, window, _| {
        dialog
            .title("磁轴键盘校准 · 界面预览")
            .w(window.rem_size() * (900. / 16.))
            .child(preview.clone())
    });
}
impl Render for CalibrationPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let spec = specification(740).expect("audited calibration preview");
        v_flex()
            .gap_3()
            .child(surface::note(
                "以下为静态状态示例，不执行键盘校准。等待设备回应的步骤可通过上方状态选项查看。",
                cx,
            ))
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .children(Sample::ALL.map(|(sample, id, label)| {
                        Button::new((ElementId::from("calibration-sample"), id))
                            .label(label)
                            .outline()
                            .selected(self.sample == sample)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.sample = sample;
                                cx.notify();
                            }))
                    })),
            )
            .child(
                v_flex()
                    .h(surface::css(420.))
                    .bg(cx.theme().popover)
                    .child(modal_header(spec, cx))
                    .child(modal_content(spec, self.sample, cx))
                    .child(modal_footer(
                        spec,
                        self.sample,
                        true,
                        cx.listener(|this, _, _, cx| {
                            this.sample = Sample::SelectKey;
                            cx.notify();
                        }),
                        cx.listener(|this, _, _, cx| {
                            this.sample = this.sample.next();
                            cx.notify();
                        }),
                        cx,
                    )),
            )
    }
}
