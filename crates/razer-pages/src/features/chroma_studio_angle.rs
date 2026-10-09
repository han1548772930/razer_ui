//! Current 1591 x/p angle dial. The inspector owns values; this entity owns
//! only pointer state and composes the existing 2245 numeric editor.
use super::super::{Colors, label};
use super::StudioNumeric;
use gpui_kit::base::motion::{self, Easing, Interpolate, Transition};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use razer_widgets::surface;
use std::{cell::Cell, rc::Rc, time::Duration};

const DIAMETER: f64 = 80.;
const BORDER: f64 = 2.;
const THUMB: f64 = 16.;

fn angle_at(bounds: Bounds<Pixels>, position: Point<Pixels>, scale: f64) -> f64 {
    let x = f64::from(f32::from(position.x - bounds.left())) / scale
        - f64::from(f32::from(bounds.size.width)) / scale / 2.
        + 2.;
    let y = f64::from(f32::from(position.y - bounds.top())) / scale
        - f64::from(f32::from(bounds.size.height)) / scale / 2.
        + 2.;
    // 1981:c floors before 1591:x rotates the horizontal origin to the top.
    let degrees = (y.atan2(x) / std::f64::consts::PI * 180.).floor();
    (degrees + 90.).rem_euclid(360.)
}

fn thumb_at(outer_width: f64, value: f64) -> (f64, f64) {
    let half_client = (outer_width - BORDER * 2.) / 2.;
    let radius = half_client - 10.;
    let radians = value * std::f64::consts::PI / 180.;
    // JS Math.round resolves negative halves toward +infinity. The source
    // offsets are relative to the padding box, inside the two-pixel border.
    let round = |value: f64| (value + 0.5).floor();
    (
        BORDER + round(radius * radians.sin()) + half_client - THUMB / 2.,
        BORDER + round(radius * -radians.cos()) + half_client - THUMB / 2.,
    )
}

#[derive(Clone, PartialEq)]
struct ThumbColor(Rgba);
impl Interpolate for ThumbColor {
    fn interpolate(&self, target: &Self, progress: f32) -> Self {
        let mix = |a: f32, b: f32| a + (b - a) * progress;
        Self(Rgba {
            r: mix(self.0.r, target.0.r),
            g: mix(self.0.g, target.0.g),
            b: mix(self.0.b, target.0.b),
            a: mix(self.0.a, target.0.a),
        })
    }
}

pub(super) struct AngleChanged {
    pub(super) value: f64,
    pub(super) after_change: bool,
    pub(super) revision: u64,
}
impl EventEmitter<AngleChanged> for StudioAngle {}

