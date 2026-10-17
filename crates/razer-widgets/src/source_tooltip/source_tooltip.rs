//! OBM portal `.drop-tips` and persistent CSS `.tip` presentation.
//! Native hover owns the trigger; Base owns motion, tooltip semantics and final
//! viewport placement. The source client-size and anchor policy lives here.
use crate::surface;
use crate::theme::TooltipColors;
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
pub enum SourceTooltipKind {
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
    /// Current Studio 2925 → 7155: same delayed portal, final main.css padding.
    Studio,
    /// Profiles `.main-nav li:hover .tooltip`: immediate, +15/+30, max-content.
    ProfilesNav,
    /// `.widget .help + .tip` / `.body-widget-tip-portal`: 14px/18px,
    /// `max-width:300px`, `right:14px;top:34px` against the widget box, and the
    /// trigger is the 14px `.widget .help` control at `right:10px;top:10px`.
    WidgetTip,
    /// Current mouse Customize `.config-row .tip.hypershift-mode-tip`:
    /// 300px border box, top:19px; at <=1279px right:1vw/top:26px.
    /// This is a persistent CSS sibling, not the widget's body portal.
    HypershiftHelp,
    /// PID231 keeps the caller's 362px width and top:19px at both breakpoints.
    HypershiftHelp231,
    /// Product 179 module 7693: conditionally mounted `.body-widget-tip-portal`,
    /// immediate visibility, 10001 stacking and source edge fallback.
    ReceiverWidgetPortal,
    /// Product 691 OLED BLE-disabled cards: `[turn-off-ble-tooltip]` pseudo
    /// element, 20px/185px card-relative anchor and 300ms linear opacity.
    OledBleDisabled,
    /// Controller `.actuation-tooltip .tip`: left aligned at top:20px, with
    /// `show-top{bottom:10px}` only when the measured lower space is too small.
    GamepadTrigger,
    /// Current Effects `.status-chroma-app .help .tip`: 410px, right:15px,
    /// top:15px with immediate visibility and a 300ms linear opacity change.
    ChromaStatus,
    /// `.manage .tip` overrides width with fit-content and min-width:165px.
    ChromaManage,
}

type Trigger = Box<dyn FnOnce(bool, &mut Window, &mut App) -> AnyElement>;

#[derive(Clone, Copy)]
pub struct HypershiftHelpLayout {
    width: f32,
    top: f32,
    narrow_top: Option<f32>,
    line_height: f32,
}

impl HypershiftHelpLayout {
    pub fn new(width: f32, top: f32) -> Self {
        Self {
            width,
            top,
            narrow_top: None,
            line_height: 18.,
        }
    }

    pub fn narrow_top(mut self, top: f32) -> Self {
        self.narrow_top = Some(top);
        self
    }

    pub fn line_height(mut self, height: f32) -> Self {
        self.line_height = height;
        self
    }

    fn position(
        self,
        trigger: Bounds<Pixels>,
        width: Pixels,
        viewport: Size<Pixels>,
        rem: Pixels,
    ) -> Point<Pixels> {
        let unit = rem / 16.;
        if let Some(top) = self.narrow_top.filter(|_| viewport.width <= unit * 1279.) {
            point(
                trigger.right() - viewport.width * 0.01 - width,
                trigger.top() + unit * top,
            )
        } else {
            point(trigger.left(), trigger.top() + unit * self.top)
        }
    }
}

