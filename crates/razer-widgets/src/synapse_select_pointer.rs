//! Verified module 7734/7278 pointer presentations. Mouse 190/229/239 retain
//! viewport-overflow placement; audio 1422 opts into its wrapper/edge branch.
use super::*;
use crate::scroll::SourceScrollable as _;
use crate::keyboard_navigation::Button as BaseButton;
use gpui_kit::base::{Positioner};
use gpui_kit::component::v_flex;

impl<I: SelectItem<Value = String> + 'static> SynapseSelectView<I> {
    pub(super) fn render_pointer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let options = self
            .list
            .read(cx)
            .delegate()
            .items
            .iter()
            .map(|item| (item.value().clone(), item.title(), item.disabled()))
            .collect::<Vec<_>>();
        let selected = self.state.read(cx).selected_value().cloned();
        let current = options
            .iter()
            .find(|(value, _, _)| Some(value) == selected.as_ref());
        let Some(title) = self
            .selected_text
            .clone()
            .or_else(|| current.map(|current| current.1.clone()))
        else {
            return div().into_any_element();
        };
        let current_image = current
            .and_then(|(value, _, _)| self.pointer_decorations.get(value))
            .and_then(|decoration| decoration.content_image.as_ref());
        let disabled = self.is_disabled(cx);
        let open = self.open && !disabled;
        let focus = self.state.focus_handle(cx).tab_stop(true);
        let generation = self.generation;
        let colors = DropdownColors::new();
        let bounds = self.pointer_bounds.clone();
        let border = motion::transition(
            (self.id.clone(), "source-pointer-border"),
            if !disabled && (open || self.hovered) {
                cx.theme().primary
            } else {
                cx.theme().input
            },
            Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
            window,
            cx,
        );
        let arrow = motion::transition(
            (self.id.clone(), "source-pointer-arrow"),
            if open { std::f32::consts::PI } else { 0. },
            Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
            window,
            cx,
        );
        let click = self.on_trigger_click.clone();
        let trigger = BaseButton::new("source-pointer-trigger")
            .track_focus(&focus)
            .disabled(disabled)
            .accessibility_label(self.label.clone().unwrap_or_else(|| title.clone()))
            .w_full()
            .h(self.pointer_style.trigger_height)
            .when(
                !matches!(
                    self.pointer_placement,
                    PointerSelectPlacement::ViewportOverflow
                ),
                |trigger| trigger.aria_expanded(open),
            )
            .border_1()
            .border_color(border)
            .text_size(css(14.))
            .line_height(css(17.))
            .text_color(cx.theme().foreground)
            .relative()
            .when(disabled, |button| button.opacity(0.3))
            .on_hover(cx.listener(|this, hovered, _, cx| {
                this.hovered = *hovered;
                cx.notify();
            }))
            .when(current_image.is_some(), |trigger| {
                trigger
                    .flex()
                    .items_center()
                    .justify_center()
                    .px(css(5.))
                    .py(css(4.))
            })
            .child(if let Some(image) = current_image {
                div().flex().items_center().overflow_hidden().child(
                    img(image.path.clone())
                        .w(css(image.width))
                        .h(css(image.height)),
                )
            } else {
                div()
                    .absolute()
                    .left(self.pointer_style.text_inset)
                    .top(css(4.))
                    .right(self.pointer_style.text_trailing_inset)
                    .truncate()
                    .child(title.clone())
            })
            .child(
                div()
                    .when(current_image.is_some(), |arrow| {
                        arrow.relative().ml_auto().flex_shrink_0()
                    })
                    .when(current_image.is_none(), |arrow| {
                        arrow.absolute().right_0().top_0()
                    })
                    .w(css(29.))
                    .h(css(25.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        razer_assets::resource_svg()
                            .path("synapse/expand.svg")
                            .size(css(10.))
                            .flex_shrink_0()
                            .with_transformation(Transformation::rotate(radians(arrow))),
                    ),
            )
            .on_click(cx.listener(move |this, event, window, cx| {
                if !disabled {
                    this.set_open(!this.open, window, cx);
                }
                if let Some(click) = &click {
                    click(event, window, cx);
                }
            }));
        let decision_height = menu_decision_height(options.len());
        let menu_height = self.pointer_style.menu_height(options.len());
        let rows = options
            .iter()
            .map(|(value, title, disabled)| {
                let value = value.clone();
                let checked = selected.as_ref() == Some(&value);
                let disabled = *disabled;
                let focus = self
                    .list
                    .read(cx)
                    .delegate()
                    .focus
                    .get(&value)
                    .cloned()
                    .expect("Every current option owns a stable focus handle");
                let image = self
                    .pointer_decorations
                    .get(&value)
                    .and_then(|decoration| decoration.content_image.as_ref());
                let row =
                    BaseButton::new(SharedString::from(format!("source-pointer-option-{value}")))
                        .track_focus(&focus)
                        .tab_stop(false)
                        .role(Role::ListBoxOption)
                        .aria_selected(checked)
                        .disabled(disabled)
                        .w_full()
                        .accessibility_label(title.clone())
                        .h(self.pointer_style.row_height)
                        .flex_shrink_0()
                        .p(css(4.))
                        .text_size(css(14.))
                        .line_height(css(17.))
                        .justify_start()
                        .when(image.is_some(), |row| {
                            row.p(css(8.)).items_center().justify_center()
                        })
                        .text_color(if checked {
                            cx.theme().primary
                        } else {
                            cx.theme().foreground
                        })
                        .when(!disabled, |row| {
                            row.hover(|style| style.bg(colors.hover()))
                                .active(|style| style.text_color(cx.theme().primary))
                        })
                        .when(disabled, |row| row.opacity(0.3))
                        .child(if let Some(image) = image {
                            div().flex().items_center().justify_center().child(
                                img(image.path.clone())
                                    .w(css(image.width))
                                    .h(css(image.height)),
                            )
                        } else {
                            div().w_full().truncate().child(title.clone())
                        })
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.confirm(value.clone(), generation, window, cx)
                        }));
                focus_target(row, &focus, FocusBehavior::LocalVertical, !disabled)
                    .ring(FocusRing::Options)
            })
            .collect::<Vec<_>>();
        let width = self.width;
        div()
            .id("source-pointer-dropdown")
            .relative()
            .w_full()
            .h(self.pointer_style.trigger_height)
            .refine_style(&self.style)
            .on_prepaint({
                let owner = cx.entity().downgrade();
                move |current, window, cx| {
                    let _ = owner.update(cx, |this, cx| this.did_layout(current, window, cx));
                }
            })
            .child(focus_target(
                trigger,
                &focus,
                FocusBehavior::LocalVertical,
                !disabled,
            ))
            .when(open, |root| {
                let reveal = Presence::new((self.id.clone(), "source-pointer-reveal"), true)
                    .transition(Transition::new(Duration::from_millis(200)).easing(Easing::Ease))
                    .sample(window, cx)
                    .progress;
                let revealed_height = revealed_menu_height(menu_height, reveal);
                let positioned_height = if matches!(
                    self.pointer_placement,
                    PointerSelectPlacement::ViewportOverflow
                ) {
                    menu_height
                } else {
                    revealed_height
                };
                let content = div()
                    .id("source-pointer-options")
                    .w(width)
                    .h(positioned_height)
                    .child(
                        v_flex()
                            .id("source-pointer-options-clip")
                            .test_support()
                            .w_full()
                            .h(revealed_height)
                            .border_1()
                            .border_color(cx.theme().input)
                            .bg(colors.surface())
                            .pl(self.pointer_style.menu_padding_left)
                            .overflow_x_hidden()
                            .on_mouse_up_out(
                                MouseButton::Left,
                                cx.listener(|this, event: &MouseUpEvent, window, cx| {
                                    // Source window.click ignores both trigger and option descendants.
                                    if !this.pointer_bounds.get().contains(&event.position) {
                                        this.set_open(false, window, cx);
                                    }
                                }),
                            )
                            .children(rows)
                            .scrollable_y()
                            .track_scroll(&self.pointer_scroll),
                    )
                    .into_any_element();
                root.child(
                    razer_assets::deferred_resource(PointerOptions {
                        bounds,
                        height: positioned_height.to_pixels(window.rem_size()),
                        decision_height: decision_height.to_pixels(window.rem_size()),
                        placement: self.pointer_placement.clone(),
                        position: self.pointer_position.clone(),
                        content: Some(content),
                    })
                    .with_priority(103),
                )
            })
            .into_any_element()
    }
}

