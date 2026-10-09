//! Current 3334/3337 uU -> SU/AU number editing; local draft submission only.
use super::*;
use gpui_kit::base::{Button as BaseButton, NumberInput, StepAction};
use gpui_kit::component::input::{Input, InputEvent, InputState, MaskPattern};
use std::{cell::Cell, rc::Rc, time::Duration};

pub(super) struct MixerNumber {
    input: Entity<InputState>,
    value: i32,
    typed: bool,
    registered: bool,
    enabled: bool,
    syncing: bool,
    max_len: Rc<Cell<usize>>,
    repeat: Option<Task<()>>,
    suppress_click: bool,
    _subscription: Subscription,
}
impl EventEmitter<i32> for MixerNumber {}

impl MixerNumber {
    fn new(value: i32, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let max_len = Rc::new(Cell::new(3));
        let limit = max_len.clone();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .mask_pattern(MaskPattern::None)
                .default_value(value.to_string())
                .validate(move |text, _| {
                    text.len() <= limit.get()
                        && text
                            .strip_prefix('-')
                            .unwrap_or(text)
                            .bytes()
                            .all(|b| b.is_ascii_digit())
                })
        });
        let subscription = cx.subscribe_in(&input, window, |this, _, event, window, cx| {
            if this.syncing {
                return;
            }
            match event {
                InputEvent::Change => {
                    this.typed = true;
                    let negative = this
                        .input
                        .read(cx)
                        .value()
                        .parse::<i32>()
                        .is_ok_and(|v| v < 0);
                    this.max_len.set(if negative { 4 } else { 3 });
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => window.blur(cx),
                InputEvent::Blur => {
                    this.registered = false;
                    this.repeat = None;
                    let value = this
                        .input
                        .read(cx)
                        .value()
                        .parse::<i32>()
                        .unwrap_or(0)
                        .clamp(0, 100);
                    this.replace_text(if this.enabled { value } else { this.value }, window, cx);
                    if this.enabled {
                        cx.emit(value);
                    }
                }
                _ => {}
            }
        });
        Self {
            input,
            value,
            typed: false,
            registered: false,
            enabled: false,
            syncing: false,
            max_len,
            repeat: None,
            suppress_click: false,
            _subscription: subscription,
        }
    }
    fn replace_text(&mut self, value: i32, window: &mut Window, cx: &mut Context<Self>) {
        self.typed = false;
        self.max_len.set(3);
        self.syncing = true;
        self.input.update(cx, |input, cx| {
            input.set_value(value.to_string(), window, cx)
        });
        self.syncing = false;
        cx.notify();
    }
    fn observe(
        &mut self,
        value: i32,
        enabled: bool,
        reset: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if reset || value != self.value {
            self.value = value;
            self.replace_text(value, window, cx);
            if reset || value == 0 || value == 100 {
                self.repeat = None;
            }
        }
        self.enabled = enabled;
        if reset || !enabled {
            self.repeat = None;
            self.registered = false;
        }
        cx.notify();
    }
    fn step(&mut self, action: StepAction, cx: &mut Context<Self>) {
        if !self.enabled {
            return;
        }
        let text = self.input.read(cx).value().to_string();
        // AU stores typed strings; integer increment uses JS + before parseInt.
        let value = match action {
            StepAction::Increment if self.typed => format!("{text}1").parse::<i32>().unwrap_or(0),
            StepAction::Increment => text.parse::<i32>().unwrap_or(0) + 1,
            StepAction::Decrement => text.parse::<i32>().unwrap_or(0) - 1,
        }
        .clamp(0, 100);
        cx.emit(value);
    }
    fn start_repeat(&mut self, action: StepAction, window: &mut Window, cx: &mut Context<Self>) {
        self.repeat = None;
        self.step(action, cx);
        self.repeat = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(300))
                    .await;
                if !this
                    .update_in(cx, |this, window, cx| {
                        if !this.enabled || !window.is_window_active() {
                            return false;
                        }
                        this.step(action, cx);
                        true
                    })
                    .unwrap_or(false)
                {
                    break;
                }
            }
        }));
    }
}

