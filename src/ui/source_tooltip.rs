//! OBM portal `.drop-tips` and persistent CSS `.tip` presentation.
//! Native hover owns the trigger; Base owns motion, tooltip semantics and final
//! viewport placement. The source client-size and anchor policy lives here.
use crate::ui::{surface, theme::TooltipColors};
use gpui_kit::base::{
    ElementExt as _, Positioner, Tooltip,
    motion::{self, Easing, Presence, Transition},
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::{cell::Cell, rc::Rc, time::Duration};

#[cfg(test)]
#[path = "source_tooltip_tests.rs"]
mod tests;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum SourceTooltipKind {
    /// 653 module 61050 / mR: bottom-right, with independent edge fallback.
    DropTips,
    /// cR portals a `.tooltip_parent` directly over the profile icon.
    ProfileWarning,
    /// pR overrides `.lock .tip` with fixed icon-left/top + 20px on mouseover.
    LockedProfile,
    /// `.tooltip-razer.bottom-left`: 300px main, intrinsic wrapper, +5px below.
    Battery,
    /// Alexa sE -> oE: bottom-left portal, 100ms mount delay and 100ms fade.
    Alexa,
    /// Profiles `.main-nav li:hover .tooltip`: immediate, +15/+30, max-content.
    ProfilesNav,
    /// `.widget .help + .tip` / `.body-widget-tip-portal`: 14px/18px,
    /// `max-width:300px`, `right:14px;top:34px` against the widget box, and the
    /// trigger is the 14px `.widget .help` control at `right:10px;top:10px`.
    WidgetTip,
    /// Product 179 module 7693: conditionally mounted `.body-widget-tip-portal`,
    /// immediate visibility, 10001 stacking and source edge fallback.
    ReceiverWidgetPortal,
    /// Product 691 OLED BLE-disabled cards: `[turn-off-ble-tooltip]` pseudo
    /// element, 20px/185px card-relative anchor and 300ms linear opacity.
    OledBleDisabled,
}

type Trigger = Box<dyn FnOnce(bool, &mut Window, &mut App) -> AnyElement>;

#[derive(IntoElement)]
pub(crate) struct SourceTooltip {
    id: ElementId,
    text: SharedString,
    width: f32,
    kind: SourceTooltipKind,
    trigger: Trigger,
}

#[derive(Default)]
struct HoverState {
    trigger: bool,
    content: bool,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    _activation: Option<Subscription>,
}

impl SourceTooltip {
    pub(crate) fn new(id: impl Into<ElementId>, text: impl Into<SharedString>, width: f32) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            width,
            kind: SourceTooltipKind::DropTips,
            trigger: Box::new(|_, _, _| div().into_any_element()),
        }
    }

    pub(crate) fn kind(mut self, kind: SourceTooltipKind) -> Self {
        self.kind = kind;
        self
    }

    pub(crate) fn trigger(
        mut self,
        trigger: impl FnOnce(bool, &mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        self.trigger = Box::new(trigger);
        self
    }
}