pub(super) struct StudioAngle {
    numeric: Entity<StudioNumeric>,
    value: f64,
    enabled: bool,
    revision: u64,
    gesture: u64,
    active: bool,
    dragging: bool,
    thumb_hovered: bool,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    focus: FocusHandle,
    _activation: Subscription,
}
impl StudioAngle {
    pub(super) fn new(
        numeric: Entity<StudioNumeric>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let activation = cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() {
                this.cancel_pointer();
                cx.notify();
            }
        });
        Self {
            numeric,
            value: 0.,
            enabled: false,
            revision: 0,
            gesture: 0,
            active: false,
            dragging: false,
            thumb_hovered: false,
            bounds: Rc::new(Cell::new(Bounds::default())),
            focus: cx.focus_handle().tab_stop(true),
            _activation: activation,
        }
    }

    pub(super) fn configure(
        &mut self,
        value: f64,
        enabled: bool,
        revision: u64,
        cx: &mut Context<Self>,
    ) {
        if revision != self.revision || !enabled {
            self.cancel_pointer();
        }
        self.value = value;
        self.enabled = enabled;
        self.revision = revision;
        cx.notify();
    }

    pub(super) fn edit_from_numeric(&mut self, value: f64, revision: u64, cx: &mut Context<Self>) {
        if !self.enabled || self.revision != revision {
            return;
        }
        // A separate editor has taken ownership, even when its value is equal.
        // Controlled echoes from this dial continue through configure instead.
        self.cancel_pointer();
        self.value = value;
        cx.notify();
    }

    fn cancel_pointer(&mut self) {
        self.gesture = self.gesture.wrapping_add(1);
        self.active = false;
        self.dragging = false;
        self.thumb_hovered = false;
    }

    fn emit(&self, after_change: bool, cx: &mut Context<Self>) {
        cx.emit(AngleChanged {
            value: self.value,
            after_change,
            revision: self.revision,
        });
        cx.notify();
    }

    fn pick(&mut self, position: Point<Pixels>, window: &Window, cx: &mut Context<Self>) {
        let bounds = self.bounds.get();
        let scale = f64::from(f32::from(window.rem_size())) / 16.;
        if bounds.size.width <= px(0.) || bounds.size.height <= px(0.) || scale <= 0. {
            return;
        }
        self.value = angle_at(bounds, position, scale);
        self.emit(false, cx);
    }

    fn press(
        &mut self,
        event: &MouseDownEvent,
        revision: u64,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        if !self.enabled || revision != self.revision {
            return;
        }
        self.gesture = self.gesture.wrapping_add(1);
        self.active = true;
        self.dragging = !matches!(event.button, MouseButton::Middle | MouseButton::Right);
        // x.v calls p for every mousedown, including middle/right, before it
        // excludes only which=2/3 from the move/up pair; navigation buttons
        // also qualify. A middle/right-only gesture has no afterChange.
        self.pick(event.position, window, cx);
    }

    fn release(&mut self, revision: u64, cx: &mut Context<Self>) {
        if !self.enabled || revision != self.revision || !self.dragging {
            return;
        }
        self.active = false;
        self.dragging = false;
        self.gesture = self.gesture.wrapping_add(1);
        self.emit(true, cx);
    }

    fn hover_thumb(&mut self, position: Point<Pixels>, window: &Window, cx: &mut Context<Self>) {
        let scale = f64::from(f32::from(window.rem_size())) / 16.;
        let bounds = self.bounds.get();
        let (left, top) = thumb_at(f64::from(f32::from(bounds.size.width)) / scale, self.value);
        let x = f64::from(f32::from(position.x - bounds.left())) / scale;
        let y = f64::from(f32::from(position.y - bounds.top())) / scale;
        let hovered =
            self.enabled && (left..=left + THUMB).contains(&x) && (top..=top + THUMB).contains(&y);
        if hovered != self.thumb_hovered {
            self.thumb_hovered = hovered;
            cx.notify();
        }
    }
}

