use gpui_kit::base::motion::{self, Easing, Transition};
use gpui_kit::component::{
    button::{Button, ButtonCustomVariant, ButtonVariants},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::time::Duration;

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

pub(crate) fn navigation_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    cx: &App,
) -> Button {
    // 依据 182 `.nav-tabs .nav`：
    // `border-radius:14px;color:#999;line-height:14px;margin-right:20px;padding:7px 10px;
    //  text-align:center;text-transform:uppercase`；
    // `:hover{background-color:#2d2d2d;color:#ccc}`；
    // `:active{background-color:#3cbf27;color:#111}`；
    // `.nav.active{background-color:#44d62c;color:#111}`。
    // 选中态背景 `#44d62c` = `primary`，文字 `#111` = `primary_foreground`；
    // 悬停底色 `#2d2d2d` = `secondary_hover`，悬停文字 `#ccc` = `foreground`；
    // 按下底色是比选中态更深的 `#3cbf27`。
    let label = label.into().to_uppercase();
    Button::new(id)
        .xsmall()
        .label(label)
        .selected(selected)
        .custom(
            ButtonCustomVariant::new(cx)
                .color(if selected {
                    cx.theme().primary
                } else {
                    cx.theme().transparent
                })
                .foreground(if selected {
                    cx.theme().primary_foreground
                } else {
                    cx.theme().muted_foreground
                })
                .hover(if selected {
                    cx.theme().primary
                } else {
                    cx.theme().secondary_hover
                })
                .active(gpui_kit::rgb(0x3cbf27).into()),
        )
        .hover(|style| {
            style
                .bg(if selected {
                    cx.theme().primary
                } else {
                    cx.theme().secondary_hover
                })
                .text_color(cx.theme().foreground)
        })
        // 按下时的文字色源码是 `#111`；`Button` 只暴露 `.hover`，按下态文字色要像
        // `keymap_close_button` 那样自持状态才能插值，这里先用变体的按下底色。
        .h(css(28.))
        .px(css(10.))
        .py_0()
        .text_size(css(12.))
        .rounded(cx.theme().font_size * (14. / 16.))
        .border_0()
}

/// `.profile-bar` 的宽度：源码 `.nav-tabs .profile-bar{width:auto}`，实测布局里
/// profile 入口 26 + 下拉 230 + 两侧 10 边距 + 更多按钮 26 + 右边距 10 + OBM 26。
/// （`has_obm` 时才多出 OBM 的 32px：26 + 10 外边距 − 4px 重叠。）
pub(crate) const PROFILE_BAR_WIDTH: f32 = 322.;
pub(crate) const PROFILE_BAR_WIDTH_OBM: f32 = 354.;
/// 顶栏右侧区：电量 `icon box 26 + 左右各 10` + 帮助按钮 24 + 右边距 10。
pub(crate) const DEVICE_RIGHT_WIDTH: f32 = 46. + 24. + 10.;

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
    let mut font = window.text_style().font();
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
                window.rem_size() * 0.75,
                &[run],
                None,
            )
            .width,
    )
}

/// 设备页标签栏最左边的返回/前进按钮：源码里的 `.nav.back` / `.nav.forward`
/// （`.nav-tabs .nav` 的 28px 高、14px 圆角、`padding:7px 10px`，
/// `background-image` 分别是 `nav_back_arrow` / `nav_fwd_arrow`，没有文字标签），
/// 不可用时套 `.nav.disabled{opacity:.3;pointer-events:none}`。
pub(crate) fn nav_arrow_button(id: &'static str, forward: bool, enabled: bool, cx: &App) -> Button {
    let (asset, label) = if forward {
        ("synapse/nav-fwd-arrow.svg", "前进")
    } else {
        ("synapse/nav-back-arrow.svg", "后退")
    };
    Button::new(id)
        .accessibility_label(label)
        .disabled(!enabled)
        .flex()
        .items_center()
        .justify_center()
        .h(css(28.))
        .px(css(10.))
        .py_0()
        .rounded(cx.theme().font_size * (14. / 16.))
        .border_0()
        .bg(cx.theme().transparent)
        .when(enabled, |button| {
            button.hover(|style| style.bg(cx.theme().secondary_hover))
        })
        .when(!enabled, |button| button.opacity(0.3))
        .child(img(asset).size(css(9.)))
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
