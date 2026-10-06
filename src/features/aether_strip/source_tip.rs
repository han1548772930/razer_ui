//! Current 784 `.tip` and Fd help portal. Base owns placement and tooltip semantics.
use super::*;
use gpui_kit::base::{
    Positioner, Tooltip,
    motion::{self, Easing, Transition},
};
use std::{cell::Cell, rc::Rc, time::Duration};

type Click = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
#[derive(Clone, Copy)]
enum Kind {
    Help,
    /// `[tooltip]` 伪元素：全局 `top:calc(100% + 5px)` 加三种锚点
    /// （`.device--badge.anchor--left` 的 `left:50%`、`.anchor--right` 的 `right:0`、
    /// `.anchor--middle` 的 `left:50% + translateX(-50%) + width:200px`）。
    Badge {
        anchor: TipAnchor,
        gap: f32,
    },
}
/// `[tooltip]:before` 的水平锚点。
#[derive(Clone, Copy, PartialEq)]
pub(super) enum TipAnchor {
    /// `right:auto`（没有 `left`）：提示框左边缘落在触发元素左边缘，源码里编号项
    /// `.indicator--item[tooltip]:before` 用的就是这一条。
    Start,
    /// `left:50%`：提示框左边缘落在触发元素中线上（`.device--badge.anchor--left`）。
    Half,
    /// `right:0`：提示框右边缘贴住触发元素右边缘（`.anchor--right`）。
    Right,
    /// `left:50%;transform:translateX(-50%)`：提示框水平居中（`.anchor--middle`）。
    Center,
}
impl TipAnchor {
    fn x(self, trigger: Bounds<Pixels>, width: Pixels) -> Pixels {
        match self {
            Self::Start => trigger.left(),
            Self::Half => trigger.left() + trigger.size.width / 2.,
            Self::Right => trigger.right() - width,
            Self::Center => trigger.left() + (trigger.size.width - width) / 2.,
        }
    }
}
#[derive(IntoElement)]
pub(super) struct TipCommand {
    id: ElementId,
    name: &'static str,
    label: String,
    disabled: bool,
    kind: Kind,
    click: Option<Click>,
    style: StyleRefinement,
}
impl TipCommand {
    pub(super) fn icon(
        id: impl Into<ElementId>,
        name: &'static str,
        label: String,
        disabled: bool,
    ) -> Self {
        Self {
            id: id.into(),
            name,
            label,
            disabled,
            // 源码里这类图标按钮与徽标同属 `[tooltip]` 伪元素：没有更具体的覆盖规则，
            // 走全局 `[tooltip]:before{right:0;top:calc(100% + 5px);width:auto;
            //  white-space:nowrap}`。
            kind: Kind::Badge {
                anchor: TipAnchor::Right,
                gap: 5.,
            },
            click: None,
            style: StyleRefinement::default(),
        }
    }
    pub(super) fn help(label: String) -> Self {
        Self {
            id: "aether-layout-help".into(),
            name: "tooltip_questionmark",
            label,
            disabled: false,
            kind: Kind::Help,
            click: None,
            style: StyleRefinement::default(),
        }
    }
    pub(super) fn on_click(
        mut self,
        click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.click = Some(Box::new(click));
        self
    }
}
impl Styled for TipCommand {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
#[derive(Default)]
struct TipState {
    hover: bool,
    bounds: Rc<Cell<Bounds<Pixels>>>,
}
impl RenderOnce for TipCommand {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state((self.id.clone(), "tooltip-state"), cx, |_, _| {
            TipState::default()
        });
        let shown = state.read(cx).hover && !self.disabled;
        let bounds = state.read(cx).bounds.clone();
        let opacity = match self.kind {
            Kind::Help => {
                if shown {
                    1.
                } else {
                    0.
                }
            }
            Kind::Badge { .. } => motion::transition(
                (self.id.clone(), "tooltip-opacity"),
                if shown { 1. } else { 0. },
                Transition::new(Duration::from_millis(300)).easing(Easing::Linear),
                window,
                cx,
            ),
        };
        let help = matches!(self.kind, Kind::Help);
        let mut button = gpui_kit::base::Button::new(self.id.clone())
            .disabled(self.disabled)
            .accessibility_label(self.label.clone())
            .size_full()
            .p_0()
            .focus_visible(|v| v.border_1().border_color(cx.theme().primary));
        if help {
            let color = motion::transition(
                (self.id.clone(), "help-background"),
                if shown {
                    cx.theme().button_foreground.opacity(0.3)
                } else {
                    crate::ui::theme::TooltipColors::help_background()
                },
                Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
                window,
                cx,
            );
            button = button
                .rounded(surface::css(7.))
                .bg(color)
                .child(img(asset(self.name)).size_full());
        } else {
            button = button.child(presentation::ActionIcon {
                id: self.id.clone(),
                name: self.name,
                disabled: self.disabled,
            });
        }
        if let Some(click) = self.click {
            button = button.on_click(move |event, window, cx| click(event, window, cx));
        }
        div()
            .id((self.id.clone(), "tip-trigger"))
            .relative()
            .size(surface::css(if help { 14. } else { 27. }))
            .refine_style(&self.style)
            .on_hover(window.listener_for(&state, |state, hover, _, cx| {
                state.hover = *hover;
                cx.notify();
            }))
            .on_prepaint({
                let bounds = bounds.clone();
                move |value, _, _| bounds.set(value)
            })
            .child(button)
            .child(
                deferred(TipLayer {
                    id: self.id,
                    label: self.label,
                    bounds,
                    kind: self.kind,
                    shown,
                    opacity,
                })
                .with_priority(200),
            )
    }
}
/// 卡片徽标 `.device--badge`：源里是带 `tooltip` 属性的元素，提示框走
/// `[tooltip]:before`（`top:calc(100% + 5px)` + 三种锚点）。轮播里
/// `.device--badge[tooltip]:before{display:none}`，只有选中卡片上的徽标在 `:hover` 时
/// 才 `display:block`，所以这里用 `enabled` 表达同一条件。
#[derive(IntoElement)]
pub(super) struct SourceTipItem {
    id: ElementId,
    name: &'static str,
    label: String,
    disabled: bool,
    enabled: bool,
    anchor: TipAnchor,
    /// `.device.active.busy .device--cta-enable:hover{background-color:#707070}`。
    hover_bg: Option<Hsla>,
    click: Option<Click>,
    style: StyleRefinement,
}
impl SourceTipItem {
    pub(super) fn new(
        id: impl Into<ElementId>,
        name: &'static str,
        label: String,
        disabled: bool,
        enabled: bool,
        anchor: TipAnchor,
    ) -> Self {
        Self {
            id: id.into(),
            name,
            label,
            disabled,
            enabled,
            anchor,
            hover_bg: None,
            click: None,
            style: StyleRefinement::default(),
        }
    }
    pub(super) fn with_hover_bg(mut self, color: Hsla) -> Self {
        self.hover_bg = Some(color);
        self
    }
    pub(super) fn on_click(
        mut self,
        click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.click = Some(Box::new(click));
        self
    }
}
impl Styled for SourceTipItem {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
impl RenderOnce for SourceTipItem {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state((self.id.clone(), "tooltip-state"), cx, |_, _| {
            TipState::default()
        });
        let shown = state.read(cx).hover && self.enabled;
        let bounds = state.read(cx).bounds.clone();
        let opacity = motion::transition(
            (self.id.clone(), "tooltip-opacity"),
            if shown { 1. } else { 0. },
            Transition::new(Duration::from_millis(300)).easing(Easing::Linear),
            window,
            cx,
        );
        // `.carousel--item .device--cta-del{background:url(close-btn);background-position:50%;
        //  background-repeat:no-repeat;border-radius:50%;height:24px;width:24px}` +
        //  `:hover{background-image:url(close-hovered-btn)}`；
        //  `.device--cta-find:hover,.device--cta-power:hover{border-color:#44d62c}`。
        let mut button = gpui_kit::base::Button::new(self.id.clone())
            .group(self.name)
            .accessibility_label(self.label.clone())
            .disabled(self.disabled)
            .size(surface::css(24.))
            .p_0()
            .rounded_full()
            .when(self.disabled, |v| v.opacity(0.3))
            .relative()
            .refine_style(&self.style)
            .child(img(asset(self.name)).size_full())
            .when(self.name == "close-btn", |v| {
                v.child(
                    img(asset("close-hovered-btn"))
                        .absolute()
                        .inset_0()
                        .size_full()
                        .opacity(0.)
                        .group_hover(self.name, |v| v.opacity(1.)),
                )
            })
            .focus_visible(|v| v.border_1().border_color(cx.theme().primary))
            .when(
                !self.disabled
                    && ["power-on-btn", "power-off-btn", "indentify-btn"].contains(&self.name),
                |v| v.hover(|s| s.border_1().border_color(cx.theme().primary)),
            )
            .when(self.hover_bg.is_some(), |v| {
                v.hover(|s| s.bg(self.hover_bg.unwrap_or(Colors::control_border())))
            });
        if let Some(click) = self.click {
            button = button.on_click(move |event, window, cx| click(event, window, cx));
        }
        div()
            .id((self.id.clone(), "tip-trigger"))
            .relative()
            .on_hover(window.listener_for(&state, |state, hover, _, cx| {
                state.hover = *hover;
                cx.notify();
            }))
            .on_prepaint({
                let bounds = bounds.clone();
                move |value, _, _| bounds.set(value)
            })
            .child(button)
            .child(
                deferred(TipLayer {
                    id: self.id,
                    label: self.label,
                    bounds,
                    kind: Kind::Badge {
                        anchor: self.anchor,
                        gap: 5.,
                    },
                    shown,
                    opacity,
                })
                .with_priority(200),
            )
    }
}
/// `[tooltip]` 伪元素的通用挂载：包住任意触发元素，负责 hover 跟踪、位置与提示层
/// （编号项 `.indicator--item[tooltip]:before{right:auto;top:calc(100% + 10px);
/// width:fit-content}` 走这一条）。
#[derive(IntoElement)]
pub(super) struct SourceTipWrap {
    id: ElementId,
    label: String,
    anchor: TipAnchor,
    gap: f32,
    style: StyleRefinement,
    child: AnyElement,
}
impl SourceTipWrap {
    pub(super) fn new(
        id: impl Into<ElementId>,
        label: String,
        anchor: TipAnchor,
        gap: f32,
        child: AnyElement,
    ) -> Self {
        Self {
            id: id.into(),
            label,
            anchor,
            gap,
            style: StyleRefinement::default(),
            child,
        }
    }
}
impl Styled for SourceTipWrap {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
impl RenderOnce for SourceTipWrap {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state((self.id.clone(), "tooltip-state"), cx, |_, _| {
            TipState::default()
        });
        let shown = state.read(cx).hover;
        let bounds = state.read(cx).bounds.clone();
        let opacity = motion::transition(
            (self.id.clone(), "tooltip-opacity"),
            if shown { 1. } else { 0. },
            Transition::new(Duration::from_millis(300)).easing(Easing::Linear),
            window,
            cx,
        );
        div()
            .id((self.id.clone(), "tip-trigger"))
            .relative()
            .refine_style(&self.style)
            .on_hover(window.listener_for(&state, |state, hover, _, cx| {
                state.hover = *hover;
                cx.notify();
            }))
            .on_prepaint({
                let bounds = bounds.clone();
                move |value, _, _| bounds.set(value)
            })
            .child(self.child)
            .child(
                deferred(TipLayer {
                    id: self.id,
                    label: self.label,
                    bounds,
                    kind: Kind::Badge {
                        anchor: self.anchor,
                        gap: self.gap,
                    },
                    shown,
                    opacity,
                })
                .with_priority(200),
            )
    }
}
struct TipLayer {
    id: ElementId,
    label: String,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    kind: Kind,
    shown: bool,
    opacity: f32,
}
struct TipLayout {
    positioner: Option<Positioner>,
    layout: <Positioner as Element>::RequestLayoutState,
    size: gpui_kit::Size<Pixels>,
}
impl IntoElement for TipLayer {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}
impl Element for TipLayer {
    type RequestLayoutState = TipLayout;
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, TipLayout) {
        let width = window.rem_size() * (300. / 16.);
        let foreground = cx.theme().foreground;
        // 全局 `[tooltip]:before{…;font-size:14px;line-height:16px;padding:8px 10px;
        //  white-space:nowrap;width:auto;z-index:100}`；`.anchor--middle` 覆盖成
        // `width:200px;white-space:break-spaces`。`.tip`（Help）仍是 300px 宽、行高 18。
        let badge = matches!(self.kind, Kind::Badge { .. });
        let centered = matches!(
            self.kind,
            Kind::Badge {
                anchor: TipAnchor::Center,
                ..
            }
        );
        let surf = |id: ElementId| {
            Tooltip::new(id)
                .when(centered, |v| v.w(width * (200. / 300.)))
                .when(badge && !centered, |v| {
                    v.px(surface::css(10.))
                        .py(surface::css(8.))
                        .line_height(surface::css(16.))
                        .whitespace_nowrap()
                })
                .when(!badge, |v| {
                    v.w(width)
                        .line_height(surface::css(18.))
                        .whitespace_normal()
                })
                .px(surface::css(10.))
                .py(surface::css(8.))
                .border_1()
                .border_color(Colors::border())
                .bg(Colors::dialog())
                .text_color(foreground)
                .font_family("Roboto")
                .text_size(surface::css(14.))
                .child(self.label.clone())
        };
        let mut measure = surf((self.id.clone(), "tip-measure").into()).into_any_element();
        // `.device--badge` 的 `width:auto` 与 `.indicator--item` 的 `width:fit-content`
        // 都是按内容取宽；Help 是固定 300px。
        let available = if badge && !centered {
            AvailableSpace::MaxContent
        } else {
            AvailableSpace::Definite(width * if centered { 200. / 300. } else { 1. })
        };
        let size = measure.layout_as_root(size(available, AvailableSpace::MinContent), window, cx);
        let content = div()
            .opacity(self.opacity)
            .when(!self.shown, |v| v.invisible())
            .child(surf((self.id.clone(), "tip-surface").into()));
        let mut positioner = Positioner::corner(Anchor::TopLeft, Point::default())
            .margin(px(0.))
            .child(content);
        let (id, layout) = positioner.request_layout(None, None, window, cx);
        (
            id,
            TipLayout {
                positioner: Some(positioner),
                layout,
                size,
            },
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut TipLayout,
        window: &mut Window,
        cx: &mut App,
    ) {
        let trigger = self.bounds.get();
        let scale = window.rem_size() / 16.;
        let mut point = match self.kind {
            // `[tooltip]:before{top:calc(100% + gap)}` + 各锚点；`display:none` 由调用方
            // 用 `enabled` 表达（源码里非选中卡片的徽标提示整体不显示）。
            Kind::Badge { anchor, gap } => point(
                anchor.x(trigger, layout.size.width),
                trigger.bottom() + scale * gap,
            ),
            // Fd: widget.right - 14 - tip.width, widget.top + 34.
            Kind::Help => point(
                trigger.right() - scale * 4. - layout.size.width,
                trigger.top() + scale * 24.,
            ),
        };
        if matches!(self.kind, Kind::Help) {
            let viewport = window.viewport_size();
            point.x = point
                .x
                .max(scale * 14.)
                .min((viewport.width - scale * 14. - layout.size.width).max(scale * 14.));
            if point.y + layout.size.height > viewport.height {
                point.x = trigger.right() + scale * 8.;
                point.y = trigger.top();
                if point.x + layout.size.width > viewport.width {
                    point.x = trigger.left() - scale * 8. - layout.size.width;
                }
                if point.y + layout.size.height > viewport.height {
                    point.y = (viewport.height - layout.size.height - scale * 10.).max(px(0.));
                }
            }
        }
        let mut positioner = layout.positioner.take().unwrap().position(point);
        positioner.prepaint(None, None, bounds, &mut layout.layout, window, cx);
        layout.positioner = Some(positioner);
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut TipLayout,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        layout.positioner.as_mut().unwrap().paint(
            None,
            None,
            bounds,
            &mut layout.layout,
            &mut (),
            window,
            cx,
        );
    }
}