impl RenderOnce for SourceTooltip {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let kind = self.kind;
        let state =
            window.use_keyed_state((self.id.clone(), "tip-hover"), cx, move |window, cx| {
                let activation = (kind == SourceTooltipKind::Alexa).then(|| {
                    cx.observe_window_activation(window, |state: &mut HoverState, window, cx| {
                        if !window.is_window_active() {
                            state.trigger = false;
                            cx.notify();
                        }
                    })
                });
                HoverState {
                    _activation: activation,
                    ..Default::default()
                }
            });
        let hovered = state.read(cx).trigger
            || (self.kind == SourceTooltipKind::ProfileWarning && state.read(cx).content);
        let anchor = state.read(cx).bounds.clone();
        // Both CSS paths retain opacity while hidden, including on reversal.
        let presence = Presence::new((self.id.clone(), "tip-opacity"), hovered)
            .transition(
                Transition::new(Duration::from_millis(
                    if matches!(
                        self.kind,
                        SourceTooltipKind::ProfilesNav
                            | SourceTooltipKind::ReceiverWidgetPortal
                            | SourceTooltipKind::WidgetTip
                    ) {
                        // `.body-widget-tip-portal{opacity:1;visibility:visible;
                        //  z-index:10001}` 覆盖 `.tip` 的 `opacity:0` 与 300ms
                        // 过渡，React 直接挂载/卸载这个 portal。
                        0
                    } else if matches!(
                        self.kind,
                        SourceTooltipKind::Battery | SourceTooltipKind::Alexa
                    ) {
                        100
                    } else {
                        300
                    },
                ))
                .easing(Easing::Linear)
                .delay(Duration::from_millis(
                    if self.kind == SourceTooltipKind::Alexa && hovered {
                        100
                    } else {
                        0
                    },
                )),
            )
            .sample(window, cx);
        let opacity = presence.progress;
        let visible = if self.kind == SourceTooltipKind::DropTips {
            // `.drop-tips` transitions visibility over 200ms; `.tip` uses 0s.
            Presence::new((self.id.clone(), "tip-visibility"), hovered)
                .transition(Transition::new(Duration::from_millis(200)).easing(Easing::Ease))
                .sample(window, cx)
                .should_render()
        } else if self.kind == SourceTooltipKind::Alexa {
            presence.should_render()
        } else {
            hovered
        };
        div()
            .id((self.id.clone(), "tip-trigger"))
            .relative()
            .flex()
            .flex_shrink_0()
            .on_hover(window.listener_for(&state, |state, hovered, _, cx| {
                state.trigger = *hovered;
                cx.notify();
            }))
            .on_prepaint({
                let anchor = anchor.clone();
                move |bounds, _, _| anchor.set(bounds)
            })
            .child((self.trigger)(hovered, window, cx))
            .child(
                deferred(TipOverlay {
                    id: self.id,
                    text: self.text,
                    width: self.width,
                    kind: self.kind,
                    anchor,
                    state,
                    hovered,
                    visible,
                    opacity,
                })
                .with_priority(
                    // `.body-widget-tip-portal{z-index:10001}`。
                    if matches!(
                        kind,
                        SourceTooltipKind::WidgetTip | SourceTooltipKind::ReceiverWidgetPortal
                    ) {
                        10001
                    } else {
                        200
                    },
                ),
            )
    }
}

fn tip_surface(id: ElementId, text: SharedString, width: Pixels) -> Tooltip {
    Tooltip::new(id)
        .w(width)
        .px(surface::css(10.))
        .py(surface::css(8.))
        .border_1()
        .border_color(TooltipColors::border())
        .bg(TooltipColors::background())
        .text_color(TooltipColors::foreground())
        .font_family("Roboto")
        .text_size(surface::css(14.))
        // `.widget .tip` keeps 18px against the 16px used by the other paths.
        .line_height(surface::css(16.))
        .whitespace_normal()
        .child(text)
}

/// `.widget .tip` hugs its content (`width:max-content`) under a 300px cap.
fn widget_tip_surface(id: ElementId, text: SharedString, max_width: Pixels) -> Tooltip {
    tip_surface(id, text, max_width)
        .w_auto()
        .max_w(max_width)
        .line_height(surface::css(18.))
}

/// `.widget .tip{right:14px;top:34px}` is measured against the widget box, while
/// the trigger is the `.widget .help` control at `right:10px;top:10px`:
/// the tip's right edge lands 4px left of the control and 24px below its top.
fn widget_tip_position(trigger: Bounds<Pixels>, width: Pixels, rem: Pixels) -> Point<Pixels> {
    point(
        trigger.right() - rem * (4. / 16.) - width,
        trigger.origin.y + rem * (24. / 16.),
    )
}

/// Measures the source's hidden `.tip` before composing the portal. RenderOnce
/// cannot read the trigger's current prepaint bounds or its tip's clientHeight.
/// Layout and paint still delegate to Base Positioner.
struct TipOverlay {
    id: ElementId,
    text: SharedString,
    width: f32,
    kind: SourceTooltipKind,
    anchor: Rc<Cell<Bounds<Pixels>>>,
    state: Entity<HoverState>,
    hovered: bool,
    visible: bool,
    opacity: f32,
}

struct TipLayout {
    positioner: Option<Positioner>,
    layout: <Positioner as Element>::RequestLayoutState,
    source_size: Size<Pixels>,
}

