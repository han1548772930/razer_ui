//! Current /synapse/update-fw/ (1.0.0 / 2604010417), verified by fresh HTTP.
//! Real firmware metadata/SDK transport is absent. Only an explicit local preview
//! can enter the updater's state machine, and its callbacks never write storage.
use crate::{
    features::Choice,
    i18n,
    model::Device,
    resources,
    ui::{
        scroll::SourceScrollable as _,
        surface::{self, css},
        theme::{FirmwareColors as Colors, PaletteColors},
    },
};
use gpui_kit::base::{
    Button, Dialog,
    motion::{self, Easing, Transition},
};
use gpui_kit::component::{
    select::{SelectEvent, SelectState},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::time::Duration;

mod state;
use state::{Component, Flow, Preset, Stage, Warning};

fn text(key: &str) -> String {
    // Ee/Ue uses English, then the original key when no locale exports it.
    // rust-i18n supplies the English fallback; don't expose our namespace.
    i18n::t_or(&format!("FIRMWARE_SOURCE.{key}"), key)
}

const SCENES: &[(&str, &str)] = &[
    ("unknown", "实际服务：状态尚未读取"),
    ("welcome", "本地预览：欢迎 / 完整流程"),
    ("prepare", "本地预览：准备中"),
    ("ready", "本地预览：等待开始更新"),
    ("upgrade", "本地预览：正在更新"),
    ("partial", "本地预览：第一部分已完成"),
    ("latest", "本地预览：第一部分已是最新"),
    ("switch", "本地预览：切换设备连接"),
    ("multiple", "本地预览：检测到多个匹配设备"),
    ("success", "本地预览：全部完成"),
    ("incomplete", "本地预览：跳过 / 部分完成"),
    ("waiting", "本地预览：等待其他设备"),
    ("disconnected", "本地预览：设备断开"),
    ("isolate-failed", "本地预览：准备失败"),
    ("failed", "本地预览：更新失败"),
    ("exit-warning", "本地预览：更新中请求关闭"),
];
const PRESETS: &[(&str, &str)] = &[
    ("keyboard-usb", "键盘：USB → 接收器"),
    ("keyboard-dongle", "键盘：接收器 → USB"),
    ("mouse-usb", "鼠标：USB → 小接收器"),
    ("mouse-dongle", "鼠标：大接收器 → USB"),
    ("keyboard-single", "键盘：仅 USB"),
];
fn choices(items: &[(&str, &str)]) -> Vec<Choice> {
    items
        .iter()
        .map(|(id, label)| Choice::new(*id, *label))
        .collect()
}

pub(super) struct CloseRequested;
pub(super) struct FirmwareUpdate {
    snapshot: Option<Device>,
    preview_enabled: bool,
    flow: Flow,
    scenario: String,
    scenes: Entity<SelectState<Vec<Choice>>>,
    presets: Entity<SelectState<Vec<Choice>>>,
    focus: FocusHandle,
    warning_focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    progress_task: Option<Task<()>>,
    wait_task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<CloseRequested> for FirmwareUpdate {}
impl FirmwareUpdate {
    pub(super) fn new(
        snapshot: Option<Device>,
        preview_enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let scenes =
            cx.new(|cx| SelectState::new(choices(SCENES), Some(IndexPath::new(0)), window, cx));
        let presets =
            cx.new(|cx| SelectState::new(choices(PRESETS), Some(IndexPath::new(0)), window, cx));
        let scene_subscription =
            cx.subscribe_in(&scenes, window, |this: &mut Self, _, event, window, cx| {
                if let SelectEvent::Confirm(Some(key)) = event {
                    this.choose(key, window, cx);
                }
            });
        let preset_subscription =
            cx.subscribe_in(&presets, window, |this: &mut Self, _, event, window, cx| {
                if let SelectEvent::Confirm(Some(key)) = event {
                    this.flow.preset = Preset::from_key(key);
                    let scenario = this.scenario.clone();
                    this.choose(&scenario, window, cx);
                }
            });
        Self {
            snapshot,
            preview_enabled,
            flow: Flow::default(),
            scenario: "unknown".into(),
            scenes,
            presets,
            focus: cx.focus_handle(),
            warning_focus: cx.focus_handle(),
            return_focus: None,
            progress_task: None,
            wait_task: None,
            _subscriptions: vec![scene_subscription, preset_subscription],
        }
    }
    pub(super) fn focus(&self, window: &mut Window, cx: &mut App) {
        if self.flow.warning.is_some() {
            self.warning_focus.focus(window, cx);
        } else {
            self.focus.focus(window, cx);
        }
    }
    /// Host close and application exit share the source's update-in-progress gate.
    /// Returning false means the original single-button warning owns the decision.
    pub(super) fn allow_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.flow.stage == Stage::Upgrade {
            self.flow.warning = Some(Warning::DeviceIsUpgrading);
            self.focus_warning(window, cx);
            return false;
        }
        // An application-close request may still be canceled in the shell's
        // save dialog. Actual entity removal owns cancellation of preview tasks.
        true
    }
    fn focus_warning(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.return_focus.is_none() {
            self.return_focus = window.focused(cx);
        }
        self.warning_focus.focus(window, cx);
        cx.notify();
    }
    fn dismiss_warning(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.flow.warning = None;
        if let Some(focus) = self.return_focus.take() {
            focus.focus(window, cx);
        }
        cx.notify();
    }
    fn choose(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.preview_enabled {
            return;
        }
        self.progress_task = None;
        self.wait_task = None;
        self.dismiss_warning(window, cx);
        self.scenario = key.into();
        self.flow.load(key);
        if self.flow.warning.is_some() {
            self.focus_warning(window, cx);
        }
        self.start_progress(window, cx);
        if self.flow.warning == Some(Warning::WaitForOtherDevices) {
            let ticket = self.flow.ticket();
            self.wait_task = Some(cx.spawn_in(window, async move |this, cx| {
                // A visible, explicitly selected sample callback. No discovery occurs.
                cx.background_executor().timer(Duration::from_secs(3)).await;
                let _ = this.update_in(cx, |this, window, cx| {
                    if this.flow.accepts(ticket)
                        && this.flow.warning == Some(Warning::WaitForOtherDevices)
                    {
                        this.dismiss_warning(window, cx);
                    }
                });
            }));
        }
        cx.notify();
    }
    fn start_progress(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.progress_task = None;
        if !matches!(self.flow.stage, Stage::Prepare | Stage::Upgrade) {
            return;
        }
        let ticket = self.flow.ticket();
        self.progress_task = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(450))
                    .await;
                let keep = this
                    .update_in(cx, |this, window, cx| {
                        let had_warning = this.flow.warning.is_some();
                        let keep = this.flow.tick(ticket);
                        if had_warning && this.flow.warning.is_none() {
                            this.dismiss_warning(window, cx);
                        }
                        cx.notify();
                        keep
                    })
                    .unwrap_or(false);
                if !keep {
                    break;
                }
            }
        }));
    }
    fn next(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.flow.next() {
            self.start_progress(window, cx);
            cx.notify();
        }
    }
    fn skip(&mut self, cx: &mut Context<Self>) {
        self.flow.skip();
        self.progress_task = None;
        cx.notify();
    }
    fn fail(&mut self, warning: Warning, window: &mut Window, cx: &mut Context<Self>) {
        self.progress_task = None;
        self.flow.fail(warning);
        self.focus_warning(window, cx);
    }
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.allow_close(window, cx) {
            cx.emit(CloseRequested);
        }
    }
    fn name(&self) -> String {
        if self.flow.preview() {
            if self.flow.preset.keyboard {
                "Razer 键盘（本地预览）".into()
            } else {
                "Razer 鼠标（本地预览）".into()
            }
        } else {
            self.snapshot
                .as_ref()
                .map(Device::display_name)
                .unwrap_or_else(|| "固件更新".into())
        }
    }
    fn component_name(&self, component: Component) -> String {
        if component == Component::Dongle {
            text("UPDATE_DONGLE")
        } else {
            self.name()
        }
    }
    fn product(&self) -> AnyElement {
        let picture = if self.flow.preview() {
            resources::dashboard_image(if self.flow.preset.keyboard { 653 } else { 182 }, 0, 1)
        } else {
            self.snapshot
                .as_ref()
                .and_then(|d| resources::dashboard_image(d.product_id, d.edition_id, d.layout_id))
        };
        h_flex()
            .w_full()
            .justify_center()
            .pb(css(20.))
            .child(
                div()
                    .w(css(466.))
                    .h(css(250.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when_some(picture, |view, path| {
                        view.child(img(path).size_full().object_fit(ObjectFit::Cover))
                    })
                    .when(picture.is_none(), |view| {
                        view.child(div().text_size(css(16.)).child(self.name()))
                    }),
            )
            .into_any_element()
    }
    fn heading(&self, key: &str) -> AnyElement {
        v_flex()
            .w_full()
            .pb(css(20.))
            .text_size(css(16.))
            .line_height(css(20.))
            .child(div().min_h(css(20.)).child(self.name().to_uppercase()))
            .child(
                div()
                    .mt(css(28.))
                    .min_h(css(20.))
                    .max_w(css(450.))
                    .text_color(Colors::highlight())
                    .child(text(key)),
            )
            .into_any_element()
    }
    fn versions(&self) -> AnyElement {
        let current = if self.flow.preview() {
            "1.00.00（示例）"
        } else {
            "未读取"
        };
        let latest = if self.flow.already_current {
            current
        } else if self.flow.preview() {
            "1.02.00（示例）"
        } else {
            "未读取"
        };
        v_flex()
            .flex_shrink_0()
            .gap(css(10.))
            .text_size(css(14.))
            .children(
                [("CURRENT_VERSION", current), ("LATEST_VERSION", latest)]
                    .into_iter()
                    .map(|(key, version)| {
                        h_flex()
                            .items_start()
                            .gap(css(18.))
                            .when(key == "LATEST_VERSION", |view| {
                                view.text_color(Colors::highlight())
                            })
                            .child(div().min_w(css(112.)).child(format!("{}:", text(key))))
                            .child(version)
                    }),
            )
            .into_any_element()
    }
    fn footer(
        &self,
        with_versions: bool,
        left: Option<(&'static str, bool)>,
        right: (&'static str, bool),
        cx: &mut Context<Self>,
    ) -> AnyElement {
        h_flex()
            .w_full()
            .items_end()
            .when(with_versions, |view| view.child(self.versions()))
            .child(
                h_flex()
                    .flex_1()
                    .justify_end()
                    .items_end()
                    .gap(css(20.))
                    .pt(css(5.))
                    .when_some(left, |view, (key, enabled)| {
                        view.child(
                            source_button("firmware-left", text(key), false, !enabled, cx)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if key == "START_OVER" {
                                        this.flow.restart();
                                        cx.notify();
                                    } else {
                                        this.skip(cx);
                                    }
                                })),
                        )
                    })
                    .child(
                        source_button("firmware-right", text(right.0), true, !right.1, cx)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if right.0 == "CLOSE" {
                                    this.close(window, cx);
                                } else {
                                    this.next(window, cx);
                                }
                            })),
                    ),
            )
            .into_any_element()
    }
    fn welcome(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .w_full()
            .child(self.product())
            .child(
                v_flex()
                    .min_h(css(104.))
                    .mb(css(40.))
                    .text_size(css(14.))
                    .child(
                        div()
                            .min_h(css(24.))
                            .line_height(css(24.))
                            .pb(css(16.))
                            .child(text("FW_UPDATER_UTILITY")),
                    )
                    .child(text("PLEASE_NOTE"))
                    .child(
                        div()
                            .ml(css(16.))
                            .mt(css(4.))
                            .child(format!("• {}", text("LAPTOP_NOTE"))),
                    ),
            )
            .child(self.footer(false, None, ("NEXT", true), cx))
            .into_any_element()
    }
    fn progress(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let stage = self.flow.stage;
        let progress = if stage == Stage::ReadyToUpgrade {
            0.
        } else {
            f32::from(self.flow.progress)
        };
        let animated = motion::transition(
            (
                "firmware-progress",
                format!("{:?}-{:?}", stage, self.flow.current),
            ),
            progress,
            Transition::new(Duration::from_millis(300)).easing(Easing::Linear),
            window,
            cx,
        );
        v_flex()
            .w_full()
            .child(self.heading(if stage == Stage::Upgrade {
                "UPDATING_NOTE"
            } else {
                "UPDATE_REQUIRED"
            }))
            .child(
                v_flex()
                    .mt(css(20.))
                    .mb(css(50.))
                    .opacity(if progress == 0. { 0.3 } else { 1. })
                    .child(div().h(css(20.)).mb(css(10.)).child(match stage {
                        Stage::Prepare => text("PENDING"),
                        Stage::Upgrade => text("UPDATING"),
                        _ => String::new(),
                    }))
                    .child(
                        div()
                            .w_full()
                            .h(css(8.))
                            .rounded(css(4.))
                            .bg(Colors::track())
                            .overflow_hidden()
                            .child(
                                div()
                                    .h_full()
                                    .w(relative(animated / 100.))
                                    .rounded(css(15.))
                                    .bg(Colors::highlight()),
                            ),
                    ),
            )
            .child(self.footer(
                true,
                Some(("SKIP", stage != Stage::Upgrade)),
                ("UPDATE", stage == Stage::ReadyToUpgrade),
                cx,
            ))
            .into_any_element()
    }
    fn partial(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut details = vec![];
        if self.flow.already_current {
            details.push(if self.flow.current == Component::Dongle {
                "DONGLE_IS_LATEST"
            } else if self.flow.preset.keyboard {
                "KEYBOARD_LATEST"
            } else {
                "MOUSE_LATEST"
            });
        }
        details.push(if self.flow.current == Component::Device {
            "NEXT_UPGRADE_DONGLE"
        } else if self.flow.preset.keyboard {
            "NEXT_UPGRADE_KEYBOARD"
        } else {
            "NEXT_UPGRADE_MOUSE"
        });
        v_flex()
            .w_full()
            .child(self.heading(if self.flow.already_current {
                "NO_UPDATE_REQUIRED"
            } else {
                "UPDATE_SUCCESSFUL"
            }))
            .child(
                v_flex().mt(css(10.)).pb(css(116.)).gap(css(12.)).children(
                    details
                        .into_iter()
                        .map(|key| div().child(text(key).replace("{{productName}}", &self.name()))),
                ),
            )
            .child(self.footer(true, Some(("SKIP", true)), ("NEXT", true), cx))
            .into_any_element()
    }
    fn switch_mode(&self, cx: &mut Context<Self>) -> AnyElement {
        let keyboard = self.flow.preset.keyboard;
        let to_usb = self.flow.current == Component::Device;
        let rows = if to_usb {
            [
                (
                    if keyboard {
                        "synapse/firmware-switch-usb-kb.svg"
                    } else {
                        "synapse/firmware-switch-usb-mouse.svg"
                    },
                    if keyboard {
                        "SWITCH_KEYBOARD_OFF"
                    } else {
                        "SWITCH_MOUSE_OFF"
                    },
                    if keyboard { 100. } else { 84. },
                    if keyboard { 60. } else { 66. },
                ),
                (
                    "synapse/firmware-pc-usb.svg",
                    if keyboard {
                        "CONNECT_KEYBOARD_WITH_CABLE"
                    } else {
                        "CONNECT_MOUSE_WITH_CABLE"
                    },
                    180.,
                    100.,
                ),
            ]
        } else {
            [
                (
                    if self.flow.preset.large_dongle {
                        "synapse/firmware-dongle-large.svg"
                    } else {
                        "synapse/firmware-dongle-small.svg"
                    },
                    "PLUGGED_HYPERSPEED_WIRELESS_DONGLE",
                    180.,
                    100.,
                ),
                (
                    if keyboard {
                        "synapse/firmware-switch-dongle-kb.svg"
                    } else {
                        "synapse/firmware-switch-dongle-mouse.svg"
                    },
                    if keyboard {
                        "SWITCH_KEYBOARD_CONNECT_TO_DONGLE"
                    } else {
                        "SWITCH_MOUSE_CONNECT_TO_DONGLE"
                    },
                    if keyboard { 100. } else { 84. },
                    if keyboard { 60. } else { 66. },
                ),
            ]
        };
        v_flex()
            .w_full()
            .line_height(css(20.))
            .child(
                div()
                    .text_size(css(16.))
                    .mb(css(30.))
                    .child(self.name().to_uppercase()),
            )
            .child(text("PLEASE_FOLLOW_STEPS"))
            .child(
                v_flex().mt(css(30.)).mb(css(40.)).gap(css(20.)).children(
                    rows.into_iter()
                        .enumerate()
                        .map(|(index, (path, key, width, height))| {
                            h_flex()
                                .relative()
                                .h(css(120.))
                                .w_full()
                                .px(css(20.))
                                .rounded(css(5.))
                                .bg(Colors::tile())
                                .child(
                                    h_flex().flex_1().child(
                                        img(path)
                                            .w(css(width))
                                            .h(css(height))
                                            .object_fit(ObjectFit::Contain),
                                    ),
                                )
                                .child(div().w(css(400.)).child(text(key)))
                                .child(
                                    div()
                                        .absolute()
                                        .left(css(-10.))
                                        .top(css(-10.))
                                        .size(css(20.))
                                        .rounded_full()
                                        .text_center()
                                        .text_size(css(12.))
                                        .line_height(css(20.))
                                        .text_color(Colors::border())
                                        .bg(Colors::highlight())
                                        .child((index + 1).to_string()),
                                )
                        }),
                ),
            )
            .child(self.footer(
                false,
                Some(("SKIP", true)),
                ("UPDATE", self.flow.can_switch()),
                cx,
            ))
            .into_any_element()
    }
    fn result(&self, cx: &mut Context<Self>) -> AnyElement {
        let done = self.flow.stage == Stage::CompleteDone;
        let remaining = self.flow.remaining();
        v_flex()
            .w_full()
            .child(self.product())
            .child(
                v_flex()
                    .pb(css(30.))
                    .mb(css(40.))
                    .min_h(css(104.))
                    .child(
                        div()
                            .text_color(Colors::highlight())
                            .min_h(css(20.))
                            .mb(css(20.))
                            .child(text("UPDATE_FINISHED")),
                    )
                    .when(!self.flow.completed.is_empty(), |view| {
                        view.child(
                            v_flex()
                                .mb(css(20.))
                                .child(text("UPDATE_SUCCESSFUL_HEADER"))
                                .children(self.flow.completed.iter().map(|part| {
                                    div()
                                        .pl(css(18.))
                                        .child(format!("• {}", self.component_name(*part)))
                                })),
                        )
                    })
                    .when(!done && !remaining.is_empty(), |view| {
                        view.child(v_flex().child(text("UPDATE_FAILED_HEADER")).children(
                            remaining.iter().map(|part| {
                                div()
                                    .pl(css(18.))
                                    .text_color(Colors::warning())
                                    .child(format!("• {}", self.component_name(*part)))
                            }),
                        ))
                    })
                    .when(
                        self.flow.preset.initial == Component::Device && self.flow.preset.dual,
                        |view| {
                            view.child(div().mt(css(22.)).child(text(
                                if self.flow.preset.keyboard {
                                    "UPGRADE_KEYBOARD_COMPLETED_NOTE"
                                } else {
                                    "UPGRADE_MOUSE_COMPLETED_NOTE"
                                },
                            )))
                        },
                    ),
            )
            .child(self.footer(
                false,
                (!done).then_some(("START_OVER", true)),
                ("CLOSE", true),
                cx,
            ))
            .into_any_element()
    }
    fn unknown(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .w_full()
            .child(self.product())
            .gap(css(20.))
            .child(
                div()
                    .text_size(css(16.))
                    .text_color(Colors::highlight())
                    .child("固件服务状态尚未读取"),
            )
            .child("尚未连接固件更新服务，无法读取可用版本、连接方式或更新条件。")
            .child(self.versions())
            .child(
                h_flex()
                    .justify_end()
                    .gap(css(20.))
                    .child(source_button(
                        "firmware-unavailable-update",
                        text("UPDATE"),
                        true,
                        true,
                        cx,
                    ))
                    .child(
                        source_button("firmware-unknown-close", text("CLOSE"), false, false, cx)
                            .on_click(cx.listener(|this, _, window, cx| this.close(window, cx))),
                    ),
            )
            .into_any_element()
    }
    fn preview_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        let preview = self.flow.preview();
        v_flex()
            .id("firmware-preview-controls")
            .test_support()
            .w_full()
            .gap(css(10.))
            .p(css(12.))
            .text_size(css(12.))
            .child(
                h_flex()
                    .flex_wrap()
                    .gap(css(12.))
                    .child(
                        surface::select(&self.scenes)
                            .items(choices(SCENES))
                            .accessibility_label("固件更新预览状态")
                            .w(css(300.)),
                    )
                    .when(preview, |view| {
                        view.child(
                            surface::select(&self.presets)
                                .items(choices(PRESETS))
                                .accessibility_label("预览设备连接")
                                .w(css(280.)),
                        )
                    }),
            )
            .child(if preview {
                "本地界面预览：设备、版本及进度均为示例，不会下载或写入固件。"
            } else {
                "选择本地预览可查看完整更新流程。"
            })
            .when(self.flow.stage == Stage::SwitchDeviceMode, |view| {
                view.child(
                    h_flex()
                        .gap(css(12.))
                        .flex_wrap()
                        .child(
                            source_button(
                                "firmware-connect-preview",
                                "预览：收到单个目标连接",
                                false,
                                false,
                                cx,
                            )
                            .w(css(190.))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.flow.connection_changed = true;
                                this.flow.matching_devices = 1;
                                cx.notify();
                            })),
                        )
                        .child(format!(
                            "预览匹配设备：{}；连接已切换：{}",
                            self.flow.matching_devices,
                            if self.flow.connection_changed {
                                "是"
                            } else {
                                "否"
                            }
                        )),
                )
            })
            .when(
                matches!(self.flow.stage, Stage::Prepare | Stage::Upgrade),
                |view| {
                    view.child(
                        h_flex()
                            .gap(css(12.))
                            .child(
                                source_button(
                                    "firmware-fail-preview",
                                    "预览：更新失败",
                                    false,
                                    false,
                                    cx,
                                )
                                .w(css(130.))
                                .on_click(cx.listener(
                                    |this, _, window, cx| {
                                        this.fail(Warning::UpdateFailed, window, cx)
                                    },
                                )),
                            )
                            .child(
                                source_button(
                                    "firmware-disconnect-preview",
                                    "预览：设备断开",
                                    false,
                                    false,
                                    cx,
                                )
                                .w(css(130.))
                                .on_click(cx.listener(
                                    |this, _, window, cx| {
                                        this.fail(Warning::Disconnected, window, cx)
                                    },
                                )),
                            ),
                    )
                },
            )
            .into_any_element()
    }
    fn warning(&self, warning: Warning, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let waiting = warning == Warning::WaitForOtherDevices;
        let width = if waiting { 440. } else { 480. };
        let panel = v_flex()
            .id("firmware-blocking-warning")
            .test_support()
            .occlude()
            .w(css(width))
            .min_h(css(if waiting { 74. } else { 162. }))
            .px(css(if waiting { 20. } else { 34. }))
            .py(css(22.))
            .items_center()
            .justify_center()
            .rounded(css(3.))
            .border_1()
            .border_color(Colors::warning())
            .bg(Colors::tile())
            .text_color(Colors::foreground())
            .text_size(css(14.))
            .line_height(css(21.))
            .text_center()
            .when_some(warning.title(), |view, key| {
                view.child(
                    h_flex()
                        .justify_center()
                        .mb(css(20.))
                        .gap(css(10.))
                        .child(
                            Icon::new(IconName::TriangleAlert)
                                .size(css(20.))
                                .text_color(Colors::warning()),
                        )
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .text_color(Colors::warning())
                                .child(text(key)),
                        ),
                )
            })
            .child(text(warning.body()).replace("{{productName}}", &self.name()))
            .when(!waiting, |view| {
                view.child(
                    h_flex()
                        .justify_center()
                        .gap(css(16.))
                        .mt(css(24.))
                        .when(warning.cancellable(), |view| {
                            view.child(
                                source_button(
                                    "firmware-warning-cancel",
                                    text("CANCEL_UPDATE"),
                                    false,
                                    false,
                                    cx,
                                )
                                .w(css(130.))
                                .h(css(28.))
                                .rounded(css(4.))
                                .on_click(cx.listener(
                                    |this, _, window, cx| {
                                        this.dismiss_warning(window, cx);
                                        this.close(window, cx);
                                    },
                                )),
                            )
                        })
                        .child(
                            source_button(
                                "firmware-warning-continue",
                                text(if warning == Warning::DeviceIsUpgrading {
                                    "CONTINUE_WITH_UPDATE"
                                } else {
                                    "RETRY"
                                }),
                                true,
                                false,
                                cx,
                            )
                            .w(css(172.))
                            .h(css(28.))
                            .rounded(css(4.))
                            .on_click(cx.listener(
                                |this, _, window, cx| {
                                    // ri.ignoreError hides the blocking warning; Start Over owns retry.
                                    this.dismiss_warning(window, cx);
                                },
                            )),
                        ),
                )
            });
        Dialog::new(cx)
            .focus_handle(self.warning_focus.clone())
            .close_on_backdrop_press(false)
            .on_ok(|_, _, _| false)
            .on_close(|_, _, _| {})
            .backdrop(div().absolute().inset_0().bg(Colors::border().opacity(0.6)))
            .popup(
                div()
                    .absolute()
                    .inset_0()
                    .w(window.viewport_size().width)
                    .h(window.viewport_size().height)
                    .flex()
                    .justify_center()
                    .items_start()
                    .pt(css(306.))
                    .child(panel),
            )
            .into_any_element()
    }
}

