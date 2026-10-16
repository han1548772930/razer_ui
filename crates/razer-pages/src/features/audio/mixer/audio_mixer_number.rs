//! Current 1342 EU/sU input, including its inverted regex, JS string + 1,
//! live update, blur commit, and Enter/Escape blur behavior.
use super::audio_mixer_theme::Colors as C;
use gpui_kit::base::Button;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_widgets::surface;

#[derive(Clone, Copy, PartialEq)]
pub(super) struct Props {
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub decimals: bool,
    pub max_len: usize,
    pub tag: &'static str,
    pub input_visible: bool,
}

pub(super) struct Number {
    input: Entity<InputState>,
    props: Props,
    typed: bool,
    syncing: bool,
    force_sync: bool,
    text: String,
    _subscription: Subscription,
}
impl EventEmitter<f32> for Number {}

fn decimal_pattern(text: &str) -> bool {
    let mut split = text.split('.');
    let first = split.next().unwrap_or_default();
    !first.is_empty()
        && first.bytes().all(|c| c.is_ascii_digit())
        && split
            .next()
            .is_none_or(|tail| tail.len() <= 2 && tail.bytes().all(|c| c.is_ascii_digit()))
        && split.next().is_none()
}
fn parse(text: &str, props: Props) -> f32 {
    let text = text.trim_start();
    let mut value = if props.decimals {
        (1..=text.len())
            .rev()
            .filter(|i| text.is_char_boundary(*i))
            .find_map(|end| text[..end].parse::<f64>().ok())
            .unwrap_or(0.)
    } else {
        let (sign, digits) = if let Some(tail) = text.strip_prefix('-') {
            (-1., tail)
        } else {
            (1., text.strip_prefix('+').unwrap_or(text))
        };
        let (radix, digits) = if digits.starts_with("0x") || digits.starts_with("0X") {
            (16, &digits[2..])
        } else {
            (10, digits)
        };
        sign * digits
            .chars()
            .map_while(|ch| ch.to_digit(radix))
            .fold(0_f64, |value, digit| {
                value * f64::from(radix) + f64::from(digit)
            })
    };
    if value.is_nan() {
        value = 0.;
    }
    if props.decimals {
        value = (value * 100. + 0.5).floor() / 100.;
    }
    value.clamp(f64::from(props.min), f64::from(props.max)) as f32
}

