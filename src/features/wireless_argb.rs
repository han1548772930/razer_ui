//! Current product 3884/3886 ARGB controller pages. Hardware status is session-only;
//! editable layout preferences are local drafts, independently of discovery.
use super::Choice;
use crate::{
    i18n,
    model::Device,
    ui::{
        hover_tip::{SourceTipPlacement, source_hover_tip, source_hover_tip_element},
        surface,
    },
};
use gpui_kit::base::{Easing, NumberInput, StepAction, step_value};
// 提供 `Div` 的悬停监听（源 `Gu` 用 `onMouseEnter`/`onMouseLeave` 直接翻转 `isMounted`）。
use gpui_kit::StatefulInteractiveElement as _;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    select::{SelectEvent, SelectState},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::OnceLock, time::Duration};

/// The source recolors one extracted layer per icon from `:hover`/`:active`
/// rules (`#multipleBrightness .icon-detection:hover .detect-b{fill:#44d62c}`,
/// `.icon-refreshing:hover path{fill:#44d62c}`, `.icon-power:hover rect{fill:
/// #7de36c}`, `.icon-warning:hover path{fill:#feab59}` …).  GPUI does not run
/// that stylesheet, so each layer asset carries the audited geometry with
/// `currentColor` paint and the wrapper supplies the idle/hover/pressed colors.
#[derive(IntoElement)]
struct SourceIcon {
    id: ElementId,
    layer: SharedString,
    /// Static layer painted underneath, for icons whose glyph keeps its own
    /// color (`.icon-power path{fill:#111}`).
    base: Option<SharedString>,
    idle: Hsla,
    hover: Hsla,
    /// `None` keeps the hover color while pressed, which is what an icon
    /// without an `:active` rule keeps showing.
    pressed: Option<Hsla>,
}
fn icon_layer(layer: SharedString, size: f32) -> AnyElement {
    div()
        .absolute()
        .inset_0()
        .flex()
        .justify_center()
        .items_center()
        .child(
            svg()
                .path(layer)
                .w(surface::css(size))
                .h(surface::css(size)),
        )
        .into_any_element()
}
impl RenderOnce for SourceIcon {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let wrapper = div()
            .id(self.id)
            .relative()
            .size(surface::css(20.))
            .text_color(self.idle)
            .hover(|style| style.text_color(self.hover))
            .children(
                self.base
                    .map(|base| img(base).absolute().inset_0().size_full()),
            )
            .child(icon_layer(self.layer, 20.));
        match self.pressed {
            Some(pressed) => wrapper
                .active(move |style| style.text_color(pressed))
                .into_any_element(),
            None => wrapper.into_any_element(),
        }
    }
}

/// The source's disabled auto-detection icon is a three-part SVG animation:
/// `@keyframes zoomout` scales the glyph from .8 over 50ms, `zoomoutc` expands
/// the `detect-c` square from .4 to 2 while its `#44d62c` paint fades, and
/// `zoomoutf` expands the `#666` `detect-f` square from .2 to 1.4 (both 700ms).
/// The enabled icon instead plays `zoomin` on the glyph (100ms, opacity 0 -> 1,
/// scale 1.8 -> 1).  Every layer keeps the audited geometry and colors; GPUI
/// owns the timing here so the local page does not turn the interaction into a
/// static swap.
#[derive(IntoElement)]
struct AutoDetectionIcon {
    id: u32,
    active: bool,
    generation: u64,
}
/// CSS `ease-out` is cubic-bezier(0, 0, .58, 1), which `Easing::EaseOut`
/// samples exactly.
fn ease_out(phase: f32) -> f32 {
    Easing::EaseOut.sample(phase.clamp(0., 1.))
}
impl AutoDetectionIcon {
    fn asset(&self, name: &str) -> SharedString {
        SharedString::from(format!("synapse/wireless-argb-{}-{name}.svg", self.id))
    }
}
/// One animated layer: the extracted square is centered in the 20px icon box,
/// so animating its size reproduces `transform:scale()` around
/// `transform-origin:center` while the fill fades through opacity.
fn expanding_layer(asset: SharedString, scale: f32, opacity: f32) -> AnyElement {
    div()
        .absolute()
        .inset_0()
        .flex()
        .justify_center()
        .items_center()
        .opacity(opacity)
        .child(
            svg()
                .path(asset)
                .w(surface::css(20. * scale))
                .h(surface::css(20. * scale)),
        )
        .into_any_element()
}
impl RenderOnce for AutoDetectionIcon {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let reduce = cx.reduce_motion();
        let active = self.active;
        let generation = self.generation;
        let (idle, hover, pressed) = if active {
            (
                Colors::primary(),
                Colors::ring_hover(),
                Colors::icon_pressed(),
            )
        } else {
            (
                Colors::foreground(),
                Colors::primary(),
                Colors::icon_pressed(),
            )
        };
        let wrapper = div()
            .id(SharedString::from(format!("argb-auto-icon-{}", self.id)))
            .relative()
            .size(surface::css(20.))
            .text_color(idle)
            .hover(|style| style.text_color(hover))
            .active(move |style| style.text_color(pressed));
        if active {
            // The ring carries the hover/pressed stroke; the glyph is fixed by
            // `.icon-detection--active .detect-b{fill:#111}`.
            let stack = div()
                .absolute()
                .inset_0()
                .child(icon_layer(self.asset("auto_on-ring"), 20.));
            let glyph = self.asset("auto_on-glyph");
            if reduce {
                return wrapper
                    .child(stack.child(icon_layer(glyph, 20.)))
                    .into_any_element();
            }
            return wrapper
                .child(stack.child(div().absolute().inset_0().with_animation(
                    SharedString::from(format!("wireless-argb-auto-on-{generation}")),
                    Animation::new(Duration::from_millis(100)).with_easing(ease_out),
                    move |layer, phase| {
                        let scale = 1.8 - 0.8 * ease_out(phase);
                        layer.child(expanding_layer(glyph.clone(), scale, ease_out(phase)))
                    },
                )))
                .into_any_element();
        }
        // Only a detection that has actually been requested plays the source's
        // `icon-detection--animation` expansion (`isFirstRender` stays static).
        if reduce || generation == 0 {
            return wrapper
                .child(icon_layer(self.asset("auto_off-glyph"), 20.))
                .into_any_element();
        }
        let green_layer = self.asset("auto_off-green");
        let gray_layer = self.asset("auto_off-gray");
        let glyph = self.asset("auto_off-glyph");
        wrapper
            .child(div().absolute().inset_0().with_animation(
                SharedString::from(format!("wireless-argb-auto-off-{generation}")),
                Animation::new(Duration::from_millis(700)).with_easing(ease_out),
                move |layers, phase| {
                    let eased = ease_out(phase);
                    layers
                        .child(expanding_layer(
                            green_layer.clone(),
                            0.4 + 1.6 * eased,
                            1. - eased,
                        ))
                        .child(expanding_layer(
                            gray_layer.clone(),
                            0.2 + 1.2 * eased,
                            1. - eased,
                        ))
                },
            ))
            .child(div().absolute().inset_0().with_animation(
                SharedString::from(format!("wireless-argb-auto-off-glyph-{generation}")),
                Animation::new(Duration::from_millis(50)).with_easing(ease_out),
                move |layer, phase| {
                    layer.child(expanding_layer(
                        glyph.clone(),
                        0.8 + 0.2 * ease_out(phase),
                        1.,
                    ))
                },
            ))
            .into_any_element()
    }
}

