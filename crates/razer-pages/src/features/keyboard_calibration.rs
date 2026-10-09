//! 740/746's mounted calibration page and modal, from the current product chunks.
//! Calibration is transient device state, never a profile setting. The live
//! entry cannot advance without a transport; development samples are isolated.
use super::*;
use crate::features::source_workspace::SourceProductWorkspace;
use gpui_kit::base::motion::{self, Easing, Presence, Transition};
use gpui_kit::component::WindowExt as _;
use razer_widgets::scroll::SourceScrollable as _;
use razer_widgets::theme::KeyboardCalibrationColors as Colors;
use std::{path::PathBuf, time::Duration};

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

/// The dashboard source stores this banner in localStorage under the exact
/// key below. Keep the same cross-startup behavior in the native shell using
/// the app's local data directory; no device or host service is involved.
const INTRO_STORAGE_KEY: &str = "showNotificationBannerCalibration";

fn intro_storage_path() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("razer_ui")
        .join(format!("{INTRO_STORAGE_KEY}.json"))
}

pub(super) fn load_intro_visibility() -> bool {
    match std::fs::read_to_string(intro_storage_path()) {
        Ok(value) => serde_json::from_str::<bool>(&value).unwrap_or(true),
        Err(_) => true,
    }
}

pub(super) fn save_intro_visibility(visible: bool) {
    let path = intro_storage_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(
        path,
        serde_json::to_string(&visible).unwrap_or_else(|_| "true".into()),
    );
}

fn ease_in_out(phase: f32) -> f32 {
    // CSS ease-in-out is cubic-bezier(.42, 0, .58, 1), not quadratic easing.
    Easing::EaseInOut.sample(phase)
}

/// 740/746 root componentDidUpdate compares selectedProfileGuid exactly.
/// A user-renamed profile must never acquire factory restrictions by its name.
pub fn is_factory_profile(product_id: u32, guid: &str) -> bool {
    specification(product_id).is_some() && guid == "af06d371-861f-4b98-8d78-bfaef5cfdebf"
}

impl KeyboardProductWorkspace {
    pub(super) fn dismiss_calibration(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(modal) = self.calibration_modal.take() {
            modal.update(cx, |modal, cx| modal.close(window, cx));
        }
    }

    pub(super) fn calibration_page(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let Some(spec) = specification(self.spec.product_id) else {
            return div().into_any_element();
        };
        let contents = v_flex()
            .gap(surface::css(20.))
            .when(self.factory_default_profile, |view| view.opacity(0.3))
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
                            .disabled(self.factory_default_profile)
                            .on_click(cx.listener(|this, _, _, cx| {
                                if this.factory_default_profile {
                                    return;
                                }
                                this.calibration_intro_visible = false;
                                save_intro_visibility(false);
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
                        .map(|panel| {
                            panel.child(
                                // Section and first button both have margin-top:20px;
                                // these adjoining block margins collapse in the source.
                                div().mt(surface::css(20.)).flex().justify_center().child(
                                    command_button(
                                        "keyboard-calibration-start",
                                        spec.text("start"),
                                        true,
                                        false,
                                        cx,
                                    )
                                    .disabled(self.factory_default_profile)
                                    .w(surface::css(146.))
                                    .on_click(cx.listener(
                                        move |this, _, window, cx| {
                                            if this.factory_default_profile {
                                                return;
                                            }
                                            this.dismiss_calibration(window, cx);
                                            this.calibration_modal = Some(CalibrationModal::open(
                                                spec,
                                                this.calibration_preview
                                                    .then_some(Sample::SelectKey),
                                                window,
                                                cx,
                                            ));
                                            cx.notify();
                                        },
                                    )),
                                ),
                            )
                        })
                        .map(|panel| {
                            panel.child(
                                h_flex()
                                    .items_start()
                                    .gap(surface::css(5.))
                                    .mt(surface::css(20.))
                                    .text_size(surface::css(12.))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(
                                        img("synapse/keyboard-calibration-info.svg")
                                            .size(surface::css(24.))
                                            .flex_shrink_0(),
                                    )
                                    .child(spec.text("note")),
                            )
                        }),
                )),
            );
        div()
            .child(contents)
            .when(self.factory_default_profile, |view| {
                view.child(factory_warning(window))
            })
            .into_any_element()
    }
}