/// Resolve after trigger prepaint. The audio branch uses the source's absolute
/// 27/28px offsets even when its 25px estimate is smaller than the rendered menu.
struct PointerOptions {
    bounds: Rc<std::cell::Cell<Bounds<Pixels>>>,
    height: Pixels,
    decision_height: Pixels,
    placement: PointerSelectPlacement,
    position: Rc<std::cell::Cell<PointerPosition>>,
    content: Option<AnyElement>,
}

enum PointerOptionsLayout {
    Viewport(Positioner, <Positioner as Element>::RequestLayoutState),
    Source(AnyElement, LayoutId),
}

#[derive(Clone, Copy, Default)]
enum PointerDirection {
    #[default]
    FlowDown,
    Up,
    Down,
}

#[derive(Clone, Copy, Default)]
pub(super) struct PointerPosition {
    direction: PointerDirection,
    pending: bool,
}

impl PointerPosition {
    pub(super) fn begin_open(mut self) -> Self {
        self.pending = true;
        self
    }
}
impl IntoElement for PointerOptions {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for PointerOptions {
    type RequestLayoutState = PointerOptionsLayout;
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
    ) -> (LayoutId, Self::RequestLayoutState) {
        if !matches!(self.placement, PointerSelectPlacement::ViewportOverflow) {
            let mut content = self.content.take().unwrap();
            let layout = content.request_layout(window, cx);
            return (layout, PointerOptionsLayout::Source(content, layout));
        }
        let mut positioner = Positioner::corner(Anchor::TopLeft, Point::default())
            .margin(px(0.))
            .occlude()
            .child(self.content.take().unwrap());
        let (layout, state) = positioner.request_layout(None, None, window, cx);
        (layout, PointerOptionsLayout::Viewport(positioner, state))
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let trigger = self.bounds.get();
        let gap = window.rem_size() / 16.;
        let mut position = self.position.get();
        let y = self.placement.menu_top(
            trigger,
            self.height,
            self.decision_height,
            window.viewport_size().height,
            gap,
            &mut position,
        );
        self.position.set(position);
        let origin = point(trigger.left(), y);
        match state {
            PointerOptionsLayout::Viewport(positioner, layout) => {
                *positioner = std::mem::replace(
                    positioner,
                    Positioner::corner(Anchor::TopLeft, Point::default()),
                )
                .position(origin);
                positioner.prepaint(None, None, bounds, layout, window, cx);
            }
            PointerOptionsLayout::Source(content, layout) => {
                let size = window.layout_bounds(*layout).size;
                window.insert_hitbox(Bounds::new(origin, size), HitboxBehavior::BlockMouse);
                content.prepaint_at(origin, window, cx);
            }
        }
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        match state {
            PointerOptionsLayout::Viewport(positioner, layout) => {
                positioner.paint(None, None, bounds, layout, &mut (), window, cx);
            }
            PointerOptionsLayout::Source(content, _) => content.paint(window, cx),
        }
    }
}

