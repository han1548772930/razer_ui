//! Current Seiren module4230 numeric behavior, with 684's property/mic CSS.
use super::colors::Colors as C;
use gpui_kit::base::Button as BaseButton;
use gpui_kit::base::{NumberInput, StepAction};
use gpui_kit::component::input::{Input, InputEvent, InputState, MaskPattern};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_widgets::surface::css;
use std::time::Duration;

#[derive(Clone, Copy)]
pub(super) enum Presentation {
    Property,
    Monitoring,
}
#[derive(Clone)]
pub(super) struct Changed(pub f64);
impl EventEmitter<Changed> for Number {}

pub(super) struct Number {
    input: Entity<InputState>,
    value: f64,
    range: (f64, f64, f64),
    decimal: bool,
    presentation: Presentation,
    disabled: bool,
    focused: bool,
    hovered: bool,
    typed: bool,
    suppress_click: bool,
    repeat: Option<Task<()>>,
    _subscription: Subscription,
}
impl Number {
    pub fn new(
        value: f64,
        range: (f64, f64, f64),
        decimal: bool,
        length: usize,
        presentation: Presentation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .mask_pattern(MaskPattern::None)
                .default_value(value.to_string())
                .validate(move |value, _| valid_draft(value, decimal, length))
        });
        let subscription =
            cx.subscribe_in(&input, window, |this, _, event, window, cx| match event {
                InputEvent::Focus => {
                    this.focused = true;
                    cx.notify();
                }
                InputEvent::Blur => {
                    this.focused = false;
                    this.repeat = None;
                    this.commit(window, cx);
                }
                InputEvent::PressEnter { .. } => window.blur(cx),
                InputEvent::Change => {
                    this.typed = true;
                    cx.notify();
                }
            });
        Self {
            input,
            value,
            range,
            decimal,
            presentation,
            disabled: false,
            focused: false,
            hovered: false,
            typed: false,
            suppress_click: false,
            repeat: None,
            _subscription: subscription,
        }
    }
    pub fn sync(
        &mut self,
        value: f64,
        disabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let changed = self.value != value || self.disabled != disabled;
        if self.value != value {
            self.value = value;
            self.typed = false;
            self.input.update(cx, |input, cx| {
                input.set_value(value.to_string(), window, cx)
            });
        }
        self.disabled = disabled;
        if disabled {
            self.repeat = None;
        }
        self.input
            .update(cx, |input, cx| input.set_disabled(disabled, cx));
        if changed {
            cx.notify();
        }
    }
    pub fn cancel(&mut self) {
        self.repeat = None;
        self.suppress_click = false;
    }
    fn submit(&mut self, value: f64, window: &mut Window, cx: &mut Context<Self>) {
        self.value = value;
        self.typed = false;
        self.input.update(cx, |input, cx| {
            input.set_value(value.to_string(), window, cx)
        });
        cx.emit(Changed(value));
        cx.notify();
    }
    fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let value = parse_input(&self.input.read(cx).value(), self.range, self.decimal, true);
        self.submit(value, window, cx);
    }
    fn step(&mut self, action: StepAction, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let text = self.input.read(cx).value();
        let sign = if action == StepAction::Increment { 1. } else { -1. };
        // Integer volumeUp retains JS string + number concatenation after a
        // typed draft; volumeDown and decimal controls use numeric arithmetic.
        let draft = if !self.decimal && self.typed && action == StepAction::Increment {
            format!("{text}{}", self.range.2)
        } else {
            let next = text.parse::<f64>().unwrap_or(f64::NAN) + sign * self.range.2;
            if self.decimal {
                format!("{next:.3}")
            } else {
                next.to_string()
            }
        };
        let value = parse_input(&draft, self.range, self.decimal, false);
        self.submit(value, window, cx);
    }
    fn hold(&mut self, action: StepAction, window: &mut Window, cx: &mut Context<Self>) {
        self.repeat = None;
        self.step(action, window, cx);
        self.repeat = Some(cx.spawn_in(window, async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(300))
                    .await;
                if view
                    .update_in(cx, |this, window, cx| {
                        if this.disabled {
                            return false;
                        }
                        this.step(action, window, cx);
                        this.value
                            != if action == StepAction::Increment {
                                this.range.1
                            } else {
                                this.range.0
                            }
                    })
                    .unwrap_or(false)
                    == false
                {
                    break;
                }
            }
        }));
    }
    fn spinner(
        &self,
        button: BaseButton,
        action: StepAction,
        cx: &mut Context<Self>,
    ) -> BaseButton {
        let down = cx.entity().downgrade();
        let up = down.clone();
        let outside = down.clone();
        let leave = down.clone();
        let visible = matches!(self.presentation, Presentation::Property) && self.hovered;
        let limit = self.value
            == if action == StepAction::Increment {
                self.range.1
            } else {
                self.range.0
            };
        button
            .w(css(14.))
            .h(css(12.))
            .p_0()
            .relative()
            .opacity(if limit { 0.3 } else { 1. })
            .when(!visible, |button| button.invisible())
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                let _ = down.update(cx, |this, cx| {
                    this.suppress_click = true;
                    this.hold(action, window, cx);
                });
            })
            .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                let _ = up.update(cx, |this, _| this.repeat = None);
                let owner = up.clone();
                cx.defer(move |cx| {
                    let _ = owner.update(cx, |this, _| this.suppress_click = false);
                });
            })
            .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
                let _ = outside.update(cx, |this, _| this.cancel());
            })
            .on_hover(move |hovered, _, cx| {
                if !hovered {
                    let _ = leave.update(cx, |this, _| this.repeat = None);
                }
            })
            .child(
                img(if action == StepAction::Increment {
                    "synapse/wired-argb-3871-stepper_up.svg"
                } else {
                    "synapse/wired-argb-3871-stepper_down.svg"
                })
                .absolute()
                .left(css(3.))
                .top(css(if action == StepAction::Increment { 5. } else { 3. }))
                .w(css(8.))
                .h(css(4.)),
            )
    }
}
impl Render for Number {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let monitoring = matches!(self.presentation, Presentation::Monitoring);
        let owner = cx.entity().downgrade();
        let up = self.spinner(BaseButton::new("up"), StepAction::Increment, cx);
        let down = self.spinner(BaseButton::new("down"), StepAction::Decrement, cx);
        div()
            .id(("seiren-eq-number", cx.entity_id()))
            .track_focus(&self.input.focus_handle(cx))
            .w(css(if monitoring { 36. } else { 80. }))
            .h(css(if monitoring { 26. } else { 27. }))
            .flex_shrink_0()
            .border_1()
            .border_color(if self.focused { C::focus() } else { C::border() })
            .bg(C::surface())
            .opacity(if self.disabled { 0.3 } else { 1. })
            .on_hover(cx.listener(|this, hovered, _, cx| {
                this.hovered = *hovered;
                cx.notify();
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if !this.disabled && event.keystroke.key == "escape" {
                    window.blur(cx);
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
                if !(action == StepAction::Increment && this.value >= this.range.1
                    || action == StepAction::Decrement && this.value <= this.range.0)
                {
                    this.step(action, window, cx);
                }
                cx.stop_propagation();
            }))
            .child(
                NumberInput::new(&self.input)
                    .disabled(self.disabled)
                    .size_full()
                    .controls_right()
                    .input(
                        div()
                            .id("numeric-input")
                            .on_click(cx.listener(|this, _, window, cx| {
                                if !this.disabled {
                                    this.input
                                        .update(cx, |input, cx| input.select_all(window, cx));
                                }
                            }))
                            .child(razer_widgets::keyboard_navigation::focus_input(
                                Input::new(&self.input)
                                    .appearance(false)
                                    .bordered(false)
                                    .focus_bordered(false)
                                    .h(css(25.))
                                    .w(css(if monitoring { 34. } else { 64. }))
                                    .p_0()
                                    .when(monitoring, |input| input.text_center())
                                    .when(!monitoring, |input| input.pl(css(4.)))
                                    .text_size(css(14.))
                                    .line_height(css(14.))
                                    .text_color(C::text()),
                                &self.input,
                                cx,
                            )),
                    )
                    .increment_button(move |_| up)
                    .decrement_button(move |_| down)
                    .on_step(move |action, window, cx| {
                        let _ = owner.update(cx, |this, cx| {
                            if std::mem::take(&mut this.suppress_click) {
                                return;
                            }
                            this.step(action, window, cx);
                        });
                    }),
            )
    }
}
fn valid_draft(value: &str, decimal: bool, length: usize) -> bool {
    if value.len() > length + usize::from(value.starts_with('-')) {
        return false;
    }
    if matches!(value, "" | "-") {
        return true;
    }
    let value = value.strip_prefix('-').unwrap_or(value);
    let mut parts = value.split('.');
    let integer = parts.next().unwrap_or_default();
    !integer.is_empty()
        && integer.bytes().all(|b| b.is_ascii_digit())
        && parts.next().is_none_or(|fraction| {
            decimal && fraction.len() <= 3 && fraction.bytes().all(|b| b.is_ascii_digit())
        })
        && parts.next().is_none()
}
fn parse_input(text: &str, range: (f64, f64, f64), decimal: bool, blur: bool) -> f64 {
    let mut value = text
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .unwrap_or(0.);
    if !decimal {
        value = (value / range.2).ceil() * range.2;
    }
    value = value.clamp(range.0, range.1);
    if decimal && blur {
        let nearest = (value / range.2).round() * range.2;
        if (value - nearest).abs() > 1e-10 {
            value = (value / range.2).ceil() * range.2;
        }
    }
    value
}
#[cfg(test)]
mod tests {
    use super::*;
    #[core::prelude::v1::test]
    fn seiren_eq_numeric_keeps_intermediate_drafts_and_decimal_blur_snapping() {
        assert!(valid_draft("-", true, 5));
        assert!(valid_draft("1.", true, 5));
        assert!(!valid_draft("1.0000", true, 5));
        assert!(!valid_draft("1.", false, 5));
        assert_eq!(parse_input("-", (-12., 12., 0.1), true, true), 0.);
        assert!((parse_input("-1.234", (-12., 12., 0.1), true, true) + 1.2).abs() < 1e-9);
        assert_eq!(parse_input("13", (4., 40., 4.), false, true), 16.);
    }
}