#[derive(IntoElement)]
pub struct SourceTooltip {
    id: ElementId,
    text: SharedString,
    width: f32,
    kind: SourceTooltipKind,
    hovered_priority: Option<usize>,
    dismiss_on_press: bool,
    hypershift_layout: Option<HypershiftHelpLayout>,
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
    pub fn new(id: impl Into<ElementId>, text: impl Into<SharedString>, width: f32) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            width,
            kind: SourceTooltipKind::DropTips,
            hovered_priority: None,
            dismiss_on_press: false,
            hypershift_layout: None,
            trigger: Box::new(|_, _, _| div().into_any_element()),
        }
    }

    pub fn kind(mut self, kind: SourceTooltipKind) -> Self {
        self.kind = kind;
        self
    }

    /// Source controls may hide their delayed portal on the initial press.
    pub fn dismiss_on_press(mut self) -> Self {
        self.dismiss_on_press = true;
        self
    }

    pub fn hypershift_layout(mut self, layout: HypershiftHelpLayout) -> Self {
        self.kind = SourceTooltipKind::HypershiftHelp;
        self.width = layout.width;
        self.hypershift_layout = Some(layout);
        self
    }

    /// Override the deferred drawing priority only while hovered. The current
    /// camera `.drop-tips.on` uses 9999, then returns to `.drop-tips`'s 200 as
    /// soon as `.on` is removed, even while its visibility/opacity fades out.
    /// Callers without an override retain their kind's existing layer policy.
    pub fn hovered_priority(mut self, priority: usize) -> Self {
        self.hovered_priority = Some(priority);
        self
    }

    fn draw_priority(&self, hovered: bool) -> usize {
        let default = if matches!(
            self.kind,
            SourceTooltipKind::HypershiftHelp
                | SourceTooltipKind::HypershiftHelp231
                | SourceTooltipKind::GamepadTrigger
                | SourceTooltipKind::ChromaStatus
                | SourceTooltipKind::ChromaManage
        ) {
            if hovered { 100 } else { 99 }
        } else if self.kind == SourceTooltipKind::Studio {
            1060
        } else if matches!(
            self.kind,
            SourceTooltipKind::WidgetTip | SourceTooltipKind::ReceiverWidgetPortal
        ) {
            // `.body-widget-tip-portal{z-index:10001}`.
            10001
        } else {
            200
        };
        if hovered {
            self.hovered_priority.unwrap_or(default)
        } else {
            default
        }
    }

    pub fn trigger(
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
                let activation = matches!(
                    kind,
                    SourceTooltipKind::Alexa | SourceTooltipKind::Studio
                )
                .then(|| {
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
        let priority = self.draw_priority(hovered);
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
                        SourceTooltipKind::Battery
                            | SourceTooltipKind::Alexa
                            | SourceTooltipKind::Studio
                    ) {
                        100
                    } else {
                        300
                    },
                ))
                .easing(Easing::Linear)
                .delay(Duration::from_millis(
                    if matches!(
                        self.kind,
                        SourceTooltipKind::Alexa | SourceTooltipKind::Studio
                    ) && hovered
                    {
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
        } else if matches!(
            self.kind,
            SourceTooltipKind::Alexa | SourceTooltipKind::Studio
        ) {
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
            .when(self.dismiss_on_press, |view| {
                view.on_any_mouse_down(window.listener_for(&state, |state, _, _, cx| {
                    state.trigger = false;
                    cx.notify();
                }))
            })
            .on_prepaint({
                let anchor = anchor.clone();
                move |bounds, _, _| anchor.set(bounds)
            })
            .child((self.trigger)(hovered, window, cx))
            .child(
                razer_assets::deferred_resource(TipOverlay {
                    id: self.id,
                    text: self.text,
                    width: self.width,
                    kind: self.kind,
                    hypershift_layout: self.hypershift_layout,
                    anchor,
                    state,
                    hovered,
                    visible,
                    opacity,
                })
                .with_priority(priority),
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
    hypershift_layout: Option<HypershiftHelpLayout>,
    anchor: Rc<Cell<Bounds<Pixels>>>,
    state: Entity<HoverState>,
    hovered: bool,
    visible: bool,
    opacity: f32,
}

struct TipLayout {
    positioner: Option<Positioner>,
    layout: Option<<Positioner as Element>::RequestLayoutState>,
    exact_content: Option<AnyElement>,
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
        let width = window.rem_size() * (self.width / 16.);
        let width = if matches!(
            self.kind,
            SourceTooltipKind::HypershiftHelp | SourceTooltipKind::HypershiftHelp231
                | SourceTooltipKind::ChromaStatus | SourceTooltipKind::ChromaManage
        ) {
            // CSS min-width/max-width are both 300px; there is no edge fallback.
            width
        } else {
            width.min(window.viewport_size().width.max(px(1.)))
        };
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
        } else if self.kind == SourceTooltipKind::ChromaManage {
            let mut measurement=tip_surface((self.id.clone(),"tip-measure").into(),self.text.clone(),width).w_auto().min_w(width).into_any_element();
            let measured=measurement.layout_as_root(size(AvailableSpace::MaxContent,AvailableSpace::MinContent),window,cx);
            size(measured.width.max(width),measured.height)
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
            SourceTooltipKind::WidgetTip
                | SourceTooltipKind::ReceiverWidgetPortal
                | SourceTooltipKind::GamepadTrigger
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
                SourceTooltipKind::Battery | SourceTooltipKind::Alexa | SourceTooltipKind::Studio
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
            matches!(
                self.kind,
                SourceTooltipKind::HypershiftHelp | SourceTooltipKind::HypershiftHelp231
            ),
            |tip| {
                tip.min_w(source_size.width)
                    .max_w(source_size.width)
                    .line_height(surface::css(
                        self.hypershift_layout
                            .map_or(18., |layout| layout.line_height),
                    ))
            },
        )
        .when(
            matches!(
                self.kind,
                SourceTooltipKind::ReceiverWidgetPortal | SourceTooltipKind::GamepadTrigger
            ),
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
                    SourceTooltipKind::Battery
                        | SourceTooltipKind::Alexa
                        | SourceTooltipKind::Studio
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
        if matches!(
            self.kind,
            SourceTooltipKind::HypershiftHelp
                | SourceTooltipKind::HypershiftHelp231
                | SourceTooltipKind::GamepadTrigger
                | SourceTooltipKind::ChromaStatus
                | SourceTooltipKind::ChromaManage
        ) {
            // Base Positioner always clamps to the viewport. The source CSS
            // sibling has no clamp, so retain Base Tooltip semantics and place
            // this particular surface using the source's exact CSS coordinates.
            let mut content = content.into_any_element();
            let child = content.request_layout(window, cx);
            let id = window.request_layout(
                Style {
                    position: Position::Absolute,
                    ..Style::default()
                },
                [child],
                cx,
            );
            return (
                id,
                TipLayout {
                    positioner: None,
                    layout: None,
                    exact_content: Some(content),
                    source_size,
                },
            );
        }
        let mut positioner = Positioner::corner(Anchor::TopLeft, Point::default())
            .margin(px(0.))
            .child(content);
        let (id, layout) = positioner.request_layout(None, None, window, cx);
        (
            id,
            TipLayout {
                positioner: Some(positioner),
                layout: Some(layout),
                exact_content: None,
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
            SourceTooltipKind::Alexa | SourceTooltipKind::Studio => point(
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
            SourceTooltipKind::GamepadTrigger => gamepad_trigger_tip_position(
                trigger,
                layout.source_size.height,
                window.viewport_size(),
                window.rem_size(),
            ),
            SourceTooltipKind::ChromaStatus | SourceTooltipKind::ChromaManage => point(
                trigger.right()-window.rem_size()*(15./16.)-layout.source_size.width,
                trigger.top()+window.rem_size()*(15./16.),
            ),
            SourceTooltipKind::HypershiftHelp => self
                .hypershift_layout
                .unwrap_or_else(|| HypershiftHelpLayout::new(self.width, 19.).narrow_top(26.))
                .position(
                    trigger,
                    layout.source_size.width,
                    window.viewport_size(),
                    window.rem_size(),
                ),
            SourceTooltipKind::HypershiftHelp231 => hypershift_help_position(
                trigger,
                layout.source_size.width,
                window.viewport_size(),
                window.rem_size(),
                19.,
            ),
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
        if let Some(content) = &mut layout.exact_content {
            window.with_element_offset(position - bounds.origin, |window| {
                content.prepaint(window, cx)
            });
            return;
        }
        let mut positioner = layout.positioner.take().unwrap().position(position);
        positioner.prepaint(
            None,
            None,
            bounds,
            layout.layout.as_mut().unwrap(),
            window,
            cx,
        );
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
        if let Some(content) = &mut layout.exact_content {
            content.paint(window, cx);
            return;
        }
        layout.positioner.as_mut().unwrap().paint(
            None,
            None,
            bounds,
            layout.layout.as_mut().unwrap(),
            &mut (),
            window,
            cx,
        );
    }
}

fn gamepad_trigger_tip_position(
    trigger: Bounds<Pixels>,
    height: Pixels,
    viewport: Size<Pixels>,
    rem: Pixels,
) -> Point<Pixels> {
    let unit = rem / 16.;
    let above = viewport.height - trigger.bottom() < height && trigger.top() > height;
    point(
        trigger.left(),
        if above {
            trigger.bottom() - unit * 10. - height
        } else {
            trigger.top() + unit * 20.
        },
    )
}

/// PID190 main.7ce428ff.css rules 95303..95899 and CO's relative help wrapper
/// in current main.81b09779.js. The media query uses the scaled source viewport;
/// `1vw` remains one percent of the full window, independent of the rem scale.
fn hypershift_help_position(
    trigger: Bounds<Pixels>,
    width: Pixels,
    viewport: Size<Pixels>,
    rem: Pixels,
    narrow_top: f32,
) -> Point<Pixels> {
    let unit = rem / 16.;
    if viewport.width <= unit * 1279. {
        point(
            trigger.right() - viewport.width * 0.01 - width,
            trigger.top() + unit * narrow_top,
        )
    } else {
        point(trigger.left(), trigger.top() + unit * 19.)
    }
}

#[cfg(test)]
mod hypershift_geometry_tests {
    use super::{HypershiftHelpLayout, hypershift_help_position};
    use gpui_kit::{Bounds, point, px, size};

    #[::core::prelude::v1::test]
    fn source_breakpoint_keeps_fixed_tip_width_and_does_not_apply_portal_edge_fallback() {
        let trigger = Bounds::new(point(px(100.), px(300.)), size(px(14.), px(14.)));
        let narrow =
            hypershift_help_position(trigger, px(300.), size(px(1279.), px(400.)), px(16.), 26.);
        assert_eq!(narrow, point(px(114. - 12.79 - 300.), px(326.)));
        assert!(
            narrow.x < px(0.),
            "source CSS can extend beyond its parent/window"
        );
        let wide =
            hypershift_help_position(trigger, px(300.), size(px(1280.), px(400.)), px(16.), 26.);
        assert_eq!(wide, point(px(100.), px(319.)));
        let variant =
            hypershift_help_position(trigger, px(362.), size(px(1279.), px(400.)), px(16.), 19.);
        assert_eq!(variant.x, narrow.x - px(62.));
        assert_eq!(
            variant.y, wide.y,
            "PID231 does not change top at the narrow breakpoint"
        );
    }

    #[::core::prelude::v1::test]
    fn source_media_width_tracks_interface_zoom_while_one_vw_uses_the_entire_viewport() {
        let trigger = Bounds::new(point(px(500.), px(100.)), size(px(21.), px(21.)));
        let tip =
            hypershift_help_position(trigger, px(450.), size(px(1918.5), px(600.)), px(24.), 26.);
        assert_eq!(tip, point(px(521. - 19.185 - 450.), px(139.)));
        let wide =
            hypershift_help_position(trigger, px(450.), size(px(1920.), px(600.)), px(24.), 26.);
        assert_eq!(wide, point(px(500.), px(128.5)));
    }

    #[::core::prelude::v1::test]
    fn keyboard_inline_tip_does_not_inherit_responsive_class_offsets() {
        let trigger = Bounds::new(point(px(100.), px(300.)), size(px(14.), px(14.)));
        let layout = HypershiftHelpLayout::new(362., 20.).line_height(16.);
        for viewport in [size(px(900.), px(400.)), size(px(1400.), px(400.))] {
            assert_eq!(
                layout.position(trigger, px(layout.width), viewport, px(16.)),
                point(px(100.), px(320.)),
            );
        }
        let normal_flow = HypershiftHelpLayout::new(362., 14.);
        assert_eq!(
            normal_flow.position(trigger, px(362.), size(px(900.), px(400.)), px(16.)),
            point(trigger.left(), trigger.bottom()),
        );
    }

    #[::core::prelude::v1::test]
    fn keyboard_responsive_tip_preserves_inline_top_and_uncapped_width_at_zoom() {
        let trigger = Bounds::new(point(px(500.), px(100.)), size(px(21.), px(21.)));
        let viewport = size(px(1918.5), px(600.));
        let uncapped = HypershiftHelpLayout::new(362., 19.).narrow_top(26.);
        assert_eq!(
            uncapped.position(trigger, px(543.), viewport, px(24.)),
            point(px(521. - 19.185 - 543.), px(139.)),
        );
        let inline_top = HypershiftHelpLayout::new(300., 20.).narrow_top(20.);
        assert_eq!(
            inline_top.position(trigger, px(450.), viewport, px(24.)),
            point(px(521. - 19.185 - 450.), px(130.)),
        );
        assert_eq!(
            inline_top.position(trigger, px(450.), size(px(1920.), px(600.)), px(24.)),
            point(px(500.), px(130.)),
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
