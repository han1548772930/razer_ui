use gpui_kit::base::motion::{self, Easing, Transition};
use gpui_kit::base::{Button as BaseButton, Presence};
use gpui_kit::component::{
    button::{Button, ButtonCustomVariant, ButtonVariants},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::time::Duration;

use super::source_tooltip::SourceTooltip;

pub(crate) use super::synapse_select::{select, select_alexa};

#[cfg(test)]
#[path = "surface_tests.rs"]
mod tests;

pub(crate) const BODY_MIN_WIDTH: f32 = 600.;
pub(crate) const BODY_MAX_WIDTH: f32 = 1240.;
pub(crate) const WIDGET_WIDTH: f32 = 600.;
pub(crate) const WIDGET_GAP: f32 = 20.;
pub(crate) const COLUMN_STACK_MAX_WIDTH: f32 = 1279.;
pub(crate) const COMPACT_COLUMN_MARGIN: f32 = 30.;
pub(crate) const WIDGET_PADDING_X: f32 = 40.;
pub(crate) const WIDGET_PADDING_Y: f32 = 30.;
pub(crate) const WIDGET_RADIUS: f32 = 5.;
pub(crate) const CONFIG_WRAPPER_MIN_WIDTH: f32 = 770.;
pub(crate) const CONFIG_WRAPPER_MAX_WIDTH: f32 = 1220.;

// Reference CSS uses a 16px root. Keep its ratios through GPUI's rem scale.
pub(crate) fn css(value: f32) -> Rems {
    rems(value / 16.)
}

/// Source media queries use viewport CSS pixels; normalize our rem-scaled UI
/// back to the source's 16px root before applying the 1279px breakpoint.
pub(crate) fn stacked_device_columns(viewport_width: f32, root_font_size: f32) -> bool {
    viewport_width * 16. / root_font_size.max(1.) <= COLUMN_STACK_MAX_WIDTH
}

/// Current source `.nav-tabs .nav`, including the more specific
/// `.nav-tabs .nav.active:hover` rule. Base Button owns input and focus.
pub(crate) fn navigation_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    cx: &App,
) -> gpui_kit::base::Button {
    let label = label.into().to_uppercase();
    gpui_kit::base::Button::new(id)
        .accessibility_label(label.clone())
        .selected(selected)
        .h(css(28.))
        .px(css(10.))
        .py(css(7.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .text_size(css(12.))
        .line_height(css(14.))
        .rounded(css(14.))
        .border_0()
        .bg(if selected {
            cx.theme().primary
        } else {
            cx.theme().transparent
        })
        .text_color(if selected {
            cx.theme().primary_foreground
        } else {
            cx.theme().muted_foreground
        })
        .hover(|style| {
            style
                .bg(if selected {
                    cx.theme().primary
                } else {
                    cx.theme().secondary_hover
                })
                .text_color(if selected {
                    cx.theme().primary_foreground
                } else {
                    cx.theme().foreground
                })
        })
        .when(!selected, |button| {
            button.active(|style| {
                style
                    .bg(super::theme::NavigationColors::pressed())
                    .text_color(cx.theme().primary_foreground)
            })
        })
        .focus_visible(|style| style.border_1().border_color(cx.theme().primary))
        .styles(|styles| styles.disabled(|style| style.opacity(0.3)))
        .child(label)
}

/// `.profile-bar` 的宽度：源码 `.nav-tabs .profile-bar{width:auto}`，实测布局里
/// profile 入口 26 + 下拉 230 + 两侧 10 边距 + 更多按钮 26 + 右边距 10 + OBM 26。
/// （`has_obm` 时才多出 OBM 的 32px：26 + 10 外边距 − 4px 重叠。）
pub(crate) const PROFILE_BAR_WIDTH: f32 = 322.;
pub(crate) const PROFILE_BAR_WIDTH_OBM: f32 = 354.;
/// CSS pixels used by an optional battery (text + 26px icon + 2*5px margin)
/// and a help item (24px + 10px margin). Text measurement is normalized to rem.
pub(crate) fn device_right_width(
    device: &crate::model::Device,
    has_help: bool,
    window: &Window,
) -> f32 {
    let battery = if device.has_battery || device.dashboard.readonly_values.is_some() {
        device.current_power_status().map_or(0., |power| {
            let text = if power.level >= 0 {
                format!("{} %", power.level)
            } else {
                "-".into()
            };
            label_width(&text, 14., window) + 26. + 10.
        })
    } else {
        0.
    };
    battery + if has_help { 24. + 10. } else { 0. }
}

pub(crate) fn nav_left() -> Div {
    gpui_kit::component::h_flex()
        .flex_grow(1.)
        .flex_shrink(0.)
        .flex_basis(relative(0.25))
}

pub(crate) fn nav_right() -> Div {
    gpui_kit::component::h_flex()
        .flex_grow(1.)
        .flex_shrink(1.)
        .flex_basis(relative(0.25))
        .items_center()
        .justify_end()
}

/// `.hover-border` 方框（26×26、`border:1px solid #222`、圆角 13、20px 图标、
/// `margin-right:10px`）的悬停/按下状态：源码带
/// `transition:border-color .2s;will-change:border-color`，
/// `:hover{border-color:#5d5d5d}`、`.active,:active{border-color:#44d62c}`。
/// 悬停态同时用于 `.dots3` 的换图（`icon_more_default` → `icon_more`）。
#[derive(Default)]
pub(crate) struct HoverBorderState {
    pub(crate) hovered: bool,
    pub(crate) pressed: bool,
    /// `.show` / `.active`：弹层打开时边框转绿。
    pub(crate) open: bool,
}

/// `.nav-tabs .navs-wrapper .dots3` 的方框：`.hover-border{height:26px;width:26px;
/// border:1px solid #222;border-radius:13px;background-size:20px;margin-right:10px}`。
pub(crate) const NAV_MORE_WIDTH: f32 = 26.;
pub(crate) const NAV_MORE_MARGIN: f32 = 10.;

/// Mounted NavBarDropdown: .profile-act with .act.action.uppercase rows.
pub(crate) fn nav_overflow(
    id: &'static str,
    items: Vec<(String, bool)>,
    on_select: impl Fn(usize, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let active_hidden = items.iter().any(|(_, selected)| *selected);
    let more = window.use_keyed_state((ElementId::from(id), "more-state"), cx, |_, _| {
        HoverBorderState::default()
    });
    let border = hover_border_color(id, &more, window, cx);
    let trigger = more.clone();
    let on_select = std::rc::Rc::new(on_select);
    gpui_kit::base::Popover::new(id)
        .trigger_with(move |_, _, cx| {
            hover_border_button(
                id,
                if active_hidden {
                    "synapse/nav-more-active.svg"
                } else if trigger.read(cx).hovered {
                    "synapse/nav-more-hover.svg"
                } else {
                    "synapse/nav-more-default.svg"
                }
                .into(),
                "More",
                border,
                active_hidden,
                NAV_MORE_WIDTH,
                trigger.clone(),
            )
            // `.navs-wrapper .dots3` overrides the shared hover-border border.
            .border_0()
            .bg(if active_hidden {
                cx.theme().primary
            } else if trigger.read(cx).hovered {
                cx.theme().secondary_hover
            } else {
                cx.theme().transparent
            })
            .mr(css(NAV_MORE_MARGIN))
            .into_any_element()
        })
        .on_open_change(move |open, _, cx| {
            more.update(cx, |state, cx| {
                state.open = *open;
                cx.notify();
            })
        })
        .content(move |_, _, cx| {
            let popup = cx.entity().downgrade();
            gpui_kit::component::v_flex()
                .bg(rgb(0x000000))
                .border_1()
                .border_color(cx.theme().border)
                .min_w(css(155.))
                .max_w(css(280.))
                .children(items.iter().enumerate().map(|(index, (label, selected))| {
                    let foreground = if *selected {
                        cx.theme().primary
                    } else {
                        cx.theme().foreground
                    };
                    let on_select = on_select.clone();
                    let popup = popup.clone();
                    gpui_kit::base::Button::new((id, index))
                        .accessibility_label(label.clone())
                        .child(label.to_uppercase())
                        .flex()
                        .items_center()
                        .justify_start()
                        .h(css(27.))
                        .px(css(6.))
                        .py(css(5.))
                        .text_size(css(14.))
                        .line_height(css(17.))
                        .text_color(foreground)
                        .bg(rgb(0x000000))
                        .hover(move |s| s.bg(rgb(0x1a1a1a)).text_color(foreground))
                        .on_click(move |_, window, cx| {
                            let _ = popup.update(cx, |popup, cx| popup.dismiss(window, cx));
                            on_select(index, window, cx);
                        })
                }))
                .into_any_element()
        })
        .into_any_element()
}

/// 设备页标签溢出：源码 `renderNavs()` 用「窗口宽 − profile 栏宽 − 右侧区宽 −
/// `.dots3` 宽 − 10」得到可用宽度，再按每个标签「文字宽（Roboto 12px）+ 20 内边距
/// + 20 间距（最后一个不加）」贪心放进可见列表，放不下的进 `.dots3` 下拉。
/// 返回 `(可见标签, 溢出标签)`。
pub(crate) fn split_navs<T: Copy>(
    navs: &[T],
    label: impl Fn(T) -> String,
    available: f32,
    window: &Window,
) -> (Vec<T>, Vec<T>) {
    let mut visible = Vec::new();
    let mut hidden = Vec::new();
    let mut used = 0.;
    for (index, nav) in navs.iter().enumerate() {
        let text = label(*nav);
        let width =
            nav_label_width(&text, window) + 20. + if index + 1 != navs.len() { 20. } else { 0. };
        if hidden.is_empty() && used + width < available {
            used += width;
            visible.push(*nav);
        } else {
            hidden.push(*nav);
        }
    }
    (visible, hidden)
}

/// `getTextWidth(i18n(name), "normal 12px Roboto")`：12px = 0.75rem。
fn nav_label_width(label: &str, window: &Window) -> f32 {
    label_width(label, 12., window)
}

pub(crate) fn label_width(label: &str, font_size: f32, window: &Window) -> f32 {
    let mut font = window.text_style().font();
    font.family = "Roboto".into();
    font.weight = gpui_kit::gpui::FontWeight::NORMAL;
    let run = gpui_kit::gpui::TextRun {
        len: label.len(),
        font,
        color: gpui_kit::gpui::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    f32::from(
        window
            .text_system()
            .shape_line(
                label.to_owned().into(),
                window.rem_size() * (font_size / 16.),
                &[run],
                None,
            )
            .width,
    ) * 16.
        / f32::from(window.rem_size())
}

/// `.hover-border.dots3` 的边框色：常态 `#222`（`theme.background`）、悬停
/// `#5d5d5d`（`theme.border`）、`.show`/`.active`/按下 `#44d62c`（`theme.primary`），
/// 按源码 `transition:border-color .2s` 插值。要在 `Popover::trigger_with`
/// 之外采样（那里的 `window`/`cx` 是只读的），所以与按钮本身拆开。
pub(crate) fn hover_border_color(
    id: &'static str,
    state: &Entity<HoverBorderState>,
    window: &mut Window,
    cx: &mut App,
) -> Hsla {
    let current = state.read(cx);
    let target = if current.open || current.pressed {
        cx.theme().primary
    } else if current.hovered {
        cx.theme().border
    } else {
        cx.theme().background
    };
    // `.hover-border{transition:border-color .2s}`（CSS 默认 `ease`）。
    gpui_kit::base::motion::transition(
        (id, "hover-border"),
        target,
        gpui_kit::base::motion::Transition::new(std::time::Duration::from_millis(200))
            .easing(gpui_kit::base::motion::Easing::Ease),
        window,
        cx,
    )
}

/// `.hover-border.dots3`：profile 栏的「更多」与标签栏溢出菜单共用同一个方框
/// （26×26、圆角 13、20px 图标、`margin-right:10px`）。边框色由
/// [`hover_border_color`] 采样后传入；`filled` 对应 `.has-actived-option`
/// 的整块绿底。
pub(crate) fn hover_border_button(
    id: &'static str,
    asset: SharedString,
    label: &'static str,
    border: Hsla,
    filled: bool,
    size: f32,
    state: Entity<HoverBorderState>,
) -> gpui_kit::base::Button {
    gpui_kit::base::Button::new(id)
        .accessibility_label(label)
        .size(css(size))
        .p_0()
        .rounded(css(13.))
        .border_1()
        .border_color(border)
        // `.has-actived-option{background-color:#44d62c}`；其余态透明。
        .when(filled, |button| button.bg(Hsla::transparent_black()))
        .child(img(asset).size(css(20.)))
        .on_hover({
            let state = state.clone();
            move |hovered, _, cx| {
                state.update(cx, |state, cx| {
                    state.hovered = *hovered;
                    if !hovered {
                        state.pressed = false;
                    }
                    cx.notify();
                });
            }
        })
        .on_mouse_down(MouseButton::Left, {
            let state = state.clone();
            move |_, _, cx| {
                state.update(cx, |state, cx| {
                    state.pressed = true;
                    cx.notify();
                });
            }
        })
        .on_mouse_up(MouseButton::Left, {
            let state = state.clone();
            move |_, _, cx| {
                state.update(cx, |state, cx| {
                    state.pressed = false;
                    cx.notify();
                });
            }
        })
        .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
            state.update(cx, |state, cx| {
                state.pressed = false;
                cx.notify();
            });
        })
}

pub(crate) fn asset_button(
    id: &'static str,
    asset: &'static str,
    label: impl Into<SharedString>,
    cx: &App,
) -> Button {
    let label = label.into();
    let states = match asset {
        "synapse/eq-reset.svg" => {
            Some(("synapse/eq-reset-hover.svg", "synapse/eq-reset-active.svg"))
        }
        // Help's active SVG is selected-page state, not a pointer-down state.
        "synapse/help-default.svg" => Some(("synapse/help-hover.svg", "synapse/help-hover.svg")),
        _ => None,
    };
    let source_icon = states.is_some() || asset == "synapse/help-active.svg";
    let icon_size = if asset.starts_with("synapse/help-") {
        24.
    } else {
        20.
    };
    Button::new(id)
        .group(id)
        .ghost()
        .p_0()
        .border_0()
        .rounded(cx.theme().radius)
        .size(css(28.))
        .accessibility_label(label.clone())
        .tooltip(label)
        .custom(
            ButtonCustomVariant::new(cx)
                .hover(if source_icon {
                    cx.theme().transparent
                } else {
                    cx.theme().secondary_hover
                })
                .active(if source_icon {
                    cx.theme().transparent
                } else {
                    cx.theme().group_box
                }),
        )
        .child(
            div()
                .id("source-icon")
                .relative()
                .size(css(icon_size))
                .child(img(asset).size_full().object_fit(ObjectFit::Contain))
                .when_some(states, |this, (hover, active)| {
                    this.child(
                        img(hover)
                            .absolute()
                            .inset_0()
                            .size_full()
                            .opacity(0.)
                            .group_hover(id, |s| s.opacity(1.)),
                    )
                    .child(
                        img(active)
                            .id("icon-pressed")
                            .absolute()
                            .inset_0()
                            .size_full()
                            .opacity(0.)
                            .group_active(id, |s| s.opacity(1.)),
                    )
                }),
        )
}

/// The source toolbar arrows are 40x38, with a 20px image and opacity .3
/// when disabled. Use one Base activation handler for mouse and keyboard.
pub(crate) fn history_button(
    id: &'static str,
    forward: bool,
    enabled: bool,
    cx: &App,
) -> gpui_kit::base::Button {
    // Dashboard 96776:l -> 54693:IUp/OXp; Profiles 43:r -> 4693.
    let label = crate::i18n::t(if forward { "FORWARD" } else { "BACK" });
    gpui_kit::base::Button::new(id)
        .accessibility_label(label.clone())
        .disabled(!enabled)
        .occlude()
        .w(css(40.))
        .h(css(38.))
        .p_0()
        .flex_shrink_0()
        .bg(cx.theme().transparent)
        .when(enabled, |button| {
            button.hover(|style| style.bg(cx.theme().secondary_hover))
        })
        .styles(|styles| styles.disabled(|style| style.opacity(0.3)))
        .focus_visible(|style| style.border_1().border_color(cx.theme().primary))
        .tooltip(move |window, cx| tooltip::Tooltip::new(label.clone()).build(window, cx))
        .child(
            img(if forward {
                "synapse/history-forward.svg"
            } else {
                "synapse/history-back.svg"
            })
            .size(css(20.)),
        )
}

#[derive(Default)]
struct CloseButtonState {
    hovered: bool,
    pressed: bool,
}

/// `.keymap-head .close`: 36px target, 20px icon, background 200ms CSS ease.
/// Base Button still owns activation, keyboard focus and accessibility.
pub(crate) fn keymap_close_button(
    id: &'static str,
    label: &'static str,
    window: &mut Window,
    cx: &mut App,
) -> gpui_kit::base::Button {
    use super::theme::KeymapCloseColors;
    let transition = Transition::new(Duration::from_millis(200)).easing(Easing::Ease);
    let state = window.use_keyed_state((ElementId::from(id), "close-state"), cx, |_, _| {
        CloseButtonState::default()
    });
    let current = state.read(cx);
    let target = if current.pressed {
        KeymapCloseColors::pressed()
    } else if current.hovered {
        KeymapCloseColors::hover()
    } else {
        KeymapCloseColors::idle()
    };
    // CSS interpolates transparent colors in premultiplied space. Interpolating
    // HSL channels directly would introduce a dark flash on the way to white.
    let premultiplied_lightness = motion::transition(
        (id, "close-lightness"),
        target.l * target.a,
        transition.clone(),
        window,
        cx,
    );
    let alpha = motion::transition((id, "close-alpha"), target.a, transition, window, cx);
    let mut background = target;
    background.a = alpha;
    background.l = if alpha > 0. {
        premultiplied_lightness / alpha
    } else {
        0.
    };
    gpui_kit::base::Button::new(id)
        .accessibility_label(label)
        .size(css(36.))
        .p_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(background)
        .child(img("synapse/mapping-close.svg").size(css(20.)))
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
}

/// Source .switch: 32x18 track, 14px handle, round in both states.
/// Base owns controlled activation, keyboard focus and accessibility.
#[derive(IntoElement)]
pub(crate) struct SynapseSwitch {
    id: ElementId,
    base: gpui_kit::base::Switch,
    checked: bool,
    disabled: bool,
    label: Option<SharedString>,
}
impl SynapseSwitch {
    pub(crate) fn new(id: impl Into<ElementId>) -> Self {
        let id = id.into();
        Self {
            base: gpui_kit::base::Switch::new(id.clone()),
            id,
            checked: false,
            disabled: false,
            label: None,
        }
    }
    pub(crate) fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self.base = self.base.checked(checked);
        self
    }
    pub(crate) fn label(mut self, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        self.base = self.base.accessibility_label(label.clone());
        self.label = Some(label);
        self
    }
    pub(crate) fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.base = self.base.accessibility_label(label);
        self
    }
    pub(crate) fn on_change(
        mut self,
        handler: impl Fn(&bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.base = self
            .base
            .on_change(move |value, _, window, cx| handler(&value, window, cx));
        self
    }
}
impl Disableable for SynapseSwitch {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self.base = self.base.disabled(disabled);
        self
    }
}
impl RenderOnce for SynapseSwitch {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = window
            .use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        let focused = focus.is_focused(window);
        // Source .switch and .handle have separate CSS ease transitions. Keep
        // the controlled value immediate while only its presentation moves.
        let background_mix = motion::transition(
            (self.id.clone(), "switch-background"),
            if self.checked { 1_f32 } else { 0_f32 },
            Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
            window,
            cx,
        );
        let left = motion::transition(
            (self.id, "switch-handle-left"),
            if self.checked { 15_f32 } else { 1_f32 },
            Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
            window,
            cx,
        );
        self.base
            .track_focus(&focus)
            .flex()
            .items_center()
            .gap_2()
            .text_size(css(14.))
            .text_color(cx.theme().foreground)
            .when(self.disabled, |s| s.opacity(0.3))
            .when(!self.disabled, |s| s.hover(|s| s.opacity(0.7)))
            .child(
                div()
                    .id("switch-track")
                    .test_support()
                    .w(css(32.))
                    .h(css(18.))
                    .flex_shrink_0()
                    .relative()
                    .border_1()
                    .border_color(if focused {
                        cx.theme().ring
                    } else {
                        cx.theme().title_bar.opacity(0.3)
                    })
                    .rounded(css(16.))
                    // Blend opaque endpoints in sRGB, as this Chromium CSS does.
                    // Hsla's generic Lerp would travel through unrelated hues.
                    .bg(cx
                        .theme()
                        .switch
                        .blend(cx.theme().primary.opacity(background_mix)))
                    .child(
                        div()
                            .id("switch-handle")
                            .test_support()
                            .absolute()
                            .left(css(left))
                            .top(css(1.))
                            .size(css(14.))
                            .rounded_full()
                            .bg(cx.theme().switch_thumb),
                    ),
            )
            .when_some(self.label, |s, label| s.child(div().min_w_0().child(label)))
    }
}

