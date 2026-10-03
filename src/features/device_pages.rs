use super::{
    controls::Control,
    lighting_color::LightingColorPicker,
    settings::Effect,
    workspace::{DeviceWorkspace, WorkspaceEvent},
};
use crate::ui::surface::{self, SynapseSwitch as Switch};
use gpui_kit::base::Button as BaseButton;
use gpui_kit::component::{
    button::{Button, ButtonCustomVariant, ButtonVariants},
    checkbox::Checkbox,
    color_picker::ColorPickerState,
    slider::Slider,
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};

pub(super) fn system_button(
    id: &'static str,
    label: &'static str,
    properties: crate::backend::system::Properties,
) -> Button {
    Button::new(id)
        .label(label)
        .outline()
        .icon(gpui_kit::assets::IconName::ExternalLink)
        .on_click(move |_, window, cx| {
            if let Err(error) = crate::backend::system::open(properties) {
                window.push_notification(format!("无法打开系统属性：{error}"), cx);
            }
        })
}
impl DeviceWorkspace {
    pub(super) fn performance_page(&self, cx: &mut Context<Self>) -> AnyElement {
        surface::page_columns()
            .child(surface::page_column(self.sensitivity_panel(cx)))
            .child(surface::page_column(
                v_flex()
                    .gap(surface::css(20.))
                    .when(!self.device().use_ble, |this| {
                        this.child(self.polling_panel(cx))
                    })
                    .child(
                        surface::panel("鼠标属性", cx).child(
                            h_flex()
                                .items_center()
                                .gap(surface::css(20.))
                                .child(
                                    img("synapse/windows-11.svg")
                                        .size(surface::css(44.))
                                        .object_fit(ObjectFit::Contain),
                                )
                                .child(
                                    BaseButton::new("mouse-properties")
                                        .accessibility_label("打开 Windows 鼠标属性")
                                        .gap(surface::css(4.))
                                        .child(
                                            Icon::new(gpui_kit::assets::IconName::ExternalLink)
                                                .size(surface::css(16.)),
                                        )
                                        .child("打开 Windows 鼠标属性")
                                        .border_0()
                                        .px_0()
                                        .h(surface::css(44.))
                                        .underline()
                                        .hover(|s| s.text_color(cx.theme().primary))
                                        .focus_visible(|s| s.text_color(cx.theme().primary))
                                        .on_click(|_, window, cx| {
                                            if let Err(error) = crate::backend::system::open(
                                                crate::backend::system::Properties::Mouse,
                                            ) {
                                                window.push_notification(
                                                    format!("无法打开系统属性：{error}"),
                                                    cx,
                                                );
                                            }
                                        }),
                                ),
                        ),
                    ),
            ))
            .into_any_element()
    }