fn source_button(
    id: &'static str,
    label: impl Into<SharedString>,
    primary: bool,
    disabled: bool,
    cx: &App,
) -> Button {
    let label = label.into().to_uppercase();
    Button::new(id)
        .disabled(disabled)
        .accessibility_label(label.clone())
        .w(css(90.))
        .h(css(27.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .p_0()
        .rounded(css(3.))
        .text_size(css(12.))
        .line_height(css(14.))
        .bg(if primary {
            Colors::highlight()
        } else {
            Colors::button_gray()
        })
        .text_color(if primary {
            Colors::background()
        } else {
            PaletteColors.white()
        })
        .opacity(if disabled { 0.5 } else { 1. })
        .hover(|style| style.opacity(if disabled { 0.5 } else { 0.7 }))
        .focus_visible(|style| style.opacity(0.7).border_1().border_color(cx.theme().ring))
        .child(label)
}

impl Render for FirmwareUpdate {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match self.flow.stage {
            Stage::Unknown => self.unknown(cx),
            Stage::Launch => self.welcome(cx),
            Stage::Prepare | Stage::ReadyToUpgrade | Stage::Upgrade => self.progress(window, cx),
            Stage::PartialCompleteUpgrade => self.partial(cx),
            Stage::SwitchDeviceMode => self.switch_mode(cx),
            Stage::CompleteDone | Stage::CompleteSkipOrFail => self.result(cx),
        };
        v_flex()
            .id("firmware-update-page")
            .test_support()
            .track_focus(&self.focus)
            .size_full()
            .bg(Colors::background())
            .text_color(Colors::foreground())
            .text_size(css(14.))
            .line_height(css(20.))
            .when(self.preview_enabled, |view| view.child(self.preview_controls(cx)))
            .child(
                div()
                    .id("firmware-update-scroll")
                    .flex_1()
                    .min_h_0()
                    .scrollable_both()
                    .child(
                        div()
                            .min_w(css(664.))
                            .min_h(css(540.))
                            .m(css(12.))
                            .border_t_2()
                            .border_color(Colors::border())
                            .child(
                                div()
                                    .w(css(640.))
                                    .pt(css(32.))
                                    .pb(css(40.))
                                    .mx_auto()
                                    .child(body),
                            ),
                    ),
            )
            .when_some(self.flow.warning, |view, warning| {
                view.child(self.warning(warning, window, cx))
            })
    }
}

impl super::AppShell {
    pub(super) fn open_firmware_update(
        &mut self,
        device: Option<Device>,
        preview: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some((page, _)) = &self.firmware_update {
            if !page.update(cx, |page, cx| page.allow_close(window, cx)) {
                self.navigate(super::Location::FirmwareUpdate, window, cx);
                return;
            }
        }
        self.initialize_firmware_update(device, preview, window, cx);
        self.navigate(super::Location::FirmwareUpdate, window, cx);
    }
    pub(super) fn initialize_firmware_update(
        &mut self,
        device: Option<Device>,
        preview: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let page = cx.new(|cx| FirmwareUpdate::new(device, preview, window, cx));
        if preview {
            page.update(cx, |page, cx| {
                page.scenes.update(cx, |state, cx| {
                    state.set_selected_value(&"welcome".to_owned(), window, cx)
                });
                page.choose("welcome", window, cx);
            });
        }
        let subscription =
            cx.subscribe_in(&page, window, |this, _, _: &CloseRequested, window, cx| {
                this.close_host_tab(super::host_tabs::HostTab::FirmwareUpdate, window, cx);
            });
        self.firmware_update = Some((page, subscription));
    }
}