/// .dot-bg's 22px grid and .dim-corner's radial fade, drawn behind content.
pub(crate) fn dot_background(cx: &App) -> AnyElement {
    let color = cx.theme().border;
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let scale = window.rem_size() / 16.;
            let step = scale * 22.;
            let width = f32::from(bounds.size.width);
            let height = f32::from(bounds.size.height);
            let step = f32::from(step);
            if width <= 0. || height <= 0. {
                return;
            }
            let dot_size = f32::from(scale * 2.);
            // The two CSS gradients expose the final 2px of a centered
            // 22px tile, rather than a dot at the tile's origin. Include
            // the preceding tile so partially visible edge dots survive.
            let start_x = ((width - step) / 2. + f32::from(scale * 20.)).rem_euclid(step) - step;
            let start_y = ((height - step) / 2. + f32::from(scale * 20.)).rem_euclid(step) - step;
            for row in 0..=(height / step) as usize + 1 {
                for col in 0..=(width / step) as usize + 1 {
                    let x = start_x + col as f32 * step;
                    let y = start_y + row as f32 * step;
                    let left = x.max(0.);
                    let top = y.max(0.);
                    let right = (x + dot_size).min(width);
                    let bottom = (y + dot_size).min(height);
                    if right <= left || bottom <= top {
                        continue;
                    }
                    // CSS radial-gradient defaults to an ellipse reaching the corners.
                    let radius = (((x - width / 2.) / (width / 2.)).powi(2)
                        + ((y - height / 2.) / (height / 2.)).powi(2))
                    .sqrt()
                        / 2_f32.sqrt();
                    window.paint_quad(fill(
                        Bounds::new(
                            bounds.origin + point(px(left), px(top)),
                            size(px(right - left), px(bottom - top)),
                        ),
                        color.opacity((1. - radius).clamp(0., 1.)),
                    ));
                }
            }
        },
    )
    .absolute()
    .size_full()
    .into_any_element()
}