    pub(super) fn polling_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        surface::panel("回报率", cx)
            .gap(surface::css(10.))
            .child(
                div()
                    .mt(surface::css(6.))
                    .child("每秒向电脑报告设备状态的次数。"),
            )
            .child(h_flex().gap(surface::css(10.)).flex_wrap().children(
                self.poll_rates().iter().map(|rate| {
                    let rate = *rate;
                    BaseButton::new(SharedString::from(format!("polling-{rate}")))
                        .accessibility_label(rate.to_string())
                        .child(rate.to_string())
                        .selected(self.settings().polling == rate)
                        .flex()
                        .items_center()
                        .justify_center()
                        .w(surface::css(72.))
                        .h(surface::css(27.))
                        .p_0()
                        .text_size(surface::css(14.))
                        .rounded(cx.theme().font_size * (3. / 16.))
                        .bg(cx.theme().background)
                        .text_color(cx.theme().foreground)
                        .border_1()
                        .border_color(if self.settings().polling == rate {
                            cx.theme().primary
                        } else {
                            cx.theme().border
                        })
                        .hover(|s| s.border_color(cx.theme().primary))
                        .focus_visible(|s| s.border_color(cx.theme().primary))
                        .on_click(
                            cx.listener(move |this, _, w, cx| {
                                this.edit(w, cx, |s| s.polling = rate)
                            }),
                        )
                }),
            ))
            .when(self.settings().polling > 1000, |this| {
                this.child(surface::note("较高的回报率会增加性能和功耗需求。", cx))
            })
            .into_any_element()
    }

    // Original CD/SM range: optional value bubble, 6px rail at bottom 25px,
    // and endpoint labels. Retain SliderState and the framework's drag/focus behavior.
    fn source_range(
        &self,
        key: Control,
        minimum: &str,
        middle: Option<&str>,
        maximum: &str,
        disabled: bool,
        cx: &App,
    ) -> AnyElement {
        let state = &self.controls.sliders[&key];
        let no_tip = middle.is_some();
        let position = state.read(cx).percentage().end;
        div()
            .id(SharedString::from(format!("control-{key:?}")))
            .relative()
            .w_full()
            .h(surface::css(if no_tip { 36. } else { 64. }))
            .when(disabled, |s| s.opacity(0.3))
            .when(!no_tip, |s| {
                s.child(
                    div()
                        .absolute()
                        .bottom(surface::css(42.))
                        .left(relative(position))
                        .ml(surface::css(-20.))
                        .w(surface::css(40.))
                        .py(surface::css(4.))
                        .rounded(surface::css(3.))
                        .text_center()
                        .text_size(surface::css(12.))
                        .bg(cx.theme().primary)
                        .text_color(cx.theme().primary_foreground)
                        .child(format!("{}", state.read(cx).value().start())),
                )
            })
            .child(
                div()
                    .absolute()
                    .bottom(surface::css(16.))
                    .w_full()
                    .child(Slider::new(state).disabled(disabled)),
            )
            .child(
                h_flex()
                    .absolute()
                    .bottom_0()
                    .w_full()
                    .justify_between()
                    .text_size(surface::css(14.))
                    .child(minimum.to_owned())
                    .when_some(middle, |s, value| s.child(value.to_owned()))
                    .child(maximum.to_owned()),
            )
            .into_any_element()
    }

    pub(super) fn calibration_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let tracking = &self.settings().tracking;
        // 182 LD renders the welcome and the fixed-width widget as direct
        // children of `.body-widgets.flex`, not inside a 600px widget column.
        h_flex()
            .id("smart-tracking-content")
            .test_support()
            .w_full()
            .max_w(surface::css(surface::BODY_MAX_WIDTH))
            .mx_auto()
            .flex_wrap()
            .items_start()
            .justify_center()
            .when(!self.intro_seen, |this| {
                this.child(
                    div()
                        .id("tracking-welcome")
                        .test_support()
                        .flex()
                        .flex_grow(1.)
                        .flex_shrink(1.)
                        .justify_center()
                        .mb(surface::css(20.))
                        .child(
                            v_flex()
                                .id("tracking-intro")
                                .test_support()
                                .relative()
                                .px(surface::css(30.))
                                .py(surface::css(20.))
                                .rounded(surface::css(5.))
                                .border_1()
                                .border_color(cx.theme().secondary_hover)
                                .bg(cx.theme().secondary_hover)
                                .child(
                                    Button::new("tracking-intro-dismiss")
                                        .absolute()
                                        .top_0()
                                        .right_0()
                                        .size(surface::css(36.))
                                        .border_0()
                                        .rounded_none()
                                        .p_0()
                                        .accessibility_label("关闭校准介绍")
                                        .custom(ButtonCustomVariant::new(cx))
                                        .child(
                                            div()
                                                .group("calibration-close")
                                                .relative()
                                                .size_full()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .child(
                                                    img("synapse/calibration-close.svg")
                                                        .size(surface::css(20.))
                                                        .group_active("calibration-close", |s| {
                                                            s.opacity(0.)
                                                        }),
                                                )
                                                .child(
                                                    img("synapse/calibration-close-active.svg")
                                                        .absolute()
                                                        .top(surface::css(8.))
                                                        .left(surface::css(8.))
                                                        .size(surface::css(20.))
                                                        .opacity(0.)
                                                        .group_hover("calibration-close", |s| {
                                                            s.opacity(1.)
                                                        })
                                                        .group_active("calibration-close", |s| {
                                                            s.opacity(0.7)
                                                        }),
                                                ),
                                        )
                                        .on_click(cx.listener(|this, event, window, cx| {
                                            // The keyboard trigger is about to disappear;
                                            // continue to the existing calibration controls.
                                            // Pointer dismissal keeps the prior focus.
                                            if matches!(event, ClickEvent::Keyboard(_)) {
                                                window.focus_next(cx);
                                            }
                                            this.intro_seen = true;
                                            cx.emit(WorkspaceEvent::IntroDismissed);
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    div()
                                        .font_family("RazerF5")
                                        .text_size(surface::css(26.))
                                        .font_weight(FontWeight::LIGHT)
                                        .line_height(surface::css(30.))
                                        .text_color(cx.theme().primary)
                                        .text_center()
                                        .mb(surface::css(10.))
                                        .child(
                                            crate::i18n::t("MOUSE_MAT_CALIBRATION_HEADER")
                                                .to_uppercase(),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_size(surface::css(14.))
                                        .line_height(surface::css(16.))
                                        .text_color(cx.theme().foreground)
                                        .text_center()
                                        .child(crate::i18n::t("MOUSE_MAT_CALIBRATION")),
                                ),
                        ),
                )
            })
            .child(
                surface::panel(crate::i18n::t_or("SMARTTRACKING", "智能追踪"), cx)
                    .id("smart-tracking-widget")
                    .test_support()
                    .flex_grow(0.)
                    .flex_shrink_0()
                    .w(surface::css(surface::WIDGET_WIDTH))
                    .min_w(surface::css(surface::WIDGET_WIDTH))
                    .max_w(surface::css(surface::WIDGET_WIDTH))
                    .my(surface::css(10.))
                    .mx_auto()
                    .gap_0()
                    .child(div().mt(surface::css(16.)).mb(surface::css(20.)).child(
                        crate::i18n::t_or(
                            "SMART_TRACKING_DISC",
                            "无论使用何种表面，都可以根据喜好设置单个中止点。",
                        ),
                    ))
                    .child(
                        h_flex()
                            .items_center()
                            .gap(surface::css(5.))
                            .child(
                                Checkbox::new("asymmetric-cutoff")
                                    .label(crate::i18n::t_or(
                                        "ENABLEASYMMETRICCUTOFF",
                                        "启用非对称中止",
                                    ))
                                    .checked(tracking.asymmetric)
                                    .on_change(cx.listener(|this, value, w, cx| {
                                        this.edit(w, cx, |s| s.tracking.asymmetric = *value)
                                    })),
                            )
                            .child(
                                Button::new("asymmetric-help")
                                    .ghost()
                                    .border_0()
                                    .p_0()
                                    .size(surface::css(16.))
                                    .accessibility_label("非对称中止说明")
                                    .tooltip(crate::i18n::t_or(
                                        "SMARTTRACKINGTOOLTIP1",
                                        "分别调整抬升和着陆距离。",
                                    ))
                                    .child(img("synapse/help-default.svg").size_full()),
                            ),
                    )
                    .when(!tracking.asymmetric, |this| {
                        this.child(
                            div()
                                .mt(surface::css(20.))
                                .mb(surface::css(10.))
                                .child(crate::i18n::t_or("TRACKINGDISTANCE", "追踪距离")),
                        )
                        .child(self.source_range(
                            Control::Tracking,
                            "低",
                            Some("中"),
                            "高",
                            false,
                            cx,
                        ))
                    })
                    .when(tracking.asymmetric, |this| {
                        this.child(
                            div()
                                .mt(surface::css(20.))
                                .mb(surface::css(10.))
                                .child(crate::i18n::t_or("LIFTOFFDISTANCE", "抬升距离")),
                        )
                        .child(self.source_range(Control::Lift, "低", None, "高", false, cx))
                        .child(
                            div()
                                .mt(surface::css(20.))
                                .mb(surface::css(10.))
                                .child(crate::i18n::t_or("LANDINGDISTANCE", "着陆距离")),
                        )
                        .child(self.source_range(Control::Landing, "低", None, "高", false, cx))
                        .child(
                            surface::note(
                                crate::i18n::t_or(
                                    "WARNING_SETTING_LANDING_DISTANCE",
                                    "将着陆距离设置得过低可能导致表面不兼容。",
                                ),
                                cx,
                            )
                            .mt(surface::css(10.))
                            .mb(surface::css(20.)),
                        )
                    })
                    .child(
                        div()
                            .mt(surface::css(20.))
                            .mb(surface::css(10.))
                            .child("重置"),
                    )
                    .child(div().mb(surface::css(10.)).child(crate::i18n::t_or(
                        "RESET_DESC1",
                        "通过同时按住左右鼠标键和滚轮 7 秒钟，可手动重置鼠标。",
                    ))),
            )
            .into_any_element()
    }

    pub(super) fn power_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = self.settings();
        let headset = self.pid() == 777;
        let power_panel = if headset {
            surface::panel_with_control(
                crate::i18n::t_or("AUDIO_POWER_SAVING_HEADER", "节能"),
                Switch::new("power-enabled")
                    .accessibility_label("节能")
                    .checked(state.power_enabled)
                    .on_change(cx.listener(|this, value, w, cx| {
                        this.edit(w, cx, |s| s.power_enabled = *value)
                    })),
                cx,
            )
        } else {
            surface::panel(crate::i18n::t_or("POWER_SAVING_HEADER", "无线节能"), cx)
        };
        surface::page_columns()
            .child(surface::page_column(
                power_panel
                    .gap(surface::css(10.))
                    .child(div().mt(surface::css(6.)).child(if headset {
                        crate::i18n::t_or(
                            "AUDIO_POWER_SAVING_DESC",
                            "以电池供电时，在无活动（分钟）后，设备将会关闭。",
                        )
                    } else {
                        crate::i18n::t_or("POWER_SAVING_DESC", "闲置以下时间（分钟）后进入睡眠模式")
                    }))
                    .child(self.source_range(
                        Control::Idle,
                        if headset { "5" } else { "1" },
                        None,
                        if headset { "60" } else { "15" },
                        headset && !state.power_enabled,
                        cx,
                    )),
            ))
            .when(!headset, |this| {
                this.child(surface::page_column(
                    surface::panel(crate::i18n::t_or("LOW_POWER_MODE_HEADER", "低能耗模式"), cx)
                        .gap(surface::css(10.))
                        .child(div().mt(surface::css(6.)).child(crate::i18n::t_or(
                            "LOW_POWER_MODE_DESC",
                            "当电池电量低于以下百分比时，进入低能耗模式。",
                        )))
                        .child(self.source_range(
                            Control::LowPower,
                            "5%",
                            None,
                            "100%",
                            state.polling > 1000,
                            cx,
                        ))
                        .when(state.polling > 1000, |this| {
                            this.child(surface::note(
                                crate::i18n::t_or(
                                    "LOW_POWER_MODE_WARN",
                                    "回报率高于 1000 Hz 时无法调整此选项。",
                                ),
                                cx,
                            ))
                        }),
                ))
            })
            .into_any_element()
    }
    pub(super) fn lighting_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let lighting = &self.settings().lighting;
        let columns = surface::page_columns()
            .child(surface::page_column(
                v_flex()
                    .flex_1()
                    .gap_5()
                    .child(
                        surface::panel_with_control(crate::i18n::t_or("BRIGHTNESS_HEADER", "亮度"),
                                Switch::new("brightness-enabled")
                                    .accessibility_label("亮度")
                                    .checked(lighting.enabled)
                                    .on_change(cx.listener(|this, value, w, cx| {
                                        this.edit(w, cx, |s| s.lighting.enabled = *value)
                                    })), cx)
                            .gap_0()
                            .child(self.source_range(Control::Brightness, "0", None, "100", !lighting.enabled, cx)),
                    )
                    .when(self.pid() == 653 || crate::product::audited_mouse_mat(self.pid()).is_some(), |this| {
                        this.child(
                            surface::panel("关闭灯光", cx).gap(surface::css(20.))
                                .child(
                                    Checkbox::new("lighting-display-off")
                                        .label("显示器关闭时")
                                        .checked(lighting.display_off)
                                        .disabled(!lighting.enabled)
                                        .on_change(cx.listener(|this, value, w, cx| {
                                            this.edit(w, cx, |s| s.lighting.display_off = *value)
                                        })),
                                )
                                .when(self.pid() == 653, |panel| panel.child(
                                    Checkbox::new("lighting-idle-enabled")
                                        .label("闲置时关闭灯光")
                                        .checked(lighting.idle_enabled)
                                        .disabled(!lighting.enabled)
                                        .on_change(cx.listener(|this, value, w, cx| {
                                            this.edit(w, cx, |s| s.lighting.idle_enabled = *value)
                                        })),
                                )
                                .child(self.source_range(Control::LightingIdle, "1", None, "15",
                                    !lighting.enabled || !lighting.idle_enabled, cx))),
                        )
                    })
                    .when(self.pid() == 777, |this| {
                        this.child(
                            surface::panel(crate::i18n::t_or("STREAM_REACTIVE_LIGHTING_HEADER", "感应式直播灯光效果"), cx)
                                .gap(surface::css(20.))
                                .child(
                                    div().relative().w_full().h(surface::css(180.))
                                        .child(img("synapse/stream-777.png")
                                            .w_full().h_full().object_fit(ObjectFit::Contain))
                                        .child(div().absolute().top(surface::css(133.)).w_full()
                                            .flex().justify_center()
                                            .child(surface::external("streamer-companion",
                                                crate::i18n::t_or("INSTALL_STREAMER_COMPANION_APP", "安装 Streamer Companion 应用程序"),
                                                "https://www.razer.com/streamer-companion-app")
                                                .primary().h(surface::css(30.)).text_size(surface::css(12.)))),
                                )
                                .child(div().text_size(surface::css(14.)).line_height(surface::css(17.))
                                    .child(crate::i18n::t_or("STREAM_REACTIVE_LIGHTING_MSG",
                                        "为你的直播带来观众互动体验。自定义通知和事件触发时的灯光效果。"))),
                        )
                    }),
            ))
            .child(surface::page_column(
                surface::panel("效果", cx).gap_0()
                    .child(
                        h_flex().id("lighting-mode-tabs").self_start()
                            .h(surface::css(36.)).p(surface::css(5.)).border_1()
                            .border_color(cx.theme().border).rounded(surface::css(18.))
                            .bg(cx.theme().group_box).hover(|s| s.border_color(cx.theme().primary))
                            .child(
                                self.lighting_mode_button("lighting-quick", !lighting.advanced, cx)
                                    .label("快速效果")
                                    .on_click(cx.listener(|this, _, w, cx| {
                                        this.edit(w, cx, |s| s.lighting.advanced = false)
                                    })),
                            )
                            .child(
                                self.lighting_mode_button("lighting-advanced", lighting.advanced, cx)
                                    .label("高级效果")
                                    .on_click(cx.listener(|this, _, w, cx| {
                                        this.edit(w, cx, |s| s.lighting.advanced = true)
                                    })),
                            ),
                    )
                    .when(!lighting.advanced, |this| {
                        this.child(div().mt(surface::css(20.)).mb(surface::css(20.))
                            .text_size(surface::css(14.)).line_height(surface::css(17.))
                            .child(crate::i18n::t_or("QUICK_EFFECTS_MSG",
                                "快速效果是可以保存到设备配置文件的预设效果，可与其他支持 Razer Chroma 的设备同步。")))
                        .child(h_flex().gap(surface::css(20.)).items_center()
                            .child(surface::select(&self.controls.effect)
                                .items(Effect::list(self.pid(), self.device().use_ble, false)
                                    .iter().map(|effect| super::controls::Choice::new(effect.id(), effect.label())).collect())
                                .id("lighting-effect").accessibility_label("快速效果")
                                .w(surface::css(150.)))
                            .child(Button::new("lighting-sync").ghost().disabled(true)
                                .border_0().p_0().h(surface::css(27.))
                                .accessibility_label("同步灯光效果")
                                .tooltip("连接兼容设备后可同步灯光效果")
                                .child(img("synapse/chroma-sync.svg").size(surface::css(26.)))
                                .child(div().text_size(surface::css(14.)).underline().child("同步到其他支持 Chroma 的设备"))))
                        .child(self.effect_parameters(cx))
                    })
                    .when(lighting.advanced, |this| {
                        this.child(h_flex().mt(surface::css(20.)).gap(surface::css(10.)).items_center()
                            .child(img("synapse/chroma-studio.svg").size(surface::css(24.)))
                            .child("Chroma Studio"))
                        .child(div().relative().mt(surface::css(20.)).w_full().h(surface::css(180.))
                            .child(img("synapse/install-chroma.png").size_full().object_fit(ObjectFit::Contain))
                            .child(div().absolute().bottom(surface::css(20.)).w_full().flex().justify_center()
                                .child(surface::external("chroma-information", "了解 Razer Chroma",
                                    "https://www.razer.com/chroma").primary().h(surface::css(27.))
                                    .text_size(surface::css(12.)))))
                        .child(div().mt(surface::css(20.)).text_size(surface::css(14.))
                            .line_height(surface::css(18.)).child(crate::i18n::t_or("ADVANCED_EFFECTS_DETAIL",
                                "通过 Razer Chroma 在多台设备上使用高级效果，并在 Chroma Studio 中创建自定义灯光。")))
                        .child(surface::note("高级效果需要 Razer Chroma。当前未连接该应用。", cx).mt(surface::css(10.)))
                    }),
            ))
            .into_any_element();
        if crate::product::audited_mouse_mat(self.pid()).is_some() {
            v_flex()
                .w_full()
                .min_w(surface::css(1024.))
                .max_w(surface::css(surface::BODY_MAX_WIDTH))
                .mx_auto()
                .child(self.mouse_mat_product_banner(cx))
                // The source image and widgets each have 10px vertical margins.
                .child(
                    div()
                        .mt(surface::css(10.))
                        .mb(surface::css(10.))
                        .child(columns),
                )
                .into_any_element()
        } else {
            columns
        }
    }

    fn mouse_mat_product_banner(&self, cx: &App) -> AnyElement {
        let device = self.device();
        // ProductImage (7855; inline _l for 3073) precedes the lighting columns
        // in every audited mat root. Its normal .widget-prod art has height 250
        // and intrinsic width; the 325px width belongs only to customDotPattern.
        div()
            .id("lighting-product-banner")
            .test_support()
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .w_full()
            .min_w(surface::css(1024.))
            .max_w(surface::css(1220.))
            .h(surface::css(250.))
            .flex_shrink_0()
            .mx_auto()
            .my(surface::css(10.))
            .child(surface::dot_background(cx))
            .when_some(
                crate::resources::device_image(
                    self.pid(),
                    device.edition_id,
                    device.layout_id,
                    crate::resources::DeviceImage::Product,
                ),
                |banner, path| {
                    banner.child(
                        img(path)
                            .id("lighting-product-image")
                            .test_support()
                            .relative()
                            .aria_label(device.display_name())
                            .h(surface::css(250.))
                            .w_auto()
                            .flex_shrink_0(),
                    )
                },
            )
            .into_any_element()
    }
    fn lighting_mode_button(&self, id: &'static str, selected: bool, cx: &App) -> Button {
        Button::new(id)
            .h(surface::css(26.))
            .px(surface::css(10.))
            .py_0()
            .border_0()
            .map(|button| Styled::rounded(button, surface::css(13.)))
            .text_size(surface::css(14.))
            .selected(selected)
            .custom(
                ButtonCustomVariant::new(cx)
                    .color(if selected {
                        cx.theme().primary
                    } else {
                        cx.theme().group_box
                    })
                    .foreground(if selected {
                        cx.theme().primary_foreground
                    } else {
                        cx.theme().foreground
                    })
                    .hover(if selected {
                        cx.theme().primary
                    } else {
                        cx.theme().group_box
                    })
                    .active(if selected {
                        cx.theme().primary
                    } else {
                        cx.theme().secondary_active
                    }),
            )
    }
    fn effect_parameters(&self, cx: &mut Context<Self>) -> AnyElement {
        let lighting = &self.settings().lighting;
        let effect = lighting.effect;
        // Mouse mats derive Reactive availability from the service's validDevices,
        // not from saved mouse profiles or explicit preview devices.
        if effect == Effect::Reactive && crate::product::audited_mouse_mat(self.pid()).is_some() {
            return h_flex()
                .id("lighting-reactive-warning")
                .test_support()
                .items_start()
                .gap(surface::css(10.))
                .mt(surface::css(20.))
                .text_size(surface::css(14.))
                // Source .warning reserves 30px for warning.ad3f47f8.svg:
                // the shared original icon is 20px with a 10px text gap.
                .child(
                    img("synapse/onboard-warning.svg")
                        .size(surface::css(20.))
                        .flex_shrink_0(),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(crate::i18n::t("REACTIVE_WARNING")),
                )
                .into_any_element();
        }
        let params = lighting.params();
        let two_colors = matches!(
            effect,
            Effect::Breathing | Effect::Starlight | Effect::Tidal
        );
        let has_color =
            two_colors || matches!(effect, Effect::Static | Effect::Ripple | Effect::Reactive);
        v_flex()
            .gap_0()
            .when(has_color, |this| {
                this.child(
                    h_flex()
                        .items_start()
                        .flex_wrap()
                        .gap(surface::css(20.))
                        .child(self.lighting_color(
                            &self.controls.color,
                            if two_colors { "颜色 1" } else { "颜色" },
                            two_colors && params.random,
                            !matches!(effect, Effect::Reactive | Effect::Static),
                        ))
                        .when(two_colors, |this| {
                            this.child(self.lighting_color(
                                &self.controls.color2,
                                "颜色 2",
                                params.random,
                                true,
                            ))
                        })
                        .when(two_colors, |this| {
                            this.child(
                                div().mt(surface::css(45.)).child(
                                    Checkbox::new("effect-random")
                                        .label("随机颜色")
                                        .checked(params.random)
                                        .on_change(cx.listener(|this, value, w, cx| {
                                            this.edit(w, cx, |s| {
                                                s.lighting.params_mut().random = *value
                                            })
                                        })),
                                ),
                            )
                        })
                        .when(
                            matches!(effect, Effect::Reactive | Effect::Starlight),
                            |this| {
                                this.child(
                                    v_flex()
                                        .mt(surface::css(20.))
                                        .w(surface::css(230.))
                                        .child("持续时间")
                                        .child(h_flex().mt(surface::css(10.)).w_full().child(
                                            Slider::new(
                                                &self.controls.sliders[&Control::EffectDuration],
                                            ),
                                        ))
                                        .child(
                                            h_flex()
                                                .justify_between()
                                                .w_full()
                                                .text_size(surface::css(14.))
                                                .child("短")
                                                .child("中")
                                                .child("长"),
                                        ),
                                )
                            },
                        ),
                )
            })
            .when(effect == Effect::AudioMeter, |this| {
                this.child(self.color_boost_editor(cx))
            })
            .when(
                matches!(effect, Effect::Wave | Effect::Wheel | Effect::Tidal),
                |this| {
                    this.child(
                        v_flex()
                            // Tidal places direction directly below its color row (mt10).
                            // Wave and Wheel start their parameter region with mt20.
                            .mt(surface::css(if effect == Effect::Tidal { 10. } else { 20. }))
                            .self_start()
                            .child("方向")
                            .child(
                                h_flex()
                                    .mt(surface::css(5.))
                                    .h(surface::css(42.))
                                    .p(surface::css(5.))
                                    .gap(surface::css(5.))
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .rounded(surface::css(20.))
                                    .bg(cx.theme().group_box)
                                    .children({
                                        let choices = match effect {
                                            Effect::Tidal => {
                                                [(1, "向外", "out"), (0, "向内", "in")]
                                            }
                                            Effect::Wheel => {
                                                [(1, "顺时针", "cw"), (2, "逆时针", "ccw")]
                                            }
                                            Effect::Wave if crate::product::audited_mouse_mat(self.pid())
                                                .and_then(|product| product.wave_direction())
                                                == Some(crate::product::MouseMatWaveDirection::ClockwiseCounterclockwise) => {
                                                [(11, "顺时针", "cw"), (12, "逆时针", "ccw")]
                                            }
                                            _ => [(1, "向左", "left"), (2, "向右", "right")],
                                        };
                                        choices.into_iter().map(|(direction, label, asset)| {
                                            let selected = params.direction == direction;
                                            Button::new(SharedString::from(format!(
                                                "effect-direction-{direction}"
                                            )))
                                            .accessibility_label(label)
                                            .tooltip(label)
                                            .selected(selected)
                                            .w(surface::css(40.))
                                            .h(surface::css(30.))
                                            .p_0()
                                            .border_0()
                                            .map(|button| Styled::rounded(button, surface::css(15.)))
                                            .custom(
                                                ButtonCustomVariant::new(cx)
                                                    .color(if selected {
                                                        cx.theme().primary
                                                    } else {
                                                        cx.theme().group_box
                                                    })
                                                    .hover(if selected {
                                                        cx.theme().primary
                                                    } else {
                                                        cx.theme().group_box
                                                    })
                                                    .active(cx.theme().primary),
                                            )
                                            .child(
                                                img(SharedString::from(format!(
                                                    "synapse/direction-{asset}{}.svg",
                                                    if selected { "-active" } else { "" }
                                                )))
                                                .size(surface::css(20.)),
                                            )
                                            .on_click(cx.listener(move |this, _, w, cx| {
                                                this.edit(w, cx, |s| {
                                                    s.lighting.params_mut().direction = direction
                                                })
                                            }))
                                        })
                                    }),
                            ),
                    )
                },
            )
            .when(effect == Effect::Ambient, |this| {
                this.child(div().mt(surface::css(20.)).child("采样屏幕区域"))
                    .child(
                        h_flex()
                            .mt(surface::css(30.))
                            .gap(surface::css(6.))
                            .children(
                                [
                                    ("full", "整个屏幕"),
                                    ("left", "左侧"),
                                    ("top", "顶部"),
                                    ("right", "右侧"),
                                    ("bottom", "底部"),
                                ]
                                .into_iter()
                                .map(|(screen, label)| {
                                    Button::new(SharedString::from(format!(
                                        "effect-screen-{screen}"
                                    )))
                                    .accessibility_label(label)
                                    .tooltip(label)
                                    .selected(params.screen == screen)
                                    .w(surface::css(24.))
                                    .h(surface::css(16.))
                                    .p_0()
                                    .rounded(px(0.))
                                    .border_1()
                                    .border_color(if params.screen == screen {
                                        cx.theme().primary
                                    } else {
                                        cx.theme().border
                                    })
                                    .custom(ButtonCustomVariant::new(cx))
                                    .child(
                                        div().relative().size_full().child(
                                            div()
                                                .absolute()
                                                .bg(cx.theme().foreground)
                                                .when(screen == "full", |s| s.inset_0())
                                                .when(screen == "left", |s| {
                                                    s.left_0()
                                                        .top_0()
                                                        .bottom_0()
                                                        .w(surface::css(4.))
                                                })
                                                .when(screen == "right", |s| {
                                                    s.right_0()
                                                        .top_0()
                                                        .bottom_0()
                                                        .w(surface::css(4.))
                                                })
                                                .when(screen == "top", |s| {
                                                    s.left_0().right_0().top_0().h(surface::css(4.))
                                                })
                                                .when(screen == "bottom", |s| {
                                                    s.left_0()
                                                        .right_0()
                                                        .bottom_0()
                                                        .h(surface::css(4.))
                                                }),
                                        ),
                                    )
                                    .on_click(cx.listener(move |this, _, w, cx| {
                                        this.edit(w, cx, |s| {
                                            s.lighting.params_mut().screen = screen.into()
                                        })
                                    }))
                                }),
                            ),
                    )
            })
            .into_any_element()
    }
    fn lighting_color(
        &self,
        state: &Entity<ColorPickerState>,
        label: &'static str,
        disabled: bool,
        allow_none: bool,
    ) -> AnyElement {
        v_flex()
            .mt(surface::css(20.))
            .w(surface::css(53.))
            .gap(surface::css(5.))
            .text_size(surface::css(14.))
            .child(label)
            .child(
                LightingColorPicker::new(state, label)
                    .disabled(disabled)
                    .allow_none(allow_none),
            )
            .into_any_element()
    }
}