impl PointerSelectPlacement {
    fn menu_top(
        &self,
        trigger: Bounds<Pixels>,
        height: Pixels,
        decision_height: Pixels,
        viewport_height: Pixels,
        scale: Pixels,
        position: &mut PointerPosition,
    ) -> Pixels {
        let decision_bottom = trigger.bottom() + scale * 2. + decision_height;
        if matches!(self, Self::ViewportOverflow) {
            return if decision_bottom > viewport_height {
                trigger.top() - scale - height
            } else {
                trigger.bottom() + scale
            };
        }
        let up = match self {
            Self::ViewportOverflow => unreachable!(),
            Self::ViewportBoundary => decision_bottom >= viewport_height,
            Self::WrapperBoundary(bounds) => {
                let wrapper_bottom = bounds.get().bottom();
                let boundary = if wrapper_bottom > px(0.) {
                    wrapper_bottom
                } else {
                    viewport_height
                };
                if decision_bottom > boundary {
                    decision_bottom > viewport_height
                } else {
                    decision_bottom >= boundary
                }
            }
        };
        if position.pending {
            position.direction = match (position.direction, up) {
                (_, true) => PointerDirection::Up,
                (PointerDirection::FlowDown, false) => PointerDirection::FlowDown,
                (_, false) => PointerDirection::Down,
            };
            position.pending = false;
        }
        match position.direction {
            // The source starts with down and no inline top/bottom. It writes
            // those properties only when componentDidUpdate changes direction.
            PointerDirection::FlowDown => trigger.bottom() + scale,
            // Module 7734 retains top27/margin1 and bottom28 even when the
            // caller's CSS changes the trigger to 35px and option rows to 32px.
            PointerDirection::Up => trigger.bottom() - scale * 28. - height,
            PointerDirection::Down => trigger.top() + scale * 28.,
        }
    }
}

fn menu_decision_height(count: usize) -> Rems {
    css((count as f32 * 25. + 2.).min(180.))
}

fn revealed_menu_height(menu_height: Rems, progress: f32) -> Rems {
    rems(menu_height.0.min(css(180. * progress).0))
}

impl PointerSelectStyle {
    fn menu_height(&self, count: usize) -> Rems {
        rems((self.row_height.0 * count as f32 + css(2.).0).min(css(180.).0))
    }
}