pub(crate) fn page_columns() -> DeviceColumns {
    DeviceColumns {
        children: Vec::new(),
        style: StyleRefinement::default(),
    }
}

/// `.widget-col` gains 30px side margins below the source's 1280px boundary.
/// That rule forces the two fixed 600px cards onto separate rows even at 1279px,
/// where ordinary flex wrapping of 600 + 20 + 600 would still fit.
#[derive(IntoElement)]
pub(crate) struct DeviceColumns {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}
impl ParentElement for DeviceColumns {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}
impl Styled for DeviceColumns {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
impl RenderOnce for DeviceColumns {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        let stacked = stacked_device_columns(
            f32::from(window.viewport_size().width),
            f32::from(window.rem_size()),
        );
        h_flex()
            .id("page-columns")
            .test_support()
            .w_full()
            .max_w(css(BODY_MAX_WIDTH))
            .mx_auto()
            .gap(css(WIDGET_GAP))
            .flex_wrap()
            .items_start()
            .justify_center()
            .when(stacked, |this| {
                this.flex_col()
                    .items_center()
                    .px(css(COMPACT_COLUMN_MARGIN))
                    .min_w(css(WIDGET_WIDTH + COMPACT_COLUMN_MARGIN * 2.))
            })
            .refine_style(&self.style)
            .children(self.children)
    }
}