fn factory_warning(window: &Window) -> AnyElement {
    let unit = window.rem_size() / 16.;
    let icon = f32::from(unit * 14.);
    let html = t("FACTORY_DEFAULT_PROFILE_WARNING_DESC").replace(
        "{{dots}}",
        &format!("<img src=\"factory-profile-dots\" width=\"{icon}\" height=\"{icon}\" />"),
    );
    // Aa's .factory-default overrides the common warning-alert position,
    // dimensions and title color; the common translateY(-100%) remains active.
    deferred(
        anchored()
            .anchor(Anchor::BottomCenter)
            .position(point(
                window.viewport_size().width / 2.,
                window.viewport_size().height * 0.45,
            ))
            .child(
                v_flex()
                    .id("keyboard-calibration-factory-warning")
                    .w(unit * 465.)
                    .p(surface::css(20.))
                    .rounded(surface::css(3.))
                    .bg(rgb(0x111111))
                    .border_1()
                    .border_color(rgb(0xfd8611))
                    .shadow(vec![BoxShadow {
                        color: rgba(0x00000033).into(),
                        offset: point(px(0.), unit * 6.),
                        blur_radius: unit * 10.,
                        spread_radius: px(0.),
                        inset: false,
                    }])
                    .font_family("Roboto")
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .text_color(rgb(0xcccccc))
                    .text_center()
                    .child(
                        div()
                            .mb(surface::css(10.))
                            .text_size(surface::css(16.))
                            .line_height(surface::css(16.8))
                            .child(t("FACTORY_DEFAULT_PROFILE_TITTLE").to_uppercase()),
                    )
                    .child(
                        gpui_kit::base::TextView::html(
                            "keyboard-calibration-factory-description",
                            html,
                        )
                        .image_source(|_| "synapse/profile-more.svg".into())
                        .selectable(false)
                        .scrollable(false),
                    ),
            ),
    )
    .with_priority(1)
    .into_any_element()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sample {
    SelectKey,
    KeySelected,
    CalibrateBottom,
    PressKey,
    VerifyBottom,
    ReleaseKey,
    CalibrateTop,
    Success,
    Failure,
}
impl Sample {
    const ALL: [(Self, &'static str, &'static str); 9] = [
        (Self::SelectKey, "select", "等待按键"),
        (Self::KeySelected, "selected", "已选择按键"),
        (Self::CalibrateBottom, "bottom", "校准按下位置"),
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
            Self::CalibrateBottom | Self::PressKey => 2,
            Self::VerifyBottom | Self::ReleaseKey => 3,
            Self::CalibrateTop | Self::Success => 4,
            Self::Failure => 2,
        }
    }
    fn has_result(self) -> bool {
        matches!(self, Self::CalibrateTop | Self::Success | Self::Failure)
    }
    fn pending(self) -> bool {
        matches!(self, Self::CalibrateBottom | Self::CalibrateTop)
    }
    fn next(self) -> Self {
        match self {
            Self::KeySelected => Self::CalibrateBottom,
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
    /// None is the real device path. Only the isolated preview supplies a fixture.
    sample: Option<Sample>,
}
impl CalibrationModal {
    fn open(
        spec: &'static Spec,
        sample: Option<Sample>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        let return_focus = window.focused(cx);
        let modal = cx.new(|cx| Self {
            spec,
            focus: cx.focus_handle(),
            return_focus,
            open: true,
            sample,
        });
        modal.read(cx).focus.clone().focus(window, cx);
        modal
    }
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        self.open = false;
        self.sample = self.sample.map(|_| Sample::SelectKey);
        if let Some(focus) = self.return_focus.take() {
            focus.focus(window, cx);
        }
        cx.notify();
    }
    fn advance_sample(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sample) = self.sample else {
            return;
        };
        if sample == Sample::SelectKey || sample.pending() {
            return;
        }
        if sample == Sample::Success {
            self.close(window, cx);
        } else {
            self.sample = Some(sample.next());
            cx.notify();
        }
    }
}
impl Render for CalibrationModal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }
        // Ea -> Ca=Sa mounts .choose-a-mat, not the unused CSS-module .modal.
        // The shared sheet has min-width:800px, and width:1050px only >=1400;
        // Ea's inline maxWidth:850px caps the latter. The source document itself
        // has min-height:720px, so preserve the 800px overflow on narrow windows.
        let viewport = window.viewport_size();
        let unit = window.rem_size() / 16.;
        let width = unit
            * if viewport.width / unit >= 1400. {
                self.spec.modal_width
            } else {
                800.
            };
        let progress = Presence::new(("keyboard-calibration-modal", "top"), true)
            .transition(Transition::new(Duration::from_millis(300)).easing(Easing::Ease))
            .sample(window, cx)
            .progress;
        let opacity = Presence::new(("keyboard-calibration-modal", "opacity"), true)
            .transition(Transition::new(Duration::from_millis(100)).easing(Easing::Linear))
            .sample(window, cx)
            .progress;
        let top = viewport.height + (unit * 100. - viewport.height) * progress;
        let height = (viewport.height - top).max(px(0.));
        let panel = v_flex()
            .id("keyboard-calibration-modal")
            .test_support()
            .occlude()
            .absolute()
            .left((viewport.width - width) / 2.)
            .top(top)
            .w(width)
            .h(height)
            .opacity(opacity)
            .bg(cx.theme().popover)
            .rounded_t(surface::css(5.))
            .text_color(cx.theme().foreground)
            .child(
                modal_header(self.spec, cx).child(
                    modal_close_button(&self.spec.text("cancel"), window, cx)
                        .on_click(cx.listener(|this, _, window, cx| this.close(window, cx))),
                ),
            )
            .child(
                div()
                    .id("keyboard-calibration-scrollbar")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .pt(surface::css(20.))
                    .pl(surface::css(25.))
                    .pb(surface::css(42.))
                    .scrollable_y()
                    .child(modal_content(
                        self.spec,
                        self.sample.unwrap_or(Sample::SelectKey),
                        cx,
                    )),
            )
            .child(modal_footer(
                self.spec,
                self.sample.unwrap_or(Sample::SelectKey),
                self.sample.is_some(),
                cx.listener(|this, _, window, cx| this.close(window, cx)),
                cx.listener(|this, _, window, cx| this.advance_sample(window, cx)),
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
                    .bg(rgba(0x00000080))
                    .opacity(opacity),
            )
            .popup(div().absolute().inset_0().child(panel))
            .into_any_element()
    }
}