impl Number {
    pub fn new(props: Props, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let text = props.value.to_string();
        let input = cx.new(|cx| InputState::new(window, cx).default_value(text.clone()));
        let subscription = cx.subscribe_in(&input, window, |this, input, event, window, cx| {
            if this.syncing {
                return;
            }
            match event {
                InputEvent::Change => {
                    let text = input.read(cx).value().to_string();
                    let rejects = text.encode_utf16().count() > this.props.max_len
                        || if this.props.decimals {
                            decimal_pattern(&text)
                        } else {
                            text.as_bytes()
                                .windows(3)
                                .any(|bytes| bytes.iter().all(u8::is_ascii_digit))
                        };
                    if rejects {
                        this.replace(this.text.clone(), window, cx);
                    } else {
                        this.text = text;
                        this.typed = true;
                        cx.emit(parse(&this.text, this.props));
                    }
                }
                InputEvent::Blur => {
                    let value = parse(&this.text, this.props);
                    this.force_sync = true;
                    this.typed = false;
                    this.replace(value.to_string(), window, cx);
                    cx.emit(value);
                }
                InputEvent::PressEnter { .. } => window.blur(cx),
                _ => {}
            }
            cx.notify();
        });
        Self {
            input,
            props,
            text,
            typed: false,
            syncing: false,
            force_sync: false,
            _subscription: subscription,
        }
    }
    fn replace(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        self.text = text.clone();
        self.syncing = true;
        self.input
            .update(cx, |input, cx| input.set_value(text, window, cx));
        self.syncing = false;
    }
    pub fn observe(&mut self, props: Props, window: &mut Window, cx: &mut Context<Self>) {
        if props.value != self.props.value || self.force_sync {
            self.force_sync = false;
            self.typed = false;
            self.replace(props.value.to_string(), window, cx);
        }
        self.props = props;
        cx.notify();
    }
    fn step(&mut self, up: bool, cx: &mut Context<Self>) {
        let text = if up && self.typed {
            format!("{}1", self.text)
        }
        // JS subtraction coerces the complete string to Number before
        // parseInput; it does not use parseFloat's prefix acceptance.
        else {
            (self
                .text
                .trim()
                .parse::<f64>()
                .unwrap_or_else(|_| if self.text.trim().is_empty() { 0. } else { f64::NAN })
                + if up { 1. } else { -1. })
            .to_string()
        };
        cx.emit(parse(&text, self.props));
    }
}
impl Render for Number {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("audio-mixer-source-number")
            .relative()
            .w(surface::css(100.))
            .h(surface::css(27.))
            .border_1()
            .border_color(C::border())
            .bg(C::surface())
            .hover(|style| style.border_color(C::selected()))
            .when(
                self.input.read(cx).focus_handle(cx).is_focused(_window),
                |element| element.border_color(C::selected()),
            )
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "up" => {
                        this.step(true, cx);
                        cx.stop_propagation();
                    }
                    "down" => {
                        this.step(false, cx);
                        cx.stop_propagation();
                    }
                    "escape" | "enter" => {
                        window.blur(cx);
                        cx.stop_propagation();
                    }
                    "." | "-" if !this.props.decimals => {
                        cx.stop_propagation();
                    }
                    _ => {}
                }
            }))
            .child(number_display(&self.text, self.props.tag))
            .when(self.props.input_visible, |element| {
                element.child(
                    div()
                        .id("audio-mixer-source-number-input")
                        .absolute()
                        .left(surface::css(6.))
                        .top_0()
                        .w_full()
                        .h(surface::css(24.))
                        .child(
                            Input::new(&self.input)
                                .appearance(false)
                                .bordered(false)
                                .focus_bordered(false)
                                .size_full()
                                .px(surface::css(10.))
                                .text_size(surface::css(14.))
                                .text_color(C::transparent()),
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.input.update(cx, |input, cx| {
                                input.focus(window, cx);
                                input.select_all(window, cx);
                            });
                        })),
                )
            })
            .children([true, false].into_iter().map(|up| {
                Button::new(if up { "mixer-number-up" } else { "mixer-number-down" })
                    .absolute()
                    .right_0()
                    .top(surface::css(if up { 0. } else { 12. }))
                    .w(surface::css(14.))
                    .h(surface::css(12.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .hover(|style| style.bg(C::pressed()))
                    .active(|style| style.bg(C::hover()))
                    .accessibility_label(if up { "Increase" } else { "Decrease" })
                    .child(
                        img(if up {
                            "synapse/audio-mixer-stepper-up.svg"
                        } else {
                            "synapse/audio-mixer-stepper-down.svg"
                        })
                        .size(surface::css(8.)),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.step(up, cx);
                    }))
            }))
    }
}

fn number_display(text: &str, tag: &str) -> Div {
    div()
        .absolute()
        .left(surface::css(11.))
        .top(surface::css(4.))
        .flex()
        .text_color(C::text())
        .text_size(surface::css(14.))
        .line_height(surface::css(17.))
        .child(div().child(format!("{text} {tag}")))
}

#[cfg(test)]
mod tests {
    use super::{Props, decimal_pattern, number_display, parse};
    use gpui_kit::{Display, Styled};
    #[test]
    fn source_number_display_keeps_flex_paragraph_without_forcing_wrap_or_width() {
        for text in ["0", "100", "-12.34"] {
            let mut display = number_display(text, "dB");
            let style = display.style();
            assert_eq!(style.display, Some(Display::Flex));
            assert!(style.size.width.is_none());
            assert!(style.text.white_space.is_none());
        }
    }
    #[test]
    fn current_decimal_regex_and_parsefloat_prefix_remain_distinct() {
        let props = Props {
            value: 0.,
            min: -45.,
            max: 0.,
            decimals: true,
            max_len: 6,
            tag: "dB",
            input_visible: true,
        };
        assert!(decimal_pattern("12.34"));
        assert!(!decimal_pattern("-12.34"));
        assert_eq!(parse("-12.34x", props), -12.34);
        assert_eq!(parse("-", props), 0.);
        assert_eq!(parse("-12.345", props), -12.34);
        assert_eq!(parse("-Infinity", props), -45.);
        let integer = Props {
            min: 0.,
            max: 100.,
            decimals: false,
            ..props
        };
        assert_eq!(parse("1e2", integer), 1.);
        assert_eq!(parse("0x20", integer), 32.);
    }
}