pub(crate) fn page_column(content: impl IntoElement) -> AnyElement {
    v_flex()
        .flex_grow(0.)
        .flex_shrink_0()
        .w(css(WIDGET_WIDTH))
        .min_w(css(WIDGET_WIDTH))
        .max_w(css(WIDGET_WIDTH))
        .child(content)
        .into_any_element()
}

pub(crate) fn panel(title: impl Into<SharedString>, cx: &App) -> Div {
    panel_with_control(title, div(), cx)
}
pub(crate) fn panel_with_control(
    title: impl Into<SharedString>,
    control: impl IntoElement,
    cx: &App,
) -> Div {
    // 依据 182 的 `.body-widgets .widget`：
    // `background-color:#111;border-radius:5px;flex:0 0 auto;font-size:14px;
    //  height:auto;margin:10px auto;max-width:600px;min-width:600px;padding:30px 40px`
    // 与 `.widget .titleRow{display:flex;justify-content:space-between}`、
    // `.widget .titleRow .title{color:#44d62c;display:flex;font-family:RazerF5,sans-serif;
    //  font-size:16px;margin-bottom:20px;text-transform:uppercase}`。
    let title = title.into().to_uppercase();
    v_flex()
        .w_full()
        .flex_shrink_0()
        .my(css(10.))
        .py(css(WIDGET_PADDING_Y))
        .px(css(WIDGET_PADDING_X))
        .bg(cx.theme().group_box)
        .rounded(css(WIDGET_RADIUS))
        .text_size(css(14.))
        .child(
            h_flex()
                .justify_between()
                .child(
                    div()
                        .font_family("RazerF5")
                        .text_size(css(16.))
                        .text_color(cx.theme().primary)
                        .mb(css(20.))
                        .child(title),
                )
                .child(control),
        )
}
/// `.widgetContent{display:flex;flex-direction:column;gap:20px}`: the widget
/// body's own column gap.
///
/// `panel`/`panel_with_control` append their children without a gap, because not
/// every widget body is a `.widgetContent` (some are a single `div`, some are a
/// plain block whose children carry only their own margins). A widget whose
/// current source body really is `.widgetContent` collects its children here so
/// the 20px gap appears exactly where the stylesheet declares it.
pub(crate) fn widget_content(children: impl IntoIterator<Item = AnyElement>) -> Div {
    v_flex().gap(css(20.)).children(children)
}
/// `_TA`/`wrA` 的 `hasSwitch:true`：原版把 `.widget-switch` 放进
/// `.titleRow > .title`（`display:flex`）里、紧跟标题文本，帮助按钮仍固定在
/// 组件右上角，所以标题行需要两个不同位置的控件。
pub(crate) fn panel_with_title_switch(
    title: impl Into<SharedString>,
    title_control: impl IntoElement,
    corner_control: impl IntoElement,
    cx: &App,
) -> Div {
    let title = title.into().to_uppercase();
    v_flex()
        .w_full()
        .flex_shrink_0()
        .my(css(10.))
        .py(css(WIDGET_PADDING_Y))
        .px(css(WIDGET_PADDING_X))
        .bg(cx.theme().group_box)
        .rounded(css(WIDGET_RADIUS))
        .text_size(css(14.))
        .child(
            h_flex()
                .justify_between()
                .child(
                    h_flex()
                        .items_center()
                        .gap(css(10.))
                        .mb(css(20.))
                        .child(
                            div()
                                .font_family("RazerF5")
                                .text_size(css(16.))
                                .text_color(cx.theme().primary)
                                .child(title),
                        )
                        .child(title_control),
                )
                .child(corner_control),
        )
}

