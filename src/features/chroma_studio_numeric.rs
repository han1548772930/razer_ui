//! Current 768/2245 integer number + range composition for 5305/6548.
//! Change updates only working paint parameters; no device preview is invented.
use super::*;
use gpui_kit::base::{NumberInput, StepAction};
use gpui_kit::component::{input::MaskPattern, slider::SliderEvent};
use std::time::Duration;

/// The source is an HTML number input, not an integer parser. Keyboard filtering
/// does not prohibit a pasted fractional/exponent value. Invalid number text is
/// exposed by that element as an empty value, which Number("") converts to zero.
fn html_number(raw: &str) -> f64 {
    let unsigned = raw.strip_prefix('-').unwrap_or(raw);
    let mut parts = unsigned.split(['e', 'E']);
    let mantissa = parts.next().unwrap_or("");
    let exponent = parts.next();
    if parts.next().is_some() {
        return 0.;
    }
    let digits = |value: &str| !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit());
    if let Some(exponent) = exponent {
        let exponent = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
        if !digits(exponent) {
            return 0.;
        }
    }
    let valid_mantissa = if let Some((integer, fraction)) = mantissa.split_once('.') {
        (integer.is_empty() || digits(integer)) && digits(fraction)
    } else {
        digits(mantissa)
    };
    if !valid_mantissa {
        return 0.;
    }
    raw.parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .unwrap_or(0.)
}

#[derive(Clone, Copy)]
pub(super) enum NumericField {
    RippleSpeed,
    RippleWidth,
    StarlightDensity,
}
impl NumericField {
    pub(super) fn effect(self) -> &'static str {
        match self {
            Self::StarlightDensity => "starlight",
            _ => "ripple",
        }
    }
    pub(super) fn field(self) -> &'static str {
        match self {
            Self::RippleSpeed => "speed",
            Self::RippleWidth => "width",
            Self::StarlightDensity => "density",
        }
    }
    fn title(self) -> String {
        match self {
            Self::RippleSpeed => label("SPEED"),
            Self::RippleWidth => format!("{} (%)", label("WIDTH_PERCENT")),
            Self::StarlightDensity => label("DENSITY"),
        }
    }
    fn limits(self) -> (i64, i64, i64) {
        // 5305 Mn/s8, jw/Ph/h1 and 6548 dV/JG, independently checked
        // against their JSX props by review-studio-reactive-ripple-starlight.cjs.
        match self {
            Self::RippleSpeed => (1, 50, 1),
            Self::RippleWidth => (100, 400, 100),
            Self::StarlightDensity => (1, 10, 1),
        }
    }
    fn normalize(self, value: f64) -> f64 {
        let (min, max, step) = self.limits();
        // 7660:x5 clamps first, then rounds to a multiple of step from zero.
        (value.clamp(min as f64, max as f64) / step as f64).round() * step as f64
    }
    fn slider(self, value: f64) -> SliderState {
        let (min, max, step) = self.limits();
        SliderState::new()
            .min(min as f32)
            .max(max as f32)
            .step(step as f32)
            .default_value(self.normalize(value) as f32)
    }
}

pub(super) struct NumericChanged {
    pub(super) field: NumericField,
    pub(super) value: f64,
    pub(super) revision: u64,
}
impl EventEmitter<NumericChanged> for StudioNumeric {}

pub(super) struct StudioNumeric {
    field: NumericField,
    value: f64,
    enabled: bool,
    revision: u64,
    input: Entity<InputState>,
    slider: Entity<SliderState>,
    focused: bool,
    held: bool,
    held_action: StepAction,
    repeated: bool,
    suppress_click: bool,
    repeat: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}
