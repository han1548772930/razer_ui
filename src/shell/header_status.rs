//! Source Home `x -> T` and shared Header `N/x` in App.eb32d7cd.chunk.js.
//! Host OS, connectivity and EngineUpgrade readback remain unknown. All states
//! in this separate surface are explicitly selected examples, never host facts.
use crate::{
    features::Choice,
    i18n,
    ui::{
        surface,
        theme::{HeaderStatusColors, MainPageColors},
    },
};
use gpui_kit::base::{Button as BaseButton, Dialog as BaseDialog, Popup};
use gpui_kit::component::{
    button::Button,
    select::{SelectEvent, SelectState},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::time::Duration;

actions!(header_status_preview, [DismissStatus]);

const SCENARIOS: &[(&str, &str)] = &[
    ("unknown", "主机状态未读取"),
    ("offline", "网络连接不可用"),
    ("update", "更新就绪，等待重启 Synapse"),
    ("compatibility", "检测到兼容模式"),
];

fn choices() -> Vec<Choice> {
    SCENARIOS
        .iter()
        .map(|(key, label)| Choice::new(*key, *label))
        .collect()
}

pub(super) fn open_preview(window: &mut Window, cx: &mut App) {
    cx.bind_keys([KeyBinding::new(
        "escape",
        DismissStatus,
        Some("HeaderStatusPreview"),
    )]);
    let view = cx.new(|cx| HeaderStatusPreview::new(window, cx));
    window.open_dialog(cx, move |dialog, window, _| {
        dialog
            .title("主应用状态 · 界面预览")
            .width(
                (window.rem_size() * 48.)
                    .min((window.viewport_size().width - window.rem_size() * 2.).max(px(0.))),
            )
            .child(view.clone())
    });
}

struct HeaderStatusPreview {
    selected: Entity<SelectState<Vec<Choice>>>,
    scenario: String,
    offline_open: bool,
    update_open: bool,
    compatibility_open: bool,
    compatibility_focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    offline_timer: Option<Task<()>>,
    notice: String,
    _subscriptions: Vec<Subscription>,
}

impl HeaderStatusPreview {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let selected =
            cx.new(|cx| SelectState::new(choices(), Some(IndexPath::new(0)), window, cx));
        let subscription = cx.subscribe_in(
            &selected,
            window,
            |this: &mut Self, _, event, window, cx| {
                if let SelectEvent::Confirm(Some(key)) = event {
                    this.choose(key, window, cx);
                }
            },
        );
        Self {
            selected,
            scenario: "unknown".into(),
            offline_open: false,
            update_open: false,
            compatibility_open: false,
            compatibility_focus: cx.focus_handle(),
            return_focus: None,
            offline_timer: None,
            notice: String::new(),
            _subscriptions: vec![subscription],
        }
    }

    fn choose(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.close_compatibility(window, cx);
        self.offline_timer = None;
        self.scenario = key.to_owned();
        self.offline_open = key == "offline";
        self.update_open = false;
        self.notice.clear();
        if key == "offline" {
            // N mounts its explanation once for three seconds. This timer only
            // controls the example's visibility; it performs no network probe.
            self.offline_timer = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(Duration::from_secs(3)).await;
                let _ = this.update(cx, |this, cx| {
                    this.offline_open = false;
                    this.offline_timer = None;
                    cx.notify();
                });
            }));
        }
        cx.notify();
    }

    fn dismiss_status(&mut self, _: &DismissStatus, _: &mut Window, cx: &mut Context<Self>) {
        if self.offline_open || self.update_open {
            self.offline_timer = None;
            self.offline_open = false;
            self.update_open = false;
            cx.notify();
        } else {
            cx.propagate();
        }
    }

    fn close_compatibility(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.compatibility_open = false;
        if let Some(focus) = self.return_focus.take() {
            focus.focus(window, cx);
        }
        cx.notify();
    }

    fn open_compatibility(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.return_focus = window.focused(cx);
        self.compatibility_open = true;
        self.compatibility_focus.focus(window, cx);
        cx.notify();
    }

    fn offline(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let trigger = BaseButton::new("header-preview-offline")
            .accessibility_label(i18n::t("ERROR_OFFLINE_2"))
            .aria_description(i18n::t("ERROR_OFFLINE_3"))
            // 55 CSS's final .toolbar override: 46 px by the 38 px toolbar.
            .w(surface::css(46.))
            .h(surface::css(38.))
            .focus_visible(|button| button.border_1().border_color(cx.theme().primary))
            .child(
                div()
                    .id("header-offline-icon")
                    .size(surface::css(32.))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(HeaderStatusColors::status_surface())
                    .hover(|icon| icon.bg(HeaderStatusColors::offline_hover()))
                    .active(|icon| icon.bg(HeaderStatusColors::offline_hover()))
                    .child(img("synapse/header-offline.svg").size(surface::css(20.))),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.offline_timer = None;
                this.offline_open = true;
                cx.notify();
            }))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.offline_timer = None;
                this.offline_open = *hovered;
                cx.notify();
            }));
        Popup::new("header-preview-offline-popup", trigger)
            .anchor(Anchor::TopRight)
            .offset(window.rem_size() * (10. / 16.))
            .when(self.offline_open, |popup| {
                popup.content(
                    v_flex()
                        .id("header-preview-offline-description")
                        .occlude()
                        .w(surface::css(300.))
                        .p(surface::css(20.))
                        .border_1()
                        .border_color(MainPageColors.tutorial_accent())
                        .rounded(surface::css(3.))
                        .bg(cx.theme().popover)
                        .text_center()
                        .child(
                            div()
                                .text_size(surface::css(16.))
                                .text_color(cx.theme().foreground)
                                .child(i18n::t("ERROR_OFFLINE_2").to_uppercase()),
                        )
                        .child(
                            div()
                                .text_size(surface::css(14.))
                                .text_color(cx.theme().muted_foreground)
                                .child(i18n::t("ERROR_OFFLINE_3")),
                        )
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                            this.offline_timer = None;
                            this.offline_open = false;
                            cx.notify();
                        })),
                )
            })
            .into_any_element()
    }

    fn update(&self, cx: &mut Context<Self>) -> AnyElement {
        let description = format!(
            "{}\n({})",
            i18n::t("UPDATE_READY"),
            i18n::t("RESTART_SYNAPSE_REQUIRED")
        );
        Popup::new(
            "header-preview-update-popup",
            BaseButton::new("header-preview-update")
                .accessibility_label(i18n::t("UPDATE_READY"))
                .aria_description(description)
                .w(surface::css(46.))
                .h(surface::css(38.))
                .bg(HeaderStatusColors::status_surface())
                .hover(|button| button.bg(cx.theme().secondary_hover))
                .focus_visible(|button| button.border_1().border_color(cx.theme().primary))
                .child(img("synapse/header-update.svg").size(surface::css(20.)))
                .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                    this.update_open = *hovered;
                    cx.notify();
                }))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.update_open = true;
                    this.notice = "已预览重启 Synapse 请求；未重启应用或系统。".into();
                    cx.notify();
                })),
        )
        .anchor(Anchor::TopRight)
        .when(self.update_open, |popup| {
            popup.content(
                v_flex()
                    .id("header-preview-update-description")
                    .occlude()
                    .px(surface::css(10.))
                    .py(surface::css(8.))
                    .border_1()
                    .border_color(HeaderStatusColors::update_tooltip_border())
                    .bg(cx.theme().popover)
                    .text_size(surface::css(14.))
                    .text_center()
                    .child(i18n::t("UPDATE_READY"))
                    .child(format!("({})", i18n::t("RESTART_SYNAPSE_REQUIRED")))
                    .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                        this.update_open = false;
                        cx.notify();
                    })),
            )
        })
        .into_any_element()
    }

    fn compatibility(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        if !self.compatibility_open {
            return div().into_any_element();
        }
        let panel = v_flex()
            .id("header-preview-compatibility-warning")
            .occlude()
            .w((window.rem_size() * (500. / 16.))
                .min((window.viewport_size().width - window.rem_size() * 2.).max(px(0.))))
            .min_h(surface::css(184.))
            .px(surface::css(30.))
            .py(surface::css(20.))
            .items_center()
            .justify_center()
            .text_center()
            .bg(cx.theme().popover)
            .border_1()
            .border_color(MainPageColors.tutorial_accent())
            .rounded(surface::css(3.))
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .child(
                h_flex()
                    .justify_center()
                    .gap(surface::css(10.))
                    .mb(surface::css(10.))
                    // Module 27875's inline width/height override the 24 px CSS rule.
                    .child(
                        img("synapse/header-compatibility-warning.svg")
                            .w(surface::css(20.))
                            .h(surface::css(27.)),
                    )
                    .child(
                        div()
                            .text_size(surface::css(16.))
                            .line_height(surface::css(16.))
                            .text_color(MainPageColors.tutorial_accent())
                            .child(i18n::t("COMPATIBILITY_MODE_DETECTED_TITLE").to_uppercase()),
                    ),
            )
            .child(
                div()
                    .mb(surface::css(8.))
                    .child(i18n::t("COMPATIBILITY_MODE_MESSAGE")),
            )
            .child(
                div()
                    .mb(surface::css(12.))
                    .text_color(cx.theme().muted_foreground)
                    .child(i18n::t("COMPATIBILITY_MODE_INSTRUCTIONS")),
            )
            .child(
                BaseButton::new("header-preview-open-properties")
                    .h(surface::css(20.))
                    .flex()
                    .items_center()
                    .gap(surface::css(6.))
                    .focus_visible(|button| button.bg(cx.theme().secondary_hover))
                    .child(i18n::t("COMPATIBILITY_MODE_OPEN_PROPERTIES_LINK"))
                    .child(img("synapse/external-link.svg").size(surface::css(12.)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.notice = "已预览打开 Synapse 属性请求；未修改系统属性。".into();
                        cx.notify();
                    })),
            );
        BaseDialog::new(cx)
            .layer(1, true)
            .focus_handle(self.compatibility_focus.clone())
            .close_on_backdrop_press(false)
            .on_ok(|_, _, _| false)
            // The source warning has no close control; Escape exits only this
            // explicit example and never claims the compatibility issue is fixed.
            .on_close(cx.listener(|this, _, window, cx| this.close_compatibility(window, cx)))
            .backdrop(
                div()
                    .absolute()
                    .inset_0()
                    .bg(cx.theme().title_bar.opacity(0.7)),
            )
            .popup(panel)
            .child(
                div()
                    .absolute()
                    .top(surface::css(16.))
                    .left_0()
                    .w_full()
                    .text_center()
                    .text_size(surface::css(12.))
                    .child(if self.notice.is_empty() {
                        "界面预览 · 按 Esc 返回".to_owned()
                    } else {
                        format!("界面预览 · 按 Esc 返回\n{}", self.notice)
                    }),
            )
            .into_any_element()
    }
}