/// `hasSwitch: !disabledReason`：有禁用原因时原版**根本不渲染** `.widget-switch`
/// （不是渲染成灰色开关），所以这种组件传入 `None` 时走不带开关的
/// [`panel_with_control`]，标题行结构其余部分完全一致。
pub(crate) fn panel_with_title_switch_opt(
    title: impl Into<SharedString>,
    title_control: Option<AnyElement>,
    corner_control: impl IntoElement,
    cx: &App,
) -> Div {
    match title_control {
        Some(title_control) => panel_with_title_switch(title, title_control, corner_control, cx),
        None => panel_with_control(title, corner_control, cx),
    }
}

/// `.widget .help`：14px 圆形帮助控件。依据当前源码的
/// `.widget .help{background-color:#4a4a4a;border-radius:50%;height:14px;
///  position:absolute;right:10px;top:10px;width:14px;transition:background-color .3s}`
/// 与 `:hover{background-color:#ffffff4d}`，悬停显示 `.widget .tip`
/// （`max-width:300px`、`font-size:14px;line-height:18px`、`padding:8px 10px`、
/// 黑底 `1px #5d5d5d`、`#ccc`）；图标是 `tooltip_questionmark.96138d2f.svg`。
pub(crate) fn help_control(id: impl Into<ElementId>, text: impl Into<SharedString>) -> AnyElement {
    help_control_kind(
        id,
        text,
        crate::ui::source_tooltip::SourceTooltipKind::WidgetTip,
    )
}