impl StudioNumeric {
    pub(super) fn new(field: NumericField, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let value = field.limits().0 as f64;
        let input = cx.new(|cx| InputState::new(window, cx).mask_pattern(MaskPattern::None));
        let slider = cx.new(|_| field.slider(value));
        let subscriptions = vec![
            cx.subscribe_in(&input, window, |this, _, event, window, cx| match event {
                InputEvent::Focus => {
                    this.focused = true;
                    this.input
                        .update(cx, |input, cx| input.select_all(window, cx));
                    cx.notify();
                }
                InputEvent::Change if this.enabled => this.edit(window, cx),
                InputEvent::Blur => {
                    this.focused = false;
                    if this.enabled {
                        this.commit(window, cx);
                    }
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => window.blur(cx),
                _ => {}
            }),
            cx.observe_in(&slider, window, |this, slider, window, cx| {
                let next = f64::from(slider.read(cx).value().start());
                if next == this.field.normalize(this.value) {
                    return;
                }
                if this.enabled {
                    this.value = next;
                    this.write_input(window, cx);
                    this.publish(cx);
                } else {
                    this.sync_slider(cx);
                }
            }),
            cx.subscribe_in(&slider, window, |this, _, event, window, cx| {
                if !this.enabled {
                    return;
                }
                match event {
                    SliderEvent::Change(value) | SliderEvent::Release(value) => {
                        this.value = f64::from(value.start());
                        this.write_input(window, cx);
                        this.publish(cx);
                    }
                }
            }),
            cx.observe_window_activation(window, |this, window, cx| {
                if !window.is_window_active() {
                    this.cancel_repeat();
                    cx.notify();
                }
            }),
        ];
        Self {
            field,
            value,
            enabled: false,
            revision: 0,
            input,
            slider,
            focused: false,
            held: false,
            held_action: StepAction::Increment,
            repeated: false,
            suppress_click: false,
            repeat: None,
            _subscriptions: subscriptions,
        }
    }
    pub(super) fn configure(
        &mut self,
        value: f64,
        enabled: bool,
        revision: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let changed = self.revision != revision;
        if changed || !enabled {
            self.cancel_repeat();
        }
        self.enabled = enabled;
        self.revision = revision;
        self.value = value;
        self.sync_slider(cx);
        // A reset/layer/tool transition replaces unfinished text as well.
        if changed || !self.focused {
            self.write_input(window, cx);
        }
        cx.notify();
    }
    fn cancel_repeat(&mut self) {
        self.repeat = None;
        self.held = false;
        self.repeated = false;
        self.suppress_click = false;
    }
    fn sync_slider(&self, cx: &mut Context<Self>) {
        self.slider.update(cx, |slider, cx| {
            *slider = self.field.slider(self.value);
            cx.notify();
        });
    }
    fn write_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| {
            input.set_value(self.value.to_string(), window, cx)
        });
    }
    fn publish(&self, cx: &mut Context<Self>) {
        cx.emit(NumericChanged {
            field: self.field,
            value: self.value,
            revision: self.revision,
        });
        cx.notify();
    }
    fn edit(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let raw = self.input.read(cx).value();
        // HTML number's empty value is Number("") == 0 in the effect root.
        self.value = html_number(&raw);
        self.sync_slider(cx);
        self.publish(cx);
    }
    fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.value = self.field.normalize(self.value);
        self.write_input(window, cx);
        self.sync_slider(cx);
        self.publish(cx);
    }
    fn step(&mut self, action: StepAction, window: &mut Window, cx: &mut Context<Self>) {
        if !self.enabled {
            return;
        }
        let (min, max, step) = self.field.limits();
        // HTML stepUp/stepDown snap an off-grid input in the requested direction.
        let next = if action == StepAction::Increment {
            ((self.value / step as f64).floor() + 1.) * step as f64
        } else {
            ((self.value / step as f64).ceil() - 1.) * step as f64
        }
        .clamp(min as f64, max as f64);
        self.value = next;
        self.commit(window, cx);
    }
    fn press(&mut self, action: StepAction, window: &mut Window, cx: &mut Context<Self>) {
        let (min, max, _) = self.field.limits();
        if !self.enabled
            || (action == StepAction::Increment && self.value == max as f64)
            || (action == StepAction::Decrement && self.value == min as f64)
        {
            return;
        }
        self.held = true;
        self.held_action = action;
        self.repeated = false;
        self.suppress_click = true;
        let revision = self.revision;
        self.repeat = Some(cx.spawn_in(window, async move |owner, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(300))
                .await;
            if !matches!(
                owner.update_in(cx, |this, _, _| {
                    this.repeated = true;
                    this.held && this.enabled && this.revision == revision
                }),
                Ok(true)
            ) {
                return;
            }
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let keep = owner.update_in(cx, |this, window, cx| {
                    if !this.held || !this.enabled || this.revision != revision {
                        return false;
                    }
                    this.step(action, window, cx);
                    let (min, max, _) = this.field.limits();
                    this.value > min as f64 && this.value < max as f64
                });
                if !matches!(keep, Ok(true)) {
                    break;
                }
            }
        }));
    }
    fn release(&mut self, action: StepAction, window: &mut Window, cx: &mut Context<Self>) {
        if self.held && !self.repeated {
            self.step(action, window, cx);
        }
        self.held = false;
        self.repeat = None;
        let revision = self.revision;
        let owner = cx.weak_entity();
        cx.defer(move |cx| {
            let _ = owner.update(cx, |this, _| {
                if this.revision == revision {
                    this.suppress_click = false;
                }
            });
        });
    }
    fn spinner(&self, action: StepAction, cx: &mut Context<Self>) -> BaseButton {
        let (min, max, _) = self.field.limits();
        let disabled = !self.enabled
            || if action == StepAction::Increment {
                self.value == max as f64
            } else {
                self.value == min as f64
            };
        BaseButton::new(if action == StepAction::Increment {
            "up"
        } else {
            "down"
        })
        .accessibility_label(if action == StepAction::Increment {
            "Increase Value"
        } else {
            "Decrease Value"
        })
        .disabled(disabled)
        .p_0()
        .w(surface::css(16.))
        .relative()
        .bg(transparent_black())
        .styles(|style| style.disabled(|style| style.opacity(0.3)))
        .when(!disabled, |view| {
            view.hover(|style| style.bg(Colors::spinner_hover()))
                .active(|style| style.bg(Colors::spinner_pressed()))
        })
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, window, cx| this.press(action, window, cx)),
        )
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(move |this, _, window, cx| this.release(action, window, cx)),
        )
        .on_hover(cx.listener(move |this, hovered, window, cx| {
            if !hovered && this.held {
                this.release(action, window, cx);
            }
        }))
        .child(
            svg()
                .path("synapse/chroma-studio-brightness-pointer.svg")
                .w(surface::css(8.))
                .h(surface::css(4.))
                .text_color(Colors::dropdown_arrow())
                .with_transformation(Transformation::rotate(radians(
                    if action == StepAction::Increment {
                        std::f32::consts::PI
                    } else {
                        0.
                    },
                ))),
        )
    }
}
impl Render for StudioNumeric {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let increment = self.spinner(StepAction::Increment, cx);
        let decrement = self.spinner(StepAction::Decrement, cx);
        let owner = cx.weak_entity();
        let (min, max, _) = self.field.limits();
        let title = self.field.title();
        let releasing = cx.weak_entity();
        let held = self.held;
        let action = self.held_action;
        let revision = self.revision;
        div()
            .relative()
            .flex()
            .flex_col()
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, _, window, _| {
                        if !held {
                            return;
                        }
                        let releasing = releasing.clone();
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                            if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
                                let _ = releasing.update(cx, |this, cx| {
                                    if this.revision == revision && this.held {
                                        this.release(action, window, cx);
                                    }
                                });
                            }
                        });
                    },
                )
                .absolute()
                .inset_0(),
            )
            .child(div().mb(surface::css(6.)).child(title.clone()))
            .child(
                div()
                    .id("studio-numeric-input")
                    .mb(surface::css(10.))
                    .w(surface::css(62.))
                    .h(surface::css(27.))
                    .border_1()
                    .border_color(if self.focused {
                        Colors::selected()
                    } else {
                        Colors::input_border()
                    })
                    .when(self.enabled, |view| {
                        view.hover(|style| style.border_color(Colors::selected()))
                    })
                    .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        if !this.enabled {
                            return;
                        }
                        match event.keystroke.key.as_str() {
                            "escape" => window.blur(cx),
                            "up" | "down" => {
                                this.step(
                                    if event.keystroke.key == "up" {
                                        StepAction::Increment
                                    } else {
                                        StepAction::Decrement
                                    },
                                    window,
                                    cx,
                                );
                                window.prevent_default();
                            }
                            _ if matches!(
                                event.keystroke.key_char.as_deref(),
                                Some("." | "+" | "-" | "e")
                            ) =>
                            {
                                window.prevent_default()
                            }
                            _ => return,
                        }
                        cx.stop_propagation();
                    }))
                    .child(
                        NumberInput::new(&self.input)
                            .disabled(!self.enabled)
                            .controls_right()
                            .size_full()
                            .input(
                                Input::new(&self.input)
                                    .aria_label(title.clone())
                                    .appearance(false)
                                    .bordered(false)
                                    .focus_bordered(false)
                                    .size_full()
                                    .px(surface::css(6.))
                                    .py(surface::css(2.))
                                    .text_size(surface::css(14.)),
                            )
                            .increment_button(move |_| increment)
                            .decrement_button(move |_| decrement)
                            .on_step(move |action, window, cx| {
                                let _ = owner.update(cx, |this, cx| {
                                    if !this.suppress_click {
                                        this.step(action, window, cx);
                                    }
                                });
                            }),
                    ),
            )
            .child(
                super::super::studio_slider::StudioSlider::new(&self.slider, self.enabled)
                    .label(title),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .mt(surface::css(3.))
                    .child(min.to_string())
                    .child(max.to_string()),
            )
    }
}
