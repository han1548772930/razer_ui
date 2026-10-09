//! Current product numeric editor; Kiyo and automation module 44230 share behavior.
use crate::surface;
use crate::theme::CameraProductColors as Colors;
use gpui_kit::base::motion::{self, Easing, Transition};
use gpui_kit::base::{Button as BaseButton, NumberInput, StepAction};
use gpui_kit::component::input::{Input, InputEvent, InputState, MaskPattern};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::time::Duration;

const REPEAT: Duration = Duration::from_millis(300);

#[derive(Clone, Debug)]
pub struct StepperEvent {
    /// Last committed numeric value; live drafts do not normalize this value.
    pub value: f64,
    /// Source `allowLiveUpdate` sends the raw string, including `""` and `"-"`.
    /// Commit and step events carry `None`.
    pub draft: Option<String>,
}
impl EventEmitter<StepperEvent> for Stepper {}

pub struct Stepper {
    id: SharedString,
    input: Entity<InputState>,
    value: f64,
    min: f64,
    max: f64,
    step: f64,
    allow_decimal: bool,
    round_up_decimals: bool,
    modes_area: bool,
    custom_keymapping: bool,
    reveal_spinners_on_hover: bool,
    allow_live_update: bool,
    disabled: bool,
    focused: bool,
    hovered: bool,
    interacting: bool,
    draft_from_typing: bool,
    suppress_pointer_click: bool,
    task: Option<Task<()>>,
    _subscription: Subscription,
}

fn valid_draft(value: &str, decimal: bool) -> bool {
    if matches!(value, "" | "-") {
        return true;
    }
    let value = value.strip_prefix('-').unwrap_or(value);
    let mut parts = value.split('.');
    let integer = parts.next().unwrap_or_default();
    !integer.is_empty()
        && integer.bytes().all(|byte| byte.is_ascii_digit())
        && parts.next().is_none_or(|fraction| {
            decimal && fraction.len() <= 3 && fraction.bytes().all(|byte| byte.is_ascii_digit())
        })
        && parts.next().is_none()
}