/// Current 179's module 7693 portals the help text immediately and applies its
/// own edge fallback. Other products retain their separately audited policy.
pub(crate) fn receiver_help_control(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
) -> AnyElement {
    help_control_kind(
        id,
        text,
        crate::ui::source_tooltip::SourceTooltipKind::ReceiverWidgetPortal,
    )
}

fn help_control_kind(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    kind: crate::ui::source_tooltip::SourceTooltipKind,
) -> AnyElement {
    let element_id = id.into();
    let text = text.into();
    SourceTooltip::new(element_id.clone(), text.to_string(), 300.)
        .kind(kind)
        .trigger(move |hovered, window, cx| {
            let background: Hsla = motion::transition(
                (element_id.clone(), "help-background"),
                if hovered {
                    rgba(0xffffff4d).into()
                } else {
                    rgb(0x4a4a4a).into()
                },
                Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
                window,
                cx,
            );
            BaseButton::new(element_id.clone())
                .w(css(14.))
                .h(css(14.))
                .flex_shrink_0()
                .p_0()
                .rounded_full()
                .bg(background)
                .accessibility_label(text.clone())
                .hover(|button| button.cursor_pointer())
                .child(img("synapse/automation-tooltip_questionmark.svg").size_full())
                .into_any_element()
        })
        .into_any_element()
}