impl Render for HeaderStatusPreview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .key_context("HeaderStatusPreview")
            .gap(surface::css(16.))
            .w_full()
            .on_action(cx.listener(Self::dismiss_status))
            .child("界面预览：以下是示例状态，未读取当前网络、兼容模式或待安装更新。")
            .child(
                surface::select(&self.selected)
                    .items(choices())
                    .w_full()
                    .accessibility_label("主应用状态样例"),
            )
            .child(
                h_flex()
                    .h(surface::css(38.))
                    .w_full()
                    .bg(cx.theme().background)
                    .child(
                        div()
                            .flex_1()
                            .pl(surface::css(12.))
                            .text_color(cx.theme().muted_foreground)
                            .child("RAZER SYNAPSE"),
                    )
                    .when(self.scenario == "offline", |row| {
                        row.child(self.offline(window, cx))
                    })
                    .when(self.scenario == "update", |row| row.child(self.update(cx))),
            )
            .when(self.scenario == "compatibility", |column| {
                column.child(
                    Button::new("header-preview-show-compatibility")
                        .label("查看兼容模式警告…")
                        .on_click(
                            cx.listener(|this, _, window, cx| this.open_compatibility(window, cx)),
                        ),
                )
            })
            .child(
                div()
                    .min_h(surface::css(110.))
                    .text_size(surface::css(12.))
                    .text_color(cx.theme().muted_foreground)
                    .child(match self.scenario.as_str() {
                        "offline" => "离线说明首次显示 3 秒；悬停或按下状态图标可再次查看。",
                        "update" => "悬停可查看重启说明；按下图标只预览请求。",
                        "compatibility" => "此示例保留原版警告正文与属性入口。Esc 仅退出预览。",
                        _ => "主机状态尚未读取。选择样例以查看条件界面。",
                    }),
            )
            .child(
                div()
                    .min_h(surface::css(20.))
                    .text_size(surface::css(12.))
                    .child(self.notice.clone()),
            )
            .child(self.compatibility(window, cx))
    }
}