impl Stepper {
    pub fn new(
        id: impl Into<SharedString>,
        value: f64,
        range: (f64, f64, f64),
        allow_decimal: bool,
        round_up_decimals: bool,
        max_length: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                // Keep the source's intermediate drafts (including `-` and `1.`).
                // NumberInput otherwise installs its own numeric mask at render.
                .mask_pattern(MaskPattern::None)
                .default_value(Self::format(value, allow_decimal, round_up_decimals))
                .validate(move |value, _| {
                    valid_draft(value, allow_decimal)
                        && max_length.is_none_or(|limit| {
                            value.len() <= limit + usize::from(value.starts_with('-'))
                        })
                })
        });
        let subscription =
            cx.subscribe_in(&input, window, |this, _, event, window, cx| match event {
                InputEvent::Focus => {
                    this.focused = true;
                    cx.notify();
                }
                InputEvent::Blur => {
                    this.focused = false;
                    this.interacting = false;
                    this.task = None;
                    this.commit(window, cx);
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => window.blur(cx),
                InputEvent::Change => {
                    this.draft_from_typing = true;
                    if this.allow_live_update && !this.disabled {
                        cx.emit(StepperEvent {
                            value: this.value,
                            draft: Some(this.input.read(cx).value().to_string()),
                        });
                    }
                    cx.notify();
                }
            });
        Self {
            id: id.into(),
            input,
            value,
            min: range.0,
            max: range.1,
            step: range.2,
            allow_decimal,
            round_up_decimals,
            modes_area: false,
            custom_keymapping: false,
            reveal_spinners_on_hover: false,
            allow_live_update: false,
            disabled: false,
            focused: false,
            hovered: false,
            interacting: false,
            draft_from_typing: false,
            suppress_pointer_click: false,
            task: None,
            _subscription: subscription,
        }
    }

    /// `.modes-area .stepper` wins over the later generic Kiyo dimensions.
    pub fn in_modes_area(mut self) -> Self {
        self.modes_area = true;
        self
    }

    /// Nommo's generic spinner CSS hides arrows until hover/focus-within.
    /// Camera and custom-keymapping callers have explicit always-visible
    /// overrides, so they must not inherit this policy from shared behavior.
    pub fn reveal_spinners_on_hover(mut self) -> Self {
        self.reveal_spinners_on_hover = true;
        self
    }

    /// Dashboard 82508's `.key-config .stepper.custom-keymapping-stepper`.
    pub fn in_custom_keymapping(mut self) -> Self {
        self.custom_keymapping = true;
        self
    }

    /// Emit accepted text drafts immediately, leaving snapping and clamping
    /// to blur/Enter or a step, as Dashboard 44230 does.
    pub fn live_update(mut self) -> Self {
        self.allow_live_update = true;
        self
    }

    fn format(value: f64, decimal: bool, round_up: bool) -> String {
        if decimal && !round_up {
            format!("{value:.3}")
        } else {
            value.to_string()
        }
    }

    fn write_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        let text = Self::format(
            self.value,
            self.allow_decimal,
            self.round_up_decimals || self.interacting,
        );
        self.input
            .update(cx, |input, cx| input.set_value(text, window, cx));
    }

    fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let mut value = self
            .input
            .read(cx)
            .value()
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .unwrap_or(0.);
        // Source integer parseInput snaps before clamping; decimal blur snaps
        // after clamping, with the source's 1e-10 floating-point tolerance.
        if !self.allow_decimal {
            value = (value / self.step).ceil() * self.step;
        }
        value = value.clamp(self.min, self.max);
        if self.allow_decimal {
            let nearest = (value / self.step).round() * self.step;
            if (value - nearest).abs() > 1e-10 {
                value = (value / self.step).ceil() * self.step;
            }
        }
        self.set_value(value, window, cx);
    }

    fn step_once(&mut self, action: StepAction, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let sign = if action == StepAction::Increment {
            1.
        } else {
            -1.
        };
        let draft = self.input.read(cx).value();
        // handleChange stores a string, while parent updates store a number.
        // Preserve the source's integer `e + stepValue` coercion as well as
        // the explicit parseFloat used by its decimal branch.
        let mut next =
            if !self.allow_decimal && self.draft_from_typing && action == StepAction::Increment {
                format!("{draft}{}", self.step).parse::<f64>().unwrap_or(0.)
            } else {
                draft.parse::<f64>().unwrap_or(f64::NAN) + sign * self.step
            };
        if !next.is_finite() {
            next = 0.;
        }
        if self.allow_decimal {
            next = (next * 1000.).round() / 1000.;
        } else {
            next = (next / self.step).ceil() * self.step;
        }
        self.set_value(next.clamp(self.min, self.max), window, cx);
    }

    fn press(&mut self, action: StepAction, window: &mut Window, cx: &mut Context<Self>) {
        self.task = None;
        self.step_once(action, window, cx);
        if self.disabled || self.at_limit(action) {
            return;
        }
        self.task = Some(cx.spawn_in(window, async move |view, cx| {
            loop {
                cx.background_executor().timer(REPEAT).await;
                let stop = view.update_in(cx, |this, window, cx| {
                    if this.disabled || this.at_limit(action) {
                        return true;
                    }
                    this.step_once(action, window, cx);
                    this.at_limit(action)
                });
                if !matches!(stop, Ok(false)) {
                    break;
                }
            }
        }));
    }

    fn at_limit(&self, action: StepAction) -> bool {
        // Source uses strict equality: an uncommitted string is not its
        // numeric bound even when the text is the same number.
        if self.draft_from_typing {
            return false;
        }
        if action == StepAction::Increment {
            self.value >= self.max
        } else {
            self.value <= self.min
        }
    }

    fn set_value(&mut self, value: f64, window: &mut Window, cx: &mut Context<Self>) {
        self.value = value;
        self.draft_from_typing = false;
        self.write_input(window, cx);
        cx.emit(StepperEvent { value, draft: None });
        cx.notify();
    }

    pub fn sync_value(
        &mut self,
        value: f64,
        min: f64,
        disabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value_changed = (self.value - value).abs() > f64::EPSILON;
        let changed = value_changed || self.min != min || self.disabled != disabled;
        self.value = value;
        self.min = min;
        self.disabled = disabled;
        if disabled {
            self.task = None;
            self.suppress_pointer_click = false;
        }
        self.input
            .update(cx, |input, cx| input.set_disabled(disabled, cx));
        if value_changed {
            self.draft_from_typing = false;
            self.write_input(window, cx);
        }
        if changed {
            cx.notify();
        }
    }

    /// A different mapping owns a fresh input draft, even when its committed
    /// number equals the previous mapping's value. Ordinary sync preserves it.
    pub fn reset_value(&mut self, value: f64, window: &mut Window, cx: &mut Context<Self>) {
        self.value = value.clamp(self.min, self.max);
        self.draft_from_typing = false;
        self.interacting = false;
        self.task = None;
        self.suppress_pointer_click = false;
        self.write_input(window, cx);
        cx.notify();
    }

    fn spinner(
        &self,
        button: BaseButton,
        action: StepAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> BaseButton {
        let at_limit = self.at_limit(action);
        let disabled = self.disabled || at_limit;
        let revealed = !self.disabled && (self.hovered || self.focused);
        let opacity = if self.reveal_spinners_on_hover {
            // The hover/focus selector has higher specificity than generic
            // `.icon.spinner.*.disabled`: even a bound arrow is fully opaque
            // while revealed. The bound guard still prevents a numeric step.
            // NumberInput retains the button hitbox even at that bound.
            let target = if revealed {
                1.
            } else if at_limit {
                0.3
            } else {
                0.
            };
            motion::transition(
                (
                    ElementId::from(("source-stepper", cx.entity_id())),
                    if action == StepAction::Increment {
                        "stepper-up-opacity"
                    } else {
                        "stepper-down-opacity"
                    },
                ),
                target,
                Transition::new(Duration::from_millis(100)).easing(Easing::Linear),
                window,
                cx,
            )
        } else if at_limit && !self.custom_keymapping {
            0.3
        } else {
            1.
        };
        let owner = cx.entity().downgrade();
        let down = owner.clone();
        let up = owner.clone();
        let outside = owner.clone();
        let leave = owner;
        button
            .w(surface::css(14.))
            .h(surface::css(12.))
            .p_0()
            .relative()
            .opacity(opacity)
            // `visibility 0s` switches immediately; retain geometry while hidden.
            .when(self.reveal_spinners_on_hover && !revealed, |button| {
                button.invisible()
            })
            .when(!disabled, |button| {
                button
                    .hover(|style| style.bg(Colors::spinner_hover()))
                    .active(|style| style.bg(Colors::spinner_pressed()))
            })
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                let _ = down.update(cx, |this, cx| {
                    this.suppress_pointer_click = true;
                    if !this.at_limit(action) {
                        this.press(action, window, cx);
                    }
                });
            })
            .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                let _ = up.update(cx, |this, _| this.task = None);
                // NumberInput installs its own click callback after decorating
                // the button. Consume that callback without taking a second
                // step, then clear any unconsumed marker after this dispatch.
                let reset = up.clone();
                cx.defer(move |cx| {
                    let _ = reset.update(cx, |this, _| this.suppress_pointer_click = false);
                });
            })
            .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
                let _ = outside.update(cx, |this, _| {
                    this.task = None;
                    this.suppress_pointer_click = false;
                });
            })
            .on_hover(move |hovered, _, cx| {
                if !*hovered {
                    let _ = leave.update(cx, |this, _| this.task = None);
                }
            })
            .child(
                img(if action == StepAction::Increment {
                    "synapse/wired-argb-3871-stepper_up.svg"
                } else {
                    "synapse/wired-argb-3871-stepper_down.svg"
                })
                .absolute()
                .left(surface::css(3.))
                .top(surface::css(if action == StepAction::Increment {
                    5.
                } else {
                    3.
                }))
                .w(surface::css(8.))
                .h(surface::css(4.)),
            )
    }
}

