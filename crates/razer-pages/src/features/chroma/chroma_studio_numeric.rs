//! Current 768/2245 number + range composition, including Audio's fractional fields.
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum NumericField {
    RippleSpeed,
    RippleWidth,
    StarlightDensity,
    WaveSpeed,
    WaveWidth,
    WavePause,
    WaveAngle,
    WheelSpeed,
    TidalSpeed,
    AudioBoost,
    AudioDecay,
}
impl NumericField {
    fn id(self) -> &'static str {
        match self {
            Self::RippleSpeed => "ripple-speed",
            Self::RippleWidth => "ripple-width",
            Self::StarlightDensity => "starlight-density",
            Self::WaveSpeed => "wave-speed",
            Self::WaveWidth => "wave-width",
            Self::WavePause => "wave-pause",
            Self::WaveAngle => "wave-angle",
            Self::WheelSpeed => "wheel-speed",
            Self::TidalSpeed => "tidal-speed",
            Self::AudioBoost => "audio-boost",
            Self::AudioDecay => "audio-decay",
        }
    }
    pub(super) fn effect(self) -> &'static str {
        match self {
            Self::StarlightDensity => "starlight",
            Self::WaveSpeed | Self::WaveWidth | Self::WavePause | Self::WaveAngle => "wave",
            Self::WheelSpeed => "wheel",
            Self::TidalSpeed => "tidal",
            Self::AudioBoost | Self::AudioDecay => "audio",
            _ => "ripple",
        }
    }
    pub(super) fn field(self) -> &'static str {
        match self {
            Self::RippleSpeed => "speed",
            Self::RippleWidth => "width",
            Self::StarlightDensity => "density",
            Self::WaveSpeed => "speed",
            Self::WaveWidth => "width",
            Self::WavePause => "pause",
            Self::WaveAngle => "angle",
            Self::WheelSpeed | Self::TidalSpeed => "speed",
            Self::AudioBoost => "boost",
            Self::AudioDecay => "decay",
        }
    }
    fn title(self) -> String {
        match self {
            Self::RippleSpeed => label("SPEED"),
            Self::RippleWidth => format!("{} (%)", label("WIDTH_PERCENT")),
            Self::StarlightDensity => label("DENSITY"),
            Self::WaveSpeed => label("SPEED"),
            Self::WaveWidth => format!("{} (%)", label("WIDTH_PERCENT")),
            Self::WavePause => label("TEXT_PAUSE_SEC"),
            Self::WaveAngle => label("TEXT_ANGLE"),
            Self::WheelSpeed => label("SPEED"),
            Self::TidalSpeed => label("SPEED"),
            Self::AudioBoost => label("TEXT_BOOST"),
            Self::AudioDecay => label("TEXT_DECAY"),
        }
    }
    fn limits(self) -> (f64, f64, f64) {
        // Bounds come from each current effect root's JSX props; the shared
        // numeric component owns clamping and step normalization.
        match self {
            Self::RippleSpeed => (1., 50., 1.),
            Self::RippleWidth => (100., 400., 100.),
            Self::StarlightDensity => (1., 10., 1.),
            Self::WaveSpeed => (0., 50., 1.),
            Self::WaveWidth => (10., 400., 1.),
            Self::WavePause => (0., 60., 1.),
            Self::WaveAngle => (0., 359., 1.),
            Self::WheelSpeed => (0., 360., 1.),
            Self::TidalSpeed => (0., 50., 1.),
            Self::AudioBoost => (0.25, 4., 0.25),
            Self::AudioDecay => (0.1, 2., 0.1),
        }
    }
    fn normalize(self, value: f64) -> f64 {
        let (min, max, step) = self.limits();
        // 7660:x5 clamps, rounds to a step from zero, then applies
        // Number(value.toFixed(2)). The registered steps have at most 2 decimals.
        let value = (value.clamp(min, max) / step).round() * step;
        (value * 100.).round() / 100.
    }
    fn blocks_character(self, character: &str) -> bool {
        matches!(character, "+" | "-" | "e") || (character == "." && self.limits().2 >= 1.)
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
pub(super) struct NumericToggleChanged {
    pub(super) field: NumericField,
    pub(super) checked: bool,
    pub(super) revision: u64,
}
impl EventEmitter<NumericToggleChanged> for StudioNumeric {}

pub(super) struct StudioNumeric {
    field: NumericField,
    value: f64,
    enabled: bool,
    revision: u64,
    companion: Option<Entity<StudioNumeric>>,
    toggle_checked: bool,
    toggle_enabled: bool,
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
        let value = field.limits().0;
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
                let next = slider.read(cx).value().start();
                // Compare at SliderState's precision. Comparing f32's 0.1
                // after promotion to f64 against 0.1f64 would keep scheduling
                // disabled resynchronization and publish spurious edits.
                if next == this.field.normalize(this.value) as f32 {
                    return;
                }
                if this.enabled {
                    this.value = this.field.normalize(f64::from(next));
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
                        this.value = this.field.normalize(f64::from(value.start()));
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
            companion: None,
            toggle_checked: false,
            toggle_enabled: false,
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
    pub(super) fn set_companion(&mut self, companion: Entity<StudioNumeric>) {
        self.companion = Some(companion);
    }
    pub(super) fn configure_toggle(
        &mut self,
        checked: bool,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        self.toggle_checked = checked;
        self.toggle_enabled = enabled;
        cx.notify();
    }
    fn toggle_changed(&self, checked: bool, revision: u64, cx: &mut Context<Self>) {
        if self.toggle_enabled && self.revision == revision {
            cx.emit(NumericToggleChanged {
                field: self.field,
                checked,
                revision,
            });
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
        .clamp(min, max);
        self.value = next;
        self.commit(window, cx);
    }
    fn press(&mut self, action: StepAction, window: &mut Window, cx: &mut Context<Self>) {
        let (min, max, _) = self.field.limits();
        if !self.enabled
            || (action == StepAction::Increment && self.value == max)
            || (action == StepAction::Decrement && self.value == min)
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
                    this.value > min && this.value < max
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
                self.value == max
            } else {
                self.value == min
            };
        BaseButton::new(if action == StepAction::Increment {
            format!("studio-{}-increase", self.field.id())
        } else {
            format!("studio-{}-decrease", self.field.id())
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let increment = self.spinner(StepAction::Increment, cx);
        let decrement = self.spinner(StepAction::Decrement, cx);
        let owner = cx.weak_entity();
        let (min, max, _) = self.field.limits();
        let title = self.field.title();
        let releasing = cx.weak_entity();
        let held = self.held;
        let action = self.held_action;
        let revision = self.revision;
        let toggle_owner = cx.weak_entity();
        let trailing = match self.field {
            NumericField::AudioBoost => Some(
                super::super::studio_checkbox::checkbox(
                    "studio-audio-auto-boost",
                    self.toggle_checked,
                    self.toggle_enabled,
                    label("TEXT_AUTO"),
                    window,
                    cx,
                )
                .on_change(move |state, _, _, cx| {
                    let _ = toggle_owner.update(cx, |this, cx| {
                        this.toggle_changed(
                            state == gpui_kit::base::CheckboxState::Checked,
                            revision,
                            cx,
                        )
                    });
                })
                .into_any_element(),
            ),
            NumericField::WaveWidth => {
                let left = gpui_kit::base::motion::transition(
                    "studio-wave-split-left",
                    if self.toggle_checked { 15_f32 } else { 1_f32 },
                    gpui_kit::base::motion::Transition::new(Duration::from_millis(300))
                        .easing(gpui_kit::base::motion::Easing::Ease),
                    window,
                    cx,
                );
                let color = surface::fade_color(
                    "studio-wave-split-color",
                    if self.toggle_checked {
                        Colors::selected()
                    } else {
                        Colors::helper()
                    },
                    300,
                    window,
                    cx,
                );
                Some(
                    div()
                        .flex()
                        .flex_col()
                        .items_end()
                        .child(div().mb(surface::css(6.)).child(label("TEXT_SPLIT")))
                        .child(
                            gpui_kit::base::Switch::new("studio-wave-split")
                                .accessibility_label(label("TEXT_SPLIT"))
                                .checked(self.toggle_checked)
                                .disabled(!self.toggle_enabled)
                                .relative()
                                .w(surface::css(32.))
                                .h(surface::css(18.))
                                .border_1()
                                .border_color(Colors::gradient_border())
                                .rounded(surface::css(9.))
                                .bg(color)
                                .focus_visible(|style| style.border_color(Colors::selected()))
                                .child(
                                    div()
                                        .absolute()
                                        .left(surface::css(left))
                                        .top(surface::css(1.))
                                        .size(surface::css(14.))
                                        .rounded_full()
                                        .bg(Colors::black()),
                                )
                                .on_change(move |checked, _, _, cx| {
                                    let _ = toggle_owner.update(cx, |this, cx| {
                                        this.toggle_changed(checked, revision, cx)
                                    });
                                }),
                        )
                        .into_any_element(),
                )
            }
            _ => None,
        };
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
            .child(
                div()
                    .id(format!("studio-{}-input-group", self.field.id()))
                    .test_support()
                    .flex()
                    .items_end()
                    .when(
                        !matches!(
                            self.field,
                            NumericField::WavePause | NumericField::WaveAngle
                        ),
                        |view| view.mb(surface::css(10.)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .when(self.field != NumericField::WaveAngle, |view| {
                                view.child(div().mb(surface::css(6.)).child(title.clone()))
                            })
                            .child(
                                div()
                                    .id(format!("studio-{}-input", self.field.id()))
                                    .test_support()
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
                                    .capture_key_down(cx.listener(
                                        |this, event: &KeyDownEvent, window, cx| {
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
                                                _ if event
                                                    .keystroke
                                                    .key_char
                                                    .as_deref()
                                                    .is_some_and(|character| {
                                                        this.field.blocks_character(character)
                                                    }) =>
                                                {
                                                    window.prevent_default()
                                                }
                                                _ => return,
                                            }
                                            cx.stop_propagation();
                                        },
                                    ))
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
                            ),
                    )
                    .children(self.companion.clone())
                    .when_some(trailing, |view, trailing| {
                        view.child(
                            div()
                                .when(self.field == NumericField::AudioBoost, |view| {
                                    view.ml(surface::css(20.))
                                })
                                .child(trailing),
                        )
                    }),
            )
            .when(
                !matches!(
                    self.field,
                    NumericField::WavePause | NumericField::WaveAngle
                ),
                |view| {
                    view.child(
                        super::super::studio_slider::StudioSlider::new(&self.slider, self.enabled)
                            .label(title),
                    )
                },
            )
            .when(
                !matches!(
                    self.field,
                    NumericField::WavePause | NumericField::WaveAngle
                ),
                |view| {
                    view.child(
                        div()
                            .flex()
                            .justify_between()
                            .mt(surface::css(3.))
                            .child(min.to_string())
                            .child(max.to_string()),
                    )
                },
            )
    }
}