#[derive(Default)]
struct CloseInteraction {
    hovered: bool,
    pressed: bool,
}

fn modal_close_button(label: &str, window: &mut Window, cx: &mut App) -> gpui_kit::base::Button {
    let state = window.use_keyed_state(
        (ElementId::from("keyboard-calibration-close"), "state"),
        cx,
        |_, _| CloseInteraction::default(),
    );
    let current = state.read(cx);
    let target: Hsla = if current.pressed && current.hovered {
        rgba(0x0000001a)
    } else if current.hovered {
        rgba(0xffffff1a)
    } else {
        rgba(0x00000000)
    }
    .into();
    let background = motion::transition(
        ("keyboard-calibration-close", "background"),
        target,
        Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
        window,
        cx,
    );
    gpui_kit::base::Button::new("keyboard-calibration-close")
        .absolute()
        .top_0()
        .right_0()
        .size(surface::css(36.))
        .p_0()
        .accessibility_label(label.to_owned())
        .bg(background)
        .on_hover(window.listener_for(&state, |state, hovered, _, cx| {
            state.hovered = *hovered;
            if !hovered {
                state.pressed = false;
            }
            cx.notify();
        }))
        .on_mouse_down(
            MouseButton::Left,
            window.listener_for(&state, |state, _, _, cx| {
                state.pressed = true;
                cx.notify();
            }),
        )
        .on_mouse_up(
            MouseButton::Left,
            window.listener_for(&state, |state, _, _, cx| {
                state.pressed = false;
                cx.notify();
            }),
        )
        .on_mouse_up_out(
            MouseButton::Left,
            window.listener_for(&state, |state, _, _, cx| {
                state.pressed = false;
                cx.notify();
            }),
        )
        .focus_visible(|style| style.border_1().border_color(cx.theme().primary))
        .child(img("synapse/mapping-close.svg").size(surface::css(20.)))
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
                    style
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
        .pt(surface::css(9.))
        .pb(surface::css(8.))
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
        .w_full()
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
        // Source uses visibility:hidden, retaining the section's layout and
        // loading subtree after the result is shown.
        .child({
            v_flex()
                .p(surface::css(20.))
                .when(result, |view| view.invisible())
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
                                .child(calibration_key(sample, cx)),
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
                    let progress = div()
                        .relative()
                        .h(surface::css(5.))
                        .rounded_full()
                        .overflow_hidden()
                        .bg(cx.theme().primary.opacity(0.3));
                    let bar = div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .w(relative(0.25))
                        .rounded_full()
                        .bg(cx.theme().primary);
                    let bar = if cx.reduce_motion() {
                        bar.left(relative(0.0)).into_any_element()
                    } else {
                        bar.with_animation(
                            "keyboard-calibration-progress",
                            Animation::new(Duration::from_secs(2))
                                .repeat()
                                .with_easing(ease_in_out),
                            |bar, phase| bar.left(relative(phase * 1.25 - 0.25)),
                        )
                        .into_any_element()
                    };
                    v_flex()
                        .gap(surface::css(8.))
                        .child(spec.text("calibrating"))
                        // Source CSS animates this bar; it never completes a
                        // calibration by itself, so the native service boundary
                        // remains explicit while the waiting animation is real.
                        .child(progress.child(bar))
                }))
        })
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
        .absolute()
        .left_0()
        .bottom_0()
        .w_full()
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