mod preview;
mod state;
mod theme;
pub(crate) use preview::open_preview;
use state::{Observation, Port};
use theme::Colors;

#[derive(Deserialize)]
struct Spec {
    product_id: u32,
    page: String,
    translations: BTreeMap<String, BTreeMap<String, String>>,
    fan_values: Vec<u32>,
}
fn spec(pid: u32) -> &'static Spec {
    static DATA: OnceLock<Vec<Spec>> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("wireless_argb_data.json")).expect("audited ARGB sources")
    })
    .iter()
    .find(|s| s.product_id == pid)
    .expect("supported ARGB product")
}
impl Spec {
    fn text(&self, key: &str) -> String {
        self.translations
            .get(&i18n::locale())
            .and_then(|v| v.get(key))
            .or_else(|| self.translations.get("en").and_then(|v| v.get(key)))
            .cloned()
            .unwrap_or_else(|| i18n::t(key))
    }
    fn minimum(&self) -> u32 {
        if self.product_id == 3884 { 1 } else { 4 }
    }
    fn asset(&self, name: &str) -> SharedString {
        format!("synapse/wireless-argb-{}-{name}.png", self.product_id).into()
    }
    /// `.icon-refreshing` / `.icon-power` / `.icon-warning` /
    /// `.icon-close-glitter` keep their audited geometry in an extracted
    /// `currentColor` layer; the interaction colors are the source's own
    /// `:hover`/`:active` declarations.
    fn icon(&self, name: &str) -> AnyElement {
        let (idle, hover, pressed) = match name {
            "refresh" => (
                Colors::foreground(),
                Colors::primary(),
                Some(Colors::icon_pressed()),
            ),
            "warning" => (
                Colors::warning(),
                Colors::warning_hover(),
                Some(Colors::warning_pressed()),
            ),
            "remove" => (
                Colors::foreground(),
                Colors::remove_hover(),
                // `.icon-close-glitter` declares no `:active` color.
                None,
            ),
            "power" => (
                Colors::primary(),
                Colors::power_hover(),
                // `.icon-power` declares no `:active` color, so a pressed
                // button keeps its hover color.
                None,
            ),
            "power_off" => (Colors::power_off(), Colors::power_off_hover(), None),
            // `.port-name-container .port-edit-icon` has no hover/active color,
            // so the pencil keeps its extracted `#44d62c` paint.
            "rename" => {
                return img(self.plain_icon("rename"))
                    .size(surface::css(20.))
                    .into_any_element();
            }
            other => unreachable!("audited wireless ARGB icon: {other}"),
        };
        SourceIcon {
            id: ElementId::Name(format!("argb-icon-{}-{name}", self.product_id).into()),
            layer: SharedString::from(format!(
                "synapse/wireless-argb-{}-{name}-hover.svg",
                self.product_id
            )),
            base: matches!(name, "power" | "power_off").then(|| {
                SharedString::from(format!(
                    "synapse/wireless-argb-{}-{name}-base.svg",
                    self.product_id
                ))
            }),
            idle,
            hover,
            pressed,
        }
        .into_any_element()
    }
    /// Extracted icon as declared by the artwork manifest.
    fn plain_icon(&self, name: &str) -> SharedString {
        SharedString::from(format!(
            "synapse/wireless-argb-{}-{name}.svg",
            self.product_id
        ))
    }
    fn mode_choices(&self) -> Vec<Choice> {
        vec![
            Choice::new("strip", self.text("TEXT_LED_STRIP")),
            Choice::new("fan", self.text("TEXT_FAN")),
        ]
    }
    fn fan_choices(&self) -> Vec<Choice> {
        self.fan_values
            .iter()
            .map(|v| Choice::new(v.to_string(), v.to_string()))
            .collect()
    }
}
pub(crate) fn supports_page(pid: u32, key: &str) -> bool {
    matches!(pid, 3884 | 3886) && spec(pid).page == key
}
pub(crate) struct WirelessArgbChanged;
impl EventEmitter<WirelessArgbChanged> for WirelessArgb {}