impl Render for StudioAngle {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let revision = self.revision;
        let gesture = self.gesture;
        let dragging = self.dragging;
        let enabled = self.enabled;
        let value = self.value;
        let scale = f32::from(window.rem_size()) / 16.;
        let bounds = self.bounds.clone();
        let owner = cx.weak_entity();
        let target = if self.active {
            Colors::canvas()
        } else if self.thumb_hovered {
            Colors::input_border()
        } else {
            Colors::selected()
        };
        let background = motion::transition(
            "studio-wave-angle-thumb-background",
            ThumbColor(target.into()),
            Transition::new(Duration::from_millis(100)).easing(Easing::EaseInOut),
            window,
            cx,
        )
        .0;
        let mut dial = div()
            .id("studio-wave-angle-dial")
            .test_support()
            .when(enabled, |dial| dial.track_focus(&self.focus))
            .role(Role::Slider)
            .aria_label("Angle Selector")
            .aria_value(value.to_string())
            .aria_numeric_value(value)
            .aria_min_numeric_value(0.)
            .aria_max_numeric_value(359.)
            .aria_numeric_value_step(1.)
            .size(surface::css(DIAMETER as f32))
            .flex_shrink_0()
            .rounded_full()
            .focus_visible(|style| {
                style.shadow(vec![BoxShadow {
                    inset: false,
                    color: Colors::selected(),
                    offset: point(px(0.), px(0.)),
                    blur_radius: px(0.),
                    spread_radius: px(scale),
                }])
            })
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                this.hover_thumb(event.position, window, cx);
            }))
            .on_hover(cx.listener(|this, hovered, _, cx| {
                if !hovered && this.thumb_hovered {
                    this.thumb_hovered = false;
                    cx.notify();
                }
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if !this.enabled || this.revision != revision {
                    return;
                }
                let value = match event.keystroke.key.as_str() {
                    "left" | "down" => (this.value - 1.).clamp(0., 359.),
                    "right" | "up" => (this.value + 1.).clamp(0., 359.),
                    "home" => 0.,
                    "end" => 359.,
                    _ => return,
                };
                this.cancel_pointer();
                this.value = value;
                this.emit(true, cx);
                window.prevent_default();
                cx.stop_propagation();
            }))
            .on_a11y_action(AccessibleAction::Increment, {
                let owner = cx.weak_entity();
                move |_, _, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        if this.enabled && this.revision == revision {
                            this.cancel_pointer();
                            this.value = (this.value + 1.).clamp(0., 359.);
                            this.emit(true, cx);
                        }
                    });
                }
            })
            .on_a11y_action(AccessibleAction::Decrement, {
                let owner = cx.weak_entity();
                move |_, _, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        if this.enabled && this.revision == revision {
                            this.cancel_pointer();
                            this.value = (this.value - 1.).clamp(0., 359.);
                            this.emit(true, cx);
                        }
                    });
                }
            })
            .child(
                canvas(
                    move |rect, _, _| bounds.set(rect),
                    move |rect, _, window, _| {
                        // This canvas owns the complete border box. CSS clientWidth
                        // excludes the border; rounding occurs in source CSS pixels.
                        let border = px(BORDER as f32 * scale);
                        window.paint_quad(quad(
                            rect,
                            rect.size.width / 2.,
                            transparent_black(),
                            border,
                            Colors::selected().opacity(77. / 255.),
                            BorderStyle::Solid,
                        ));
                        let center = Bounds::centered_at(
                            rect.center(),
                            size(px(4. * scale), px(4. * scale)),
                        );
                        window.paint_quad(quad(
                            center,
                            px(2. * scale),
                            Colors::selected().opacity(0.3),
                            px(0.),
                            transparent_black(),
                            BorderStyle::Solid,
                        ));
                        let (left, top) =
                            thumb_at(f64::from(f32::from(rect.size.width) / scale), value);
                        let thumb = Bounds::new(
                            rect.origin + point(px(left as f32 * scale), px(top as f32 * scale)),
                            size(px(THUMB as f32 * scale), px(THUMB as f32 * scale)),
                        );
                        window.paint_quad(quad(
                            thumb,
                            px(8. * scale),
                            background,
                            px(scale),
                            Colors::selected(),
                            BorderStyle::Solid,
                        ));
                        if !dragging {
                            return;
                        }
                        let moving = owner.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                            if phase == DispatchPhase::Bubble {
                                let _ = moving.update(cx, |this, cx| {
                                    if this.enabled
                                        && this.dragging
                                        && this.revision == revision
                                        && this.gesture == gesture
                                    {
                                        this.pick(event.position, window, cx);
                                    }
                                });
                            }
                        });
                        let releasing = owner.clone();
                        window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                            if phase == DispatchPhase::Bubble {
                                let _ = releasing.update(cx, |this, cx| {
                                    if this.gesture == gesture {
                                        this.release(revision, cx);
                                    }
                                });
                            }
                        });
                    },
                )
                .size_full(),
            );
        for button in MouseButton::all() {
            dial = dial
                .on_mouse_down(
                    button,
                    cx.listener(move |this, event, window, cx| {
                        this.press(event, revision, window, cx)
                    }),
                )
                // A release can arrive before the first dragging frame installs
                // its window listeners. These local fallbacks use current state.
                .on_mouse_up(
                    button,
                    cx.listener(move |this, _, _, cx| this.release(revision, cx)),
                )
                .on_mouse_up_out(
                    button,
                    cx.listener(move |this, _, _, cx| this.release(revision, cx)),
                );
        }
        div()
            .id("studio-wave-angle")
            .test_support()
            .flex()
            .flex_col()
            .child(div().mb(surface::css(6.)).child(label("TEXT_ANGLE")))
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(dial)
                    .child(div().ml(surface::css(5.)).child(self.numeric.clone())),
            )
    }
}

#[cfg(test)]
mod current_angle_tests {
    use super::super::{ChromaStudio, NumericChanged, NumericField, StudioProperties, source};
    use super::{AngleChanged, StudioAngle, angle_at, thumb_at};
    use gpui_kit::component::Root;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{
        App, AppContext, Bounds, Entity, InputEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
        MouseUpEvent, NavigationDirection, Pixels, Point, Subscription, TestAppContext, Window,
        WindowHandle, point, px, size,
    };
    use std::{cell::RefCell, rc::Rc};

    #[test]
    fn source_pointer_origin_floor_and_scaled_geometry() {
        let bounds = Bounds::new(point(px(20.), px(40.)), size(px(80.), px(80.)));
        for (x, y, expected) in [
            (58., 48., 0.),
            (88., 78., 90.),
            (58., 108., 180.),
            (28., 78., 270.),
        ] {
            assert_eq!(angle_at(bounds, point(px(x), px(y)), 1.), expected);
        }
        // At the ordinary center line the source's -2px origin matters.
        assert_eq!(angle_at(bounds, point(px(60.), px(50.)), 1.), 4.);
        let radians = 32.7_f64.to_radians();
        let position = point(
            px(58. + (30. * radians.sin()) as f32),
            px(78. - (30. * radians.cos()) as f32),
        );
        assert_eq!(angle_at(bounds, position, 1.), 32.);
        let scaled = Bounds::new(point(px(20.), px(40.)), size(px(120.), px(120.)));
        assert_eq!(angle_at(scaled, point(px(122.), px(97.)), 1.5), 90.);
    }