fn calibration_key(sample: Sample, cx: &App) -> AnyElement {
    if sample != Sample::SelectKey {
        return div().child("A").into_any_element();
    }
    let caret = div().font_weight(FontWeight::EXTRA_BOLD).child("|");
    if cx.reduce_motion() {
        return caret.into_any_element();
    }
    // Full source keyframes: 0%,50%,100% opacity:1; 25%,75% opacity:0.
    caret
        .with_animation(
            "keyboard-calibration-caret",
            Animation::new(Duration::from_millis(1100)).repeat(),
            |caret, phase| {
                caret.opacity(if (phase * 4.).floor() as u8 % 2 == 0 {
                    1.
                } else {
                    0.
                })
            },
        )
        .into_any_element()
}

struct CalibrationPreview {
    product_id: u32,
    workspace: Entity<SourceProductWorkspace>,
    keyboard: Entity<KeyboardProductWorkspace>,
    _keyboard_subscription: Subscription,
    _modal_subscription: Option<Subscription>,
    failure_close_task: Option<Task<()>>,
}
impl CalibrationPreview {
    fn new(product_id: u32, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let device = razer_model::demo::registered_preview(product_id)
            .expect("registered calibration keyboard");
        let workspace = cx.new(|cx| SourceProductWorkspace::new(device, window, cx));
        workspace.update(cx, |workspace, cx| {
            workspace.set_page_key("TAB_CALIBRATION", window, cx)
        });
        let keyboard = workspace
            .read(cx)
            .keyboard_preview_page()
            .expect("calibration keyboard page attached");
        keyboard.update(cx, |keyboard, _| keyboard.calibration_preview = true);
        let subscription = cx.observe(&keyboard, |this, keyboard, cx| {
            let modal = keyboard.read(cx).calibration_modal.clone();
            this._modal_subscription =
                modal.map(|modal| cx.observe(&modal, |_, _, cx| cx.notify()));
            cx.notify();
        });
        Self {
            product_id,
            workspace,
            keyboard,
            _keyboard_subscription: subscription,
            _modal_subscription: None,
            failure_close_task: None,
        }
    }