impl Render for Stepper {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus = self.input.focus_handle(cx);
        let owner = cx.entity().downgrade();
        let increment = self.spinner(
            BaseButton::new("increment"),
            StepAction::Increment,
            window,
            cx,
        );
        let decrement = self.spinner(
            BaseButton::new("decrement"),
            StepAction::Decrement,
            window,
            cx,
        );
        let opacity = if self.disabled { 0.3 } else { 1. };
        let opacity = if self.custom_keymapping {
            // The final .key-config .stepper shorthand replaces the generic
            // border transition with opacity .2s (default ease). Spinner
            // opacity stays 1 in every custom state; it has no changing target.
            surface::fade_opacity(self.id.clone(), opacity, 200, window, cx)
        } else {
            opacity
        };
        let root = div()
            .id(self.id.clone())
            .track_focus(&focus)
            .w(surface::css(if self.custom_keymapping {
                58.
            } else if self.modes_area {
                60.
            } else {
                62.
            }))
            .h(surface::css(if self.custom_keymapping {
                25.
            } else if self.modes_area {
                27.
            } else {
                26.
            }))
            .when(self.custom_keymapping, |root| {
                root.relative().mb(surface::css(10.))
            })
            .flex_shrink_0()
            .bg(Colors::background())
            .border_1()
            .border_color(
                if self.focused && !self.disabled && !self.custom_keymapping {
                    Colors::focus()
                } else {
                    Colors::border()
                },
            )
            .opacity(opacity)
            .on_hover(cx.listener(|this, hovered, _, cx| {
                if this.hovered != *hovered {
                    this.hovered = *hovered;
                    cx.notify();
                }
            }))
            .when(!self.disabled, |root| {
                root.hover(|style| style.border_color(Colors::focus()))
            })
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.disabled {
                    return;
                }
                if event.keystroke.key == "escape" {
                    window.blur(cx);
                    cx.stop_propagation();
                } else if this.custom_keymapping {
                    let action = match event.keystroke.key.as_str() {
                        "up" => StepAction::Increment,
                        "down" => StepAction::Decrement,
                        _ => return,
                    };
                    this.step_once(action, window, cx);
                    cx.stop_propagation();
                }
            }))
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, window, cx| {
                if this.disabled || !this.focused {
                    return;
                }
                let delta = event.delta.pixel_delta(px(1.)).y;
                if delta == px(0.) {
                    return;
                }
                let action = if delta > px(0.) {
                    StepAction::Increment
                } else {
                    StepAction::Decrement
                };
                // Wheel guards compare props.value with coercive >= / <=;
                // spinner classes instead compare the draft with strict ===.
                let wheel_at_limit = if this.allow_live_update && this.draft_from_typing {
                    let draft = this.input.read(cx).value();
                    let value = if draft.is_empty() {
                        0.
                    } else {
                        draft.parse::<f64>().unwrap_or(f64::NAN)
                    };
                    if action == StepAction::Increment {
                        value >= this.max
                    } else {
                        value <= this.min
                    }
                } else {
                    this.at_limit(action)
                };
                if !wheel_at_limit {
                    this.step_once(action, window, cx);
                }
                cx.stop_propagation();
            }));
        if self.custom_keymapping {
            // Source children are absolute in the 56 x 23 padding box. Its
            // root padding does not move them: input left 6, width 90%, and
            // independent 12px spinners at top/bottom (a 1px overlap).
            return root
                .child(
                    div()
                        .id("stepper-input")
                        .absolute()
                        .left(surface::css(6.))
                        .top_0()
                        .w(surface::css(50.4))
                        .h(surface::css(24.))
                        .on_click(cx.listener(|this, _, window, cx| {
                            if !this.disabled && !this.interacting {
                                this.interacting = true;
                                this.input
                                    .update(cx, |input, cx| input.select_all(window, cx));
                            }
                        }))
                        .child(
                            Input::new(&self.input)
                                .appearance(false)
                                .bordered(false)
                                .focus_bordered(false)
                                .size_full()
                                .p_0()
                                .text_size(surface::css(14.))
                                .line_height(surface::css(14.))
                                .text_color(Colors::text()),
                        ),
                )
                .child(
                    increment
                        .absolute()
                        .right_0()
                        .top_0()
                        .focusable(false)
                        .disabled(self.disabled || self.at_limit(StepAction::Increment)),
                )
                .child(
                    decrement
                        .absolute()
                        .right_0()
                        .bottom_0()
                        .focusable(false)
                        .disabled(self.disabled || self.at_limit(StepAction::Decrement)),
                )
                .into_any_element();
        }
        root.child(
            NumberInput::new(&self.input)
                .disabled(self.disabled)
                .size_full()
                .controls_right()
                .input(
                    div()
                        .id("stepper-input")
                        .on_click(cx.listener(|this, _, window, cx| {
                            if !this.disabled && !this.interacting {
                                this.interacting = true;
                                this.input
                                    .update(cx, |input, cx| input.select_all(window, cx));
                            }
                        }))
                        .child(
                            Input::new(&self.input)
                                .appearance(false)
                                .bordered(false)
                                .focus_bordered(false)
                                .h(surface::css(if self.modes_area { 25. } else { 24. }))
                                .w(surface::css(if self.modes_area { 42. } else { 44. }))
                                .p_0()
                                .pl(surface::css(if self.modes_area { 5. } else { 6. }))
                                .text_size(surface::css(14.))
                                .line_height(surface::css(if self.modes_area { 17. } else { 14. }))
                                .text_color(Colors::text()),
                        ),
                )
                .increment_button(move |_| increment)
                .decrement_button(move |_| decrement)
                .on_step(move |action, window, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        if std::mem::take(&mut this.suppress_pointer_click) {
                            return;
                        }
                        this.step_once(action, window, cx);
                    });
                }),
        )
        .into_any_element()
    }
}
