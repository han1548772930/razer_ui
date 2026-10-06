//! Current Studio 1991: three-position range and frame-stepped SVG preview.
//! Millisecond values are supplied by each effect root, not inferred here.
use super::*;
use gpui_kit::component::slider::{SliderEvent, SliderState};

pub(super) struct StudioDuration {
    slider: Entity<SliderState>,
    index: usize,
    enabled: bool,
    mounted: bool,
    effect: String,
    preview_hover: bool,
    slider_hover: bool,
    dragging: bool,
    phase: f32,
    frame_pending: bool,
    window: Option<AnyWindowHandle>,
    activation: Option<Subscription>,
    _subscriptions: Vec<Subscription>,
}

pub(super) struct DurationChanged(pub(super) usize);
impl EventEmitter<DurationChanged> for StudioDuration {}

impl StudioDuration {
    pub(super) fn new(cx: &mut Context<Self>) -> Self {
        let slider = cx.new(|_| Self::state(1));
        let subscriptions = vec![
            cx.observe(&slider, |this, slider, cx| {
                let next = slider.read(cx).value().start().round() as usize;
                if next == this.index {
                    return;
                }
                if this.enabled && this.mounted {
                    this.index = next;
                    // Accessibility uses set_value without a Release event.
                    if !this.dragging {
                        cx.emit(DurationChanged(next));
                    }
                    cx.notify();
                } else {
                    slider.update(cx, |slider, cx| {
                        *slider = Self::state(this.index);
                        cx.notify();
                    });
                }
            }),
            cx.subscribe(&slider, |this, _, event, cx| {
                if let SliderEvent::Release(value) = event {
                    this.dragging = false;
                    if this.enabled && this.mounted {
                        this.index = value.start().round() as usize;
                        cx.emit(DurationChanged(this.index));
                    }
                    cx.notify();
                }
            }),
        ];
        Self {
            slider,
            index: 1,
            enabled: false,
            mounted: false,
            effect: String::new(),
            preview_hover: false,
            slider_hover: false,
            dragging: false,
            phase: 0.,
            frame_pending: false,
            window: None,
            activation: None,
            _subscriptions: subscriptions,
        }
    }

    fn state(index: usize) -> SliderState {
        SliderState::new()
            .min(0.)
            .max(2.)
            .step(1.)
            .default_value(index as f32)
    }

    pub(super) fn attach(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let handle = window.window_handle();
        if self
            .window
            .is_some_and(|old| old.window_id() == handle.window_id())
        {
            return;
        }
        self.window = Some(handle);
        self.frame_pending = false;
        self.preview_hover = false;
        self.slider_hover = false;
        self.dragging = false;
        self.activation = Some(cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() {
                this.preview_hover = false;
                this.slider_hover = false;
                this.dragging = false;
                cx.notify();
            }
        }));
    }

    pub(super) fn configure(
        &mut self,
        effect: &str,
        index: usize,
        enabled: bool,
        mounted: bool,
        cx: &mut Context<Self>,
    ) {
        if self.effect != effect || !mounted || !enabled {
            self.preview_hover = false;
            self.slider_hover = false;
            self.dragging = false;
        }
        if self.effect != effect || !mounted {
            self.phase = 0.;
        }
        self.effect = effect.into();
        self.enabled = enabled;
        self.mounted = mounted;
        // Mark the expected index before notifying the observed base state.
        self.index = index;
        self.slider.update(cx, |slider, cx| {
            *slider = Self::state(index);
            cx.notify();
        });
        cx.notify();
    }

    fn animated(&self) -> bool {
        self.enabled && self.mounted && (self.preview_hover || self.slider_hover || self.dragging)
    }
}

impl Render for StudioDuration {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.animated() && window.is_window_active() && !self.frame_pending {
            self.frame_pending = true;
            let owner = cx.weak_entity();
            let window_id = window.window_handle().window_id();
            window.on_next_frame(move |window, cx| {
                let _ = owner.update(cx, |this, cx| {
                    if !this
                        .window
                        .is_some_and(|handle| handle.window_id() == window_id)
                    {
                        return;
                    }
                    this.frame_pending = false;
                    if this.animated() && window.is_window_active() {
                        // Source adds CSS pixels per requestAnimationFrame,
                        // not pixels per second. Wrapping preserves repeat-x.
                        this.phase = (this.phase
                            + match this.index {
                                0 => 1.5,
                                2 => 2.,
                                _ => 1.75,
                            })
                            % 218.;
                        cx.notify();
                    }
                });
            });
            window.request_animation_frame();
        }
        let asset = match (self.effect.as_str(), self.index) {
            ("reactive", 1) => "wave-reactive-md-gray",
            ("reactive", 2) => "wave-reactive-sm-gray",
            (_, 0) => "wave-lg-gray",
            (_, 2) => "wave-sm-gray",
            _ => "wave-md-gray",
        };
        // Inspector width is 250px. Three 218px tiles cover its content
        // through the complete repeat phase; SVG aspect ratio is 230:80.
        let preview = div()
            .id("studio-duration-preview")
            .relative()
            .w_full()
            .h(surface::css(100.))
            .overflow_hidden()
            .on_hover(cx.listener(|this, hovered, _, cx| {
                this.preview_hover = this.enabled && *hovered;
                cx.notify();
            }))
            .children((-1..=1).map(|tile| {
                img(SharedString::from(source().assets[asset].clone()))
                    .absolute()
                    .left(surface::css(self.phase + tile as f32 * 218.))
                    .top(surface::css(13.))
                    .w(surface::css(218.))
                    .h(surface::css(218. * 80. / 230.))
            }));
        div()
            .flex()
            .flex_col()
            .child(div().mb(surface::css(6.)).child(label("TEXT_DURATION")))
            .child(preview)
            .child(
                div()
                    .id("studio-duration-range")
                    .mb(surface::css(10.))
                    .on_hover(cx.listener(|this, hovered, _, cx| {
                        this.slider_hover = this.enabled && *hovered;
                        cx.notify();
                    }))
                    .capture_any_mouse_down(cx.listener(|this, event: &MouseDownEvent, _, cx| {
                        if this.enabled && event.button == MouseButton::Left {
                            this.dragging = true;
                            cx.notify();
                        }
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.dragging = false;
                            cx.notify();
                        }),
                    )
                    .on_mouse_up_out(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.dragging = false;
                            cx.notify();
                        }),
                    )
                    .child(
                        super::studio_slider::StudioSlider::new(&self.slider, self.enabled)
                            .label(label("TEXT_DURATION")),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .mt(surface::css(3.))
                            .child(label("TEXT_SLOW"))
                            .child(label("TEXT_FAST")),
                    ),
            )
    }
}