    fn select_sample(&mut self, sample: Sample, window: &mut Window, cx: &mut Context<Self>) {
        self.failure_close_task = None;
        self.workspace.update(cx, |workspace, cx| {
            workspace.set_page_key("TAB_CALIBRATION", window, cx)
        });
        let spec = specification(self.product_id).expect("audited calibration preview");
        self.keyboard.update(cx, |keyboard, cx| {
            keyboard.dismiss_calibration(window, cx);
            keyboard.calibration_modal =
                Some(CalibrationModal::open(spec, Some(sample), window, cx));
            cx.notify();
        });
        if sample == Sample::Failure {
            // The source starts a one-second idle watchdog and closes the
            // calibration surface after 15 seconds without a fresh input.
            // The preview has no input redirect, so expose the same terminal
            // failure timeout without fabricating a device response.
            self.failure_close_task = Some(cx.spawn_in(window, async move |this, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(15_000))
                    .await;
                let _ = this.update_in(cx, |this, window, cx| {
                    let failed = this
                        .keyboard
                        .read(cx)
                        .calibration_modal
                        .as_ref()
                        .is_some_and(|modal| {
                            let modal = modal.read(cx);
                            modal.open && modal.sample == Some(Sample::Failure)
                        });
                    if failed {
                        this.keyboard
                            .update(cx, |keyboard, cx| keyboard.dismiss_calibration(window, cx));
                    }
                    this.failure_close_task = None;
                    cx.notify();
                });
            }));
        }
        cx.notify();
    }
}
pub fn open_preview(window: &mut Window, cx: &mut App) {
    let preview = cx.new(|cx| CalibrationPreview::new(740, window, cx));
    window.open_dialog(cx, move |dialog, window, _| {
        dialog
            .title("磁轴键盘校准 · 界面预览")
            .w(window.rem_size() * (1100. / 16.))
            .child(preview.clone())
    });
}
impl Render for CalibrationPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_sample = self
            .keyboard
            .read(cx)
            .calibration_modal
            .as_ref()
            .and_then(|modal| {
                let modal = modal.read(cx);
                modal.open.then_some(modal.sample).flatten()
            });
        v_flex()
            .gap_3()
            .child(surface::note(
                "独立的产品界面预览，不执行键盘校准。可从校准页面开始，或选择下方状态查看对应弹窗；等待状态不会自动完成。",
                cx,
            ))
            .child(
                h_flex().gap_2().children([(740, "740"), (746, "746")].map(|(pid, id)| {
                    Button::new((ElementId::from("calibration-product"), id))
                        .label(source_product(pid).expect("registered keyboard").name.clone())
                        .outline()
                        .selected(self.product_id == pid)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if this.product_id != pid {
                                this.keyboard.update(cx, |keyboard, cx| keyboard.dismiss_calibration(window, cx));
                                *this = Self::new(pid, window, cx);
                                cx.notify();
                            }
                        }))
                })),
            )
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .children(Sample::ALL.map(|(sample, id, label)| {
                        Button::new((ElementId::from("calibration-sample"), id))
                            .label(label)
                            .outline()
                            .selected(active_sample == Some(sample))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.select_sample(sample, window, cx);
                            }))
                    })),
            )
            .child(
                div()
                    .id("calibration-preview-workspace")
                    .h(surface::css(650.))
                    .min_h_0()
                    .child(self.workspace.clone()),
            )
    }
}
