//! Current 653 Up/Pp media popup, mounted by _M at the roller shape.
use super::*;
use razer_widgets::keyboard_navigation::Button as BaseButton;
use razer_widgets::theme::KeyboardSpecialColors as Colors;

impl DeviceWorkspace {
    fn media_mapping_selected(&self) -> bool {
        self.mapping
            .as_ref()
            .is_some_and(|mapping| matches!(mapping.input.as_str(), "ScrollUp" | "ScrollDown"))
    }
    pub(in crate::features) fn keyboard_media_pointer(
        &mut self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self.pid() != 653 || !self.keyboard_key_tip.media_shown {
            return;
        }
        let over_anchor = self
            .keyboard_key_tip
            .regions
            .get("DIAL_CLICK")
            .is_some_and(|bounds| bounds.get().contains(&position));
        let over_popup = self.keyboard_key_tip.media_bounds.get().contains(&position);
        if !over_anchor && !over_popup && !self.media_mapping_selected() {
            self.keyboard_key_tip.media_shown = false;
            if self
                .keyboard_key_tip
                .target
                .as_deref()
                .is_some_and(|input| matches!(input, "ScrollUp" | "ScrollDown"))
            {
                self.keyboard_key_tip.reset();
            }
            cx.notify();
        }
    }
    pub(in crate::features) fn keyboard_media_tip(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if self.pid() != 653 || self.page != Tab::Customize {
            return None;
        }
        // .tip-special transitions opacity for .3s, while visibility hides it
        // immediately. Keep sampling while hidden so a re-entry can reverse.
        let opacity = gpui_kit::base::motion::Presence::new(
            "keyboard-media-opacity",
            self.keyboard_key_tip.media_shown,
        )
        .transition(
            gpui_kit::base::motion::Transition::new(std::time::Duration::from_millis(300))
                .easing(gpui_kit::base::motion::Easing::Ease),
        )
        .sample(window, cx)
        .progress;
        if !self.keyboard_key_tip.media_shown {
            return None;
        }
        let anchor = self.keyboard_key_tip.regions.get("DIAL_CLICK")?.clone();
        let source = razer_assets::keyboard_source_for_layout(self.device().layout_id)?;
        let bindings = if self.hypershift {
            &self.settings().hypershift_bindings
        } else {
            &self.settings().bindings
        };
        let mut rows = v_flex()
            .absolute()
            .top(surface::css(55.))
            .left(surface::css(10.));
        for input in source
            .iter()
            .flat_map(razer_assets::KeyboardSourceGroup::inputs)
            .filter(|input| {
                matches!(
                    input.button_key(),
                    "DIAL_LEFT_ROTATION" | "DIAL_RIGHT_ROTATION"
                )
            })
        {
            let id = input.id().to_owned();
            let binding = bindings
                .get(&id)
                .filter(|value| value.as_str() != "default");
            let disabled = binding.is_some_and(|value| self.mapping_is_disabled(value));
            let mapped = binding.is_some() && !disabled;
            let name = binding
                .map(|value| self.mapping_summary(value).1)
                .unwrap_or_else(|| {
                    super::super::super::keyboard_products::keyboard_special_label(
                        input.assignment_value(),
                    )
                });
            let color = if mapped && self.hypershift {
                Colors::hyper()
            } else if mapped {
                Colors::accent()
            } else if disabled {
                Colors::disabled()
            } else {
                Colors::text()
            };
            let icon = if input.button_key() == "DIAL_LEFT_ROTATION" {
                "synapse/keyboard653-current/icon_scrolldown.63625b62.svg"
            } else {
                "synapse/keyboard653-current/icon_scrollup.d58ba5fc.svg"
            };
            let hovered = id.clone();
            let tip_bounds = self.keyboard_key_tip.regions.get(&id)?.clone();
            rows = rows.child(
                BaseButton::new(SharedString::from(format!("keyboard-media-{id}")))
                    .relative()
                    .h(surface::css(30.))
                    .ml(surface::css(33.))
                    .mb(surface::css(10.))
                    .px(surface::css(10.))
                    .border_1()
                    .border_color(Colors::transparent())
                    .rounded(surface::css(3.))
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .text_color(color)
                    .hover(|style| style.bg(Colors::hover()))
                    .active(|style| {
                        style
                            .bg(Colors::background())
                            .border_color(Colors::accent())
                    })
                    .focus_visible(|style| style.border_color(Colors::accent()))
                    .hover_listener_mode(HoverListenerMode::InputModalityIndependent)
                    .on_hover(cx.listener(move |this, entered: &bool, _, cx| {
                        this.hover_keyboard_key_tip(&hovered, *entered, cx)
                    }))
                    .child(
                        img(icon)
                            .absolute()
                            .left(surface::css(-35.))
                            .top(surface::css(5.))
                            .w(surface::css(32.))
                            .h(surface::css(20.)),
                    )
                    .child(
                        div()
                            .ml(surface::css(10.))
                            .hover(|style| style.text_color(Colors::text()))
                            .max_w(surface::css(155.))
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(
                                div()
                                    .id(SharedString::from(format!("keyboard-media-label-{id}")))
                                    .test_support()
                                    .aria_label(name.clone())
                                    .on_prepaint(move |bounds, _, _| tip_bounds.set(bounds))
                                    .child(name),
                            ),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if this
                            .mapping
                            .as_ref()
                            .is_none_or(|mapping| mapping.input != id)
                        {
                            this.continue_with(Continue::Input(id.clone()), window, cx);
                        }
                    })),
            );
        }
        let bounds = self.keyboard_key_tip.media_bounds.clone();
        let popup = div()
            .id("keyboard-media-popup")
            .test_support()
            .occlude()
            .opacity(opacity)
            .relative()
            .w(surface::css(234.))
            .h(surface::css(143.))
            .p(surface::css(10.))
            .rounded(surface::css(5.))
            .bg(Colors::background())
            .shadow(vec![BoxShadow {
                color: Colors::shadow(),
                inset: false,
                offset: point(px(0.), px(0.)),
                blur_radius: px(20.),
                spread_radius: px(0.),
            }])
            .on_prepaint(move |rect, _, _| bounds.set(rect))
            .child(
                img(if self.device().edition_id == 130 {
                    "synapse/keyboard653-current/roller-white-3x.7863b66d.png"
                } else {
                    "synapse/keyboard653-current/roller-3x.7fe981b9.png"
                })
                .absolute()
                .left(surface::css(10.))
                .top(surface::css(9.))
                .w(surface::css(91.))
                .h(surface::css(27.)),
            )
            .child(
                div()
                    .absolute()
                    .left(surface::css(8.4))
                    .top(surface::css(11.4))
                    .w(surface::css(93.))
                    .h(surface::css(23.))
                    .border(surface::css(2.))
                    .border_color(if self.hypershift {
                        Colors::hyper()
                    } else {
                        Colors::accent()
                    }),
            )
            .child(rows)
            .into_any_element();
        Some(
            razer_assets::deferred_resource(MediaPosition {
                anchor,
                viewport: self.customize_drawer.body_scroll(),
                drawer: self.customize_drawer.open,
                child: popup,
                layout: None,
            })
            .with_priority(102)
            .into_any_element(),
        )
    }
}
struct MediaPosition {
    anchor: Rc<Cell<Bounds<Pixels>>>,
    viewport: ScrollHandle,
    drawer: bool,
    child: AnyElement,
    layout: Option<LayoutId>,
}
impl IntoElement for MediaPosition {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for MediaPosition {
    type RequestLayoutState = ();
    type PrepaintState = bool;
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
    ) -> (LayoutId, ()) {
        let layout = self.child.request_layout(window, cx);
        self.layout = Some(layout);
        (
            window.request_layout(
                Style {
                    position: Position::Absolute,
                    display: Display::Flex,
                    ..Style::default()
                },
                [layout],
                cx,
            ),
            (),
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        let anchor = self.anchor.get();
        if anchor.size.width <= px(0.) {
            return false;
        }
        let Some(layout) = self.layout else {
            return false;
        };
        let child = window.layout_bounds(layout);
        let css = |value| surface::css(value).to_pixels(window.rem_size());
        let origin = point(
            anchor.left() - css(10.),
            if self.drawer {
                anchor.top() - css(20.)
            } else {
                self.viewport.bounds().top() + css(95.)
            },
        );
        window.with_element_offset(origin - child.origin, |window| {
            self.child.prepaint(window, cx)
        });
        true
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        painted: &mut bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        if *painted {
            self.child.paint(window, cx);
        }
    }
}