impl Render for MixerNumber {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let enabled = self.enabled;
        let typed = self.typed;
        let value = self.input.read(cx).value().parse::<i32>().unwrap_or(0);
        let owner = cx.entity().downgrade();
        let step_owner = owner.clone();
        let focus = self.input.focus_handle(cx);
        let arrow = move |button: BaseButton, action, asset, top, limit| {
            let down = owner.clone();
            let up = owner.clone();
            let out = owner.clone();
            let hover = owner.clone();
            let click = owner.clone();
            let button = button
                .w(surface::css(14.))
                .h(surface::css(12.))
                .p_0()
                .relative()
                .disabled(!enabled || limit)
                .when(limit, |b| b.opacity(0.3))
                .hover(|s| s.bg(rgba(0xffffff1a)))
                .active(|s| s.bg(rgba(0x0000001a)))
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    if enabled && !limit {
                        let _ = down.update(cx, |this, cx| this.start_repeat(action, window, cx));
                    }
                })
                .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                    let _ = up.update(cx, |this, _| this.repeat = None);
                })
                .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
                    let _ = out.update(cx, |this, _| this.repeat = None);
                })
                .on_hover(move |hovered, _, cx| {
                    if !hovered {
                        let _ = hover.update(cx, |this, _| this.repeat = None);
                    }
                })
                .child(
                    img(asset)
                        .absolute()
                        .left(surface::css(3.))
                        .top(surface::css(top))
                        .w(surface::css(8.))
                        .h(surface::css(4.)),
                );
            StatefulInteractiveElement::on_click(button, move |event, _, cx| {
                if matches!(event, ClickEvent::Mouse(_)) {
                    let _ = click.update(cx, |this, _| this.suppress_click = true);
                }
            })
        };
        let increment = arrow.clone();
        let input = self.input.clone();
        div()
            .id("mixer-number")
            .track_focus(&focus)
            .w(surface::css(62.))
            .h(surface::css(26.))
            .flex_shrink_0()
            .relative()
            .border_1()
            .border_color(rgb(0x5d5d5d))
            .bg(rgb(0x111111))
            .text_color(rgb(0xcccccc))
            .when(!enabled, |d| d.opacity(0.3))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.enabled && matches!(event.keystroke.key.as_str(), "escape" | "enter") {
                    window.blur(cx);
                    cx.stop_propagation();
                }
            }))
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                if !this.enabled || !this.registered {
                    return;
                }
                let delta = event.delta.pixel_delta(px(1.)).y;
                if delta > px(0.) && this.value < 100 {
                    this.step(StepAction::Increment, cx);
                }
                if delta < px(0.) && this.value > 0 {
                    this.step(StepAction::Decrement, cx);
                }
                cx.stop_propagation();
            }))
            .child(
                NumberInput::new(&self.input)
                    .disabled(!enabled)
                    .size_full()
                    .controls_right()
                    .input(
                        div()
                            .id("text")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if this.enabled && !this.registered {
                                    this.registered = true;
                                    input.update(cx, |input, cx| input.select_all(window, cx));
                                }
                            }))
                            .child(
                                Input::new(&self.input)
                                    .appearance(false)
                                    .bordered(false)
                                    .focus_bordered(false)
                                    .disabled(!enabled)
                                    .h(surface::css(24.))
                                    .w(surface::css(44.))
                                    .p_0()
                                    .pl(surface::css(6.))
                                    .text_size(surface::css(14.))
                                    .line_height(surface::css(14.)),
                            ),
                    )
                    .increment_button(move |b| {
                        increment(
                            b,
                            StepAction::Increment,
                            "synapse/stepper-up.svg",
                            5.,
                            !typed && value == 100,
                        )
                    })
                    .decrement_button(move |b| {
                        arrow(
                            b,
                            StepAction::Decrement,
                            "synapse/stepper-down.svg",
                            3.,
                            !typed && value == 0,
                        )
                    })
                    .on_step(move |action, _, cx| {
                        let _ = step_owner.update(cx, |this, cx| {
                            if std::mem::take(&mut this.suppress_click) {
                                return;
                            }
                            this.step(action, cx);
                        });
                    }),
            )
    }
}

impl AudioProductWorkspace {
    pub(super) fn add_mixer_number(
        &mut self,
        control: &AudioControl,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.spec.product_id, 3334 | 3337)
            || !matches!(
                control.path.as_str(),
                "/device/streamMixerSettings/streamMixVolume/value"
                    | "/device/streamMixerSettings/playbackMixVolume/value"
            )
        {
            return;
        }
        let value = self
            .draft
            .pointer(&control.path)
            .and_then(Value::as_f64)
            .unwrap_or(0.) as i32;
        let number = cx.new(|cx| MixerNumber::new(value, window, cx));
        let path = control.path.clone();
        self.subscriptions.push(cx.subscribe_in(
            &number,
            window,
            move |this, _, value: &i32, window, cx| {
                if this.page == "STREAM_MIXER_HEADER" {
                    this.edit(&path, json!(value), window, cx);
                }
            },
        ));
        self.mixer_numbers.insert(control.path.clone(), number);
    }
    pub(super) fn sync_mixer_numbers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.observe_mixer_numbers(false, window, cx);
    }
    pub(super) fn reset_mixer_numbers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.observe_mixer_numbers(true, window, cx);
    }
    fn observe_mixer_numbers(&self, reset: bool, window: &mut Window, cx: &mut Context<Self>) {
        for (path, number) in &self.mixer_numbers {
            let value = self
                .draft
                .pointer(path)
                .and_then(Value::as_f64)
                .unwrap_or(0.) as i32;
            let enabled = self.page == "STREAM_MIXER_HEADER"
                && self.control(path).is_some_and(|c| self.enabled(c));
            number.update(cx, |number, cx| {
                number.observe(value, enabled, reset, window, cx)
            });
        }
    }
}
