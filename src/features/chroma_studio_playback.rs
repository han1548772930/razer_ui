//! Current 5805 Playback transitions and 2245 cycles input.
use super::studio_dropdown::{DropdownState, StudioDropdown, arrow};
use super::*;
use gpui_kit::base::{NumberInput, StepAction};
use gpui_kit::component::input::MaskPattern;
use std::time::Duration;

#[derive(Deserialize)]
pub(super) struct PlaybackSource {
    start: Vec<String>,
    end: Vec<String>,
    labels: BTreeMap<String, String>,
    min: i64,
    max: i64,
}
pub(super) struct PlaybackChanged(pub(super) Value);
impl EventEmitter<PlaybackChanged> for StudioPlayback {}
pub(super) struct StudioPlayback {
    effect: String,
    start: String,
    end: String,
    cycles: i64,
    remembered: Option<i64>,
    enabled: bool,
    mounted: bool,
    focused: bool,
    input: Entity<InputState>,
    start_dropdown: Entity<DropdownState>,
    end_dropdown: Entity<DropdownState>,
    held: bool,
    repeated: bool,
    suppress_click: bool,
    repeat: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}
impl StudioPlayback {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).mask_pattern(MaskPattern::None));
        let start_dropdown = cx.new(|_| DropdownState::new());
        let end_dropdown = cx.new(|_| DropdownState::new());
        let subscriptions = vec![
            cx.observe(&start_dropdown, |_, _, cx| cx.notify()),
            cx.observe(&end_dropdown, |_, _, cx| cx.notify()),
            cx.subscribe_in(&input, window, |this, _, event, window, cx| match event {
                InputEvent::Focus => {
                    this.focused = true;
                    this.input
                        .update(cx, |input, cx| input.select_all(window, cx));
                    cx.notify();
                }
                InputEvent::Change if this.enabled && this.end == "after" => this.edit(window, cx),
                InputEvent::Blur => {
                    this.focused = false;
                    if this.enabled && this.end == "after" {
                        this.commit_cycles(window, cx);
                    }
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => window.blur(cx),
                _ => {}
            }),
            cx.observe_window_activation(window, |this, window, _| {
                if !window.is_window_active() {
                    this.repeat = None;
                    this.held = false;
                    this.suppress_click = false;
                }
            }),
        ];
        Self {
            effect: String::new(),
            start: String::new(),
            end: String::new(),
            cycles: -1,
            remembered: None,
            enabled: false,
            mounted: false,
            focused: false,
            input,
            start_dropdown,
            end_dropdown,
            held: false,
            repeated: false,
            suppress_click: false,
            repeat: None,
            _subscriptions: subscriptions,
        }
    }
    pub(super) fn configure(
        &mut self,
        effect: &str,
        params: &Value,
        enabled: bool,
        mounted: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let remount = self.effect != effect;
        self.effect = effect.into();
        if !mounted || remount {
            self.remembered = None;
        }
        self.mounted = mounted;
        self.enabled = enabled && mounted;
        self.start = params["playbackStart"].as_str().unwrap_or("").into();
        self.end = params["playbackEnd"].as_str().unwrap_or("").into();
        self.cycles = params["cycles"].as_i64().unwrap_or(-1);
        self.write_input(window, cx);
        if !self.enabled || remount {
            self.start_dropdown
                .update(cx, |s, cx| s.set_open(false, cx));
            self.end_dropdown.update(cx, |s, cx| s.set_open(false, cx));
            self.repeat = None;
            self.held = false;
            self.repeated = false;
            self.suppress_click = false;
        }
        cx.notify();
    }
    fn write_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| {
            input.set_value(self.cycles.to_string(), window, cx)
        });
    }
    fn publish(&self, patch: Value, cx: &mut Context<Self>) {
        cx.emit(PlaybackChanged(patch));
        cx.notify();
    }
    fn choose_start(&mut self, value: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.enabled {
            return;
        }
        self.start_dropdown
            .update(cx, |s, cx| s.set_open(false, cx));
        if self.start == value {
            return;
        }
        self.start = value.into();
        self.end = if value == "random" { "never" } else { "after" }.into();
        self.cycles = if value == "random" {
            -1
        } else {
            self.remembered.unwrap_or(1)
        };
        self.write_input(window, cx);
        self.publish(serde_json::json!({"playbackStart":self.start,"playbackEnd":self.end,"cycles":self.cycles}),cx);
    }
    fn choose_end(&mut self, value: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.enabled || (value == "toggle" && self.start == "random") {
            return;
        }
        self.end_dropdown.update(cx, |s, cx| s.set_open(false, cx));
        if self.end == value {
            return;
        }
        self.end = value.into();
        self.cycles = if value == "after" {
            self.remembered.unwrap_or(1)
        } else {
            -1
        };
        self.write_input(window, cx);
        self.publish(
            serde_json::json!({"playbackEnd":self.end,"cycles":self.cycles}),
            cx,
        );
    }
    fn edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let raw = self.input.read(cx).value().to_string();
        let digits: String = raw.chars().filter(char::is_ascii_digit).collect();
        let value = digits.bytes().fold(0_i64, |value, c| {
            (value * 10 + i64::from(c - b'0')).min(source().playback.max)
        });
        let text = if digits.is_empty() {
            String::new()
        } else {
            value.to_string()
        };
        if raw != text {
            self.input
                .update(cx, |input, cx| input.set_value(text, window, cx));
        }
        self.cycles = value;
        self.remembered = Some(value);
        // 5805:y publishes each input change with preview=false; native
        // preview is not available, but the working parameter is still live.
        self.publish(serde_json::json!({"cycles":value}), cx);
    }
    fn commit_cycles(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cycles = self
            .cycles
            .clamp(source().playback.min, source().playback.max);
        self.remembered = Some(self.cycles);
        self.write_input(window, cx);
        self.publish(serde_json::json!({"cycles":self.cycles}), cx);
    }
    fn step(&mut self, action: StepAction, window: &mut Window, cx: &mut Context<Self>) {
        if !self.enabled
            || self.end != "after"
            || (action == StepAction::Increment && self.cycles >= source().playback.max)
            || (action == StepAction::Decrement && self.cycles <= source().playback.min)
        {
            return;
        }
        self.cycles = (self.cycles
            + if action == StepAction::Increment {
                1
            } else {
                -1
            })
        .clamp(source().playback.min, source().playback.max);
        self.commit_cycles(window, cx);
    }
    fn press(&mut self, action: StepAction, window: &mut Window, cx: &mut Context<Self>) {
        if !self.enabled
            || self.end != "after"
            || (action == StepAction::Increment && self.cycles >= source().playback.max)
            || (action == StepAction::Decrement && self.cycles <= source().playback.min)
        {
            return;
        }
        self.held = true;
        self.repeated = false;
        self.suppress_click = true;
        self.repeat = Some(cx.spawn_in(window, async move |owner, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(300))
                .await;
            // 2245 marks timeout completion before the first interval tick.
            // Releasing between 300 and 400ms therefore does not step.
            if !matches!(
                owner.update_in(cx, |this, _, _| {
                    this.repeated = true;
                    this.held && this.enabled
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
                    if !this.held || !this.enabled {
                        return false;
                    }
                    this.repeated = true;
                    this.step(action, window, cx);
                    this.cycles > source().playback.min && this.cycles < source().playback.max
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
        let owner = cx.weak_entity();
        cx.defer(move |cx| {
            let _ = owner.update(cx, |this, _| this.suppress_click = false);
        });
    }
    fn spinner(
        &self,
        button: BaseButton,
        action: StepAction,
        cx: &mut Context<Self>,
    ) -> BaseButton {
        let owner = cx.weak_entity();
        let press = owner.clone();
        let release = owner.clone();
        let leave = owner.clone();
        button
            .p_0()
            .relative()
            .w(surface::css(16.))
            .bg(transparent_black())
            .hover(|style| style.bg(Colors::spinner_hover()))
            .when(
                (action == StepAction::Increment && self.cycles == source().playback.max)
                    || (action == StepAction::Decrement && self.cycles == source().playback.min),
                |view| view.opacity(0.3),
            )
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                let _ = press.update(cx, |this, cx| this.press(action, window, cx));
            })
            .on_mouse_up(MouseButton::Left, move |_, window, cx| {
                let _ = release.update(cx, |this, cx| this.release(action, window, cx));
            })
            .on_hover(move |hovered, window, cx| {
                if !hovered {
                    let _ = leave.update(cx, |this, cx| {
                        if this.held {
                            this.release(action, window, cx);
                        }
                    });
                }
            })
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
    fn menu(&self, start: bool, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let state = if start {
            &self.start_dropdown
        } else {
            &self.end_dropdown
        };
        let id = if start {
            "studio-playback-start"
        } else {
            "studio-playback-end"
        };
        let value = if start { &self.start } else { &self.end };
        let shown = if value == "toggle" {
            &self.start
        } else {
            value
        };
        let title = source()
            .playback
            .labels
            .get(shown)
            .map(|key| label(key))
            .unwrap_or_default();
        let options = if start {
            &source().playback.start
        } else {
            &source().playback.end
        };
        let body = div()
            .id((ElementId::from(id), "items"))
            .max_h(surface::css(110.))
            .scrollable_y()
            .children(
                options
                    .iter()
                    .filter(|option| start || option.as_str() != "toggle" || self.start != "random")
                    .map(|option| {
                        let shown = if option == "toggle" {
                            &self.start
                        } else {
                            option
                        };
                        let title = source()
                            .playback
                            .labels
                            .get(shown)
                            .map(|key| label(key))
                            .unwrap_or_default();
                        let selected = option == value;
                        let option = option.clone();
                        BaseButton::new((ElementId::from(id), SharedString::from(option.clone())))
                            .accessibility_label(title.clone())
                            .w_full()
                            .px(surface::css(6.))
                            .py(surface::css(4.))
                            .text_size(surface::css(14.))
                            .line_height(relative(1.36))
                            .justify_start()
                            .bg(transparent_black())
                            .text_color(if selected {
                                Colors::selected()
                            } else {
                                Colors::text()
                            })
                            .hover(|style| style.bg(Colors::menu_hover()))
                            .child(title)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if start {
                                    this.choose_start(&option, window, cx);
                                } else {
                                    this.choose_end(&option, window, cx);
                                }
                            }))
                    }),
            )
            .into_any_element();
        let open = state.read(cx).is_open();
        let trigger = BaseButton::new((ElementId::from(id), "trigger"))
            .accessibility_label(title.clone())
            .relative()
            .w_full()
            .h(surface::css(27.))
            .pl(surface::css(6.))
            .pr(surface::css(30.))
            .py_0()
            .justify_start()
            .bg(Colors::panel())
            .border_1()
            .border_color(if open {
                Colors::selected()
            } else {
                Colors::input_border()
            })
            .text_size(surface::css(14.))
            .child(title)
            .child(
                div()
                    .absolute()
                    .right(surface::css(10.))
                    .top(surface::css(10.))
                    .child(arrow(
                        open,
                        (ElementId::from(id), "arrow").into(),
                        window,
                        cx,
                    )),
            );
        StudioDropdown::new(id, state, self.enabled, trigger, body, 0., 0.)
            .block()
            .into_any_element()
    }
}
impl Render for StudioPlayback {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let increment = self.spinner(BaseButton::new("up"), StepAction::Increment, cx);
        let decrement = self.spinner(BaseButton::new("down"), StepAction::Decrement, cx);
        let owner = cx.weak_entity();
        let cycles = div()
            .flex()
            .items_center()
            .child(
                div()
                    .id("studio-cycles-input")
                    .w(surface::css(62.))
                    .h(surface::css(27.))
                    .border_1()
                    .border_color(if self.focused {
                        Colors::selected()
                    } else {
                        Colors::input_border()
                    })
                    .hover(|style| style.border_color(Colors::selected()))
                    .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        if event.keystroke.key == "escape" {
                            window.blur(cx);
                            cx.stop_propagation();
                        } else if matches!(
                            event.keystroke.key.as_str(),
                            "." | "+" | "-" | "e" | "E"
                        ) && !event.keystroke.modifiers.control
                            && !event.keystroke.modifiers.platform
                        {
                            window.prevent_default();
                            cx.stop_propagation();
                        } else if matches!(event.keystroke.key.as_str(), "up" | "down") {
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
                            cx.stop_propagation();
                        }
                    }))
                    .child(
                        NumberInput::new(&self.input)
                            .disabled(!self.enabled)
                            .controls_right()
                            .size_full()
                            .input(
                                Input::new(&self.input)
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
            .child(div().ml(surface::css(5.)).child(label("TEXT_TIMES")));
        div()
            .flex()
            .flex_col()
            .child(div().mb(surface::css(6.)).child(label("TEXT_START")))
            .child(
                div()
                    .mb(surface::css(10.))
                    .child(self.menu(true, window, cx)),
            )
            .child(div().mb(surface::css(6.)).child(label("TEXT_END")))
            .child(
                div()
                    .when(self.end == "after", |view| view.mb(surface::css(10.)))
                    .child(self.menu(false, window, cx)),
            )
            .when(self.end == "after", |view| view.child(cycles))
    }
}