/// 指针状态（悬停 / 按下）跟踪，供带 CSS `transition` 的控件做插值。
///
/// GPUI 的 `.hover(...)`/`.active(...)` 是**瞬时**样式，而源码里这类控件写的是
/// `transition:opacity .3s`、`transition:background-color .2s,border-color .2s,color .2s`
/// （`.thx-btn`、`.hover-btn`、`.hyper-wrapper`）。这里把状态放进 keyed state，
/// 再用 [`track_pointer`] 挂到任意可交互元素上，由调用方用 [`fade_opacity`] /
/// [`fade_color`] 取值。
#[derive(Default)]
pub(crate) struct PointerState {
    hovered: bool,
    pressed: bool,
}

impl PointerState {
    /// 当前 `(hovered, pressed)`。
    pub(crate) fn sample(&self) -> (bool, bool) {
        (self.hovered, self.pressed)
    }
}

/// 建立（或复用）某个元素 id 的指针状态。
pub(crate) fn pointer_state(
    id: impl Into<ElementId>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<PointerState> {
    window.use_keyed_state((id.into(), "pointer"), cx, |_, _| PointerState::default())
}

/// 把悬停与按下监听挂到元素上。
pub(crate) fn track_pointer<E: InteractiveElement + StatefulInteractiveElement>(
    element: E,
    state: &Entity<PointerState>,
    window: &mut Window,
) -> E {
    element
        .on_hover(window.listener_for(state, |pointer, hovered, _, cx| {
            pointer.hovered = *hovered;
            if !*hovered {
                pointer.pressed = false;
            }
            cx.notify();
        }))
        .on_mouse_down(
            MouseButton::Left,
            window.listener_for(state, |pointer, _, _, cx| {
                pointer.pressed = true;
                cx.notify();
            }),
        )
        .on_mouse_up(
            MouseButton::Left,
            window.listener_for(state, |pointer, _, _, cx| {
                pointer.pressed = false;
                cx.notify();
            }),
        )
}

/// 源码 `transition:opacity <ms>` 的不透明度插值。
pub(crate) fn fade_opacity(
    id: impl Into<ElementId>,
    target: f32,
    millis: u64,
    window: &mut Window,
    cx: &mut App,
) -> f32 {
    motion::transition(
        (id.into(), "fade-opacity"),
        target,
        Transition::new(Duration::from_millis(millis)).easing(Easing::Ease),
        window,
        cx,
    )
}

/// 源码 `transition:background-color|border-color|color <ms>` 的颜色插值。
pub(crate) fn fade_color(
    id: impl Into<ElementId>,
    target: Hsla,
    millis: u64,
    window: &mut Window,
    cx: &mut App,
) -> Hsla {
    motion::transition(
        (id.into(), "fade-color"),
        target,
        Transition::new(Duration::from_millis(millis)).easing(Easing::Ease),
        window,
        cx,
    )
}

/// `.check-item` / `.check-box`：20×20 圆角 2.4px 方框（`1px solid #737373`，悬停
/// `#44d62c`，选中底 `#44d62c`），勾由 `:before`/`:after` 两条 `#111` 线组成
/// （`ticktop .2s ease`、`tickbottom .1s ease`）；`.check-text` 是 `#ccc`、
/// 14px/17px、相对方框左移 30px、上移 2px，首字母大写；`.check-item{margin-bottom:9px}`，
/// 禁用时整行 `opacity:.3;pointer-events:none`。
pub(crate) fn check_item(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    checked: bool,
    disabled: bool,
    window: &mut Window,
    cx: &mut App,
) -> BaseButton {
    check_item_with_style(
        id,
        label,
        checked,
        disabled,
        CheckItemStyle {
            unchecked_background: rgb(0x111111).into(),
            tick_bottom_origin: (0.6, 10.),
        },
        window,
        cx,
    )
}

/// Source CSS variants, without changing the default used by other pages.
pub(crate) struct CheckItemStyle {
    pub(crate) unchecked_background: Hsla,
    pub(crate) tick_bottom_origin: (f32, f32),
}

pub(crate) fn check_item_with_style(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    checked: bool,
    disabled: bool,
    style: CheckItemStyle,
    window: &mut Window,
    cx: &mut App,
) -> BaseButton {
    let element_id = id.into();
    let label = label.into();
    let tick_top = Presence::new((element_id.clone(), "tick-top"), checked)
        .transition(Transition::new(Duration::from_millis(200)).easing(Easing::Ease))
        .sample(window, cx)
        .progress;
    let tick_bottom = Presence::new((element_id.clone(), "tick-bottom"), checked)
        .transition(Transition::new(Duration::from_millis(100)).easing(Easing::Ease))
        .sample(window, cx)
        .progress;
    BaseButton::new(element_id.clone())
        .accessibility_label(label.clone())
        .flex()
        .items_center()
        .justify_start()
        .w_full()
        .m_0()
        .p_0()
        .bg(rgba(0x00000000))
        .mb(css(9.))
        .when(disabled, |button| button.opacity(0.3).disabled(true))
        .child(
            div()
                .relative()
                .size(css(20.))
                .flex_shrink_0()
                .rounded(css(2.4))
                .border_1()
                .border_color(if checked {
                    rgb(0x44d62c)
                } else {
                    rgb(0x737373)
                })
                .bg(if checked {
                    Hsla::from(rgb(0x44d62c))
                } else {
                    style.unchecked_background
                })
                .hover(|style| style.border_color(rgb(0x44d62c)))
                // `.check-text`：方框内相对定位、左移 30px、上移 2px。
                .child(
                    div()
                        .absolute()
                        .left(css(30.))
                        .top(css(2.))
                        .text_size(css(14.))
                        .line_height(css(17.))
                        .text_color(rgb(0xcccccc))
                        .child(first_letter_uppercase(label.clone())),
                )
                .when(checked, |view| {
                    view.child(check_tick(tick_top, tick_bottom, style.tick_bottom_origin))
                }),
        )
}

/// `.check-box:before/:after` 的两段勾线（`rotate(-145deg)` 长 15.4px、
/// `rotate(-50deg)` 长 9.6px，宽 3px、`#111`），长度按 `ticktop`/`tickbottom` 动画插值。
fn check_tick(tick_top: f32, tick_bottom: f32, bottom_origin: (f32, f32)) -> AnyElement {
    canvas(
        move |_, _, _| (),
        move |bounds, _, window, _| {
            let scale = f32::from(bounds.size.width) / 20.;
            for (x, y, angle, length) in [
                (8.6_f32, 16.4_f32, -145_f32, 15.4 * tick_top),
                (bottom_origin.0, bottom_origin.1, -50., 9.6 * tick_bottom),
            ] {
                let angle = angle.to_radians();
                let start = bounds.origin + point(px(x * scale), px(y * scale));
                let mut path = PathBuilder::stroke(px(3. * scale));
                path.move_to(start);
                path.line_to(
                    start
                        + point(
                            px(-angle.sin() * length * scale),
                            px(angle.cos() * length * scale),
                        ),
                );
                if let Ok(path) = path.build() {
                    window.paint_path(path, rgb(0x111111));
                }
            }
        },
    )
    .absolute()
    .inset_0()
    .size_full()
    .into_any_element()
}

/// `.check-text:first-letter{text-transform:uppercase}`。
fn first_letter_uppercase(text: SharedString) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// `OTA` 的 `.foot` 灰标：`.foot{position:absolute;text-transform:uppercase}`、
/// `.foot.min{left:0}`、`.foot.mid{left:0;width:100%;text-align:center}`、
/// `.foot.mid1{left:0;width:66%}`、`.foot.mid2{left:0;width:133%}`、`.foot.max{right:0}`。
pub(crate) fn slider_tags(
    min: &str,
    mid: Option<&str>,
    max: &str,
    boost: Option<&str>,
) -> AnyElement {
    let tag = |label: &str| {
        div()
            .font_family("Roboto")
            .text_size(css(14.))
            .line_height(css(17.))
            .child(label.to_uppercase())
    };
    let mut marks = div()
        .relative()
        .w_full()
        .h(css(17.))
        .child(div().absolute().left_0().child(tag(min)));
    marks = match boost {
        Some(boost) => {
            let mid = mid.unwrap_or_default();
            marks
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .w(relative(0.66))
                        .text_center()
                        .child(tag(mid)),
                )
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .w(relative(1.33))
                        .text_center()
                        .child(tag(max)),
                )
                .child(div().absolute().right_0().child(tag(boost)))
        }
        None => {
            let mut marks = marks;
            if let Some(mid) = mid {
                marks = marks.child(
                    div()
                        .absolute()
                        .left_0()
                        .w_full()
                        .text_center()
                        .child(tag(mid)),
                );
            }
            marks.child(div().absolute().right_0().child(tag(max)))
        }
    };
    marks.into_any_element()
}