#[cfg(test)]
mod pointer_placement_tests {
    use super::{
        PointerPosition, PointerSelectPlacement, PointerSelectStyle, css, menu_decision_height,
        revealed_menu_height,
    };
    use gpui_kit::{Bounds, Pixels, point, px, size};
    use std::{cell::Cell, prelude::v1::test, rc::Rc};

    fn trigger(height: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(20.), px(200.)), size(px(96.), px(height)))
    }

    #[test]
    fn pointer_audio_wrapper_uses_both_boundaries_and_preserves_equality() {
        // Current 1422 module7734: bottom+2+min(180,25*n+2) = 281.
        for (wrapper, viewport, expected) in [
            (281., 500., 147.),
            (280., 500., 228.),
            (282., 500., 228.),
            (280., 280., 147.),
            (400., 281., 228.),
            (0., 281., 147.),
            (0., 282., 228.),
        ] {
            let bounds = Rc::new(Cell::new(Bounds::new(
                point(px(0.), px(0.)),
                size(px(500.), px(wrapper)),
            )));
            let placement = PointerSelectPlacement::WrapperBoundary(bounds);
            assert_eq!(
                placement.menu_top(
                    trigger(27.),
                    px(52.),
                    px(52.),
                    px(viewport),
                    px(1.),
                    &mut PointerPosition::default().begin_open(),
                ),
                px(expected),
                "wrapper={wrapper}, viewport={viewport}",
            );
        }
    }

    #[test]
    fn pointer_audio_filter_uses_25px_estimate_and_retains_inline_direction() {
        // Current 1422 CSS: five image rows32 + borders2; module7734 still
        // estimates rows25. A fresh down menu has no inline top property.
        let style = PointerSelectStyle::new()
            .trigger_height(css(35.))
            .row_height(css(32.));
        let height = style.menu_height(5).to_pixels(px(16.));
        let estimate = menu_decision_height(5).to_pixels(px(16.));
        assert_eq!(height, px(162.));
        assert_eq!(estimate, px(127.));
        let placement = PointerSelectPlacement::ViewportBoundary;
        let mut position = PointerPosition::default().begin_open();
        let top = |viewport, position: &mut PointerPosition| {
            placement.menu_top(
                trigger(35.),
                height,
                estimate,
                px(viewport),
                px(1.),
                position,
            )
        };
        assert_eq!(top(365., &mut position), px(236.));
        // Resize while open does not recompute source openDirection.
        assert_eq!(top(364., &mut position), px(236.));
        position = position.begin_open();
        assert_eq!(top(364., &mut position), px(45.));
        position = position.begin_open();
        assert_eq!(top(365., &mut position), px(228.));
        // The source retains top27 after that first direction change.
        position = position.begin_open();
        assert_eq!(top(600., &mut position), px(228.));

        // The source animates max-height180. Its up menu keeps bottom28 while
        // growing, and five 32px rows reach their own height before the cap.
        let middle = revealed_menu_height(style.menu_height(5), 0.5).to_pixels(px(16.));
        assert_eq!(middle, px(90.));
        assert_eq!(
            revealed_menu_height(style.menu_height(5), 0.9).to_pixels(px(16.)),
            px(162.),
        );
        position = position.begin_open();
        assert_eq!(
            placement.menu_top(
                trigger(35.),
                middle,
                estimate,
                px(364.),
                px(1.),
                &mut position
            ),
            px(117.),
        );
        assert_eq!(top(364., &mut position), px(45.));
    }

    #[test]
    fn pointer_mouse_keeps_strict_overflow_and_source_gap_at_zoom() {
        let mut position = PointerPosition::default().begin_open();
        assert_eq!(
            PointerSelectPlacement::ViewportOverflow.menu_top(
                trigger(27.),
                px(52.),
                px(52.),
                px(281.),
                px(1.),
                &mut position,
            ),
            px(228.),
        );
        let scaled = Bounds::new(point(px(25.), px(250.)), size(px(120.), px(33.75)));
        assert_eq!(
            PointerSelectPlacement::ViewportOverflow.menu_top(
                scaled,
                px(65.),
                px(65.),
                px(351.25),
                px(1.25),
                &mut position,
            ),
            px(285.),
        );
        assert_eq!(
            PointerSelectPlacement::ViewportBoundary.menu_top(
                scaled,
                px(65.),
                px(65.),
                px(351.25),
                px(1.25),
                &mut PointerPosition::default().begin_open(),
            ),
            px(183.75),
        );
    }
}