impl IntoElement for TipOverlay {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for TipOverlay {
    type RequestLayoutState = TipLayout;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, TipLayout) {
        let width =
            (window.rem_size() * (self.width / 16.)).min(window.viewport_size().width.max(px(1.)));
        let source_size = if self.kind == SourceTooltipKind::DropTips {
            let mut measurement = tip_surface(
                (self.id.clone(), "tip-measure").into(),
                self.text.clone(),
                width,
            )
            .into_any_element();
            let measured = measurement.layout_as_root(
                size(AvailableSpace::Definite(width), AvailableSpace::MinContent),
                window,
                cx,
            );
            // JS copies clientWidth/clientHeight (border excluded) to the
            // portal's border-box width/height. Its `.on` height is auto.
            measured.map(|length| (length - px(2.)).max(px(1.)))
        } else if self.kind == SourceTooltipKind::ProfilesNav {
            let mut measurement = tip_surface(
                (self.id.clone(), "tip-measure").into(),
                self.text.clone(),
                width,
            )
            .w_auto()
            .into_any_element();
            let measured = measurement.layout_as_root(
                size(AvailableSpace::MaxContent, AvailableSpace::MinContent),
                window,
                cx,
            );
            size(measured.width.max(px(1.)), px(0.))
        } else if matches!(
            self.kind,
            SourceTooltipKind::WidgetTip | SourceTooltipKind::ReceiverWidgetPortal
        ) {
            // `width:max-content` with `max-width:300px`; the height stays auto.
            let mut measurement = widget_tip_surface(
                (self.id.clone(), "tip-measure").into(),
                self.text.clone(),
                width,
            )
            .into_any_element();
            let measured = measurement.layout_as_root(
                size(AvailableSpace::MaxContent, AvailableSpace::MinContent),
                window,
                cx,
            );
            size(measured.width.min(width).max(px(1.)), measured.height)
        } else if self.kind == SourceTooltipKind::OledBleDisabled {
            // `[turn-off-ble-tooltip]:before` is max-content with no width cap.
            let mut measurement = tip_surface(
                (self.id.clone(), "tip-measure").into(),
                self.text.clone(),
                width,
            )
            .w_auto()
            .into_any_element();
            let measured = measurement.layout_as_root(
                size(AvailableSpace::MaxContent, AvailableSpace::MinContent),
                window,
                cx,
            );
            size(measured.width.max(px(1.)), measured.height)
        } else {
            size(width, px(0.))
        };
        let text = if self.hovered || self.kind != SourceTooltipKind::DropTips {
            self.text.clone()
        } else {
            // AR/mR remove the text on mouseleave while the empty box fades.
            SharedString::default()
        };
        let surface = tip_surface(
            (self.id.clone(), "tip-surface").into(),
            text,
            source_size.width,
        )
        .when(
            matches!(
                self.kind,
                SourceTooltipKind::Battery | SourceTooltipKind::Alexa
            ),
            |tip| tip.w_auto().max_w(source_size.width),
        )
        .when(self.kind == SourceTooltipKind::Alexa, |tip| {
            tip.px(surface::css(8.))
                .py(surface::css(7.))
                .line_height(relative(1.22))
        })
        .when(self.kind == SourceTooltipKind::WidgetTip, |tip| {
            // The measured max-content width is already the rendered width.
            tip.w(source_size.width).max_w(source_size.width)
        })
        .when(
            self.kind == SourceTooltipKind::ReceiverWidgetPortal,
            |tip| {
                tip.w(source_size.width)
                    .max_w(source_size.width)
                    .line_height(surface::css(18.))
            },
        )
        .when(
            !self.hovered && self.kind == SourceTooltipKind::DropTips,
            |tip| tip.h(source_size.height),
        );
        let content = div()
            .id((self.id.clone(), "tip-popup"))
            .test_support()
            .opacity(self.opacity)
            .when(
                matches!(
                    self.kind,
                    SourceTooltipKind::Battery | SourceTooltipKind::Alexa
                ),
                |view| view.w(width).flex().justify_end(),
            )
            .when(!self.visible, |view| view.invisible())
            .when(
                self.visible && self.kind == SourceTooltipKind::ProfileWarning,
                |view| {
                    view.on_hover(window.listener_for(&self.state, |state, hovered, _, cx| {
                        state.content = *hovered;
                        cx.notify();
                    }))
                },
            )
            .child(surface);
        let mut positioner = Positioner::corner(Anchor::TopLeft, Point::default())
            .margin(px(0.))
            .child(content);
        let (id, layout) = positioner.request_layout(None, None, window, cx);
        (
            id,
            TipLayout {
                positioner: Some(positioner),
                layout,
                source_size,
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
        let trigger = self.anchor.get();
        let target = match self.kind {
            SourceTooltipKind::DropTips => {
                drop_tip_position(trigger, layout.source_size, window.viewport_size())
            }
            SourceTooltipKind::ProfileWarning => trigger.bottom_left(),
            SourceTooltipKind::LockedProfile => {
                trigger.origin + point(window.rem_size() * 1.25, window.rem_size() * 1.25)
            }
            SourceTooltipKind::Battery => {
                let margin = window.rem_size() * (8. / 16.);
                let left = (trigger.right() - layout.source_size.width)
                    .max(margin)
                    .min(
                        (window.viewport_size().width - margin - layout.source_size.width)
                            .max(margin),
                    );
                point(left, trigger.bottom() + window.rem_size() * (5. / 16.))
            }
            SourceTooltipKind::Alexa => point(
                trigger.right() - layout.source_size.width,
                trigger.bottom() + window.rem_size() * (5. / 16.),
            ),
            SourceTooltipKind::ProfilesNav => {
                trigger.origin
                    + point(
                        window.rem_size() * (15. / 16.),
                        window.rem_size() * (30. / 16.),
                    )
            }
            // 两者都是 `.widget .help + .tip` 的 `createPortal` 提示，位置算法
            // 同属外壳 `positionTip`。
            SourceTooltipKind::WidgetTip | SourceTooltipKind::ReceiverWidgetPortal => {
                shell_tip_position(
                    trigger,
                    layout.source_size,
                    window.viewport_size(),
                    window.rem_size(),
                )
            }
            SourceTooltipKind::OledBleDisabled => {
                let unit = window.rem_size() / 16.;
                trigger.origin + point(unit * 20., unit * 185.)
            }
        };
        let position = if self.kind == SourceTooltipKind::DropTips {
            point(
                motion::transition(
                    (self.id.clone(), "tip-left"),
                    target.x,
                    Transition::new(Duration::from_millis(100)).easing(Easing::Ease),
                    window,
                    cx,
                ),
                motion::transition(
                    (self.id.clone(), "tip-top"),
                    target.y,
                    Transition::new(Duration::from_millis(100)).easing(Easing::Ease),
                    window,
                    cx,
                ),
            )
        } else {
            target
        };
        let mut positioner = layout.positioner.take().unwrap().position(position);
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

/// 组件外壳 `KrA`(3858)/`_TA`(3880) 的 `positionTip`：`.widget .help + .tip`
/// 以 `createPortal` 挂到 `document.body`，位置先取
/// `widget.right - 14 - tipWidth` / `widget.top + 34`，再按
/// `.main-container > #body-wrapper` 的矩形做溢出回退——右溢出贴
/// `container.right - 14`、左溢出贴 `container.left + 14`、下溢出去帮助图标右侧
/// （`help.right + 8`、`help.top`）、再右溢出翻到图标左侧
/// （`help.left - 8 - tipWidth`）、仍溢出则上移到 `container.bottom - 10`。
/// 每个产品包的外壳都走同一段代码，因此所有 `.widget .help` 提示都适用。
///
/// 本地用视口代替 `#body-wrapper`（下界与右界一致，左界取 0），与 179 的既有
/// `receiver_help_control` 路径保持同一近似；容器顶边源算法不使用。
fn shell_tip_position(
    trigger: Bounds<Pixels>,
    tip: Size<Pixels>,
    viewport: Size<Pixels>,
    rem: Pixels,
) -> Point<Pixels> {
    let unit = rem / 16.;
    let mut position = widget_tip_position(trigger, tip.width, rem);
    if position.x + tip.width > viewport.width {
        position.x = viewport.width - unit * 14. - tip.width;
    }
    if position.x < px(0.) {
        position.x = unit * 14.;
    }
    if position.y + tip.height > viewport.height {
        position = point(trigger.right() + unit * 8., trigger.top());
        if position.x + tip.width > viewport.width {
            position.x = trigger.left() - unit * 8. - tip.width;
        }
        if position.y + tip.height > viewport.height {
            position.y = viewport.height - tip.height - unit * 10.;
        }
    }
    position
}

fn drop_tip_position(
    trigger: Bounds<Pixels>,
    tip: Size<Pixels>,
    viewport: Size<Pixels>,
) -> Point<Pixels> {
    point(
        if trigger.right() + tip.width > viewport.width {
            trigger.left() - tip.width
        } else {
            trigger.right()
        },
        if trigger.bottom() + tip.height > viewport.height {
            trigger.bottom() - tip.height
        } else {
            trigger.bottom()
        },
    )
}