    #[test]
    fn source_thumb_uses_client_box_and_javascript_half_rounding() {
        assert_eq!(thumb_at(80., 0.), (32., 4.));
        assert_eq!(thumb_at(80., 90.), (60., 32.));
        assert_eq!(thumb_at(80., 180.), (32., 60.));
        assert_eq!(thumb_at(80., 270.), (4., 32.));
        // At a resized odd width the radius is a half pixel; Rust round()
        // would put the left-side thumb one pixel too far left.
        assert_eq!(thumb_at(81., 270.), (4.5, 32.5));
        assert_eq!(thumb_at(81., 90.), (61.5, 32.5));
    }

    struct Harness {
        handle: WindowHandle<Root>,
        props: Entity<StudioProperties>,
        angle: Entity<StudioAngle>,
        events: Rc<RefCell<Vec<(f64, bool)>>>,
        _subscription: Subscription,
    }

    fn wave(cx: &mut TestAppContext) -> Harness {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_reduce_motion(true);
        });
        let mut props = None;
        let handle = cx.open_window(size(px(360.), px(1000.)), |window, cx| {
            let studio = cx.new(ChromaStudio::new);
            let editor = studio.read(cx).properties.clone();
            editor.update(cx, |editor, cx| {
                let defaults = source()
                    .effects
                    .iter()
                    .find(|item| item.name == "wave")
                    .unwrap();
                editor.current = Some((1, "wave".to_owned()));
                editor.tool = "pen".into();
                editor.params = defaults.paint_params.clone();
                editor.params["angle"] = 0.into();
                editor.params2 = serde_json::json!({});
                editor.layer_params = defaults.params.clone();
                editor.paint_params = defaults.paint_params.clone();
                editor.attach(window, cx);
            });
            props = Some(editor.clone());
            Root::new(editor, window, cx)
        });
        let props = props.unwrap();
        let events = Rc::new(RefCell::new(Vec::new()));
        let (angle, subscription) = cx.update(|cx| {
            let angle = props.read(cx).angle.clone().unwrap();
            let history = events.clone();
            let subscription = cx.subscribe(&angle, move |_, event: &AngleChanged, _| {
                history.borrow_mut().push((event.value, event.after_change));
            });
            (angle, subscription)
        });
        Harness {
            handle,
            props,
            angle,
            events,
            _subscription: subscription,
        }
    }

    fn down(window: &mut Window, position: Point<Pixels>, button: MouseButton, cx: &mut App) {
        window.dispatch_event(
            MouseDownEvent {
                button,
                position,
                modifiers: Default::default(),
                click_count: 1,
                first_mouse: false,
            }
            .to_platform_input(),
            cx,
        );
    }
    fn moving(window: &mut Window, position: Point<Pixels>, button: MouseButton, cx: &mut App) {
        window.dispatch_event(
            MouseMoveEvent {
                position,
                pressed_button: Some(button),
                modifiers: Default::default(),
            }
            .to_platform_input(),
            cx,
        );
    }
    fn up(window: &mut Window, position: Point<Pixels>, button: MouseButton, cx: &mut App) {
        window.dispatch_event(
            MouseUpEvent {
                button,
                position,
                modifiers: Default::default(),
                click_count: 1,
            }
            .to_platform_input(),
            cx,
        );
    }

    #[gpui_kit::test]
    fn mounted_dial_links_numeric_and_captures_window_pointer_phases(cx: &mut TestAppContext) {
        let h = wave(cx);
        cx.update_window(h.handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let bounds = window.find("studio-wave-angle-dial").bounds();
            let input = window.find("studio-wave-angle-input").bounds();
            let scale = f32::from(window.rem_size()) / 16.;
            assert_eq!(bounds.size, size(px(80. * scale), px(80. * scale)));
            assert_eq!(input.left() - bounds.right(), px(5. * scale));
            assert_eq!(input.center().y, bounds.center().y);
            let origin = bounds.center() - point(px(2. * scale), px(2. * scale));
            let right = origin + point(px(30. * scale), px(0.));
            let outside = origin + point(px(0.), px(120. * scale));
            down(window, right, MouseButton::Left, cx);
            window.render_frame(cx);
            assert_eq!(h.props.read(cx).params["angle"], 90.);
            assert!(h.angle.read(cx).dragging);
            moving(window, outside, MouseButton::Left, cx);
            window.render_frame(cx);
            assert_eq!(h.props.read(cx).params["angle"], 180.);
            up(window, outside, MouseButton::Left, cx);
            window.render_frame(cx);
            assert!(!h.angle.read(cx).dragging);
            assert_eq!(
                h.events.borrow().as_slice(),
                &[(90., false), (180., false), (180., true)]
            );
            moving(window, right, MouseButton::Left, cx);
            window.render_frame(cx);
            assert_eq!(h.props.read(cx).params["angle"], 180.);

            // A native numeric edit feeds the same retained dial, with no dial event.
            window.click_at(
                "studio-wave-angle-input",
                point(px(8. * scale), px(13. * scale)),
                cx,
            );
            window.input("123", cx);
            window.press("enter", cx);
            assert_eq!(h.props.read(cx).params["angle"], 123.);
            assert_eq!(h.angle.read(cx).value, 123.);
            assert_eq!(h.events.borrow().len(), 3);

            // Release without an intervening render must also finish the drag.
            down(window, right, MouseButton::Left, cx);
            up(window, outside, MouseButton::Left, cx);
            window.render_frame(cx);
            assert!(!h.angle.read(cx).dragging);
            assert_eq!(h.events.borrow().last(), Some(&(90., true)));
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn buttons_and_external_edits_do_not_leave_a_stale_capture(cx: &mut TestAppContext) {
        let h = wave(cx);
        cx.update_window(h.handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let bounds = window.find("studio-wave-angle-dial").bounds();
            let scale = f32::from(window.rem_size()) / 16.;
            let origin = bounds.center() - point(px(2. * scale), px(2. * scale));
            let right = origin + point(px(30. * scale), px(0.));
            let outside = origin + point(px(0.), px(120. * scale));
            for button in [MouseButton::Middle, MouseButton::Right] {
                h.events.borrow_mut().clear();
                down(window, right, button, cx);
                window.render_frame(cx);
                moving(window, outside, button, cx);
                up(window, outside, button, cx);
                window.render_frame(cx);
                assert_eq!(h.events.borrow().as_slice(), &[(90., false)]);
                assert!(!h.angle.read(cx).dragging);
                assert!(h.angle.read(cx).active);
            }
            for button in [
                MouseButton::Navigate(NavigationDirection::Back),
                MouseButton::Navigate(NavigationDirection::Forward),
            ] {
                h.events.borrow_mut().clear();
                down(window, right, button, cx);
                window.render_frame(cx);
                moving(window, outside, button, cx);
                window.render_frame(cx);
                up(window, outside, button, cx);
                window.render_frame(cx);
                assert_eq!(
                    h.events.borrow().as_slice(),
                    &[(90., false), (180., false), (180., true)]
                );
            }
            down(window, right, MouseButton::Left, cx);
            window.render_frame(cx);
            let numeric = h.props.read(cx).numeric.as_ref().unwrap()[6].clone();
            let revision = h.props.read(cx).control_revision;
            // A same-value edit still takes ownership away from the old dial.
            numeric.update(cx, |_, cx| {
                cx.emit(NumericChanged {
                    field: NumericField::WaveAngle,
                    value: 90.,
                    revision,
                })
            });
            window.render_frame(cx);
            assert!(!h.angle.read(cx).dragging);
            h.events.borrow_mut().clear();
            moving(window, outside, MouseButton::Left, cx);
            up(window, outside, MouseButton::Left, cx);
            window.render_frame(cx);
            assert_eq!(h.props.read(cx).params["angle"], 90.);
            assert!(h.events.borrow().is_empty());

            down(window, right, MouseButton::Left, cx);
            window.render_frame(cx);
            h.props.update(cx, |props, cx| props.reset(cx));
            let reset = h.props.read(cx).params["angle"].clone();
            moving(window, outside, MouseButton::Left, cx);
            up(window, outside, MouseButton::Left, cx);
            window.render_frame(cx);
            assert_eq!(h.props.read(cx).params["angle"], reset);
            assert!(!h.angle.read(cx).dragging);
            h.props.update(cx, |props, cx| {
                props.tool = "select".into();
                props.refresh_numeric(cx);
            });
            window.render_frame(cx);
            down(window, right, MouseButton::Left, cx);
            up(window, right, MouseButton::Left, cx);
            window.render_frame(cx);
            assert_eq!(h.props.read(cx).params["angle"], reset);
            assert!(!h.angle.read(cx).active);
        })
        .unwrap();
    }
}