/// 源 `Gu` 的两个 `isMounted` 提示各由自己的状态位控制
/// （`toggleTooltipDetection` / `toggleTooltipRefresh`），这里同样分开。
#[derive(Clone, Copy, PartialEq)]
enum ArgbIcon {
    Detection,
    Refresh,
}

pub(crate) struct WirelessArgb {
    spec: &'static Spec,
    ports: Vec<Port>,
    observation: Observation,
    active_ports: Vec<u32>,
    detected: BTreeMap<u32, u32>,
    auto_detection: bool,
    max_leds: u32,
    limit_dismissed: bool,
    preview: bool,
    reveal_editor: bool,
    alert: Option<String>,
    last_request: Option<String>,
    names: BTreeMap<u32, Entity<InputState>>,
    modes: BTreeMap<u32, Entity<SelectState<Vec<Choice>>>>,
    counts: BTreeMap<(u32, u32), Entity<InputState>>,
    fans: BTreeMap<(u32, u32), Entity<SelectState<Vec<Choice>>>>,
    editing_name: Option<u32>,
    /// `.port-item:hover .icon-close-glitter{display:inline-block}` with
    /// `.port-item .icon-close-glitter{display:none;margin-left:-10px}`: the
    /// remove glyph is absent — and so takes no space — until its own
    /// `.port-item` row is hovered.
    hovered_segment: Option<(u32, u32)>,
    /// 源里检测/刷新图标的提示由鼠标进入/离开直接切换挂载，没有任何展示延迟。
    hovered_icon: Option<ArgbIcon>,
    /// 源 `Gu position:"bottom-right"` 的 LED 数量提示同样按端口即时挂载。
    hovered_detected: Option<u32>,
    subscriptions: Vec<Subscription>,
    syncing: bool,
    next_id: u32,
    auto_animation: u64,
}
impl WirelessArgb {
    fn render_product(&self, cx: &Context<Self>) -> AnyElement {
        let visible = self.observation.power_on() && self.observation != Observation::Protection;
        // 检测图标（`#icon-detection-wrapper` + 自身 `isMounted` 提示）。
        // `StatefulInteractiveElement` 只对带 `id` 的元素成立（源里这两个提示的目标也都
        // 是带 id 的 `#icon-detection-wrapper` / `#icon-refreshing-wrapper`）。
        let detection = div().id("icon-detection-wrapper").relative().child(
            Button::new("argb-auto")
                .ghost()
                .small()
                .child(AutoDetectionIcon {
                    id: self.spec.product_id,
                    active: self.auto_detection,
                    generation: self.auto_animation,
                })
                .selected(self.auto_detection)
                .accessibility_label(self.spec.text("GLITTER_MESSAGE_AUTO_DETECTION"))
                .on_click(
                    cx.listener(|this, _, _, cx| this.request("CHANGE_AUTO_DETECTION_STATUS", cx)),
                ),
        );
        let detection = detection
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.hovered_icon = hovered.then_some(ArgbIcon::Detection);
                cx.notify();
            }))
            .when(self.hovered_icon == Some(ArgbIcon::Detection), |row| {
                row.child(source_hover_tip(
                    "argb-auto-tip",
                    self.spec.text("GLITTER_MESSAGE_AUTO_DETECTION"),
                    SourceTipPlacement::BottomLeft,
                ))
            });
        // 刷新图标（`#icon-refreshing-wrapper` + 自身 `isMounted` 提示）。
        let refresh = div().id("icon-refreshing-wrapper").relative().child(
            Button::new("argb-refresh")
                .ghost()
                .small()
                .child(self.spec.icon("refresh"))
                .accessibility_label(self.spec.text("GLITTER_MESSAGE_REFRESH_ICON"))
                .on_click(cx.listener(|this, _, _, cx| this.request("REFRESH_PORTS", cx))),
        );
        let refresh = refresh
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.hovered_icon = hovered.then_some(ArgbIcon::Refresh);
                cx.notify();
            }))
            .when(self.hovered_icon == Some(ArgbIcon::Refresh), |row| {
                row.child(source_hover_tip(
                    "argb-refresh-tip",
                    self.spec.text("GLITTER_MESSAGE_REFRESH_ICON"),
                    SourceTipPlacement::BottomLeft,
                ))
            });
        let product = div()
            .relative()
            .w(surface::css(260.))
            .h(surface::css(275.))
            .child(
                img(self.spec.asset("prd-3x"))
                    .size(surface::css(260.))
                    .object_fit(ObjectFit::Contain),
            )
            .when(self.observation == Observation::Detecting, |view| {
                view.child(
                    img(SharedString::from(format!(
                        "synapse/wireless-argb-{}-detecting.svg",
                        self.spec.product_id
                    )))
                    .absolute()
                    .top(surface::css(33.))
                    .left(surface::css(20.))
                    .w(surface::css(220.))
                    .h(surface::css(115.)),
                )
            })
            .child(
                v_flex()
                    .absolute()
                    .bottom(surface::css(127.))
                    .left_0()
                    .w_full()
                    .h(surface::css(115.))
                    .justify_between()
                    .when(self.observation == Observation::Detecting, |view| {
                        view.invisible()
                    })
                    .child(
                        h_flex()
                            .justify_center()
                            .gap(surface::css(20.))
                            .when(!visible, |s| s.invisible())
                            .child(detection)
                            .child(refresh),
                    )
                    .child(
                        h_flex().justify_center().child(
                            Button::new("argb-power")
                                .ghost()
                                .small()
                                .child(self.spec.icon(if self.observation.power_on() {
                                    "power"
                                } else {
                                    "power_off"
                                }))
                                .selected(self.observation.power_on())
                                .accessibility_label(
                                    self.spec.text("GLITTER_MESSAGE_STAND_BY_MODE"),
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.request("CHANGE_POWER_STATUS", cx)
                                })),
                        ),
                    ),
            );
        h_flex()
            .justify_center()
            .m(surface::css(10.))
            .child(product)
            .into_any_element()
    }
    fn render_status(&self, cx: &Context<Self>) -> AnyElement {
        let old = self.spec.product_id == 3886;
        let observation = if !old
            && self.observation == Observation::Ready
            && !self.limit_dismissed
            && self
                .ports
                .iter()
                .filter(|p| self.active_ports.contains(&p.id))
                .map(Port::total)
                .sum::<u32>()
                > 240
        {
            Observation::LedLimit
        } else {
            self.observation
        };
        let (title, body) = match observation {
            Observation::Unavailable => {
                return h_flex()
                    .justify_center()
                    .child(surface::note("暂时无法读取控制器状态。", cx))
                    .into_any_element();
            }
            Observation::Detecting => {
                return h_flex()
                    .justify_center()
                    .child(surface::note("正在检测 ARGB 设备…", cx))
                    .into_any_element();
            }
            Observation::Empty => (
                if old {
                    "GLITTER_NO_DEVICES_DETECT"
                } else {
                    "GLITTER_NO_DEVICES_DETECTED"
                },
                if old {
                    "GLITTER_NO_DEVICES_DETECT_MSG"
                } else {
                    "GLITTER_NO_DEVICES_DETECTED_MSG"
                },
            ),
            Observation::Standby => (
                "GLITTER_MESSAGE_STAND_BY_MODE",
                "GLITTER_MESSAGE_STAND_BY_MODE_MSG",
            ),
            Observation::Mobile => ("", "GLITTER_MESSAGE_STOP_MOBILE_SYNC"),
            Observation::Bluetooth => (
                "GLITTER_MESSAGE_BLUETOOTH_MODE",
                "GLITTER_MESSAGE_BLUETOOTH_MODE_MSG",
            ),
            Observation::DcRequired => (
                "GLITTER_CONNECTION_REQUIRED",
                "GLITTER_CONNECTION_REQUIRED_MSG",
            ),
            Observation::Protection => (
                "GLITTER_PROTECTION_ENABLED",
                "GLITTER_PROTECTION_ENABLED_MSG_0",
            ),
            Observation::LedLimit => (
                if old {
                    "GLITTER_LIMIT_EXCEEDED"
                } else {
                    "GLITTER_LED_LIMIT_EXCEEDED"
                },
                if old {
                    "GLITTER_LIMIT_EXCEEDED_MSG"
                } else {
                    "GLITTER_LED_LIMIT_EXCEEDED_MSG"
                },
            ),
            Observation::Ready => return div().into_any_element(),
        };
        let mut card = v_flex()
            .w(surface::css(600.))
            .p(surface::css(20.))
            .gap(surface::css(10.))
            .rounded(surface::css(5.))
            .bg(Colors::panel())
            .border_1()
            .border_color(
                if matches!(
                    self.observation,
                    Observation::Mobile | Observation::Bluetooth
                ) {
                    Colors::message_border()
                } else {
                    Colors::warning()
                },
            )
            .text_center();
        if !title.is_empty() {
            card = card.child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.spec.text(title)),
            );
        }
        card = card.child(div().px(surface::css(40.)).child(self.spec.text(body)));
        if self.observation == Observation::Protection {
            for ix in 1..=4 {
                card = card.child(div().text_left().px(surface::css(40.)).child(format!(
                        "{ix}. {}",
                        self.spec
                            .text(&format!("GLITTER_PROTECTION_ENABLED_MSG_{ix}"))
                    )));
            }
        }
        if self.observation == Observation::Mobile {
            card = card.child(
                Button::new("argb-connect")
                    .label(self.spec.text("TEXT_CONNECT"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.request("CHANGE_CONNECTION_MODE:2.4G", cx)
                    })),
            );
        }
        if observation == Observation::LedLimit {
            card = card.child(
                Button::new("argb-dismiss-limit")
                    .ghost()
                    .label(self.spec.text("DISMISS"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.preview {
                            this.observation = Observation::Ready;
                            this.limit_dismissed = true;
                        }
                        cx.notify();
                    })),
            );
        }
        h_flex().justify_center().child(card).into_any_element()
    }
    fn render_port(&self, port: &Port, cx: &Context<Self>) -> AnyElement {
        let id = port.id;
        let mut card = v_flex()
            .relative()
            .w(surface::css(600.))
            .min_h(surface::css(199.))
            .px(surface::css(40.))
            .pt(surface::css(26.))
            .pb(surface::css(30.))
            .rounded(surface::css(5.))
            .bg(Colors::panel());
        let name = if self.editing_name == Some(id) {
            Input::new(&self.names[&id])
                .w(surface::css(200.))
                .into_any_element()
        } else {
            Button::new(("argb-rename", id))
                .ghost()
                .small()
                .label(port.name.clone())
                .child(self.spec.icon("rename"))
                .font_family("RazerF5")
                .text_size(surface::css(16.))
                .text_color(Colors::primary())
                .on_click(cx.listener(move |this, _, window, cx| {
                    if this.editable() {
                        this.editing_name = Some(id);
                        this.names[&id].update(cx, |s, cx| s.focus(window, cx));
                        cx.notify();
                    }
                }))
                .into_any_element()
        };
        // 源把 `.help` 与 `.tip` 直接放在 `.port-container.widget` 里：
        // `<div className="help"/><div className="tip">{getTextItem(OT.gt9)}</div>`，
        // CSS 为 `.widget .help{background-color:#4a4a4a;border-radius:50%;height:14px;
        // position:absolute;right:10px;top:10px;width:14px}`。共享的
        // `surface::help_control` 已经实现该控件与 `.widget .tip`，这里只补回绝对定位。
        card = card.child(h_flex().justify_between().child(name)).child(
            div()
                .absolute()
                .right(surface::css(10.))
                .top(surface::css(10.))
                .child(surface::help_control(
                    ("argb-help", id),
                    self.spec.text("GLITTER_TIP_HELP_PORT_MESSAGE"),
                )),
        );
        let mut rows = v_flex().flex_1().min_w_0();
        for (ix, segment) in port.segments().iter().enumerate() {
            let sid = segment.id;
            let device = if ix == 0 {
                surface::select(&self.modes[&id])
                    .items(self.spec.mode_choices())
                    .accessibility_label(self.spec.text("TEXT_DEVICE_TYPE"))
                    .w(surface::css(150.))
                    .into_any_element()
            } else {
                div()
                    .w(surface::css(150.))
                    .child(format!(
                        "{} {}",
                        self.spec.text(if port.strip_mode {
                            "TEXT_LED_STRIP"
                        } else {
                            "TEXT_FAN"
                        }),
                        ix + 1
                    ))
                    .into_any_element()
            };
            let count = if port.strip_mode {
                let owner = cx.entity().downgrade();
                div()
                    .w(surface::css(70.))
                    .h(surface::css(27.))
                    .border_1()
                    .border_color(Colors::border())
                    .child(
                        NumberInput::new(&self.counts[&(id, sid)])
                            .size_full()
                            .controls_right()
                            .input(
                                Input::new(&self.counts[&(id, sid)])
                                    .appearance(false)
                                    .bordered(false)
                                    .focus_bordered(false)
                                    .h_full()
                                    .pl_1()
                                    .text_sm(),
                            )
                            .on_step(move |action, window, cx| {
                                let _ = owner.update(cx, |this, cx| {
                                    this.step_leds(id, sid, action, window, cx)
                                });
                            }),
                    )
                    .into_any_element()
            } else {
                surface::select(&self.fans[&(id, sid)])
                    .items(self.spec.fan_choices())
                    .accessibility_label(self.spec.text("GLITTER_NO_OF_LED"))
                    .w(surface::css(70.))
                    .into_any_element()
            };
            if ix == 0 {
                rows = rows.child(
                    h_flex()
                        .gap(surface::css(20.))
                        .mb(surface::css(10.))
                        .child(
                            div()
                                .w(surface::css(150.))
                                .child(self.spec.text("TEXT_DEVICE_TYPE")),
                        )
                        .child(self.spec.text("GLITTER_NO_OF_LED")),
                );
            }
            rows = rows.child(
                h_flex()
                    // `MU` renders one `.port-item` per strip/bend and mounts
                    // `pU` (the `.icon-close-glitter` svg) only for `index !== 0`,
                    // so the hover state is tracked per row, not per port.
                    .id(SharedString::from(format!("argb-port-item-{id}-{sid}")))
                    .gap(surface::css(20.))
                    .my(surface::css(5.))
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        let next = hovered.then_some((id, sid));
                        if this.hovered_segment != next {
                            this.hovered_segment = next;
                            cx.notify();
                        }
                    }))
                    .child(device)
                    .child(count)
                    .when(ix > 0 && self.hovered_segment == Some((id, sid)), |row| {
                        row.child(
                            Button::new(SharedString::from(format!("argb-remove-{id}-{sid}")))
                                .ghost()
                                .small()
                                // `.icon-close-glitter{margin-left:-10px}` pulls
                                // the revealed glyph toward its row content.
                                .ml(surface::css(-10.))
                                .child(self.spec.icon("remove"))
                                .accessibility_label(i18n::t("REMOVE"))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.remove(id, sid, window, cx)
                                })),
                        )
                    }),
            );
        }
        if (port.strip_mode && port.strip.len() < 4)
            || (!port.strip_mode && self.spec.product_id == 3884)
        {
            let label = self.spec.text(if port.strip_mode {
                "GLITTER_ADD_BEND"
            } else {
                "GLITTER_ADD_FAN"
            });
            // `.port-add-bend` is an inline-flex 27px `#707070` row
            // (`.common-font-styles` gives it Roboto 14px). 3884 draws
            // `.underline`'s 1px `:after` rule (`bottom:.6px;
            // background-color:#707070`) that follows the text to `#44d62c` on
            // `.underline:hover`; 3886 declares no `.underline` rules and
            // underlines `.port-add-bend` itself, without a hover color.
            let text = if self.spec.product_id == 3884 {
                div()
                    .relative()
                    .border_b_1()
                    .border_color(Colors::add_bend())
                    .group(SharedString::from(format!("argb-add-bend-{id}")))
                    .group_hover(SharedString::from(format!("argb-add-bend-{id}")), |style| {
                        style
                            .text_color(Colors::primary())
                            .border_color(Colors::primary())
                    })
                    .child(label)
                    .into_any_element()
            } else {
                div().underline().child(label).into_any_element()
            };
            rows = rows.child(
                Button::new(("argb-add", id))
                    .ghost()
                    .small()
                    .h(surface::css(27.))
                    .my(surface::css(5.))
                    .font_family("Roboto")
                    .text_size(surface::css(14.))
                    .text_color(Colors::add_bend())
                    .child(text)
                    .disabled(!port.strip_mode && port.total() >= self.max_leds)
                    .on_click(cx.listener(move |this, _, window, cx| this.add(id, window, cx))),
            );
        }
        let detected = self.detected.get(&id).copied().unwrap_or(0);
        if port.segments().len() > 1 {
            rows = rows.child(
                h_flex()
                    .gap(surface::css(10.))
                    .h(surface::css(27.))
                    .child(
                        div()
                            .w(surface::css(150.))
                            .child(self.spec.text("GLITTER_TOTAL_LED_COUNT")),
                    )
                    .child(port.total().to_string()),
            );
        }
        // 源 3884/3886：`Gu position:"bottom-right"`，内容由
        // `getTextItem(OT.vml, {ledCount: '<span style="color:#44d62c">N</span>'})` 生成
        // （只有数字是主题绿），并且按悬停即时挂载。
        rows = rows.child(
            div()
                .id(("argb-detected-wrapper", id))
                .relative()
                .child(
                    Button::new(("argb-detected", id))
                        .ghost()
                        .small()
                        .label(format!("{detected} LED")),
                )
                .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                    this.hovered_detected = hovered.then_some(id);
                    cx.notify();
                }))
                .when(self.hovered_detected == Some(id), |row| {
                    let template = self.spec.text("GLITTER_DETECTED_LED_COUNT");
                    let count = detected.to_string();
                    let (prefix, suffix) = match template.split_once("{{ledCount}}") {
                        Some((prefix, suffix)) => (prefix.to_owned(), suffix.to_owned()),
                        None => (template.clone(), String::new()),
                    };
                    row.child(source_hover_tip_element(
                        ("argb-detected-tip", id),
                        SourceTipPlacement::BottomRight,
                        h_flex()
                            .child(prefix)
                            .child(div().text_color(cx.theme().primary).child(count))
                            .child(suffix),
                    ))
                }),
        );
        let asset = if port.strip_mode {
            format!("strip-{}", port.strip.len())
        } else {
            "fan".into()
        };
        card = card.child(
            h_flex()
                .items_start()
                .mt(surface::css(11.))
                .gap(surface::css(20.))
                .child(rows)
                .child(
                    img(self.spec.asset(&asset))
                        .size(surface::css(100.))
                        .flex_shrink_0(),
                ),
        );
        if !port.dismissed && port.total() != detected {
            card = card.child(
                v_flex()
                    .mt(surface::css(10.))
                    .p(surface::css(20.))
                    .border_1()
                    .border_color(Colors::notice_border())
                    .rounded(surface::css(5.))
                    .text_size(surface::css(13.))
                    .child(self.spec.text("GLITTER_MESSAGE_CHROMA_STUDIO"))
                    .child(
                        h_flex()
                            .justify_between()
                            .mt(surface::css(10.))
                            .child(
                                Button::new(("argb-chroma", id))
                                    .outline()
                                    .small()
                                    .label(self.spec.text("DASHBOARD_CHROMA_STUDIO"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.request("LAUNCH_CHROMA_STUDIO", cx)
                                    })),
                            )
                            .child(
                                Button::new(("argb-dismiss", id))
                                    .ghost()
                                    .small()
                                    .label(self.spec.text("DISMISS"))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Some(p) = this.ports.iter_mut().find(|p| p.id == id)
                                        {
                                            p.dismissed = true;
                                        }
                                        cx.notify();
                                    })),
                            ),
                    ),
            );
        }
        let limit = if self.spec.product_id == 3884 {
            self.max_leds
        } else {
            120
        };
        if port.total() > limit {
            card = card.child(
                div()
                    .mt(surface::css(10.))
                    .text_color(Colors::warning())
                    .child(if self.spec.product_id == 3884 {
                        format!(
                            "{} {}",
                            self.spec
                                .text("GLITTER_MESSAGE_EXCEEDED_WARNING_1")
                                .replace("{{maxLed}}", &limit.to_string()),
                            self.spec.text("GLITTER_MESSAGE_EXCEEDED_WARNING_2")
                        )
                    } else {
                        self.spec.text("GLITTER_MESSAGE_EXCEEDED_WARNING")
                    }),
            );
        }
        card.into_any_element()
    }
}
impl WirelessArgb {
    pub(crate) fn new(device: &Device, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::for_product(device.product_id, window, cx)
    }
    fn for_product(pid: u32, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let spec = spec(pid);
        let mut this = Self {
            spec,
            ports: (1..=3)
                .map(|id| Port::new(id, format!("{} {id}", spec.text("TEXT_PORT"))))
                .collect(),
            observation: Observation::Unavailable,
            active_ports: vec![],
            detected: BTreeMap::new(),
            auto_detection: false,
            max_leds: 80,
            limit_dismissed: false,
            preview: false,
            reveal_editor: false,
            alert: None,
            last_request: None,
            names: BTreeMap::new(),
            modes: BTreeMap::new(),
            counts: BTreeMap::new(),
            fans: BTreeMap::new(),
            editing_name: None,
            hovered_segment: None,
            hovered_icon: None,
            hovered_detected: None,
            subscriptions: vec![],
            syncing: false,
            next_id: 1,
            auto_animation: 0,
        };
        this.rebuild(window, cx);
        this
    }
    pub(crate) fn snapshot(&self) -> Value {
        json!({"ports":self.ports})
    }
    pub(crate) fn restore(
        &mut self,
        saved: Option<&Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ports = (1..=3)
            .map(|id| Port::new(id, format!("{} {id}", self.spec.text("TEXT_PORT"))))
            .collect();
        if let Some(saved) = saved
            .and_then(|s| s.get("ports"))
            .and_then(|v| serde_json::from_value::<Vec<Port>>(v.clone()).ok())
        {
            for port in &mut self.ports {
                if let Some(candidate) = saved.iter().find(|p| p.id == port.id) {
                    let valid = !candidate.name.trim().is_empty()
                        && candidate.name.chars().count() <= 128
                        && !candidate.strip.is_empty()
                        && candidate.strip.len() <= 4
                        && !candidate.fan.is_empty()
                        && candidate.fan.len() <= 16
                        && candidate
                            .strip
                            .iter()
                            .all(|s| s.value >= self.spec.minimum() && s.value <= 240)
                        && candidate
                            .fan
                            .iter()
                            .all(|s| self.spec.fan_values.contains(&s.value));
                    if valid {
                        *port = candidate.clone();
                        port.dismissed = false;
                        if self.spec.product_id == 3884 {
                            port.name = port.name.chars().take(32).collect();
                        } else {
                            port.fan.truncate(1);
                        }
                    }
                }
            }
        }
        // IDs are local retained identities; never trust duplicates from a profile.
        self.next_id = 1;
        for port in &mut self.ports {
            for segment in port.strip.iter_mut().chain(&mut port.fan) {
                segment.id = self.next_id;
                self.next_id += 1;
            }
        }
        self.editing_name = None;
        self.rebuild(window, cx);
        cx.notify();
    }
    pub(crate) fn dismiss(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.editing_name = None;
        self.alert = None;
        cx.notify();
    }
    fn changed(&mut self, cx: &mut Context<Self>) {
        if !self.preview {
            cx.emit(WirelessArgbChanged);
        }
        cx.notify();
    }
    fn editable(&self) -> bool {
        self.preview && self.observation.ports_visible()
    }
    fn maximum(&self, port: u32) -> u32 {
        if self.spec.product_id == 3884 {
            self.max_leds
        } else {
            self.detected.get(&port).copied().unwrap_or(0).max(40)
        }
    }
    fn rebuild(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.subscriptions.clear();
        self.names.clear();
        self.modes.clear();
        self.counts.clear();
        self.fans.clear();
        for port in self.ports.clone() {
            let id = port.id;
            let name = cx.new(|cx| InputState::new(window, cx).default_value(port.name));
            self.subscriptions.push(cx.subscribe_in(
                &name,
                window,
                move |this, input, event, window, cx| {
                    if !this.syncing
                        && matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. })
                    {
                        let value = input.read(cx).value().to_string();
                        this.rename(id, value, window, cx);
                    }
                },
            ));
            self.names.insert(id, name);
            let choices = self.spec.mode_choices();
            let mode = cx.new(|cx| SelectState::new(choices, None, window, cx));
            self.subscriptions.push(cx.subscribe_in(
                &mode,
                window,
                move |this, _, event, window, cx| {
                    if !this.syncing && this.editable() {
                        if let SelectEvent::Confirm(Some(value)) = event {
                            if let Some(port) = this.ports.iter_mut().find(|p| p.id == id) {
                                port.strip_mode = value == "strip";
                                port.dismissed = false;
                            }
                            this.sync(window, cx);
                            this.changed(cx);
                        }
                    }
                },
            ));
            self.modes.insert(id, mode);
            for segment in port.strip {
                let sid = segment.id;
                let input = cx
                    .new(|cx| InputState::new(window, cx).default_value(segment.value.to_string()));
                self.subscriptions.push(cx.subscribe_in(
                    &input,
                    window,
                    move |this, input, event, window, cx| {
                        if !this.syncing
                            && matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. })
                        {
                            if let Ok(value) = input.read(cx).value().parse::<u32>() {
                                this.set_leds(id, sid, value, window, cx);
                            } else {
                                this.sync(window, cx);
                            }
                        }
                    },
                ));
                self.counts.insert((id, sid), input);
            }
            for segment in port.fan {
                let sid = segment.id;
                let choices = self.spec.fan_choices();
                let select = cx.new(|cx| SelectState::new(choices, None, window, cx));
                self.subscriptions.push(cx.subscribe_in(
                    &select,
                    window,
                    move |this, _, event, window, cx| {
                        if !this.syncing {
                            if let SelectEvent::Confirm(Some(value)) = event {
                                if let Ok(value) = value.parse::<u32>() {
                                    this.set_leds(id, sid, value, window, cx);
                                }
                            }
                        }
                    },
                ));
                self.fans.insert((id, sid), select);
            }
        }
        self.sync(window, cx);
    }
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.syncing = true;
        for port in &self.ports {
            self.names[&port.id].update(cx, |s, cx| s.set_value(port.name.clone(), window, cx));
            self.modes[&port.id].update(cx, |s, cx| {
                s.set_selected_value(
                    &if port.strip_mode { "strip" } else { "fan" }.to_string(),
                    window,
                    cx,
                )
            });
            for segment in &port.strip {
                if let Some(input) = self.counts.get(&(port.id, segment.id)) {
                    input.update(cx, |s, cx| {
                        s.set_value(segment.value.to_string(), window, cx)
                    });
                }
            }
            for segment in &port.fan {
                if let Some(select) = self.fans.get(&(port.id, segment.id)) {
                    select.update(cx, |s, cx| {
                        s.set_selected_value(&segment.value.to_string(), window, cx)
                    });
                }
            }
        }
        self.syncing = false;
    }
    fn rename(&mut self, id: u32, value: String, window: &mut Window, cx: &mut Context<Self>) {
        if !self.editable() || self.editing_name != Some(id) {
            return;
        }
        if !value.trim().is_empty() {
            let max = if self.spec.product_id == 3884 {
                32
            } else {
                128
            };
            if let Some(port) = self.ports.iter_mut().find(|p| p.id == id) {
                port.name = value.chars().take(max).collect();
            }
        }
        self.editing_name = None;
        self.sync(window, cx);
        self.changed(cx);
    }
    fn set_leds(
        &mut self,
        id: u32,
        sid: u32,
        value: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.editable() {
            return;
        }
        let minimum = self.spec.minimum();
        let maximum = self.maximum(id);
        if let Some(port) = self.ports.iter_mut().find(|p| p.id == id) {
            if port.strip_mode {
                port.set_leds(sid, value, minimum, maximum);
            } else if self.spec.fan_values.contains(&value) {
                port.set_leds(sid, value, 1, maximum);
            }
        }
        self.sync(window, cx);
        self.changed(cx);
    }
    fn step_leds(
        &mut self,
        id: u32,
        sid: u32,
        action: StepAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(input) = self.counts.get(&(id, sid)) else {
            return;
        };
        let draft = input.read(cx).value();
        let Some(value) = step_value(
            &draft,
            action,
            1.,
            Some(self.spec.minimum() as f64),
            Some(self.maximum(id) as f64),
        )
        .and_then(|v| v.parse::<u32>().ok()) else {
            return;
        };
        self.set_leds(id, sid, value, window, cx);
    }
    fn add(&mut self, id: u32, window: &mut Window, cx: &mut Context<Self>) {
        if !self.editable() {
            return;
        }
        if let Some(port) = self.ports.iter_mut().find(|p| p.id == id) {
            if port.strip_mode && port.strip.len() >= 4 {
                return;
            }
            if !port.strip_mode && (self.spec.product_id != 3884 || port.total() >= self.max_leds) {
                return;
            }
            port.add(self.spec.minimum(), self.next_id);
            self.next_id += 1;
        }
        self.rebuild(window, cx);
        self.changed(cx);
    }
    fn remove(&mut self, id: u32, sid: u32, window: &mut Window, cx: &mut Context<Self>) {
        if !self.editable() {
            return;
        }
        if let Some(port) = self.ports.iter_mut().find(|p| p.id == id) {
            let values = port.segments_mut();
            if values.len() > 1 && values[0].id != sid {
                values.retain(|s| s.id != sid);
            }
            port.dismissed = false;
        }
        self.rebuild(window, cx);
        self.changed(cx);
    }
    fn request(&mut self, kind: &str, cx: &mut Context<Self>) {
        // No service adapter is present: never turn a request into an observation.
        if kind == "CHANGE_AUTO_DETECTION_STATUS" {
            // The source starts its icon transition as soon as the command is
            // issued, before the service reports the next auto-detection
            // value. Keep that visual phase local without inventing hardware
            // state in the unavailable-service path.
            self.auto_animation = self.auto_animation.wrapping_add(1);
        }
        if self.preview {
            self.last_request = Some(kind.into());
        } else {
            self.alert = Some("暂时无法读取控制器状态，请连接设备后重试。".into());
        }
        cx.notify();
    }
}

impl Render for WirelessArgb {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let show_ports = self.observation.ports_visible()
            && (self.spec.product_id == 3884 || self.reveal_editor);
        v_flex()
            .w_full()
            .min_w(surface::css(620.))
            .font_family("Roboto")
            .text_color(Colors::foreground())
            .text_size(surface::css(14.))
            .child(self.render_product(cx))
            .child(self.render_status(cx))
            .when(show_ports, |view| {
                view.child(
                    h_flex()
                        .items_start()
                        .justify_center()
                        .flex_wrap()
                        .gap(surface::css(20.))
                        .mt(surface::css(20.))
                        .children(
                            self.ports
                                .iter()
                                .filter(|p| self.active_ports.contains(&p.id))
                                .map(|port| self.render_port(port, cx)),
                        ),
                )
            })
            .when_some(self.alert.clone(), |view, alert| {
                view.child(
                    h_flex()
                        .justify_center()
                        .mt_3()
                        .child(surface::note(alert, cx)),
                )
            })
    }
}