/// `.h1-body{color:#ccc;margin-bottom:10px}`：控件标题下方那段说明文字。
pub(crate) fn h1_body(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_color(cx.theme().group_box_foreground)
        .mb(css(10.))
        .child(text.into())
}
pub(crate) fn note(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
}
pub(crate) fn external(
    id: &'static str,
    label: impl Into<SharedString>,
    url: &'static str,
) -> Button {
    Button::new(id)
        .label(label)
        .outline()
        .icon(gpui_kit::assets::IconName::ExternalLink)
        .on_click(move |_, _, cx| cx.open_url(url))
}
pub(crate) fn product_image(pid: u32, edition_id: u32, layout_id: u32) -> AnyElement {
    div()
        .size_full()
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .when_some(
            crate::resources::device_image(
                pid,
                edition_id,
                layout_id,
                crate::resources::DeviceImage::Product,
            ),
            |this, path| this.child(img(path).object_fit(ObjectFit::Contain).size_full()),
        )
        .into_any_element()
}

pub(crate) fn product_banner(pid: u32, edition_id: u32, layout_id: u32, cx: &App) -> AnyElement {
    div()
        .relative()
        .flex()
        .justify_center()
        .items_center()
        .w_full()
        .h(css(250.))
        .max_w(css(1220.))
        .min_w(css(1024.))
        .mx_auto()
        .my(css(10.))
        .child(dot_background(cx))
        .child(
            div()
                .relative()
                .w(css(325.))
                .h_full()
                .child(product_image(pid, edition_id, layout_id)),
        )
        .into_any_element()
}

pub(crate) fn config_wrapper() -> Div {
    div()
        .relative()
        .w_full()
        .min_w(css(CONFIG_WRAPPER_MIN_WIDTH))
        .max_w(css(CONFIG_WRAPPER_MAX_WIDTH))
        .mx_auto()
}
